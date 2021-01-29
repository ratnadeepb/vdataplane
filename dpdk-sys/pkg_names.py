#!/usr/bin/python3
import subprocess
import os

# cmd = "pkg-config --static --libs libdpdk"
d = os.getenv("RTE_SDK")
libs = os.listdir(d + "/build/lib")
for lib in libs:
	if lib.endswith("so"):
		
proc = subprocess.Popen(cmd, stdout=subprocess.PIPE)
output, err = proc.communicate()

pkglist = set()

for elem in output.split():
	e = elem.decode('utf-8')
	if e.find("-l:lib") > -1:
		pkglist.add(e[6:-2])
	elif e.find("-lrte") > -1:
		pkglist.add(e[2:])

print(pkglist)