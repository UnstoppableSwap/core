#![doc = include_str!("../README.md")]

mod bridge;

use std::{
    collections::HashMap,
    ops::Deref,
    pin::Pin,
    str::FromStr,
    sync::{Arc, OnceLock},
};

use anyhow::{bail, Context, Result};
use cxx::let_cxx_string;
use tokio::sync::Mutex;

use bridge::ffi;

/// A thread-safe wrapper around the C++ wallet manager, which is a singleton.
/// Initialized via [`WalletManager::get`].
static WALLET_MANAGER: OnceLock<Arc<Mutex<WalletManager>>> = OnceLock::new();

/// A singleton responsible for managing (creating, opening, ...) wallets.
pub struct WalletManager {
    /// A wrapper around the raw C++ wallet manager pointer.
    inner: RawWalletManager,
    /// A map of opened wallets, indexed by their absolute path.
    wallets: HashMap<String, Arc<Mutex<Wallet>>>,
    /// The daemon to connect to. All wallets opened
    /// by the manager will connect to this daemon, too.
    daemon: Option<Daemon>,
}

/// This is our own wrapper around a raw C++ wallet manager pointer.
struct RawWalletManager(*mut ffi::WalletManager);

/// A single Monero wallet.
pub struct Wallet {
    inner: RawWallet,
}

/// This is our own wrapper around a raw C++ wallet pointer.
struct RawWallet(*mut ffi::Wallet);

/// The progress of synchronization of a wallet with the remote node.
pub struct SyncProgress {
    /// The current block height of the wallet.
    pub current_block: u64,
    /// The target block height of the wallet.
    pub target_block: u64,
}

/// The status of a transaction.
pub struct TxStatus {
    /// The amount received in the transaction.
    pub received: monero::Amount,
    /// Whether the transaction is in the mempool.
    pub in_pool: bool,
    /// The number of confirmations the transaction has.
    pub confirmations: u64,
}

/// A receipt returned after successfully publishing a transaction.
/// Contains basic information needed for later verification.
pub struct TxReceipt {
    pub txid: String,
    pub tx_key: String,
    pub height: u64,
}

/// A remote node to connect to.
#[derive(Debug, Clone, Default)]
pub struct Daemon {
    pub address: String,
    pub ssl: bool,
}

/// A wrapper around a pending transaction.
pub struct PendingTransaction(*mut ffi::PendingTransaction);

impl WalletManager {
    /// For now we don't support custom difficulty
    const DEFAULT_KDF_ROUNDS: u64 = 1;

    /// Get the wallet manager instance.
    /// You can optionally pass a daemon with which the wallet manager and
    /// all wallets opened by the manager will connect.
    pub async fn get<'a>(daemon: Option<Daemon>) -> anyhow::Result<Arc<Mutex<Self>>> {
        let manager = WALLET_MANAGER.get_or_init(|| {
            let manager = ffi::getWalletManager();
            if manager.is_null() {
                panic!("Failed to get wallet manager, got null pointer");
            }
            let manager = Self {
                inner: RawWalletManager(manager),
                wallets: HashMap::new(),
                daemon: daemon.clone(),
            };

            Arc::new(Mutex::new(manager))
        });

        {
            // We do this in a block to ensure that the lock is released as soon as possible.
            let mut lock = manager.lock().await;

            // If a remote node is provided, set it as the default remote node.
            if let Some(daemon) = daemon {
                lock.set_daemon_address(&daemon.address);
            }
        }

