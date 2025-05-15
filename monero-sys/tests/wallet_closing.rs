use monero_sys::{Daemon, WalletManager};

const STAGENET_REMOTE_NODE: &str = "node.sethforprivacy.com:38089";

#[tokio::test]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("info,test=debug,monero_harness=debug,monero_rpc=debug,wallet_closing=trace,monero_sys=trace")
        .with_test_writer()
        .init();

    let temp_dir = tempfile::tempdir().unwrap();
    let daemon = Daemon {
        address: STAGENET_REMOTE_NODE.into(),
        ssl: true,
    };

    let wallet_manager_mutex = WalletManager::get(Some(daemon)).await.unwrap();

    {
        let mut wallet_manager = wallet_manager_mutex.lock().await;

        while !wallet_manager.connected().await {
            tracing::info!("Waiting to connect to daemon...");
            std::thread::sleep(std::time::Duration::from_secs(1));
        }

        let _wallet = wallet_manager
            .open_or_create_wallet(
                temp_dir
                    .path()
                    .join("wallet212321")
                    .display()
                    .to_string()
                    .as_str(),
                None,
                monero::Network::Stagenet,
            )
            .await
            .unwrap();
    }

    // Sleep for 2 seconds to allow the wallet to be closed
    tracing::info!("Sleeping for 2 seconds");
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    tracing::info!("Finished");

    assert!(wallet_manager_mutex
        .lock()
        .await
        .all_open_wallets()
        .is_empty());
}
