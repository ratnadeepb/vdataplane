/*
 * Created on Sat Jan 02 2021:20:03:19
 * Created by Ratnadeep Bhattacharya
 */

use std::{fmt, slice};

use crate::apis::{Mbuf, Mempool};
use pnet::{
	datalink::{MacAddr, NetworkInterface},
	ipnetwork::IpNetwork,
	packet::{
		arp::{ArpHardwareTypes, ArpOperations, ArpPacket, MutableArpPacket},
		ethernet::{EtherTypes, EthernetPacket, MutableEthernetPacket},
		MutablePacket, Packet,
	},
};
use std::net::Ipv4Addr;

pub struct Server {
	interfaces: Vec<NetworkInterface>,
}

impl Server {
	/// Create a new server
	pub fn new() -> Self {
		Self {
			interfaces: Vec::new(),
		}
	}

	/// Add a new (ip, mac) socket to the server
	pub fn add(
		&mut self,
		name: &str,
		desc: &str,
		index: u32,
		mac: Option<MacAddr>,
		ips: Vec<IpNetwork>,
		flags: u32,
	) {
		let iface = NetworkInterface {
			name: String::from(name),
			description: String::from(desc),
			index,
			mac,
			ips,
			flags,
		};
		self.interfaces.push(iface);
	}

	/// Get interface by IP
	fn get_iface_by_ip(&self, mip: &Ipv4Addr) -> Option<NetworkInterface> {
		for iface in &self.interfaces {
			for ip_nw in &iface.ips {
				match ip_nw {
					IpNetwork::V4(ip) => {
						if &ip.ip() == mip {
							return Some(iface.clone());
						}
					}
					IpNetwork::V6(_) => return None,
				}
			}
		}
		None
	}

	/// Convert IP address to u32
	pub fn ipaddr_to_u32(ip: &Ipv4Addr) -> u32 {
		let p = ip.octets();
		(((p[0] & 0xFF) as u32) << 24)
			| (((p[1] & 0xFF) as u32) << 16)
			| (((p[2] & 0xFF) as u32) << 8)
			| ((p[1] & 0xFF) as u32)
	}

	/// Detect if a packet is an ARP Request
	/// Return Some((local_ip, remote_ip))
	/// look at `man netdev` for the meaning of flags
	pub fn detect_arp(&self, buf: &Mbuf) -> Option<(Ipv4Addr, Ipv4Addr)> {
		let pkt = unsafe { dpdk_sys::_pkt_raw_addr(buf.get_ptr()) };
		let len = buf.data_len();
		let mut eth_buf = unsafe { slice::from_raw_parts(pkt, len) };

		match EthernetPacket::new(&mut eth_buf) {
			Some(eth_pkt) => match eth_pkt.get_ethertype() {
				EtherTypes::Arp => {
					for iface in &self.interfaces {
						for ip_nw in &iface.ips {
							match ip_nw {
								IpNetwork::V4(ip) => {
									let local_ip = ip.ip();
									let arp = ArpPacket::new(eth_pkt.payload())?;
									let tip = arp.get_target_proto_addr();
									let remote_ip = arp.get_sender_proto_addr();
									if tip == local_ip {
										#[cfg(feature = "debug")]
										println!(
											"local and remote IP: {:#?}->{:#?}",
											local_ip, remote_ip
										);
										return Some((local_ip, remote_ip));
									}
								}
								IpNetwork::V6(_) => return None,
							}
						}
					}
					return None;
				}
				_ => return None,
			},
			None => None,
		}
	}

