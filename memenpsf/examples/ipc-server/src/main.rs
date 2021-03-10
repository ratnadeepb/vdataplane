use std::{collections::HashMap, fs, io::Read, os::unix::net::UnixListener, sync::Arc, thread};

use crossbeam::{channel::bounded, sync::ShardedLock};
use memenpsf::Interface;
use rand::random;

const CAP: usize = 32;

fn main() {
    let sock_name = "/tmp/fd-passrd.socket";
    fs::remove_file(sock_name).ok();
    let listener = UnixListener::bind(sock_name).unwrap();

    // Key := Name of the NF
    // Value[0] := sender to send headers to the interface
    // Value[1] := receiver to receive headers from the headers
    let client_map = Arc::new(ShardedLock::new(HashMap::new()));
    let mut workers = Vec::new();

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let name = format!("eth{}", random::<u8>());
                let cap = CAP;
                let typ = 1; // server
                let mut buf = [0; 30];
                    
                let client_name = String::from_utf8(Vec::from(buf)).unwrap();
                println!("client name: {}", &client_name);

                let (s1, recvr) = bounded::<[u8; 2]>(CAP);
                let (sender, r1) = bounded::<[u8; 2]>(CAP);

                match client_map.write() {
                    Ok(mut map) => {
                        (*map).insert(client_name.clone(), (s1, r1));
                    }
                    Err(p) => {
                        let mut map = p.into_inner();
                        (*map).insert(client_name.clone(), (s1, r1));
                    }
                }

                let worker = thread::spawn(move || {
                    Interface::<[u8; 2]>::run(name, cap, typ, stream, recvr, sender)
                });
                workers.push(worker);

                // REVIEW: This code is placed within the match stream loop to allow us to send/receive data to/from the interface without more complicated mechanisms to access the sender and receiver channel.
                // This restricts the server to a single client only since it gets stuck within this infinite loop

                // /The infinite loop here ensures that the client reads after the server writes
                // This is essentially multiple writes to ensure that the server gets the data
                // This needs to be rectified
                loop {
                    match client_map.read() {
                        Ok(client_map) => match client_map.get(&client_name) {
                            Some((s1, r1)) => {
                                match r1.try_recv() {
                                    Ok(buf) => println!("got data: {:#?}", buf),
                                    Err(_e) => {}
                                };
                                // let msg = "server1".as_bytes();
                                match s1.try_send([5, 6]) {
                                    Ok(_) => {}
                                    Err(_) => {}
                                }
                            }
                            None => {}
                        },
                        Err(p) => {
                            let client_map = p.into_inner();
                            let (_, r1) = client_map.get(&client_name).unwrap();
                            match r1.try_recv() {
                                Ok(buf) => println!("got data: {:#?}", buf),
                                Err(_e) => {}
                            }
                        }
                    }
                }
            }
            Err(e) => eprintln!("failed to connect: {}", e),
        }
    }
    for worker in workers {
        match worker.join() {
            Ok(_) => {}
            Err(e) => eprintln!("error joining thread: {:#?}", e),
        }
    }
    println!("Hello, world!");
}
