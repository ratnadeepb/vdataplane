//! This module defines a struct that wraps around DPDK memory rings
//! And certain structures associated with communicating between DPDK processes
//!
//! The Ring structure simply wraps around rte_ring
//!
//! A Channel is a combination to two Ring structures - one for sending packets and the other for receiving.

use anyhow::Result;
use libc::c_uint;
// use chashmap::CHashMap;
use std::{
	marker::{Send, Sync},
	os::raw,
	ptr,
	ptr::NonNull,
};

use super::{Mbuf, MemoryError, WrappedCString};

/// The RingType is whether message is being sent from engine to container or from contianer to engine
pub enum RingType {
	P2E, // Packetiser to Engine
	E2P, // Engine to Packetiser
}

/// A ring is intended to communicate between two DPDK processes by sending/receiving `Mbuf`.
/// For best performance, each socket should have a dedicated `Mempool`.
///
/// C2E and E2C are from the client's perspective. The client is receiving and the client is
/// sending. For the server, it's the opposite.
pub struct Ring {
	rtype: RingType,
	raw: NonNull<dpdk_sys::rte_ring>,
}

impl Ring {
	const RING_F_SP_ENQ: u8 = 0x0001;
	const RING_F_SC_DEQ: u8 = 0x0002;
	const RING_FLAGS: u8 = Self::RING_F_SP_ENQ | Self::RING_F_SC_DEQ;
	const RING_CAPACITY: usize = 512;

	/// Return a Ring created from a pointer if the pointer is not null
	pub fn from_ptr(rtype: RingType, r: *mut dpdk_sys::rte_ring) -> Result<Self, MemoryError> {
		if let Some(raw) = NonNull::new(r) {
			Ok(Self { rtype, raw })
		} else {
			Err(MemoryError::NoBuf)
		}
	}

	pub fn new(rtype: RingType, socket_id: raw::c_int) -> Result<Self, MemoryError> {
		let r;
		match &rtype {
			RingType::P2E => r = "C2E",
			RingType::E2P => r = "E2C",
		};
		let nm = WrappedCString::to_cstring(format!("{}", r))?;
		match NonNull::new(unsafe {
			dpdk_sys::rte_ring_create(
				nm.as_ptr(),
				Self::RING_CAPACITY as raw::c_uint,
				socket_id,
				Self::RING_FLAGS as raw::c_uint,
			)
		}) {
			Some(raw) => Ok(Self { rtype, raw }),
			None => Err(MemoryError::new()),
		}
	}

	/// Get the name to lookup with
	#[inline]
	pub fn name(&self) -> &str {
		match self.rtype {
			RingType::P2E => "C2E",
			RingType::E2P => "E2C",
		}
	}

