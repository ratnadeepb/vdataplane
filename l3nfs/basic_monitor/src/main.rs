//! Look into the metadata of every packet and figure out if it belongs to a new connection or an existing one

use l3enginelib::process::ConnIdentity;
use std::collections::HashMap;

struct ConnTable {
    map: HashMap<u64, ConnIdentity>,
}

fn main() {
    println!("Hello, world!");
}
