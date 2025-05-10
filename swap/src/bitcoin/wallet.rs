use crate::bitcoin::{Address, Amount, Transaction};
use crate::cli::api::tauri_bindings::{
    TauriBackgroundProgress, TauriBitcoinFullScanProgress, TauriBitcoinSyncProgress, TauriEmitter,
    TauriHandle,
};
use crate::seed::Seed;
use anyhow::{anyhow, bail, Context, Result};
use bdk_electrum::electrum_client::{ElectrumApi, GetHistoryRes};
use bdk_electrum::BdkElectrumClient;
use bdk_wallet::bitcoin::FeeRate;
use bdk_wallet::bitcoin::Network;
use bdk_wallet::export::FullyNodedExport;
use bdk_wallet::psbt::PsbtUtils;
use bdk_wallet::rusqlite::Connection;
use bdk_wallet::template::{Bip84, DescriptorTemplate};
use bdk_wallet::KeychainKind;
use bdk_wallet::SignOptions;
use bdk_wallet::WalletPersister;
use bdk_wallet::{Balance, PersistedWallet};
use bitcoin::bip32::Xpriv;
use bitcoin::ScriptBuf;
use bitcoin::{psbt::Psbt as PartiallySignedTransaction, Txid};
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fmt;
use std::fmt::Debug;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::{watch, Mutex};
use tracing::{debug_span, Instrument};

use super::bitcoin_address::revalidate_network;
use super::BlockHeight;
use derive_builder::Builder;

/// Assuming we add a spread of 3% we don't want to pay more than 3% of the
/// amount for tx fees.
const MAX_RELATIVE_TX_FEE: Decimal = dec!(0.03);
const MAX_ABSOLUTE_TX_FEE: Decimal = dec!(100_000);
const DUST_AMOUNT: Amount = Amount::from_sat(546);

/// Configuration for how the wallet should be persisted.
#[derive(Debug, Clone)]
pub enum PersisterConfig {
    SqliteFile { data_dir: PathBuf },
    InMemorySqlite,
}

/// Holds the configuration parameters for creating a Bitcoin wallet.
/// The actual Wallet<Connection> will be constructed from this configuration.
#[derive(Builder, Clone)]
#[builder(
    name = "WalletBuilder",
    pattern = "owned",
    setter(into, strip_option),
    build_fn(
        name = "validate_config",
        private,
        error = "derive_builder::UninitializedFieldError"
    ),
    derive(Clone)
)]
pub struct WalletConfig {
    seed: Seed,
    network: Network,
    electrum_rpc_url: String,
    persister: PersisterConfig,
    env_config: Option<crate::env::Config>,
    finality_confirmations: u32,
    target_block: usize,
    sync_interval: Duration,
    #[builder(default)]
    tauri_handle: Option<TauriHandle>,
}

impl WalletBuilder {
    /// Asynchronously builds the `Wallet<Connection>` using the configured parameters.
    /// This method contains the core logic for wallet initialization, including
    /// database setup, key derivation, and potential migration from older wallet formats.
    pub async fn build(self) -> Result<Wallet<Connection>> {
        let config = self
            .validate_config()
            .map_err(|e| anyhow!("Builder validation failed: {e}"))?;

        let client = Client::new(&config.electrum_rpc_url, config.sync_interval)
            .context("Failed to create Electrum client")?;

        match &config.persister {
            PersisterConfig::SqliteFile { data_dir } => {
                let env_config = config
                    .env_config
                    .context("env_config is required for file-based SQLite wallet")?;

                let xpriv_derivation_network = env_config.bitcoin_network;
                let xprivkey = config
                    .seed
                    .derive_extended_private_key(xpriv_derivation_network)
                    .context("Failed to derive extended private key for file wallet")?;

                let wallet_parent_dir = data_dir.join(Wallet::<Connection>::WALLET_PARENT_DIR_NAME);
                let wallet_dir = wallet_parent_dir.join(Wallet::<Connection>::WALLET_DIR_NAME);
                let wallet_path = wallet_dir.join(Wallet::<Connection>::WALLET_FILE_NAME);
                let wallet_exists = wallet_path.exists();

                tokio::fs::create_dir_all(&wallet_dir)
                    .await
                    .context("Failed to create wallet directory")?;

                let connection = Connection::open(&wallet_path).context(format!(
                    "Failed to open SQLite database at {:?}",
                    wallet_path
                ))?;

                if wallet_exists {
                    Wallet::create_existing(
                        xprivkey,
                        config.network,
                        client,
                        connection,
                        config.finality_confirmations,
                        config.target_block,
                        config.tauri_handle.clone(),
                    )
                    .await
                    .context("Failed to load existing wallet")
                } else {
                    let old_wallet_export = Wallet::<Connection>::get_pre_1_0_0_bdk_wallet_export(
                        data_dir,
                        config.network,
                        &config.seed,
                        env_config,
                    )
                    .await
                    .context("Failed to get pre-1.0.0 BDK wallet export for migration")?;

                    Wallet::create_new(
                        xprivkey,
                        config.network,
                        client,
                        connection,
                        config.finality_confirmations,
                        config.target_block,
                        old_wallet_export,
                        config.tauri_handle.clone(),
                    )
                    .await
                    .context("Failed to create new wallet")
                }
            }
            PersisterConfig::InMemorySqlite => {
                let xprivkey = config
                    .seed
                    .derive_extended_private_key(config.network)
                    .context("Failed to derive extended private key for in-memory wallet")?;

                let persister = Connection::open_in_memory()
                    .context("Failed to open in-memory SQLite database")?;

                Wallet::create_new(
                    xprivkey,
                    config.network,
                    client,
                    persister,
                    config.finality_confirmations,
                    config.target_block,
                    None,
                    config.tauri_handle.clone(),
                )
                .await
                .context("Failed to create new in-memory wallet")
            }
        }
    }
}

/// This is our wrapper around a bdk wallet and a corresponding
/// bdk electrum client.
/// It unifies all the functionality we need when interacting
/// with the bitcoin network.
///
/// This wallet is generic over the persister, which may be a
/// rusqlite connection, or an in-memory database, or something else.
#[derive(Clone)]
pub struct Wallet<Persister = Connection, C = Client> {
    /// The wallet, which is persisted to the disk.
    wallet: Arc<Mutex<PersistedWallet<Persister>>>,
    /// The database connection used to persist the wallet.
    persister: Arc<Mutex<Persister>>,
    /// The electrum client.
    client: Arc<Mutex<C>>,
    /// The network this wallet is on.
    network: Network,
    /// The number of confirmations (blocks) we require for a transaction
    /// to be considered final.
    ///
    /// Usually set to 1.
    finality_confirmations: u32,
    /// We want our transactions to be confirmed after this many blocks
    /// (used for fee estimation).
    target_block: usize,
    /// The Tauri handle
    tauri_handle: Option<TauriHandle>,
}

/// This is our wrapper around a bdk electrum client.
pub struct Client {
    /// The underlying bdk electrum client.
    electrum: BdkElectrumClient<bdk_electrum::electrum_client::Client>,
    /// The history of transactions for each script.
    script_history: BTreeMap<ScriptBuf, Vec<GetHistoryRes>>,
    /// The subscriptions to the status of transactions.
    subscriptions: HashMap<(Txid, ScriptBuf), Subscription>,
    /// The time of the last sync.
    last_sync: Instant,
    /// How often we sync with the server.
    sync_interval: Duration,
    /// The height of the latest block we know about.
    latest_block_height: BlockHeight,
}

/// A subscription to the status of a given transaction
/// that can be used to wait for the transaction to be confirmed.
#[derive(Debug, Clone)]
pub struct Subscription {
    /// A receiver used to await updates to the status of the transaction.
    receiver: watch::Receiver<ScriptStatus>,
    /// The number of confirmations we require for a transaction to be considered final.
    finality_confirmations: u32,
    /// The transaction ID we are subscribing to.
    txid: Txid,
}

/// The possible statuses of a script.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ScriptStatus {
    Unseen,
    InMempool,
    Confirmed(Confirmed),
    Retrying,
}

/// The status of a confirmed transaction.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Confirmed {
    /// The depth of this transaction within the blockchain.
    ///
    /// Zero if the transaction is included in the latest block.
    depth: u32,
}