	/// Lookup a Ring
	pub fn lookup(rtype: RingType) -> Result<Self, MemoryError> {
		let r;
		match &rtype {
			RingType::P2E => r = "C2E",
			RingType::E2P => r = "E2C",
		};
		let nm = WrappedCString::to_cstring(format!("{}", r))?;
		let raw = unsafe { dpdk_sys::rte_ring_lookup(nm.as_ptr()) };

		if raw.is_null() {
			return Err(MemoryError::NoEntries);
		}
		#[cfg(feature = "debug")]
		println!("ring raw pointer: {:p}", raw);
		Self::from_ptr(rtype, raw)
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

	/// Enqueue packets in bulk onto the ring
	pub fn enqueue_burst(&self, mut pkts: Vec<Mbuf>) -> usize {
		let mut ptrs = Vec::with_capacity(pkts.len());
		for pkt in pkts.drain(..) {
			ptrs.push(pkt.into_ptr());
		}
		#[cfg(feature = "debug")]
		{
			println!("enqueue_burst - ptrs sz: {}", ptrs.len());
			let mut free: c_uint = 0;
			let sz = unsafe {
				dpdk_sys::_rte_ring_enqueue_burst(
					self.get_ptr(),
					ptrs.as_mut_ptr() as *mut *mut raw::c_void,
					pkts.len() as u32,
					// ptr::null::<u32>() as *mut u32,
					&mut free as *mut c_uint,
				) as usize
			};
			println!("enqueue_burst - sz: {}", sz);
			println!("enqueue_burst - free: {}", free);
			sz
		}
		#[cfg(not(feature = "debug"))]
		unsafe {
			dpdk_sys::_rte_ring_enqueue_burst(
				self.get_ptr(),
				ptrs.as_mut_ptr() as *mut *mut raw::c_void,
				pkts.len() as u32,
				ptr::null::<u32>() as *mut u32,
			) as usize
		}
	}

	/// Enqueue packets in bulk onto the ring
	pub fn enqueue_bulk(&self, mut pkts: Vec<Mbuf>) -> usize {
		let mut ptrs = Vec::with_capacity(pkts.len());
		for pkt in pkts.drain(..) {
			ptrs.push(pkt.into_ptr());
		}
		#[cfg(feature = "debug")]
		{
			println!("enqueue_bulk - ptrs sz: {}", ptrs.len());
			let mut free: c_uint = 0;
			let sz = unsafe {
				dpdk_sys::_rte_ring_enqueue_bulk(
					self.get_ptr(),
					ptrs.as_mut_ptr() as *mut *mut raw::c_void,
					pkts.len() as u32,
					// ptr::null::<u32>() as *mut u32,
					&mut free as *mut c_uint,
				) as usize
			};
			println!("enqueue_bulk - sz: {}", sz);
			println!("enqueue_bulk - free: {}", free);
			sz
		}
		#[cfg(not(feature = "debug"))]
		unsafe {
			dpdk_sys::_rte_ring_enqueue_bulk(
				self.get_ptr(),
				ptrs.as_mut_ptr() as *mut *mut raw::c_void,
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
	pub to_engine: Ring,     // send packets from client to engine
	pub to_packetiser: Ring, // send packets from engine to client
}

unsafe impl Send for Channel {}
unsafe impl Sync for Channel {}

impl Channel {
	pub fn new() -> Result<Self, MemoryError> {
		let socket_id = unsafe { dpdk_sys::rte_socket_id() };

		let engine_to_client = Ring::new(RingType::E2P, socket_id as i32)?;
		let client_to_engine = Ring::new(RingType::P2E, socket_id as i32)?;

		Ok(Self {
			to_engine: client_to_engine,
			to_packetiser: engine_to_client,
		})
	}

	/// Lookup both C2E and E2C rings for this channel
	pub fn lookup() -> Result<Self, MemoryError> {
		#[cfg(feature = "debug")]
		println!("lookup channel");
		let to_packetiser = Ring::lookup(RingType::E2P)?;
		let to_engine = Ring::lookup(RingType::P2E)?;
		#[cfg(feature = "debug")]
		{
			println!(
				"channel: lookup : to_engine found {:p}",
				to_engine.get_ptr()
			);
			println!(
				"channel: lookup : to_packetiser found {:p}",
				to_packetiser.get_ptr()
			);
		}
		Ok(Self {
			to_engine: to_engine,
			to_packetiser: to_packetiser,
		})
	}

	/// Send a packet from engine to packetiser
	pub fn send_to_engine(&self, pkt: Mbuf) -> Result<(), MemoryError> {
		self.to_engine.enqueue(pkt)
	}

	/// Send a packet from engine to packetiser
	pub fn receive_from_engine(&self, pkt: &mut Mbuf) -> Result<(), MemoryError> {
		self.to_packetiser.dequeue(pkt)
	}

	/// Send a packet from engine to packetiser
	pub fn send_to_packetiser(&self, pkt: Mbuf) -> Result<(), MemoryError> {
		self.to_packetiser.enqueue(pkt)
	}

	/// Send a packet from engine to packetiser
	pub fn receive_from_packetiser(&self, pkt: &mut Mbuf) -> Result<(), MemoryError> {
		self.to_engine.dequeue(pkt)
	}

	/// Send bulk to packetiser
	pub fn send_to_packetiser_bulk(&self, pkts: Vec<Mbuf>) -> usize {
		// self.to_packetiser.enqueue_bulk(pkts)
		// self.to_packetiser.enqueue_burst(pkts)
		#[cfg(feature = "debug")]
		println!("pkts sz: {}", pkts.len());
		let mut i = 0;
		for pkt in pkts {
			#[cfg(feature = "debug")]
			if i == 0 {
				println!("pkt: {:#?}", &pkt);
			}
			match self.to_packetiser.enqueue(pkt) {
				Ok(_) => i += 1,
				Err(e) => {
					log::error!("{}: Memory error trying to send to packetiser: {}", i, e);
					#[cfg(feature = "debug")]
					println!("{}: Memory error trying to send to packetiser: {}", i, e);
					break;
				}
			}
		}
		i
	}

	/// Receive bulk from packetiser
	pub fn recv_from_packetiser_bulk(&self, pkts: &mut Vec<Mbuf>, rx_burst_max: usize) -> usize {
		let mut i = 0;
		let mut k = 0;
		for pkt in pkts {
			if k == rx_burst_max {
				break;
			}
			k += 1;
			match self.to_engine.dequeue(pkt) {
				Ok(_) => i += 1,
				Err(_e) => {
					#[cfg(feature = "debug")]
					println!("failed to dequeue: {}", _e);
				}
			}
		}
		#[cfg(feature = "debug")]
		println!("dequeued {} pkts", i);
		// self.to_engine.dequeue_burst(pkts, rx_burst_max)
		i
	}

	/// Send bulk to engine
	pub fn send_to_engine_bulk(&self, pkts: Vec<Mbuf>) -> usize {
		self.to_engine.enqueue_bulk(pkts)
	}

	/// Receive bulk from engine
	pub fn recv_from_engine_burst(&self, pkts: &mut Vec<Mbuf>, rx_burst_max: usize) -> usize {
		#[cfg(feature = "debug")]
		println!("recv_from_engine_burst - size: {}", rx_burst_max);
		self.to_packetiser.dequeue_burst(pkts, rx_burst_max)
	}
}
