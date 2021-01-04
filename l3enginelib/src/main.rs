/*
 * Created on Fri Dec 25 2020:15:42:58
 * Created by Ratnadeep Bhattacharya
 */

// production flags
// #![warn(
//     missing_docs,
//     rust_2018_idioms,
//     missing_debug_implementations,
//     broken_intra_doc_links
// )]
// #![allow(clippy::type_complexity)]

// DEVFLAGS: development flags - remove in production
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use l3enginelib::{
	apis::{eal_cleanup, eal_init, Mbuf, Mempool, Memzone, Port, RingClientMap},
	net::MacAddr,
	server::Server,
};
use log;
use smoltcp::wire::Ipv4Address;
use state::Storage;
use std::{
	mem,
	ptr::NonNull,
	sync::mpsc::{sync_channel, Receiver, SyncSender, TryRecvError},
	thread::sleep,
	time::Duration,
	vec,
};
use zmq::Context;

// These three values need to be the same here and in the `l3packetiser` crate
const PACKETISER_ID: u16 = 1;
const PACKETISER_BURST: usize = 512;
const G_MEMPOOL_NAME: &str = "GLOBAL_MEMPOOL";

const TX_QUEUE_CACHE_SZ: usize = 512;
const RX_BURST_MAX: usize = 32;
const TX_BURST_MAX: usize = 32;

const PACKETISER_ZMQ_PORT: &str = "tcp://*:5555";

/// A central mempool for all cores.
///
/// DPDK mempool is based on DPDK's lockless ring and thus thread-safe
pub static MEMPOOL: Storage<Mempool> = Storage::new();

/// Send/Receive packets to/fro the processing core
pub(crate) static PROC_CHANNEL: Storage<RingClientMap> = Storage::new();

pub const NUM_RX_THREADS: usize = 1;
pub const NUM_TX_THREADS: usize = 1;
const PROCESSOR_THREAD: u16 = 1; // ID of the process that will process the packets

fn handle_signal() {
	unimplemented!()
}

/// Function to receive packets from the NIC
fn rx_packetiser(ports: &Vec<Port>, rxq_id: u16) -> usize {
	let mempool = MEMPOOL.get();
	let mut count = 0;
	for i in 0..ports.len() {
		let rx_pkts = ports[i].receive(mempool, rxq_id);
		if rx_pkts.len() > 0 {
			count += rx_pkts.len();
			if let Some(ch) = PROC_CHANNEL.try_get() {
				for pkt in rx_pkts {
					match ch.send(PROCESSOR_THREAD, pkt) {
						Ok(_) => {}
						Err(e) => log::error!("failed to send packets to processing core: {}", e),
					}
				}
			}
		}
	}
	count
}

/// Function to send packets to the NIC
fn tx_packetiser(ports: &Vec<Port>, txq_id: u16) -> usize {
	let mut pkts = Vec::with_capacity(TX_BURST_MAX);
	let mempool = MEMPOOL.get();
	if let Some(ch) = PROC_CHANNEL.try_get() {
		for i in 0..TX_BURST_MAX {
			match Mbuf::new(mempool) {
				Ok(mb) => pkts.push(mb),
				Err(e) => {
					log::error!("unable to create mbuf: {}", e);
					return 0;
				}
			}
			match ch.receive(PROCESSOR_THREAD, &mut pkts[i]) {
				Ok(_) => {}
				Err(e) => {
					log::error!("failed to receive packets from the processing core: {}", e);
					pkts.pop(); // remove the last buffer from the vector
					break; // no more packets to receive
				}
			}
		}
	}
	let count = pkts.len();
	if count > 0 {
		// REVIEW: As of now sending out of the first port always
		let tx_count = ports[0].send(pkts, txq_id);
		if tx_count == 0 {
			log::info!("no packets sent out");
		}
	}
	count
}

fn external_pkt_processing(ports: &Vec<Port>, server: &Server) -> (usize, usize) {
	let queue_id = unsafe { dpdk_sys::_rte_lcore_id() as u16 };
	let mempool = MEMPOOL.get();
	let mut pkts: Vec<Mbuf> = Vec::new();
	for i in 0..ports.len() {
		let mut rx_pkts = ports[i].receive(mempool, queue_id);
		pkts.extend(rx_pkts.drain(..));
	}

	let rx_count = pkts.len();
	// detect and send arp response
	let mp = MEMPOOL.get();
	for pkt in &mut pkts {
		#[cfg(feature = "debug")]
		{
			let ether_hdr = unsafe { dpdk_sys::_pkt_ether_hdr(pkt.get_ptr()) };
			if !ether_hdr.is_null() {
				// println!("packet: {}", unsafe { (*ip_hdr).next_proto_id });
				// println!("packet ether type: {:?}", unsafe {
				// 	(*ether_hdr).ether_type
				// });
			}
		}

		if let Some(ip) = server.detect_arp(pkt) {
			println!("main: arp packet for ip: {:?}", ip);
			if let Some(out_arp) = server.send_arp_reply(pkt, mp) {
				let tx_count = ports[0].send(vec![out_arp], queue_id);
				println!("main: arp packet for ip: {:?}", ip);
			}
		}

		if let Some(out_icmp) = server.icmp_reply(pkt, mp) {
			let tx_count = ports[0].send(vec![out_icmp], queue_id);
			println!("main: icmp packet");
		}
	}

	let tx_count = ports[0].send(pkts, queue_id);
	// let rx_count = rx_packetiser(ports, queue_id);
	// let tx_count = tx_packetiser(ports, queue_id);
	#[cfg(feature = "debug")]
	if rx_count > 0 || tx_count > 0 {
		// println!(
		// 	"Received {} packets and sent out {} packets",
		// 	rx_count, tx_count
		// );
	}
	(rx_count, tx_count)
}

