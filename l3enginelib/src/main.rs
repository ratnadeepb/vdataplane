use l3enginelib::{eal_cleanup, eal_init, Channel, Mbuf, Mempool, Port};
use log;
use std::{sync::{
	atomic::{AtomicBool, Ordering},
	Arc,
}};
use zmq::Context;

const G_MEMPOOL_NAME: &str = "GLOBAL_MEMPOOL";
const QUEUE_SZ: usize = 32;
const PACKETISER_ZMQ_PORT: &str = "tcp://*:5555";

/// Handle Ctrl+C
fn handle_signal(kr: Arc<AtomicBool>) {
	ctrlc::set_handler(move || {
		kr.store(false, Ordering::SeqCst);
	})
	.expect("Error setting Ctrl-C handler");
}

fn recv_pkts(port: &Port, in_pkts: &mut Vec<Mbuf>, ch: &Channel) -> usize {
	let len = in_pkts.capacity() - in_pkts.len();
	if len == 0 {
		return 0usize;
	}

	let queue_id = unsafe { dpdk_sys::_rte_lcore_id() as u16 };
	// let bufs = port.receive(queue_id, len);
	// for pkt in bufs {
	// 	in_pkts.push(pkt);
	// }
	// NOTE: Would this work?
	in_pkts.extend(port.receive(queue_id, len));

	let len = ch.send_to_packetiser_bulk(in_pkts);

	len
}

fn xmit_pkts(port: &Port, out_pkts: &mut Vec<Mbuf>, ch: &Channel) -> usize {
	let len = out_pkts.capacity() - out_pkts.len();
	if len > 0 {
		let mut pkts = Vec::with_capacity(len);
		ch.recv_from_packetiser_bulk(&mut pkts, len);
		out_pkts.extend(pkts);
	}

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

	let cores = vec![0];

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

	#[cfg(feature = "debug")]
	println!("main: waiting for secondary");
	let context = Context::new();
	let responder = context.socket(zmq::REP).unwrap(); // fatal error
	assert!(responder.bind(PACKETISER_ZMQ_PORT).is_ok());
	let mut msg = zmq::Message::new();
	responder.recv(&mut msg, 0).unwrap();

	// start the channel
	let channel = Channel::new().unwrap(); // we can't work otherwise!

	// hold packets received from outside and packetiser
	let mut in_pkts: Vec<Mbuf> = Vec::with_capacity(QUEUE_SZ);
	let mut out_pkts: Vec<Mbuf> = Vec::with_capacity(QUEUE_SZ);

	// handling Ctrl+C
	let keep_running = Arc::new(AtomicBool::new(true));
	// let kr = keep_running.clone();
	handle_signal(keep_running.clone());

	#[cfg(feature = "debug")]
	println!("main: secondary started");
	while keep_running.load(Ordering::SeqCst) {
		let _rsz = recv_pkts(&port, &mut in_pkts, &channel);
		#[cfg(feature = "debug")]
		println!("Received {} packets", _rsz);
		let _tsz = xmit_pkts(&port, &mut out_pkts, &channel);
		#[cfg(feature = "debug")]
		println!("Received {} packets", _rsz);
	}

	#[cfg(feature = "debug")]
	println!("main: stopping");
	unsafe { dpdk_sys::_pkt_stop_and_close_ports() };
	#[cfg(feature = "debug")]
	println!("main: ports closed");
	eal_cleanup(&mempool).unwrap();
}
