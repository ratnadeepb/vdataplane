/*
 * Created on Mon Dec 28 2020:12:18:43
 * Created by Ratnadeep Bhattacharya
 */

use std::{ptr::NonNull, mem, error::Error, ptr, slice, os::raw, marker::Send};
use super::{MemoryError, BufError};
use crate::MEMPOOL;

/// A trait for returning the size type in bytes
///
/// Size of structs are used for bound checks when reading and writing packets
///
/// # Derivable
///
/// The `SizeOf` trait can be used with `#[derive]` and defaults to `std::mem::size_of::<Self>()`
pub trait SizeOf {
	/// Return the size of a type in bytes
	fn size_of() -> usize;
}

impl SizeOf for () {
	fn size_of() -> usize {
		std::mem::size_of::<()>()
	}
}

impl SizeOf for u8 {
	fn size_of() -> usize {
		std::mem::size_of::<u8>()
	}
}

impl SizeOf for [u8; 2] {
	fn size_of() -> usize {
		std::mem::size_of::<[u8; 2]>()
	}
}

impl SizeOf for [u8; 16] {
	fn size_of() -> usize {
		std::mem::size_of::<[u8; 16]>()
	}
}

impl SizeOf for ::std::net::Ipv6Addr {
	fn size_of() -> usize {
		std::mem::size_of::<std::net::Ipv6Addr>()
	}
}

pub struct Mbuf {
	raw: NonNull<dpdk_sys::rte_mbuf>,
}

unsafe impl Send for Mbuf {}

impl Mbuf {
	pub fn new() -> Result<Self, MemoryError> {
		let mempool;
		match MEMPOOL.try_get() {
			Some(mp) => mempool = mp.get_ptr(),
			None => return Err(MemoryError::BadVal),
		};

		let r = unsafe { dpdk_sys::_rte_pktmbuf_alloc(mempool) };
		match NonNull::new(r) {
			Some(raw) => Ok(Self { raw }),
			None => Err(MemoryError::NoBuf),
		}
	}

	/// Create a new message buffer from a byte array
	#[inline]
	pub fn from_bytes(data: &[u8]) -> Result<Self, Box<dyn Error>> {
		let mut mbuf = Mbuf::new()?;
		mbuf.extend(0, data.len())?;
		mbuf.write_data_slice(0, data)?;
		Ok(mbuf)
	}

	/// Creates a new `Mbuf` from a raw pointer
	#[inline]
	pub unsafe fn from_ptr(ptr: *mut dpdk_sys::rte_mbuf) -> Self {
		Mbuf {
			raw: NonNull::new_unchecked(ptr),
		}
	}

	/// Returns the raw struct needed for FFI calls
	#[inline]
	pub fn raw(&self) -> &dpdk_sys::rte_mbuf {
		unsafe { self.raw.as_ref() }
	}

	/// Returns the raw struct needed for FFI calls
	#[inline]
	pub fn raw_mut(&mut self) -> &mut dpdk_sys::rte_mbuf {
		unsafe { self.raw.as_mut() }
	}

	/// Return mutable reference to the C struct for FFI calls
	/// Does not consume the buffer
	#[inline]
	pub fn get_ptr(&self) -> *mut dpdk_sys::rte_mbuf {
		self.raw.as_ptr()
	}

	/// Acquires the underlying raw struct pointer.
	///
	/// The `Mbuf` is consumed. It is the caller's the responsibility to
	/// free the raw pointer after use. Otherwise the buffer is leaked.
	#[inline]
	pub fn into_ptr(self) -> *mut dpdk_sys::rte_mbuf {
		let ptr = self.raw.as_ptr();
		mem::forget(self);
		ptr
	}

	/// Returns amount of data stored in the buffer
	#[inline]
	pub fn data_len(&self) -> usize {
		self.raw().data_len as usize
	}

	/// Returns the raw pointer from the offset
	#[inline]
	pub unsafe fn data_address(&self, offset: usize) -> *mut u8 {
		let raw = self.raw();
		(raw.buf_addr as *mut u8).offset(raw.data_off as isize + offset as isize)
	}

	/// Returns the amount of bytes left in the buffer
	#[inline]
	fn tailroom(&self) -> usize {
		let raw = self.raw();
		(raw.buf_len - raw.data_off - raw.data_len) as usize
	}

	/// Extends the data buffer at offset `len`
	///
	/// If the offset is not the end of data
	/// data after offset is shifted down to make room
	#[inline]
	pub fn extend(&mut self, offset: usize, len: usize) -> Result<(), BufError> {
		if len <= 0 || offset > self.data_len() || len > self.tailroom() {
			return Err(BufError::NotResized);
		}
		// shift down data to make room
		let to_copy = self.data_len() - offset;
		if to_copy > 0 {
			unsafe {
				let src = self.data_address(offset);
				let dst = self.data_address(offset + len);
				ptr::copy(src, dst, to_copy); // this is an expensive copy op
			}
		}

		// do some record keeping
		self.raw_mut().data_len += len as u16;
		self.raw_mut().pkt_len += len as u32;

		Ok(())
	}

	/// Shrinks the data buffer at offset by `len` bytes
	///
	/// The data at offset is shifted up
	#[inline]
	pub fn shrink(&mut self, offset: usize, len: usize) -> Result<(), BufError> {
		if len <= 0 || offset + len > self.data_len() {
			return Err(BufError::NotResized);
		}

		// shifts up data to fill the room
		let to_copy = self.data_len() - offset - len;
		if to_copy > 0 {
			unsafe {
				let src = self.data_address(offset + len);
				let dst = self.data_address(offset);
				ptr::copy(src, dst, to_copy); // expensive copy
			}
		}

		// do some record keeping
		self.raw_mut().data_len -= len as u16;
		self.raw_mut().pkt_len -= len as u32;

		Ok(())
	}

