#!/bin/bash
# build the container
docker build . -t ratnadeepb/go-client:latest
# run the container in detached mode and publish ports
# docker run -it -d -P --name proxy1 ratnadeepb/go-client:latest
docker run -it -d -v /mydata/vdataplane_v3/go_docker/log:/log -p 9999:9999 --name proxy1 ratnadeepb/go-client:latest
# docker run -it -d --mount source=log,destination=/log -p 9999:9999 --name proxy1 ratnadeepb/go-client:latest