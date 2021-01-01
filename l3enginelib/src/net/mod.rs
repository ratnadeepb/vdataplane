/*
 * Created on Mon Dec 28 2020:13:48:35
 * Created by Ratnadeep Bhattacharya
 */

mod ipv4hdr;
mod mac;

pub use ipv4hdr::*;
pub use mac::*;

use thiserror::Error;
use libc::ENODEV;
use dpdk_sys;

#[derive(Debug, Error)]
pub enum MacError {
	#[error("invalid port id: {}", _0)]
	InvalidID(u8),
	#[error("Other error: {}", _0)]
	OtherErr(i32),
}

impl MacError {
	pub fn new(id: u8) -> Self {
		let errno = unsafe { dpdk_sys::_rte_errno() };
		match errno {
			ENODEV => MacError::InvalidID(id),
			_ => MacError::OtherErr(errno),
		}
	}
}