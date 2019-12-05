FROM ubuntu:19.04

WORKDIR /src

ENV CC=clang

# Install common packages
RUN apt-get update && \
    apt-get install --no-install-recommends -y \
    ca-certificates \
    curl file git \
    build-essential \
    openssl libssl-dev \
    clang pkg-config llvm \
    autoconf automake autotools-dev libtool xutils-dev

# Install Rust
RUN curl https://sh.rustup.rs -sSf | \
    sh -s -- --default-toolchain stable -y

# Install wasm-pack
RUN $HOME/.cargo/bin/cargo install wasm-pack
