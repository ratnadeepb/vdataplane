/// known issues:
// 1. https://github.com/rust-lang/rust/issues/54341

// all the necessary DPDK functions, types and constants are defined
// in the following header files.
#include <rte_config.h>
#include <rte_eal.h>
#include <rte_errno.h>
#include <rte_ethdev.h>
#include <rte_kni.h>
#include <rte_malloc.h>
#include <rte_ring.h>

// libnuma functions and types
#include <numa.h>

// pcap functions and types
#include <pcap.h>

// bindgen can't generate bindings for static functions defined in C
// header files. these shims are necessary to expose them to FFI.

/**
 * Error number value, stored per-thread, which can be queried after
 * calls to certain functions to determine why those functions failed.
 */
int _rte_errno(void);

/**
 * Allocate a new mbuf from a mempool.
 */
struct rte_mbuf *_rte_pktmbuf_alloc(struct rte_mempool *mp);

/**
 * Free a packet mbuf back into its original mempool.
 */
void _rte_pktmbuf_free(struct rte_mbuf *m);

/**
 * Allocate a bulk of mbufs, initialize refcnt and reset the fields to
 * default values.
 */
int _rte_pktmbuf_alloc_bulk(struct rte_mempool *pool, struct rte_mbuf **mbufs,
                            unsigned count);

/**
 * Put several objects back in the mempool.
 */
void _rte_mempool_put_bulk(struct rte_mempool *mp, void *const *obj_table,
                           unsigned int n);

/**
 * Retrieve a burst of input packets from a receive queue of an Ethernet
 * device. The retrieved packets are stored in *rte_mbuf* structures whose
 * pointers are supplied in the *rx_pkts* array.
 */
uint16_t _rte_eth_rx_burst(uint16_t port_id, uint16_t queue_id,
                           struct rte_mbuf **rx_pkts, const uint16_t nb_pkts);

/**
 * Send a burst of output packets on a transmit queue of an Ethernet device.
 */
uint16_t _rte_eth_tx_burst(uint16_t port_id, uint16_t queue_id,
                           struct rte_mbuf **tx_pkts, uint16_t nb_pkts);

/* Added by Deep */
/* Return the number of entries in a ring. */
unsigned int _rte_ring_count(const struct rte_ring *r);

/* Dequeue several objects from a ring. */
unsigned int _rte_ring_dequeue_bulk(struct rte_ring *r, void **obj_table,
                                    unsigned int n, unsigned int *available);

/* Put one object back in the mempool. */
void _rte_mempool_put(struct rte_mempool *mp, void *obj);

/* Get one object from the mempool. */
int _rte_mempool_get(struct rte_mempool *mp, void **obj);

/* Enqueue one object on a ring. */
int _rte_ring_enqueue(struct rte_ring *r, void *obj);

/* Return the number of TSC cycles since boot */
uint64_t _rte_get_tsc_cycles(void);

/* Return the Application thread ID of the execution unit. */
unsigned _rte_lcore_id(void);

/* Get the number of cycles in one second for the default timer. */
uint64_t _rte_get_timer_hz(void);

/* Atomically decrement a counter by one */
void _rte_atomic16_dec(rte_atomic16_t *v);

/* Dequeue multiple objects from a ring up to a maximum number. */
unsigned int _rte_ring_dequeue_burst(struct rte_ring *r, void **obj_table,
                                     unsigned int n, unsigned int *available);

/* Dequeue one object from a ring. */
int _rte_ring_dequeue(struct rte_ring *r, void **obj_p);

/* Enqueue several objects on a ring. */
unsigned int _rte_ring_enqueue_bulk(struct rte_ring *r, void *const *obj_table,
                                    unsigned int n, unsigned int *free_space);

/* Parse IP to u32 */
int _pkt_parse_ip(char *ip_str, uint32_t *dest);

/* Get the Ethernet header from the packet */
struct rte_ether_hdr *_pkt_ether_hdr(struct rte_mbuf *pkt);

/* Get the IPv4 header from the packet */
struct rte_ipv4_hdr *_pkt_ipv4_hdr(struct rte_mbuf *pkt);

/* Get the TCP header from the packet */
struct rte_tcp_hdr *_pkt_tcp_hdr(struct rte_mbuf *pkt);

/* Get the UDP header from the packet */
struct rte_udp_hdr *_pkt_udp_hdr(struct rte_mbuf *pkt);

/* Get the ICMP header from the packet */
struct rte_icmp_hdr *_pkt_icmp_hdr(struct rte_mbuf *pkt);

/* Get ICMP checksum */
uint16_t _pkt_icmp_checksum(uint16_t checksum);

/* Get the ARP header from the packet */
struct rte_arp_hdr *_pkt_arp_hdr(struct rte_mbuf *pkt);

/* Copy Ether Address from source dest */
void _rte_ether_addr_copy(const struct rte_ether_addr *__restrict ea_from, struct rte_ether_addr *__restrict ea_to);

/* flush a local memring cache */
void _rte_mempool_cache_flush(struct rte_mempool_cache *cache,
                              struct rte_mempool *mp);

/* Convert a 16-bit value from CPU order to big endian. */
rte_be16_t _rte_cpu_to_be_16(uint16_t x);

/* Convert a 32-bit value from big endian order to CPU. */
uint32_t _rte_be_to_cpu_32(rte_be32_t x);

/* Detect ARP Request header */
int _pkt_detect_arp(struct rte_mbuf *buf, uint32_t local_ip);

/* Create an ARP Response packet */
struct rte_mbuf *_pkt_arp_response(struct rte_mbuf *pkt, struct rte_mempool *mp);

void stop_and_close_ports();