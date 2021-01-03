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

use std::{process::exit, thread::sleep, time::Duration};

use packetiser::{Packetiser, RoutingTable};
use state::Storage;
use zmq::Context;

pub const BURST_MAX: usize = 512;
pub(crate) static TABLE: Storage<RoutingTable> = Storage::new();

const PACKETISER_ZMQ_PORT: &str = "tcp://localhost:5555";

// DEVFLAGS: development flags - remove in production
#[allow(while_true)]
// use packetiser;
fn main() {
    packetiser::start();
    let _proc = packetiser::Packetiser::new(BURST_MAX);
    
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
    while true {
    }
}
