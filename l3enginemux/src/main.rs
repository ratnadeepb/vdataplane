mod mux;

use crossbeam::{
    channel::{bounded, Receiver, Sender},
    sync::ShardedLock,
    thread,
};
use l3enginelib::Mbuf;
use memenpsf::MemEnpsf;
use mux::*;
use state::Storage;

use std::{
    collections::HashMap,
    fs,
    net::Ipv4Addr,
    os::unix::net::{UnixListener, UnixStream},
    ptr,
    slice::from_raw_parts_mut,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::spawn,
};

use log;

const SOCK_NAME: &str = "/tmp/fd-passrd.socket";
const CAP: usize = 20;
const BURST_SZ: usize = 512;
const MEMENPSF: &str = "memenpsf";
const MTU: usize = 1536; // NOTE: Definition in multiple places

// static MEMPOOL: Storage<Mempool> = Storage::new();
// static SERVICE_MAP: ShardedLock<HashMap<&str, Sender<&mut [u8]>>> =
//     ShardedLock::new(HashMap::new());
static MUX: Storage<Mux> = Storage::new();

/// Handle Ctrl+C
fn handle_signal(kr: Arc<AtomicBool>) {
    ctrlc::set_handler(move || {
        kr.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");
}

fn handle_client(name: &str, stream: UnixStream, cons: Receiver<Mbuf>) {
    let mut dev = MemEnpsf::new(name, CAP, stream);
    let mux = MUX.get();
    loop {
        // TODO: introduce ctrl + c
        match cons.recv() {
            Ok(buf) => {
                let pkt = unsafe { dpdk_sys::_pkt_raw_addr(buf.into_ptr()) };
                let pkt = unsafe { ptr::read(pkt as *const _ as *const [u8; MTU]) };
                match dev.xmit_to_client(pkt) {
                    Ok(_) => {}
                    Err(_e) => {
                        log::error!("Buffer full: dropping packet");
                        #[cfg(feature = "debug")]
                        println!("Buffer full: dropping packet");
                    }
                }
            }
            Err(_) => {
                log::info!("channel has been closed");
                #[cfg(featuer = "debug")]
                println!("channel has been closed");
                return;
            }
        }
        match dev.recv_from_client() {
            Some(pkt) => {
                if let Ok(buf) = Mbuf::from_bytes(&pkt, &mux.mempool()) {
                    &mux.out_buf.push(buf).unwrap();
                    #[cfg(feature = "debug")]
                    println!("Added packet to outbuf");
                }
            }
            None => {}
        }
    }
}

// fn route_pkts(local: LocalIPMac, pkt: Mbuf, mp: &Mempool) {
// let buf = unsafe { from_raw_parts_mut(dpdk_sys::_pkt_raw_addr(pkt.get_ptr()), MTU) };
// let tuple = match FiveTuple::parse_pkt(buf, &local, &vec![0][..]) {
//     Ok(f) => f,
//     Err(e) => {
//         log::error!("Dropping packet because: {:#?}", e);
//         return;
//     }
// };
// if tuple.ethertype() == FiveTuple::ETHERTYPE_ARP {
//     tuple.handle_arp(local, mp);
// }
// // NOTE: Service name is hardcoded for now
// let srvc = "dummy";
// if let Some(sender) = service_map.get(&srvc) {}
// }

fn main() {
    let mux = Mux::new().unwrap(); // fatal failure
    #[cfg(feature = "debug")]
    println!("mux created");
    MUX.set(mux);
    let mux = MUX.get();
    mux::start();
    #[cfg(feature = "debug")]
    println!("mux started");

    let mac = [0x90, 0xe2, 0xba, 0xb2, 0x98, 0x48];
    let ip = Ipv4Addr::new(10, 10, 1, 1);
    let local = LocalIPMac::new(ip, mac);

    // handling Ctrl+C
    let keep_running = Arc::new(AtomicBool::new(true));
    // let kr = keep_running.clone();
    handle_signal(keep_running.clone());
    fs::remove_file(SOCK_NAME).ok();
    let service_map: ShardedLock<HashMap<&str, Sender<Mbuf>>> = ShardedLock::new(HashMap::new());
    let _listener_thd = thread::scope(|s| {
        s.spawn(|_| {
            let listener = UnixListener::bind(SOCK_NAME).unwrap();
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        let (send, recv) = bounded(BURST_SZ);
                        let l = match service_map.write() {
                            Ok(mut map) => {
                                map.insert("dummy", send);
                                map.len()
                            }
                            Err(p_err) => {
                                // Handles the case if another thread panicked
                                // while holding the lock
                                let mut map = p_err.into_inner();
                                map.insert("dummy", send);
                                map.len()
                            }
                        };
                        let name = format!("{}{}", MEMENPSF, l);
                        spawn(move || {
                            handle_client(&name[..], stream, recv);
                        });
                    }
                    Err(e) => log::error!("failed to connect: {}", e),
                }
            }
        });
    })
    .unwrap();
    let mux = Mux::new().unwrap(); // fatal failure
    #[cfg(feature = "debug")]
    println!("mux created");
    mux::start();
    #[cfg(feature = "debug")]
    println!("mux started");

    let mac = [0x90, 0xe2, 0xba, 0x87, 0x6b, 0xa4];
    let ip = Ipv4Addr::new(10, 10, 1, 1);
    let local = LocalIPMac::new(ip, mac);

    // handling Ctrl+C
    let keep_running = Arc::new(AtomicBool::new(true));
    // let kr = keep_running.clone();
    handle_signal(keep_running.clone());
    #[cfg(feature = "debug")]
    println!("main loop starting");
    while keep_running.load(Ordering::SeqCst) {
        // receive packets
        let mut _sz = 0;
        while mux.in_buf.is_empty() {
            _sz = mux.recv_from_engine_burst();
            #[cfg(feature = "debug")]
            println!("received {} packets", _sz);
        }

        // processing received packets
        for _ in 0..mux.in_buf.len() {
            match mux.in_buf.pop() {
                Some(pkt) => {
                    // route_pkts(local, pkt, &mux.mempool());
                    thread::scope(|s| {
                        s.spawn(|_| {
                            let buf = unsafe {
                                from_raw_parts_mut(dpdk_sys::_pkt_raw_addr(pkt.get_ptr()), MTU)
                            };
                            let tuple = match FiveTuple::parse_pkt(buf, &local, &vec![0][..]) {
                                Ok(f) => f,
                                Err(e) => {
                                    log::error!("Dropping packet because: {:#?}", e);
                                    drop(pkt);
                                    return;
                                }
                            };
                            if tuple.ethertype() == FiveTuple::ETHERTYPE_ARP {
                                tuple.handle_arp(local, &mux.mempool());
                            }
                            // NOTE: Service name is hardcoded for now
                            let srvc = "dummy";
                            match service_map.read() {
                                Ok(map) => {
                                    // map.get(&srvc).unwrap().to_owned()
                                    // TODO: remove the unwraps
                                    let sender = map.get(&srvc).unwrap();
                                    sender.send(pkt).unwrap();
                                }
                                Err(p_err) => {
                                    let map = p_err.into_inner();
                                    let sender = map.get(&srvc).unwrap();
                                    sender.send(pkt).unwrap();
                                }
                            };
                        });
                    })
                    .unwrap();
                }
                None => break,
            }
        }

        // TODO: Send packets to clients or drop them
        // TODO: Check packets received from clients and send them out
    }
    // match listener_thd.join() {
    //     Ok(_) => {}
    //     Err(e) => log::error!("listener thread did not exit cleanly: {:#?}", e),
    // }
}
