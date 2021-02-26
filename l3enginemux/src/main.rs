mod mux;

use log;
use mux::Mux;
use std::{
    net::Ipv4Addr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

const CAP: usize = 20;
const BURST_SZ: usize = 512;
const MTU: usize = 1536; // NOTE: Definition in multiple places
const MUX_ZMQ_PORT: &str = "tcp://localhost:5555";
const G_MEMPOOL_NAME: &str = "GLOBAL_MEMPOOL"; // NOTE: Definition in multiple places

/// Handle Ctrl+C
fn handle_signal(kr: Arc<AtomicBool>) {
    ctrlc::set_handler(move || {
        kr.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");
}

fn route_pkts() {}

fn main() {
    mux::start();
    #[cfg(feature = "debug")]
    println!("mux started");

    let mx = Mux::new().unwrap(); // fatal failure
    #[cfg(feature = "debug")]
    println!("mux created");

    let mac = [0x90, 0xe2, 0xba, 0x87, 0x6b, 0xa4];
    let ip = Ipv4Addr::new(192, 168, 0, 2);

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
    handle_signal(keep_running.clone());
    let kr = keep_running.clone();

    let mx = Mux::new().unwrap(); // a failure here should crash the system

    #[cfg(feature = "debug")]
    println!("main loop starting");
    while keep_running.load(Ordering::SeqCst) {
        // receive packets
        let mut _rsz = 0;
        if !mx.in_buf.is_full() {
            _rsz = mx.recv_from_engine_burst();
            #[cfg(feature = "debug")]
            if _rsz > 0 {
                println!("received {} packets", _rsz);
            }
        }

        // processing received packets
        for _ in 0..mx.in_buf.len() {
            match mx.in_buf.pop() {
                Some(pkt) => {
                    // send the packets back
                    match mx.out_buf.push(pkt) {
                        Ok(()) => {}
                        Err(pkt) => drop(pkt),
                    }
                }
                None => break,
            }
        }
        // TODO: Send packets to clients or drop them
        if mx.out_buf.len() > 0 {
            let _xsz = mx.xmit_to_engine_bulk();
            #[cfg(feature = "debug")]
            println!("Sent out {} packets", _xsz);
        }
        // TODO: Check packets received from clients and send them out
    }
}
