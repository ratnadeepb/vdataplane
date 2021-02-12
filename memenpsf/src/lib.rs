//! A user-space memory interface based on shared memory IPC
//!
//! Control messages - specifically positions of read/write pointers
//! and location recv/xmit queues - are communicated over Unix sockets

use std::{
    io::{Result, Write},
    os::unix::{io::AsRawFd, net::UnixStream},
    ptr,
};

use ipc_queue::RingBuf;
use libc::{ftruncate, mmap, MAP_SHARED, PROT_READ, PROT_WRITE};

// const MTU: usize = 1536;

pub struct MemEnpsf<'a, T> {
    name: &'a str,
    s2c_q: RingBuf<T>,
    c2s_q: RingBuf<T>,
    cap: usize,
    stream: UnixStream,
}

impl<'a, T> MemEnpsf<'a, T> {
    /// Create a new interface
    pub fn new(name: &'a str, cap: usize, mut stream: UnixStream) -> Self {
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

    pub fn cap(&self) -> usize {
        self.cap
    }

    pub fn name(&self) -> &str {
        self.name
    }

    pub fn recv_from_client(&mut self) -> Option<T> {
        let res = self.c2s_q.pop();
        let buf = self.c2s_q.pointers();
        match self.stream.write(&buf) {
            Ok(sz) => {
                println!("sent {} elems", sz);
            }
            Err(e) => println!("failed to send: {}", e),
        };
        res
    }

    pub fn xmit_to_client(&mut self, buf: T) -> Result<()> {
        let res = self.s2c_q.push(buf);
        let buf = self.c2s_q.pointers();
        match self.stream.write(&buf) {
            Ok(sz) => {
                println!("sent {} elems", sz);
            }
            Err(e) => println!("failed to send: {}", e),
        };
        res
    }

    pub fn recv_from_srv(&mut self) -> Option<T> {
        let res = self.s2c_q.pop();
        let buf = self.c2s_q.pointers();
        match self.stream.write(&buf) {
            Ok(sz) => {
                println!("sent {} elems", sz);
            }
            Err(e) => println!("failed to send: {}", e),
        };
        res
    }

    pub fn xmit_to_srv(&mut self, buf: T) -> Result<()> {
        let res = self.c2s_q.push(buf);
        let buf = self.c2s_q.pointers();
        match self.stream.write(&buf) {
            Ok(sz) => {
                println!("sent {} elems", sz);
            }
            Err(e) => println!("failed to send: {}", e),
        };
        res
    }
}
