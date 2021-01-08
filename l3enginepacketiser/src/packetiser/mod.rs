/*
 * Created on Thu Dec 31 2020:13:02:50
 * Created by Ratnadeep Bhattacharya
 */

// DEVFLAGS: development flags - remove in production
#![allow(dead_code)]

use crate::{BURST_MAX, TABLE};
///
/// This is the first client we are building.
/// The design is that the main DPDK process only handles incoming and outgoing packets
/// Incoming packets are handed over to a secondary DPDK process
/// This module performs the basic processing and sends them over to the clients
/// It will also receive the modules from the clients and send them to main process
///
/// This module will ultimately hold filters that can be enabled to apply certain policies
///
use chashmap::CHashMap;
use crossbeam_queue::ArrayQueue;
use l3enginelib::{
	apis::{eal_init, Channel, Mbuf, MemoryError, Mempool, RingClientMap, RingClientMapError},
	net::Ipv4Hdr,
};
use smoltcp::wire::Ipv4Address;
use std::result::Result;

const G_MEMPOOL_NAME: &str = "GLOBAL_MEMPOOL";

pub(crate) struct RoutingTable {
	ip_id_map: CHashMap<Ipv4Address, u16>,
	id_ip_map: CHashMap<u16, Ipv4Address>,
}

impl RoutingTable {
	pub(crate) fn new() -> Self {
		Self {
			ip_id_map: CHashMap::new(),
			id_ip_map: CHashMap::new(),
		}
	}

	pub(crate) fn add_client(&self, client_id: u16, client_ip: Ipv4Address) {
		self.ip_id_map.insert(client_ip, client_id);
		self.id_ip_map.insert(client_id, client_ip);
	}

	pub fn remove_by_id(&self, client_id: u16) {
		self.id_ip_map.remove(&client_id);
	}

	pub fn remove_by_ip(&self, client_ip: Ipv4Address) {
		self.ip_id_map.remove(&client_ip);
	}

	pub(crate) fn lookup_by_ip(&self, client_ip: Ipv4Address) -> bool {
		self.ip_id_map.contains_key(&client_ip)
	}

	pub(crate) fn lookup_by_id(&self, client_id: u16) -> bool {
		self.id_ip_map.contains_key(&client_id)
	}

	pub(crate) fn id_from_ip(&self, client_ip: Ipv4Address) -> u16 {
		match self.ip_id_map.get(&client_ip) {
			Some(rg) => *rg,
			None => 0,
		}
	}
}

pub struct Packetiser {
	channel: Channel, // receive and transmit packets from and to the main process
	mempool: Mempool, // mempool to use
	clientmap: RingClientMap,
	pub(crate) i_bufqueue: ArrayQueue<Mbuf>, // packets that have been received from the primary process
	pub(crate) o_bufqueue: ArrayQueue<Mbuf>, // packets that have been received from clients
	cap: usize,                              // number of packets to be held in the buffers at any time
	allocated_ids: Vec<u16>,                 // hold all ids that have been allocated
}

pub static mut LAST_ALLOCATED_ID: u16 = 1; // ID of packetiser itself

impl Packetiser {
	const PACKETISER_ID: u16 = 1;

	pub fn new(cap: usize) -> Self {
		let channel = Channel::lookup(Self::PACKETISER_ID).unwrap(); // fatal error
		#[cfg(feature = "debug")]
		println!("found channel");
		let mempool = Mempool::lookup(G_MEMPOOL_NAME).unwrap(); // fatal error
		#[cfg(feature = "debug")]
		println!("found mempool, address: {:p}", mempool.get_ptr());
		let clientmap = RingClientMap::new();
		let i_bufqueue = ArrayQueue::new(cap);
		let o_bufqueue = ArrayQueue::new(cap);
		let allocated_ids = Vec::new();
		Self {
			channel,
			mempool,
			clientmap,
			i_bufqueue,
			o_bufqueue,
			cap,
			allocated_ids,
		}
	}

	pub fn add_client(&mut self) -> Result<(), RingClientMapError> {
		let mut key = 0;
		unsafe {
			for id in 0..LAST_ALLOCATED_ID {
				if !self.allocated_ids.contains(&id) {
					key = id;
					break;
				}
			}
			if key == 0 {
				key = LAST_ALLOCATED_ID + 1;
				LAST_ALLOCATED_ID += 1;
			}
		}
		self.allocated_ids.push(key);
		self.clientmap.add_client(key)
	}

	pub fn remove_clients(&mut self, key: u16) {
		let mut index = 0;
		for id in &self.allocated_ids {
			if *id == key {
				break;
			}
			index += 1;
		}
		self.allocated_ids.remove(index);
		self.clientmap.remove_client(key);
	}

	pub(crate) fn recv_from_engine_bulk(&self) -> Result<usize, MemoryError> {
		// calculate the space left in the queue
		let range = self.i_bufqueue.capacity() - self.i_bufqueue.len();

		// get those many packets only
		let mut pkts = Vec::with_capacity(range);
		for _ in 0..range {
			let buf_res = Mbuf::new(&self.mempool);
			match buf_res {
				Ok(buf) => pkts.push(buf),
				Err(_) => break, // break the loop when no more buffers can be created
			}
		}
		if pkts.len() == 0 {
			log::error!("packetiser: no buffer could be created");
			#[cfg(feature = "debug")]
			println!("packetiser: no buffer could be created");
			return Err(MemoryError::new());
		}
		let len = pkts.len();

		let count = self.channel.recv_from_engine_bulk(&mut pkts, len);
		if count != 0 {
			pkts.drain(0..count)
				.for_each(|pkt| self.i_bufqueue.push(pkt).unwrap());
			#[cfg(feature = "debug")]
			println!("packetiser: received packets");
		}
		Ok(count)
	}

