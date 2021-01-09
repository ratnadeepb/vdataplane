/*
 * Created on Sat Jan 09 2021:12:00:39
 * Created by Ratnadeep Bhattacharya
 */

use crate::{
	Server, FROM_PACKETISER, MEMPOOL, OUT_PKTS, PROCESSOR_THREAD, PROC_CHANNEL, TO_PACKETISER,
};
use crossbeam_queue::SegQueue;
use l3enginelib::apis::{Mbuf, Mempool, Port};
use state::Storage;

pub(crate) fn get_external_pkts(ports: &Vec<Port>, server: &Server) -> usize {
	let queue_id = unsafe { dpdk_sys::_rte_lcore_id() as u16 };
	let pkts = ports[0].receive(queue_id);
	let out_pkts = OUT_PKTS.get();
	let ring_pkts = TO_PACKETISER.get();
	let len = pkts.len();

	for mut pkt in pkts {
		let ether_hdr = unsafe { dpdk_sys::_pkt_ether_hdr(pkt.get_ptr()) };
		if !ether_hdr.is_null() {
			let ether_type = unsafe { (*ether_hdr).ether_type };
			if ether_type != 0 {
				#[cfg(feature = "debug")]
				println!("ether type: {}", ether_type);
				match server.detect_arp(&pkt) {
					Some(_ip) => {
						let mp = MEMPOOL.get();
						if let Some(out_arp) = server.send_arp_reply(&mut pkt, mp) {
							#[cfg(feature = "debug")]
							println!("arp type");
							out_pkts.push(out_arp);
						}
					}
					None => {
						if ether_type == 4 {
							// IPv4 packet
							#[cfg(feature = "debug")]
							println!("ipv4 type");
							ring_pkts.push(pkt);
						}
					}
				}
			} else {
				drop(pkt);
			}
		} else {
			drop(pkt);
		}
	}
	len
}

pub(crate) fn get_from_packetiser() -> usize {
	let queue_id = unsafe { dpdk_sys::_rte_lcore_id() as u16 };
	let out_pkts = OUT_PKTS.get();
	let ring_pkts = FROM_PACKETISER.get();
	let ch = PROC_CHANNEL.get();
	let len = ring_pkts.len(); // number of packets packetiser has sent
	let mp = MEMPOOL.get();
	let mut pkts = Vec::with_capacity(len);
	for l in 0..len {
		match Mbuf::new(mp) {
			Ok(buf) => pkts.push(buf),
			Err(_) => {
				pkts.truncate(l);
				break;
			}
		}
	}
	let pkt_len = pkts.len(); // number of buffers created
	let mut s_len = 0; // number of packets gotten out of the packetiser ring
	for mut pkt in pkts {
		match ch.receive(PROCESSOR_THREAD, &mut pkt) {
			Ok(()) => {
				out_pkts.push(pkt);
				s_len += 1;
			}
			Err(_) => break,
		}
	}

	len - s_len // number of packets still left in the packetiser ring
}
