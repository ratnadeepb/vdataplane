/*
 * Created on Sat Jan 09 2021:13:25:02
 * Created by Ratnadeep Bhattacharya
 */

use crate::{OUT_PKTS, PROCESSOR_THREAD, PROC_CHANNEL, TO_PACKETISER};
use l3enginelib::apis::Port;

pub(crate) fn send_pkts_out(ports: &Vec<Port>) -> usize {
	let queue_id = unsafe { dpdk_sys::_rte_lcore_id() as u16 };
	let out_pkts = OUT_PKTS.get();
	let mut pkts = Vec::with_capacity(out_pkts.len());
	if out_pkts.is_empty() {
		return 0usize;
	}
	// for _ in 0..sz {
	// 	match out_pkts.pop() {
	// 		Some(pkt) => pkts.push(pkt),
	// 		None => break,
	// 	}
	// }
	while let Some(pkt) = out_pkts.pop() {
		pkts.push(pkt);
	}
	ports[0].send(pkts, queue_id ^ 1)
}

pub(crate) fn send_to_packetiser() -> usize {
	let queue_id = unsafe { dpdk_sys::_rte_lcore_id() as u16 };
	let ring_pkts = TO_PACKETISER.get();
	let ch = PROC_CHANNEL.get();
	if ring_pkts.is_empty() {
		return 0usize;
	}
	let mut len = 0;
	while let Some(pkt) = ring_pkts.pop() {
		match ch.send(PROCESSOR_THREAD, pkt) {
			Ok(()) => len += 1,
			Err(_) => break,
		}
	}
	len
}
