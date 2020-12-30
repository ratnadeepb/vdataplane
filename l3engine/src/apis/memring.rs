/*
 * Created on Mon Dec 28 2020:12:26:07
 * Created by Ratnadeep Bhattacharya
 */

use std::{ptr::NonNull, os::raw, ffi::CString, marker::{Sync, Send}, ptr};
use anyhow::Result;
use chashmap::CHashMap;

use super::{MemoryError, Mbuf, RingClientMapError};

/// The RingType is whether message is being sent from engine to container or from contianer to engine
pub enum RingType {
	RX,
	TX,
}

/// A ring is intended to communicate between two DPDK processes by sending/receiving `Mbuf`.
/// For best performance, each socket should have a dedicated `Mempool`.
pub struct Ring {
	client_id: u16,
	rtype: RingType,
	raw: NonNull<dpdk_sys::rte_ring>,
}

impl Ring {
	const NO_FLAGS: u8 = 0;
	const RING_CAPACITY: usize = 512;

	/// Return a Ring created from a pointer if the pointer is not null
	pub fn from_ptr(client_id: u16, rtype: RingType, r: *mut dpdk_sys::rte_ring) -> Result<Self, MemoryError> {
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
		socket_id: raw::c_int
	) -> Result<Self, MemoryError> {
		let r;
		match &rtype {
			RingType::RX => r = "RX",
			RingType::TX => r = "TX",
		};
		let nm: CString;
		match CString::new(format!("{}-{}", r, client_id)) {
			Ok(cstr) => nm = cstr,
			Err(_) => return Err(MemoryError::BadVal),
		};
		match NonNull::new(unsafe {dpdk_sys::rte_ring_create(nm.as_ptr(), Self::RING_CAPACITY as raw::c_uint, socket_id, Self::NO_FLAGS as raw::c_uint)}) {
			Some(raw) => Ok(Self {
				client_id,
				rtype,
				raw
			}),
			None => Err(MemoryError::new()),
		}
	}

	/// Get the name to lookup with
	#[inline]
	pub fn name(&self) -> String {
		let name: &str;
		match self.rtype {
			RingType::RX => name = "RX-".into(),
			RingType::TX => name = "TX-".into(),
		}
		let id = format!("{}", self.client_id);
		format!("{}{}", name, id)
	}

	/// Enqueue a single packet onto the ring
	pub fn enqueue(&self, pkt: Mbuf) -> Result<(), MemoryError> {
		match unsafe { dpdk_sys::_rte_ring_enqueue(self.get_ptr(), pkt.into_ptr() as *mut raw::c_void) } {
			0 => Ok(()),
			_ => Err(MemoryError::new()),
		}
	}

	/// Dequeue a single packet from the ring
	pub fn dequeue(&self, pkt: &mut Mbuf) -> Result<(), MemoryError> {
		match unsafe {
			dpdk_sys::_rte_ring_dequeue(self.get_ptr(), &mut (pkt.get_ptr() as *mut _ as *mut raw::c_void)) } {
			0 => Ok(()),
			_ => Err(MemoryError::new()),
		}
	}

	/// Enqueue a single packet onto the ring
	pub fn enqueue_bulk(&self, pkts: Vec<*mut dpdk_sys::rte_mbuf>) -> Result<(), MemoryError> {
		match unsafe { dpdk_sys::_rte_ring_enqueue_bulk(self.get_ptr(), pkts.as_ptr() as *mut *mut raw::c_void, pkts.len() as u32, ptr::null::<u32>() as *mut u32) } {
			0 => Ok(()),
			_ => Err(MemoryError::new()),
		}
	}