        Ok(manager.clone())
    }

    /// Create a new wallet, or open if it already exists.
    pub async fn open_or_create_wallet(
        &mut self,
        filename: &str,
        password: Option<&str>,
        network: monero::Network,
    ) -> anyhow::Result<Arc<Mutex<Wallet>>> {
        tracing::info!(
            "Opening or creating wallet: {}. Currently opened {} wallets.",
            filename,
            self.wallets.len()
        );

        // If we've already loaded this wallet, return it.
        if self.wallets.contains_key(filename) {
            let wallet = self.wallets[filename].clone();
            return Ok(wallet);
        }

        let kdf_rounds = Self::DEFAULT_KDF_ROUNDS;
        // If we haven't loaded the wallet, but it already exists, open it.
        if self.wallet_exists(filename).await {
            tracing::debug!(wallet=%filename, "Wallet already exists, opening it");

            return self
                .open_wallet(filename, password, network)
                .await
                .context(format!("Failed to open wallet `{}`", &filename));
        }

        // Otherwise, create (and open) a new wallet.
        let_cxx_string!(path = filename);
        let_cxx_string!(password = password.unwrap_or(""));
        let_cxx_string!(language = "");
        let network_type = network.into();

        let wallet_pointer =
            self.inner
                .pinned()
                .createWallet(&path, &password, &language, network_type, kdf_rounds);

        if wallet_pointer.is_null() {
            anyhow::bail!("Failed to create wallet, got null pointer");
        }

        let raw_wallet = RawWallet(wallet_pointer);
        let wallet = Wallet::new(raw_wallet, self.daemon.clone())
            .await
            .context(format!("Failed to initialize wallet `{}`", &filename))?;

        let wallet = Arc::new(Mutex::new(wallet));
        self.wallets.insert(filename.to_string(), wallet.clone());

        Ok(wallet)
    }

    /// Create a new wallet from keys or open if it already exists.
    #[allow(clippy::too_many_arguments)]
    pub async fn open_or_create_wallet_from_keys(
        &mut self,
        path: &str,
        password: Option<&str>,
        network: monero::Network,
        address: &monero::Address,
        view_key: monero::PrivateKey,
        spend_key: monero::PrivateKey,
        restore_height: u64,
    ) -> Result<Arc<Mutex<Wallet>>> {
        if self.wallet_exists(path).await {
            tracing::info!(wallet=%path, "Wallet already exists, opening it");

            self.open_wallet(path, password, network)
                .await
                .context(format!("Failed to open wallet `{}`", &path))?;
        }

        let_cxx_string!(path = path);
        let_cxx_string!(password = password.unwrap_or(""));
        let_cxx_string!(language = "");
        let network_type = network.into();
        let_cxx_string!(address = address.to_string());
        let_cxx_string!(view_key = view_key.to_string());
        let_cxx_string!(spend_key = spend_key.to_string());
        let kdf_rounds = Self::DEFAULT_KDF_ROUNDS;

        let wallet_pointer = self.inner.pinned().createWalletFromKeys(
            &path,
            &password,
            &language,
            network_type,
            restore_height,
            &address,
            &view_key,
            &spend_key,
            kdf_rounds,
        );

        if wallet_pointer.is_null() {
            anyhow::bail!("Failed to create wallet from keys, got null pointer");
        }

        let raw_wallet = RawWallet(wallet_pointer);
        let wallet = Wallet::new(raw_wallet, self.daemon.clone())
            .await
            .context(format!("Failed to initialize wallet `{}`", &path))?;

        let wallet = Arc::new(Mutex::new(wallet));
        self.wallets.insert(path.to_string(), wallet.clone());

        Ok(wallet)
    }

    /// Recover a wallet from a mnemonic seed (electrum seed).
    #[allow(clippy::too_many_arguments)]
    pub async fn recover_wallet(
        &mut self,
        path: &str,
        password: &str,
        mnemonic: &str,
        network: monero::Network,
        restore_height: u64,
    ) -> anyhow::Result<Arc<Mutex<Wallet>>> {
        let_cxx_string!(path = path);
        let_cxx_string!(password = password);
        let_cxx_string!(mnemonic = mnemonic);
        let_cxx_string!(seed_offset = "");

        let network_type = network.into();
        let wallet_pointer = self.inner.pinned().recoveryWallet(
            &path,
            &password,
            &mnemonic,
            network_type,
            restore_height,
            Self::DEFAULT_KDF_ROUNDS,
            &seed_offset,
        );

        let raw_wallet = RawWallet(wallet_pointer);
        let wallet = Wallet::new(raw_wallet, self.daemon.clone()).await?;

        let wallet = Arc::new(Mutex::new(wallet));
        self.wallets.insert(path.to_string(), wallet.clone());

        Ok(wallet)
    }

    /// Close a wallet, optionally storing the wallet state.
    ///
    /// This function is only safe because
    ///  - the only way to obtain an owned `Mutex<Wallet>` is by `Arc::into_inner`, guaranteeing
    ///    that there are no other references to the wallet, and
    ///  - it consumes the Mutex, guaranteeing that no other references to the wallet
    ///    will be created.
    ///
    /// **DO NOT CHANGE OR YOU WILL TRIGGER UNDEFINED BEHAVIOR (BAD)**
    pub async fn close_wallet(&mut self, wallet: Mutex<Wallet>) -> anyhow::Result<()> {
        let path = wallet.lock().await.path();

        tracing::debug!(wallet_path = %path, "Closing wallet");

        // Safety: we know we have a valid, unique pointer to the wallet
        let success = unsafe {
            self.inner
                .pinned()
                .closeWallet(wallet.lock().await.inner.0, true)
        };

        if !success {
            anyhow::bail!("Failed to close wallet");
        }

        // Remove the wallet from the map
        self.wallets.remove(&path);

        Ok(())
    }

    pub async fn close_all_wallets(&mut self) -> anyhow::Result<()> {
        // To drop the Arcs, we need to remove all wallets from the map first.
        let mut wallets = HashMap::new();
        std::mem::swap(&mut wallets, &mut self.wallets);

        for (path, wallet) in wallets {
            let wallet = Arc::into_inner(wallet).context(
                "Couldn't unwrap Arc<Mutex<Wallet>>, not all other Arcs have been dropped",
            )?;
            self.close_wallet(wallet)
                .await
                .context(format!("Failed to close wallet `{}`", &path))?;
        }

        Ok(())
    }

    /// Open a wallet. Only used internally. Use [`WalletManager::open_or_create_wallet`] instead.
    ///
    /// Todo: add listener support?
    async fn open_wallet(
        &mut self,
        path: &str,
        password: Option<&str>,
        network_type: monero::Network,
    ) -> anyhow::Result<Arc<Mutex<Wallet>>> {
        let_cxx_string!(path = path);
        let_cxx_string!(password = password.unwrap_or(""));
        let network_type = network_type.into();
        let kdf_rounds = Self::DEFAULT_KDF_ROUNDS;

        let wallet_pointer = unsafe {
            self.inner.pinned().openWallet(
                &path,
                &password,
                network_type,
                kdf_rounds,
                std::ptr::null_mut(),
            )
        };

        if wallet_pointer.is_null() {
            anyhow::bail!("Failed to open wallet: got null pointer")
        }

        let raw_wallet = RawWallet(wallet_pointer);

        let wallet = Wallet::new(raw_wallet, self.daemon.clone())
            .await
            .context("Failed to initialize wallet")?;

        let wallet = Arc::new(Mutex::new(wallet));
        self.wallets.insert(path.to_string(), wallet.clone());

        Ok(wallet)
    }

    /// Get all open wallets.
    pub fn all_open_wallets(&self) -> Vec<Arc<Mutex<Wallet>>> {
        self.wallets.values().cloned().collect()
    }

    /// Set the address of the remote node ("daemon").
    fn set_daemon_address(&mut self, address: &str) {
        tracing::debug!(%address, "Updating wallet manager's remote node");

        let_cxx_string!(address = address);

        self.inner.pinned().setDaemonAddress(&address);
    }

    /// Check if a wallet exists at the given path.
    pub async fn wallet_exists(&mut self, path: &str) -> bool {
        let_cxx_string!(path = path);

        self.inner.pinned().walletExists(&path)
    }

    /// Check if the wallet manager is connected to the configured daemon.
    ///
    /// The manager might takes a few seconds to connect to the daemon after startup.
    pub async fn connected(&mut self) -> bool {
        let mut version = 0;
        unsafe { self.inner.pinned().connected(&mut version) }
    }

    /// Get the current blockchain height, if the manager is connected to a daemon.
    ///
    /// Returns None if the manager is not connected to a daemon.
    pub async fn blockchain_height(&mut self) -> Option<u64> {
        match self.inner.pinned().blockchainHeight() {
            0 => None,
            height => Some(height),
        }
    }
}

