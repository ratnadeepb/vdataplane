use crossbeam::queue::ArrayQueue;
use l3enginelib::{eal_cleanup, eal_init, process, Mbuf, Mempool, Port};
use log;
use std::{
    io::Write,
    net::TcpStream,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

const G_MEMPOOL_NAME: &str = "GLOBAL_MEMPOOL";
const QUEUE_SZ: usize = 32;
const QUEUE_CAPA: usize = 512;

/// Handle Ctrl+C
fn handle_signal(kr: Arc<AtomicBool>) {
    ctrlc::set_handler(move || {
        kr.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");
}

fn recv_pkts(port: &Port, len: usize) -> Vec<Mbuf> {
    let queue_id = unsafe { dpdk_sys::_rte_lcore_id() as u16 };
    let bufs = port.receive(queue_id, len);
    #[cfg(features = "debug")]
    if bufs.len() > 0 {
        println!("bufs len - recv_pkts: {}", bufs.len());
    }
    bufs
}

// fn xmit_pkts(port: &Port, out_pkts: &mut Vec<Mbuf>) -> usize {
fn xmit_pkts(port: &Port, out_pkts: Arc<ArrayQueue<Mbuf>>) -> usize {
    let queue_id = unsafe { dpdk_sys::_rte_lcore_id() as u16 };
    let mut bufs = Vec::with_capacity(out_pkts.len());
    while !out_pkts.is_empty() {
        match out_pkts.pop() {
            Some(pkt) => bufs.push(pkt),
            None => break,
        }
    }
    let num = port.send(&bufs, queue_id ^ 1);
    // out_pkts.clear(); // deallocate all buffers
    bufs.clear();
    num
}

fn main() {
    log::info!("Initializing DPDK env ...");
    let args = vec![
        String::from("-l 0-1"),
        String::from("-n 4"),
        String::from("--proc-type=primary"),
        String::from("--base-virtaddr=0x7f000000000"),
        String::from("--"),
        String::from("-p 3"),
        String::from("-n 2"),
    ];
    #[cfg(debug)]
    println!("main process args: {:?}", &args);
    eal_init(args).unwrap();

    #[cfg(feature = "debug")]
    println!("environment initialised");

    let cores = vec![0, 1];

    log::info!("setup mempool");
    let mempool;
    match Mempool::new(G_MEMPOOL_NAME) {
        Ok(mp) => {
            #[cfg(feature = "debug")]
            println!("mempool address: {:p}", mp.get_ptr());
            mempool = mp;
        }
        Err(e) => panic!("Failed to initialize mempool: {}", e),
    }

    #[cfg(feature = "debug")]
    println!("mempool set");

    log::info!("setup ports");
    #[cfg(feature = "debug")]
    println!("setup ports");
    let eth_devs = "port0";
    let mut port = Port::new(eth_devs, 0u16).unwrap();
    port.configure(cores.len() as u16, &mempool).unwrap();
    port.start().unwrap();

    #[cfg(feature = "debug")]
    println!("ports set");

    let mut stream = TcpStream::connect("127.0.0.1:9999").unwrap(); // a fatal failure

    #[cfg(feature = "debug")]
    println!("stream connected");

    // hold packets received from outside and packetiser
    let in_pkts: Arc<ArrayQueue<Mbuf>> = Arc::new(ArrayQueue::new(QUEUE_CAPA));
    let out_pkts: Arc<ArrayQueue<Mbuf>> = Arc::new(ArrayQueue::new(QUEUE_CAPA));

    let in_pkt_clone = in_pkts.clone();
    let out_pkt_clone = out_pkts.clone();

    // handling Ctrl+C
    let keep_running = Arc::new(AtomicBool::new(true));
    // let kr = keep_running.clone();
    handle_signal(keep_running.clone());

    std::thread::spawn(move || {
        process(in_pkt_clone, out_pkt_clone);
    });

    while keep_running.load(Ordering::SeqCst) {
        let mut bufs = recv_pkts(&port, QUEUE_SZ);
        let mut drop_arp_pkts_index = Vec::with_capacity(QUEUE_SZ);
        let mut i: usize = 0;

        for buf in &bufs {
            #[cfg(feature = "debug")]
            {
                let core_id = unsafe { dpdk_sys::_rte_lcore_id() as u16 };
                println!("Main core index: {}", unsafe {
                    dpdk_sys::rte_lcore_index(core_id as i32)
                });
                println!("Main Socket ID: {}", unsafe { dpdk_sys::rte_socket_id() });
                println!("Next lcore ID: {}", unsafe {
                    dpdk_sys::rte_get_next_lcore(core_id as u32, 1, 0)
                });

                println!("mbuf: {:#?}", &buf);
                // match process::serialize_conn(&buf) {
                //     Some(v) => {
                //         #[cfg(feature = "debug")]
                //         println!("got conn serliased, size: {}", v.len());
                //         match process::deserialize_conn(v) {
                //             Some(conn) => println!("got conn deserliased: {:#?}", conn),
                //             None => println!("didn't get conn serliased"),
                //         }
                //     }
                //     None => {
                //         #[cfg(feature = "debug")]
                //         println!("serialise err")
                //     }
                // }
            }

            // REVIEW: right now an arp reply is sent for every packet
            // this needs to be changed to receiving an IP from external sources
            // and responding only for that IP
            let arp_ptr =
                unsafe { dpdk_sys::_pkt_arp_response(buf.get_ptr(), (&mempool).get_ptr()) };
            if !arp_ptr.is_null() {
                let arp_buf = unsafe { Mbuf::from_ptr(arp_ptr) };
                // REVIEW: this is just sending the debug def of the packet as bytes over a TCP stream
                // ultimately, we want to send serialised packets over this connection
                let pkt = format!("{:#?}", buf);
                if let Ok(sz) = stream.write(pkt.as_bytes()) {
                    println!("sent {} bytes to proxy", sz);
                }
                match out_pkts.push(arp_buf) {
                    Ok(()) => {}
                    Err(arp) => drop(arp),
                }
                drop_arp_pkts_index.push(i); // add index of arp packet
            }
            i += 1;
        }

        // drop the arp packets before processing
        while !drop_arp_pkts_index.is_empty() {
            match drop_arp_pkts_index.pop() {
                Some(k) => drop(bufs.remove(k)),
                None => break,
            }
        }

        // process whatever has not been dropped
        for buf in bufs {
            match in_pkts.push(buf) {
                Ok(()) => {}
                Err(pkt) => drop(pkt),
            }
        }

        // if bufs.len() > 0 {
        //     for buf in bufs.drain(..) {
        //         let offset = buf.raw().data_off;
        //         let len = buf.raw().data_len;
        //         match buf.read_data::<u8>(offset.into()) {
        //             Ok(ptr) => {
        //                 let data = unsafe { slice::from_raw_parts(ptr.as_ptr(), len.into()) };
        //                 if let Ok(sz) = stream.write(data) {
        //                     println!("sent {} bytes to proxy", sz);
        //                 }
        //             }
        //             Err(_) => {}
        //         }
        //     }
        // }

        xmit_pkts(&port, out_pkts.clone());
    }

    #[cfg(feature = "debug")]
    println!("main: stopping");
    unsafe { dpdk_sys::_pkt_stop_and_close_ports() };
    #[cfg(feature = "debug")]
    println!("main: ports closed");
    eal_cleanup(&mempool).unwrap();
}
