/*
 * Created on Sat Jan 02 2021:20:03:19
 * Created by Ratnadeep Bhattacharya
 */

use std::{collections::HashMap, fmt, os::raw::c_char};

use crate::apis::{Mbuf, Mempool, WrappedCString};
use smoltcp::wire::{EthernetAddress, Ipv4Address};

pub struct Server {
	sockets: HashMap<Ipv4Address, Vec<EthernetAddress>>,
}

impl Server {
	/// Create a new server
	pub fn new() -> Self {
		Self {
			sockets: HashMap::new(),
		}
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
		let ip_str = WrappedCString::to_cstring(format!("{}", ip)).unwrap();

		unsafe {
			dpdk_sys::_pkt_parse_ip(ip_str.as_ptr() as *mut c_char, &mut *ip_addr);
		}
		*ip_addr
	}

	/// Detect if a packet is an ARP Request
	pub fn detect_arp(&self, buf: &Mbuf) -> Option<Ipv4Address> {
		let arp_hdr = unsafe { *dpdk_sys::_pkt_arp_hdr(buf.get_ptr()) };
		println!("{:#?}", arp_hdr);
		for key in self.sockets.keys() {
			let sip = Self::ipaddr_to_u32(key);
			unsafe {
				match dpdk_sys::_pkt_detect_arp(buf.get_ptr(), sip) {
					1 => return Some(*key),
					_ => {}
				}
			}
		}
		None
	}

	/// Generate an ARP Request if an incoming packet is an ARP Request meant for us
	pub fn send_arp_reply(&self, buf: &mut Mbuf, mp: &Mempool) -> Option<Mbuf> {
		match self.detect_arp(buf) {
			Some(ip) => unsafe {
				let ether_hdr = dpdk_sys::_pkt_ether_hdr(buf.get_ptr());
				let ipv4_hdr = dpdk_sys::_pkt_ipv4_hdr(buf.get_ptr());
				let tip = (*ipv4_hdr).dst_addr;
				let sip = Self::ipaddr_to_u32(&ip);
				let tha = &(*ether_hdr).d_addr as *const _ as *mut _;
				let frm = &(*ether_hdr).s_addr as *const _ as *mut _;
				Some(Mbuf::from_ptr(dpdk_sys::_pkt_arp_response(
					tha,
					frm,
					tip,
					sip,
					mp.get_ptr(),
				)))
			},
			None => None,
		}
	}

	/// Provide an ICMP Reply
	pub fn icmp_reply(&self, buf: &mut Mbuf, mp: &Mempool) -> Option<Mbuf> {
		unsafe {
			let icmp_hdr = dpdk_sys::_pkt_icmp_hdr(buf.get_ptr());
			if icmp_hdr.is_null() {
				return None;
			}
			#[cfg(feature = "debug")]
			println!("ICMP packet detected");
			let ether_hdr = dpdk_sys::_pkt_ether_hdr(buf.get_ptr());
			let ipv4_hdr = dpdk_sys::_pkt_ipv4_hdr(buf.get_ptr());

			if (*icmp_hdr).icmp_type == dpdk_sys::RTE_IP_ICMP_ECHO_REQUEST as u8
				&& (*icmp_hdr).icmp_code == 0
			{
				drop(buf);
				match Mbuf::new(mp) {
					Ok(icmp_pkt) => {
						let out_ether_hdr = dpdk_sys::_pkt_ether_hdr(icmp_pkt.get_ptr());
						let out_ipv4_hdr = dpdk_sys::_pkt_ipv4_hdr(icmp_pkt.get_ptr());
						let mut out_icmp_hdr = dpdk_sys::_pkt_icmp_hdr(icmp_pkt.get_ptr());

						// invert the src and dst addresses
						dpdk_sys::_rte_ether_addr_copy(
							&mut (*ether_hdr).s_addr,
							&mut (*out_ether_hdr).d_addr,
						);
						dpdk_sys::_rte_ether_addr_copy(
							&mut (*ether_hdr).d_addr,
							&mut (*out_ether_hdr).s_addr,
						);

						(*ipv4_hdr).src_addr = (*out_ipv4_hdr).dst_addr;
						(*ipv4_hdr).dst_addr = (*out_ipv4_hdr).src_addr;

						(*icmp_hdr).icmp_type = dpdk_sys::RTE_IP_ICMP_ECHO_REPLY as u8;

						(*out_icmp_hdr).icmp_cksum =
							dpdk_sys::_pkt_icmp_checksum((*icmp_hdr).icmp_cksum);

						return Some(icmp_pkt);
					}
					Err(e) => {
						log::error!("main: failed to create packet: {}", e);
						return None;
					}
				}
			}
		}
		None
	}
}

impl fmt::Debug for Server {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct(&format!("{:?}", self.sockets)).finish()
	}
}
