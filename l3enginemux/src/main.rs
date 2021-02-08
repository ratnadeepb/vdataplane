mod mux;

use crossbeam::{
    channel::{bounded, Receiver, Sender},
    sync::ShardedLock,
    thread,
};
use l3enginelib::Mbuf;
use memenpsf::MemEnpsf;
use mux::*;

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
const MUX_ZMQ_PORT: &str = "tcp://localhost:5555";

// static MEMPOOL: Storage<Mempool> = Storage::new();
// static SERVICE_MAP: ShardedLock<HashMap<&str, Sender<&mut [u8]>>> =
//     ShardedLock::new(HashMap::new());
// static MUX: Storage<Mux> = Storage::new();

/// Handle Ctrl+C
fn handle_signal(kr: Arc<AtomicBool>) {
    ctrlc::set_handler(move || {
        kr.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");
}

fn handle_client(name: &str, stream: UnixStream, cons: Receiver<Mbuf>, mux: Arc<Mux>) {
    let mut dev = MemEnpsf::new(name, CAP, stream);
    // let mux = MUX.get();
    loop {
        // TODO: introduce ctrl + c
        match cons.recv() {
            Ok(buf) => {
                // original location of Mbuf forgotten here
                let pkt = unsafe { dpdk_sys::_pkt_raw_addr(buf.into_ptr()) };
                let pkt = unsafe { ptr::read(pkt as *const _ as *const [u8; MTU]) };
                match dev.xmit_to_client(pkt) {
                    Ok(_) => {}
                    Err(_e) => {
                        log::error!("Buffer full: packet dropped");
                        #[cfg(feature = "debug")]
                        println!("Buffer full: packet dropped");
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

fn route_pkts(
    service_map: Arc<ShardedLock<HashMap<&str, Sender<Mbuf>>>>,
    local: LocalIPMac,
    pkt: Mbuf,
    mux: Arc<Mux>,
) {
    let buf = unsafe { from_raw_parts_mut(dpdk_sys::_pkt_raw_addr(pkt.get_ptr()), MTU) };
    let tuple = match FiveTuple::parse_pkt(buf, &local, &vec![0][..]) {
        Ok(f) => f,
        Err(e) => {
            log::error!("Dropping packet because: {:#?}", e);
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
}

fn main() {
    mux::start();
    #[cfg(feature = "debug")]
    println!("mux started");

    let mux = Arc::new(Mux::new().unwrap()); // fatal failure
    #[cfg(feature = "debug")]
    println!("mux created");

    let mac = [0x90, 0xe2, 0xba, 0x87, 0x6b, 0xa4];
    let ip = Ipv4Addr::new(10, 10, 1, 1);
    let local = LocalIPMac::new(ip, mac);

    #[cfg(feature = "debug")]
    println!("packetiser: sending ready msg to main");
    let context = zmq::Context::new();
    let requester = context.socket(zmq::REQ).unwrap(); // fatal error
    assert!(requester.connect(MUX_ZMQ_PORT).is_ok());
    requester.send("Hello", 0).unwrap();
    #[cfg(feature = "debug")]
    println!("packetiser: sent ready msg to main");

    // handling Ctrl+C
    let keep_running = Arc::new(AtomicBool::new(true));
    // let kr = keep_running.clone();
    handle_signal(keep_running.clone());
    fs::remove_file(SOCK_NAME).ok();
    let service_map: Arc<ShardedLock<HashMap<&str, Sender<Mbuf>>>> =
        Arc::new(ShardedLock::new(HashMap::new()));
    let service_map_clone = service_map.clone();
    let mux_clone = mux.clone();
    let _listener_thd = thread::scope(|s| {
        s.spawn(|_| {
            let listener = UnixListener::bind(SOCK_NAME).unwrap();
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        let cloned_mux = mux_clone.clone();
                        let (send, recv) = bounded(BURST_SZ);
                        let l = match service_map_clone.write() {
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
                            handle_client(&name[..], stream, recv, cloned_mux);
                        });
                    }
                    Err(e) => log::error!("failed to connect: {}", e),
                }
            }
        });
    })
    .unwrap();

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
                    let srvc_map = service_map.clone();
                    let mux_clone = mux.clone();
                    thread::scope(|s| {
                        s.spawn(|_| {
                            route_pkts(srvc_map, local, pkt, mux_clone);
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
}
