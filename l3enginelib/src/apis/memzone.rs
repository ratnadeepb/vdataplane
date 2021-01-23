/*
 * Created on Wed Dec 30 2020:16:35:23
 * Created by Ratnadeep Bhattacharya
 */

use dpdk_sys;
use std::result::Result;
use super::{MemoryError, WrappedCString};

pub struct Memzone {
	raw: *const dpdk_sys::rte_memzone,
}

impl Memzone {
	pub fn new(name: &str, len: usize) -> Result<Self, MemoryError> {
		let nm = WrappedCString::to_cstring(name)?;
		let raw = unsafe { dpdk_sys::rte_memzone_reserve(nm.as_ptr(), len as u64, dpdk_sys::rte_socket_id() as i32, dpdk_sys::RTE_MEMZONE_2MB)};
		
		if raw.is_null() {
			return Err(MemoryError::new());
		}
		Ok(Self { raw })
	}

	pub fn lookup(name: &str) -> Result<Self, MemoryError> {
		let nm = WrappedCString::to_cstring(name)?;
		let raw = unsafe { dpdk_sys::rte_memzone_lookup(nm.as_ptr()) };
		if raw.is_null() {
			return Err(MemoryError::new());
		}
		Ok(Self { raw })
	}

	pub fn virt_addr(&self) -> u64 {
		unsafe { (*self.raw).__bindgen_anon_1.addr_64 }
	}
}

impl Drop for Memzone {
	fn drop(&mut self) {
		match unsafe { dpdk_sys::rte_memzone_free(self.raw)} {
			0 => {},
			_ => log::error!("resource release failed; invalid memzone"),
		}
	}
}