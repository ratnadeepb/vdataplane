//! Look into the metadata of every packet and figure out if it belongs to a new connection or an existing one

use std::{io::Write, os::unix::net::UnixStream};

use crossbeam::channel::{bounded, Receiver, Sender, TryRecvError, TrySendError};
use memenpsf::Interface;
use rand::random;
use std::thread::spawn;

use l3enginelib::{ipc::new_int, process::ConnIdentity};
use std::collections::HashMap;

const CAP: usize = 32; // NOTE: should match the CAP in server ipc.rs

fn monitor(bufs: Vec<[u8; 24]>) {
    #[cfg(feature = "debug")]
    {
        for buf in bufs {
            println!("in monitor: {:#?}", buf);
        }
    }
}

fn main() {
    let sock_name = "/tmp/fd-passrd.socket";
    let mut stream = UnixStream::connect(sock_name).unwrap();
    let name = format!("eth{}", random::<u8>());
    let cap = CAP;
    let typ = 0; // client
    let buf = "basic_monitor".as_bytes();

    match stream.write(&buf) {
        Ok(sz) => println!("sent name: {}", sz),
        Err(e) => println!("error sending name: {}", e),
    }
    stream.flush();

    let interface = new_int(name, cap, stream, typ);

    loop {
        let bufs = interface.recv_vectored();
        if bufs.len() > 0 {
            monitor(bufs);
        } else {
            println!("no bufs received");
        }
        let pkt = [0; 24];
        match interface.xmit(pkt) {
            Ok(_) => {}
            Err(_) => {}
        }
    }
}