/// Defines a watchable transaction.
///
/// For a transaction to be watchable, we need to know two things: Its
/// transaction ID and the specific output script that is going to change.
/// A transaction can obviously have multiple outputs but our protocol purposes,
/// we are usually interested in a specific one.
pub trait Watchable {
    /// The transaction ID.
    fn id(&self) -> Txid;
    /// The script of the output we are interested in.
    fn script(&self) -> ScriptBuf;
    /// Convenience method to get both the script and the txid.
    fn script_and_txid(&self) -> (ScriptBuf, Txid) {
        (self.script(), self.id())
    }
}

/// An object that can estimate fee rates and minimum relay fees.
pub trait EstimateFeeRate {
    /// Estimate the fee rate for a given target block.
    fn estimate_feerate(&self, target_block: usize) -> Result<FeeRate>;
    /// Get the minimum relay fee.
    fn min_relay_fee(&self) -> Result<bitcoin::Amount>;
}

impl Wallet {
    /// If this many consequent addresses are unused, we stop the full scan.
    /// This needs to be a very big number, because we generate a lot of addresses
    /// which might end up unused.
    const SCAN_STOP_GAP: usize = 1_000;
    /// The batch size for the full scan.
    const SCAN_BATCH_SIZE: usize = 32;

    const WALLET_PARENT_DIR_NAME: &str = "wallet";
    const WALLET_DIR_NAME: &str = "wallet-new";
    const WALLET_FILE_NAME: &str = "walletdb.sqlite";

    async fn get_pre_1_0_0_bdk_wallet_export(
        data_dir: impl AsRef<Path>,
        network: Network,
        seed: &Seed,
        env_config: crate::env::Config,
    ) -> Result<Option<pre_1_0_0_bdk::Export>> {
        // Construct the directory in which the old (<1.0.0 bdk) wallet was stored
        let wallet_parent_dir = data_dir.as_ref().join(Self::WALLET_PARENT_DIR_NAME);
        let pre_bdk_1_0_0_wallet_dir = wallet_parent_dir.join(pre_1_0_0_bdk::WALLET);
        let pre_bdk_1_0_0_wallet_exists = pre_bdk_1_0_0_wallet_dir.exists();

        if pre_bdk_1_0_0_wallet_exists {
            tracing::info!("Found old Bitcoin wallet (pre 1.0.0 bdk). Migrating...");

            // We need to support the legacy wallet format for the migration path.
            // We need to convert the network to the legacy BDK network type.
            let legacy_network = match network {
                Network::Bitcoin => bdk::bitcoin::Network::Bitcoin,
                Network::Testnet => bdk::bitcoin::Network::Testnet,
                _ => bail!("Unsupported network: {}", network),
            };

            let xprivkey = seed.derive_extended_private_key_legacy(legacy_network)?;
            let old_wallet =
                pre_1_0_0_bdk::OldWallet::new(&pre_bdk_1_0_0_wallet_dir, xprivkey, env_config)
                    .await?;

            let export = old_wallet.export("old-wallet").await?;

            tracing::debug!(
                external_index=%export.external_derivation_index,
                internal_index=%export.internal_derivation_index,
                "Constructed export of old Bitcoin wallet (pre 1.0.0 bdk) for migration"
            );

            Ok(Some(export))
        } else {
            Ok(None)
        }
    }

    /// Create a new wallet, persisted to a sqlite database.
    /// This is a private API so we allow too many arguments.
    #[allow(clippy::too_many_arguments)]
    pub async fn with_sqlite(
        seed: &Seed,
        network: Network,
        electrum_rpc_url: &str,
        data_dir: impl AsRef<Path>,
        finality_confirmations: u32,
        target_block: usize,
        sync_interval: Duration,
        env_config: crate::env::Config,
        tauri_handle: Option<TauriHandle>,
    ) -> Result<Wallet<bdk_wallet::rusqlite::Connection>> {
        // Construct the private key, directory and wallet file for the new (>= 1.0.0) bdk wallet
        let xprivkey = seed.derive_extended_private_key(env_config.bitcoin_network)?;
        let wallet_dir = data_dir
            .as_ref()
            .join(Self::WALLET_PARENT_DIR_NAME)
            .join(Self::WALLET_DIR_NAME);
        let wallet_path = wallet_dir.join(Self::WALLET_FILE_NAME);
        let wallet_exists = wallet_path.exists();

        // Connect to the electrum server.
        let client = Client::new(electrum_rpc_url, sync_interval)?;

        // Make sure the wallet directory exists.
        tokio::fs::create_dir_all(&wallet_dir).await?;

        let connection = Connection::open(&wallet_path)?;

        // If the new Bitcoin wallet (> 1.0.0 bdk) already exists, we open it
        if wallet_exists {
            Self::create_existing(
                xprivkey,
                network,
                client,
                connection,
                finality_confirmations,
                target_block,
                tauri_handle,
            )
            .await
        } else {
            // If the new Bitcoin wallet (> 1.0.0 bdk) does not yet exist:
            // We check if we have an old (< 1.0.0 bdk) wallet. If so, we migrate.
            let export =
                Self::get_pre_1_0_0_bdk_wallet_export(data_dir, network, seed, env_config).await?;

            Self::create_new(
                xprivkey,
                network,
                client,
                connection,
                finality_confirmations,
                target_block,
                export,
                tauri_handle,
            )
            .await
        }
    }

    /// Create a new wallet, persisted to an in-memory sqlite database.
    /// Should only be used for testing.
    #[cfg(test)]
    pub async fn with_sqlite_in_memory(
        seed: &Seed,
        network: Network,
        electrum_rpc_url: &str,
        finality_confirmations: u32,
        target_block: usize,
        sync_interval: Duration,
        tauri_handle: Option<TauriHandle>,
    ) -> Result<Wallet<bdk_wallet::rusqlite::Connection>> {
        Self::create_new(
            seed.derive_extended_private_key(network)?,
            network,
            Client::new(electrum_rpc_url, sync_interval).expect("Failed to create electrum client"),
            bdk_wallet::rusqlite::Connection::open_in_memory()?,
            finality_confirmations,
            target_block,
            None,
            tauri_handle,
        )
        .await
    }

    /// Create a new wallet in the database and perform a full scan.
    /// This is a private API so we allow too many arguments.
    #[allow(clippy::too_many_arguments)]
    async fn create_new<Persister>(
        xprivkey: Xpriv,
        network: Network,
        client: Client,
        mut persister: Persister,
        finality_confirmations: u32,
        target_block: usize,
        old_wallet: Option<pre_1_0_0_bdk::Export>,
        tauri_handle: Option<TauriHandle>,
    ) -> Result<Wallet<Persister>>
    where
        Persister: WalletPersister + Sized,
        <Persister as WalletPersister>::Error: std::error::Error + Send + Sync + 'static,
    {
        let external_descriptor = Bip84(xprivkey, KeychainKind::External)
            .build(network)
            .context("Failed to build external wallet descriptor")?;

        let internal_descriptor = Bip84(xprivkey, KeychainKind::Internal)
            .build(network)
            .context("Failed to build change wallet descriptor")?;

        let mut wallet = bdk_wallet::Wallet::create(external_descriptor, internal_descriptor)
            .network(network)
            .create_wallet(&mut persister)
            .context("Failed to create wallet")?;

        // If we have an old wallet, we need to reveal the addresses that were used before
        // to speed up the initial sync.
        if let Some(old_wallet) = old_wallet {
            tracing::info!("Migrating from old Bitcoin wallet (< 1.0.0 bdk)");

            let _ = wallet
                .reveal_addresses_to(KeychainKind::External, old_wallet.external_derivation_index);
            let _ = wallet
                .reveal_addresses_to(KeychainKind::Internal, old_wallet.internal_derivation_index);

            wallet.persist(&mut persister)?;
        }

        tracing::debug!("Starting initial Bitcoin wallet scan");

        // Send progress updates to the UI
        let progress_handle = tauri_handle.new_background_process_with_initial_progress(
            TauriBackgroundProgress::FullScanningBitcoinWallet,
            TauriBitcoinFullScanProgress::Unknown,
        );

        let progress_handle_clone = progress_handle.clone();
        let full_scan = wallet.start_full_scan().inspect(move |_, curr_index, _| {
            tracing::debug!(
                "Full scanning Bitcoin wallet, current at index {}",
                curr_index
            );
            progress_handle_clone.update(TauriBitcoinFullScanProgress::Known {
                current_index: curr_index as u64,
            });
        });

        let full_scan_result = client.electrum.full_scan(
            full_scan,
            Self::SCAN_STOP_GAP,
            Self::SCAN_BATCH_SIZE,
            true,
        )?;

        wallet.apply_update(full_scan_result)?;
        wallet.persist(&mut persister)?;

        progress_handle.finish();

        tracing::debug!("Initial Bitcoin wallet scan completed");

        Ok(Wallet {
            wallet: Arc::new(Mutex::new(wallet)),
            client: Arc::new(Mutex::new(client)),
            network,
            finality_confirmations,
            target_block,
            persister: Arc::new(Mutex::new(persister)),
            tauri_handle,
        })
    }

