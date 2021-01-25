//! Implements the support for TCP at the packetiser

use smoltcp::{
	iface::EthernetInterface,
	socket::{
		IcmpEndpoint, IcmpPacketMetadata, IcmpSocket, IcmpSocketBuffer, SocketHandle, SocketSet,
		TcpSocket, TcpSocketBuffer,
	},
	time::Instant,
};
use std::{collections::HashMap, sync::Arc};

use crate::send_icmp_ping;

use super::{icmp, EthDevEmulator, IfaceEmulator};

pub(crate) struct TcpSock<'a>(TcpSocket<'a>);

impl<'a> TcpSock<'a> {
	pub(crate) fn new() -> Self {
		let rx_buf = TcpSocketBuffer::new(vec![0; 1024]);
		let tx_buf = TcpSocketBuffer::new(vec![0; 1024]);
		let tcp_socket = TcpSocket::new(rx_buf, tx_buf);
		Self(tcp_socket)
	}
}

pub(crate) struct IcmpSock<'a>(IcmpSocket<'a>);

impl<'a> IcmpSock<'a> {
	const IDENT: u16 = 0x22b;

	pub(crate) fn new() -> Self {
		let rx_buffer = IcmpSocketBuffer::new(vec![IcmpPacketMetadata::EMPTY], vec![0; 256]);
		let tx_buffer = IcmpSocketBuffer::new(vec![IcmpPacketMetadata::EMPTY], vec![0; 256]);
		let icmp_socket = IcmpSocket::new(rx_buffer, tx_buffer);
		Self(icmp_socket)
	}

	// pub(crate) fn process_icmp(&self) {
	// 	let mut send_at = Instant::from_millis(0);
	// 	let mut seq_no = 0;
	// 	let mut received = 0;
	// 	let mut echo_payload = [0xffu8; 40];
	// 	let mut waiting_queue = HashMap::new();
	// 	let ident = 0x22b;
	// 	loop {
	// 		let timestamp = Instant::now();
	// 	}
	// }
}

pub(crate) struct SockSet<'a> {
	iface: EthernetInterface<'a, EthDevEmulator>,
	sock_set: SocketSet<'a>,
	icmp_handle: SocketHandle,
	tcp_handle: SocketHandle,
}

impl<'a> SockSet<'a> {
	pub(crate) fn new(mac: [u8; 6], addr: [u8; 4], prefix: u8) -> Self {
		let iface = IfaceEmulator::new(mac, addr, prefix).new_iface();
		let mut sock_set = SocketSet::new(vec![]);
		let icmp_handle = sock_set.add(IcmpSock::new().0);
		let tcp_handle = sock_set.add(TcpSock::new().0);
		// let sock_set = Arc::new(sock_set);
		Self {
			iface,
			sock_set,
			icmp_handle,
			tcp_handle,
		}
	}

	pub(crate) fn process_pkts(&mut self) {
		loop {
			let timestamp = Instant::now();
			// let sock_set = *self.sock_set.clone();
			match self.iface.poll(&mut self.sock_set, timestamp) {
				Ok(_) => {}
				Err(e) => log::error!("poll error: {}", e),
			}
			{
				let _timestamp = Instant::now();
				{
					let mut icmp_sock = self.sock_set.get::<IcmpSocket>(self.icmp_handle);
					#[cfg(feature = "debug")]
					println!("got icmp socket");
					if !icmp_sock.is_open() {
						icmp_sock
							.bind(IcmpEndpoint::Ident(IcmpSock::IDENT))
							.unwrap();
					}
					// TODO: hand over to icmp processing
				}
				{
					let mut _tcp_sock = self.sock_set.get::<TcpSocket>(self.tcp_handle);
					#[cfg(feature = "debug")]
					println!("got tcp socket");
					// TODO: hand over to tcp processing}
				}
			}
		}
	}
}
