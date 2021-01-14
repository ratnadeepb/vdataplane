#!/bin/bash
sudo docker run \
	--rm -it -d \
	--security-opt seccomp=unconfined \
	--privileged \
	-v /mnt/huge:/mnt/huge \
	-v /usr/src/linux-headers-$kernel_version:/usr/src/linux-headers-$kernel_version \
	-v /usr/src/kernels/$kernel_version:/usr/src/kernels/$kernel_version \
	-v /lib/modules/$kernel_version:/lib/modules/$kernel_version \
	-v /sys/bus/pci/devices:/sys/bus/pci/devices \
	-v /sys/kernel/mm/hugepages:/sys/kernel/mm/hugepages \
	-v /sys/devices/system/node:/sys/devices/system/node \
	-v /dev:/dev \
	--device=/dev/hugepages:/dev/hugepages \
	--device=/dev/uio0:/dev/uio0 \
	-v "{RTE_SDK}":"{RTE_SDK}" \
	--network=host \
	--name sidecar \
	ratnadeepb/L7proxy