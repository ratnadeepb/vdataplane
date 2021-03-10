#![allow(unreachable_code)]
use std::{io::Write, os::unix::net::UnixStream};

use crossbeam::channel::{bounded, Receiver, Sender, TryRecvError, TrySendError};
use memenpsf::Interface;
use rand::random;
// use rayon::spawn;
use std::thread::spawn;

const CAP: usize = 32;

struct Comm<T> {
    sender: Sender<T>,
    recvr: Receiver<T>,
}

impl<T> Comm<T> {
    fn new(sender: Sender<T>, recvr: Receiver<T>) -> Self {
        Self { sender, recvr }
    }

    fn send(&self, msg: T) -> Result<(), TrySendError<T>> {
        self.sender.try_send(msg)
    }

    fn recv(&self) -> Result<T, TryRecvError> {
        self.recvr.try_recv()
    }
}

fn main() {
    let sock_name = "/tmp/fd-passrd.socket";
    let mut stream = UnixStream::connect(sock_name).unwrap();
    let name = format!("eth{}", random::<u8>());
    let cap = CAP;
    let typ = 0; // client
    let buf = "client1".as_bytes();
    stream.write(&buf).unwrap();
    let (s1, recvr) = bounded::<[u8; 2]>(CAP);
    let (sender, r1) = bounded::<[u8; 2]>(CAP);
    let _worker = spawn(move || Interface::<[u8; 2]>::run(name, cap, typ, stream, recvr, sender));
    let comm = Comm::new(s1, r1);

    // REVIEW: This infinite loop here ensures that the client reads after the server writes
    // This is essentially multiple writes to ensure that the server gets the data
    // This needs to be rectified
    loop {
        let msg = [1, 2];
        // let msg = "client1".as_bytes();
        match comm.send(msg) {
            Ok(()) => {}
            Err(_) => {}
        }

        match comm.recv() {
            Ok(buf) => println!("got data: {:#?}", buf),
            Err(_e) => {}
        };

        let msg2 = [3, 4];
        // let msg2 = "client1".as_bytes();
        match comm.send(msg2) {
            Ok(()) => {}
            Err(_) => {}
        }
    }

    match _worker.join() {
        Ok(_) => {}
        Err(e) => eprintln!("error joining thread: {:#?}", e),
    }
    println!("worker joined");
}
