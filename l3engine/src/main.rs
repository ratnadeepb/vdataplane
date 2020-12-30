/*
 * Created on Fri Dec 25 2020:15:42:58
 * Created by Ratnadeep Bhattacharya
 */

pub mod apis;
pub mod net;

use apis::{Channel, Mbuf, Mempool, Port, eal_init};
use state::Storage;
use log;
use std::{sync::mpsc::{sync_channel, Receiver, SyncSender, TryRecvError}, vec};
use crossbeam_queue::ArrayQueue;

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
pub(crate) static PROC_CHANNEL: Storage<Channel> = Storage::new();


pub const NUM_RX_THREADS: u16 = 1;
pub const NUM_TX_THREADS: u16 = 1;

fn handle_signal() {
	unimplemented!()
}

fn process_incoming_pkts(pkts: &Vec<*mut dpdk_sys::rte_mbuf>) {
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
					match ch.send_to_processor(ptrs) {
						Ok(_) => {},
						Err(e) => log::error!("failed to send packets to processing core: {}", e),
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
		let mut ptrs = Vec::with_capacity(TX_BURST_MAX);
		if let Some(ch) = PROC_CHANNEL.try_get() {
			match ch.recv_from_processor(&mut ptrs) {
				Ok(_) => {},
				Err(e) => log::error!("failed to send packets to processing core: {}", e),
			}
		}
		if ptrs.len() > 0 {
			// REVIEW: As of now sending out of the first port always
			let tx_count = unsafe { dpdk_sys::_rte_eth_tx_burst(ports[0].id, txq_id, ptrs.as_mut_ptr(), TX_BURST_MAX as u16) };
			if tx_count == 0 {
				log::info!("no packets sent out");
			}
		}
	}
}


fn main() {
	log::info!("Initializing DPDK env ...");
	// NOTE: hardcoded for now
	// later get a python script scan the system and populate a config file
	// let args = vec![String::from("-l 0-3"), String::from("-n 4"), String::from("--proc-type=primary"), String::from("--allow-option 0000:07:00.0"), String::from("--allow-option 0000:07:00.1")];
	let args = vec![String::from("-l 0-3"), String::from("-n 4"), String::from("--proc-type=primary")];
	eal_init(args).unwrap();

	// NOTE: once again hardcoded
	// let cores = [0..=3].iter().collect::<Vec<_>>();
	// let num_cores = unsafe { dpdk_sys::rte_lcore_count() } as u16;
	// let cur_core = unsafe { dpdk_sys::_rte_lcore_id() };
	// let rx_cores = NUM_RX_THREADS;
	// let tx_cores = num_cores - rx_cores;
	let socket_id = unsafe { dpdk_sys::rte_socket_id() } as i32;

	log::info!("setup mempool");
	let mempool;
	// match Mempool::new(String::from(G_MEMPOOL_NAME), G_MEMPOOL_CAPACITY, G_CACHE_SIZE, socket_id) {
	match Mempool::new(G_MEMPOOL_NAME) {
		Ok(mp) => mempool = mp,
		Err(e) => panic!("Failed to initialize mempool: {}", e),
	}
	MEMPOOL.set(mempool);
	#[cfg(feature = "debug")]
	println!("mempool set"); // debug

	log::info!("setup ports");
	let eth_devs = vec!["0000:07:00.0", "0000:07:00.1"];
	for id in 0..=1 {
		Port::new(eth_devs[id], id as u16).unwrap().configure().unwrap();
	}
	#[cfg(feature = "debug")]
	println!("ports set"); // debug
}