pub fn print_mac_addrs(ports: &Vec<Port>) {
	for port in ports {
		println!("Port {}: {:?}", port.id, port.mac_addr().unwrap());
	}
}

#[allow(while_true)]
fn main() {
	log::info!("Initializing DPDK env ...");
	// NOTE: hardcoded for now
	// later get a python script scan the system and populate a config file
	let args = vec![
		String::from("-lcores=\"(0-1)@0\""),
		String::from("-n 4"),
		String::from("--proc-type=primary"),
		String::from("--"),
		String::from("-p 3"),
		String::from("-n 2"),
	];
	println!("main process args: {:?}", &args); // debug
	#[cfg(debug)]
	println!("main process args: {:?}", &args);
	eal_init(args).unwrap();

	#[cfg(feature = "debug")]
	println!("environment initialised"); // debug

	// NOTE: once again hardcoded
	let cores = (0..2).collect::<Vec<u32>>();
	// let num_cores = unsafe { dpdk_sys::rte_lcore_count() } as u16;
	// let cur_core = unsafe { dpdk_sys::_rte_lcore_id() };
	// let rx_cores = NUM_RX_THREADS;
	// let tx_cores = num_cores - rx_cores;
	// let socket_id = unsafe { dpdk_sys::rte_socket_id() } as i32;

	log::info!("setup mempool");
	let mempool;
	match Mempool::new(G_MEMPOOL_NAME) {
		Ok(mp) => mempool = mp,
		Err(e) => panic!("Failed to initialize mempool: {}", e),
	}
	MEMPOOL.set(mempool);
	#[cfg(feature = "debug")]
	println!("mempool set");

	log::info!("setup ports");
	let eth_devs = vec!["port0", "port1"];
	let mut ports: Vec<Port> = Vec::new();
	{
		let mempool = MEMPOOL.get();
		for id in 0..=1 {
			let mut p = Port::new(eth_devs[id], id as u16).unwrap();
			p.configure(cores.len() as u16, mempool).unwrap();
			p.start().unwrap();
			ports.push(p);
		}
	} // lock on MEMPOOL released

	#[cfg(feature = "debug")]
	{
		println!("ports set");
		print_mac_addrs(&ports);
	}

	let mut server = Server::new();
	let ip_addr1 = Ipv4Address::new(10, 10, 2, 1);
	let ip_addr2 = Ipv4Address::new(10, 10, 1, 2);
	let mac1 = ports[0].mac_addr().unwrap().to_ethernetaddr();
	let mac2 = ports[1].mac_addr().unwrap().to_ethernetaddr();
	server.add(ip_addr1, mac1);
	server.add(ip_addr2, mac2);
	// let macs = [ports[0].mac_addr().unwrap().to_ethernetaddr(),
	// ports[1].mac_addr().unwrap().to_ethernetaddr()].to_vec();
	// server.add_macs(ip_addr, macs);
	#[cfg(feature = "debug")]
	println!("Server IP and Macs: {:?}", server);

	let memzone = Memzone::new("TEST_MEMZONE", mem::size_of::<dpdk_sys::rte_mbuf>() * 10).unwrap();

	#[cfg(feature = "debug")]
	{
		println!("test memzone set"); // debug
		println!("Test memzone addr: {}", memzone.virt_addr());
	}

	let ringmap = RingClientMap::new();
	#[cfg(feature = "debug")]
	println!("main process: created ringmap");
	ringmap.add_client(PACKETISER_ID).unwrap(); // fatal error
	#[cfg(debug)]
	println!("main process: added packetiser to ringmap");

	// wait for the secondary process to start up
	#[cfg(feature = "debug")]
	println!("main: waiting for secondary");
	let context = Context::new();
	let responder = context.socket(zmq::REP).unwrap(); // fatal error
	assert!(responder.bind(PACKETISER_ZMQ_PORT).is_ok());
	let mut msg = zmq::Message::new();
	responder.recv(&mut msg, 0).unwrap();

	#[cfg(feature = "debug")]
	println!("main: secondary started");
	// secondary has started up; start processing packets
	while true {
		// #[cfg(feature = "debug")]
		// println!("main: starting rx/tx out of ports");
		let _ = external_pkt_processing(&ports, &server);
		// break;
	}

	#[cfg(feature = "debug")]
	println!("main: stopping");
	let mempool = MEMPOOL.get();
	eal_cleanup(mempool).unwrap();
	log::info!("main: stopped");
	#[cfg(feature = "debug")]
	println!("main: stopped");
}
