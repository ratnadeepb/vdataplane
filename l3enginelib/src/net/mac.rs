/*
 * Created on Mon Dec 28 2020:13:47:53
 * Created by Ratnadeep Bhattacharya
 */

// development flags - remove in production
#![allow(dead_code)]
#![allow(unused_imports)]

use super::MacError;
use crate::apis::{Mbuf, Port};
use std::{result::Result, fmt};
use dpdk_sys;
use smoltcp::wire::EthernetAddress;

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

	pub fn to_ethernetaddr(&self) -> EthernetAddress {
		EthernetAddress::from_bytes(&self.0)
	}
}

impl fmt::Debug for MacAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct(&format!("{}", self.to_ethernetaddr())).finish()
    }
}