# This Dockerfile builds the asb binary

FROM rust:1.82.0-slim-bookworm AS builder

WORKDIR /build

# Install dependencies
# See .github/workflows/action.yml as well
RUN apt-get update && \
    apt-get install -y \
        git \
        clang \
        libsnappy-dev \
        build-essential \
        cmake \
        libboost-all-dev \
        miniupnpc \
        libunbound-dev \
        graphviz \
        doxygen \
        libunwind8-dev \
        pkg-config \
        libssl-dev \
        libzmq3-dev \
        libsodium-dev \
        libhidapi-dev \
        libabsl-dev \
        libusb-1.0-0-dev \
        libprotobuf-dev \
        protobuf-compiler \
        ccache && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

COPY . .

# Update submodules recursively
RUN git submodule sync --recursive && git submodule update --init --recursive

WORKDIR /build/swap

RUN cargo build -vv --release --bin=asb

FROM debian:bookworm-slim

WORKDIR /data

COPY --from=builder /build/target/release/asb /bin/asb

ENTRYPOINT ["asb"]
