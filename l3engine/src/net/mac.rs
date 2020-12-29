/*
 * Created on Mon Dec 28 2020:13:47:53
 * Created by Ratnadeep Bhattacharya
 */

pub struct MacAddr([u8; 6]);

impl MacAddr {
	#[inline]
	pub fn into_ether_addr(mac: Self) -> dpdk_ffi::rte_ether_addr {
		dpdk_ffi::rte_ether_addr { addr_bytes: mac.0 }
	}

	#[inline]
	pub fn from_ether_addr(mac: dpdk_ffi::rte_ether_addr) -> Self {
		Self(mac.addr_bytes)
	}
}