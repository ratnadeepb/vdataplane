//! This module defines a struct that wraps around DPDK memory rings
//! And certain structures associated with communicating between DPDK processes
//!
//! The Ring structure simply wraps around rte_ring
//!
//! A Channel is a combination to two Ring structures - one for sending packets and the other for receiving.
//!
//! The RingClientMap structure is basically a hashmap that maps clients to their respective channels.

use anyhow::Result;
use chashmap::CHashMap;
use std::{
	marker::{Send, Sync},
	os::raw,
	ptr,
	ptr::NonNull,
};

use super::{Mbuf, MemoryError, RingClientMapError, WrappedCString};

/// The RingType is whether message is being sent from engine to container or from contianer to engine
pub enum RingType {
	C2E, // Client to Engine
	E2C, // Engine to Client
}

/// A ring is intended to communicate between two DPDK processes by sending/receiving `Mbuf`.
/// For best performance, each socket should have a dedicated `Mempool`.
///
/// C2E and E2C are from the client's perspective. The client is receiving and the client is
/// sending. For the server, it's the opposite.
pub struct Ring {
	client_id: u16,
	rtype: RingType,
	raw: NonNull<dpdk_sys::rte_ring>,
}

impl Ring {
	// const NO_FLAGS: u8 = 0;
	const RING_F_SP_ENQ: u8 = 0x0001;
	const RING_F_SC_DEQ: u8 = 0x0002;
	const RING_FLAGS: u8 = Self::RING_F_SP_ENQ | Self::RING_F_SC_DEQ;
	const RING_CAPACITY: usize = 512;

	/// Return a Ring created from a pointer if the pointer is not null
	pub fn from_ptr(
		client_id: u16,
		rtype: RingType,
		r: *mut dpdk_sys::rte_ring,
	) -> Result<Self, MemoryError> {
		if let Some(raw) = NonNull::new(r) {
			Ok(Self {
				client_id,
				rtype,
				raw,
			})
		} else {
			Err(MemoryError::NoBuf)
		}
	}

	pub fn new(
		client_id: u16,
		rtype: RingType,
		socket_id: raw::c_int,
	) -> Result<Self, MemoryError> {
		let r;
		match &rtype {
			RingType::C2E => r = "C2E",
			RingType::E2C => r = "E2C",
		};
		let nm = WrappedCString::to_cstring(format!("{}-{}", r, client_id))?;
		match NonNull::new(unsafe {
			dpdk_sys::rte_ring_create(
				nm.as_ptr(),
				Self::RING_CAPACITY as raw::c_uint,
				socket_id,
				Self::RING_FLAGS as raw::c_uint,
			)
		}) {
			Some(raw) => Ok(Self {
				client_id,
				rtype,
				raw,
			}),
			None => Err(MemoryError::new()),
		}
	}

	/// Get the name to lookup with
	#[inline]
	pub fn name(&self) -> String {
		let name: &str;
		match self.rtype {
			RingType::C2E => name = "C2E-".into(),
			RingType::E2C => name = "E2C-".into(),
		}
		let id = format!("{}", self.client_id);
		format!("{}{}", name, id)
	}

	/// Lookup a Ring
	pub fn lookup(rtype: RingType, client_id: u16) -> Result<Self, MemoryError> {
		let r;
		match &rtype {
			RingType::C2E => r = "C2E",
			RingType::E2C => r = "E2C",
		};
		let nm = WrappedCString::to_cstring(format!("{}-{}", r, client_id))?;
		let raw = unsafe { dpdk_sys::rte_ring_lookup(nm.as_ptr()) };

		if raw.is_null() {
			return Err(MemoryError::NoEntries);
		}
		#[cfg(feature = "debug")]
		println!("ring raw pointer: {:p}", raw);
		Self::from_ptr(client_id, rtype, raw)
	}

	/// Enqueue a single packet onto the ring
	pub fn enqueue(&self, pkt: Mbuf) -> Result<(), MemoryError> {
		match unsafe {
			dpdk_sys::_rte_ring_enqueue(self.get_ptr(), pkt.into_ptr() as *mut raw::c_void)
		} {
			0 => {
				#[cfg(feature = "debug")]
				println!("enqueued packets");
				Ok(())
			}
			_ => Err(MemoryError::new()),
		}
	}

	/// Dequeue a single packet from the ring
	pub fn dequeue(&self, pkt: &mut Mbuf) -> Result<(), MemoryError> {
		match unsafe {
			dpdk_sys::_rte_ring_dequeue(
				self.get_ptr(),
				&mut (pkt.get_ptr() as *mut _ as *mut raw::c_void),
			)
		} {
			0 => Ok(()),
			_ => Err(MemoryError::new()),
		}
	}

