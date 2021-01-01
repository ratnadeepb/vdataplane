/*
 * Created on Thu Dec 31 2020:13:50:59
 * Created by Ratnadeep Bhattacharya
 */

mod packetiser;

use packetiser::{Packetiser, RoutingTable};
use state::Storage;

pub const BURST_MAX: usize = 512;
pub(crate) static TABLE: Storage<RoutingTable> = Storage::new();

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