impl RawWalletManager {
    /// Get a pinned reference to the inner (c++) wallet manager.
    /// This is a convenience function necessary because
    /// the ffi interface mostly takes a Pin<&mut T> but
    /// we haven't figured out how to hold that in the struct.
    pub fn pinned(&mut self) -> Pin<&mut ffi::WalletManager> {
        unsafe {
            Pin::new_unchecked(
                self.0
                    .as_mut()
                    .expect("wallet manager pointer not to be null"),
            )
        }
    }
}

/// Safety: Todo
unsafe impl Send for RawWalletManager {}

impl Wallet {
    const MAIN_ACCOUNT_INDEX: u32 = 0;

    /// Create and initialize new wallet from a raw C++ wallet pointer.
    async fn new(inner: RawWallet, daemon: Option<Daemon>) -> anyhow::Result<Self> {
        if inner.0.is_null() {
            anyhow::bail!("Failed to create wallet: got null pointer");
        }

        let mut wallet = Self { inner };

        tracing::debug!("Initializing wallet");

        wallet.check_error()?;

        let daemon = daemon.unwrap_or_default();

        wallet
            .init(&daemon.address, daemon.ssl)
            .await
            .context("Failed to initialize wallet")?;

        wallet.start_refresh();

        Ok(wallet)
    }

