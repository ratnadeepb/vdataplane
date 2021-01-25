//! This is the starting point of our engine
//!
//! The main function starts the primary DPDK process

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

mod rxbin;
mod txbin;

use crossbeam_queue::SegQueue;
use l3enginelib::apis::{eal_cleanup, eal_init, Mbuf, Mempool, Memzone, Port, RingClientMap};
use libc::{IFF_BROADCAST, IFF_ECHO, IFF_PROMISC, IFF_UP};
use log;
// use pnet::ipnetwork::{IpNetwork, Ipv4Network};
use rxbin::{get_external_pkts, get_from_packetiser};
// use smoltcp::wire::Ipv4Address;
use state::Storage;
use std::{
	cell::Cell,
	mem,
	// net::Ipv4Addr,
	ptr::NonNull,
	sync::mpsc::{sync_channel, Receiver, SyncSender, TryRecvError},
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
	time::Duration,
	vec,
};
use txbin::{send_pkts_out, send_to_packetiser};
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

/// Unbounded queue to hold packets that are to sent immediately out
pub static OUT_PKTS: Storage<SegQueue<Mbuf>> = Storage::new();
/// Unbounded queue to hold packets that are to sent to the packetiser
pub static TO_PACKETISER: Storage<SegQueue<Mbuf>> = Storage::new();
/// Unbounded queue to hold packets that are coming from the packetiser
pub static FROM_PACKETISER: Storage<SegQueue<Mbuf>> = Storage::new();

pub const NUM_RX_THREADS: usize = 1;
pub const NUM_TX_THREADS: usize = 1;
const PROCESSOR_THREAD: u16 = 1; // ID of the process that will process the packets

fn handle_signal(kr: Arc<AtomicBool>) {
	ctrlc::set_handler(move || {
		kr.store(false, Ordering::SeqCst);
	})
	.expect("Error setting Ctrl-C handler");
}

// pub fn print_mac_addrs(ports: &Vec<Port>) {
// 	for port in ports {
// 		println!("Port {}: {:?}", port.id, port.mac_addr().unwrap());
// 	}
// }

fn rx_thread_main(kr: Arc<AtomicBool>, ports: Vec<Port>) {
	while kr.load(Ordering::SeqCst) {
		// get packets from outside
		let _rx_sz = get_external_pkts(&ports);
		#[cfg(feature = "debug")]
		if _rx_sz > 0 {
			println!("received: {} pkt(s)", _rx_sz);
		}
		// send packets to the packetiser
		send_to_packetiser();
	}
}

fn tx_thread_main(kr: Arc<AtomicBool>, ports: Vec<Port>) {
	while kr.load(Ordering::SeqCst) {
		// get packets from packetiser
		get_from_packetiser();

		// send all outgoing packets
		let _tx_sz = send_pkts_out(&ports);
		#[cfg(feature = "debug")]
		if _tx_sz > 0 {
			println!("sent: {} pkt(s)", _tx_sz);
		}
	}
}

