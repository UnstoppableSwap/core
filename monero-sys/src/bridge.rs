use cxx::CxxString;

/// This is the main ffi module that exposes the Monero C++ API to Rust.
/// See [cxx.rs](https://cxx.rs/book/ffi-modules.html) for more information
/// on how this works exactly.
///
/// Basically, we just write a corresponding rust function/type header for every c++
/// function/type we wish to call.
#[cxx::bridge(namespace = "Monero")]
pub mod ffi {

    /// The type of the network.
    enum NetworkType {
        #[rust_name = "Mainnet"]
        MAINNET,
        #[rust_name = "Testnet"]
        TESTNET,
        #[rust_name = "Stagenet"]
        STAGENET,
    }

    /// The status of the connection to the daemon.
    #[repr(u32)]
    enum ConnectionStatus {
        #[rust_name = "Disconnected"]
        ConnectionStatus_Disconnected = 0,
        #[rust_name = "Connected"]
        ConnectionStatus_Connected = 1,
        #[rust_name = "WrongVersion"]
        ConnectionStatus_WrongVersion = 2,
    }

    unsafe extern "C++" {
        include!("wallet/api/wallet2_api.h");
        include!("bridge.h");

        /// A manager for multiple wallets.
        type WalletManager;

        /// A single wallet.
        type Wallet;

        /// The type of the network.
        type NetworkType;

        /// The status of the connection to the daemon.
        type ConnectionStatus;

        /// A pending transaction.
        type PendingTransaction;

        /// A wallet listener.
        ///
        /// Can be attached to a wallet and will get notified upon specific events.
        type WalletListener;

        /// Get the wallet manager.
        fn getWalletManager() -> *mut WalletManager;

        /// Create a new wallet.
        fn createWallet(
            self: Pin<&mut WalletManager>,
            path: &CxxString,
            password: &CxxString,
            language: &CxxString,
            network_type: NetworkType,
            kdf_rounds: u64,
        ) -> *mut Wallet;

        /// Create a new wallet from keys.
        #[allow(clippy::too_many_arguments)]
        fn createWalletFromKeys(
            self: Pin<&mut WalletManager>,
            path: &CxxString,
            password: &CxxString,
            language: &CxxString,
            network_type: NetworkType,
            restore_height: u64,
            address: &CxxString,
            view_key: &CxxString,
            spend_key: &CxxString,
            kdf_rounds: u64,
        ) -> *mut Wallet;

        /// Recover a wallet from a mnemonic seed (electrum seed).
        #[allow(clippy::too_many_arguments)]
        fn recoveryWallet(
            self: Pin<&mut WalletManager>,
            path: &CxxString,
            password: &CxxString,
            mnemonic: &CxxString,
            network_type: NetworkType,
            restore_height: u64,
            kdf_rounds: u64,
            seed_offset: &CxxString,
        ) -> *mut Wallet;

        ///virtual Wallet * openWallet(const std::string &path, const std::string &password, NetworkType nettype, uint64_t kdf_rounds = 1, WalletListener * listener = nullptr) = 0;
        unsafe fn openWallet(
            self: Pin<&mut WalletManager>,
            path: &CxxString,
            password: &CxxString,
            network_type: NetworkType,
            kdf_rounds: u64,
            listener: *mut WalletListener,
        ) -> *mut Wallet;

        /// Close a wallet, optionally storing the wallet state.
        unsafe fn closeWallet(
            self: Pin<&mut WalletManager>,
            wallet: *mut Wallet,
            store: bool,
        ) -> bool;

        /// Check whether a wallet exists at the given path.
        fn walletExists(self: Pin<&mut WalletManager>, path: &CxxString) -> bool;

        /// Set the address of the remote node ("daemon").
        fn setDaemonAddress(self: Pin<&mut WalletManager>, address: &CxxString);

        /// Get the path of the wallet.
        fn walletPath(wallet: &Wallet) -> UniquePtr<CxxString>;

        /// Get the status of the wallet and an error string if there is one.
        fn statusWithErrorString(
            self: &Wallet,
            status: &mut i32,
            error_string: Pin<&mut CxxString>,
        );

        /// Address for the given account and address index.
        /// address(0, 0) is the main address.
        fn address(wallet: &Wallet, account_index: u32, address_index: u32)
            -> UniquePtr<CxxString>;

        /// Initialize the wallet by connecting to the specified remote node (daemon).
        #[allow(clippy::too_many_arguments)]
        fn init(
            self: Pin<&mut Wallet>,
            daemon_address: &CxxString,
            upper_transaction_size_limit: u64,
            daemon_username: &CxxString,
            daemon_password: &CxxString,
            use_ssl: bool,
            light_wallet: bool,
            proxy_address: &CxxString,
        ) -> bool;

        /// Check whether the wallet is connected to the daemon.
        fn connected(self: &Wallet) -> ConnectionStatus;

        /// Start the background refresh thread (refreshes every 10 seconds).
        fn startRefresh(self: Pin<&mut Wallet>);

        /// Refresh the wallet asynchronously.
        fn refreshAsync(self: Pin<&mut Wallet>);

        /// Set the daemon address.
        fn setWalletDaemon(wallet: Pin<&mut Wallet>, daemon_address: &CxxString) -> bool;

        /// Set whether the daemon is trusted.
        fn setTrustedDaemon(self: Pin<&mut Wallet>, trusted: bool);

        /// Get the current blockchain height.
        fn blockChainHeight(self: &Wallet) -> u64;

        /// Get the daemon's blockchain height.
        fn daemonBlockChainTargetHeight(self: &Wallet) -> u64;

        /// Check if wallet was ever synchronized.
        fn synchronized(self: &Wallet) -> bool;

        /// Get the total balance across all accounts in atomic units (piconero).
        fn balanceAll(self: &Wallet) -> u64;

        /// Get the total unlocked balance across all accounts in atomic units (piconero).
        fn unlockedBalanceAll(self: &Wallet) -> u64;

        /// Refresh the wallet synchronously.
        fn refresh(self: Pin<&mut Wallet>) -> bool;

        /// Force a specific restore height.
        fn setRefreshFromBlockHeight(self: Pin<&mut Wallet>, height: u64);

        /// Set whether to allow mismatched daemon versions.
        fn setAllowMismatchedDaemonVersion(self: Pin<&mut Wallet>, allow_mismatch: bool);

        /// Check whether a transaction is in the mempool / confirmed.
        fn checkTxKey(
            wallet: Pin<&mut Wallet>,
            txid: &CxxString,
            tx_key: &CxxString,
            address: &CxxString,
            received: &mut u64,
            in_pool: &mut bool,
            confirmations: &mut u64,
        ) -> bool;

        /// Create a new transaction.
        fn createTransaction(
            wallet: Pin<&mut Wallet>,
            dest_address: &CxxString,
            amount: u64,
        ) -> *mut PendingTransaction;

        /// Create a sweep transaction.
        fn createSweepTransaction(
            wallet: Pin<&mut Wallet>,
            dest_address: &CxxString,
        ) -> *mut PendingTransaction;

        /// Get the status of a pending transaction.
        fn status(self: &PendingTransaction) -> i32;

        /// Get the error string of a pending transaction.
        fn pendingTransactionErrorString(tx: &PendingTransaction) -> UniquePtr<CxxString>;

        /// Get the first transaction id of a pending transaction (if any).
        fn pendingTransactionTxId(tx: &PendingTransaction) -> UniquePtr<CxxString>;

        /// Get all transaction ids of a pending transaction.
        fn pendingTransactionTxIds(tx: &PendingTransaction) -> UniquePtr<CxxVector<CxxString>>;

        /// Get the transaction key (r) for a given txid.
        fn walletGetTxKey(wallet: &Wallet, txid: &CxxString) -> UniquePtr<CxxString>;

        /// Commit a pending transaction to the blockchain.
        fn commit(
            self: Pin<&mut PendingTransaction>,
            filename: &CxxString,
            overwrite: bool,
        ) -> bool;

        /// Dispose of a pending transaction object.
        unsafe fn disposeTransaction(self: Pin<&mut Wallet>, tx: *mut PendingTransaction);
    }
}