    /// Get the path to the wallet file.
    pub fn path(&self) -> String {
        ffi::walletPath(&self.inner).to_string()
    }

    /// Get the address for the given account and address index.
    /// address(0, 0) is the main address.
    /// We don't use anything besides the main address so this is a private method (for now).
    fn address(&self, account_index: u32, address_index: u32) -> monero::Address {
        let address = ffi::address(&self.inner, account_index, address_index);
        monero::Address::from_str(&address.to_string()).expect("wallet's own address to be valid")
    }

    pub fn set_daemon_address(&mut self, address: &str) -> anyhow::Result<()> {
        let_cxx_string!(address = address);
        let success = ffi::setWalletDaemon(self.inner.pinned(), &address);

        if !success {
            self.check_error().context("Failed to set daemon address")?;
            anyhow::bail!("Failed to set daemon address");
        }

        Ok(())
    }

    /// Get the main address of the walllet (account 0, address 0).
    pub fn main_address(&self) -> monero::Address {
        self.address(Self::MAIN_ACCOUNT_INDEX, 0)
    }

    /// Initialize the wallet and download initial values from the remote node.
    /// Does not actuallyt sync the wallet, use any of the refresh methods to do that.
    async fn init(&mut self, daemon_address: &str, ssl: bool) -> anyhow::Result<()> {
        let_cxx_string!(daemon_address = daemon_address);
        let_cxx_string!(daemon_username = "");
        let_cxx_string!(daemon_password = "");
        let_cxx_string!(proxy_address = "");

        let success = self.inner.pinned().init(
            &daemon_address,
            0,
            &daemon_username,
            &daemon_password,
            ssl,
            true,
            &proxy_address,
        );

        if !success {
            self.check_error().context("Failed to initialize wallet")?;
            anyhow::bail!("Failed to initialize wallet");
        }

        Ok(())
    }

    /// Get the sync progress of the wallet as a percentage.
    ///
    /// Returns a zeroed sync progress if the daemon is not connected.
    pub async fn sync_progress(&mut self) -> SyncProgress {
        let current_block = self.inner.blockChainHeight();
        let target_block = self.daemon_blockchain_height().unwrap_or(0);

        if target_block == 0 {
            return SyncProgress::zero();
        }

        SyncProgress::new(current_block, target_block)
    }

