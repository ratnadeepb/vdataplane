#include "bindings.h"
#include <rte_errno.h>
#include <rte_ethdev.h>
#include <rte_mbuf.h>
#include <rte_mempool.h>

int
_rte_errno(void)
{
        return rte_errno;
}

struct rte_mbuf *
_rte_pktmbuf_alloc(struct rte_mempool *mp)
{
        return rte_pktmbuf_alloc(mp);
}

void
_rte_pktmbuf_free(struct rte_mbuf *m)
{
        rte_pktmbuf_free(m);
}

int
_rte_pktmbuf_alloc_bulk(struct rte_mempool *pool, struct rte_mbuf **mbufs,
                        unsigned count)
{
        return rte_pktmbuf_alloc_bulk(pool, mbufs, count);
}

void
_rte_mempool_put_bulk(struct rte_mempool *mp, void *const *obj_table,
                      unsigned int n)
{
        rte_mempool_put_bulk(mp, obj_table, n);
}

uint16_t
_rte_eth_rx_burst(uint16_t port_id, uint16_t queue_id,
                  struct rte_mbuf **rx_pkts, const uint16_t nb_pkts)
{
        return rte_eth_rx_burst(port_id, queue_id, rx_pkts, nb_pkts);
}

uint16_t
_rte_eth_tx_burst(uint16_t port_id, uint16_t queue_id,
                  struct rte_mbuf **tx_pkts, uint16_t nb_pkts)
{
        return rte_eth_tx_burst(port_id, queue_id, tx_pkts, nb_pkts);
}

/* Added by Deep */
unsigned int
_rte_ring_count(const struct rte_ring *r)
{
        return rte_ring_count(r);
}

unsigned int
_rte_ring_dequeue_bulk(struct rte_ring *r, void **obj_table, unsigned int n,
                       unsigned int *available)
{
        return rte_ring_dequeue_bulk(r, obj_table, n, available);
}

void
_rte_mempool_put(struct rte_mempool *mp, void *obj)
{
        return rte_mempool_put(mp, obj);
}

int
_rte_mempool_get(struct rte_mempool *mp, void **obj)
{
        return rte_mempool_get(mp, obj);
}

int
_rte_ring_enqueue(struct rte_ring *r, void *obj)
{
        return rte_ring_enqueue(r, obj);
}

uint64_t
_rte_get_tsc_cycles(void)
{
        return rte_get_tsc_cycles();
}

unsigned
_rte_lcore_id(void)
{
        return rte_lcore_id();
}

uint64_t
_rte_get_timer_hz(void)
{
        return rte_get_timer_hz();
}

void
_rte_atomic16_dec(rte_atomic16_t *v)
{
        return rte_atomic16_dec(v);
}

unsigned int
_rte_ring_dequeue_burst(struct rte_ring *r, void **obj_table, unsigned int n,
                        unsigned int *available)
{
        return rte_ring_dequeue_burst(r, obj_table, n, available);
}

int
_rte_ring_dequeue(struct rte_ring *r, void **obj_p)
{
        return rte_ring_dequeue(r, obj_p);
}

unsigned int
_rte_ring_enqueue_bulk(struct rte_ring *r, void *const *obj_table,
                       unsigned int n, unsigned int *free_space)
{
        return rte_ring_enqueue_bulk(r, obj_table, n, free_space);
}

void
_pkt_stop_and_close_ports()
{
        uint16_t port_id = 0;
        RTE_ETH_FOREACH_DEV(port_id)
        {
                rte_eth_dev_stop(port_id);
                rte_eth_dev_close(port_id);
        }
        exit(0);
}

struct rte_ether_hdr *
_pkt_ether_hdr(struct rte_mbuf *pkt)
{
        if (unlikely(pkt == NULL)) {
                return NULL;
        }
        return rte_pktmbuf_mtod(pkt, struct rte_ether_hdr *);
}

struct rte_ipv4_hdr *
_pkt_ipv4_hdr(struct rte_mbuf *pkt)
{
        struct rte_ipv4_hdr *ipv4 = rte_pktmbuf_mtod_offset(pkt, struct rte_ipv4_hdr *, sizeof(struct rte_ether_hdr));

        if (ipv4 == NULL) {
                return NULL;
        }

        printf("_pkt_ipv4_hdr: got non null ipv4 header\n"); // debug
        // struct rte_ipv4_hdr *ipv4 =
        //     (struct rte_ipv4_hdr *)(rte_pktmbuf_mtod(pkt, uint8_t *) +
        //                             sizeof(struct rte_ether_hdr));

        /* In an IP packet, the first 4 bits determine the version.
         * The next 4 bits are called the Internet Header Length, or IHL.
         * DPDK's ipv4_hdr struct combines both the version and the IHL into one
         * uint8_t.
         */
        uint8_t version = (ipv4->version_ihl >> 4) & 0b1111;
        if (unlikely(version != 4)) {
                printf("_pkt_ipv4_hdr: not ipv4\n"); // debug
                char *ip_src = rte_malloc(NULL, 15, 0); // debug
                uint32_t ip = rte_be_to_cpu_32(ipv4->src_addr); // debug
                _pkt_parse_ip(ip_src, &ip); // debug
                printf("_pkt_ipv4_hdr: src addr: %s\n", ip_src); // debug
                return NULL;
        }
        return ipv4;
}