    /// Load existing wallet data from the database
    async fn create_existing<Persister>(
        xprivkey: Xpriv,
        network: Network,
        client: Client,
        mut persister: Persister,
        finality_confirmations: u32,
        target_block: usize,
        tauri_handle: Option<TauriHandle>,
    ) -> Result<Wallet<Persister>>
    where
        Persister: WalletPersister + Sized,
        <Persister as WalletPersister>::Error: std::error::Error + Send + Sync + 'static,
    {
        let external_descriptor = Bip84(xprivkey, KeychainKind::External)
            .build(network)
            .context("Failed to build external wallet descriptor")?;

        let internal_descriptor = Bip84(xprivkey, KeychainKind::Internal)
            .build(network)
            .context("Failed to build change wallet descriptor")?;

        tracing::debug!("Loading Bitcoin wallet from database");

        let wallet = bdk_wallet::Wallet::load()
            .descriptor(KeychainKind::External, Some(external_descriptor))
            .descriptor(KeychainKind::Internal, Some(internal_descriptor))
            .extract_keys()
            .load_wallet(&mut persister)
            .context("Failed to open database")?
            .context("No wallet found in database")?;

        let wallet = Wallet {
            wallet: Arc::new(Mutex::new(wallet)),
            client: Arc::new(Mutex::new(client)),
            network,
            finality_confirmations,
            target_block,
            persister: Arc::new(Mutex::new(persister)),
            tauri_handle,
        };

        Ok(wallet)
    }

    /// Broadcast the given transaction to the network and emit a tracing statement
    /// if done so successfully.
    ///
    /// Returns the transaction ID and a future for when the transaction meets
    /// the configured finality confirmations.
    pub async fn broadcast(
        &self,
        transaction: Transaction,
        kind: &str,
    ) -> Result<(Txid, Subscription)> {
        let txid = transaction.compute_txid();

        // to watch for confirmations, watching a single output is enough
        let subscription = self
            .subscribe_to((txid, transaction.output[0].script_pubkey.clone()))
            .await;

        let client = self.client.lock().await;
        client
            .transaction_broadcast(&transaction)
            .with_context(|| {
                format!("Failed to broadcast Bitcoin {} transaction {}", kind, txid)
            })?;

        tracing::info!(%txid, %kind, "Published Bitcoin transaction");

        Ok((txid, subscription))
    }

    pub async fn get_raw_transaction(&self, txid: Txid) -> Result<Arc<Transaction>> {
        self.get_tx(txid)
            .await
            .with_context(|| format!("Could not get raw tx with id: {}", txid))
    }

    pub async fn status_of_script<T>(&self, tx: &T) -> Result<ScriptStatus>
    where
        T: Watchable,
    {
        self.client.lock().await.status_of_script(tx)
    }

    pub async fn subscribe_to(&self, tx: impl Watchable + Send + 'static) -> Subscription {
        let txid = tx.id();
        let script = tx.script();

        let sub = self
            .client
            .lock()
            .await
            .subscriptions
            .entry((txid, script.clone()))
            .or_insert_with(|| {
                let (sender, receiver) = watch::channel(ScriptStatus::Unseen);
                let client = self.client.clone();

                tokio::spawn(async move {
                    let mut last_status = None;

                    loop {
                        let new_status = client.lock()
                            .await
                            .status_of_script(&tx)
                            .unwrap_or_else(|error| {
                                tracing::warn!(%txid, "Failed to get status of script: {:#}", error);
                                ScriptStatus::Retrying
                            });

                        if new_status != ScriptStatus::Retrying
                        {
                            last_status = Some(debug_status_change(txid, last_status, new_status));

                            let all_receivers_gone = sender.send(new_status).is_err();

                            if all_receivers_gone {
                                tracing::debug!(%txid, "All receivers gone, removing subscription");
                                client.lock().await.subscriptions.remove(&(txid, script));
                                return;
                            }
                        }

                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                }.instrument(debug_span!("BitcoinWalletSubscription")));

                Subscription {
                    receiver,
                    finality_confirmations: self.finality_confirmations,
                    txid,
                }
            })
            .clone();

        sub
    }

    pub async fn wallet_export(&self, role: &str) -> Result<FullyNodedExport> {
        let wallet = self.wallet.lock().await;
        match bdk_wallet::export::FullyNodedExport::export_wallet(
            &wallet,
            &format!("{}-{}", role, self.network),
            true,
        ) {
            Result::Ok(wallet_export) => Ok(wallet_export),
            Err(err_msg) => Err(anyhow::Error::msg(err_msg)),
        }
    }

    /// Get a transaction from the Electrum server or the cache.
    pub async fn get_tx(&self, txid: Txid) -> Result<Arc<Transaction>> {
        let client = self.client.lock().await;
        let tx = client
            .get_tx(txid)
            .context("Failed to get transaction from cache or Electrum server")?;

        Ok(tx)
    }

    /// Sync the wallet with the blockchain, optionally calling a callback on progress updates.
    /// This will NOT emit progress events to the UI.
    pub async fn sync_with_custom_callback<F>(&self, mut callback: Option<F>) -> Result<()>
    where
        F: FnMut(usize, usize) + Send + 'static,
    {
        let sync_request = self
            .wallet
            .lock()
            .await
            .start_sync_with_revealed_spks()
            .inspect(move |_, progress| {
                if let Some(cb) = callback.as_mut() {
                    cb(progress.consumed(), progress.total());
                }

                tracing::debug!(
                    "Syncing wallet ({:?} / {:?} done)",
                    progress.consumed(),
                    progress.total()
                );
            })
            .build();

        // We spawn a blocking task to sync the wallet
        // because the sync method is blocking.
        let client_mutex = self.client.clone();
        let res = tokio::task::spawn_blocking(move || {
            let client = client_mutex.blocking_lock();
            client
                .electrum
                .sync(sync_request, Self::SCAN_BATCH_SIZE, true)
        })
        .await??;

        let mut wallet = self.wallet.lock().await;
        wallet.apply_update(res)?;

        let mut persister = self.persister.lock().await;
        wallet.persist(&mut persister)?;

        Ok(())
    }

    /// Sync the wallet with the blockchain
    /// and emit progress events to the UI
    pub async fn sync(&self) -> Result<()> {
        let background_process_handle = self
            .tauri_handle
            .new_background_process_with_initial_progress(
                TauriBackgroundProgress::SyncingBitcoinWallet,
                TauriBitcoinSyncProgress::Unknown,
            );

        let background_process_handle_clone = background_process_handle.clone();
        self.sync_with_custom_callback::<Box<dyn Fn(usize, usize) + Send>>(Some(Box::new(
            move |consumed, total| {
                background_process_handle_clone.update(TauriBitcoinSyncProgress::Known {
                    consumed: consumed as u64,
                    total: total as u64,
                });
            },
        )))
        .await?;

        background_process_handle.finish();

        Ok(())
    }

    /// Calculate the fee for a given transaction.
    ///
    /// Will fail if the transaction inputs are not owned by this wallet.
    pub async fn transaction_fee(&self, txid: Txid) -> Result<Amount> {
        let transaction = self
            .get_tx(txid)
            .await
            .context("Could not find tx in bdk wallet when trying to determine fees")?;
        let fee = self.wallet.lock().await.calculate_fee(&transaction)?;

        Ok(fee)
    }
}

// These are the methods that are always available, regardless of the persister.
impl<T, C> Wallet<T, C> {
    /// Get the network of this wallet.
    pub fn network(&self) -> Network {
        self.network
    }

    /// Get the finality confirmations of this wallet.
    pub fn finality_confirmations(&self) -> u32 {
        self.finality_confirmations
    }