    /// Sync the wallet with the remote node.
    ///
    /// You can optionally provide a listener that is called
    /// every time there is new sync progress.
    ///
    /// This may take some time. To avoid starving other tasks, this function
    /// takes a mutex and releases the lock in between polls.
    pub async fn wait_until_synced(
        mutex: Arc<Mutex<Self>>,
        listener: Option<impl Fn(SyncProgress)>,
    ) -> anyhow::Result<()> {
        // We wait for 250ms before polling the wallet's sync status again.
        // This is ok because this doesn't involve any blocking calls.
        const POLL_INTERVAL_MILLIS: u64 = 250;

        tracing::debug!("Waiting for wallet to sync");

        // Initiate the sync (make sure to drop the lock right after)
        {
            let mut wallet = mutex.lock().await;
            wallet.refresh_async().await;
            tracing::debug!("Wallet refresh initiated");
        }

        // Wait until the wallet is connected to the daemon.
        loop {
            let connected = { mutex.lock().await.connected() };

            if connected {
                break;
            }

            tracing::trace!(
                "Wallet not connected to daemon, sleeping for {}ms",
                POLL_INTERVAL_MILLIS
            );

            tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MILLIS)).await;
        }

        // Continue polling until the sync is complete
        loop {
            // Get the current sync status (releasing the lock immediately afterwords)
            let (synced, sync_progress) = {
                let mut wallet = mutex.lock().await;
                (wallet.synchronized(), wallet.sync_progress().await)
            };

            // Notify the listener (if it exists)
            if let Some(listener) = &listener {
                listener(sync_progress);
            }

            // If the wallet is synced, break out of the loop.
            if synced {
                break;
            }

            tracing::trace!(
                "Wallet sync not complete, sleeping for {}ms",
                POLL_INTERVAL_MILLIS
            );

            // Otherwise, sleep for a bit and try again.
            tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MILLIS)).await;
        }

        tracing::debug!("Wallet synced");

        Ok(())
    }

    fn connected(&self) -> bool {
        match self.inner.connected() {
            ffi::ConnectionStatus::Connected => true,
            ffi::ConnectionStatus::WrongVersion => {
                tracing::warn!("Version mismatch with daemon");
                false
            }
            ffi::ConnectionStatus::Disconnected => false,
            // Fallback since C++ allows any other value.
            status => {
                tracing::error!("Unknown connection status: `{}`", status.repr);
                false
            }
        }
    }

    /// Start the background refresh thread (refreshes every 10 seconds).
    fn start_refresh(&mut self) {
        self.inner.pinned().startRefresh();
    }

    /// Refresh the wallet asynchronously.
    /// Same as start_refresh except that the background thread only
    /// refreshes once. Maybe?
    async fn refresh_async(&mut self) {
        self.inner.pinned().refreshAsync();
    }

    /// Poll the daemon repeatedly until a transaction has a specified number of confirmations.
    /// Optionally accepts a listener that is called every time there is a new confirmation
    /// with the current number of confirmations.
    ///
    /// The default check interval is 15 seconds.
    ///
    /// Returns an error if the transaction is not found or the amount is incorrect.
    pub async fn wait_until_confirmed(
        wallet: Arc<Mutex<Wallet>>,
        txid: &str,
        tx_key: monero::PrivateKey,
        destination_address: &monero::Address,
        expected_amount: monero::Amount,
        confirmations: u64,
        listener: Option<impl Fn(u64)>,
    ) -> anyhow::Result<()> {
        const DEFAULT_CHECK_INTERVAL_SECS: u64 = 15;

        let mut poll_interval = tokio::time::interval(tokio::time::Duration::from_secs(
            DEFAULT_CHECK_INTERVAL_SECS,
        ));

        loop {
            poll_interval.tick().await;

            let tx_status = match wallet
                .lock()
                .await
                .check_tx_status(txid, tx_key, destination_address)
                .await
            {
                Ok(tx_status) => tx_status,
                Err(e) => {
                    tracing::error!(
                        "Failed to check tx status: {}, rechecking in {}s",
                        e,
                        DEFAULT_CHECK_INTERVAL_SECS
                    );
                    continue;
                }
            };

            // Make sure the amount is correct
            if tx_status.received != expected_amount {
                tracing::error!(
                    "Transaction received amount mismatch: expected {}, got {}",
                    expected_amount,
                    tx_status.received
                );
                return Err(anyhow::anyhow!(
                    "Transaction received amount mismatch: expected {}, got {}",
                    expected_amount,
                    tx_status.received
                ));
            }

            // If the listener exists, notify it of the result
            if let Some(listener) = &listener {
                listener(tx_status.confirmations);
            }

            // Stop when we have the required number of confirmations
            if tx_status.confirmations >= confirmations {
                break;
            }
        }

        // Signal success
        Ok(())
    }

    /// Get the current blockchain height.
    pub fn blockchain_height(&self) -> u64 {
        self.inner.blockChainHeight()
    }

    /// Get the daemon's blockchain height.
    ///
    /// Returns the height of the blockchain, if connected.
    /// Returns None if not connected.
    pub fn daemon_blockchain_height(&mut self) -> Option<u64> {
        // Here we actually use the _target_ height -- incase the remote node is
        // currently catching up we want to work with the height it ends up at.
        match self.inner.daemonBlockChainTargetHeight() {
            0 => None,
            height => Some(height),
        }
    }

    /// Get the total balance across all accounts.
    pub fn total_balance(&self) -> monero::Amount {
        let balance = self.inner.balanceAll();
        monero::Amount::from_pico(balance)
    }

    /// Get the total unlocked balance across all accounts in atomic units.
    pub fn unlocked_balance(&self) -> monero::Amount {
        let balance = self.inner.unlockedBalanceAll();
        monero::Amount::from_pico(balance)
    }

    /// Check if the wallet is synced with the daemon.
    pub fn synchronized(&self) -> bool {
        self.inner.synchronized()
    }

    /// Set the allow mismatched daemon version flag.
    ///
    /// This is needed for regnet compatibility.
    ///
    /// _Do not use for anything besides testing._
    pub fn allow_mismatched_daemon_version(&mut self) {
        self.inner.pinned().setAllowMismatchedDaemonVersion(true);
    }

    /// Check the status of a transaction.
    pub async fn check_tx_status(
        &mut self,
        txid: &str,
        tx_key: monero::PrivateKey,
        address: &monero::Address,
    ) -> anyhow::Result<TxStatus> {
        let_cxx_string!(txid = txid);
        let_cxx_string!(tx_key = tx_key.to_string());
        let_cxx_string!(address = address.to_string());

        let mut received = 0;
        let mut in_pool = false;
        let mut confirmations = 0;

        let success = ffi::checkTxKey(
            self.inner.pinned(),
            &txid,
            &tx_key,
            &address,
            &mut received,
            &mut in_pool,
            &mut confirmations,
        );

        if !success {
            self.check_error().context("Failed to check tx key")?;
            anyhow::bail!("Failed to check tx key");
        }

        Ok(TxStatus {
            received: monero::Amount::from_pico(received),
            in_pool,
            confirmations,
        })
    }

    /// Transfer a specified amount of monero to a specified address and return a receipt containing
    /// the transaction id, transaction key and current blockchain height. This can be used later
    /// to prove the transfer or to wait for confirmations.
    pub async fn transfer(
        &mut self,
        address: &monero::Address,
        amount: monero::Amount,
    ) -> anyhow::Result<TxReceipt> {
        let_cxx_string!(address = address.to_string());
        let amount = amount.as_pico();

        // First we need to create a pending transaction.
        let mut pending_tx = PendingTransaction(ffi::createTransaction(
            self.inner.pinned(),
            &address,
            amount,
        ));

        // Get the txid from the pending transaction before we publish,
        // otherwise it might be null.
        let txid = ffi::pendingTransactionTxId(&pending_tx) // UniquePtr<CxxString>
            .to_string();

        // Publish the transaction
        let result = pending_tx
            .publish()
            .await
            .context("Failed to publish transaction");

        // Check for errors (make sure to dispose the transaction)
        if result.is_err() {
            self.dispose_transaction(pending_tx).await;
            bail!("Failed to publish transaction");
        }

        // Fetch the tx key from the wallet.
        let_cxx_string!(txid_cxx = txid.clone());
        let tx_key = ffi::walletGetTxKey(&self.inner, &txid_cxx).to_string();

        // Get current blockchain height (wallet height).
        let height = self.blockchain_height();

        // Dispose the pending transaction object to avoid memory leak.
        self.dispose_transaction(pending_tx).await;

        Ok(TxReceipt {
            txid,
            tx_key,
            height,
        })
    }

    /// Sweep all funds from the wallet to a specified address.
    /// Returns a list of transaction ids of the created transactions.
    pub async fn sweep(&mut self, address: &monero::Address) -> anyhow::Result<Vec<String>> {
        let_cxx_string!(address = address.to_string());

        // Create the sweep transaction
        let mut pending_tx =
            PendingTransaction(ffi::createSweepTransaction(self.inner.pinned(), &address));

        // Get the txids from the pending transaction before we publish,
        // otherwise it might be null.
        let txids: Vec<String> = ffi::pendingTransactionTxIds(&pending_tx)
            .into_iter()
            .map(|s| s.to_string())
            .collect();

        // Publish the transaction
        let result = pending_tx
            .publish()
            .await
            .context("Failed to publish transaction");

        // Dispose of the transaction to avoid leaking memory.
        self.dispose_transaction(pending_tx).await;

        result.map(|_| txids)
    }

    /// Dispose (deallocate) a pending transaction object.
    /// Always call this before dropping a pending transaction object,
    /// otherwise we leak memory.
    async fn dispose_transaction(&mut self, tx: PendingTransaction) {
        unsafe { self.inner.pinned().disposeTransaction(tx.0) };
    }

    /// Return `Ok` when the wallet is ok, otherwise return the error.
    /// This is a convenience method we use for retrieving errors after
    /// a method call failed.
    fn check_error(&mut self) -> anyhow::Result<()> {
        let mut status = 0;
        let mut error_string = String::new();
        let_cxx_string!(error_string_ref = &mut error_string);

        self.inner
            .statusWithErrorString(&mut status, error_string_ref);

        // If the status is ok, we return None
        if status == 0 {
            return Ok(());
        }

        let error_type = if status == 2 { "critical" } else { "error" };

        // Otherwise we return the error
        bail!(format!(
            "Experienced wallet error ({}): {}",
            error_type, error_string
        ))
    }
}