	/// Send a bulk of packets to the engine
	pub(crate) fn send_to_engine_bulk(&self) -> usize {
		if self.o_bufqueue.is_empty() {
			return 0;
		}

		// OPTIMISE: this is a horrible construction
		let mut pkts = Vec::with_capacity(BURST_MAX);
		for pkt in self.o_bufqueue.pop() {
			pkts.push(pkt);
		}
		#[cfg(feature = "debug")]
		println!("packetiser: sending packets");
		self.channel.send_to_engine_bulk(pkts)
	}

	pub(crate) fn recv_from_clients(&self, key: u16, pkt: Mbuf) -> Result<(), RingClientMapError> {
		self.clientmap.send(key, pkt)
	}

	pub(crate) fn send_to_clients(
		&self,
		key: u16,
		pkt: &mut Mbuf,
	) -> Result<(), RingClientMapError> {
		self.clientmap.receive(key, pkt)
	}

	pub(crate) fn ipv4hdr(&self, pkt: &mut Mbuf) -> Ipv4Hdr {
		Ipv4Hdr::from_mbuf(pkt)
	}

	/// Store packets sent from the main process in the incoming buffer
	pub(crate) fn store_incoming(&self) -> Result<(), MemoryError> {
		let mut len = self.i_bufqueue.capacity() - self.i_bufqueue.len();
		let mut pkts = Vec::with_capacity(len);
		for i in 0..len {
			match Mbuf::new(&self.mempool) {
				Ok(buf) => pkts.push(buf),
				Err(e) => {
					log::error!("packetiser: buf creation failed: {}", e);
					len = i + 1;
					break;
				}
			}
		}
		let count = self.channel.recv_from_engine_bulk(&mut pkts, len);
		for pkt in pkts.drain(0..count) {
			match self.i_bufqueue.push(pkt) {
				Ok(_) => {}
				Err(_) => log::error!("packetiser: failed to store packet"),
			}
		}
		Ok(())
	}

	pub(crate) fn forward_incoming_packets(&self) {
		for _ in 0..self.i_bufqueue.len() {
			let mut pkt = self.i_bufqueue.pop().unwrap();
			let iphdr = Ipv4Hdr::from_mbuf(&mut pkt);
			let client_ip = iphdr.get_dst_addr();
			let client_id = TABLE.get().id_from_ip(client_ip);
			self.clientmap.send(client_id, pkt).unwrap();
		}
	}

	/// Store packets to be sent to the main process in the outgoing buffer
	pub(crate) fn store_outgoing(&self) -> Result<(), MemoryError> {
		let mut len = self.o_bufqueue.capacity() - self.o_bufqueue.len();
		let mut pkts = Vec::with_capacity(len);
		for i in 0..len {
			match Mbuf::new(&self.mempool) {
				Ok(buf) => pkts.push(buf),
				Err(e) => {
					log::error!("packetiser: buf creation failed: {}", e);
					len = i + 1;
					break;
				}
			}
		}
		// a simple round robing policy to collect packets from clients
		let mut i = 0;
		while i != len {
			for key in &self.allocated_ids {
				let mut pkt = Mbuf::new(&self.mempool)?;
				match self.clientmap.receive(*key, &mut pkt) {
					Ok(_) => match self.o_bufqueue.push(pkt) {
						Ok(_) => i += 1,
						Err(_) => log::error!("packetiser: failed to add to out buf"),
					},
					Err(_) => continue,
				}
			}
		}
		Ok(())
	}

	pub(crate) fn send_outgoing_packets(&self) -> Result<(), RingClientMapError> {
		let len = self.o_bufqueue.len();
		for _ in 0..len {
			let mb = self.o_bufqueue.pop();
			match mb {
				Some(buf) => match self.channel.send_to_engine(buf) {
					Ok(_) => continue,
					Err(_) => log::error!("packetiser: failed to send buf out to engine"),
				},
				None => log::error!("packetiser: failed to send buf out to engine"),
			}
		}
		Ok(())
	}

	fn u32_to_ipaddr(&self, ip: u32) -> Ipv4Address {
		Ipv4Address::from_bytes(&[
			((ip >> 24) & 0xFF) as u8,
			((ip >> 16) & 0xFF) as u8,
			((ip >> 8) & 0xFF) as u8,
			(ip & 0xFF) as u8,
		])
	}

	/// Get the header of an IPv4 packet
	pub fn get_ip_hdr(&self, buf: &mut Mbuf) -> Option<Ipv4Address> {
		let ipv4_hdr = unsafe { dpdk_sys::_pkt_ipv4_hdr(buf.get_ptr()) };
		#[cfg(feature = "debug")]
		println!("in get_ip_hdr");
		if ipv4_hdr.is_null() {
			return None;
		}
		let dst_addr = unsafe { (*ipv4_hdr).dst_addr };
		Some(Self::u32_to_ipaddr(&self, dst_addr))
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
	println!("packetiser args: {:?}", &args);
	eal_init(args).unwrap();
	log::info!("started packetiser");
	#[cfg(feature = "debug")]
	println!("packetiser started");
}
