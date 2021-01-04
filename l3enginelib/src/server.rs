/*
 * Created on Sat Jan 02 2021:20:03:19
 * Created by Ratnadeep Bhattacharya
 */

use std::{collections::HashMap, fmt};

use crate::apis::{Mbuf, Mempool};
use smoltcp::wire::{Ipv4Address, EthernetAddress};

pub struct Server {
	sockets: HashMap<Ipv4Address, Vec<EthernetAddress>>, 
}

impl Server {
	/// Create a new server
	pub fn new() -> Self {
		Self { sockets: HashMap::new() }
	}

	/// Add a new (ip, mac) socket to the server
	pub fn add(&mut self, ip: Ipv4Address, mac: EthernetAddress) {
		self.sockets.insert(ip, [mac].to_vec());
	}

	/// Add another mac to the same IP
	pub fn add_mac_to_ip(&mut self, ip: Ipv4Address, mac: EthernetAddress) {
		if self.sockets.contains_key(&ip) {
			self.sockets.entry(ip).and_modify(|val| val.push(mac));
		}
	}

	/// Add multiple macs to the same IP
	pub fn add_macs(&mut self, ip: Ipv4Address, mac: Vec<EthernetAddress>) {
		self.sockets.insert(ip, mac);
	}

	/// Convert IP address to u32
	pub fn ipaddr_to_u32(ip: &Ipv4Address) -> u32 {
		let mut ip_addr = Box::new(0u32); // ensure memory allocation is in heap
		let mut arr: [i8; 4] = [0; 4];
		let bytes = ip.as_bytes();
		for i in 0..4 {
			arr[i] = bytes[i] as i8;
		}

		unsafe { dpdk_sys::_pkt_parse_ip(arr.as_mut_ptr(), &mut *ip_addr); }
		*ip_addr
	}

	/// Detect if a packet is an ARP Request
	pub fn detect_arp(&self, buf: &Mbuf) -> Option<Ipv4Address> {
		for key in self.sockets.keys() {
			let sip = Self::ipaddr_to_u32(key);
			unsafe {
				match dpdk_sys::_pkt_detect_arp(buf.get_ptr(), sip) {
					1 => return Some(*key),
					_ => continue,
				}
			}
		}
		None
	}

	/// Generate an ARP Request if an incoming packet is an ARP Request meant for us
	pub fn send_arp_reply(&self, buf: &Mbuf, mp: &Mempool) -> Option<Mbuf> {
		match self.detect_arp(buf) {
			Some(_ip) => {
				unsafe {
					let ether_hdr = dpdk_sys::_pkt_ether_hdr(buf.get_ptr());
					let ipv4_hdr = dpdk_sys::_pkt_ipv4_hdr(buf.get_ptr());
					let tip = (*ipv4_hdr).dst_addr;
					let sip = (*ipv4_hdr).src_addr;
					let tha = &(*ether_hdr).d_addr as *const _ as *mut _;
					let frm = &(*ether_hdr).s_addr as *const _ as *mut _;
					Some(Mbuf::from_ptr(
						dpdk_sys::_pkt_arp_response(tha, frm, tip, sip, mp.get_ptr())
					))
				}
			},
			None => None,
		}
	}
}

impl fmt::Debug for Server {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct(&format!("{:?}", self.sockets)).finish()
    }
}