/// # Safety: Todo
unsafe impl Send for RawWallet {}

impl PendingTransaction {
    /// Return `Ok` when the pending transaction is ok, otherwise return the error.
    /// This is a convenience method we use for retrieving errors after
    /// a method call failed.
    fn check_error(&self) -> anyhow::Result<()> {
        let status = self.status();
        let error_string = ffi::pendingTransactionErrorString(self);

        if status == 0 {
            return Ok(());
        }

        let error_type = if status == 2 { "critical" } else { "error" };

        bail!(format!(
            "Experienced pending transaction error ({}): {}",
            error_type, error_string
        ))
    }

    /// Publish this transaction to the blockchain or return an error.
    ///
    /// **Important**: you still have to dispose the transaction.
    async fn publish(&mut self) -> anyhow::Result<()> {
        self.check_error().context("Failed to create transaction")?;

        // Then we commit it to the blockchain.
        let_cxx_string!(filename = ""); // Empty filename means we commit to the blockchain
        let success = self.pinned().commit(&filename, false);

        if success {
            Ok(())
        } else {
            // Get the error from the pending transaction.
            Err(self
                .check_error()
                .context("Failed to commit transaction to blockchain")
                .err()
                .unwrap_or(anyhow::anyhow!(
                    "Failed to commit transaction to blockchain"
                )))
        }
    }
}

