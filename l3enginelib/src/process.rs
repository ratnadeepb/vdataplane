use crate::Mbuf;
use bincode::{deserialize, serialize};
use crossbeam::queue::ArrayQueue;
use serde::{Deserialize, Serialize};
use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    convert::TryInto,
    hash::{Hash, Hasher},
    net::Ipv4Addr,
    sync::Arc,
};

#[derive(Debug)]
pub(crate) struct ConnState {
    rx_win: u16,
    seq_num: u32,
    ack_num: u32,
}

#[derive(Hash, Serialize, Deserialize, Debug)]
pub struct ConnIdentity {
    dst_ip: Ipv4Addr,
    dst_mac: [u8; 6],
    dst_port: u16,
    src_ip: Ipv4Addr,
    src_mac: [u8; 6],
    src_port: u16,
}

const MBUF_BIN_SZ: usize = 24;

pub(crate) fn serialize_conn(buf: &Mbuf) -> Option<[u8; MBUF_BIN_SZ]> {
    // testing indicates connection identity is encoded to 24 bytes
    match parse_pkt(&buf) {
        Some((conn_id, _)) => match serialize(&conn_id) {
            Ok(v) => Some(v.try_into().unwrap_or_default()),
            Err(_) => None,
        },
        None => None,
    }
}

pub(crate) fn deserialize_conn(bytes: [u8; MBUF_BIN_SZ]) -> Option<ConnIdentity> {
    match deserialize(&bytes) {
        Ok(conn) => Some(conn),
        Err(_e) => {
            #[cfg(feature = "debug")]
            println!("failed to deserialize: {:#?}", _e);
            None
        }
    }
}

pub fn hash_conn(conn: ConnIdentity) -> u64 {
    #[cfg(feature = "debug")]
    println!("hashing connection");
    let mut hasher = DefaultHasher::new();
    conn.hash(&mut hasher);
    hasher.finish()
}

fn parse_u32_2_ipv4(ip: u32) -> Ipv4Addr {
    Ipv4Addr::from(u32::from_be(ip))
}

fn parse_mac(mac: [u8; 6]) -> [u8; 6] {
    let mut te_mac = [0; 6];
    for i in 0..6 {
        te_mac[i] = u8::from_be(mac[i]);
    }
    te_mac
}

fn parse_pkt(pkt: &Mbuf) -> Option<(ConnIdentity, ConnState)> {
    #[cfg(feature = "debug")]
    println!("parsing packet");
    unsafe {
        let ether_hdr_ptr = dpdk_sys::_pkt_ether_hdr(pkt.get_ptr());
        if ether_hdr_ptr.is_null() {
            return None;
        }

        let ether_hdr = *ether_hdr_ptr;
        let dst_mac: [u8; 6] = parse_mac(ether_hdr.d_addr.addr_bytes);
        let src_mac: [u8; 6] = parse_mac(ether_hdr.s_addr.addr_bytes);

        let ip_hdr_ptr = dpdk_sys::_pkt_ipv4_hdr(pkt.get_ptr());
        if ip_hdr_ptr.is_null() {
            return None;
        }

        let ip_hdr = *ip_hdr_ptr;
        let src_ip = parse_u32_2_ipv4(ip_hdr.src_addr);
        let dst_ip = parse_u32_2_ipv4(ip_hdr.dst_addr);

        let tcp_hdr_ptr = dpdk_sys::_pkt_tcp_hdr(pkt.get_ptr());
        if tcp_hdr_ptr.is_null() {
            return None;
        }

        let tcp_hdr = *tcp_hdr_ptr;
        let src_port = u16::from_be(tcp_hdr.src_port);
        let dst_port = u16::from_be(tcp_hdr.dst_port);
        let seq_num = u32::from_be(tcp_hdr.sent_seq);
        let ack_num = u32::from_be(tcp_hdr.recv_ack);
        let rx_win = u16::from_be(tcp_hdr.rx_win);

        let conn_id = ConnIdentity {
            dst_ip,
            dst_mac,
            dst_port,
            src_ip,
            src_mac,
            src_port,
        };
        let conn_state = ConnState {
            rx_win,
            seq_num,
            ack_num,
        };
        Some((conn_id, conn_state))
    }
}

pub fn process(in_pkts: Arc<ArrayQueue<Mbuf>>, _out_pkts: Arc<ArrayQueue<Mbuf>>) {
    // The key is the hash value of a connection identity
    let mut connections: HashMap<u64, ConnState> = HashMap::new();

    loop {
        // if there are packets to process
        if !in_pkts.is_empty() {
            // if in_pkts.is_full() {
            #[cfg(feature = "debug")]
            {
                let core_id = unsafe { dpdk_sys::_rte_lcore_id() as u16 };
                println!("Process core index: {}", unsafe {
                    dpdk_sys::rte_lcore_index(core_id as i32)
                });
                println!("Process Socket ID: {}", unsafe {
                    dpdk_sys::rte_socket_id()
                });
                println!("processing packet");
            }
            match in_pkts.pop() {
                Some(pkt) => match parse_pkt(&pkt) {
                    Some((conn_id, conn_state)) => {
                        #[cfg(feature = "debug")]
                        println!("serialising in process");
                        // match serialize_conn(&conn_id) {
                        //     Some(_buf) => {
                        //         #[cfg(feature = "debug")]
                        //         println!("got conn serliased, size: {}", _buf.len());
                        //     }
                        //     None => {
                        //         #[cfg(feature = "debug")]
                        //         println!("serialise err");
                        //     }
                        // }
                        let h = hash_conn(conn_id);
                        let _ = connections.insert(h, conn_state);
                    }
                    None => {
                        #[cfg(feature = "debug")]
                        println!("dropping in process");
                        drop(pkt);
                    }
                },
                None => {}
            }
        }
    }
}
