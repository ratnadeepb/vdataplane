use crossbeam::queue::ArrayQueue;
use l3enginelib::{eal_cleanup, eal_init, Mbuf, Mempool, Port};
use log;
use std::{
    fmt::format,
    io::Write,
    iter::Enumerate,
    net::TcpStream,
    slice,
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

fn xmit_pkts(port: &Port, out_pkts: &mut Vec<Mbuf>) -> usize {
    let queue_id = unsafe { dpdk_sys::_rte_lcore_id() as u16 };
    let num = port.send(out_pkts, queue_id ^ 1);
    out_pkts.clear(); // deallocate all buffers
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
    // let in_pkts: Arc<ArrayQueue<Mbuf>> = Arc::new(ArrayQueue::new(QUEUE_CAPA));

    // handling Ctrl+C
    let keep_running = Arc::new(AtomicBool::new(true));
    // let kr = keep_running.clone();
    handle_signal(keep_running.clone());

    while keep_running.load(Ordering::SeqCst) {
        // if !in_pkts.is_full() {
        // get new packets if the incoming queue is not full
        let mut bufs = recv_pkts(&port, QUEUE_SZ);
        let mut drop_arp_pkts_index = Vec::with_capacity(QUEUE_SZ);
        let mut i: usize = 0;

        let mut out_pkts: Vec<Mbuf> = Vec::with_capacity(QUEUE_SZ);

        for buf in &bufs {
            #[cfg(feature = "debug")]
            println!("mbuf: {:#?}", &buf);
            let arp_ptr =
                unsafe { dpdk_sys::_pkt_arp_response(buf.get_ptr(), (&mempool).get_ptr()) };
            if !arp_ptr.is_null() {
                let arp_buf = unsafe { Mbuf::from_ptr(arp_ptr) };
                let pkt = format!("{:#?}", buf);
                if let Ok(sz) = stream.write(pkt.as_bytes()) {
                    println!("sent {} bytes to proxy", sz);
                }
                &out_pkts.push(arp_buf);
                drop_arp_pkts_index.push(i); // add index of arp packet
            }
            i += 1;
        }

        // // drop the arp packets before processing
        // while !drop_arp_pkts_index.is_empty() {
        //     match drop_arp_pkts_index.pop() {
        //         Some(k) => drop(bufs.remove(k)),
        //         None => break,
        //     }
        // }

        if bufs.len() > 0 {
            for buf in bufs.drain(..) {
                let offset = buf.raw().data_off;
                let len = buf.raw().data_len;
                match buf.read_data::<u8>(offset.into()) {
                    Ok(ptr) => {
                        let data = unsafe { slice::from_raw_parts(ptr.as_ptr(), len.into()) };
                        if let Ok(sz) = stream.write(data) {
                            println!("sent {} bytes to proxy", sz);
                        }
                    }
                    Err(_) => {}
                }
            }
        }

        xmit_pkts(&port, &mut out_pkts);
    }
    // }

    #[cfg(feature = "debug")]
    println!("main: stopping");
    unsafe { dpdk_sys::_pkt_stop_and_close_ports() };
    #[cfg(feature = "debug")]
    println!("main: ports closed");
    eal_cleanup(&mempool).unwrap();
}