	/// Generate an ARP Request if an incoming packet is an ARP Request meant for us
	pub fn send_arp_reply(&self, buf: &Mbuf, mp: &Mempool) -> Option<Mbuf> {
		let (local_ip, remote_ip) = self.detect_arp(buf)?;
		let mut eth_buf = [0u8; 42];
		let mut eth_pkt = MutableEthernetPacket::new(&mut eth_buf)?;
		let iface = self.get_iface_by_ip(&local_ip)?;
		let source_mac = iface.mac?;
		eth_pkt.set_destination(MacAddr::broadcast());
		eth_pkt.set_source(source_mac);
		eth_pkt.set_ethertype(EtherTypes::Arp);

		let mut arp_buffer = [0u8; 28];
		let mut arp_packet = MutableArpPacket::new(&mut arp_buffer)?;
		arp_packet.set_hardware_type(ArpHardwareTypes::Ethernet);
		arp_packet.set_protocol_type(EtherTypes::Ipv4);
		arp_packet.set_hw_addr_len(6);
		arp_packet.set_proto_addr_len(4);
		arp_packet.set_operation(ArpOperations::Request);
		arp_packet.set_sender_hw_addr(source_mac);
		arp_packet.set_sender_proto_addr(local_ip);
		arp_packet.set_target_hw_addr(MacAddr::zero());
		arp_packet.set_target_proto_addr(remote_ip);

		eth_pkt.set_payload(arp_packet.packet_mut());

		match Mbuf::from_bytes(eth_pkt.packet(), mp) {
			Ok(out_buf) => Some(out_buf),
			Err(_) => None,
		}
	}

	// Provide an ICMP Reply
	// pub fn icmp_reply(&self, buf: &mut Mbuf, mp: &Mempool) -> Option<Mbuf> {
	// 	unsafe {
	// 		let icmp_hdr = dpdk_sys::_pkt_icmp_hdr(buf.get_ptr());
	// 		if icmp_hdr.is_null() {
	// 			return None;
	// 		}
	// 		#[cfg(feature = "debug")]
	// 		println!("ICMP packet detected");
	// 		let ether_hdr = dpdk_sys::_pkt_ether_hdr(buf.get_ptr());
	// 		let ipv4_hdr = dpdk_sys::_pkt_ipv4_hdr(buf.get_ptr());

	// 		if (*icmp_hdr).icmp_type == dpdk_sys::RTE_IP_ICMP_ECHO_REQUEST as u8
	// 			&& (*icmp_hdr).icmp_code == 0
	// 		{
	// 			drop(buf);
	// 			match Mbuf::new(mp) {
	// 				Ok(icmp_pkt) => {
	// 					let out_ether_hdr = dpdk_sys::_pkt_ether_hdr(icmp_pkt.get_ptr());
	// 					let out_ipv4_hdr = dpdk_sys::_pkt_ipv4_hdr(icmp_pkt.get_ptr());
	// 					let mut out_icmp_hdr = dpdk_sys::_pkt_icmp_hdr(icmp_pkt.get_ptr());

	// 					// invert the src and dst addresses
	// 					dpdk_sys::_rte_ether_addr_copy(
	// 						&mut (*ether_hdr).s_addr,
	// 						&mut (*out_ether_hdr).d_addr,
	// 					);
	// 					dpdk_sys::_rte_ether_addr_copy(
	// 						&mut (*ether_hdr).d_addr,
	// 						&mut (*out_ether_hdr).s_addr,
	// 					);

	// 					(*ipv4_hdr).src_addr = (*out_ipv4_hdr).dst_addr;
	// 					(*ipv4_hdr).dst_addr = (*out_ipv4_hdr).src_addr;

	// 					(*icmp_hdr).icmp_type = dpdk_sys::RTE_IP_ICMP_ECHO_REPLY as u8;

	// 					(*out_icmp_hdr).icmp_cksum =
	// 						dpdk_sys::_pkt_icmp_checksum((*icmp_hdr).icmp_cksum);

	// 					return Some(icmp_pkt);
	// 				}
	// 				Err(e) => {
	// 					log::error!("main: failed to create packet: {}", e);
	// 					return None;
	// 				}
	// 			}
	// 		}
	// 	}
	// 	None
	// }
}

impl fmt::Debug for Server {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct(&format!("{:#?}", self.interfaces)).finish()
	}
}
