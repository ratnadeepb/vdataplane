#!/usr/bin/python3

from scapy.all import *


def gen_tcp(dst):
    """
    create a tcp packet to send to the dst
    """
	eth_layer = Ether()
    ip_layer = IP(dst=dst)
    tcp_layer = TCP()
    return eth_layer / ip_layer / tcp_layer


def send(pkt):
	"""
	send a packet across
	"""
    send(pkt)


def send_tcp(dst):
	"""
	send a tcp packet to dst
	"""
    send(gen_tcp(dst))


if __name__ == "__main__":
	"""
	ethernet = Ether()
	network = IP(dst = '192.168.1.1')
	transport = ICMP()
	packet = ethernet/network/transport
	sendp(packet, iface="en0")
	"""
	while True:
    	send_tcp("10.10.1.1")
