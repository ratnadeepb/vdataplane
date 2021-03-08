//! This is the library module that provides structures like
//! Mbuf, Mempool, Ring, Channel, RingClientMap, Port and associated functions
//!
//! These structures and functions enable the user to interact with DPDK in a safe manner
//! and without also having to manually figure out certain interaction semantics

pub mod apis;
pub mod process;

pub use apis::*;
pub use process::*;
