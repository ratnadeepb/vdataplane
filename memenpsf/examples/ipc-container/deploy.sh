#!/bin/bash
sudo docker build -t ratnadeepb/ipc-client .
sudo docker push ratnadeepb/ipc-client:latest

# we need permissions on the socket
chmod 777 /tmp/fd-passrd.socket

sudo docker run -v /tmp:/tmp -d ratnadeepb/ipc-client:latest