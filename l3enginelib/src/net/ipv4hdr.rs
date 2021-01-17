use crate::apis::Mbuf;
use std::net::Ipv4Addr;
use dpdk_sys;

pub struct Ipv4Hdr(dpdk_sys::rte_ipv4_hdr);

impl Ipv4Hdr {
	/// Get IPv4 header from mbuf
	pub fn from_mbuf(buf: &mut Mbuf) -> Self {
		Self(unsafe {*(dpdk_sys::_pkt_ipv4_hdr(buf.get_ptr()) as *mut dpdk_sys::rte_ipv4_hdr)})
	}

	fn convert(&self, ip: u32) -> Ipv4Addr {
		Ipv4Addr::new(((ip >> 24) & 0xFF) as u8, ((ip >> 16) & 0xFF) as u8, ((ip >> 8) & 0xFF) as u8, (ip & 0xFF) as u8,)
	}

	/// Get source address
	pub fn get_src_addr(&self) -> Ipv4Addr {
		(&self).convert(self.0.src_addr)
	}

	/// Get destination address
	pub fn get_dst_addr(&self) -> Ipv4Addr {
		(&self).convert(self.0.dst_addr)
	}
}