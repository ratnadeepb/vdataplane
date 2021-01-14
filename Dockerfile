# Building client container

ARG DPDK_VERSION=18.05
ARG OS_VER=18.04
FROM ubuntu:${OS_VER}

ENV RTE_TARGET=x86_64-native-linuxapp-gcc
ENV RTE_SDK=/dpdk-stable-19.11.6


RUN apt-get update && apt-get install -y \
	build-essential \
    libnuma-dev \
    linux-headers-$(uname -r) \
	linux-modules-extra-$(uname -r) \
	libclang-dev \
	clang \
	llvm-dev \
	libpcap-dev \
	dpdk-igb-uio-dkms \
    ethtool \
    net-tools \
    git \
    libunwind8 \
    apt-transport-https \
    libtool \
    python3 \
	libzmq3-dev \
	python3-pip