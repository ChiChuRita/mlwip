FROM debian:bullseye-slim

RUN apt-get update && apt-get install -y \
    build-essential \
    cmake \
    check \
    git \
    gdb \
    valgrind \
    clangd \
    clang \
    curl \
    libclang-dev \
    && rm -rf /var/lib/apt/lists/*

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable

# Add Rust to PATH
ENV PATH="/root/.cargo/bin:${PATH}"

# Verify Rust installation
RUN rustc --version && cargo --version