#define IP_PROTOCOL_ICMP 1
#define IP_PROTOCOL_TCP 6
#define IP_PROTOCOL_UDP 17

struct rte_tcp_hdr *
_pkt_tcp_hdr(struct rte_mbuf *pkt)
{
        struct rte_ipv4_hdr *ipv4 = _pkt_ipv4_hdr(pkt);

        if (unlikely(ipv4 ==
                     NULL)) { // Since we aren't dealing with IPv6 packets for
                              // now, we can ignore anything that isn't IPv4
                return NULL;
        }

        if (ipv4->next_proto_id != IP_PROTOCOL_TCP) {
                return NULL;
        }

        uint8_t *pkt_data = rte_pktmbuf_mtod(pkt, uint8_t *) +
                            sizeof(struct rte_ether_hdr) +
                            sizeof(struct rte_ipv4_hdr);
        return (struct rte_tcp_hdr *)pkt_data;
}

struct rte_udp_hdr *
_pkt_udp_hdr(struct rte_mbuf *pkt)
{
        struct rte_ipv4_hdr *ipv4 = _pkt_ipv4_hdr(pkt);

        if (unlikely(ipv4 ==
                     NULL)) { // Since we aren't dealing with IPv6 packets for
                              // now, we can ignore anything that isn't IPv4
                return NULL;
        }

        if (ipv4->next_proto_id != IP_PROTOCOL_UDP) {
                return NULL;
        }

        uint8_t *pkt_data = rte_pktmbuf_mtod(pkt, uint8_t *) +
                            sizeof(struct rte_ether_hdr) +
                            sizeof(struct rte_ipv4_hdr);
        return (struct rte_udp_hdr *)pkt_data;
}

struct rte_icmp_hdr *
_pkt_icmp_hdr(struct rte_mbuf *pkt)
{
        struct rte_ipv4_hdr *ipv4 = _pkt_ipv4_hdr(pkt);

        if (unlikely(ipv4 ==
                     NULL)) { // Since we aren't dealing with IPv6 packets for
                              // now, we can ignore anything that isn't IPv4
                return NULL;
        }

        if (ipv4->next_proto_id != IP_PROTOCOL_ICMP) {
                printf("not icmp\n"); // debug
                return NULL;
        }
        printf("icmp pkt\n"); // debug

        uint8_t *pkt_data = rte_pktmbuf_mtod(pkt, uint8_t *) +
                            sizeof(struct rte_ether_hdr) +
                            sizeof(struct rte_ipv4_hdr);

        return (struct rte_icmp_hdr *)pkt_data;
}

uint16_t _pkt_icmp_checksum(uint16_t cksum)
{
        cksum = ~cksum & 0xffff;
        cksum += ~htons(RTE_IP_ICMP_ECHO_REQUEST << 8) & 0xffff;
        cksum += htons(RTE_IP_ICMP_ECHO_REPLY << 8);
        cksum = (cksum & 0xffff) + (cksum >> 16);
        cksum = (cksum & 0xffff) + (cksum >> 16);
        return ~cksum;
}

void _rte_ether_addr_copy(const struct rte_ether_addr *__restrict ea_from, struct rte_ether_addr *__restrict ea_to)
{
        rte_ether_addr_copy(ea_from, ea_to);
}

void
_rte_mempool_cache_flush(struct rte_mempool_cache *cache,
                         struct rte_mempool *mp)
{
        rte_mempool_cache_flush(cache, mp);
}

struct rte_arp_hdr *
_pkt_arp_hdr(struct rte_mbuf *pkt)
{
        return rte_pktmbuf_mtod_offset(pkt, struct rte_arp_hdr *, sizeof(struct rte_ether_hdr));
}

rte_be16_t
_rte_cpu_to_be_16(uint16_t x)
{
        return rte_cpu_to_be_16(x);
}

uint32_t
_rte_be_to_cpu_32(rte_be32_t x)
{
        return rte_be_to_cpu_32(x);
}

