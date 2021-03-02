package main

import (
	"bufio"
	"fmt"
	"log"
	"net"
	"os"
)

func main() {
	ln, err := net.Listen("tcp", "127.0.0.1:9999")
	if err != nil {
		log.Fatalln(err)
	}

	for {
		// accept connection
		c, err := ln.Accept()
		if err != nil {
			log.Println(err)
			continue
		}

		// handle connection
		go handleServerConnection(c)
	}
}

func handleServerConnection(c net.Conn) {
	remoteAddr := c.RemoteAddr().String()
	log.Println("Connection from: ", remoteAddr)

	// echo received messages
	scanner := bufio.NewScanner(c)
	f, err := os.Create("/log/data.txt")

	if err != nil {
		log.Fatal(err)
	}

	defer f.Close()

	for {
		ok := scanner.Scan()
		if !ok {
			break
		}

		fmt.Println(scanner.Text())
		_, err2 := f.WriteString(scanner.Text())
		if err2 != nil {
			log.Fatal(err2)
		}
	}

	log.Println("Client at ", remoteAddr, " disconnected")
}
