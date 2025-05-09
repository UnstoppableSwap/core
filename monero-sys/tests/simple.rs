use monero_sys::{Daemon, SyncProgress, Wallet, WalletManager};

const PASSWORD: &str = "test";

const STAGENET_REMOTE_NODE: &str = "node.sethforprivacy.com:38089";
const STAGENET_WALLET_SEED: &str = "echo ourselves ruined oven masterful wives enough addicted future cottage illness adopt lucky movement tiger taboo imbalance antics iceberg hobby oval aloof tuesday uttered oval";
const STAGENET_WALLET_RESTORE_HEIGHT: u64 = 1728128;

#[tokio::test]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            "warn,test=debug,monero_harness=debug,monero_rpc=debug,simple=trace,monero_sys=trace",
        )
        .with_test_writer()
        .init();

    let temp_dir = tempfile::tempdir().unwrap();
    let daemon = Daemon {
        address: STAGENET_REMOTE_NODE.into(),
        ssl: true,
    };
    let wallet_manager_mutex = WalletManager::get(Some(daemon)).await.unwrap();
    let mut wallet_manager = wallet_manager_mutex.lock().await;

    while !wallet_manager.connected().await {
        tracing::info!("Waiting to connect to daemon...");
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    tracing::info!("Connected to daemon");
    tracing::info!(
        "Daemon height: {}",
        wallet_manager.blockchain_height().await.unwrap()
    );

    let wallet_name = "recovered_wallet";
    let wallet_path = temp_dir.path().join(wallet_name).display().to_string();

    tracing::info!("Recovering wallet from seed");
    let wallet_mutex = wallet_manager
        .recover_wallet(
            &wallet_path,
            PASSWORD,
            STAGENET_WALLET_SEED,
            monero::Network::Stagenet,
            STAGENET_WALLET_RESTORE_HEIGHT,
        )
        .await
        .expect("Failed to recover wallet");

    tracing::info!(
        "Primary address: {}",
        wallet_mutex.lock().await.main_address()
    );

    // Wait for a while to let the wallet sync, checking sync status
    tracing::info!("Waiting for wallet to sync...");

    Wallet::wait_until_synced(
        wallet_mutex.clone(),
        Some(|sync_progress: SyncProgress| {
            tracing::info!("Sync progress: {}%", sync_progress.percentage());
        }),
    )
    .await
    .expect("Failed to sync wallet");

    tracing::info!("Wallet is synchronized!");

    let balance = wallet_mutex.lock().await.total_balance();
    tracing::info!("Balance: {}", balance.as_pico());

    let unlocked_balance = wallet_mutex.lock().await.unlocked_balance();
    tracing::info!("Unlocked balance: {}", unlocked_balance.as_pico());
}
