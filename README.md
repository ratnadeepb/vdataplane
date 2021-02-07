# DPDK based central L3 Engine and Dockerized L7 Stack

Design heavily influenced by Capsule (Rust crate) and openNetVM.

## Description

1. `dpdk-sys`: the sys lib containing the DPDK functions
2. `l3enginelib`: both a lib containing the APIs and a bin running the (raw) pkt recv/xmit loop
3. `l3enginemux`: runs the routing function sending packets to clients
4. `ipc-queue`: implements a wait-free lock-free pointer based queue in contiguous virtual memory
5. `memenpsf`: two ipc-queues built on shared memory to transfer packets and using an Unix socket for control messages
6. `client-container`: a representation of the client container

[External repo (enhanced locally)](https://github.com/stemjail/fdpass-rs) -> `fdpass-rs`: used to transmit file descriptors across a Unix socket

## Future Work

- Implement a L3/L4 network stack for `l3enginemux` (and security functions)
- Implement a L7 filter stack for `client-container` (and security functions)
- Test everything exhaustively

## Known Issue

- The build script is more generic now but FFI interface remains a little unstable, in terms of changing between DPDK versions. It still might create different interface for different DPDK versions. But most of the major stuff is steady now.

## Possible Issues

- `Ring`, `Mbuf` and `Mempool` define a `get_ptr` function which works as such:
  ```Rust
  pub fn get_ptr(&self) -> *mut dpdk_ffi::rte_ring {
  	self.raw.as_ptr()
  }
  ```
  where `raw` is a `ptr::NonNull` type.</br>
  This allows sending and receiving packets, manipulating packets, receiving and freeing `mbuf` memory without taking a mutable pointer to any of these structures. The assumptions (possibly invalid) are that:
  1. Some underlying structures like `rte_mempool` and `rte_ring` are thread safe
  2. Others like `rte_mbuf` will typically not be accessed from multiple threads
- `Ring` and `Mbuf` have been marked as `Send` while `Mempool` has been marked as both `Sync` and `Send`. This has been done to enable passing/sharing related pointers between threads. There probably is no adverse side effect of this but **this has not been tested.**
- Supported for `DPDK 19.11`. Certain libraries have changed in `DPDK 20` (as listed in `build.rs`)