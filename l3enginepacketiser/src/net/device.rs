//! Create an emulated device
//!
//! This will create a single device on the packetiser process
//! ideally for all network cards in the system

use std::collections::BTreeMap;

use smoltcp::{
	iface::{EthernetInterface, EthernetInterfaceBuilder, NeighborCache},
	phy::{Device, DeviceCapabilities, RxToken, TxToken},
	time::Instant,
	wire::{EthernetAddress, IpAddress, IpCidr, Ipv4Address, Ipv4Cidr},
	Result,
};

use crate::packetiser::Packetiser;

pub(crate) struct EthDevEmulator {
	incoming: Vec<u8>,
	outgoing: Vec<u8>,
}

impl EthDevEmulator {
	const MTU: usize = 1536;

	pub(crate) fn new() -> Self {
		Self {
			incoming: Vec::with_capacity(Self::MTU),
			outgoing: Vec::with_capacity(Self::MTU),
		}
	}
}

pub(crate) struct EthDevRxToken<'a>(&'a mut [u8]);

impl<'a> RxToken for EthDevRxToken<'a> {
	fn consume<R, F>(mut self, _timestamp: Instant, f: F) -> Result<R>
	where
		F: FnOnce(&mut [u8]) -> Result<R>,
	{
		f(&mut self.0)
	}
}

pub(crate) struct EthDevTxToken<'a>(&'a mut [u8]);

impl<'a> TxToken for EthDevTxToken<'a> {
	fn consume<R, F>(self, _timestamp: Instant, len: usize, f: F) -> Result<R>
	where
		F: FnOnce(&mut [u8]) -> Result<R>,
	{
		f(&mut self.0[..len])
	}
}

impl<'a> Device<'a> for EthDevEmulator {
	type RxToken = EthDevRxToken<'a>;
	type TxToken = EthDevTxToken<'a>;

	fn receive(&'a mut self) -> Option<(Self::RxToken, Self::TxToken)> {
		Some((
			EthDevRxToken(&mut self.incoming[..]),
			EthDevTxToken(&mut self.outgoing[..]),
		))
	}

	fn transmit(&'a mut self) -> Option<Self::TxToken> {
		Some(EthDevTxToken(&mut self.outgoing[..]))
	}

	fn capabilities(&self) -> DeviceCapabilities {
		let mut caps = DeviceCapabilities::default();
		caps.max_transmission_unit = Self::MTU;
		caps.max_burst_size = Some(1);
		caps
	}
}

pub(crate) struct IfaceEmulator {
	device: EthDevEmulator,
	mac: EthernetAddress,
	ip: IpCidr,
}

impl<'a> IfaceEmulator {
	pub(crate) fn new(mac: [u8; 6], addr: [u8; 4], prefix: u8) -> Self {
		let mac = EthernetAddress(mac);
		let ipv4 = IpAddress::Ipv4(Ipv4Address::new(addr[0], addr[0], addr[0], addr[0]));
		let ip = IpCidr::new(ipv4, prefix);
		let device = EthDevEmulator::new();
		Self { device, mac, ip }
	}

	pub(crate) fn new_iface(self) -> EthernetInterface<'a, EthDevEmulator> {
		EthernetInterfaceBuilder::new(self.device)
			.ethernet_addr(self.mac)
			.neighbor_cache(NeighborCache::new(BTreeMap::new()))
			.ip_addrs([self.ip])
			.finalize()
	}
}