int
_pkt_parse_ip(char *ip_str, uint32_t *dest)
{
        int ret;
        int ip[4];

        if (ip_str == NULL || dest == NULL) {
                return -1;
        }

        ret = sscanf(ip_str, "%u.%u.%u.%u", &ip[0], &ip[1], &ip[2], &ip[3]);
        if (ret != 4) {
                return -1;
        }
        *dest = RTE_IPV4(ip[0], ip[1], ip[2], ip[3]);
        return 0;
}

void
_pkt_parse_char_ip(char* ip_dest, uint32_t ip_src) {
        snprintf(ip_dest, 16, "%u.%u.%u.%u", (ip_src >> 24) & 0xFF, (ip_src >> 16) & 0xFF,
                (ip_src >> 8) & 0xFF, ip_src & 0xFF);
}

int
_pkt_detect_arp(struct rte_mbuf *pkt, uint32_t local_ip)
{
        struct rte_ether_hdr *ether_hdr = _pkt_ether_hdr(pkt);
        struct rte_arp_hdr *arp_hdr;

        if (rte_cpu_to_be_16(ether_hdr->ether_type) == RTE_ETHER_TYPE_ARP) {
                arp_hdr = _pkt_arp_hdr(pkt);
                if (rte_cpu_to_be_16(arp_hdr->arp_opcode) ==
                    RTE_ARP_OP_REQUEST) {
                        if (rte_be_to_cpu_32(arp_hdr->arp_data.arp_tip) ==
                            local_ip) {
                                return 1;
                        }
                }
        }
        return 0;
}

struct rte_mbuf *
_pkt_arp_response(struct rte_mbuf *pkt, struct rte_mempool *mp)
{
        struct rte_ether_hdr *ether_hdr = _pkt_ether_hdr(pkt);
        struct rte_arp_hdr *arp_hdr;
        struct rte_ether_hdr *eth_hdr;

        if (rte_cpu_to_be_16(ether_hdr->ether_type) != RTE_ETHER_TYPE_ARP) {
                return NULL;
        }
        
        arp_hdr = _pkt_arp_hdr(pkt);

        if (rte_cpu_to_be_16(arp_hdr->arp_opcode) != RTE_ARP_OP_REQUEST) {
                return NULL;
        }

        struct rte_ether_addr *tha = &ether_hdr->d_addr;
        struct rte_ether_addr *frm = &ether_hdr->s_addr;
        
        arp_hdr = rte_pktmbuf_mtod_offset(pkt, struct rte_arp_hdr *, sizeof(struct rte_ether_hdr));
        uint32_t tip = arp_hdr->arp_data.arp_sip;
        uint32_t sip = arp_hdr->arp_data.arp_tip;

        struct rte_mbuf *out_pkt = NULL;
        struct rte_arp_hdr *out_arp_hdr = NULL;

        size_t pkt_size = 0;

        out_pkt = rte_pktmbuf_alloc(mp);
        if (out_pkt == NULL) {
                rte_free(out_pkt);
                return NULL;
        }

        pkt_size = sizeof(struct rte_ether_hdr) + sizeof(struct rte_arp_hdr);
        out_pkt->data_len = pkt_size;
        out_pkt->pkt_len = pkt_size;

        // SET ETHER HEADER INFO
        eth_hdr = _pkt_ether_hdr(out_pkt);
        rte_ether_addr_copy(frm, &eth_hdr->s_addr);
        eth_hdr->ether_type = rte_cpu_to_be_16(RTE_ETHER_TYPE_ARP);
        rte_ether_addr_copy(tha, &eth_hdr->d_addr);

        // SET ARP HDR INFO
        out_arp_hdr = rte_pktmbuf_mtod_offset(out_pkt, struct rte_arp_hdr *,
                                              sizeof(struct rte_ether_hdr));

        out_arp_hdr->arp_hardware = rte_cpu_to_be_16(RTE_ARP_HRD_ETHER);
        out_arp_hdr->arp_protocol = rte_cpu_to_be_16(RTE_ETHER_TYPE_IPV4);
        out_arp_hdr->arp_hlen = RTE_ETHER_ADDR_LEN;
        // out_arp_hdr->arp_hlen = 6;
        out_arp_hdr->arp_plen = sizeof(uint32_t);
        out_arp_hdr->arp_opcode = rte_cpu_to_be_16(RTE_ARP_OP_REPLY);

        rte_ether_addr_copy(frm, &out_arp_hdr->arp_data.arp_sha);
        out_arp_hdr->arp_data.arp_sip = sip;

        rte_ether_addr_copy(tha, &out_arp_hdr->arp_data.arp_tha);
        out_arp_hdr->arp_data.arp_tip = tip;

        return out_pkt;
}