	/// Dequeue a single packet from the ring
	pub fn dequeue_bulk(&self, pkts: &mut Vec<*mut dpdk_sys::rte_mbuf>) -> Result<(), MemoryError> {
		match unsafe {
			dpdk_sys::_rte_ring_dequeue_bulk(self.get_ptr(), pkts.as_ptr() as *mut *mut raw::c_void, pkts.len() as u32, ptr::null::<u32>() as *mut u32) } {
			0 => Ok(()),
			_ => Err(MemoryError::new()),
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
pub(crate) struct Channel {
	pub(crate) tx_q: Ring, // send packets from client to engine
	pub(crate) rx_q: Ring, // send packets from engine to client
}

unsafe impl Send for Channel {}
unsafe impl Sync for Channel {}

impl Channel {
	pub fn new(client_id: u16) -> Result<Self, MemoryError> {
		let socket_id = unsafe { dpdk_sys::rte_socket_id() };

		let rx_q = Ring::new(client_id, RingType::RX, socket_id as i32)?;
		let tx_q = Ring::new(client_id, RingType::TX, socket_id as i32)?;

		Ok(Self {
			tx_q,
			rx_q
		})
	}

	/// Send a packet from engine to client
	pub fn send_to_engine(&self, pkt: Mbuf) -> Result<(), MemoryError> {
		self.tx_q.enqueue(pkt)
	}

	/// Send a packet from engine to client
	pub fn receive_from_engine(&self, pkt: &mut Mbuf) -> Result<(), MemoryError> {
		self.rx_q.dequeue(pkt)
	}

	/// Send a packet from engine to client
	pub fn send_to_client(&self, pkt: Mbuf) -> Result<(), MemoryError> {
		self.rx_q.enqueue(pkt)
	}

	/// Send a packet from engine to client
	pub fn receive_from_client(&self, pkt: &mut Mbuf) -> Result<(), MemoryError> {
		self.tx_q.dequeue(pkt)
	}

	/// Send bulk to processor core
	pub fn send_to_processor(&self, pkts: Vec<*mut dpdk_sys::rte_mbuf>) -> Result<(), MemoryError> {
		self.tx_q.enqueue_bulk(pkts)
	}

	/// Receive bulk from processor core
	pub fn recv_from_processor(&self, pkts: &mut Vec<*mut dpdk_sys::rte_mbuf>) -> Result<(), MemoryError> {
		self.tx_q.dequeue_bulk(pkts)
	}
}

/// Channel to Client mapping
pub struct RingClientMap {
	pub(crate) ringmap: CHashMap<u16, Channel>,
}

impl RingClientMap {
	pub fn new() -> Self {
		Self { ringmap: CHashMap::new() }
	}

	/// Add a client to the system
	pub fn add_client(&self, client_id: u16) -> Result<(), RingClientMapError> {
		let channel;
		match Channel::new(client_id) {
			Ok(ch) => channel = ch,
			Err(e) => return Err(RingClientMapError::MemoryError(e)),
		}
		self.ringmap.insert(client_id, channel);
		Ok(())
	}

	/// Remove a client from the system
	pub fn remove_client(&self, client_id: u16) {
		self.ringmap.remove(&client_id);
	}

	/// Send a packet to a container
	pub fn send(&self, key: u16, pkt: Mbuf) -> Result<(), RingClientMapError> {
		let channel;
		match self.ringmap.get(&key) {
			Some(ch) => channel = ch,
			None => return Err(RingClientMapError::ClientNotFound(key)),
		};
		match channel.send_to_client(pkt) {
			Ok(()) => Ok(()),
			Err(e) => Err(RingClientMapError::MemoryError(e)),
		}
	}

	/// Receive a packet from a container
	pub fn receive(&self, key: u16, pkt: &mut Mbuf) -> Result<(), RingClientMapError> {
		let channel;
		match self.ringmap.get(&key) {
			Some(ch) => channel = ch,
			None => return Err(RingClientMapError::ClientNotFound(key)),
		}
		match channel.receive_from_client(pkt) {
			Ok(()) => Ok(()),
			Err(e) => Err(RingClientMapError::MemoryError(e)),
		}
	}
}