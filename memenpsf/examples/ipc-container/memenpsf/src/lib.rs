//! A user-space shared memory IPC
//!
//! Control messages - positions of read/write pointers
//! and location recv/xmit queues - are communicated over Unix sockets

use std::{
    cell::RefCell,
    io::{Read, Result, Write},
    os::unix::{io::AsRawFd, net::UnixStream},
    ptr,
};

use crossbeam::channel::{Receiver, Sender};
use ipc_queue::RingBuf;
use libc::{ftruncate, mmap, MAP_SHARED, PROT_READ, PROT_WRITE};

enum HostType {
    Client,
    Server,
}

// constants representing functions performed
const ERR: u8 = 100;
const WRITE: u8 = 1;
const READ: u8 = 2;

// const MTU: usize = 1536;

struct MemEnpsf<T: std::fmt::Debug + std::marker::Copy> {
    name: String,
    s2c_q: RingBuf<T>,
    c2s_q: RingBuf<T>,
    cap: usize,
    stream: UnixStream,
}

impl<'a, T: std::fmt::Debug + std::marker::Copy> MemEnpsf<T> {
    fn new_srv(name: String, cap: usize, mut stream: UnixStream) -> Self {
        let fd = fdpass::recv_fd(&mut stream, vec![0u8]).unwrap();
        unsafe { ftruncate(fd.as_raw_fd(), cap as i64) };
        let shm = unsafe {
            mmap(
                ptr::null_mut(),
                cap,
                PROT_READ | PROT_WRITE,
                MAP_SHARED,
                fd.as_raw_fd(),
                0,
            )
        };
        let bs_s2c = shm;
        let bs_c2s = unsafe { shm.offset(cap as isize) };
        let s2c_q = RingBuf::<T>::new(bs_s2c, cap);
        let c2s_q = RingBuf::<T>::new(bs_c2s, cap);
        Self {
            name,
            s2c_q,
            c2s_q,
            cap,
            stream,
        }
    }

    // Create a new interface for client
    fn new_client(name: String, cap: usize, mut stream: UnixStream) -> Self {
        println!("new client func");
        let fd = shm_open_anonymous::shm_open_anonymous();
        println!("fd created");
        if let Err(e) = fdpass::send_fd(&mut stream, &[0], &fd) {
            println!("Errored out: {:#?}", e);
        }
        println!("fd sent");

        unsafe { ftruncate(fd.as_raw_fd(), cap as i64) };
        let shm = unsafe {
            mmap(
                ptr::null_mut(),
                cap,
                PROT_READ | PROT_WRITE,
                MAP_SHARED,
                fd.as_raw_fd(),
                0,
            )
        };
        println!("fd mapped");

        let bs_s2c = shm;
        let bs_c2s = unsafe { shm.offset(cap as isize) };
        let s2c_q = RingBuf::<T>::new(bs_s2c, cap);
        let c2s_q = RingBuf::<T>::new(bs_c2s, cap);
        println!("ringbufs ready");

        Self {
            name,
            s2c_q,
            c2s_q,
            cap,
            stream,
        }
    }

    fn recv_from_client(&mut self) -> Option<T> {
        self.c2s_q.pop()
    }

    fn xmit_to_client(&mut self, buf: T) -> Result<()> {
        self.s2c_q.push(buf)
    }

    fn recv_from_srv(&mut self) -> Option<T> {
        self.s2c_q.pop()
    }

    fn xmit_to_srv(&mut self, buf: T) -> Result<()> {
        self.c2s_q.push(buf)
    }
}

pub struct Interface<T: std::fmt::Debug + std::marker::Copy> {
    stype: HostType,
    int: RefCell<MemEnpsf<T>>, // allow mutating the queue without a mut ref to Interface
}

