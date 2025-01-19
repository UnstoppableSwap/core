#!/bin/bash

# This script is used to extract the built monero_c library from the build directory of the monero_c repository
# extracts it and copies it to the monero-native directory such that it can be used by the monero-native crate.
# Execute this script from the monero-native directory

set -x -e

# See https://github.com/MrCyjaneK/monero_c for the most up-to-date build docs,
# this is an example and a starting point for building monero_c for use in Rust
# but it should be automated either using CMake or Cargo (preferred).

# Detect architecture.
ARCH=$(uname -m)
OS=$(uname -s)

# Ensure we are in the monero-native directory, fail if not.
if [ ! -d "./monero_c" ]; then
    echo "Error: monero_c directory not found. Please run this script from the monero-native directory."
    exit 1
fi

case $ARCH-$OS in
    x86_64-Linux)
        TARGET_ARCH="x86_64-linux-gnu"
        ;;
    i686-Linux)
        TARGET_ARCH="i686-linux-gnu"
        ;;
    aarch64-Linux)
        TARGET_ARCH="aarch64-linux-gnu"
        ;;
    x86_64-Android)
        TARGET_ARCH="x86_64-linux-android"
        ;;
    i686-Android)
        TARGET_ARCH="i686-linux-android"
        ;;
    aarch64-Android)
        TARGET_ARCH="aarch64-linux-android"
        ;;
    armv7l-Android)
        TARGET_ARCH="arm-linux-androideabi"
        ;;
    i686-Windows)
        TARGET_ARCH="i686-w64-mingw32"
        ;;
    x86_64-Windows)
        TARGET_ARCH="x86_64-w64-mingw32"
        ;;
    x86_64-Darwin)
        TARGET_ARCH="host-apple-darwin"
        ;;
    arm64-Darwin)
        TARGET_ARCH="aarch64-apple-darwin"
        ;;
    *)
        echo "Unsupported architecture: $ARCH on OS: $OS"
        exit 1
        ;;
esac


# TOOD: Use .so or .dylib depending on the architecture

# Unzip the archive but keep it
unxz -f -k monero_c/release/monero/${TARGET_ARCH}_libwallet2_api_c.dylib.xz

# Create target directories if they don't exist
mkdir -p "./target/debug/deps"
mkdir -p "./target/release/deps"

# Copy the built .so file to a generic name.
SO_FILE="./monero_c/release/monero/${TARGET_ARCH}_libwallet2_api_c.dylib"
if [[ -f "$SO_FILE" ]]; then
    cp "$SO_FILE" "./libs/libwallet2_api_c.dylib"
    echo "Copied $SO_FILE to libwallet2_api_c.dylib"
else
    echo "Error: $SO_FILE not found."
    exit 1
fi
