/*
 * Created on Mon Dec 28 2020:13:48:08
 * Created by Ratnadeep Bhattacharya
 */

use crate::apis::Mbuf;
use smoltcp::wire::Ipv4Address;
use dpdk_ffi;

pub struct Ipv4Hdr(dpdk_ffi::rte_ipv4_hdr);

impl Ipv4Hdr {
	/// Get IPv4 header from mbuf
	pub fn from_mbuf(buf: Mbuf) -> Self {
		Self(unsafe {*(dpdk_ffi::_pkt_ipv4_hdr(buf.get_ptr()) as *mut dpdk_ffi::rte_ipv4_hdr)})
	}

	fn convert(&self, ip: u32) -> [u8; 4] {
		[((ip >> 24) & 0xFF) as u8, ((ip >> 16) & 0xFF) as u8, ((ip >> 8) & 0xFF) as u8, (ip & 0xFF) as u8,]
	}

	/// Get source address
	pub fn get_src_addr(&self) -> Ipv4Address {
		Ipv4Address::from_bytes(&self.convert(self.0.src_addr))
	}

	/// Get destination address
	pub fn get_dst_addr(&self) -> Ipv4Address {
		Ipv4Address::from_bytes(&self.convert(self.0.dst_addr))
	}
}