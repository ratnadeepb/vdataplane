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

pub mod apis;
pub mod net;

use l3enginelib::apis::{Mbuf, Mempool, Memzone, Port, RingClientMap, eal_cleanup, eal_init};
use state::Storage;
use log;
use std::{sync::mpsc::{sync_channel, Receiver, SyncSender, TryRecvError}, vec, mem, ptr::NonNull};

// These three values need to be the same here and in the `l3packetiser` crate
const PACKETISER_ID: u16 = 1;
const PACKETISER_BURST: usize = 512;
const G_MEMPOOL_NAME: &str = "GLOBAL_MEMPOOL";
// const G_MEMPOOL_CAPACITY: usize = 512;
// const G_CACHE_SIZE: usize = 32;

const TX_QUEUE_CACHE_SZ: usize = 512;
const RX_BURST_MAX: usize = 32;
const TX_BURST_MAX: usize = 32;

/// A central mempool for all cores.
///
/// DPDK mempool is based on DPDK's lockless ring and thus thread-safe
pub static MEMPOOL: Storage<Mempool> = Storage::new();

/// Send/Receive packets to/fro the processing core
pub(crate) static PROC_CHANNEL: Storage<RingClientMap> = Storage::new();


pub const NUM_RX_THREADS: u16 = 1;
pub const NUM_TX_THREADS: u16 = 1;
const PROCESSOR_THREAD: u16 = 1; // ID of the process that will process the packets

fn handle_signal() {
	unimplemented!()
}

/// Function to receive packets from the NIC
fn rx_thread_main(receiver: Receiver<i8>, ports: Vec<Port>, rxq_id: u16) {
	let mut keep_running = true;
	match receiver.try_recv() {
		Ok(t) => {
			match t {
				-1 => keep_running = false,
				_ => log::error!("unknown message received in rx thread"),
			}
		},
		Err(e) => {
			match e {
				TryRecvError::Empty => {},
				TryRecvError::Disconnected => log::error!("sync channel disconnected in rx thread"),
			}
		}
	}
	
	while keep_running {
		for i in 0..ports.len() {
			let mut ptrs = Vec::with_capacity(RX_BURST_MAX);
			let rx_count = unsafe { dpdk_sys::_rte_eth_rx_burst(ports[i].id, rxq_id, ptrs.as_mut_ptr(), RX_BURST_MAX as u16) };
			if rx_count > 0 {
				if let Some(ch) = PROC_CHANNEL.try_get() {
					for ptr in ptrs.drain(..) {
						let pkt = Mbuf { raw: unsafe { NonNull::new_unchecked(ptr) }};
						match ch.send(PROCESSOR_THREAD, pkt) {
							Ok(_) => { },
							Err(e) => log::error!("failed to send packets to processing core: {}", e),
						}
					}
				}
			}
		}
	}
}

/// Function to send packets to the NIC
fn tx_thread_main(receiver: Receiver<i8>, ports: Vec<Port>, txq_id: u16) {
	let mut keep_running = true;
	match receiver.try_recv() {
		Ok(t) => {
			match t {
				-1 => keep_running = false,
				_ => log::error!("unknown message received in rx thread"),
			}
		},
		Err(e) => {
			match e {
				TryRecvError::Empty => {},
				TryRecvError::Disconnected => log::error!("sync channel disconnected in rx thread"),
			}
		}
	}
	while keep_running {
		let mut pkts = Vec::with_capacity(TX_BURST_MAX);
		let mempool = MEMPOOL.get(); // panics if MEMPOOL has not been set
		if let Some(ch) = PROC_CHANNEL.try_get() {
			for i in 0..TX_BURST_MAX {
				match Mbuf::new(mempool) {
					Ok(mb) => pkts.push(mb),
					Err(e) => {
						log::error!("unable to create mbuf: {}", e);
						return;
					}
				}
				match ch.receive(PROCESSOR_THREAD, &mut pkts[i]) {
					Ok(_) => {},
					Err(e) => {
						log::error!("failed to receive packets from the processing core: {}", e);
						pkts.pop(); // remove the last buffer from the vector
						break; // no more packets to receive
					}
				}
			}
		}
		if pkts.len() > 0 {
			let mut ptrs = Vec::with_capacity(TX_BURST_MAX);
			for pkt in pkts {
				ptrs.push(pkt.into_ptr());
			}
			// REVIEW: As of now sending out of the first port always
			let tx_count = unsafe { dpdk_sys::_rte_eth_tx_burst(ports[0].id, txq_id, ptrs.as_mut_ptr(), TX_BURST_MAX as u16) };
			if tx_count == 0 {
				log::info!("no packets sent out");
			}
		}
	}
}

// DEVFLAGS: development flags - remove in production
#[allow(unused_variables)]
#[allow(while_true)]
fn main() {
	log::info!("Initializing DPDK env ...");
	// NOTE: hardcoded for now
	// later get a python script scan the system and populate a config file
	let args = vec![String::from("-l 0-1"), String::from("-n 4"), String::from("--proc-type=primary"), String::from("--"), String::from("-p 3"), String::from("-n 2")];
	println!("main process args: {:?}", &args); // debug
	#[cfg(debug)]
	println!("main process args: {:?}", &args);
	eal_init(args).unwrap();

	#[cfg(feature = "debug")]
	println!("environment initialised"); // debug

	// NOTE: once again hardcoded
	// let cores = [0..=3].iter().collect::<Vec<_>>();
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
	// let eth_devs = vec!["0000:07:00.0", "0000:07:00.1"];
	let eth_devs = vec!["port0", "port1"];
	for id in 0..=1 {
		Port::new(eth_devs[id], id as u16).unwrap().configure(NUM_RX_THREADS, NUM_TX_THREADS).unwrap();
	}
	
	#[cfg(feature = "debug")]
	println!("ports set"); // debug

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

	while true {

	}

	#[cfg(feature = "debug")]
	println!("main process stopping");
	eal_cleanup().unwrap();
	log::info!("stopped main process");
	#[cfg(feature = "debug")]
	println!("main process stopped");
}