	/// Resizes the data buffer
	#[inline]
	pub fn resize(&mut self, offset: usize, len: isize) -> Result<(), BufError> {
		if len < 0 {
			self.shrink(offset, -len as usize)
		} else {
			self.extend(offset, len as usize)
		}
	}

	/// Truncates the data buffer to len
	#[inline]
	pub fn truncate(&mut self, to_len: usize) -> Result<(), BufError> {
		if to_len >= self.data_len() {
			return Err(BufError::NotResized);
		}

		self.raw_mut().data_len = to_len as u16;
		self.raw_mut().pkt_len = to_len as u32;

		Ok(())
	}

	/// Reads the data at offset as `T` and returns it as a raw pointer.
	#[inline]
	pub fn read_data<T: SizeOf>(&self, offset: usize) -> Result<NonNull<T>, BufError> {
		if offset >= self.data_len() {
			return Err(BufError::BadOffset(offset, self.data_len()));
		}

		if offset + T::size_of() > self.data_len() {
			return Err(BufError::OutOfBuffer(T::size_of(), self.data_len() - offset));
		}

		unsafe {
			let item = self.data_address(offset) as *mut T;
			Ok(NonNull::new_unchecked(item))
		}
	}

	/// Reads the data at offset as a slice of `T` and returns the slice as
	/// a raw pointer.
	#[inline]
	pub fn read_data_slice<T: SizeOf>(
		&self,
		offset: usize,
		count: usize,
	) -> Result<NonNull<[T]>, BufError> {
		if offset >= self.data_len() {
			return Err(BufError::BadOffset(offset, self.data_len()));
		}
		if offset + T::size_of() * count > self.data_len() - offset {
			return Err(BufError::OutOfBuffer(T::size_of() * count, self.data_len() - offset))
		}

		unsafe {
			let item0 = self.data_address(offset) as *mut T;
			let slice = slice::from_raw_parts_mut(item0, count) as *mut [T];
			Ok(NonNull::new_unchecked(slice))
		}
	}

	/// Writes a slice of `T` to the data buffer at offset and returns the
	/// new copy as a raw pointer.
	///
	/// Before writing to the data buffer, should call `Mbuf::extend` first
	/// to make sure enough space is allocated for the write and data is not
	/// being overridden.
	#[inline]
	pub fn write_data_slice<T: SizeOf>(
		&mut self,
		offset: usize,
		slice: &[T],
	) -> Result<NonNull<[T]>, BufError> {
		let count = slice.len();

		if offset + T::size_of() * count > self.data_len() {
			return Err(BufError::OutOfBuffer(T::size_of() * count, self.data_len() - offset));
		}

		unsafe {
			let src = slice.as_ptr();
			let dst = self.data_address(offset) as *mut T;
			ptr::copy_nonoverlapping(src, dst, count);
		}

		self.read_data_slice(offset, count)
	}

	/// Allocates a Vec of `Mbuf`s of `len` size.
	pub fn alloc_bulk(len: usize) -> Result<Vec<Mbuf>, MemoryError> {
		let mut ptrs = Vec::with_capacity(len);
		let mempool;
		match MEMPOOL.try_get() {
			Some(mp) => mempool = mp.get_ptr(),
			None => return Err(MemoryError::BadVal),
		};

		let mbufs = unsafe {
			// dpdk_sys::_rte_pktmbuf_alloc_bulk(mempool, ptrs.as_mut_ptr(), len as raw::c_uint);
			let rb = dpdk_sys::_rte_pktmbuf_alloc_bulk(mempool, ptrs.as_mut_ptr(), len as raw::c_uint);
			match rb {
				0 => {
					ptrs.set_len(len);
					ptrs.into_iter()
					.map(|ptr| Mbuf::from_ptr(ptr))
					.collect::<Vec<_>>()
				},
				_ => return Err(MemoryError::NoBuf),
			}
		};

		Ok(mbufs)
	}

	/// Frees the message buffers in bulk.
	pub fn free_bulk(mbufs: Vec<Mbuf>) {
		let ptrs = mbufs.into_iter().map(Mbuf::into_ptr).collect::<Vec<_>>();
		Self::mbuf_free_bulk(ptrs);
	}

	/// Frees the `rte_mbuf` in bulk.
	pub fn mbuf_free_bulk(mbufs: Vec<*mut dpdk_sys::rte_mbuf>) {
		if mbufs.is_empty() {
			return;
		}

		let mut to_free = Vec::with_capacity(mbufs.len());
		let pool = unsafe { (*mbufs[0]).pool };

		for mbuf in mbufs.into_iter() {
			if pool == unsafe { (*mbuf).pool } {
				to_free.push(mbuf as *mut raw::c_void);
			} else {
				unsafe {
					let len = to_free.len();
					dpdk_sys::_rte_mempool_put_bulk(pool, to_free.as_ptr(), len as u32);
					to_free.set_len(0);
				}

				to_free.push(mbuf as *mut raw::c_void);
			}
		}

		unsafe {
			let len = to_free.len();
			dpdk_sys::_rte_mempool_put_bulk(pool, to_free.as_ptr(), len as u32);
			to_free.set_len(0);
		}
	}
}