use monero_c_rust::{WalletManager, NetworkType, WalletConfig, WalletError, WalletResult, WalletStatus_Ok};
use std::fs;
use std::sync::Arc;
use std::time::Instant;
use tempfile::TempDir;

const TEST_WALLET_NAMES: &[&str] = &[
    "test_wallet",
    "mainnet_wallet",
    "testnet_wallet",
    "stagenet_wallet",
];

/// Helper function to clean up existing wallet files in a temporary directory.
fn check_and_delete_existing_wallets(temp_dir: &TempDir) -> std::io::Result<()> {
    for name in TEST_WALLET_NAMES {
        // Construct absolute paths for wallet files.
        let wallet_file = temp_dir.path().join(name);
        let keys_file = temp_dir.path().join(format!("{}.keys", name));
        let address_file = temp_dir.path().join(format!("{}.address.txt", name)); // Added

        // Delete wallet file if it exists.
        if wallet_file.exists() {
            if let Err(e) = fs::remove_file(&wallet_file) {
                println!("Warning: Failed to delete wallet file {:?}: {}", wallet_file, e);
            } else {
                println!("Deleted existing wallet file: {:?}", wallet_file);
            }
        }

        // Delete keys file if it exists.
        if keys_file.exists() {
            if let Err(e) = fs::remove_file(&keys_file) {
                println!("Warning: Failed to delete keys file {:?}: {}", keys_file, e);
            } else {
                println!("Deleted existing keys file: {:?}", keys_file);
            }
        }

        // Delete address file if it exists.
        if address_file.exists() {
            if let Err(e) = fs::remove_file(&address_file) {
                println!("Warning: Failed to delete address file {:?}: {}", address_file, e);
            } else {
                println!("Deleted existing address file: {:?}", address_file);
            }
        }
    }
    Ok(())
}

/// Sets up the test environment by creating a temporary directory and initializing the WalletManager.
///
/// Returns:
/// - An `Arc` wrapped `WalletManager` instance.
/// - A `TempDir` representing the temporary directory.
fn setup() -> WalletResult<(Arc<WalletManager>, TempDir)> {
    println!("Setting up test environment...");
    let temp_dir = tempfile::tempdir().expect("Failed to create temporary directory");
    check_and_delete_existing_wallets(&temp_dir).expect("Failed to clean up existing wallets");

    println!("Creating WalletManager...");
    let start = Instant::now();
    let manager = WalletManager::new()?;
    println!("WalletManager creation took {:?}", start.elapsed());

    Ok((manager, temp_dir))
}

/// Tears down the test environment by deleting wallet files.
///
/// Args:
/// - `temp_dir`: Reference to the temporary directory.
///
/// Returns:
/// - `Result<(), std::io::Error>` indicating success or failure.
fn teardown(temp_dir: &TempDir) -> std::io::Result<()> {
    println!("Tearing down test environment...");
    check_and_delete_existing_wallets(temp_dir)
}

