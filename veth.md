# Config

## Note about Docker runtime network namespace

Docker creates the network namespace at `CNI_NETNS=/proc/$pid/ns/net` instead of at `/var/run/`

This can be changed:
```bash
pid=$(docker inspect -f '{{.State.Pid}}' <container_name OR UUID>)
sudo mkdir -p /var/run/netns
sudo ln -sf /proc/$pid/ns/net /var/run/netns/<container_name OR UUID>
sudo ip netns
sudo ip netns exec <container_name OR UUID> ip a
```

## change to the network directory

```bash
cd /etc/cni/net.d/
```

## kubernetes uses the first file alphabetically in the above directory

```bash
vi 20-demo.conf
   {
        "cniVersion": "0.3.1",
        "name": "myDemoPlugin",
        "type": "demo",
        "bridge": "demobr",
        "gateway": "10.0.0.1",
    }
```

## Create the bridge

```bash
sudo brctl addbr demobr
sudo ip addr add 10.0.0.1/24 dev demobr
```

## Go to the plugin directory

```bash
cd /opt/cni/bin
vi demo
#!/bin/bash
exec 2 >> /var/log/demolog
echo $CNI_COMMAND >> /var/log/demolog

conf = $(cat /dev/stdin) # read the config from the stdin

# Retrieve custom variables
bridge=$(echo $conf | jq -r ".bridge") # jq is for parsing json values
gateway=$(echo $conf | jq -r ".gateway")

# Name the network space
nsname=$CNI_CONTAINERID # container runtime sets the container id in this envar

# IPAM
ip_addr=10.0.0.100/24

if [[ $CNI_COMMAND == "ADD" ]]; then
    # create our namespace
    mkdir -p /var/run/netns
    ln -sfT $CNI_NETNS /var/run/netns/$nsname
    # veth pair
    ip link add veth_root type veth peer name veth_ns
    # Handle veth_root
    ip link set veth_root master $bridge
    ip link set veth_root up
    ip link set dev $bridge up
    
    # move the other link to the ns
    ip link set veth_ns netns $nsname
    ip -netns $nsname link set dev veth_ns down
    ip -netns $nsname link set veth_ns name $CNI_IFNAME
    ip -netns $nsname link set $CNI_IFNAME up

    # IPAM
    ip -netns $nsname addr add $ip_addr dev $CNI_IFNAME

    # Routing
    ip -netns $nsname route add default via $gateway

    # MAC of added interface
    mac = $(ip -netns $nsname link name $CNI_IFNAME | grep link | awk '{print $2}')
    # Interface index of the added interface
    interface_index=$(ip -netns $nsname link show $CNI_IFNAME | grep $CNI_IFNAME | awk -F ':' '{print $1}')

    OUTPUT_TEMPLATE='
    {
        "cniVersion": "0.3.1",
        "interfaces": [
            {
                "name": "%s",
                "mac": "%s",
                "sandbox": "%s", # network namespace
            }
        ],
        "ips": [
            {
                "version": "4",
                "address": "%s",
                "gateway": "%s",
                "interface": "%s",
            }
        ]
    }
    '
    OUTPUT=$(printf "$OUTPUT_TEMPLATE" $CNI_IFNAME $mac $CNI_NETNS $ip_addr $gateway $interface_index)

    echo $OUTPUT >> /var/log/demolog
fi

if [[ $CNI_COMMAND == "DELETE" ]]; then
    ip -netns del $nsname || true
    ip link del veth_root || true
fi
```

## Checking the network

```bash
ip link show type veth
ip link show type bridge
bridge link show | grep cni0 # brctl show cni0
```

## Manually doing it

1. Docker container is started with `None` network

```bash
docker run -d -t --net=none --name=<name> <image> <command>
```

2. Get the container sandbox key and container id

```bash
sandbox_key=$(sudo docker inspect -f '{{.NetworkSettings.SandboxKey}}' <container id | UUID>)
pid=$(sudo docker inspect -f '{{.State.Pid}}' <container_name OR UUID>)
```

3. Set up the network

```bash
cat /etc/cni/net.d/10-mynet.conf | CNI_COMMAND=ADD \
    CNI_CONTAINERID=<cont id> \
    CNI_NETNS=<sandbox key> \
    CNI_IFNAME=eth0 \
    CNI_PATH=/opt/cni/bin /usr/local/sbin/cni/bridge
```