#!/usr/bin/python3
#
# Created on Fri Jan 08 2021:20:27:16
# Created by Ratnadeep Bhattacharya
#

import scapy

def gen_tcp(dst):
    """
    create a tcp packet to send to the dst
    """
	eth_layer = scapy.Ether()
    ip_layer = scapy.IP(dst=dst)
    tcp_layer = scapy.TCP()
    return eth_layer / ip_layer / tcp_layer

def send(pkt, iface):
	"""
	send a packet across
	"""
    scapy.sendp(pkt, iface=iface)


def send_tcp(dst, iface):
	"""
	send a tcp packet to dst
	"""
    send(gen_tcp(dst), iface)


if __name__ == "__main__":
	"""
	ethernet = Ether()
	network = IP(dst = '192.168.1.1')
	transport = ICMP()
	packet = ethernet/network/transport
	sendp(packet, iface="en0")
	"""
    send_tcp("10.10.1.1", "enp6s0f0")