    /// Get the target block of this wallet.
    ///
    /// This is the the number of blocks we want to wait at most for
    /// one ofour transaction to be confirmed.
    pub fn target_block(&self) -> usize {
        self.target_block
    }
}

impl<Persister, C> Wallet<Persister, C>
where
    Persister: WalletPersister + Sized,
    <Persister as WalletPersister>::Error: std::error::Error + Send + Sync + 'static,
    C: EstimateFeeRate + Send + Sync + 'static,
{
    pub async fn sign_and_finalize(&self, mut psbt: bitcoin::psbt::Psbt) -> Result<Transaction> {
        // Acquire the wallet lock once here for efficiency within the non-finalized block
        let wallet_guard = self.wallet.lock().await;

        let finalized = wallet_guard.sign(&mut psbt, SignOptions::default())?;

        if !finalized {
            bail!("PSBT is not finalized")
        }

        // Release the lock if finalization succeeded
        drop(wallet_guard);

        let tx = psbt.extract_tx();
        Ok(tx?)
    }

    /// Returns the total Bitcoin balance, which includes pending funds
    pub async fn balance(&self) -> Result<Amount> {
        Ok(self.wallet.lock().await.balance().total())
    }

    /// Returns the balance info of the wallet, including unconfirmed funds etc.
    pub async fn balance_info(&self) -> Result<Balance> {
        Ok(self.wallet.lock().await.balance())
    }

    /// Reveals the next address from the wallet.
    pub async fn new_address(&self) -> Result<Address> {
        let mut wallet = self.wallet.lock().await;

        // Only reveal a new address if absolutely necessary
        // We want to avoid revealing more and more addresses
        let address = wallet.next_unused_address(KeychainKind::External).address;

        // Important: persist that we revealed a new address.
        // Otherwise the wallet might reuse it (bad).
        let mut persister = self.persister.lock().await;
        wallet.persist(&mut persister)?;

        Ok(address)
    }

    /// Builds a partially signed transaction
    ///
    /// Ensures that the address script is at output index `0`
    /// for the partially signed transaction.
    pub async fn send_to_address(
        &self,
        address: Address,
        amount: Amount,
        change_override: Option<Address>,
    ) -> Result<PartiallySignedTransaction> {
        // Check address and change address for network equality.
        let address = revalidate_network(address, self.network)?;

        change_override
            .as_ref()
            .map(|a| revalidate_network(a.clone(), self.network))
            .transpose()
            .context("Change address is not on the correct network")?;

        let mut wallet = self.wallet.lock().await;
        let client = self.client.lock().await;
        let fee_rate = client.estimate_feerate(self.target_block)?;
        let script = address.script_pubkey();

        // Build the transaction.
        let mut tx_builder = wallet.build_tx();
        tx_builder.add_recipient(script.clone(), amount);
        tx_builder.fee_rate(fee_rate);
        let mut psbt = tx_builder.finish()?;

        match psbt.unsigned_tx.output.as_mut_slice() {
            // our primary output is the 2nd one? reverse the vectors
            [_, second_txout] if second_txout.script_pubkey == script => {
                psbt.outputs.reverse();
                psbt.unsigned_tx.output.reverse();
            }
            [first_txout, _] if first_txout.script_pubkey == script => {
                // no need to do anything
            }
            [_] => {
                // single output, no need do anything
            }
            _ => bail!("Unexpected transaction layout"),
        }

        if let ([_, change], [_, psbt_output], Some(change_override)) = (
            &mut psbt.unsigned_tx.output.as_mut_slice(),
            &mut psbt.outputs.as_mut_slice(),
            change_override,
        ) {
            tracing::info!(change_override = ?change_override, "Overwriting change address");
            change.script_pubkey = change_override.script_pubkey();
            // Might be populated based on the previously set change address, but for the
            // overwrite we don't know unless we ask the user for more information.
            psbt_output.bip32_derivation.clear();
        }

        Ok(psbt)
    }

    /// Calculates the maximum "giveable" amount of this wallet.
    ///
    /// We define this as the maximum amount we can pay to a single output,
    /// already accounting for the fees we need to spend to get the
    /// transaction confirmed.
    pub async fn max_giveable(&self, locking_script_size: usize) -> Result<Amount> {
        tracing::debug!(locking_script_size, "Calculating max giveable");

        let mut wallet = self.wallet.lock().await;
        let balance = wallet.balance();
        if balance.total() < DUST_AMOUNT {
            return Ok(Amount::ZERO);
        }
        let client = self.client.lock().await;
        let min_relay_fee = client.min_relay_fee()?;

        if balance.total() < min_relay_fee {
            return Ok(Amount::ZERO);
        }

        let fee_rate = client.estimate_feerate(self.target_block)?;

        let mut tx_builder = wallet.build_tx();

        let dummy_script = ScriptBuf::from(vec![0u8; locking_script_size]);
        tx_builder.drain_to(dummy_script);
        tx_builder.fee_rate(fee_rate);
        tx_builder.drain_wallet();

        let psbt = tx_builder
            .finish()
            .context("Failed to build transaction to figure out max giveable")?;

        let max_giveable = psbt
            .unsigned_tx
            .output
            .iter()
            .map(|o| o.value)
            .sum::<Amount>();

        tracing::debug!(fee=?psbt.fee_amount().map(|a| a.to_sat()), "Calculated max giveable");

        Ok(max_giveable)
    }

    /// Estimate total tx fee for a pre-defined target block based on the
    /// transaction weight. The max fee cannot be more than MAX_PERCENTAGE_FEE
    /// of amount
    pub async fn estimate_fee(
        &self,
        weight: usize,
        transfer_amount: bitcoin::Amount,
    ) -> Result<bitcoin::Amount> {
        let client = self.client.lock().await;
        let fee_rate = client.estimate_feerate(self.target_block)?;
        let min_relay_fee = client.min_relay_fee()?;

        estimate_fee(weight, transfer_amount, fee_rate, min_relay_fee)
    }
}

impl Client {
    /// Create a new client to this electrum server.
    pub fn new(electrum_rpc_url: &str, sync_interval: Duration) -> Result<Self> {
        let client = bdk_electrum::electrum_client::Client::new(electrum_rpc_url)?;
        Ok(Self {
            electrum: BdkElectrumClient::new(client),
            script_history: Default::default(),
            last_sync: Instant::now()
                .checked_sub(sync_interval)
                .ok_or(anyhow!("failed to set last sync time"))?,
            sync_interval,
            latest_block_height: BlockHeight::from(0),
            subscriptions: Default::default(),
        })
    }

    /// Update the client state, if the refresh duration has passed.
    ///
    /// Optionally force an update even if the sync interval has not passed.
    pub fn update_state(&mut self, force: bool) -> Result<()> {
        let now = Instant::now();

        if !force && now.duration_since(self.last_sync) < self.sync_interval {
            return Ok(());
        }

        self.last_sync = now;
        self.update_script_histories()?;
        self.update_block_height()?;

        Ok(())
    }

    /// Update the block height.
    fn update_block_height(&mut self) -> Result<()> {
        let latest_block = self
            .electrum
            .inner
            .block_headers_subscribe()
            .context("Failed to subscribe to header notifications")?;
        let latest_block_height = BlockHeight::try_from(latest_block)?;

        if latest_block_height > self.latest_block_height {
            tracing::trace!(
                block_height = u32::from(latest_block_height),
                "Got notification for new block"
            );
            self.latest_block_height = latest_block_height;
        }

        Ok(())
    }

    /// Update the script histories.
    fn update_script_histories(&mut self) -> Result<()> {
        let scripts = self.script_history.keys().map(|s| s.as_script());

        let histories = self
            .electrum
            .inner
            .batch_script_get_history(scripts)
            .context("Failed to fetch script histories")?;

        if histories.len() != self.script_history.len() {
            bail!(
                "Expected {} script histories, got {}",
                self.script_history.len(),
                histories.len()
            );
        }

        let scripts = self.script_history.keys().cloned();
        self.script_history = scripts.zip(histories).collect();

        Ok(())
    }

    /// Broadcast a transaction to the network.
    pub fn transaction_broadcast(&self, transaction: &Transaction) -> Result<Arc<Txid>> {
        // Broadcast the transaction to the network.
        let res = self
            .electrum
            .transaction_broadcast(transaction)
            .context("Failed to broadcast transaction")?;

        // Add the transaction to the cache.
        self.electrum.populate_tx_cache(vec![transaction.clone()]);

        Ok(Arc::new(res))
    }