impl SyncProgress {
    /// Create a new sync progress object.
    fn new(current_block: u64, target_block: u64) -> Self {
        Self {
            current_block,
            target_block,
        }
    }

    /// Create a new sync progress object with zero progess.
    fn zero() -> Self {
        Self {
            current_block: 0,
            target_block: 1,
        }
    }

    /// Get the sync progress as a fraction.
    pub fn fraction(&self) -> f32 {
        self.current_block as f32 / self.target_block as f32
    }

    /// Get the sync progress as a percentage.
    pub fn percentage(&self) -> f32 {
        100.0 * self.fraction()
    }
}

impl RawWallet {
    /// Convenience method for getting a pinned reference to the inner (c++) wallet.
    fn pinned(&mut self) -> Pin<&mut ffi::Wallet> {
        unsafe { Pin::new_unchecked(self.0.as_mut().expect("wallet pointer not to be null")) }
    }
}

// We implement Deref for RawWallet such that we can use the
// const c++ methods directly on the RawWallet struct.
impl Deref for RawWallet {
    type Target = ffi::Wallet;

    fn deref(&self) -> &ffi::Wallet {
        unsafe { self.0.as_ref().expect("wallet pointer not to be null") }
    }
}

impl PendingTransaction {
    fn pinned(&mut self) -> Pin<&mut ffi::PendingTransaction> {
        unsafe {
            Pin::new_unchecked(
                self.0
                    .as_mut()
                    .expect("pending transaction pointer not to be null"),
            )
        }
    }
}

impl Deref for PendingTransaction {
    type Target = ffi::PendingTransaction;

    fn deref(&self) -> &ffi::PendingTransaction {
        unsafe {
            self.0
                .as_ref()
                .expect("pending transaction pointer not to be null")
        }
    }
}

unsafe impl Send for PendingTransaction {}