#[test]
fn test_wallet_manager_creation() {
    println!("Running test_wallet_manager_creation");
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    // Construct the full path for the wallet within temp_dir.
    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path.to_str().expect("Failed to convert wallet path to string");

    let wallet_result = manager.create_wallet(wallet_str, "password", "English", NetworkType::Mainnet);
    assert!(wallet_result.is_ok(), "WalletManager creation seems to have failed");

    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_wallet_creation() {
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path.to_str().expect("Failed to convert wallet path to string");

    let wallet = manager.create_wallet(wallet_str, "password", "English", NetworkType::Mainnet);
    assert!(wallet.is_ok(), "Failed to create wallet");
    let wallet = wallet.unwrap();
    assert!(wallet.is_deterministic().is_ok(), "Wallet creation seems to have failed");

    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_restore_mnemonic_integration() {
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path.to_str().expect("Failed to convert wallet path to string").to_string();
    let mnemonic_seed = "hemlock jubilee eden hacksaw boil superior inroads epoxy exhale orders cavernous second brunt saved richly lower upgrade hitched launching deepest mostly playful layout lower eden".to_string();

    let restored_wallet = manager.restore_mnemonic(
        wallet_str.clone(),
        "password".to_string(),
        mnemonic_seed,
        NetworkType::Mainnet,
        0, // Restore from the beginning of the blockchain.
        1, // Default KDF rounds.
        "".to_string(), // No seed offset.
    );

    assert!(restored_wallet.is_ok(), "Failed to restore wallet: {:?}", restored_wallet.err());

    // Check that the wallet is deterministic.
    let wallet = restored_wallet.unwrap();
    assert!(wallet.is_deterministic().is_ok(), "Restored wallet seems to have failed");
    assert!(wallet.is_deterministic().unwrap(), "Restored wallet should be deterministic");

    // Optionally, verify the address if applicable.
    let address_result = wallet.get_address(0, 0);
    assert!(address_result.is_ok(), "Failed to retrieve address: {:?}", address_result.err());
    let address = address_result.unwrap();
    assert_eq!(
        address,
        "45wsWad9EwZgF3VpxQumrUCRaEtdyyh6NG8sVD3YRVVJbK1jkpJ3zq8WHLijVzodQ22LxwkdWx7fS2a6JzaRGzkNU8K2Dhi",
        "Address does not match expected value"
    );

    // Clean up wallet files.
    teardown(&temp_dir).expect("Failed to clean up after test");
}
// TODO: Test with offset.

#[test]
fn test_restore_polyseed_integration() {
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path.to_str().expect("Failed to convert wallet path to string").to_string();
    let mnemonic_seed = "capital chief route liar question fix clutch water outside pave hamster occur always learn license knife".to_string();

    let restored_wallet = manager.restore_polyseed(
        wallet_str.clone(),
        "password".to_string(),
        mnemonic_seed,
        NetworkType::Mainnet,
        0, // Restore from the beginning of the blockchain.
        1, // Default KDF rounds.
        "".to_string(), // No seed offset.
        true, // Create a new wallet.
    );

    assert!(restored_wallet.is_ok(), "Failed to restore wallet: {:?}", restored_wallet.err());

    // Check that the wallet is deterministic.
    let wallet = restored_wallet.unwrap();
    assert!(wallet.is_deterministic().is_ok(), "Restored wallet seems to have failed");
    assert!(wallet.is_deterministic().unwrap(), "Restored wallet should be deterministic");

    // Optionally, verify the address if applicable.
    let address_result = wallet.get_address(0, 0);
    assert!(address_result.is_ok(), "Failed to retrieve address: {:?}", address_result.err());
    let address = address_result.unwrap();
    assert_eq!(
        address,
        "465cUW8wTMSCV8oVVh7CuWWHs7yeB1oxhNPrsEM5FKSqadTXmobLqsNEtRnyGsbN1rbDuBtWdtxtXhTJda1Lm9vcH2ZdrD1",
        "Address does not match expected value"
    );

    // Clean up wallet files.
    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_generate_from_keys_integration() {
    println!("Running test_generate_from_keys_integration");
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path.to_str().expect("Failed to convert wallet path to string");

    let wallet = manager.generate_from_keys(
        wallet_str.to_string(),
        "45wsWad9EwZgF3VpxQumrUCRaEtdyyh6NG8sVD3YRVVJbK1jkpJ3zq8WHLijVzodQ22LxwkdWx7fS2a6JzaRGzkNU8K2Dhi".to_string(),
        "29adefc8f67515b4b4bf48031780ab9d071d24f8a674b879ce7f245c37523807".to_string(),
        "3bc0b202cde92fe5719c3cc0a16aa94f88a5d19f8c515d4e35fae361f6f2120e".to_string(),
        0,
        "password".to_string(),
        "English".to_string(),
        NetworkType::Mainnet,
        1, // KDF rounds.
    );

    assert!(wallet.is_ok(), "Failed to generate wallet from keys: {:?}", wallet.err());

    // Verify that the wallet was generated correctly.
    let wallet = wallet.expect("Failed to create wallet");

    // This is required even though "English" was passed as the seed language above.
    let set_language_result = wallet.set_seed_language("English");
    assert!(
        set_language_result.is_ok(),
        "Failed to set seed language: {:?}",
        set_language_result.err()
    );

    // The address should be "45wsWad9...".
    let address_result = wallet.get_address(0, 0);
    assert!(address_result.is_ok(), "Failed to get address: {:?}", address_result.err());
    let address = address_result.unwrap();
    assert_eq!(address, "45wsWad9EwZgF3VpxQumrUCRaEtdyyh6NG8sVD3YRVVJbK1jkpJ3zq8WHLijVzodQ22LxwkdWx7fS2a6JzaRGzkNU8K2Dhi");

    // Get the seed.  It should be "hemlock jubilee...".
    let seed_result = wallet.get_seed(None);
    assert!(seed_result.is_ok(), "Failed to get seed: {:?}",
            seed_result.err());
    let seed = seed_result.unwrap();
    assert_eq!(seed, "hemlock jubilee eden hacksaw boil superior inroads epoxy exhale orders cavernous second brunt saved richly lower upgrade hitched launching deepest mostly playful layout lower eden");

    // Clean up wallet files.
    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_generate_view_only_from_keys_integration() {
    println!("Running test_generate_from_keys_integration");
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path.to_str().expect("Failed to convert wallet path to string");

    let wallet = manager.generate_from_keys(
        wallet_str.to_string(),
        "45wsWad9EwZgF3VpxQumrUCRaEtdyyh6NG8sVD3YRVVJbK1jkpJ3zq8WHLijVzodQ22LxwkdWx7fS2a6JzaRGzkNU8K2Dhi".to_string(),
        "".to_string(),
        "3bc0b202cde92fe5719c3cc0a16aa94f88a5d19f8c515d4e35fae361f6f2120e".to_string(),
        0,
        "password".to_string(),
        "English".to_string(),
        NetworkType::Mainnet,
        1, // KDF rounds.
    );

    assert!(wallet.is_ok(), "Failed to generate wallet from keys: {:?}", wallet.err());

    // Verify that the wallet was generated correctly.
    let wallet = wallet.expect("Failed to create wallet");

    // This is required even though "English" was passed as the seed language above.
    let set_language_result = wallet.set_seed_language("English");
    assert!(
        set_language_result.is_ok(),
        "Failed to set seed language: {:?}",
        set_language_result.err()
    );

    // The address should be "45wsWad9...".
    let address_result = wallet.get_address(0, 0);
    assert!(address_result.is_ok(), "Failed to get address: {:?}", address_result.err());
    let address = address_result.unwrap();
    assert_eq!(address, "45wsWad9EwZgF3VpxQumrUCRaEtdyyh6NG8sVD3YRVVJbK1jkpJ3zq8WHLijVzodQ22LxwkdWx7fS2a6JzaRGzkNU8K2Dhi");

    // The wallet should not be deterministic.
    let is_deterministic_result = wallet.is_deterministic();
    assert!(
        is_deterministic_result.is_ok(),
        "Failed to check if wallet is deterministic: {:?}",
        is_deterministic_result.err()
    );
    assert!(!is_deterministic_result.unwrap(), "Wallet should not be deterministic");

    // Clean up wallet files.
    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_get_seed() {
    println!("Running test_get_seed");
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    // Construct the full path for the wallet within temp_dir.
    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path.to_str().expect("Failed to convert wallet path to string");

    let wallet = manager
        .create_wallet(wallet_str, "password", "English", NetworkType::Mainnet)
        .expect("Failed to create wallet");

    // Test getting seed without offset.
    println!("Attempting to get seed without offset...");
    let start = Instant::now();
    let result = wallet.get_seed(None);
    println!("get_seed without offset took {:?}", start.elapsed());
    assert!(result.is_ok(), "Failed to get seed: {:?}", result.err());
    assert!(!result.unwrap().is_empty(), "Seed is empty");

    // Test getting seed with an offset.
    println!("Attempting to get seed with offset...");
    let start = Instant::now();
    let result_with_offset = wallet.get_seed(Some("example_offset"));
    println!("get_seed with offset took {:?}", start.elapsed());
    assert!(result_with_offset.is_ok(), "Failed to get seed with offset: {:?}", result_with_offset.err());
    assert!(!result_with_offset.unwrap().is_empty(), "Seed with offset is empty");

    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_get_address() {
    println!("Running test_get_address");
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    // Construct the full path for the wallet within temp_dir.
    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path.to_str().expect("Failed to convert wallet path to string");

    let wallet = manager
        .create_wallet(wallet_str, "password", "English", NetworkType::Mainnet)
        .expect("Failed to create wallet");
    println!("Attempting to get address...");
    let start = Instant::now();
    let result = wallet.get_address(0, 0);
    println!("get_address took {:?}", start.elapsed());
    assert!(result.is_ok(), "Failed to get address: {:?}", result.err());
    assert!(!result.unwrap().is_empty(), "Address is empty");

    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_is_deterministic() {
    println!("Running test_is_deterministic");
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    // Construct the full path for the wallet within temp_dir.
    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path.to_str().expect("Failed to convert wallet path to string");

    let wallet = manager
        .create_wallet(wallet_str, "password", "English", NetworkType::Mainnet)
        .expect("Failed to create wallet");
    println!("Checking if wallet is deterministic...");
    let start = Instant::now();
    let result = wallet.is_deterministic();
    println!("is_deterministic check took {:?}", start.elapsed());
    assert!(result.is_ok(), "Failed to check if wallet is deterministic: {:?}", result.err());
    assert!(result.unwrap(), "Wallet should be deterministic");

    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_wallet_creation_with_different_networks() {
    println!("Running test_wallet_creation_with_different_networks");
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    // Define wallet names and corresponding network types.
    let wallets = vec![
        ("mainnet_wallet", NetworkType::Mainnet),
        ("testnet_wallet", NetworkType::Testnet),
        ("stagenet_wallet", NetworkType::Stagenet),
    ];

    for (name, net_type) in wallets {
        println!("Creating wallet: {} on network type {:?}", name, net_type);

        // Construct the full path for each wallet within temp_dir.
        let wallet_path = temp_dir.path().join(name);
        let wallet_str = wallet_path.to_str().expect("Failed to convert wallet path to string");

        let wallet = manager.create_wallet(wallet_str, "password", "English", net_type);
        assert!(wallet.is_ok(), "Failed to create wallet: {}", name);
    }

    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_multiple_address_generation() {
    println!("Running test_multiple_address_generation");
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    // Construct the full path for the wallet within temp_dir.
    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path.to_str().expect("Failed to convert wallet path to string");

    let wallet = manager
        .create_wallet(wallet_str, "password", "English", NetworkType::Mainnet)
        .expect("Failed to create wallet");

    for i in 0..5 {
        println!("Generating address {}...", i);
        let start = Instant::now();
        let result = wallet.get_address(0, i);
        println!("Address generation took {:?}", start.elapsed());
        assert!(result.is_ok(), "Failed to get address {}: {:?}", i, result.err());
        assert!(!result.unwrap().is_empty(), "Address {} is empty", i);
    }

    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_wallet_error_display() {
    println!("Running test_wallet_error_display");

    // Test WalletError::FfiError variant.
    let error = WalletError::FfiError("Test error".to_string());
    match error {
        WalletError::FfiError(msg) => assert_eq!(msg, "Test error"),
        _ => panic!("Expected FfiError variant"),
    }

    // Test WalletError::NullPointer variant.
    let error = WalletError::NullPointer;
    match error {
        WalletError::NullPointer => assert!(true),
        _ => panic!("Expected NullPointer variant"),
    }

    // Test WalletError::WalletErrorCode variant.
    let error = WalletError::WalletErrorCode(2, "Sample wallet error".to_string());
    match error {
        WalletError::WalletErrorCode(code, msg) => {
            assert_eq!(code, 2);
            assert_eq!(msg, "Sample wallet error");
        },
        _ => panic!("Expected WalletErrorCode variant"),
    }
}

#[test]
fn test_wallet_status_integration() {
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    // Create a wallet.
    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path.to_str().expect("Failed to convert wallet path to string");
    let wallet = manager.create_wallet(wallet_str, "password", "English", NetworkType::Mainnet)
        .expect("Failed to create wallet");

    // Check the status of the wallet.
    let status = manager.get_status(wallet.ptr.as_ptr());
    assert!(status.is_ok(), "Expected status OK, got error: {:?}", status.err());

    // Clean up.
    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_open_wallet_integration() {
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    // Create a wallet.
    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path.to_str().expect("Failed to convert wallet path to string");

    let wallet = manager
        .create_wallet(wallet_str, "password", "English", NetworkType::Mainnet)
        .expect("Failed to create wallet");

    // Drop the wallet to simulate closing it.
    drop(wallet);

    // Try opening the wallet.
    let open_result = manager.open_wallet(wallet_str, "password", NetworkType::Mainnet);
    assert!(open_result.is_ok(), "Failed to open wallet: {:?}", open_result.err());

    // Clean up.
    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_open_wallet_invalid_password() {
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path.to_str().expect("Failed to convert wallet path to string");

    // Create a wallet with a valid password.
    let wallet = manager.create_wallet(wallet_str, "correct_password", "English", NetworkType::Mainnet)
        .expect("Failed to create wallet");

    // Drop the wallet
    drop(wallet);

    // Attempt to open the wallet with an incorrect password.
    let open_result = manager.open_wallet(wallet_str, "wrong_password", NetworkType::Mainnet);
    assert!(open_result.is_err(), "Expected an error when opening wallet with incorrect password");

    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_open_wallet_invalid_path() {
    let (manager, _temp_dir) = setup().expect("Failed to set up test environment");

    // Try to open a wallet at a non-existent path.
    let invalid_path = "/invalid/path/to/non_existent_wallet";
    let open_result = manager.open_wallet(invalid_path, "password", NetworkType::Mainnet);

    // Check if the result is an error.
    match open_result {
        Err(e) => {
            // Inspect the error to check the specific status and error message.
            if let WalletError::WalletErrorCode(status, error_message) = e {
                assert_ne!(
                    status, WalletStatus_Ok,
                    "Expected a non-OK status code, got OK instead."
                );
                assert!(
                    error_message.contains("file not found") || error_message.contains("openWallet"),
                    "Unexpected error message: {}",
                    error_message
                );
            } else {
                panic!("Expected WalletErrorCode, got {:?}", e);
            }
        }
        Ok(_) => panic!("Expected an error when opening a non-existent wallet, but it succeeded."),
    }
}

#[test]
fn test_get_balance_integration() {
    println!("Running test_get_balance_integration");
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    // Construct the full path for the wallet within temp_dir.
    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path.to_str().expect("Failed to convert wallet path to string");

    // Create the wallet.
    let wallet = manager
        .create_wallet(wallet_str, "password", "English", NetworkType::Mainnet)
        .expect("Failed to create wallet");

    // Fetch the balance.
    println!("Fetching wallet balance...");
    let start = Instant::now();
    let balance_result = wallet.get_balance(0); // Account index 0.
    println!("Fetching balance took {:?}", start.elapsed());

    assert!(balance_result.is_ok(), "Failed to fetch balance: {:?}", balance_result.err());

    let balance = balance_result.unwrap();
    println!("Balance: {:?}", balance);

    // Ensure the balance values make sense.
    // assert!(balance.balance >= 0, "Balance should be non-negative");
    // assert!(balance.unlocked_balance >= 0, "Unlocked balance should be non-negative");
    // These assertions are meaningless with the constraints of the type.
    // TODO: Test with scanning integration.

    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_create_account_integration() {
    println!("Running test_create_account_integration");
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    // Construct the full path for the wallet within temp_dir.
    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path.to_str().expect("Failed to convert wallet path to string");

    // Create the wallet.
    let wallet = manager
        .create_wallet(wallet_str, "password", "English", NetworkType::Mainnet)
        .expect("Failed to create wallet");

    // Create a new account with a label.
    println!("Creating a new account...");
    let start = Instant::now();
    let result = wallet.create_account("Test Account Integration");
    println!("create_account took {:?}", start.elapsed());

    assert!(result.is_ok(), "Failed to create account: {:?}", result.err());

    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_get_accounts_integration() {
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path.to_str().expect("Failed to convert wallet path to string");

    let wallet = manager
        .create_wallet(wallet_str, "password", "English", NetworkType::Mainnet)
        .expect("Failed to create wallet");

    // Add multiple accounts.
    wallet.create_account("Integration Account 1").expect("Failed to create account");
    wallet.create_account("Integration Account 2").expect("Failed to create account");

    // Fetch accounts.
    let accounts_result = wallet.get_accounts();
    assert!(accounts_result.is_ok(), "Failed to fetch accounts: {:?}", accounts_result.err());
    let accounts = accounts_result.unwrap().accounts;
    assert_eq!(accounts.len(), 3, "Expected 3 accounts");

    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_close_wallet_integration() {
    println!("Running test_close_wallet_integration");
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    // Construct the full path for the wallet within temp_dir.
    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path.to_str().expect("Failed to convert wallet path to string");

    // Create the wallet.
    let mut wallet = manager
        .create_wallet(wallet_str, "password", "English", NetworkType::Mainnet)
        .expect("Failed to create wallet");

    // Use the wallet for operations...
    println!("Using the wallet for operations...");

    // Close the wallet.
    println!("Closing the wallet...");
    let start = Instant::now();
    let close_result = wallet.close_wallet();
    println!("close_wallet took {:?}", start.elapsed());
    assert!(close_result.is_ok(), "Failed to close wallet: {:?}", close_result.err());

    // Attempt to close the wallet again.
    println!("Attempting to close the wallet again...");
    let start = Instant::now();
    let close_again_result = wallet.close_wallet();
    println!("Second close_wallet call took {:?}", start.elapsed());
    assert!(close_again_result.is_ok(), "Failed to close wallet a second time: {:?}", close_again_result.err());

    // Clean up.
    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_get_height_integration() {
    println!("Running test_get_height_integration");
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    // Construct the full path for the wallet within temp_dir.
    let wallet_path = temp_dir.path().join("test_wallet_height");
    let wallet_str = wallet_path.to_str().expect("Failed to convert wallet path to string");

    // Create the wallet.
    let _wallet = manager
        .create_wallet(wallet_str, "password", "English", NetworkType::Mainnet)
        .expect("Failed to create wallet");

    // Fetch the blockchain height.
    println!("Fetching blockchain height...");
    let start = Instant::now();
    let height_result = manager.get_height();
    let duration = start.elapsed();
    println!("Blockchain height retrieval took {:?}", duration);

    assert!(
        height_result.is_ok(),
        "Failed to fetch blockchain height: {:?}",
        height_result.err()
    );

    let height = height_result.unwrap();
    println!("Current blockchain height: {}", height);
    assert!(height == 0, "Blockchain height should be equal to 0.");

    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_refresh_integration_success() {
    println!("Running test_refresh_integration_success");
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    // Construct the full path for the wallet within temp_dir.
    let wallet_path = temp_dir.path().join("refresh_integration_wallet");
    let wallet_str = wallet_path.to_str().expect("Failed to convert wallet path to string");

    // Create the wallet.
    let wallet = manager
        .create_wallet(wallet_str, "password", "English", NetworkType::Mainnet)
        .expect("Failed to create wallet");
    println!("Wallet created successfully.");

    // Define initialization configuration.
    let config = WalletConfig {
        daemon_address: "http://localhost:18081".to_string(),
        upper_transaction_size_limit: 10000, // TODO: use sane value.
        daemon_username: "user".to_string(),
        daemon_password: "pass".to_string(),
        use_ssl: false,
        light_wallet: false,
        proxy_address: "".to_string(),
    };

    // Perform the initialization.
    println!("Initializing the wallet...");
    let start = Instant::now();
    let init_result = wallet.init(config);
    let duration = start.elapsed();
    println!("Initialization took {:?}", duration);

    assert!(init_result.is_ok(), "Failed to initialize wallet: {:?}", init_result.err());

    // Perform a refresh operation after initialization.
    println!("Refreshing the wallet...");
    let refresh_start = Instant::now();
    let refresh_result = wallet.refresh();
    let refresh_duration = refresh_start.elapsed();
    println!("Refresh operation took {:?}", refresh_duration);

    assert!(refresh_result.is_ok(), "Failed to refresh wallet: {:?}", refresh_result.err());

    // Clean up wallet files.
    fs::remove_file(wallet_str).expect("Failed to delete test wallet");
    fs::remove_file(format!("{}.keys", wallet_str)).expect("Failed to delete test wallet keys");

    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_init_integration_success() {
    println!("Running test_init_integration_success");
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    // Construct the full path for the wallet within temp_dir.
    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path.to_str().expect("Failed to convert wallet path to string");

    // Create the wallet.
    let wallet = manager
        .create_wallet(wallet_str, "password", "English", NetworkType::Mainnet)
        .expect("Failed to create wallet");
    println!("Wallet created successfully.");

    // Define initialization configuration.
    let config = WalletConfig {
        daemon_address: "http://localhost:18081".to_string(),
        upper_transaction_size_limit: 10000,
        daemon_username: "user".to_string(),
        daemon_password: "pass".to_string(),
        use_ssl: false,
        light_wallet: false,
        proxy_address: "".to_string(),
    };

    // Perform the initialization.
    println!("Initializing the wallet...");
    let start = Instant::now();
    let init_result = wallet.init(config);
    let duration = start.elapsed();
    println!("Initialization took {:?}", duration);

    assert!(init_result.is_ok(), "Failed to initialize wallet: {:?}", init_result.err());

    // Perform a refresh operation after initialization.
    println!("Refreshing the wallet...");
    let refresh_result = wallet.refresh();
    assert!(refresh_result.is_ok(), "Failed to refresh wallet after initialization: {:?}", refresh_result.err());

    // Clean up wallet files.
    fs::remove_file(wallet_str).expect("Failed to delete test wallet");
    fs::remove_file(format!("{}.keys", wallet_str)).expect("Failed to delete test wallet keys");

    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_set_seed_language_integration() {
    println!("Running test_set_seed_language_integration");
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    // Construct the full path for the wallet within temp_dir.
    let wallet_path = temp_dir.path().join("set_seed_language_wallet");
    let wallet_str = wallet_path.to_str().expect("Failed to convert wallet path to string");

    // Create the wallet.
    let wallet = manager
        .create_wallet(wallet_str, "password", "English", NetworkType::Mainnet)
        .expect("Failed to create wallet");
    println!("Wallet created successfully.");

    // Set the seed language to French.
    println!("Setting seed language to French...");
    let start = Instant::now();
    let set_language_result = wallet.set_seed_language("French");
    let duration = start.elapsed();
    println!("set_seed_language took {:?}", duration);

    assert!(
        set_language_result.is_ok(),
        "Failed to set seed language: {:?}",
        set_language_result.err()
    );

    // Optionally, verify the seed language by retrieving it.
    // This requires implementing a corresponding `get_seed_language` method.

    // Clean up wallet files.
    fs::remove_file(wallet_str).expect("Failed to delete test wallet");
    fs::remove_file(format!("{}.keys", wallet_str)).expect("Failed to delete test wallet keys");

    teardown(&temp_dir).expect("Failed to clean up after test");
}
