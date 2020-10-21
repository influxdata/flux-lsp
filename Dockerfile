FROM ubuntu:20.04

WORKDIR /src

ENV CC=clang
ENV PATH="~/.cargo/bin:${PATH}"

# Install common packages
RUN apt-get update && \
  apt-get install --no-install-recommends -y \
  ca-certificates \
  curl file git openssh \
  build-essential \
  openssl libssl-dev \
  clang pkg-config llvm \
  autoconf automake autotools-dev libtool xutils-dev git \
  && rm -rf /var/lib/{apt,dpkg,cache,log}

# Install Rust
ENV RUST_VERSION 1.47.0
RUN curl https://sh.rustup.rs -sSf | \
  sh -s -- --default-toolchain $RUST_VERSION -y

# Install wasm-pack
RUN ~/.cargo/bin/cargo install wasm-pack

# Install Node.js
RUN curl -sL https://deb.nodesource.com/setup_12.x | bash -
RUN apt-get install -y nodejs

# Install linting tools
RUN ~/.cargo/bin/rustup component add rustfmt
RUN ~/.cargo/bin/rustup component add clippy
