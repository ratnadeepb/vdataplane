/*
 * Created on Thu Dec 31 2020:13:50:59
 * Created by Ratnadeep Bhattacharya
 */

// DEVFLAGS: development flags - remove in production
#![allow(dead_code)]
#![allow(unused_imports)]

// production flags
// #![warn(
//     missing_docs,
//     rust_2018_idioms,
//     missing_debug_implementations,
//     broken_intra_doc_links
// )]
// #![allow(clippy::type_complexity)]

mod packetiser;

use ctrlc;
use std::{
    process::exit,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::sleep,
    time::Duration,
};

use packetiser::{Packetiser, RoutingTable};
use state::Storage;
use zmq::Context;

pub const BURST_MAX: usize = 512;
pub(crate) static TABLE: Storage<RoutingTable> = Storage::new();

const PACKETISER_ZMQ_PORT: &str = "tcp://localhost:5555";

fn handle_signal(kr: Arc<AtomicBool>) {
    ctrlc::set_handler(move || {
        kr.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");
}

// DEVFLAGS: development flags - remove in production
#[allow(while_true)]
// use packetiser;
fn main() {
    packetiser::start();
    let proc = packetiser::Packetiser::new(BURST_MAX);
    #[cfg(feature = "debug")]
    println!("packetiser created");
    TABLE.set(RoutingTable::new());

    #[cfg(feature = "debug")]
    println!("packetiser: sending ready msg to main");
    let context = Context::new();
    let requester = context.socket(zmq::REQ).unwrap(); // fatal error
    assert!(requester.connect(PACKETISER_ZMQ_PORT).is_ok());
    requester.send("Hello", 0).unwrap();
    #[cfg(feature = "debug")]
    println!("packetiser: sent ready msg to main");

    #[cfg(feature = "debug")]
    println!("packetiser: created routing table");

    // handling Ctrl+C
    let keep_running = Arc::new(AtomicBool::new(true));
    let kr = keep_running.clone();
    handle_signal(keep_running.clone());

    while kr.load(Ordering::SeqCst) {
        match proc.recv_from_engine_bulk() {
            Ok(_count) => {
                // #[cfg(feature = "debug")]
                // println!(
                //     "count: {} and i_bufqueue size: {}",
                //     _count,
                //     proc.i_bufqueue.len()
                // );
            }
            Err(e) => log::error!("Error receiving from engine: {}", e),
        }

        #[cfg(feature = "debug")]
        {
            while !proc.i_bufqueue.is_empty() {
                if let Some(mut pkt) = proc.i_bufqueue.pop() {
                    // check if IP
                    if let Some(ip) = proc.get_ip_hdr(&mut pkt) {
                        println!("Got ipv4 pkt from {:#?}", ip);
                    } else {
                        println!("Not an ipv4 pkt");
                    }
                    if let Err(_) = proc.o_bufqueue.push(pkt) {
                        log::error!("failed to put in out buf");
                    }
                }
            }
            // println!("after while !proc.i_bufqueue.is_empty() loop");

            match proc.send_outgoing_packets() {
                Ok(_) => {
                    // #[cfg(feature = "debug")]
                    // println!("sent packets back to engine");
                }
                Err(e) => log::error!("Error receiving from engine: {}", e),
            }
        }
    }
}
