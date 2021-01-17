mod ipv4hdr;
// mod mac;
// mod arp;

pub use ipv4hdr::*;
// pub use mac::*;
// pub use arp::*;

use dpdk_sys;
use libc::ENODEV;
use thiserror::Error;

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
