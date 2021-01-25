//! ICMP macros

// DEVFLAGS: remove in production
#![allow(unused_macros)]

use byteorder::NetworkEndian;
use smoltcp::wire::Icmpv4Repr::{EchoReply, EchoRequest};

#[macro_export]
macro_rules! send_icmp_ping {
	( $repr_type:ident, $packet_type:ident, $ident:expr, $seq_no:expr,
      $echo_payload:expr, $socket:expr, $remote_addr:expr ) => {{
		let icmp_repr = $repr_type::EchoRequest {
			ident: $ident,
			seq_no: $seq_no,
			data: &$echo_payload,
			};

		let icmp_payload = $socket.send(icmp_repr.buffer_len(), $remote_addr).unwrap();

		let icmp_packet = $packet_type::new_unchecked(icmp_payload);
		(icmp_repr, icmp_packet)
		}};
}

#[macro_export]
macro_rules! get_icmp_pong {
	( $repr_type:ident, $repr:expr, $payload:expr, $waiting_queue:expr, $remote_addr:expr,
      $timestamp:expr, $received:expr ) => {{
		if let $repr_type::EchoReply { seq_no, data, .. } = $repr {
			if let Some(_) = $waiting_queue.get(&seq_no) {
				let packet_timestamp_ms = NetworkEndian::read_i64(data);
				println!(
					"{} bytes from {}: icmp_seq={}, time={}ms",
					data.len(),
					$remote_addr,
					seq_no,
					$timestamp.total_millis() - packet_timestamp_ms
				);
				$waiting_queue.remove(&seq_no);
				$received += 1;
				}
			}
		}};
}
