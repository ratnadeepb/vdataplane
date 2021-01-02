/*
 * Created on Mon Dec 28 2020:12:28:07
 * Created by Ratnadeep Bhattacharya
 */

// DEVFLAGS: development flags - remove in production
#![allow(dead_code)]

use super::{Mbuf, Mempool, PortError};

pub struct Port<'a> {
	pub id: u16,
	pub device: &'a str,
	pub dev_info: dpdk_sys::rte_eth_dev_info,
}

impl<'a> Port<'a> {
	const PORTMASK: u8 = 0x03;
	const DEFAULT_RSS_HF: u64 = (dpdk_sys::ETH_RSS_IP | dpdk_sys::ETH_RSS_TCP |dpdk_sys::ETH_RSS_UDP | dpdk_sys::ETH_RSS_SCTP | dpdk_sys::ETH_RSS_L2_PAYLOAD) as u64;
	
	const RX_BURST_MAX: u16 = 32;
	const TX_BURST_MAX: u16 = 32;

	const RSS_SYMMETRIC_KEY: [u8; 40] = [
		0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a,
		0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a,
		0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a,
	];

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
	}

	pub fn configure(&mut self, num_rx_threads: u16, num_tx_threads: u16) -> Result<(), PortError> {
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
		match unsafe { dpdk_sys::rte_eth_dev_configure(self.id, num_rx_threads, num_tx_threads, &conf)} {
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

	/// Receive packets from the port
	pub fn receive(&self, mempool: Mempool, queue_id: u16) -> Vec<Mbuf> {
		// OPTIMISE: horrible constructions all over
		let mut pkts: Vec<Mbuf> = Vec::with_capacity(Self::TX_BURST_MAX as usize);
		for _ in 0..Self::TX_BURST_MAX {
			match Mbuf::new(&mempool) {
				Ok(buf) => pkts.push(buf),
				Err(_) => log::error!("port receive: failed to create mbuf"),
			}
		}

		let mut ptrs: Vec<*mut dpdk_sys::rte_mbuf> = Vec::with_capacity(pkts.len());
		for pkt in &pkts {
			ptrs.push(pkt.get_ptr());
		}

		unsafe { dpdk_sys::_rte_eth_rx_burst(self.id, queue_id, ptrs.as_ptr() as *mut *mut dpdk_sys::rte_mbuf, Self::TX_BURST_MAX) };
		pkts
	}

	/// Send packets out of the port
	pub fn send(&self, pkts: &mut Vec<Mbuf>, queue_id: u16) {
		let mut ptrs = Vec::with_capacity(pkts.len());
		for pkt in pkts {
			ptrs.push(pkt.get_ptr());
		}

		unsafe { dpdk_sys::_rte_eth_tx_burst(self.id, queue_id, ptrs.as_ptr() as *mut *mut dpdk_sys::rte_mbuf, Self::RX_BURST_MAX) };
	}
}