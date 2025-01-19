# `monero_rust`
Proof of concept `monero_c` bindings for Rust.

## Getting started
<!--
### Prerequisites
You may need
```
sudo apt-get install libhidapi-dev
```
-->
### Build the `monero_c` library

#### Pull the `monero_c` library sub-module
```bash
# Execute this from the monero-native directory
git submodule update --init --recursive
```

#### Build the `monero_c` library
```bash
cd monero_c # Navigate to the monero_c directory

# Update the submodule for the monero source code
git submodule update --init --recursive --force

# Apply patches to the monero_c library
for coin in monero wownero zano; do ./apply_patches.sh $coin; done

# Build the monero_c library for the current architecture
./build_single.sh monero aarch64-apple-darwin -j14 # Adjust the architecture as needed
```

#### Run the extraction script
```bash
cd .. # Navigate back to the monero-native directory
./scripts/extract_from_build_from_source.sh
```

### Run demo
With the library in a supported location:
```
cargo run --example basic
```

## Using `monero_rust` in your own crate
Refer to the `example` folder.  Library placement is the same as for the demo. 