#[allow(while_true)]
fn main() {
	log::info!("Initializing DPDK env ...");
	// NOTE: hardcoded for now
	// later get a python script scan the system and populate a config file
	let args = vec![
		String::from("-l 0-1"),
		String::from("-n 4"),
		String::from("--proc-type=primary"),
		String::from("--base-virtaddr=0x7f000000000"),
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
	let cur_core = unsafe { dpdk_sys::_rte_lcore_id() };

	log::info!("setup mempool");
	let mempool;
	match Mempool::new(G_MEMPOOL_NAME) {
		Ok(mp) => {
			#[cfg(feature = "debug")]
			println!("mempool address: {:p}", mp.get_ptr());
			mempool = mp;
		}
		Err(e) => panic!("Failed to initialize mempool: {}", e),
	}
	MEMPOOL.set(mempool);
	#[cfg(feature = "debug")]
	println!("mempool set");

	log::info!("setup ports");
	// let eth_devs = vec!["port0", "port1"];
	let eth_devs = vec!["port0"];
	let mut ports: Vec<Port> = Vec::new();
	{
		let mempool = MEMPOOL.get();
		let mut p = Port::new(eth_devs[0], 0u16).unwrap();
		p.configure(cores.len() as u16, mempool).unwrap();
		p.start().unwrap();
		ports.push(p);
		// for id in 0..=1 {
		// 	let mut p = Port::new(eth_devs[id], id as u16).unwrap();
		// 	p.configure(cores.len() as u16, mempool).unwrap();
		// 	p.start().unwrap();
		// 	ports.push(p);
		// }
	} // lock on MEMPOOL released

	#[cfg(feature = "debug")]
	{
		println!("ports set");
		// print_mac_addrs(&ports);
	}

	// let mut server = Server::new();
	// let ip_addr1 = Ipv4Addr::new(10, 10, 1, 1);
	// let prefix = 24;
	// let ip_nw1 = Ipv4Network::new(ip_addr1, prefix).unwrap();
	// let ip_addr2 = Ipv4Address::new(10, 10, 1, 2);
	// let mac1 = ports[0].mac_addr().unwrap();
	// let mac1 = ports[0].mac_addr().unwrap().to_ethernetaddr();
	// let mac2 = ports[1].mac_addr().unwrap().to_ethernetaddr();
	// server.add(
	// 	"iface1",
	// 	"test interface",
	// 	0,
	// 	Some(mac1),
	// 	vec![IpNetwork::V4(ip_nw1)],
	// 	(IFF_UP | IFF_BROADCAST | IFF_PROMISC | IFF_ECHO) as u32,
	// );
	// server.add(ip_addr1, mac1);
	// server.add(ip_addr2, mac2);
	// let macs = [ports[0].mac_addr().unwrap().to_ethernetaddr(),
	// ports[1].mac_addr().unwrap().to_ethernetaddr()].to_vec();
	// server.add_macs(ip_addr, macs);
	// #[cfg(feature = "debug")]
	// println!("Server IP and Macs: {:?}", server);

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

	// set PROC_CHANNEL
	PROC_CHANNEL.set(ringmap);
	// packets to be sent out
	OUT_PKTS.set(SegQueue::new());
	// packets to be sent to the packetiser
	TO_PACKETISER.set(SegQueue::new());
	// packets to be received from the packetiser
	FROM_PACKETISER.set(SegQueue::new());

	// handling Ctrl+C
	let keep_running = Arc::new(AtomicBool::new(true));
	let kr = keep_running.clone();
	handle_signal(keep_running.clone());

	#[cfg(feature = "debug")]
	println!("main: secondary started");
	// secondary has started up; start processing packets
	while kr.load(Ordering::SeqCst) {
		// get packets from outside
		let _rx_sz = get_external_pkts(&ports);
		#[cfg(feature = "debug")]
		if _rx_sz > 0 {
			println!("received: {} pkts", _rx_sz);
		}
		// send packets to the packetiser
		send_to_packetiser();
		// get packets from packetiser
		get_from_packetiser();

		// send all outgoing packets
		let _tx_sz = send_pkts_out(&ports);
		#[cfg(feature = "debug")]
		if _tx_sz > 0 {
			println!("sent: {} pkts", _tx_sz);
		}
	}

	// dpdk_sys::rte_eal_remote_launch(
	// 	Some(rx_thread_main),
	// 	&[kr.clone(), &ports, &server],
	// 	cur_core,
	// );

	#[cfg(feature = "debug")]
	println!("main: stopping");
	unsafe { dpdk_sys::_pkt_stop_and_close_ports() };
	#[cfg(feature = "debug")]
	println!("main: ports closed");
	let mempool = MEMPOOL.get();
	#[cfg(feature = "debug")]
	println!("main: mempool cleaned");
	eal_cleanup(mempool).unwrap();
}
