/*
 * Created on Mon Dec 28 2020:12:28:07
 * Created by Ratnadeep Bhattacharya
 */

use std::{ffi::CString, os::raw::c_char};
use super::PortError;
use crate::{NUM_RX_THREADS, NUM_TX_THREADS};

pub struct Port<'a> {
	pub(crate) id: u16,
	pub(crate) device: &'a str,
	pub(crate) dev_info: dpdk_sys::rte_eth_dev_info,
}

impl<'a> Port<'a> {
	const PORTMASK: u8 = 0x02;
	const DEFAULT_RSS_HF: u64 = (dpdk_sys::ETH_RSS_IP | dpdk_sys::ETH_RSS_TCP |dpdk_sys::ETH_RSS_UDP | dpdk_sys::ETH_RSS_SCTP | dpdk_sys::ETH_RSS_L2_PAYLOAD) as u64;

	const RSS_SYMMETRIC_KEY: [u8; 40] = [
		0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a,
		0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a,
		0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a,
	];

	// pub fn new(device: String) -> Result<Self, PortError> {
	pub fn new(device: &'a str, id: u16) -> Result<Self, PortError> {
		let mut dev_info = dpdk_sys::rte_eth_dev_info::default();
		match unsafe { dpdk_sys::rte_eth_dev_info_get(id, &mut dev_info) } {
			0 => Ok(Self {
					id,
					device,
					dev_info,
				}),
			_ => Err(PortError::new()),
		}
		// let mut id = 0u16;
		// let cstr: CString;
		// match CString::new(&device[..]) {
		// 	Ok(cstring) => cstr = cstring,
		// 	Err(_) => return Err(PortError::NoDevice),
		// }
		// match unsafe { dpdk_sys::rte_eth_dev_get_port_by_name(cstr.as_ptr() as *const c_char, &mut id) } {
		// 	0 => {
		// 		println!("got the port by name"); // debug
		// 		let mut dev_info = dpdk_sys::rte_eth_dev_info::default();
		// 		match unsafe { dpdk_sys::rte_eth_dev_info_get(id, &mut dev_info) } {
		// 			0 => Ok(Self {
		// 					id,
		// 					device,
		// 					dev_info,
		// 				}),
		// 			_ => Err(PortError::new()),
		// 		}
		// 	},
		// 	_ => Err(PortError::new()),
		// }
	}

	pub fn configure(&mut self) -> Result<(), PortError> {
		let mut conf = dpdk_sys::rte_eth_conf::default();

		conf.rxmode.mq_mode = dpdk_sys::rte_eth_rx_mq_mode::ETH_MQ_RX_RSS;
		conf.rxmode.max_rx_pkt_len = dpdk_sys::RTE_ETHER_MAX_LEN;
		conf.rxmode.split_hdr_size = 0;
		conf.rxmode.offloads = dpdk_sys::DEV_RX_OFFLOAD_CHECKSUM as u64;

		conf.rx_adv_conf.rss_conf.rss_hf = Self::DEFAULT_RSS_HF & self.dev_info.flow_type_rss_offloads;
		let rss_symmetric_key = &Self::RSS_SYMMETRIC_KEY as *const u8 as *mut u8;
		conf.rx_adv_conf.rss_conf.rss_key = rss_symmetric_key;

		conf.txmode.mq_mode = 0;
		conf.txmode.offloads = 0x00000002 | 0x00000004 | 0x00000008;

		// turns on optimization for fast release of mbufs.
		if self.dev_info.tx_offload_capa & dpdk_sys::DEV_TX_OFFLOAD_MBUF_FAST_FREE as u64 > 0 {
			conf.txmode.offloads |= dpdk_sys::DEV_TX_OFFLOAD_MBUF_FAST_FREE as u64;
		}

		// configure the device
		match unsafe { dpdk_sys::rte_eth_dev_configure(self.id, NUM_RX_THREADS, NUM_TX_THREADS, &conf)} {
			0 => {},
			_ => return Err(PortError::new()),
		};

		// sets the port's promiscuous mode
		match unsafe { dpdk_sys::rte_eth_promiscuous_enable(self.id) } {
			0 => {},
			_ => return Err(PortError::new()),
		};
		Ok(())
	}

	/// Get user device in PCI notation
	pub fn get_name(&self) -> &str {
		self.device
	}
}