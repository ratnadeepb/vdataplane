# DPDK based central L3 Engine and Dockerized L7 Stack

Design heavily influenced by Capsule module and openNetVM.

## Known Issue

- In some cases, `#include <immintrin.h>` had to be added to `/usr/local/include/rte_memcpy.h` (cloudlab)
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