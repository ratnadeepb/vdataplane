/*
 * Created on Mon Dec 28 2020:12:16:49
 * Created by Ratnadeep Bhattacharya
 */

mod mbuf;
mod mempool;
mod memring;
mod port;

pub use mbuf::*;
pub use mempool::*;
pub use memring::*;
pub use port::*;

use thiserror::Error;
use libc::{EINVAL, ENOSPC, EEXIST, ENOMEM, ENODEV, ENOTSUP, ENOBUFS, ENOENT, EAGAIN, EALREADY, EFAULT, EPROTO, ENOEXEC};
use std::{ffi::CString, os::raw};
use dpdk_ffi;
use log;

#[derive(Error, Debug)]
pub enum MemoryError {
	#[error("function could not get pointer to rte_config structure")]
	NoConfig,
	#[error("function was called from a secondary process instance")]
	SecondaryProcess,
	#[error("cache size provided is too large, or priv_size is not aligned.")]
	Invalid,
	#[error("the maximum number of memzones has already been allocated")]
	NoSpace,
	#[error("a memzone with the same name already exists")]
	Exists,
	#[error("no appropriate memory area found in which to create memzone")]
	NoMem,
	#[error("not enough memory")]
	NoBuf,
	#[error("not enough entries to dequeue")]
	NoEntries,
	#[error("bad val")]
	BadVal, // should never hit this
}

#[derive(Error, Debug)]
pub enum RingClientMapError {
	#[error("Memory Error")]
	MemoryError(MemoryError),
	#[error("Client {} not found", _0)]
	ClientNotFound(u16)
}

impl MemoryError {
	pub fn new() -> Self {
		let errno = unsafe { dpdk_ffi::_rte_errno() };
		match errno {
			1001 => MemoryError::SecondaryProcess,
			1002 => MemoryError::NoConfig,
			EINVAL => MemoryError::Invalid,
			ENOSPC => MemoryError::NoSpace,
			EEXIST => MemoryError::Exists,
			ENOMEM => MemoryError::NoMem,
			ENOBUFS => MemoryError::NoBuf,
			ENOENT => MemoryError::NoEntries,
			_ => MemoryError::BadVal,
		}
	}
}

#[derive(Error, Debug)]
pub enum BufError {
	/// The offset exceeds the buffer length
	#[error("Offset {} exceed the buffer length {}", _0, _1)]
	BadOffset(usize, usize),

	/// The buffer is not resized
	#[error("Buffer is not resized")]
	NotResized,

	/// The struct exceeds the remaining buffer length
	#[error("Struct size {} exceeds the remaining buffer length {}", _0, _1)]
	OutOfBuffer(usize, usize),
}

#[derive(Error, Debug)]
pub enum PortError {
	#[error("No such device")]
	NoDevice,
	#[error("cache size provided is too large, or priv_size is not aligned.")]
	Invalid,
	#[error("function not supported on this device")]
	NoSupport,
	#[error("bad val")]
	BadVal,
}

impl PortError {
	pub fn new() -> Self {
		let errno = unsafe { dpdk_ffi::_rte_errno() };
		match errno {
			ENODEV => PortError::NoDevice,
			EINVAL => PortError::Invalid,
			ENOTSUP => PortError::NoSupport,
			_ => PortError::BadVal,
		}
	}
}

#[derive(Error, Debug)]
pub enum EALErrors {
	#[error("either a bus or system resource was not available, setup may be attempted again")]
	NoRsrc,
	#[error("that the rte_eal_init function has already been called, and cannot be called again")]
	DuplicateCall,
	#[error("invalid parameters were passed as argv/argc")]
	InvalidArgs,
	#[error("failure likely caused by an out-of-memory condition")]
	NoMem,
	#[error("memory setup issues")]
	BadMemCnfg,
	#[error("EAL cannot initialize on this system")]
	BadSystem,
	#[error("PCI bus is either not present, or is not readable by the eal")]
	BadPCI,
	#[error("service core failed to launch successfully")]
	CoreLaunchFailed,
	#[error("tailq configuration name was not found in memory configuration (in init); error in releasing all resources (in cleanup)")]
	Fault,
	#[error("should not have hit this. code: {}", _0)]
	Weird(i32),
}

impl EALErrors {
	pub fn new() -> Self {
		let errno = unsafe { dpdk_ffi::_rte_errno() };
		match errno {
			EAGAIN => EALErrors::NoRsrc,
			EALREADY => EALErrors::DuplicateCall,
			EINVAL => EALErrors::InvalidArgs,
			ENOMEM => EALErrors::NoRsrc,
			ENODEV => EALErrors::BadMemCnfg,
			ENOTSUP => EALErrors::BadSystem,
			EPROTO => EALErrors::BadPCI,
			ENOEXEC => EALErrors::CoreLaunchFailed,
			EFAULT => EALErrors::Fault,
			_ => EALErrors::Weird(errno),
		}
	}
}


/// Initializes the Environment Abstraction Layer (EAL)
pub fn eal_init(args: Vec<String>) -> Result<(), EALErrors> {
	log::info!("Args: {:?}", &args);
	let len = args.len() as raw::c_int;
	// the panic is fine here since it's due to wrong arguments
	// though this error can be handled better
	let args = args.into_iter().map(|s| CString::new(s).unwrap()).collect::<Vec<_>>();
	let mut ptrs = args
        .iter()
        .map(|s| s.as_ptr() as *mut raw::c_char)
		.collect::<Vec<_>>();
	match unsafe { dpdk_ffi::rte_eal_init(len, ptrs.as_mut_ptr()) } {
		-1 => {
			log::error!("failed to initialize eal");
			Err(EALErrors::new())
		},
		_ => Ok(()),
	}
}

/// Cleans up the Environment Abstraction Layer (EAL).
pub fn eal_cleanup() -> Result<(), EALErrors> {
    match unsafe { dpdk_ffi::rte_eal_cleanup() } {
		0 => Ok(()),
		_ => Err(EALErrors::Fault),
	}
}