	/// Enqueue a single packet onto the ring
	pub fn enqueue_bulk(&self, mut pkts: Vec<Mbuf>) -> usize {
		#[cfg(feature = "debug")]
		println!("starting channel enqueue");
		let mut ptrs = Vec::with_capacity(pkts.len());
		for pkt in pkts.drain(..) {
			ptrs.push(pkt.into_ptr());
		}
		unsafe {
			dpdk_sys::_rte_ring_enqueue_bulk(
				self.get_ptr(),
				ptrs.as_ptr() as *mut *mut raw::c_void,
				pkts.len() as u32,
				ptr::null::<u32>() as *mut u32,
			) as usize
		}
	}

	/// Dequeue a single packet from the ring
	pub fn dequeue_burst(&self, pkts: &mut Vec<Mbuf>, rx_burst_max: usize) -> usize {
		// get the raw pointers to the mbufs
		#[cfg(feature = "debug")]
		println!("dequeue_bulk");
		let mut ptrs = Vec::with_capacity(rx_burst_max);
		for pkt in pkts {
			ptrs.push(pkt.get_ptr());
		}
		#[cfg(feature = "debug")]
		{
			let cnt = unsafe {
				// pass the raw pointers
				dpdk_sys::_rte_ring_dequeue_burst(
					self.get_ptr(),
					ptrs.as_ptr() as *mut *mut raw::c_void,
					ptrs.len() as u32,
					ptr::null::<u32>() as *mut u32,
				) as usize
			};
			println!("Dequeued {} pkts", cnt);
			return cnt;
		}
		#[cfg(not(feature = "debug"))]
		unsafe {
			// pass the raw pointers
			dpdk_sys::_rte_ring_dequeue_burst(
				self.get_ptr(),
				ptrs.as_ptr() as *mut *mut raw::c_void,
				ptrs.len() as u32,
				ptr::null::<u32>() as *mut u32,
			) as usize
		}
	}

	/// Return mutable reference to the C struct for FFI calls
	/// Does not consume the buffer
	#[inline]
	pub fn get_ptr(&self) -> *mut dpdk_sys::rte_ring {
		self.raw.as_ptr()
	}

	/// Returns the raw struct needed for FFI calls.
	#[inline]
	pub fn raw(&self) -> &dpdk_sys::rte_ring {
		unsafe { self.raw.as_ref() }
	}

	/// Returns the raw struct needed for FFI calls.
	#[inline]
	pub fn raw_mut(&mut self) -> &mut dpdk_sys::rte_ring {
		unsafe { self.raw.as_mut() }
	}
}

impl Drop for Ring {
	fn drop(&mut self) {
		unsafe {
			dpdk_sys::rte_ring_free(self.raw_mut());
		}
	}
}

/// The engine and client communicate with each other through
/// a transmit and a receive Ring
/// These two Rings together form a channel
pub struct Channel {
	pub client_to_engine: Ring, // send packets from client to engine
	pub engine_to_client: Ring, // send packets from engine to client
}

unsafe impl Send for Channel {}
unsafe impl Sync for Channel {}

impl Channel {
	pub fn new(client_id: u16) -> Result<Self, MemoryError> {
		let socket_id = unsafe { dpdk_sys::rte_socket_id() };

		let engine_to_client = Ring::new(client_id, RingType::E2C, socket_id as i32)?;
		let client_to_engine = Ring::new(client_id, RingType::C2E, socket_id as i32)?;

		Ok(Self {
			client_to_engine,
			engine_to_client,
		})
	}

	/// Lookup both C2E and E2C rings for this channel
	pub fn lookup(client_id: u16) -> Result<Self, MemoryError> {
		let engine_to_client = Ring::lookup(RingType::E2C, client_id)?;
		let client_to_engine = Ring::lookup(RingType::C2E, client_id)?;
		#[cfg(feature = "debug")]
		{
			println!(
				"channel: lookup : client_to_engine found {:p}",
				client_to_engine.get_ptr()
			);
			println!(
				"channel: lookup : engine_to_client found {:p}",
				engine_to_client.get_ptr()
			);
		}
		Ok(Self {
			client_to_engine,
			engine_to_client,
		})
	}

	/// Send a packet from engine to client
	pub fn send_to_engine(&self, pkt: Mbuf) -> Result<(), MemoryError> {
		self.client_to_engine.enqueue(pkt)
	}

	/// Send a packet from engine to client
	pub fn receive_from_engine(&self, pkt: &mut Mbuf) -> Result<(), MemoryError> {
		self.engine_to_client.dequeue(pkt)
	}

