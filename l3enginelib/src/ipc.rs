use chashmap::CHashMap;
use crossbeam::{
    channel::{bounded, Receiver, Sender},
    select,
};
use memenpsf::Interface;
use rand::random;
use rayon::ThreadPoolBuilder;
use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    fs,
    hash::{Hash, Hasher},
    io::Read,
    os::unix::net::{UnixListener, UnixStream},
    sync::Arc,
};

// Functions to be used by both servers and clients

pub fn new_int(name: String, cap: usize, stream: UnixStream, typ: u8) -> Interface<[u8; 24]> {
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
            }
        }

        let bufs = int.recv_vectored();
        if bufs.len() > 0 {
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
    stream: UnixStream,
    typ: u8,
    recvr: Receiver<[u8; 24]>,
    sender: Sender<[u8; 24]>,
) {
    let int = new_int(name, cap, stream, typ);
    run_loop(int, recvr, sender);
}

// Functions to be used by the server only
const CAP: usize = 32;

pub(crate) struct NFMap {
    map: CHashMap<u64, (Receiver<[u8; 24]>, Sender<[u8; 24]>)>,
}

impl NFMap {
    pub(crate) fn new() -> Self {
        let map = CHashMap::new();
        Self { map }
    }

    pub(crate) fn insert(&self, key: u64, val: (Receiver<[u8; 24]>, Sender<[u8; 24]>)) {
        self.map.insert(key, val);
    }
}



pub(crate) fn srv_run() {
    let sock_name = "/tmp/fd-passrd.socket";
    fs::remove_file(sock_name).ok();
    let listener = UnixListener::bind(sock_name).unwrap();

    let client_map = Arc::new(NFMap::new());

    let pool = ThreadPoolBuilder::new().num_threads(5).build().unwrap(); // fatal error

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let name = format!("eth{}", random::<u8>());
                let cap = CAP;
                let typ = 1; // server
                let mut buf = [0; 30];

                stream.read(&mut buf).unwrap();
                let client_name = String::from_utf8(Vec::from(buf)).unwrap();
                println!("client name: {}", &client_name);

                let mut hasher = DefaultHasher::new();
                client_name.hash(&mut hasher);
                let h_name = hasher.finish();

                let (s1, recvr) = bounded::<[u8; 24]>(CAP);
                let (sender, r1) = bounded::<[u8; 24]>(CAP);

                client_map.insert(h_name, (r1, s1));

                pool.install(|| run(name, cap, stream, typ, recvr, sender));
            }
            Err(_) => {}
        }
    }
}
