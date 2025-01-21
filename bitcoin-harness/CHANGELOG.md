# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1] - 2022-09-19

### Changed

-   Replace JSON RPC Client with thomaseizinger/rust-jsonrpc-client

## [0.2.0] - 2020-12-04

### Changed

-   **Breaking Change**: Use `bitcoincore_rpc_json::GetRawTransactionVerboseResponse` as return type for `get_raw_transaction_verbose` to get all fields.
-   Expose bitcoind rpc client from wallet.
-   **Breaking Change**: Upgrade rust-bitcoin to 0.25.

## [0.1.0] - 2020-11-11

### Added

-   A library to spin up a bitcoind node and do the things we always do: activate segwit, fund addresses, mine blocks.

[Unreleased]: https://github.com/coblox/bitcoin-harness-rs/compare/0.2.1...HEAD
[0.2.1]: https://github.com/coblox/bitcoin-harness-rs/compare/0.2.1...0.2.0
[0.2.0]: https://github.com/coblox/bitcoin-harness-rs/compare/0.1.0...0.2.0
[0.1.0]: https://github.com/coblox/bitcoin-harness-rs/compare/5549a14a3c5021998a5b4b681bf92b5f2fddf525...0.1.0
