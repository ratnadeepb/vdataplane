//! This module defines a routing table and the packetiser struct
//!
//! The RoutingTable maintains a map between client IDs and IPs assigned to them
//!
//! The Mux runs the secondary DPDK thread.
//! This thread gets packets from the primary
//! This thread is responsible for
//! registering clients
//! and routing packets to and from clients
//!
//! The design is that the main DPDK process only handles incoming and outgoing packets
//! Incoming packets are handed over to a secondary DPDK process
//! This module performs the basic processing and sends them over to the clients
//! It will also receive the packets from the clients and send them to main process
//!
//! The Mux uses crossbeam MPMC ArrayQueues but the underlying Ring APIs use vectors.
//! So recv and transmit functions here perform a conversion
//! Typically, the alternative is using a lock - std::RWLock or crossbeam::AtomicCell and so on

mod fivetuple;

pub(crate) use fivetuple::*;
use l3enginelib::apis::eal_init;

use anyhow::Result;
use crossbeam::queue::ArrayQueue;
use l3enginelib::{Channel, Mbuf, MemoryError, Mempool};

pub(crate) struct Mux {
	channel: Channel,                     // communicating with the engine
	mempool: Mempool,                     // the memory pool for Mbufs
	pub(crate) in_buf: ArrayQueue<Mbuf>,  // hold the incoming packets
	pub(crate) out_buf: ArrayQueue<Mbuf>, // hold the outgoing packets
}

impl<'a> Mux {
	const G_MEMPOOL_NAME: &'a str = "GLOBAL_MEMPOOL";
	const BURST_SZ: usize = 512;

	pub(crate) fn new() -> Option<Self> {
		#[cfg(feature = "debug")]
		println!("starting mux");
		let channel = Channel::lookup().ok()?;
		#[cfg(feature = "debug")]
		{
			println!("found channel");
			println!(
				"packetiser: new : client_to_engine found {:p}",
				channel.to_engine.get_ptr()
			);
			println!(
				"packetiser: new : engine_to_client found {:p}",
				channel.to_packetiser.get_ptr()
			);
		}
		let mempool = Mempool::lookup(Self::G_MEMPOOL_NAME).ok()?;
		#[cfg(feature = "debug")]
		println!("found mempool, address: {:p}", mempool.get_ptr());
		let in_buf = ArrayQueue::new(Self::BURST_SZ);
		let out_buf = ArrayQueue::new(Self::BURST_SZ);
		Some(Mux {
			channel,
			mempool,
			in_buf,
			out_buf,
		})
	}

	pub(crate) fn mempool(&self) -> &Mempool {
		&self.mempool
	}

	pub(crate) fn recv(&self, buf: &mut Vec<u8>) -> Result<(), MemoryError> {
		let mut pkt = match Mbuf::from_bytes(&buf[..], &self.mempool) {
			Ok(pkt) => pkt,
			Err(_) => return Err(MemoryError::NoMem),
		};
		self.channel.receive_from_engine(&mut pkt)
	}

	pub(crate) fn recv_from_engine_burst(&self) -> usize {
		let len = self.in_buf.capacity() - self.in_buf.len();
		let mut pkts = Vec::with_capacity(len);
		for k in 0..len {
			match Mbuf::new(&self.mempool) {
				Ok(buf) => pkts.push(buf),
				Err(_) => {
					pkts.truncate(k);
					break;
				}
			}
		}
		let sz = self.channel.recv_from_engine_burst(&mut pkts, len);
		pkts.truncate(sz);
		pkts.drain(..)
			.for_each(|pkt| self.in_buf.push(pkt).unwrap()); // we should never hit the unwrap
		sz
	}

	pub(crate) fn xmit(&self, buf: &Vec<u8>) -> Result<(), MemoryError> {
		let pkt = match Mbuf::from_bytes(&buf[..], &self.mempool) {
			Ok(pkt) => pkt,
			Err(_) => return Err(MemoryError::NoMem),
		};
		self.channel.send_to_engine(pkt)
	}

	pub(crate) fn xmit_to_engine_bulk(&self) -> usize {
		if self.out_buf.is_empty() {
			return 0usize;
		}
		let len = self.out_buf.capacity() - self.out_buf.len();
		let mut pkts = Vec::with_capacity(len);
		while !self.out_buf.is_empty() {
			pkts.push(self.out_buf.pop().unwrap()); // we should never hit this unwrap
		}
		let sz = self.channel.send_to_engine_bulk(&mut pkts);
		sz
	}
}

pub(crate) fn start() {
	let args = vec![
		String::from("-l 2-3"),
		String::from("-n 4"),
		String::from("--proc-type=secondary"),
		String::from("--"),
		String::from("-n 0"),
	];
	#[cfg(feature = "debug")]
	println!("mux args: {:?}", &args);
	eal_init(args).unwrap();
	#[cfg(feature = "debug")]
	println!("mux started");
}