impl From<monero::Network> for ffi::NetworkType {
    fn from(network: monero::Network) -> Self {
        match network {
            monero::Network::Mainnet => ffi::NetworkType::Mainnet,
            monero::Network::Testnet => ffi::NetworkType::Testnet,
            monero::Network::Stagenet => ffi::NetworkType::Stagenet,
        }
    }
}

/// We want do use the `monero-rs` type so we convert as early as possible.
impl From<ffi::NetworkType> for monero::Network {
    fn from(network: ffi::NetworkType) -> Self {
        match network {
            ffi::NetworkType::Mainnet => monero::Network::Mainnet,
            ffi::NetworkType::Testnet => monero::Network::Testnet,
            ffi::NetworkType::Stagenet => monero::Network::Stagenet,
            // We have to include this path due to the way C++ translates the enum.
            // The enum only has these 3 values.
            _ => unreachable!(
                "There should be no other network type besides Mainnet, Testnet, and Stagenet"
            ),
        }
    }
}

/// This is a bridge that enables us to capture c++ log messages and forward them
/// to tracing.
///
/// We do this by installing a custom callback to the easylogging++ logging system
/// that forwards all log messages to our rust callback function.
#[cxx::bridge(namespace = "monero_rust_log")]
pub mod log {
    extern "Rust" {
        fn forward_cpp_log(
            level: u8,
            file: &CxxString,
            line: u32,
            func: &CxxString,
            msg: &CxxString,
        );
    }

    unsafe extern "C++" {
        include!("easylogging++.h");
        include!("bridge.h");

        fn install_log_callback();
    }
}

/// This is the actual rust function that forwards the c++ log messages to tracing.
/// Just calls e.g. `tracing::info!` with the appropriate log level and message.
fn forward_cpp_log(level: u8, file: &CxxString, _line: u32, func: &CxxString, msg: &CxxString) {
    let _file_str = file.to_string();
    let msg_str = msg.to_string();
    let func_str = func.to_string();

    // We don't want to log the performance timer.
    if func_str.starts_with("tools::LoggingPerformanceTimer") {
        return;
    }

    match level {
        0 => tracing::trace!(target: "monero_cpp", function=func_str, "{}", msg_str),
        1 => tracing::debug!(target: "monero_cpp", function=func_str, "{}", msg_str),
        2 => tracing::info!(target: "monero_cpp", function=func_str, "{}", msg_str),
        3 => tracing::warn!(target: "monero_cpp", function=func_str, "{}", msg_str),
        4 => tracing::error!(target: "monero_cpp", function=func_str, "{}", msg_str),
        _ => tracing::info!(target: "monero_cpp", function=func_str, "{}", msg_str),
    }
}
