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
    && rm -rf /var/lib/apt/lists/*
