# A two stage build
FROM rust:1.50 AS build

WORKDIR /usr/src
RUN rustup target add x86_64-unknown-linux-musl

RUN USER=root cargo new async_client

# Build the dependencies
WORKDIR /usr/src/fdpass-rs
COPY fdpass-rs/ ./
RUN cargo build

WORKDIR /usr/src/ipc-queue
COPY ipc-queue/ ./
RUN cargo build

WORKDIR /usr/src/memenpsf
COPY memenpsf/ ./
RUN cargo build

# Build the app
WORKDIR /usr/src/ipc-client
COPY ipc-client/ ./

RUN cargo install --target x86_64-unknown-linux-musl --path .

# Copy the statically-linked binary into a scratch container
FROM scratch
COPY --from=build /usr/local/cargo/bin/ipc-client .
USER 1000

CMD ["./ipc-client"]