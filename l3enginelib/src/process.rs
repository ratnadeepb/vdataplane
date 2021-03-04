use crossbeam::queue::ArrayQueue;
use l3enginelib::Mbuf;
use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
    net::Ipv4Addr,
    sync::Arc,
};

#[derive(Debug)]
struct ConnState {
    rx_win: u16,
    seq_num: u32,
    ack_num: u32,
}

#[derive(Hash)]
struct ConnIdentity {
    dst_ip: Ipv4Addr,
    dst_mac: [u8; 6],
    dst_port: u16,
    src_ip: Ipv4Addr,
    src_mac: [u8; 6],
    src_port: u16,
}

fn hash_conn(conn: ConnIdentity) -> u64 {
    #[cfg(feature = "debug")]
    println!("hashing connection");
    let mut hasher = DefaultHasher::new();
    conn.hash(&mut hasher);
    hasher.finish()
}

fn parse_u32_2_ipv4(ip: u32) -> Ipv4Addr {
    Ipv4Addr::from(ip)
}

fn parse_pkt(pkt: &Mbuf) -> Option<(u64, ConnState)> {
    #[cfg(feature = "debug")]
    println!("parsing packet");
    unsafe {
        let ether_hdr_ptr = dpdk_sys::_pkt_ether_hdr(pkt.get_ptr());
        if ether_hdr_ptr.is_null() {
            return None;
        }

        let ether_hdr = *ether_hdr_ptr;
        let dst_mac: [u8; 6] = ether_hdr.d_addr.addr_bytes;
        let src_mac: [u8; 6] = ether_hdr.s_addr.addr_bytes;

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
        let src_port = tcp_hdr.src_port;
        let dst_port = tcp_hdr.dst_port;
        let seq_num = tcp_hdr.sent_seq;
        let ack_num = tcp_hdr.recv_ack;
        let rx_win = tcp_hdr.rx_win;

        let conn_id = ConnIdentity {
            dst_ip,
            dst_mac,
            dst_port,
            src_ip,
            src_mac,
            src_port,
        };
        let h = hash_conn(conn_id);
        let conn_state = ConnState {
            rx_win,
            seq_num,
            ack_num,
        };
        Some((h, conn_state))
    }
}

pub(crate) fn process(in_pkts: Arc<ArrayQueue<Mbuf>>, _out_pkts: Arc<ArrayQueue<Mbuf>>) {
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
                    Some((k, v)) => {
                        let _ = connections.insert(k, v);
                    }
                    None => drop(pkt),
                },
                None => {}
            }
        }
    }
}