	/// Send a packet from engine to client
	pub fn send_to_client(&self, pkt: Mbuf) -> Result<(), MemoryError> {
		self.engine_to_client.enqueue(pkt)
	}

	/// Send a packet from engine to client
	pub fn receive_from_client(&self, pkt: &mut Mbuf) -> Result<(), MemoryError> {
		self.client_to_engine.dequeue(pkt)
	}

	/// Send bulk to client
	pub fn send_to_client_bulk(&self, pkts: Vec<Mbuf>) -> usize {
		self.engine_to_client.enqueue_bulk(pkts)
	}

	/// Receive bulk from client
	pub fn recv_from_client_bulk(&self, pkts: &mut Vec<Mbuf>, rx_burst_max: usize) -> usize {
		self.client_to_engine.dequeue_burst(pkts, rx_burst_max)
	}

	/// Send bulk to engine
	pub fn send_to_engine_bulk(&self, pkts: Vec<Mbuf>) -> usize {
		self.client_to_engine.enqueue_bulk(pkts)
	}

	/// Receive bulk from engine
	pub fn recv_from_engine_burst(&self, pkts: &mut Vec<Mbuf>, rx_burst_max: usize) -> usize {
		#[cfg(feature = "debug")]
		println!("recv_from_engine_bulk");
		self.engine_to_client.dequeue_burst(pkts, rx_burst_max)
	}
}

/// Channel to Client mapping
pub struct RingClientMap {
	pub(crate) ringmap: CHashMap<u16, Channel>,
}

impl RingClientMap {
	pub fn new() -> Self {
		Self {
			ringmap: CHashMap::new(),
		}
	}

	pub fn len(&self) -> usize {
		self.ringmap.len()
	}

	/// Add a client to the system
	pub fn add_client(&self, client_id: u16) -> Result<(), RingClientMapError> {
		#[cfg(feature = "debug")]
		println!("add_client: adding {}", client_id);
		let channel;
		match Channel::new(client_id) {
			Ok(ch) => {
				#[cfg(feature = "debug")]
				{
					println!(
						"add_client: client_to_engine added {:p}",
						ch.client_to_engine.get_ptr()
					);
					println!(
						"add_client: engine_to_client added {:p}",
						ch.engine_to_client.get_ptr()
					);
				}
				channel = ch
			}
			Err(e) => return Err(RingClientMapError::MemoryError(e)),
		}
		self.ringmap.insert(client_id, channel);
		Ok(())
	}

	/// Remove a client from the system
	pub fn remove_client(&self, client_id: u16) {
		self.ringmap.remove(&client_id);
	}

	/// Send a packet to a client
	pub fn send(&self, key: u16, pkt: Mbuf) -> Result<(), RingClientMapError> {
		let channel;
		// ReadGuard is held within the next block alone
		match self.ringmap.get(&key) {
			Some(ch) => channel = ch,
			None => return Err(RingClientMapError::ClientNotFound(key)),
		};
		channel.send_to_client(pkt)?;
		#[cfg(feature = "debug")]
		println!("Sent to client: {}", key);
		Ok(())
	}

	/// Receive a packet from a client
	pub fn receive(&self, key: u16, pkt: &mut Mbuf) -> Result<(), RingClientMapError> {
		let channel;
		// ReadGuard is held within the next block alone
		match self.ringmap.get(&key) {
			Some(ch) => channel = ch,
			None => return Err(RingClientMapError::ClientNotFound(key)),
		}
		channel.receive_from_client(pkt)?;
		#[cfg(feature = "debug")]
		println!("Received from client: {}", key);
		Ok(())
	}

	/// Send packets to a client in bulk
	pub fn send_bulk(&self, key: u16, pkts: Vec<Mbuf>) -> Result<usize, RingClientMapError> {
		let channel;
		// ReadGuard is held within the next block alone
		match self.ringmap.get(&key) {
			Some(ch) => channel = ch,
			None => return Err(RingClientMapError::ClientNotFound(key)),
		}
		let count = channel.send_to_client_bulk(pkts);
		Ok(count)
	}

	/// Receive packets from a client in bulk
	pub fn receive_bulk(
		&self,
		key: u16,
		pkts: &mut Vec<Mbuf>,
		rx_burst_max: usize,
	) -> Result<usize, RingClientMapError> {
		let channel;
		// ReadGuard is held within the next block alone
		match self.ringmap.get(&key) {
			Some(ch) => channel = ch,
			None => return Err(RingClientMapError::ClientNotFound(key)),
		}
		let count = channel.recv_from_client_bulk(pkts, rx_burst_max);
		Ok(count)
	}
}
