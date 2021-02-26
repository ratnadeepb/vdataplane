//! The five tuple extracts and stores the following information from the packets
//! 	Source MAC
//! 	Source IP
//! 	Destination MAC
//! 	Destination IP
//! 	Service Port

use etherparse::{InternetSlice, LinkSlice, ReadError, SlicedPacket, TransportSlice};
use l3enginelib::{Mbuf, Mempool};
use pnet::{
    datalink::MacAddr,
    packet::{
        arp::{ArpHardwareTypes, ArpOperations, MutableArpPacket},
        ethernet::{EtherTypes, MutableEthernetPacket},
        MutablePacket, Packet,
    },
};
use std::net::Ipv4Addr;
use thiserror::Error;

#[derive(Error, Debug)]
pub(crate) enum FiveTupleErr {
    #[error("Invalid link")]
    InvalidLink,
    #[error("Invalid IP")]
    InvalidIP,
    #[error("Invalid transport")]
    InvalidTransport,
    #[error("Rejecting UDP packets")]
    NoUDP,
    #[error("Wrong destination mac")]
    WrongMAC,
    #[error("Wrong destination IP")]
    WrongIP,
    #[error("No such destination port in use")]
    WrongPort,
    #[error("Rejecting IPv6 packets")]
    NoIPv6,
    #[error("Read Error: {}", _0)]
    ReadError(ReadError),
}

impl From<ReadError> for FiveTupleErr {
    fn from(e: ReadError) -> Self {
        Self::ReadError(e)
    }
}

/// FiveTuple holds six pieces of information
/// source and destination mac of the packet
/// source and destination ip of the packet
/// the destination port
/// the ether type
pub(crate) struct FiveTuple {
    src_mac: [u8; 6],
    src_ip: Ipv4Addr,
    dst_mac: [u8; 6],
    dst_ip: Ipv4Addr,
    dst_port: u16,
    ethertype: u16,
}

impl FiveTuple {
    pub(crate) fn parse_pkt(
        pkt: &[u8],
        local_ip: Ipv4Addr,
        local_mac: [u8; 6],
        _ports: &[u16],
    ) -> Result<Self, FiveTupleErr> {
        let sliced_pkt = SlicedPacket::from_ethernet(pkt)?;
        let linkslice = sliced_pkt.link.ok_or(FiveTupleErr::InvalidLink)?;
        let ipslice = sliced_pkt.ip.ok_or(FiveTupleErr::InvalidIP)?;
        let transportslice = sliced_pkt.transport.ok_or(FiveTupleErr::InvalidTransport)?;
        let link = match linkslice {
            LinkSlice::Ethernet2(l) => l.to_header(),
        };
        let ip = match ipslice {
            InternetSlice::Ipv4(ip) => ip,
            _ => return Err(FiveTupleErr::NoIPv6),
        };
        let transport = match transportslice {
            TransportSlice::Tcp(tcp) => tcp,
            TransportSlice::Udp(_) => return Err(FiveTupleErr::NoUDP),
        };
        let src_mac = link.source;
        let dst_mac = link.destination;
        let ethertype = link.ether_type;
        let src_ip = ip.source_addr();
        let dst_ip = ip.destination_addr();
        let dst_port = transport.destination_port();
        if dst_ip != local_ip {
            return Err(FiveTupleErr::WrongIP);
        }

        if dst_mac != local_mac {
            return Err(FiveTupleErr::WrongMAC);
        }

        Ok(Self {
            src_ip,
            src_mac,
            dst_mac,
            dst_ip,
            dst_port,
            ethertype,
        })
    }

    /// Get the Ethertype
    pub(crate) fn ethertype(&self) -> u16 {
        self.ethertype.to_be()
    }

    /// Convert IP address to u32
    pub(crate) fn ipaddr_to_u32(ip: &Ipv4Addr) -> u32 {
        let p = ip.octets();
        (((p[0] & 0xFF) as u32) << 24)
            | (((p[1] & 0xFF) as u32) << 16)
            | (((p[2] & 0xFF) as u32) << 8)
            | ((p[1] & 0xFF) as u32)
    }

    fn arr_to_macaddr(mac: [u8; 6]) -> MacAddr {
        MacAddr::new(mac[0], mac[1], mac[2], mac[3], mac[4], mac[5])
    }

    const ETHERTYPE_ARP: u16 = 0x0806;
    const ARP_OP_REQUEST: u16 = 1;
    const ARP_OP_REPLY: u16 = 2;

    /// Detect ARP packets and send a reply
    pub fn handle_arp(&self, local_ip: Ipv4Addr, local_mac: [u8; 6], mp: &Mempool) -> Option<Mbuf> {
        if local_mac != self.dst_mac || local_ip != self.dst_ip {
            return None;
        }

        if self.ethertype.to_be() == Self::ETHERTYPE_ARP {
            if local_mac == self.dst_mac || local_ip == self.dst_ip {
                let mut eth_buf = [0u8; 42];
                let mut eth_pkt = MutableEthernetPacket::new(&mut eth_buf)?;
                eth_pkt.set_destination(MacAddr::broadcast());
                let source_mac = Self::arr_to_macaddr(local_mac);
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
                arp_packet.set_target_proto_addr(self.src_ip);

                eth_pkt.set_payload(arp_packet.packet_mut());

                match Mbuf::from_bytes(eth_pkt.packet(), mp) {
                    Ok(out_buf) => return Some(out_buf),
                    Err(_) => {}
                }
            }
        }
        None
    }
}
