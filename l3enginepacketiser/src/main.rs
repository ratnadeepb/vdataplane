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

use packetiser::{Packetiser, RoutingTable};
use state::Storage;

pub const BURST_MAX: usize = 512;
pub(crate) static TABLE: Storage<RoutingTable> = Storage::new();

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
    println!("packetiser: created routing table");
    while true {

    }
}
