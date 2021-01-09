/*
 * Created on Mon Dec 28 2020:12:28:07
 * Created by Ratnadeep Bhattacharya
 */

// DEVFLAGS: development flags - remove in production
#![allow(dead_code)]

use crate::net::MacAddr;

use super::{Mbuf, Mempool, PortError};

pub struct Port {
	pub id: u16,
	pub device: &'static str,
	pub dev_info: dpdk_sys::rte_eth_dev_info,
}

impl Port {
	const PORTMASK: u8 = 0x03;
	const DEFAULT_RSS_HF: u64 = (dpdk_sys::ETH_RSS_IP
		| dpdk_sys::ETH_RSS_TCP
		| dpdk_sys::ETH_RSS_UDP
		| dpdk_sys::ETH_RSS_SCTP
		| dpdk_sys::ETH_RSS_L2_PAYLOAD) as u64;
	const RX_BURST_MAX: u16 = 32;
	const TX_BURST_MAX: u16 = 32;

	const RTE_MP_RX_DESC_DEFAULT: u16 = 512;
	const RTE_MP_TX_DESC_DEFAULT: u16 = 512;

	const RSS_SYMMETRIC_KEY: [u8; 40] = [
		0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d,
		0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a,
		0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a,
	];

	pub fn new(device: &'static str, id: u16) -> Result<Self, PortError> {
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

	pub fn configure(&mut self, num_cores: u16, mempool: &Mempool) -> Result<(), PortError> {
		let mut conf = dpdk_sys::rte_eth_conf::default();

		conf.rxmode.mq_mode = dpdk_sys::rte_eth_rx_mq_mode::ETH_MQ_RX_RSS;
		conf.rxmode.max_rx_pkt_len = dpdk_sys::RTE_ETHER_MAX_LEN;
		conf.rxmode.split_hdr_size = 0;
		conf.rxmode.offloads = dpdk_sys::DEV_RX_OFFLOAD_CHECKSUM as u64;

		conf.rx_adv_conf.rss_conf.rss_hf =
			Self::DEFAULT_RSS_HF & self.dev_info.flow_type_rss_offloads;
		let rss_symmetric_key = &Self::RSS_SYMMETRIC_KEY as *const u8 as *mut u8;
		conf.rx_adv_conf.rss_conf.rss_key = rss_symmetric_key;

		conf.txmode.mq_mode = 0;
		conf.txmode.offloads = 0x00000002 | 0x00000004 | 0x00000008;

		// turns on optimization for fast release of mbufs.
		if self.dev_info.tx_offload_capa & dpdk_sys::DEV_TX_OFFLOAD_MBUF_FAST_FREE as u64 > 0 {
			conf.txmode.offloads |= dpdk_sys::DEV_TX_OFFLOAD_MBUF_FAST_FREE as u64;
		}

		// configure the device
		match unsafe { dpdk_sys::rte_eth_dev_configure(self.id, num_cores, num_cores, &conf) } {
			0 => {}
			_ => return Err(PortError::new()),
		};

		// queue set up
		let rx_conf = &self.dev_info.default_rxconf;
		let tx_conf = &self.dev_info.default_txconf;

		let mut n_cores = num_cores;

		if num_cores % 2 == 1 {
			n_cores += 1;
		} else {
			n_cores = num_cores;
		}

		for i in 0..n_cores {
			unsafe {
				match dpdk_sys::rte_eth_rx_queue_setup(
					self.id,
					i,
					Self::RTE_MP_RX_DESC_DEFAULT,
					dpdk_sys::rte_eth_dev_socket_id(self.id) as u32,
					rx_conf,
					mempool.get_ptr(),
				) {
					0 => {}
					_ => {
						let e = PortError::new();
						log::error!("main: couldn't set up rx queue for port {}: {}", self.id, e);
						return Err(e);
					}
				}

				match dpdk_sys::rte_eth_tx_queue_setup(
					self.id,
					i,
					Self::RTE_MP_TX_DESC_DEFAULT,
					dpdk_sys::rte_eth_dev_socket_id(self.id) as u32,
					tx_conf,
				) {
					0 => {}
					_ => {
						let e = PortError::new();
						log::error!("main: couldn't set up tx queue for port {}: {}", self.id, e);
						return Err(e);
					}
				}
			}
		}

		// sets the port's promiscuous mode
		match unsafe { dpdk_sys::rte_eth_promiscuous_enable(self.id) } {
			0 => {}
			_ => return Err(PortError::new()),
		};
		Ok(())
	}

	/// Start the port
	pub fn start(&self) -> Result<(), PortError> {
		unsafe {
			match dpdk_sys::rte_eth_dev_start(self.id) {
				0 => Ok(()),
				_ => Err(PortError::new()),
			}
		}
	}

	/// Get user device in PCI notation
	pub fn get_name(&self) -> &str {
		self.device
	}

	/// Get mac address for port
	pub fn mac_addr(&self) -> Result<MacAddr, PortError> {
		unsafe {
			let mac = dpdk_sys::rte_ether_addr::default();
			match dpdk_sys::rte_eth_macaddr_get(self.id, &mac as *const _ as *mut _) {
				0 => Ok(MacAddr::from_ether_addr(mac)),
				_ => Err(PortError::new()),
			}
		}
	}

	pub fn receive(&self, queue_id: u16) -> Vec<Mbuf> {
		const RX_BURST_MAX: usize = 32;
		let mut ptrs = Vec::with_capacity(RX_BURST_MAX);

		let len = unsafe {
			dpdk_sys::_rte_eth_rx_burst(self.id, queue_id, ptrs.as_mut_ptr(), RX_BURST_MAX as u16)
		};

		unsafe {
			ptrs.set_len(len as usize);
			ptrs.into_iter()
				.map(|ptr| Mbuf::from_ptr(ptr))
				.collect::<Vec<_>>()
		}
	}

	/// Send packets out of the port
	pub fn send(&self, pkts: Vec<Mbuf>, queue_id: u16) -> usize {
		let len = pkts.len();
		let mut ptrs = pkts.into_iter().map(Mbuf::into_ptr).collect::<Vec<_>>();

		let count = unsafe {
			dpdk_sys::_rte_eth_tx_burst(
				self.id,
				queue_id,
				// ptrs.as_ptr() as *mut *mut dpdk_sys::rte_mbuf,
				ptrs.as_mut_ptr(),
				ptrs.len() as u16,
			) as usize
		};
		if count < len {
			super::mbuf_free_bulk(ptrs);
		}
		count
	}
}
