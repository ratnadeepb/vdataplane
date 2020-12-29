#!/usr/bin/python3
import os
import sys


def cpu_count():
    return os.cpu_count()

# script taken from: https://github.com/sdnfv/openNetVM/blob/master/scripts/corehelper.py


def onvm_ht_isEnabled():
    lscpu_output = os.popen('lscpu -p').readlines()
    for line in lscpu_output:
        try:
            line_csv = line.split(',')
            phys_core = int(line_csv[0])
            logical_core = int(line_csv[1])
            if phys_core != logical_core:
                return True
        except ValueError:
            pass

    return False


def mem_threshold():
    mem = os.popen("cat /proc/meminfo").readlines()
    free_mem_gb = int(mem[1].split()[1]) / 1000 / 1000
    return free_mem_gb


def eth_cards():
    dirname = os.getenv("RTE_SDK")
    if not dirname:
        sys.stderr.write("RTE_SDK undefined")

    cmd = dirname + "/usertools/dpdk-devbind.py --status"
    devs = os.popen(cmd).readlines()

    primary_devs = []
    backup_devs = []

    for line in devs:
        if line.find("*Active*") > -1:
            cnt = line.split()[0].split(':')[1]
        if line.find("if=") > -1 and line.find(cnt) == -1:
            primary_devs.append(line.split()[0])
        if line.find("if=") > -1 and line.find(cnt) > -1 and line.find("*Active*") == -1:
            backup_devs.append(line.split()[0])
    return (primary_devs, backup_devs)


def run():
    if onvm_ht_isEnabled():
        sys.stderr.write("disable hyperthreading\n")
        sys.stderr.write("Run: `sudo ./no_hyperthread.sh`\n")

    sockets = cpu_count()

    if sockets < 4:
        sys.stderr.write("Too few cores\n")
        sys.exit(1)

    mem = mem_threshold()
    print(mem)
    if mem < 4:
        sys.stderr.write("Should have at least 4 gb of free memory\n")
        sys.exit(1)

    print("sockets found:", sockets)
    print("memory found:", mem, "gb")

    primary_devs, backup_devs = eth_cards()
    if primary_devs and backup_devs:
        print("Primary cards:", primary_devs)
        print("Backup cards:", backup_devs)
    elif not primary_devs:
        sys.stderr.write("primary devs not found\n")
        print("Backup cards:", backup_devs)
    elif not primary_devs and not backup_devs:
        sys.stderr.write("no suitable eth cards found\n")
        sys.exit(1)


if __name__ == "__main__":
    run()
