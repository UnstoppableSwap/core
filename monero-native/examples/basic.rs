use monero_c_rust::{NetworkType, WalletConfig, WalletError, WalletManager};
use tempfile::TempDir;

fn main() -> Result<(), WalletError> {
    let manager = WalletManager::new()?;

    let temp_dir = TempDir::new().expect("Failed to create temporary directory");
    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path.to_str().unwrap();

    let wallet = manager.restore_polyseed(
        wallet_str.to_string(),
        "password".to_string(),
        "capital chief route liar question fix clutch water outside pave hamster occur always learn license knife".to_string(),
        NetworkType::Stagenet,
        1767926, // Restore from the beginning of the blockchain.
        1, // Default KDF rounds.
        "".to_string(), // No seed offset.
        true, // Create a new wallet.
    )?;

    println!("Wallet created successfully.");

    // Print the primary address.
    println!("Primary address: {}", wallet.get_address(0, 0)?);

    // Initialize the wallet.
    let config = WalletConfig {
        daemon_address: "http://localhost:38081".to_string(),
        upper_transaction_size_limit: 10000, // TODO: use sane value.
        daemon_username: "".to_string(),
        daemon_password: "".to_string(),
        use_ssl: false,
        light_wallet: false,
        proxy_address: "".to_string(),
    };

    // Set WalletManager's daemon address
    manager.set_daemon_address(&config.daemon_address);

    // Perform the initialization.
    wallet.init(config)?;
    wallet.throw_if_error()?;

    let target_block = manager.get_height_target()?;
    println!("Target block: {}", target_block);

    // Refresh the wallet.
    wallet.refresh()?;
    // wallet.refresh_async()?;
    wallet.throw_if_error()?;

    // Wait for the refresh to complete.
    loop {
        let height = wallet
            .get_blockchain_height()
            .expect("Failed to get blockchain height");
        println!("Current blockchain height: {}", height);
        if height > 1768865 {
            // After this height we can get_balance.
            break ();
        }
        // Wait one second.
        std::thread::sleep(std::time::Duration::from_secs(1));
    }

    // Get the balance.
    let balance_result = wallet.get_balance(0); // Account index 0.
    let balance = balance_result.unwrap();
    println!("Balance: {:?}", balance);

    // Clean up the wallet.
    std::fs::remove_file(wallet_str).expect("Failed to delete test wallet");
    std::fs::remove_file(format!("{}.keys", wallet_str))
        .expect("Failed to delete test wallet keys");

    Ok(())
}
