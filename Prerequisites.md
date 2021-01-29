# Installing DPDK and all other prerequisites

## Update libraries
```bash
sudo apt update
sudo apt install -y build-essential linux-headers-$(uname -r) git libnuma-dev linux-modules-extra-$(uname -r) libclang-dev clang llvm-dev libpcap-dev dpdk-igb-uio-dkms

python3 --version

ret=`echo $?`; if [[ $ret -ne 0 ]]; then sudo apt install python3; fi

sudo apt install -y libzmq3-dev python3-pip
pip3 install setuptools
```

## Install Ninja
```bash
sudo apt install ninja-build
```

## Install Meson
Get meson from [here](https://github.com/mesonbuild/meson/releases/) - 0.56.0 is the latest version.
```bash
wget https://github.com/mesonbuild/meson/releases/download/0.56.0/meson-0.56.0.tar.gz
tar xvfh meson-0.56.0.tar.gz
cd meson-0.56.0 && sudo python3 setup.py install
```

## Hugepages
### If `sudo su` permission is there
```bash
sudo su
echo "vm.nr_hugepages=2048" >> /etc/sysctl.conf
sysctl -e -p
exit
```
### Otherwise
```bash
sudo vim /etc/sysctl.conf # Add "vm.nr_hugepages=2048" at the end of the file
sudo sysctl -e -p
```

## DPDK
### Get DPDK
```bash

cd
wget http://fast.dpdk.org/rel/dpdk-20.11.tar.xz # latest stable version
tar xvfh dpdk-20.11.tar.xz
cd dpdk-20.11
echo "export RTE_SDK=$(pwd)"  >> ~/.bashrc
echo "export RTE_TARGET=x86_64-native-linuxapp-gcc" >> ~/.bashrc
source ~/.bashrc
```

### Build and Install DPDK
```bash
cd $RTE_SDK
meson build
ninja -C build && sudo ninja -C build install
sudo ldconfig
```

### Export the Libraries
```bash
echo "export LD_LIBRARY_PATH=$RTE_SDK/lib" >> ~/.bashrc
source ~/.bashrc
```

### Get igb_uio driver
```bash
cd
git clone git://dpdk.org/dpdk-kmods
cd dpdk-kmods/linux/igb_uio
make
sudo modprobe igb_uio
sudo insmod igb_uio.ko
```