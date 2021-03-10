use chashmap::CHashMap;
use crossbeam::channel::{bounded, Receiver, Sender};
use memenpsf::{Interface, Stream};
use rand::random;

use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    sync::{Arc, RwLock},
};

use async_std::os::unix::net::UnixListener;
use async_std::prelude::*;

// Functions to be used by both servers and clients

pub fn new_int(name: String, cap: usize, stream: Box<dyn Stream>, typ: u8) -> Interface<[u8; 24]> {
    Interface::<[u8; 24]>::new(name, cap, stream, typ)
}

fn run_loop(int: Interface<[u8; 24]>, recvr: Receiver<[u8; 24]>, sender: Sender<[u8; 24]>) {
    loop {
        // check if we have received anything to send out
        match recvr.try_recv() {
            Ok(msg) => match int.xmit(msg) {
                Ok(_) => {
                    #[cfg(feature = "debug")]
                    println!("sent msg");
                }
                Err(_e) => {
                    #[cfg(feature = "debug")]
                    println!("failed to send msg: {:#?}", _e);
                }
            },
            Err(_e) => {
                #[cfg(feature = "debug")]
                println!("empty recv channel; {:#?}", _e);
                break;
            }
        }

        let bufs = int.recv_vectored();
        if bufs.len() > 0 {
            #[cfg(feature = "debug")]
            println!("received from client NF");
            for buf in bufs {
                match sender.try_send(buf) {
                    Ok(_) => {}
                    Err(_e) => {
                        #[cfg(feature = "debug")]
                        println!("channel full");
                        drop(buf);
                    }
                }
            }
        }
    }
}

pub fn run(
    name: String,
    cap: usize,
    stream: Box<dyn Stream>,
    typ: u8,
    recvr: Receiver<[u8; 24]>,
    sender: Sender<[u8; 24]>,
) {
    let int = new_int(name, cap, stream, typ);
    run_loop(int, recvr, sender);
}

// Functions to be used by the server only
const CAP: usize = 32; // NOTE: should match the CAP on the client NFs
                       // const NUM_LISTENER_THRDS: usize = 5;

pub(crate) struct NFMap {
    map: CHashMap<u64, (Receiver<[u8; 24]>, Sender<[u8; 24]>)>,
}

impl NFMap {
    pub(crate) fn new() -> Self {
        let map = CHashMap::new();
        Self { map }
    }

    pub(crate) fn insert(&self, key: u64, val: (Receiver<[u8; 24]>, Sender<[u8; 24]>)) {
        let _ = self.map.insert(key, val);
    }
}

fn send_to_all_nfs(
    pkt: [u8; 24],
    names: &Vec<String>,
    map: &CHashMap<u64, (Receiver<[u8; 24]>, Sender<[u8; 24]>)>,
) {
    for n in names {
        let mut hasher = DefaultHasher::new();
        n.hash(&mut hasher);
        let h = hasher.finish();
        match map.get(&h) {
            Some(v) => {
                let (_r, s) = &*v;
                match s.try_send(pkt) {
                    Ok(_) => {}
                    Err(_e) => {}
                }
            }
            None => {}
        }
    }
}

/// client name should be exactly this size
const CLIENT_NAME_SZ: usize = 30;

pub(crate) async fn srv_run(m_recvr: Receiver<[u8; 24]>) {
    let sock_name = "/tmp/fd-passrd.socket";
    fs::remove_file(sock_name).ok();
    let listener = UnixListener::bind(sock_name).await.unwrap();

    // if using the std (sync) listener then it should be set to nonblocking
    // listener
    //     .set_nonblocking(true)
    //     .expect("Couldn't set non blocking");

    let client_map = Arc::new(NFMap::new());
    let map = client_map.clone();

    let client_names = Arc::new(RwLock::new(Vec::<String>::new()));
    let names = client_names.clone();

    let listener_thrd = tokio::spawn(async move {
        let mut incoming = listener.incoming();

        while let Some(stream) = incoming.next().await {
            match stream {
                Ok(mut stream) => {
                    let name = format!("eth{}", random::<u8>());
                    let cap = CAP;
                    let typ = 1; // server
                    let mut buf = [0; CLIENT_NAME_SZ];

                    stream.s_read(&mut buf).unwrap();

                    let client_name = String::from_utf8(Vec::from(buf)).unwrap();
                    println!("client name: {}", &client_name);
                    match client_names.write() {
                        Ok(mut names) => {
                            (*names).push(client_name.clone());
                        }
                        Err(p) => {
                            let mut names = p.into_inner();
                            (*names).push(client_name.clone());
                        }
                    }

                    let mut hasher = DefaultHasher::new();
                    client_name.hash(&mut hasher);
                    let name_hash = hasher.finish();

                    let (s1, recvr) = bounded::<[u8; 24]>(CAP);
                    let (sender, r1) = bounded::<[u8; 24]>(CAP);

                    client_map.insert(name_hash, (r1, s1));

                    tokio::spawn(
                        async move { run(name, cap, Box::new(stream), typ, recvr, sender) },
                    );
                }
                Err(_) => {}
            }
        }
    });

    println!("out of the streaming loop");
    match m_recvr.try_recv() {
        Ok(pkt) => match names.read() {
            Ok(names) => {
                send_to_all_nfs(pkt, &*names, &map.map);
            }
            Err(p) => {
                let names = p.into_inner();
                send_to_all_nfs(pkt, &*names, &map.map);
            }
        },
        Err(_e) => {}
    }
    let _ = listener_thrd.await;
}
