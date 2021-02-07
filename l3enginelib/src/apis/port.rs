//! The Port structure is a wrapper around physical NIC ports

// TODO: It might be a good idea for Port to hold a mapping between core id and send and receive queues. This is currently done in `txbin` with `queue_id ^ 1` for the transmit queue and by ensuring that every Port always sets up an even number of queues.

// DEVFLAGS: development flags - remove in production
#![allow(dead_code)]

// use crate::net::MacAddr;
// use pnet::datalink::MacAddr;
use std::marker::{Send, Sync};

use super::{Mbuf, Mempool, PortError};

#[derive(Clone, Copy)]
pub struct Port {
	pub id: u16,
	pub device: &'static str,
	pub dev_info: dpdk_sys::rte_eth_dev_info,
}

unsafe impl Sync for Port {}
unsafe impl Send for Port {}

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
		let mut n_cores = num_cores;

		if num_cores % 2 == 1 {
			n_cores += 1;
		} else {
			n_cores = num_cores;
		}

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
		#[cfg(feature = "debug")]
		println!("configuring port");
		match unsafe { dpdk_sys::rte_eth_dev_configure(self.id, n_cores, n_cores, &conf) } {
			0 => {}
			_ => return Err(PortError::new()),
		};

		// queue set up
		let rx_conf = &self.dev_info.default_rxconf;
		let tx_conf = &self.dev_info.default_txconf;

		#[cfg(feature = "debug")]
		println!("n_cores: {}", n_cores);

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

				#[cfg(feature = "debug")]
				println!("rx queue configured");

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
				#[cfg(feature = "debug")]
				println!("tx queue configured");
			}
		}
		#[cfg(feature = "debug")]
		println!("all queues configured");

		// sets the port's promiscuous mode
		match unsafe { dpdk_sys::rte_eth_promiscuous_enable(self.id) } {
			0 => {
				#[cfg(feature = "debug")]
				println!("promiscuous mode enabled");
			}
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

	pub fn receive(&self, queue_id: u16, sz: usize) -> Vec<Mbuf> {
		// const RX_BURST_MAX: usize = 32;
		let mut ptrs = Vec::with_capacity(sz);

		let len =
			unsafe { dpdk_sys::_rte_eth_rx_burst(self.id, queue_id, ptrs.as_mut_ptr(), sz as u16) };

		unsafe {
			ptrs.set_len(len as usize);
			ptrs.into_iter()
				.map(|ptr| Mbuf::from_ptr(ptr))
				.collect::<Vec<_>>()
		}
	}

	/// Send packets out of the port
	pub fn send(&self, pkts: &Vec<Mbuf>, queue_id: u16) -> usize {
		let len = pkts.len();
		let mut ptrs = pkts.into_iter().map(Mbuf::get_ptr).collect::<Vec<_>>();

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
