//! Look into the metadata of every packet and figure out if it belongs to a new connection or an existing one

// use std::{io::Write, os::unix::net::UnixStream};
use async_std::prelude::*;
use async_std::{io::Write, os::unix::net::UnixStream};

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

/// Number of bytes sent should match the number of bytes expected by the server
const CLIENT_NAME_SZ: usize = 30; // should be same as in l3enginelib/ipc

#[async_std::main]
async fn main() {
    let sock_name = "/tmp/fd-passrd.socket";
    let mut stream = UnixStream::connect(sock_name).await.unwrap();
    let name = format!("eth{}", random::<u8>());
    let cap = CAP;
    let typ = 0; // client

    // pad with 0s to make buffer of length CLIENT_NAME_SZ
    let mut b = vec![0; CLIENT_NAME_SZ];
    let buf = "basic_monitor".as_bytes();
    for i in 0..buf.len() {
        b[i] = buf[i];
    }

    match stream.write(&b).await {
        Ok(sz) => println!("sent name: {}", sz),
        Err(e) => println!("error sending name: {}", e),
    }
    // stream.flush();

    let interface = new_int(name, cap, Box::new(stream), typ);

    loop {
        let bufs = interface.recv_vectored();
        if bufs.len() > 0 {
            monitor(bufs);
        } 
        // else {
        //     println!("no bufs received");
        // }
        let pkt = [0; 24];
        match interface.xmit(pkt) {
            Ok(_) => {}
            Err(_) => {}
        }
    }
}
