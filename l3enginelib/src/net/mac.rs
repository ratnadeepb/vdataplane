/*
 * Created on Mon Dec 28 2020:13:47:53
 * Created by Ratnadeep Bhattacharya
 */

// development flags - remove in production
#![allow(dead_code)]
#![allow(unused_imports)]

use super::MacError;
use std::result::Result;
use dpdk_sys;

pub struct MacAddr([u8; 6]);

impl MacAddr {
	#[inline]
	pub fn into_ether_addr(mac: Self) -> dpdk_sys::rte_ether_addr {
		dpdk_sys::rte_ether_addr { addr_bytes: mac.0 }
	}

	#[inline]
	pub fn from_ether_addr(mac: dpdk_sys::rte_ether_addr) -> Self {
		Self(mac.addr_bytes)
	}

	// pub fn print_mac_addrs() -> Result<(), MacError> {
	// 	for port in 0..=1 {

	// 	}
	// }
}