    /// Get the status of a script.
    pub fn status_of_script(&mut self, script: &impl Watchable) -> Result<ScriptStatus> {
        let (script, txid) = script.script_and_txid();

        if !self.script_history.contains_key(&script) {
            self.script_history.insert(script.clone(), vec![]);

            // Immediately refetch the status of the script
            // when we first subscribe to it.
            self.update_state(true)?;
        } else {
            // Otherwise, don't force a refetch.
            self.update_state(false)?;
        }

        let history = self.script_history.entry(script).or_default();

        let history_of_tx: Vec<&GetHistoryRes> = history
            .iter()
            .filter(|entry| entry.tx_hash == txid)
            .collect();

        // Destructure history_of_tx into the last entry and the rest.
        let [rest @ .., last] = history_of_tx.as_slice() else {
            // If there is no history of the transaction, it is unseen.
            return Ok(ScriptStatus::Unseen);
        };

        // There should only be one entry per txid, we will ignore the rest
        if !rest.is_empty() {
            tracing::warn!(%txid, "Found multiple history entries for the same txid. Ignoring all but the last one.");
        }

        match last.height {
            // If the height is 0 or less, the transaction is still in the mempool.
            ..=0 => Ok(ScriptStatus::InMempool),
            // Otherwise, the transaction has been included in a block.
            height => Ok(ScriptStatus::Confirmed(
                Confirmed::from_inclusion_and_latest_block(
                    u32::try_from(height)?,
                    u32::from(self.latest_block_height),
                ),
            )),
        }
    }

    /// Get a transaction from the Electrum server.
    /// Fails if the transaction is not found.
    pub fn get_tx(&self, txid: Txid) -> Result<Arc<Transaction>> {
        self.electrum
            .fetch_tx(txid)
            .context("Failed to get transaction from the Electrum server")
    }
}

impl EstimateFeeRate for Client {
    fn estimate_feerate(&self, target_block: usize) -> Result<FeeRate> {
        // Get the fee rate in BTC/kvB
        let btc_per_kvb = self.electrum.inner.estimate_fee(target_block)?;
        let amount_per_kvb = Amount::from_btc(btc_per_kvb)?;
        // Convert to sat/kwu
        let amount_per_kwu = amount_per_kvb.checked_div(4).context("fee rate overflow")?;

        Ok(FeeRate::from_sat_per_kwu(amount_per_kwu.to_sat()))
    }

    fn min_relay_fee(&self) -> Result<bitcoin::Amount> {
        let relay_fee_btc = self.electrum.inner.relay_fee()?;

        Amount::from_btc(relay_fee_btc).context("relay fee out of range")
    }
}

fn debug_status_change(txid: Txid, old: Option<ScriptStatus>, new: ScriptStatus) -> ScriptStatus {
    match (old, new) {
        (None, new_status) => {
            tracing::debug!(%txid, status = %new_status, "Found relevant Bitcoin transaction");
        }
        (Some(old_status), new_status) if old_status != new_status => {
            tracing::debug!(%txid, %new_status, %old_status, "Bitcoin transaction status changed");
        }
        _ => {}
    }

    new
}

impl Subscription {
    pub async fn wait_until_final(&self) -> Result<()> {
        let conf_target = self.finality_confirmations;
        let txid = self.txid;

        tracing::info!(%txid, required_confirmation=%conf_target, "Waiting for Bitcoin transaction finality");

        let mut seen_confirmations = 0;

        self.wait_until(|status| match status {
            ScriptStatus::Confirmed(inner) => {
                let confirmations = inner.confirmations();

                if confirmations > seen_confirmations {
                    tracing::info!(%txid,
                        seen_confirmations = %confirmations,
                        needed_confirmations = %conf_target,
                        "Waiting for Bitcoin transaction finality");
                    seen_confirmations = confirmations;
                }

                inner.meets_target(conf_target)
            }
            _ => false,
        })
        .await
    }

    pub async fn wait_until_seen(&self) -> Result<()> {
        self.wait_until(ScriptStatus::has_been_seen).await
    }

    pub async fn wait_until_confirmed_with<T>(&self, target: T) -> Result<()>
    where
        T: Into<u32>,
        T: Copy,
    {
        self.wait_until(|status| status.is_confirmed_with(target))
            .await
    }

    pub async fn wait_until(&self, mut predicate: impl FnMut(&ScriptStatus) -> bool) -> Result<()> {
        let mut receiver = self.receiver.clone();

        while !predicate(&receiver.borrow()) {
            receiver
                .changed()
                .await
                .context("Failed while waiting for next status update")?;
        }

        Ok(())
    }
}

pub mod pre_1_0_0_bdk {
    //! This module contains some code for creating a bdk wallet from before the update.
    //! We need to keep this around to be able to migrate the wallet.

    use std::path::Path;
    use std::sync::Arc;

    use anyhow::{anyhow, bail, Result};
    use bdk::bitcoin::{util::bip32::ExtendedPrivKey, Network};
    use bdk::sled::Tree;
    use bdk::KeychainKind;
    use tokio::sync::Mutex;

    use crate::env;

    pub const WALLET: &str = "wallet";
    const SLED_TREE_NAME: &str = "default_tree";

    /// The is the old bdk wallet before the migration.
    /// We need to contruct it before migration to get the keys and revelation indeces.
    pub struct OldWallet<D = Tree> {
        wallet: Arc<Mutex<bdk::Wallet<D>>>,
        network: Network,
    }

    /// This is all the data we need from the old wallet to be able to migrate it
    /// and check whether we did it correctly.
    pub struct Export {
        /// Wallet descriptor and blockheight.
        pub export: bdk_wallet::export::FullyNodedExport,
        /// Index of the last external address that was revealed.
        pub external_derivation_index: u32,
        /// Index of the last internal address that was revealed.
        pub internal_derivation_index: u32,
    }

    impl OldWallet {
        /// Create a new old wallet.
        pub async fn new(
            data_dir: impl AsRef<Path>,
            xprivkey: ExtendedPrivKey,
            env_config: env::Config,
        ) -> Result<Self> {
            let data_dir = data_dir.as_ref();
            let wallet_dir = data_dir.join(WALLET);
            let database = bdk::sled::open(wallet_dir)?.open_tree(SLED_TREE_NAME)?;
            let network = env_config.bitcoin_network;

            // Convert bitcoin network to the bdk network type...
            let network = match network {
                bitcoin::Network::Bitcoin => bdk::bitcoin::Network::Bitcoin,
                bitcoin::Network::Testnet => bdk::bitcoin::Network::Testnet,
                bitcoin::Network::Regtest => bdk::bitcoin::Network::Regtest,
                bitcoin::Network::Signet => bdk::bitcoin::Network::Signet,
                _ => bail!("Unsupported network"),
            };

            let wallet = bdk::Wallet::new(
                bdk::template::Bip84(xprivkey, KeychainKind::External),
                Some(bdk::template::Bip84(xprivkey, KeychainKind::Internal)),
                network,
                database,
            )?;

            Ok(Self {
                wallet: Arc::new(Mutex::new(wallet)),
                network,
            })
        }