impl<'a, T: std::fmt::Debug + std::marker::Copy> Interface<T> {
    pub fn new(name: String, cap: usize, stream: UnixStream, typ: u8) -> Self {
        let int;
        let stype;
        if typ == 0 {
            stype = HostType::Client;
            int = RefCell::new(MemEnpsf::<T>::new_client(name, cap, stream));
            println!("new client interface");
        } else {
            stype = HostType::Server;
            int = RefCell::new(MemEnpsf::new_srv(name, cap, stream));
        }
        Self { stype, int }
    }

    pub fn cap(&self) -> usize {
        self.int.borrow().cap
    }

    pub fn name(&self) -> String {
        String::from(&self.int.borrow().name)
    }

    pub fn stype(&self) -> &str {
        match self.stype {
            HostType::Server => "server",
            HostType::Client => "client",
        }
    }

    pub fn send_ctrl_msg(&self, msg: [u8; 2], r_or_w: u8) -> Result<usize> {
        let mut ctrl_msg = [0; 4];
        ctrl_msg[0] = 0; // reserved if we want to introduce a UUID later
        ctrl_msg[1] = r_or_w; // was a read performed or a write?
        ctrl_msg[2] = msg[0];
        ctrl_msg[3] = msg[1];
        self.int.borrow_mut().stream.write(&ctrl_msg)
    }

    pub fn recv_ctrl_msg(&self) {
        let mut ctrl_msg = [0; 4];
        let r_or_w = match self.int.borrow_mut().stream.read(&mut ctrl_msg) {
            Ok(_) => ctrl_msg[1],
            Err(_) => ERR,
        };

        match self.stype {
            HostType::Client => {
                if r_or_w == WRITE {
                    // server sent data, update write pointer in client on server to client queue
                    self.int.borrow().s2c_q.update_write_index(ctrl_msg[2]);
                } else if r_or_w == READ {
                    // server read data, update read pointer in client on client to server queue
                    self.int.borrow().c2s_q.update_read_index(ctrl_msg[3]);
                }
            }
            HostType::Server => {
                if r_or_w == WRITE {
                    // client sent data, update write pointer in server on client to server queue
                    self.int.borrow().c2s_q.update_write_index(ctrl_msg[2]);
                } else {
                    // client read data, update read pointer in server on server to client queue
                    self.int.borrow().s2c_q.update_read_index(ctrl_msg[3]);
                }
            }
        }
    }

    /// Transmit a packet across the interface
    pub fn xmit(&self, buf: T) -> Result<()> {
        match self.stype {
            HostType::Client => {
                let res = self.int.borrow_mut().xmit_to_srv(buf);
                let ctrl = self.int.borrow().c2s_q.indices();
                match self.send_ctrl_msg(ctrl, WRITE) {
                    Ok(_sz) => {}
                    Err(_e) => {}
                }
                res
            }
            HostType::Server => {
                let res = self.int.borrow_mut().xmit_to_client(buf);
                let ctrl = self.int.borrow().s2c_q.indices();
                match self.send_ctrl_msg(ctrl, WRITE) {
                    Ok(_sz) => {}
                    Err(_e) => {}
                }
                res
            }
        }
    }

    fn recv(&self) -> Option<T> {
        match self.stype {
            HostType::Client => {
                let res = self.int.borrow_mut().recv_from_srv();
                let ctrl = self.int.borrow().s2c_q.indices();
                match self.send_ctrl_msg(ctrl, READ) {
                    Ok(_sz) => {}
                    Err(_e) => {}
                }
                res
            }
            HostType::Server => {
                let res = self.int.borrow_mut().recv_from_client();
                let ctrl = self.int.borrow().c2s_q.indices();
                match self.send_ctrl_msg(ctrl, READ) {
                    Ok(_sz) => {}
                    Err(_e) => {}
                }
                res
            }
        }
    }

    /// Read all packets from the queue
    pub fn recv_vectored(&self) -> Vec<T> {
        let mut bufs = Vec::with_capacity(self.int.borrow().cap);

        // get packets while the queue is not empty
        loop {
            let buf = self.recv();
            match buf {
                Some(pkt) => bufs.push(pkt),
                None => break,
            }
        }
        bufs
    }

    fn run_loop(&self, sender: Sender<T>, recvr: Receiver<T>) {
        self.int.borrow().stream.set_nonblocking(true).unwrap();
        println!("run_loop");
        loop {
            self.recv_ctrl_msg();
            let mut in_pkts = self.recv_vectored();
            if in_pkts.len() > 0 {
                println!("in_pkts len: {}", in_pkts.len());
            }
            if in_pkts.len() > 0 {
                // REVIEW: How to send packets out of the function?
                in_pkts
                    .drain(..)
                    .for_each(|pkt| match sender.try_send(pkt) {
                        Ok(_) => {}
                        Err(_) => {}
                    });
            }
            if let Some(pkt) = self.recv() {
                let p = pkt.clone();
                match sender.try_send(p) {
                    Ok(_) => {}
                    Err(_) => {}
                }
            }

            // REVIEW: How to send packets to the queue
            // Should we use crossbeam channels?
            // If we only deal with packet headers, this might be ok
            match recvr.try_recv() {
                Ok(pkt) => match self.xmit(pkt) {
                    Ok(_) => {}
                    Err(_e) => {}
                },
                Err(_) => {}
            }
        }
    }

    pub fn run(
        name: String,
        cap: usize,
        typ: u8,
        stream: UnixStream,
        recvr: Receiver<T>, // recv data from the process to send out of the interface
        sender: Sender<T>,  // send data to the process received from the interface
    ) {
        println!("run");
        let interface = Self::new(name, cap, stream, typ);
        println!("interface created");
        interface.run_loop(sender, recvr);
    }
}