        /// Get a full export of the wallet including descriptors and blockheight.
        /// It also includes the internal (change) address and external (receiving) address derivation indices.
        pub async fn export(&self, role: &str) -> Result<Export> {
            let wallet = self.wallet.lock().await;
            let export = bdk::wallet::export::FullyNodedExport::export_wallet(
                &wallet,
                &format!("{}-{}", role, self.network),
                true,
            )
            .map_err(|_| anyhow!("Failed to export old wallet descriptor"))?;

            // Because we upgraded bdk, the type id changed.
            // Thus, we serialize to json and then deserialize to the new type.
            let json = serde_json::to_string(&export)?;
            let export = serde_json::from_str::<bdk_wallet::export::FullyNodedExport>(&json)?;

            let external_info = wallet.get_address(bdk::wallet::AddressIndex::LastUnused)?;
            let external_derivation_index = external_info.index;

            let internal_info =
                wallet.get_internal_address(bdk::wallet::AddressIndex::LastUnused)?;
            let internal_derivation_index = internal_info.index;

            Ok(Export {
                export,
                internal_derivation_index,
                external_derivation_index,
            })
        }
    }
}

fn estimate_fee(
    weight: usize,
    transfer_amount: Amount,
    fee_rate: FeeRate,
    min_relay_fee: Amount,
) -> Result<Amount> {
    if transfer_amount.to_sat() <= 546 {
        bail!("Amounts needs to be greater than Bitcoin dust amount.")
    }
    let fee_rate_svb = fee_rate.to_sat_per_vb_ceil();

    if fee_rate_svb > 100_000_000 || min_relay_fee.to_sat() > 100_000_000 {
        bail!("A fee_rate or min_relay_fee of > 1BTC does not make sense")
    }

    let min_relay_fee = if min_relay_fee.to_sat() == 0 {
        // if min_relay_fee is 0 we don't fail, we just set it to 1 satoshi;
        Amount::ONE_SAT
    } else {
        min_relay_fee
    };

    let weight = Decimal::from(weight);
    let weight_factor = dec!(4.0);
    let fee_rate = Decimal::from_u64(fee_rate_svb).context("Failed to parse fee rate")?;

    let sats_per_vbyte = weight / weight_factor * fee_rate;

    tracing::debug!(
        %weight,
        %fee_rate,
        %sats_per_vbyte,
        "Estimated fee for transaction",
    );

    let transfer_amount = Decimal::from(transfer_amount.to_sat());
    let max_allowed_fee = transfer_amount * MAX_RELATIVE_TX_FEE;
    let min_relay_fee = Decimal::from(min_relay_fee.to_sat());

    let recommended_fee = if sats_per_vbyte < min_relay_fee {
        tracing::warn!(
            "Estimated fee of {} is smaller than the min relay fee, defaulting to min relay fee {}",
            sats_per_vbyte,
            min_relay_fee
        );
        min_relay_fee.to_u64()
    } else if sats_per_vbyte > max_allowed_fee && sats_per_vbyte > MAX_ABSOLUTE_TX_FEE {
        tracing::warn!(
            "Hard bound of transaction fees reached. Falling back to: {} sats",
            MAX_ABSOLUTE_TX_FEE
        );
        MAX_ABSOLUTE_TX_FEE.to_u64()
    } else if sats_per_vbyte > max_allowed_fee {
        tracing::warn!(
            "Relative bound of transaction fees reached. Falling back to: {} sats",
            max_allowed_fee
        );
        max_allowed_fee.to_u64()
    } else {
        sats_per_vbyte.to_u64()
    };
    let amount = recommended_fee
        .map(bitcoin::Amount::from_sat)
        .context("Could not estimate tranasction fee.")?;

    Ok(amount)
}

impl Watchable for (Txid, ScriptBuf) {
    fn id(&self) -> Txid {
        self.0
    }

    fn script(&self) -> ScriptBuf {
        self.1.clone()
    }
}

impl ScriptStatus {
    pub fn from_confirmations(confirmations: u32) -> Self {
        match confirmations {
            0 => Self::InMempool,
            confirmations => Self::Confirmed(Confirmed::new(confirmations - 1)),
        }
    }
}

impl Confirmed {
    pub fn new(depth: u32) -> Self {
        Self { depth }
    }

    /// Compute the depth of a transaction based on its inclusion height and the
    /// latest known block.
    ///
    /// Our information about the latest block might be outdated. To avoid an
    /// overflow, we make sure the depth is 0 in case the inclusion height
    /// exceeds our latest known block,
    pub fn from_inclusion_and_latest_block(inclusion_height: u32, latest_block: u32) -> Self {
        let depth = latest_block.saturating_sub(inclusion_height);

        Self { depth }
    }

    pub fn confirmations(&self) -> u32 {
        self.depth + 1
    }

    pub fn meets_target<T>(&self, target: T) -> bool
    where
        T: Into<u32>,
    {
        self.confirmations() >= target.into()
    }

    pub fn blocks_left_until<T>(&self, target: T) -> u32
    where
        T: Into<u32> + Copy,
    {
        if self.meets_target(target) {
            0
        } else {
            target.into() - self.confirmations()
        }
    }
}

impl ScriptStatus {
    /// Check if the script has any confirmations.
    pub fn is_confirmed(&self) -> bool {
        matches!(self, ScriptStatus::Confirmed(_))
    }

    /// Check if the script has met the given confirmation target.
    pub fn is_confirmed_with<T>(&self, target: T) -> bool
    where
        T: Into<u32>,
    {
        match self {
            ScriptStatus::Confirmed(inner) => inner.meets_target(target),
            _ => false,
        }
    }

    // Calculate the number of blocks left until the target is met.
    pub fn blocks_left_until<T>(&self, target: T) -> u32
    where
        T: Into<u32> + Copy,
    {
        match self {
            ScriptStatus::Confirmed(inner) => inner.blocks_left_until(target),
            _ => target.into(),
        }
    }

    pub fn has_been_seen(&self) -> bool {
        matches!(self, ScriptStatus::InMempool | ScriptStatus::Confirmed(_))
    }
}

impl fmt::Display for ScriptStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScriptStatus::Unseen => write!(f, "unseen"),
            ScriptStatus::InMempool => write!(f, "in mempool"),
            ScriptStatus::Retrying => write!(f, "retrying"),
            ScriptStatus::Confirmed(inner) => {
                write!(f, "confirmed with {} blocks", inner.confirmations())
            }
        }
    }
}

#[cfg(test)]
pub struct StaticFeeRate {
    fee_rate: FeeRate,
    min_relay_fee: bitcoin::Amount,
}

#[cfg(test)]
impl StaticFeeRate {
    pub fn new(fee_rate: FeeRate, min_relay_fee: bitcoin::Amount) -> Self {
        Self {
            fee_rate,
            min_relay_fee,
        }
    }
}

#[cfg(test)]
impl EstimateFeeRate for StaticFeeRate {
    fn estimate_feerate(&self, _target_block: usize) -> Result<FeeRate> {
        Ok(self.fee_rate)
    }

    fn min_relay_fee(&self) -> Result<bitcoin::Amount> {
        Ok(self.min_relay_fee)
    }
}

#[cfg(test)]
#[derive(Debug)]
pub struct TestWalletBuilder {
    utxo_amount: u64,
    sats_per_vb: u64,
    min_relay_fee_sats: u64,
    key: bitcoin::bip32::Xpriv,
    num_utxos: u8,
}

#[cfg(test)]
impl TestWalletBuilder {
    /// Creates a new, funded wallet with sane default fees.
    ///
    /// Unless you are testing things related to fees, this is likely what you
    /// want.
    pub fn new(amount: u64) -> Self {
        TestWalletBuilder {
            utxo_amount: amount,
            sats_per_vb: 1,
            min_relay_fee_sats: 1000,
            key: "tprv8ZgxMBicQKsPeZRHk4rTG6orPS2CRNFX3njhUXx5vj9qGog5ZMH4uGReDWN5kCkY3jmWEtWause41CDvBRXD1shKknAMKxT99o9qUTRVC6m".parse().unwrap(),
            num_utxos: 1,
        }
    }

    pub fn with_zero_fees(self) -> Self {
        Self {
            sats_per_vb: 0,
            min_relay_fee_sats: 0,
            ..self
        }
    }

    pub fn with_fees(self, sats_per_vb: u64, min_relay_fee_sats: u64) -> Self {
        Self {
            sats_per_vb,
            min_relay_fee_sats,
            ..self
        }
    }

    pub fn with_key(self, key: bitcoin::bip32::Xpriv) -> Self {
        Self { key, ..self }
    }

    pub fn with_num_utxos(self, number: u8) -> Self {
        Self {
            num_utxos: number,
            ..self
        }
    }

    pub async fn build(self) -> Wallet<Connection, StaticFeeRate> {
        use bdk_wallet::chain::BlockId;
        use bdk_wallet::test_utils::{insert_checkpoint, receive_output_in_latest_block};

        let bdk_network = bitcoin::Network::Regtest;

        let external_descriptor = Bip84(self.key, KeychainKind::External)
            .build(bdk_network)
            .expect("Failed to build external descriptor for test wallet");
        let internal_descriptor = Bip84(self.key, KeychainKind::Internal)
            .build(bdk_network)
            .expect("Failed to build internal descriptor for test wallet");

        let mut persister = bdk_wallet::rusqlite::Connection::open_in_memory()
            .expect("Failed to open in-memory DB for test wallet");

        let bdk_core_wallet = bdk_wallet::Wallet::create(external_descriptor, internal_descriptor)
            .network(bdk_network)
            .create_wallet(&mut persister)
            .expect("Failed to create bdk_wallet::Wallet for test");

        let client = StaticFeeRate::new(
            FeeRate::from_sat_per_vb(self.sats_per_vb).unwrap(),
            bitcoin::Amount::from_sat(self.min_relay_fee_sats),
        );

        let wallet = Wallet {
            wallet: Arc::new(Mutex::new(bdk_core_wallet)), // This field was missing
            client: Arc::new(Mutex::new(client)),
            persister: Arc::new(Mutex::new(persister)), // Use the created persister
            network: Network::Regtest,
            finality_confirmations: 1,
            target_block: 1,
            tauri_handle: None,
        };

        let mut locked_wallet = wallet.wallet.try_lock().unwrap();

        // Create a block
        insert_checkpoint(
            &mut *locked_wallet,
            BlockId {
                height: 42,
                hash: <bitcoin::blockdata::block::BlockHash as bitcoin::hashes::Hash>::all_zeros(),
            },
        );

        // Fund the wallet with fake utxos
        for _ in 0..self.num_utxos {
            receive_output_in_latest_block(&mut *locked_wallet, self.utxo_amount);
        }

        // Create another block to confirm the utxos
        insert_checkpoint(
            &mut *locked_wallet,
            BlockId {
                height: 43,
                hash: <bitcoin::blockdata::block::BlockHash as bitcoin::hashes::Hash>::all_zeros(),
            },
        );

        drop(locked_wallet);

        wallet
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitcoin::{PublicKey, TxLock};
    use crate::tracing_ext::capture_logs;
    use bitcoin::address::NetworkUnchecked;
    use bitcoin::hashes::Hash;
    use proptest::prelude::*;
    use tracing::level_filters::LevelFilter;

    #[test]
    fn given_depth_0_should_meet_confirmation_target_one() {
        let script = ScriptStatus::Confirmed(Confirmed { depth: 0 });

        let confirmed = script.is_confirmed_with(1_u32);

        assert!(confirmed)
    }

    #[test]
    fn given_confirmations_1_should_meet_confirmation_target_one() {
        let script = ScriptStatus::from_confirmations(1);

        let confirmed = script.is_confirmed_with(1_u32);

        assert!(confirmed)
    }

    #[test]
    fn given_inclusion_after_lastest_known_block_at_least_depth_0() {
        let included_in = 10;
        let latest_block = 9;

        let confirmed = Confirmed::from_inclusion_and_latest_block(included_in, latest_block);

        assert_eq!(confirmed.depth, 0)
    }

    #[test]
    fn given_depth_0_should_return_0_blocks_left_until_1() {
        let script = ScriptStatus::Confirmed(Confirmed { depth: 0 });

        let blocks_left = script.blocks_left_until(1_u32);

        assert_eq!(blocks_left, 0)
    }

    #[test]
    fn given_depth_1_should_return_0_blocks_left_until_1() {
        let script = ScriptStatus::Confirmed(Confirmed { depth: 1 });

        let blocks_left = script.blocks_left_until(1_u32);

        assert_eq!(blocks_left, 0)
    }

    #[test]
    fn given_depth_0_should_return_1_blocks_left_until_2() {
        let script = ScriptStatus::Confirmed(Confirmed { depth: 0 });

        let blocks_left = script.blocks_left_until(2_u32);

        assert_eq!(blocks_left, 1)
    }

    #[test]
    fn given_one_BTC_and_100k_sats_per_vb_fees_should_not_hit_max() {
        // 400 weight = 100 vbyte
        let weight = 400;
        let amount = bitcoin::Amount::from_sat(100_000_000);

        let sat_per_vb = 100;
        let fee_rate = FeeRate::from_sat_per_vb(sat_per_vb).unwrap();

        let relay_fee = bitcoin::Amount::ONE_SAT;
        let is_fee = estimate_fee(weight, amount, fee_rate, relay_fee).unwrap();

        // weight / 4.0 *  sat_per_vb
        let should_fee = bitcoin::Amount::from_sat(10_000);
        assert_eq!(is_fee, should_fee);
    }

    #[test]
    fn given_1BTC_and_1_sat_per_vb_fees_and_100ksat_min_relay_fee_should_hit_min() {
        // 400 weight = 100 vbyte
        let weight = 400;
        let amount = bitcoin::Amount::from_sat(100_000_000);

        let sat_per_vb = 1;
        let fee_rate = FeeRate::from_sat_per_vb(sat_per_vb).unwrap();

        let relay_fee = bitcoin::Amount::from_sat(100_000);
        let is_fee = estimate_fee(weight, amount, fee_rate, relay_fee).unwrap();

        // weight / 4.0 *  sat_per_vb would be smaller than relay fee hence we take min
        // relay fee
        let should_fee = bitcoin::Amount::from_sat(100_000);
        assert_eq!(is_fee, should_fee);
    }

    #[test]
    fn given_1mio_sat_and_1k_sats_per_vb_fees_should_hit_relative_max() {
        // 400 weight = 100 vbyte
        let weight = 400;
        let amount = bitcoin::Amount::from_sat(1_000_000);

        let sat_per_vb = 1_000;
        let fee_rate = FeeRate::from_sat_per_vb(sat_per_vb).unwrap();

        let relay_fee = bitcoin::Amount::ONE_SAT;
        let is_fee = estimate_fee(weight, amount, fee_rate, relay_fee).unwrap();

        // weight / 4.0 *  sat_per_vb would be greater than 3% hence we take max
        // relative fee.
        let should_fee = bitcoin::Amount::from_sat(30_000);
        assert_eq!(is_fee, should_fee);
    }

    #[test]
    fn given_1BTC_and_4mio_sats_per_vb_fees_should_hit_total_max() {
        // even if we send 1BTC we don't want to pay 0.3BTC in fees. This would be
        // $1,650 at the moment.
        let weight = 400;
        let amount = bitcoin::Amount::from_sat(100_000_000);

        let sat_per_vb = 4_000_000;
        let fee_rate = FeeRate::from_sat_per_vb(sat_per_vb).unwrap();

        let relay_fee = bitcoin::Amount::ONE_SAT;
        let is_fee = estimate_fee(weight, amount, fee_rate, relay_fee).unwrap();

        // weight / 4.0 *  sat_per_vb would be greater than 3% hence we take total
        // max allowed fee.
        assert_eq!(is_fee.to_sat(), MAX_ABSOLUTE_TX_FEE.to_u64().unwrap());
    }

    proptest! {
        #[test]
        fn given_randon_amount_random_fee_and_random_relay_rate_but_fix_weight_does_not_error(
            amount in 547u64..,
            sat_per_vb in 1u64..100_000_000,
            relay_fee in 0u64..100_000_000u64
        ) {
            let weight = 400;
            let amount = bitcoin::Amount::from_sat(amount);

            let fee_rate = FeeRate::from_sat_per_vb(sat_per_vb).unwrap();

            let relay_fee = bitcoin::Amount::from_sat(relay_fee);
            let _is_fee = estimate_fee(weight, amount, fee_rate, relay_fee).unwrap();

        }
    }

    proptest! {
        #[test]
        fn given_amount_in_range_fix_fee_fix_relay_rate_fix_weight_fee_always_smaller_max(
            amount in 1u64..100_000_000,
        ) {
            let weight = 400;
            let amount = bitcoin::Amount::from_sat(amount);

            let sat_per_vb = 100;
            let fee_rate = FeeRate::from_sat_per_vb(sat_per_vb).unwrap();

            let relay_fee = bitcoin::Amount::ONE_SAT;
            let is_fee = estimate_fee(weight, amount, fee_rate, relay_fee).unwrap();

            // weight / 4 * 1_000 is always lower than MAX_ABSOLUTE_TX_FEE
            assert!(is_fee.to_sat() < MAX_ABSOLUTE_TX_FEE.to_u64().unwrap());
        }
    }

    proptest! {
        #[test]
        fn given_amount_high_fix_fee_fix_relay_rate_fix_weight_fee_always_max(
            amount in 100_000_000u64..,
        ) {
            let weight = 400;
            let amount = bitcoin::Amount::from_sat(amount);

            let sat_per_vb = 1_000;
            let fee_rate = FeeRate::from_sat_per_vb(sat_per_vb).unwrap();

            let relay_fee = bitcoin::Amount::ONE_SAT;
            let is_fee = estimate_fee(weight, amount, fee_rate, relay_fee).unwrap();

            // weight / 4 * 1_000  is always higher than MAX_ABSOLUTE_TX_FEE
            assert!(is_fee.to_sat() >= MAX_ABSOLUTE_TX_FEE.to_u64().unwrap());
        }
    }

    proptest! {
        #[test]
        fn given_fee_above_max_should_always_errors(
            sat_per_vb in 100_000_000u64..(u64::MAX / 250),
        ) {
            let weight = 400;
            let amount = bitcoin::Amount::from_sat(547u64);

            let fee_rate = FeeRate::from_sat_per_vb(sat_per_vb).unwrap();

            let relay_fee = bitcoin::Amount::from_sat(1);
            assert!(estimate_fee(weight, amount, fee_rate, relay_fee).is_err());

        }
    }

    proptest! {
        #[test]
        fn given_relay_fee_above_max_should_always_errors(
            relay_fee in 100_000_000u64..
        ) {
            let weight = 400;
            let amount = bitcoin::Amount::from_sat(547u64);

            let fee_rate = FeeRate::from_sat_per_vb(1).unwrap();

            let relay_fee = bitcoin::Amount::from_sat(relay_fee);
            assert!(estimate_fee(weight, amount, fee_rate, relay_fee).is_err());
        }
    }

    #[tokio::test]
    async fn given_no_balance_returns_amount_0() {
        let wallet = TestWalletBuilder::new(0).with_fees(1, 1).build().await;
        let amount = wallet.max_giveable(TxLock::script_size()).await.unwrap();

        assert_eq!(amount, Amount::ZERO);
    }

    #[tokio::test]
    async fn given_balance_below_min_relay_fee_returns_amount_0() {
        let wallet = TestWalletBuilder::new(1000)
            .with_fees(1, 1001)
            .build()
            .await;
        let amount = wallet.max_giveable(TxLock::script_size()).await.unwrap();

        assert_eq!(amount, Amount::ZERO);
    }

    #[tokio::test]
    async fn given_balance_above_relay_fee_returns_amount_greater_0() {
        let wallet = TestWalletBuilder::new(10_000).build().await;
        let amount = wallet.max_giveable(TxLock::script_size()).await.unwrap();

        assert!(amount.to_sat() > 0);
    }

    /// This test ensures that the relevant script output of the transaction
    /// created out of the PSBT is at index 0. This is important because
    /// subscriptions to the transaction are on index `0` when broadcasting the
    /// transaction.
    #[tokio::test]
    async fn given_amounts_with_change_outputs_when_signing_tx_then_output_index_0_is_ensured_for_script(
    ) {
        // This value is somewhat arbitrary but the indexation problem usually occurred
        // on the first or second value (i.e. 547, 548) We keep the test
        // iterations relatively low because these tests are expensive.
        let above_dust = 547;
        let balance = 2000;

        // We don't care about fees in this test, thus use a zero fee rate
        let wallet = TestWalletBuilder::new(balance)
            .with_zero_fees()
            .build()
            .await;

        // sorting is only relevant for amounts that have a change output
        // if the change output is below dust it will be dropped by the BDK
        for amount in above_dust..(balance - (above_dust - 1)) {
            let (A, B) = (PublicKey::random(), PublicKey::random());
            let change = wallet.new_address().await.unwrap();
            let txlock = TxLock::new(&wallet, bitcoin::Amount::from_sat(amount), A, B, change)
                .await
                .unwrap();
            let txlock_output = txlock.script_pubkey();

            let tx = wallet.sign_and_finalize(txlock.into()).await.unwrap();
            let tx_output = tx.output[0].script_pubkey.clone();

            assert_eq!(
                tx_output, txlock_output,
                "Output {:?} index mismatch for amount {} and balance {}",
                tx.output, amount, balance
            );
        }
    }

    #[tokio::test]
    async fn can_override_change_address() {
        let wallet = TestWalletBuilder::new(50_000).build().await;
        let custom_change = "bcrt1q08pfqpsyrt7acllzyjm8q5qsz5capvyahm49rw"
            .parse::<Address<NetworkUnchecked>>()
            .unwrap()
            .assume_checked();

        let psbt = wallet
            .send_to_address(
                wallet.new_address().await.unwrap(),
                Amount::from_sat(10_000),
                Some(custom_change.clone()),
            )
            .await
            .unwrap();
        let transaction = wallet.sign_and_finalize(psbt).await.unwrap();

        match transaction.output.as_slice() {
            [first, change] => {
                assert_eq!(first.value, Amount::from_sat(10_000));
                assert_eq!(change.script_pubkey, custom_change.script_pubkey());
            }
            _ => panic!("expected exactly two outputs"),
        }
    }

    #[test]
    fn printing_status_change_doesnt_spam_on_same_status() {
        let writer = capture_logs(LevelFilter::DEBUG);

        let inner = bitcoin::hashes::sha256d::Hash::all_zeros();
        let tx = Txid::from_raw_hash(inner);
        let mut old = None;
        old = Some(debug_status_change(tx, old, ScriptStatus::Unseen));
        old = Some(debug_status_change(tx, old, ScriptStatus::InMempool));
        old = Some(debug_status_change(tx, old, ScriptStatus::InMempool));
        old = Some(debug_status_change(tx, old, ScriptStatus::InMempool));
        old = Some(debug_status_change(tx, old, ScriptStatus::InMempool));
        old = Some(debug_status_change(tx, old, ScriptStatus::InMempool));
        old = Some(debug_status_change(tx, old, ScriptStatus::InMempool));
        old = Some(debug_status_change(
            tx,
            old,
            ScriptStatus::Confirmed(Confirmed { depth: 0 }),
        ));
        old = Some(debug_status_change(
            tx,
            old,
            ScriptStatus::Confirmed(Confirmed { depth: 1 }),
        ));
        old = Some(debug_status_change(
            tx,
            old,
            ScriptStatus::Confirmed(Confirmed { depth: 1 }),
        ));
        old = Some(debug_status_change(
            tx,
            old,
            ScriptStatus::Confirmed(Confirmed { depth: 2 }),
        ));
        debug_status_change(tx, old, ScriptStatus::Confirmed(Confirmed { depth: 2 }));

        assert_eq!(
            writer.captured(),
            r"DEBUG swap::bitcoin::wallet: Found relevant Bitcoin transaction txid=0000000000000000000000000000000000000000000000000000000000000000 status=unseen
DEBUG swap::bitcoin::wallet: Bitcoin transaction status changed txid=0000000000000000000000000000000000000000000000000000000000000000 new_status=in mempool old_status=unseen
DEBUG swap::bitcoin::wallet: Bitcoin transaction status changed txid=0000000000000000000000000000000000000000000000000000000000000000 new_status=confirmed with 1 blocks old_status=in mempool
DEBUG swap::bitcoin::wallet: Bitcoin transaction status changed txid=0000000000000000000000000000000000000000000000000000000000000000 new_status=confirmed with 2 blocks old_status=confirmed with 1 blocks
DEBUG swap::bitcoin::wallet: Bitcoin transaction status changed txid=0000000000000000000000000000000000000000000000000000000000000000 new_status=confirmed with 3 blocks old_status=confirmed with 2 blocks
"
        )
    }

    fn confs(confirmations: u32) -> ScriptStatus {
        ScriptStatus::from_confirmations(confirmations)
    }

    proptest::proptest! {
        #[test]
        fn funding_never_fails_with_insufficient_funds(funding_amount in 3000u32.., num_utxos in 1..5u8, sats_per_vb in 1u64..500u64, key in crate::proptest::bitcoin::extended_priv_key(), alice in crate::proptest::ecdsa_fun::point(), bob in crate::proptest::ecdsa_fun::point()) {
            proptest::prop_assume!(alice != bob);

            tokio::runtime::Runtime::new().unwrap().block_on(async move {
                let wallet = TestWalletBuilder::new(funding_amount as u64)
                    .with_key(key)
                    .with_num_utxos(num_utxos)
                    .with_fees(sats_per_vb, 1000)
                    .build()
                    .await;

                let amount = wallet.max_giveable(TxLock::script_size()).await.unwrap();
                let psbt: PartiallySignedTransaction = TxLock::new(&wallet, amount, PublicKey::from(alice), PublicKey::from(bob), wallet.new_address().await.unwrap()).await.unwrap().into();
                let result = wallet.sign_and_finalize(psbt).await;

                result.expect("transaction to be signed");
            });
        }
    }
}
