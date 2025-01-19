use std::ffi::{CStr, CString};
use std::fmt;
use std::os::raw::{c_int, c_void};
use std::path::PathBuf;
use std::ptr::NonNull;
use std::sync::Arc;

pub mod bindings;
pub use bindings::WalletStatus_Critical;
pub use bindings::WalletStatus_Error;
pub use bindings::WalletStatus_Ok;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkType {
    Mainnet = bindings::NetworkType_MAINNET as isize,
    Testnet = bindings::NetworkType_TESTNET as isize,
    Stagenet = bindings::NetworkType_STAGENET as isize,
}

impl NetworkType {
    pub fn from_c_int(value: c_int) -> Option<Self> {
        match value {
            bindings::NetworkType_MAINNET => Some(NetworkType::Mainnet),
            bindings::NetworkType_TESTNET => Some(NetworkType::Testnet),
            bindings::NetworkType_STAGENET => Some(NetworkType::Stagenet),
            _ => None,
        }
    }

    pub fn to_c_int(self) -> c_int {
        self as c_int
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WalletError {
    #[error("Null pointer encountered")]
    NullPointer,
    #[error("FFI error: {0}")]
    FfiError(String),
    #[error("Wallet error (code {0}): {1}")]
    WalletErrorCode(c_int, String),
}

pub type WalletResult<T> = Result<T, WalletError>;

#[derive(Debug)]
pub struct Account {
    pub index: u32,
    pub label: String,
    pub balance: u64,
    pub unlocked_balance: u64,
}

#[derive(Debug)]
pub struct GetAccounts {
    pub accounts: Vec<Account>,
}

pub struct Wallet {
    pub ptr: NonNull<c_void>,
    pub manager: Arc<WalletManager>,
    pub is_closed: bool,
}

pub struct WalletManager {
    ptr: NonNull<c_void>,
}

/// Configuration parameters for initializing a wallet.
#[derive(Debug, Clone)]
pub struct WalletConfig {
    pub daemon_address: String,
    pub upper_transaction_size_limit: u64,
    pub daemon_username: String,
    pub daemon_password: String,
    pub use_ssl: bool,
    pub light_wallet: bool,
    pub proxy_address: String,
}

impl Default for WalletConfig {
    fn default() -> Self {
        WalletConfig {
            daemon_address: "localhost:18081".to_string(),
            upper_transaction_size_limit: 10000, // TODO: set sane value.
            daemon_username: "".to_string(),
            daemon_password: "".to_string(),
            use_ssl: false,
            light_wallet: false,
            proxy_address: "".to_string(),
        }
    }
}

pub type BlockHeight = u64;

#[derive(Debug)]
pub struct Refreshed;

/// Represents a destination address and the amount to send.
#[derive(Debug, Clone)]
pub struct Destination {
    /// The recipient's address.
    pub address: String,
    /// The amount to send to the recipient (in atomic units).
    pub amount: u64,
}

/// Represents the result of a transfer operation.
#[derive(Debug)]
pub struct Transfer {
    /// The transaction ID of the transfer.
    pub txid: String,
    /// The transaction key, if requested.
    pub tx_key: Option<String>,
    /// The total amount sent in the transfer.
    pub amount: u64,
    /// The fee associated with the transfer.
    pub fee: u64,
}

/// Represents the result of checking a transaction key against a transaction ID and address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckTxKey {
    /// Indicates whether the transaction key is valid.
    pub valid: bool,
    /// An optional error message providing details if the verification fails.
    pub error: Option<String>,
}

impl WalletManager {
    /// Creates a new `WalletManager`.
    ///
    /// # Example
    ///
    /// ```
    /// use monero_c_rust::WalletManager;
    /// let manager = WalletManager::new();
    /// assert!(manager.is_ok());
    /// ```
    pub fn new() -> WalletResult<Arc<Self>> {
        unsafe {
            bindings::MONERO_WalletManagerFactory_setLogLevel(4);
            let ptr = bindings::MONERO_WalletManagerFactory_getWalletManager();
            let ptr = NonNull::new(ptr).ok_or(WalletError::NullPointer)?;
            Ok(Arc::new(WalletManager { ptr }))
        }
    }

    /// Check the status of a wallet to ensure it's in a valid state.
    ///
    /// # Example
    ///
    /// ```rust
    /// use monero_c_rust::{WalletManager, NetworkType};
    /// use tempfile::TempDir;
    ///
    /// let temp_dir = TempDir::new().expect("Failed to create temporary directory");
    /// let wallet_path = temp_dir.path().join("test_wallet");
    /// let wallet_str = wallet_path.to_str().unwrap();
    ///
    /// let manager = WalletManager::new().unwrap();
    /// let wallet_result = manager.create_wallet(wallet_str, "password", "English", NetworkType::Mainnet);
    /// assert!(wallet_result.is_ok(), "Failed to create wallet: {:?}", wallet_result.err());
    /// let wallet = wallet_result.unwrap();
    ///
    /// // Check the status of the wallet, expecting OK
    /// let status_result = manager.get_status(wallet.ptr.as_ptr());
    /// assert!(status_result.is_ok(), "Failed to get status: {:?}", status_result.err());
    /// assert_eq!(status_result.unwrap(), (), "Expected status to be OK");
    ///
    /// // Clean up wallet files.
    /// std::fs::remove_file(wallet_str).expect("Failed to delete test wallet");
    /// std::fs::remove_file(format!("{}.keys", wallet_str)).expect("Failed to delete test wallet keys");
    /// ```
    pub fn get_status(&self, wallet_ptr: *mut c_void) -> WalletResult<()> {
        if wallet_ptr.is_null() {
            return Err(WalletError::NullPointer); // Ensure NullPointer is returned for null wallet
        }

        unsafe {
            let status = bindings::MONERO_Wallet_status(wallet_ptr);

            if status == bindings::WalletStatus_Ok {
                Ok(())
            } else {
                let error_ptr = bindings::MONERO_Wallet_errorString(wallet_ptr);
                let error_msg = if error_ptr.is_null() {
                    "Unknown error".to_string()
                } else {
                    CStr::from_ptr(error_ptr).to_string_lossy().into_owned()
                };
                Err(WalletError::WalletErrorCode(status, error_msg))
            }
        }
    }

    /// Check the status of a wallet and throw an error if an issue is found.
    ///
    /// # Example
    ///
    /// ```rust
    /// use monero_c_rust::{WalletManager, NetworkType};
    /// use tempfile::TempDir;
    ///
    /// let temp_dir = TempDir::new().expect("Failed to create temporary directory");
    /// let wallet_path = temp_dir.path().join("test_wallet");
    /// let wallet_str = wallet_path.to_str().unwrap();
    ///
    /// let manager = WalletManager::new().unwrap();
    /// let wallet_result = manager.create_wallet(wallet_str, "password", "English", NetworkType::Mainnet);
    /// assert!(wallet_result.is_ok(), "Failed to create wallet: {:?}", wallet_result.err());
    /// let wallet = wallet_result.unwrap();
    ///
    /// // Check the status of the wallet, expecting OK
    /// let status_result = manager.throw_if_error(wallet.ptr.as_ptr());
    /// assert!(status_result.is_ok(), "Failed to get status: {:?}", status_result.err());
    ///
    /// // Clean up wallet files.
    /// std::fs::remove_file(wallet_str).expect("Failed to delete test wallet");
    /// std::fs::remove_file(format!("{}.keys", wallet_str)).expect("Failed to delete test wallet keys");
    /// ```
    pub fn throw_if_error(&self, wallet_ptr: *mut c_void) -> WalletResult<()> {
        if wallet_ptr.is_null() {
            return Err(WalletError::NullPointer);
        }

        unsafe {
            let status = bindings::MONERO_Wallet_status(wallet_ptr);
            if status == bindings::WalletStatus_Ok {
                Ok(())
            } else {
                let error_ptr = bindings::MONERO_Wallet_errorString(wallet_ptr);
                let error_msg = if error_ptr.is_null() {
                    "Unknown error".to_string()
                } else {
                    CStr::from_ptr(error_ptr).to_string_lossy().into_owned()
                };
                Err(WalletError::WalletErrorCode(status, error_msg))
            }
        }
    }

    /// Sets the daemon address of WalletManager
    pub fn set_daemon_address(&self, daemon_address: &str) -> WalletResult<()> {
        let c_daemon_address = CString::new(daemon_address)
            .map_err(|_| WalletError::FfiError("Invalid daemon address".to_string()))?;

        unsafe {
            bindings::MONERO_WalletManager_setDaemonAddress(
                self.ptr.as_ptr(),
                c_daemon_address.as_ptr(),
            );
            Ok(())
        }
    }

    /// Create a new wallet.
    ///
    /// Generates a new wallet with the provided path, password, language, and network type, with a
    /// new mnemonic seed generated in the specified language.
    ///
    /// # Example
    ///
    /// ```
    /// use monero_c_rust::{WalletManager, NetworkType};
    /// use std::fs;
    /// use std::path::Path;
    /// use tempfile::TempDir;
    ///
    /// let temp_dir = TempDir::new().expect("Failed to create temporary directory");
    /// let wallet_path = temp_dir.path().join("test_wallet");
    /// let wallet_str = wallet_path.to_str().unwrap();
    ///
    /// let manager = WalletManager::new().unwrap();
    /// let wallet = manager.create_wallet(wallet_str, "password", "English", NetworkType::Mainnet);
    /// assert!(wallet.is_ok());
    ///
    /// // Clean up wallet files.
    /// std::fs::remove_file(wallet_str).expect("Failed to delete test wallet");
    /// std::fs::remove_file(format!("{}.keys", wallet_str)).expect("Failed to delete test wallet keys");
    /// ```
    pub fn create_wallet(
        self: &Arc<Self>,
        path: PathBuf,
        password: &str,
        language: &str,
        network_type: NetworkType,
    ) -> WalletResult<Wallet> {
        let c_path = CString::new(path.to_str().unwrap())
            .map_err(|_| WalletError::FfiError("Invalid path".to_string()))?;
        let c_password = CString::new(password)
            .map_err(|_| WalletError::FfiError("Invalid password".to_string()))?;
        let c_language = CString::new(language)
            .map_err(|_| WalletError::FfiError("Invalid language".to_string()))?;

        unsafe {
            let wallet_ptr = bindings::MONERO_WalletManager_createWallet(
                self.ptr.as_ptr(),
                c_path.as_ptr(),
                c_password.as_ptr(),
                c_language.as_ptr(),
                network_type.to_c_int(),
            );

            self.throw_if_error(wallet_ptr)?;
            if wallet_ptr.is_null() {
                return Err(WalletError::NullPointer);
            }

            Ok(Wallet {
                ptr: NonNull::new(wallet_ptr).unwrap(),
                manager: Arc::clone(self),
                is_closed: false,
            })
        }
    }

    /// Restores a wallet from a mnemonic seed.
    ///
    /// This method restores a Monero wallet using the provided mnemonic seed, network type, and other
    /// configuration parameters.
    ///
    /// # Example
    ///
    /// ```rust
    /// use monero_c_rust::{WalletManager, NetworkType};
    /// use tempfile::TempDir;
    /// use std::fs;
    ///
    /// let temp_dir = TempDir::new().expect("Failed to create temporary directory");
    /// let wallet_path = temp_dir.path().join("restored_wallet");
    /// let wallet_str = wallet_path.to_str().unwrap().to_string();
    ///
    /// let manager = WalletManager::new().expect("Failed to create WalletManager");
    ///
    /// let mnemonic = "hemlock jubilee eden hacksaw boil superior inroads epoxy exhale orders cavernous second brunt saved richly lower upgrade hitched launching deepest mostly playful layout lower eden".to_string();
    /// let wallet = manager.restore_mnemonic(
    ///     wallet_str,
    ///     "strong_password".to_string(),
    ///     mnemonic,
    ///     NetworkType::Mainnet,
    ///     0, // Restore from the beginning of the blockchain.
    ///     1, // Default KDF rounds.
    ///     "".to_string(), // No seed offset.
    /// ).expect("Failed to restore wallet");
    ///
    /// // Use the wallet as needed...
    ///
    /// // Clean up wallet files.
    /// fs::remove_file(&wallet_path).expect("Failed to delete restored wallet");
    /// fs::remove_file(format!("{}.keys", wallet_path.display())).expect("Failed to delete restored wallet keys");
    /// ```
    pub fn restore_mnemonic(
        self: &Arc<Self>,
        path: String,
        password: String,
        seed: String,
        network_type: NetworkType,
        restore_height: u64,
        kdf_rounds: u64,
        seed_offset: String, // TODO: Make an Option.
    ) -> WalletResult<Wallet> {
        // Convert Rust strings to C-compatible strings.
        let c_path = CString::new(path)
            .map_err(|_| WalletError::FfiError("Invalid path string".to_string()))?;
        let c_password = CString::new(password)
            .map_err(|_| WalletError::FfiError("Invalid password string".to_string()))?;
        let c_seed = CString::new(seed)
            .map_err(|_| WalletError::FfiError("Invalid seed string".to_string()))?;
        let c_seed_offset = CString::new(seed_offset)
            .map_err(|_| WalletError::FfiError("Invalid seed_offset string".to_string()))?;

        unsafe {
            let wallet_ptr = bindings::MONERO_WalletManager_recoveryWallet(
                self.ptr.as_ptr(),
                c_path.as_ptr(),
                c_password.as_ptr(),
                c_seed.as_ptr(),
                network_type.to_c_int(),
                restore_height,
                kdf_rounds,
                c_seed_offset.as_ptr(),
            );

            // Check for errors using the returned wallet pointer.
            self.throw_if_error(wallet_ptr)?;
            if wallet_ptr.is_null() {
                return Err(WalletError::NullPointer);
            }

            Ok(Wallet {
                ptr: NonNull::new(wallet_ptr).unwrap(),
                manager: Arc::clone(self),
                is_closed: false,
            })
        }
    }

    /// Restores a wallet from a polyseed.
    ///
    /// # Example
    ///
    /// ```rust
    /// use monero_c_rust::{WalletManager, NetworkType};
    /// use tempfile::TempDir;
    /// use std::fs;
    ///
    /// let temp_dir = TempDir::new().expect("Failed to create temporary directory");
    /// let wallet_path = temp_dir.path().join("test_wallet");
    /// let wallet_str = wallet_path.to_str().unwrap().to_string();
    ///
    /// let manager = WalletManager::new().expect("Failed to create WalletManager");
    ///
    /// let polyseed = "capital chief route liar question fix clutch water outside pave hamster occur always learn license knife".to_string();
    /// let restored_wallet = manager.restore_polyseed(
    ///     wallet_str.clone(),
    ///     "password".to_string(),
    ///     polyseed,
    ///     NetworkType::Mainnet,
    ///     0, // Restore from the beginning of the blockchain.
    ///     1, // Default KDF rounds.
    ///     "".to_string(), // No seed offset.
    ///     true, // Create a new wallet.
    /// ).expect("Failed to restore wallet from polyseed");
    ///
    /// // Use the restored_wallet as needed...
    ///
    /// // Clean up wallet files.
    /// fs::remove_file(&wallet_path).expect("Failed to delete restored wallet");
    /// fs::remove_file(format!("{}.keys", wallet_path.display())).expect("Failed to delete restored wallet keys");
    /// ```
    pub fn restore_polyseed(
        self: &Arc<Self>,
        path: String,
        password: String,
        polyseed: String,
        network_type: NetworkType,
        restore_height: u64,
        kdf_rounds: u64,
        seed_offset: String,
        new_wallet: bool,
    ) -> WalletResult<Wallet> {
        // Convert Rust strings to C-compatible strings.
        let c_path = CString::new(path)
            .map_err(|_| WalletError::FfiError("Invalid path string".to_string()))?;
        let c_password = CString::new(password)
            .map_err(|_| WalletError::FfiError("Invalid password string".to_string()))?;
        let c_polyseed = CString::new(polyseed)
            .map_err(|_| WalletError::FfiError("Invalid mnemonic string".to_string()))?;
        let c_seed_offset = CString::new(seed_offset)
            .map_err(|_| WalletError::FfiError("Invalid seed offset string".to_string()))?;

        unsafe {
            let wallet_ptr = bindings::MONERO_WalletManager_createWalletFromPolyseed(
                self.ptr.as_ptr(),
                c_path.as_ptr(),
                c_password.as_ptr(),
                network_type.to_c_int(),
                c_polyseed.as_ptr(),
                c_seed_offset.as_ptr(),
                new_wallet,
                restore_height,
                kdf_rounds,
            );

            // Check for errors using the returned wallet pointer.
            self.throw_if_error(wallet_ptr)?;
            if wallet_ptr.is_null() {
                return Err(WalletError::NullPointer);
            }

            Ok(Wallet {
                ptr: NonNull::new(wallet_ptr).unwrap(),
                manager: Arc::clone(self),
                is_closed: false,
            })
        }
    }

    /// Checks if a given wallet exists
    pub fn wallet_exists(self, path: PathBuf) -> WalletResult<bool> {
        let c_path = CString::new(path.to_str().unwrap())
            .map_err(|_| WalletError::FfiError("Invalid name".to_string()))?;

        unsafe {
            let exists =
                bindings::MONERO_WalletManager_walletExists(self.ptr.as_ptr(), c_path.as_ptr());
            Ok(exists)
        }
    }

    /// Generates a wallet from provided keys.
    ///
    /// # Example
    ///
    /// ```rust
    /// use monero_c_rust::{WalletManager, NetworkType};
    /// use std::fs;
    /// use std::path::Path;
    /// use tempfile::TempDir;
    ///
    /// let temp_dir = TempDir::new().expect("Failed to create temporary directory");
    /// let wallet_path = temp_dir.path().join("test_wallet");
    /// let wallet_str = wallet_path.to_str().unwrap();
    ///
    /// let manager = WalletManager::new().unwrap();
    /// let wallet = manager.generate_from_keys(
    ///     wallet_str.to_string(),
    ///     "45wsWad9EwZgF3VpxQumrUCRaEtdyyh6NG8sVD3YRVVJbK1jkpJ3zq8WHLijVzodQ22LxwkdWx7fS2a6JzaRGzkNU8K2Dhi".to_string(),
    ///     "".to_string(), // Spend key optional: you can pass either the spend key or the view key.
    ///     "3bc0b202cde92fe5719c3cc0a16aa94f88a5d19f8c515d4e35fae361f6f2120e".to_string(),
    ///     0, // Restore from the beginning of the blockchain.
    ///     "password".to_string(),
    ///     "English".to_string(),
    ///     NetworkType::Mainnet,
    ///     1, // Default KDF rounds.
    /// );
    /// assert!(wallet.is_ok(), "Failed to generate wallet from keys: {:?}", wallet.err());
    ///
    /// // Clean up wallet files.
    /// std::fs::remove_file(wallet_str).expect("Failed to delete test wallet");
    /// std::fs::remove_file(format!("{}.keys", wallet_str)).expect("Failed to delete test wallet keys");
    /// ```
    pub fn generate_from_keys(
        self: &Arc<Self>,
        filename: String,
        address: String,
        spendkey: String,
        viewkey: String,
        restore_height: u64,
        password: String,
        language: String,
        network_type: NetworkType,
        kdf_rounds: u64,
    ) -> WalletResult<Wallet> {
        let c_filename = CString::new(filename)
            .map_err(|_| WalletError::FfiError("Invalid filename".to_string()))?;
        let c_password = CString::new(password)
            .map_err(|_| WalletError::FfiError("Invalid password".to_string()))?;
        let c_language = CString::new(language)
            .map_err(|_| WalletError::FfiError("Invalid language".to_string()))?;
        let c_address = CString::new(address)
            .map_err(|_| WalletError::FfiError("Invalid address".to_string()))?;
        let c_spendkey = CString::new(spendkey)
            .map_err(|_| WalletError::FfiError("Invalid spendkey".to_string()))?;
        let c_viewkey = CString::new(viewkey)
            .map_err(|_| WalletError::FfiError("Invalid viewkey".to_string()))?;

        unsafe {
            let wallet_ptr = bindings::MONERO_WalletManager_createWalletFromKeys(
                self.ptr.as_ptr(),
                c_filename.as_ptr(),
                c_password.as_ptr(),
                c_language.as_ptr(),
                network_type.to_c_int(),
                restore_height,
                c_address.as_ptr(),
                c_viewkey.as_ptr(),
                c_spendkey.as_ptr(),
                kdf_rounds,
            );

            if wallet_ptr.is_null() {
                return Err(WalletError::NullPointer);
            }

            self.throw_if_error(wallet_ptr)?;

            Ok(Wallet {
                ptr: NonNull::new(wallet_ptr).unwrap(),
                manager: Arc::clone(self),
                is_closed: false,
            })
        }
    }

    /// Opens an existing wallet with the provided path, password, and network type.
    ///
    /// # Example
    ///
    /// ```rust
    /// use monero_c_rust::{WalletManager, NetworkType};
    /// use tempfile::TempDir;
    /// use std::fs;
    ///
    /// let temp_dir = TempDir::new().expect("Failed to create temporary directory");
    /// let wallet_path = temp_dir.path().join("test_wallet");
    /// let wallet_str = wallet_path.to_str().unwrap();
    ///
    /// let manager = WalletManager::new().unwrap();
    ///
    /// // First, create a wallet to open later.
    /// let wallet_result = manager.create_wallet(wallet_str, "password", "English", NetworkType::Mainnet);
    /// assert!(wallet_result.is_ok(), "Failed to create wallet: {:?}", wallet_result.err());
    /// let wallet = wallet_result.unwrap();
    ///
    /// // Close the wallet by dropping it.
    /// drop(wallet);
    ///
    /// // Now try to open the existing wallet.
    /// let open_result = manager.open_wallet(wallet_str, "password", NetworkType::Mainnet);
    /// assert!(open_result.is_ok(), "Failed to open wallet: {:?}", open_result.err());
    /// let opened_wallet = open_result.unwrap();
    ///
    /// // Clean up wallet files.
    /// fs::remove_file(wallet_str).expect("Failed to delete test wallet");
    /// fs::remove_file(format!("{}.keys", wallet_str)).expect("Failed to delete test wallet keys");
    /// ```
    pub fn open_wallet(
        self: &Arc<Self>,
        path: PathBuf,
        password: &str,
        network_type: NetworkType,
    ) -> WalletResult<Wallet> {
        let c_path = CString::new(path.into_os_string().into_string().unwrap())
            .map_err(|_| WalletError::FfiError("Invalid path".to_string()))?;
        let c_password = CString::new(password)
            .map_err(|_| WalletError::FfiError("Invalid password".to_string()))?;

        unsafe {
            let wallet_ptr = bindings::MONERO_WalletManager_openWallet(
                self.ptr.as_ptr(),
                c_path.as_ptr(),
                c_password.as_ptr(),
                network_type.to_c_int(),
            );

            self.throw_if_error(wallet_ptr)?;
            if wallet_ptr.is_null() {
                Err(self.get_status(wallet_ptr).unwrap_err())
            } else {
                // Ensuring that we properly close the wallet when it's no longer needed
                let wallet = Wallet {
                    ptr: NonNull::new(wallet_ptr).unwrap(),
                    manager: Arc::clone(self),
                    is_closed: false,
                };
                Ok(wallet)
            }
        }
    }

    /// Retrieves the current blockchain height.
    ///
    /// This method communicates with the connected daemon to obtain the latest
    /// blockchain height. It returns a `BlockHeight` on success or a `WalletError` on failure.
    ///
    /// # Example
    ///
    /// ```rust
    /// use monero_c_rust::{WalletManager, NetworkType};
    ///
    /// let manager = WalletManager::new().unwrap();
    /// let height = manager.get_height().unwrap();
    /// println!("Current blockchain height: {}", height);
    /// ```
    pub fn get_height(&self) -> WalletResult<BlockHeight> {
        unsafe {
            let height = bindings::MONERO_WalletManager_blockchainHeight(self.ptr.as_ptr());
            // Assuming the FFI call does not set an error, directly return the height.
            // If error handling is required, additional checks should be implemented here.
            Ok(height)
        }
    }

    pub fn get_height_target(&self) -> WalletResult<BlockHeight> {
        unsafe {
            let target_height = bindings::MONERO_WalletManager_blockTarget(self.ptr.as_ptr());
            Ok(target_height)
        }
    }
}

impl Wallet {
    /// Retrieves the wallet's seed with an optional offset.
    ///
    /// # Example
    ///
    /// ```
    /// use monero_c_rust::{WalletManager, NetworkType};
    /// use tempfile::TempDir;
    /// use std::fs;
    ///
    /// let temp_dir = TempDir::new().expect("Failed to create temporary directory");
    /// let wallet_path = temp_dir.path().join("test_wallet");
    /// let wallet_str = wallet_path.to_str().unwrap();
    ///
    /// let manager = WalletManager::new().unwrap();
    /// let wallet_result = manager.create_wallet(wallet_str, "password", "English", NetworkType::Mainnet);
    /// assert!(wallet_result.is_ok(), "Failed to create wallet: {:?}", wallet_result.err());
    /// let wallet = wallet_result.unwrap();
    ///
    /// // Get seed with no offset
    /// let seed = wallet.get_seed(None);
    /// assert!(seed.is_ok(), "Failed to get seed: {:?}", seed.err());
    /// let seed = seed.unwrap();
    /// assert!(!seed.is_empty(), "Seed should not be empty");
    ///
    /// // Get seed with an offset
    /// let seed_with_offset = wallet.get_seed(Some("offset"));
    /// assert!(seed_with_offset.is_ok(), "Failed to get seed with offset: {:?}", seed_with_offset.err());
    /// let seed_with_offset = seed_with_offset.unwrap();
    /// assert!(!seed_with_offset.is_empty(), "Seed with offset should not be empty");
    ///
    /// // Clean up wallet files.
    /// fs::remove_file(wallet_str).expect("Failed to delete test wallet");
    /// fs::remove_file(format!("{}.keys", wallet_str)).expect("Failed to delete test wallet keys");
    /// ```
    pub fn get_seed(&self, seed_offset: Option<&str>) -> WalletResult<String> {
        let c_seed_offset = CString::new(seed_offset.unwrap_or(""))
            .map_err(|_| WalletError::FfiError("Invalid seed_offset".to_string()))?;

        unsafe {
            let seed_ptr = bindings::MONERO_Wallet_seed(self.ptr.as_ptr(), c_seed_offset.as_ptr());

            self.throw_if_error()?;
            if seed_ptr.is_null() {
                return Err(self.get_last_error());
            }

            let seed = CStr::from_ptr(seed_ptr).to_string_lossy().into_owned();
            if seed.is_empty() {
                return Err(WalletError::FfiError("Received empty seed".to_string()));
            }

            Ok(seed)
        }
    }

    /// Retrieves the wallet's address for the given account and address index.
    ///
    /// # Example
    ///
    /// ```
    /// use monero_c_rust::{WalletManager, NetworkType};
    /// use tempfile::TempDir;
    /// use std::fs;
    ///
    /// let temp_dir = TempDir::new().expect("Failed to create temporary directory");
    /// let wallet_path = temp_dir.path().join("test_wallet");
    /// let wallet_str = wallet_path.to_str().unwrap();
    ///
    /// let manager = WalletManager::new().unwrap();
    /// let wallet = manager.create_wallet(wallet_str, "password", "English", NetworkType::Mainnet).unwrap();
    /// let address = wallet.get_address(0, 0);
    /// assert!(address.is_ok(), "Failed to get address: {:?}", address.err());
    ///
    /// // Clean up wallet files.
    /// fs::remove_file(wallet_str).expect("Failed to delete test wallet");
    /// fs::remove_file(format!("{}.keys", wallet_str)).expect("Failed to delete test wallet keys");
    /// ```
    pub fn get_address(&self, account_index: u64, address_index: u64) -> WalletResult<String> {
        unsafe {
            let address_ptr =
                bindings::MONERO_Wallet_address(self.ptr.as_ptr(), account_index, address_index);

            self.throw_if_error()?;
            if address_ptr.is_null() {
                Err(self.get_last_error())
            } else {
                let address = CStr::from_ptr(address_ptr).to_string_lossy().into_owned();
                Ok(address)
            }
        }
    }

    /// Checks if the wallet is deterministic.
    ///
    /// # Example
    ///
    /// ```
    /// use monero_c_rust::{WalletManager, NetworkType};
    /// use tempfile::TempDir;
    /// use std::fs;
    ///
    /// let temp_dir = TempDir::new().expect("Failed to create temporary directory");
    /// let wallet_path = temp_dir.path().join("test_wallet");
    /// let wallet_str = wallet_path.to_str().unwrap();
    ///
    /// let manager = WalletManager::new().unwrap();
    /// let wallet_result = manager.create_wallet(wallet_str, "password", "English", NetworkType::Mainnet);
    /// assert!(wallet_result.is_ok(), "Failed to create wallet: {:?}", wallet_result.err());
    /// let wallet = wallet_result.unwrap();
    /// let is_deterministic = wallet.is_deterministic();
    /// assert!(is_deterministic.is_ok(), "Failed to check if wallet is deterministic: {:?}", is_deterministic.err());
    /// assert!(is_deterministic.unwrap(), "Wallet should be deterministic");
    ///
    /// // Clean up wallet files.
    /// fs::remove_file(wallet_str).expect("Failed to delete test wallet");
    /// fs::remove_file(format!("{}.keys", wallet_str)).expect("Failed to delete test wallet keys");
    /// ```
    pub fn is_deterministic(&self) -> WalletResult<bool> {
        unsafe {
            let result = bindings::MONERO_Wallet_isDeterministic(self.ptr.as_ptr());

            self.throw_if_error()?;
            Ok(result)
        }
    }

    /// Retrieves the last error from the wallet.
    ///
    /// # Example
    ///
    /// ```
    /// use monero_c_rust::{WalletManager, NetworkType, WalletError};
    /// let manager = WalletManager::new().unwrap();
    /// // Intentionally pass an invalid wallet to force an error.
    /// let invalid_wallet = manager.create_wallet("", "", "", NetworkType::Mainnet);
    /// if let Err(err) = invalid_wallet {
    ///     if let WalletError::WalletErrorCode(_, error_msg) = err {
    ///         // Check that an error message was produced
    ///         assert!(!error_msg.is_empty(), "Error message should not be empty");
    ///     }
    /// }
    /// ```
    pub fn get_last_error(&self) -> WalletError {
        unsafe {
            let error_ptr = bindings::MONERO_Wallet_errorString(self.ptr.as_ptr());
            let status = bindings::MONERO_Wallet_status(self.ptr.as_ptr());

            let error_msg = if error_ptr.is_null() {
                "Unknown error".to_string()
            } else {
                CStr::from_ptr(error_ptr).to_string_lossy().into_owned()
            };

            WalletError::WalletErrorCode(status, error_msg)
        }
    }

    /// Checks for any errors by inspecting the wallet status and throws an error if found.
    ///
    /// # Returns
    /// - `Ok(())` if no error is found.
    /// - `Err(WalletError)` if an error is encountered.
    pub fn throw_if_error(&self) -> WalletResult<()> {
        let status_result = self.manager.get_status(self.ptr.as_ptr());
        if status_result.is_err() {
            return status_result; // Return the error if the status is not OK.
        }
        Ok(())
    }

    /// Retrieves the balance and unlocked balance for the given account index.
    ///
    /// # Example
    ///
    /// ```
    /// use monero_c_rust::{WalletManager, NetworkType, WalletResult};
    /// use tempfile::TempDir;
    ///
    /// let temp_dir = TempDir::new().expect("Failed to create temporary directory");
    /// let wallet_path = temp_dir.path().join("test_wallet");
    /// let wallet_str = wallet_path.to_str().unwrap();
    ///
    /// let manager = WalletManager::new().unwrap();
    /// let _wallet = manager.create_wallet(wallet_str, "password", "English", NetworkType::Mainnet).unwrap();
    ///
    /// let balance = _wallet.get_balance(0);
    /// assert!(balance.is_ok(), "Failed to get balance: {:?}", balance.err());
    ///
    /// // Clean up wallet files.
    /// std::fs::remove_file(wallet_str).expect("Failed to delete test wallet");
    /// std::fs::remove_file(format!("{}.keys", wallet_str)).expect("Failed to delete test wallet keys");
    /// ```
    pub fn get_balance(&self, account_index: u32) -> WalletResult<GetBalance> {
        unsafe {
            let balance = bindings::MONERO_Wallet_balance(self.ptr.as_ptr(), account_index);

            self.throw_if_error()?;
            let unlocked_balance =
                bindings::MONERO_Wallet_unlockedBalance(self.ptr.as_ptr(), account_index);

            self.throw_if_error()?;
            Ok(GetBalance {
                balance,
                unlocked_balance,
            })
        }
    }

    /// Creates a new subaddress account with the given label.
    ///
    /// # Example
    ///
    /// ```
    /// use monero_c_rust::{WalletManager, NetworkType};
    /// use tempfile::TempDir;
    /// use std::fs;
    ///
    /// // Set up the test environment.
    /// let temp_dir = TempDir::new().expect("Failed to create temporary directory");
    /// let wallet_path = temp_dir.path().join("test_wallet");
    /// let wallet_str = wallet_path.to_str().unwrap();
    ///
    /// // Initialize the wallet manager and create a wallet.
    /// let manager = WalletManager::new().unwrap();
    /// let wallet_result = manager.create_wallet(wallet_str, "password", "English", NetworkType::Mainnet);
    /// assert!(wallet_result.is_ok(), "Failed to create wallet: {:?}", wallet_result.err());
    /// let wallet = wallet_result.unwrap();
    ///
    /// // Create a new account with a label.
    /// let result = wallet.create_account("New Account");
    /// assert!(result.is_ok(), "Failed to create account: {:?}", result.err());
    ///
    /// // Clean up wallet files.
    /// fs::remove_file(wallet_str).expect("Failed to delete test wallet");
    /// fs::remove_file(format!("{}.keys", wallet_str)).expect("Failed to delete test wallet keys");
    /// ```
    pub fn create_account(&self, label: &str) -> WalletResult<()> {
        let c_label =
            CString::new(label).map_err(|_| WalletError::FfiError("Invalid label".to_string()))?;

        unsafe {
            bindings::MONERO_Wallet_addSubaddressAccount(self.ptr.as_ptr(), c_label.as_ptr());
            self.throw_if_error()
        }
    }

    /// Retrieves all accounts associated with the wallet.
    ///
    /// # Example
    ///
    /// ```rust
    /// use monero_c_rust::{WalletManager, NetworkType};
    /// use tempfile::TempDir;
    /// use std::fs;
    ///
    /// let temp_dir = TempDir::new().expect("Failed to create temporary directory");
    /// let wallet_path = temp_dir.path().join("test_wallet");
    /// let wallet_str = wallet_path.to_str().unwrap();
    ///
    /// let manager = WalletManager::new().unwrap();
    /// let wallet = manager.create_wallet(wallet_str, "password", "English", NetworkType::Mainnet).expect("Failed to create wallet");
    ///
    /// // Initially, there should be one account (the primary account).
    /// let initial_accounts = wallet.get_accounts().expect("Failed to retrieve accounts");
    /// assert_eq!(initial_accounts.accounts.len(), 1, "Initial account count mismatch");
    /// assert_eq!(initial_accounts.accounts[0].label, "Primary account", "Expected primary account label");
    ///
    /// // Create additional accounts.
    /// wallet.create_account("Account 1").expect("Failed to create account 1");
    /// wallet.create_account("Account 2").expect("Failed to create account 2");
    ///
    /// // Retrieve all accounts again; we should have three now.
    /// let all_accounts = wallet.get_accounts().expect("Failed to retrieve all accounts");
    /// assert_eq!(all_accounts.accounts.len(), 3, "Expected 3 accounts after creation");
    ///
    /// // Verify the labels of the accounts.
    /// assert_eq!(all_accounts.accounts[0].label, "Primary account", "First account should be the primary account");
    /// assert_eq!(all_accounts.accounts[1].label, "Account 1", "Second account should be 'Account 1'");
    /// assert_eq!(all_accounts.accounts[2].label, "Account 2", "Third account should be 'Account 2'");
    ///
    /// // Clean up wallet files.
    /// fs::remove_file(wallet_str).expect("Failed to delete test wallet");
    /// fs::remove_file(format!("{}.keys", wallet_str)).expect("Failed to delete test wallet keys");
    /// ```
    pub fn get_accounts(&self) -> WalletResult<GetAccounts> {
        unsafe {
            let accounts_size = bindings::MONERO_Wallet_numSubaddressAccounts(self.ptr.as_ptr());
            self.throw_if_error()?;

            let mut accounts = Vec::new();

            for i in 0..accounts_size as u32 {
                let label_ptr = bindings::MONERO_Wallet_getSubaddressLabel(self.ptr.as_ptr(), i, 0);
                let label = if label_ptr.is_null() {
                    "Unnamed".to_string()
                } else {
                    CStr::from_ptr(label_ptr).to_string_lossy().into_owned()
                };

                let balance = bindings::MONERO_Wallet_balance(self.ptr.as_ptr(), i);
                let unlocked_balance =
                    bindings::MONERO_Wallet_unlockedBalance(self.ptr.as_ptr(), i);

                accounts.push(Account {
                    index: i,
                    label,
                    balance,
                    unlocked_balance,
                });
            }

            Ok(GetAccounts { accounts })
        }
    }

    /// Closes the wallet, releasing any resources associated with it.
    ///
    /// After calling this method, the `Wallet` instance should no longer be used.
    ///
    /// # Example
    ///
    /// ```rust
    /// use monero_c_rust::{WalletManager, NetworkType, WalletResult};
    /// use tempfile::TempDir;
    /// use std::fs;
    ///
    /// let temp_dir = TempDir::new().expect("Failed to create temporary directory");
    /// let wallet_path = temp_dir.path().join("test_wallet");
    /// let wallet_str = wallet_path.to_str().unwrap();
    ///
    /// let manager = WalletManager::new().unwrap();
    /// let mut wallet = manager.create_wallet(wallet_str, "password", "English", NetworkType::Mainnet).unwrap();
    ///
    /// // Use the wallet for operations...
    ///
    /// // Now close the wallet
    /// let close_result = wallet.close_wallet();
    /// assert!(close_result.is_ok(), "Failed to close wallet: {:?}", close_result.err());
    ///
    /// // Clean up wallet files.
    /// fs::remove_file(wallet_str).expect("Failed to delete test wallet");
    /// fs::remove_file(format!("{}.keys", wallet_str)).expect("Failed to delete test wallet keys");
    /// ```
    pub fn close_wallet(&mut self) -> WalletResult<()> {
        if self.is_closed {
            return Ok(());
        }
        unsafe {
            let result = bindings::MONERO_WalletManager_closeWallet(
                self.manager.ptr.as_ptr(),
                self.ptr.as_ptr(),
                false, // Don't save the wallet by default.
            );
            if result {
                self.is_closed = true;
                Ok(())
            } else {
                Err(WalletError::FfiError("Failed to close wallet".to_string()))
            }
        }
    }

    /// Initializes the wallet with the provided daemon settings.
    ///
    /// This method must be called after creating or opening a wallet to synchronize it
    /// with the daemon and prepare it for operations like refreshing.
    ///
    /// # Example
    ///
    /// ```rust
    /// use monero_c_rust::{WalletManager, NetworkType, WalletConfig};
    /// use tempfile::TempDir;
    ///
    /// let temp_dir = TempDir::new().expect("Failed to create temporary directory");
    /// let wallet_path = temp_dir.path().join("test_wallet");
    /// let wallet_str = wallet_path.to_str().unwrap();
    ///
    /// let manager = WalletManager::new().unwrap();
    /// let wallet = manager.create_wallet(wallet_str, "password", "English", NetworkType::Mainnet)
    ///     .expect("Failed to create wallet");
    ///
    /// let config = WalletConfig {
    ///     daemon_address: "http://localhost:18081".to_string(),
    ///     upper_transaction_size_limit: 10000,
    ///     daemon_username: "user".to_string(),
    ///     daemon_password: "pass".to_string(),
    ///     use_ssl: false,
    ///     light_wallet: false,
    ///     proxy_address: "".to_string(),
    /// };
    ///
    /// let init_result = wallet.init(config);
    /// assert!(init_result.is_ok(), "Failed to initialize wallet: {:?}", init_result.err());
    /// ```
    pub fn init(&self, config: WalletConfig) -> WalletResult<()> {
        // Store original address for debugging
        let original_address = config.daemon_address.clone();

        let c_daemon_address = CString::new(config.daemon_address)
            .map_err(|_| WalletError::FfiError("Invalid daemon address".to_string()))?;
        let c_daemon_username = CString::new(config.daemon_username)
            .map_err(|_| WalletError::FfiError("Invalid daemon username".to_string()))?;
        let c_daemon_password = CString::new(config.daemon_password)
            .map_err(|_| WalletError::FfiError("Invalid daemon password".to_string()))?;
        let c_proxy_address = CString::new(config.proxy_address)
            .map_err(|_| WalletError::FfiError("Invalid proxy address".to_string()))?;

        unsafe {
            // Debug print the daemon address before conversion
            println!("Original daemon address: {}", original_address);

            // Print the CString pointer and contents
            let daemon_addr_ptr = c_daemon_address.as_ptr();
            println!("Daemon address CString pointer: {:p}", daemon_addr_ptr);
            println!(
                "Daemon address contents: {}",
                CStr::from_ptr(daemon_addr_ptr).to_string_lossy()
            );

            let c_empty = CString::new("").unwrap();
            let c_log_tag = CString::new("moneroc").unwrap();

            // Debug print empty and log tag pointers
            println!("Empty string pointer: {:p}", c_empty.as_ptr());
            println!("Log tag pointer: {:p}", c_log_tag.as_ptr());

            // Store raw pointers for debugging after FFI calls
            let raw_daemon_addr = c_daemon_address.into_raw();
            println!("Raw daemon address pointer: {:p}", raw_daemon_addr);

            // Verify contents right before FFI call
            println!(
                "Raw daemon address contents: {}",
                CStr::from_ptr(raw_daemon_addr).to_string_lossy()
            );

            bindings::MONERO_Wallet_init3(
                self.ptr.as_ptr(),
                c_empty.clone().into_raw(), // Will be freed by C code
                c_log_tag.into_raw(),       // Will be freed by C code
                c_empty.into_raw(),         // Will be freed by C code
                true,
            );

            let result = bindings::MONERO_Wallet_init(
                self.ptr.as_ptr(),
                raw_daemon_addr, // Using our stored raw pointer
                config.upper_transaction_size_limit,
                c_daemon_username.into_raw(), // Will be freed by C code
                c_daemon_password.into_raw(), // Will be freed by C code
                config.use_ssl,
                config.light_wallet,
                c_proxy_address.into_raw(), // Will be freed by C code
            );

            // Try to peek at the contents after FFI call (this is unsafe and just for debugging)
            println!(
                "Attempting to read daemon address after FFI call: {}",
                CStr::from_ptr(raw_daemon_addr).to_string_lossy()
            );

            if result {
                Ok(())
            } else {
                // Print the error if initialization fails
                let error = self.get_last_error();
                println!("Initialization failed with error: {:?}", error);
                Err(error)
            }
        }
    }

    /// Refreshes the wallet's state by synchronizing it with the blockchain.
    ///
    /// # Example
    ///
    /// ```rust
    /// use monero_c_rust::{WalletManager, NetworkType, WalletConfig};
    /// use std::fs;
    /// use tempfile::TempDir;
    ///
    /// fn main() {
    ///     // Create a temporary directory for testing purposes.
    ///     let temp_dir = TempDir::new().expect("Failed to create temporary directory");
    ///     let wallet_path = temp_dir.path().join("test_wallet");
    ///     let wallet_str = wallet_path.to_str().expect("Failed to convert wallet path to string");
    ///
    ///     // Initialize the WalletManager.
    ///     let manager = WalletManager::new().expect("Failed to create WalletManager");
    ///
    ///     // Create a new wallet.
    ///     let wallet = manager
    ///         .create_wallet(wallet_str, "password", "English", NetworkType::Mainnet)
    ///         .expect("Failed to create wallet");
    ///
    ///     // Define the wallet initialization configuration.
    ///     let config = WalletConfig {
    ///         daemon_address: "http://localhost:18081".to_string(),
    ///         upper_transaction_size_limit: 10000,
    ///         daemon_username: "user".to_string(),
    ///         daemon_password: "pass".to_string(),
    ///         use_ssl: false,
    ///         light_wallet: false,
    ///         proxy_address: "".to_string(),
    ///     };
    ///
    ///     // Initialize the wallet with the specified configuration.
    ///     let init_result = wallet.init(config);
    ///     assert!(init_result.is_ok(), "Failed to initialize wallet: {:?}", init_result.err());
    ///
    ///     // Perform a refresh operation after initialization.
    ///     let refresh_result = wallet.refresh();
    ///     assert!(refresh_result.is_ok(), "Failed to refresh wallet: {:?}", refresh_result.err());
    ///
    ///     // Optionally, you can verify the refresh by checking the blockchain height or other metrics.
    ///     // For example:
    ///     let height = manager.get_height().expect("Failed to get blockchain height");
    ///     println!("Current blockchain height: {}", height);
    ///
    ///     // Clean up wallet files.
    ///     fs::remove_file(wallet_str).expect("Failed to delete test wallet");
    ///     fs::remove_file(format!("{}.keys", wallet_str)).expect("Failed to delete test wallet keys");
    /// }
    /// ```
    pub fn refresh(&self) -> WalletResult<Refreshed> {
        unsafe {
            let result = bindings::MONERO_Wallet_refresh(self.ptr.as_ptr());

            if result {
                Ok(Refreshed)
            } else {
                // Retrieve the last error from the wallet
                Err(self.get_last_error())
            }
        }
    }

    pub fn refresh_async(&self) -> WalletResult<Refreshed> {
        unsafe {
            bindings::MONERO_Wallet_refreshAsync(self.ptr.as_ptr());
            return Ok(Refreshed);
        }
    }

    pub fn get_blockchain_height(&self) -> WalletResult<u64> {
        unsafe {
            let height = bindings::MONERO_Wallet_blockChainHeight(self.ptr.as_ptr());
            Ok(height)
        }
    }

    /// Initiates a transfer from the wallet to the specified destinations.
    ///
    /// # Returns
    ///
    /// * `WalletResult<Transfer>` - On success, returns a `Transfer` struct containing transaction details.
    ///   On failure, returns a `WalletError`.
    pub fn transfer(
        &self,
        account_index: u32,
        destinations: Vec<Destination>,
        get_tx_key: bool,
        sweep_all: bool,
    ) -> WalletResult<Transfer> {
        // Define separators
        let separator = ";";
        let separator_c = CString::new(separator)
            .map_err(|_| WalletError::FfiError("Invalid separator".to_string()))?;

        // Concatenate destination addresses and amounts.
        let addresses: Vec<String> = destinations.iter().map(|d| d.address.clone()).collect();
        let address_list = addresses.join(separator);
        let c_address_list = CString::new(address_list)
            .map_err(|_| WalletError::FfiError("Invalid address list".to_string()))?;

        let amounts: Vec<String> = destinations.iter().map(|d| d.amount.to_string()).collect();
        let amount_list = amounts.join(separator);
        let c_amount_list = CString::new(amount_list)
            .map_err(|_| WalletError::FfiError("Invalid amount list".to_string()))?;

        // TODO: Payment IDs.
        let payment_id = CString::new("")
            .map_err(|_| WalletError::FfiError("Invalid payment_id".to_string()))?;
        let mixin_count = 16;

        // Pending transaction priority - default to 0 (Default)
        let pending_tx_priority = bindings::Priority_Default;

        // Subaddress account
        let subaddr_account = account_index;

        // TODO: Preferred inputs.
        let c_preferred_inputs = CString::new("")
            .map_err(|_| WalletError::FfiError("Invalid preferred inputs".to_string()))?;

        // Separator for preferred inputs
        let preferred_inputs_separator = CString::new("")
            .map_err(|_| WalletError::FfiError("Invalid preferred inputs separator".to_string()))?;

        unsafe {
            // Create the transaction with multiple destinations.
            let tx_ptr = bindings::MONERO_Wallet_createTransactionMultDest(
                self.ptr.as_ptr(),
                c_address_list.as_ptr(),
                separator_c.as_ptr(),
                payment_id.as_ptr(),
                sweep_all,
                c_amount_list.as_ptr(),
                separator_c.as_ptr(),
                mixin_count,
                pending_tx_priority,
                subaddr_account,
                c_preferred_inputs.as_ptr(),
                preferred_inputs_separator.as_ptr(),
            );

            // Check for errors.
            let ptr_as_mut_c_void = self.manager.ptr.as_ptr() as *mut c_void;
            self.manager.throw_if_error(ptr_as_mut_c_void)?;
            if tx_ptr.is_null() {
                return Err(WalletError::NullPointer);
            }

            // Get the transaction ID.
            let txid_ptr = bindings::MONERO_PendingTransaction_txid(tx_ptr, separator_c.as_ptr());
            if txid_ptr.is_null() {
                return Err(WalletError::FfiError(
                    "Failed to get transaction ID".to_string(),
                ));
            }
            let txid = CStr::from_ptr(txid_ptr).to_string_lossy().into_owned();

            // Get the fee.
            let fee = bindings::MONERO_PendingTransaction_fee(tx_ptr);

            // Optionally get the transaction key.
            let tx_key = if get_tx_key {
                let c_txid = CString::new(txid.clone())
                    .map_err(|_| WalletError::FfiError("Invalid txid".to_string()))?;
                let tx_key_ptr =
                    bindings::MONERO_Wallet_getTxKey(self.ptr.as_ptr(), c_txid.as_ptr());
                if tx_key_ptr.is_null() {
                    None
                } else {
                    Some(CStr::from_ptr(tx_key_ptr).to_string_lossy().into_owned())
                }
            } else {
                None
            };

            // Submit the transaction.
            //
            // TODO: Make submission optional.
            let tx_ptr_as_i8 = tx_ptr as *const i8;
            let submit_result =
                bindings::MONERO_Wallet_submitTransaction(self.ptr.as_ptr(), tx_ptr_as_i8);
            if !submit_result {
                return Err(WalletError::FfiError(
                    "Failed to submit transaction".to_string(),
                ));
            }

            Ok(Transfer {
                txid,
                tx_key,
                amount: destinations.iter().map(|d| d.amount).sum(),
                fee,
            })
        }
    }

    /// Sweep all funds from the specific account to the specified destination.
    ///
    /// TODO: Example / docs-tests.
    pub fn sweep_all(
        &self,
        account_index: u32,
        destination: Destination,
        get_tx_key: bool,
    ) -> WalletResult<Transfer> {
        // Convert the destination address to a CString.
        let c_address = CString::new(destination.address.clone())
            .map_err(|_| WalletError::FfiError("Invalid address".to_string()))?;

        // Placeholder values for fields not needed in sweep_all.
        let empty_separator =
            CString::new("").map_err(|_| WalletError::FfiError("Invalid separator".to_string()))?;
        let payment_id = CString::new("")
            .map_err(|_| WalletError::FfiError("Invalid payment_id".to_string()))?;
        let mixin_count = 16;
        let pending_tx_priority = bindings::Priority_Default;
        let c_preferred_inputs = CString::new("")
            .map_err(|_| WalletError::FfiError("Invalid preferred inputs".to_string()))?;
        let preferred_inputs_separator = CString::new("")
            .map_err(|_| WalletError::FfiError("Invalid preferred inputs separator".to_string()))?;

        unsafe {
            // Create the sweep transaction.
            let tx_ptr = bindings::MONERO_Wallet_createTransactionMultDest(
                self.ptr.as_ptr(),
                c_address.as_ptr(),
                empty_separator.as_ptr(),
                payment_id.as_ptr(),
                true, // Sweep all funds.
                empty_separator.as_ptr(),
                empty_separator.as_ptr(),
                mixin_count,
                pending_tx_priority,
                account_index,
                c_preferred_inputs.as_ptr(),
                preferred_inputs_separator.as_ptr(),
            );

            // Check for errors.
            let ptr_as_mut_c_void = self.manager.ptr.as_ptr() as *mut c_void;
            self.manager.throw_if_error(ptr_as_mut_c_void)?;
            if tx_ptr.is_null() {
                return Err(WalletError::NullPointer);
            }

            // Get the transaction ID.
            let txid_ptr =
                bindings::MONERO_PendingTransaction_txid(tx_ptr, empty_separator.as_ptr());
            if txid_ptr.is_null() {
                return Err(WalletError::FfiError(
                    "Failed to get transaction ID".to_string(),
                ));
            }
            let txid = CStr::from_ptr(txid_ptr).to_string_lossy().into_owned();

            // Get the fee.
            let fee = bindings::MONERO_PendingTransaction_fee(tx_ptr);

            // Optionally get the transaction key.
            let tx_key = if get_tx_key {
                let c_txid = CString::new(txid.clone())
                    .map_err(|_| WalletError::FfiError("Invalid txid".to_string()))?;
                let tx_key_ptr =
                    bindings::MONERO_Wallet_getTxKey(self.ptr.as_ptr(), c_txid.as_ptr());
                if tx_key_ptr.is_null() {
                    None
                } else {
                    Some(CStr::from_ptr(tx_key_ptr).to_string_lossy().into_owned())
                }
            } else {
                None
            };

            // Submit the transaction.
            //
            // TODO: Make submission optional.
            let tx_ptr_as_i8 = tx_ptr as *const i8;
            let submit_result =
                bindings::MONERO_Wallet_submitTransaction(self.ptr.as_ptr(), tx_ptr_as_i8);
            if !submit_result {
                return Err(WalletError::FfiError(
                    "Failed to submit sweep transaction".to_string(),
                ));
            }

            Ok(Transfer {
                txid,
                tx_key,
                amount: 0, // Since it's sweeping all, amount is not predefined.
                fee,
            })
        }
    }

    /// Sets the seed language for the wallet.
    ///
    /// # Example
    ///
    /// ```rust
    /// use monero_c_rust::{WalletManager, NetworkType};
    /// use tempfile::TempDir;
    /// use std::fs;
    ///
    /// let temp_dir = TempDir::new().expect("Failed to create temporary directory");
    /// let wallet_path = temp_dir.path().join("test_wallet");
    /// let wallet_str = wallet_path.to_str().unwrap();
    ///
    /// let manager = WalletManager::new().unwrap();
    /// let wallet = manager.create_wallet(wallet_str, "password", "English", NetworkType::Mainnet).unwrap();
    ///
    /// // Change the seed language to Spanish
    /// let result = wallet.set_seed_language("Spanish");
    /// assert!(result.is_ok(), "Failed to set seed language: {:?}", result.err());
    ///
    /// // Clean up wallet files.
    /// fs::remove_file(wallet_str).expect("Failed to delete test wallet");
    /// fs::remove_file(format!("{}.keys", wallet_str)).expect("Failed to delete test wallet keys");
    /// ```
    pub fn set_seed_language(&self, language: &str) -> WalletResult<()> {
        let c_language = CString::new(language)
            .map_err(|_| WalletError::FfiError("Invalid language string".to_string()))?;

        unsafe {
            bindings::MONERO_Wallet_setSeedLanguage(self.ptr.as_ptr(), c_language.as_ptr());
            self.throw_if_error()
        }
    }

    /// Checks the validity of a transaction key for a given transaction ID and address.
    ///
    /// This method verifies whether the provided transaction key (`tx_key`) is valid for the
    /// specified transaction ID (`txid`) and address (`address`). If valid, it returns the
    /// amount received, whether the transaction is in the pool, and the number of confirmations.
    ///
    /// TODO: Example / docs-tests.
    pub fn check_tx_key(
        &self,
        txid: String,
        tx_key: String,
        address: String,
        received: Option<u64>,
        in_pool: Option<bool>,
        confirmations: Option<u64>,
    ) -> WalletResult<CheckTxKey> {
        // Convert Rust strings to C-compatible strings.
        let c_txid = CString::new(txid)
            .map_err(|_| WalletError::FfiError("Invalid txid string".to_string()))?;
        let c_tx_key = CString::new(tx_key)
            .map_err(|_| WalletError::FfiError("Invalid tx_key string".to_string()))?;
        let c_address = CString::new(address)
            .map_err(|_| WalletError::FfiError("Invalid address string".to_string()))?;

        // Assign default values if optional parameters are not provided.
        let received_val = received.unwrap_or(0);
        let in_pool_val = in_pool.unwrap_or(false);
        let confirmations_val = confirmations.unwrap_or(0);

        // Call the C function.
        let result = unsafe {
            bindings::MONERO_Wallet_checkTxKey(
                self.ptr.as_ptr(),
                c_txid.as_ptr(),
                c_tx_key.as_ptr(),
                c_address.as_ptr(),
                received_val,
                in_pool_val,
                confirmations_val,
            )
        };

        if result {
            Ok(CheckTxKey {
                valid: true,
                error: None,
            })
        } else {
            // Retrieve the last error.
            Err(WalletError::FfiError(
                "Transaction key is invalid.".to_string(),
            ))
        }
    }
}

#[derive(Debug)]
pub struct GetBalance {
    pub balance: u64,
    pub unlocked_balance: u64,
}

impl fmt::Display for GetBalance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut total = Decimal::from(self.balance);
        total
            .set_scale(12)
            .expect("12 is smaller than max precision of 28");

        let mut unlocked = Decimal::from(self.unlocked_balance);
        unlocked
            .set_scale(12)
            .expect("12 is smaller than max precision of 28");

        write!(
            f,
            "total balance: {}, unlocked balance: {}",
            total, unlocked
        )
    }
}

impl Drop for Wallet {
    fn drop(&mut self) {
        if !self.is_closed {
            let _ = self.close_wallet();
        }
    }
}

#[cfg(test)]
use std::fs;
#[cfg(test)]
use tempfile::TempDir;

#[cfg(test)]
fn check_and_delete_existing_wallets(temp_dir: &TempDir) -> std::io::Result<()> {
    let test_wallet_names = &[
        "test_wallet",
        "mainnet_wallet",
        "testnet_wallet",
        "stagenet_wallet",
    ];

    for name in test_wallet_names {
        let wallet_file = temp_dir.path().join(name);
        let keys_file = temp_dir.path().join(format!("{}.keys", name));

        if wallet_file.exists() {
            fs::remove_file(&wallet_file)?;
        }
        if keys_file.exists() {
            fs::remove_file(&keys_file)?;
        }
    }
    Ok(())
}

#[cfg(test)]
fn setup() -> WalletResult<(Arc<WalletManager>, TempDir)> {
    let temp_dir = tempfile::tempdir().expect("Failed to create temporary directory");
    check_and_delete_existing_wallets(&temp_dir).expect("Failed to clean up existing wallets");

    let manager = WalletManager::new()?;
    Ok((manager, temp_dir))
}

#[cfg(test)]
fn teardown(temp_dir: &TempDir) -> std::io::Result<()> {
    check_and_delete_existing_wallets(temp_dir)
}

#[test]
fn test_wallet_manager_creation() {
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path
        .to_str()
        .expect("Failed to convert wallet path to string");

    let wallet_result =
        manager.create_wallet(wallet_str, "password", "English", NetworkType::Mainnet);
    assert!(wallet_result.is_ok(), "WalletManager creation failed");

    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_wallet_creation() {
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path
        .to_str()
        .expect("Failed to convert wallet path to string");

    let wallet = manager.create_wallet(wallet_str, "password", "English", NetworkType::Mainnet);
    assert!(wallet.is_ok(), "Failed to create wallet");

    let wallet = wallet.unwrap();

    assert!(
        wallet.is_deterministic().is_ok(),
        "Wallet creation seems to have failed"
    );

    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_get_seed() {
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path
        .to_str()
        .expect("Failed to convert wallet path to string");

    // Create a new wallet.
    let wallet = manager
        .create_wallet(wallet_str, "password", "English", NetworkType::Mainnet)
        .expect("Failed to create wallet");

    // Test getting seed with no offset (None).
    let result = wallet.get_seed(None);
    assert!(
        result.is_ok(),
        "Failed to get seed without offset: {:?}",
        result.err()
    );
    assert!(!result.unwrap().is_empty(), "Seed without offset is empty");

    // Test getting seed with a specific offset (Some("offset")).
    let result_with_offset = wallet.get_seed(Some("offset"));
    assert!(
        result_with_offset.is_ok(),
        "Failed to get seed with offset: {:?}",
        result_with_offset.err()
    );
    assert!(
        !result_with_offset.unwrap().is_empty(),
        "Seed with offset is empty"
    );

    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_get_address() {
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path
        .to_str()
        .expect("Failed to convert wallet path to string");

    let wallet = manager
        .create_wallet(wallet_str, "password", "English", NetworkType::Mainnet)
        .expect("Failed to create wallet");
    let result = wallet.get_address(0, 0);
    assert!(result.is_ok(), "Failed to get address: {:?}", result.err());
    assert!(!result.unwrap().is_empty(), "Address is empty");

    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_is_deterministic() {
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path
        .to_str()
        .expect("Failed to convert wallet path to string");

    let wallet = manager
        .create_wallet(wallet_str, "password", "English", NetworkType::Mainnet)
        .expect("Failed to create wallet");
    let result = wallet.is_deterministic();
    assert!(
        result.is_ok(),
        "Failed to check if wallet is deterministic: {:?}",
        result.err()
    );
    assert!(result.unwrap(), "Wallet should be deterministic");

    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_wallet_creation_with_different_networks() {
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    let wallets = vec![
        ("mainnet_wallet", NetworkType::Mainnet),
        ("testnet_wallet", NetworkType::Testnet),
        ("stagenet_wallet", NetworkType::Stagenet),
    ];

    for (name, net_type) in wallets {
        let wallet_path = temp_dir.path().join(name);
        let wallet_str = wallet_path
            .to_str()
            .expect("Failed to convert wallet path to string");

        let wallet = manager.create_wallet(wallet_str, "password", "English", net_type);
        assert!(wallet.is_ok(), "Failed to create wallet: {}", name);
    }

    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_restore_mnemonic_success() {
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path
        .to_str()
        .expect("Failed to convert wallet path to string")
        .to_string();

    // Example mnemonic seed (ensure this is a valid seed for your context).
    let mnemonic_seed = "hemlock jubilee eden hacksaw boil superior inroads epoxy exhale orders cavernous second brunt saved richly lower upgrade hitched launching deepest mostly playful layout lower eden".to_string();

    let wallet = manager.restore_mnemonic(
        wallet_str.clone(),
        "password".to_string(),
        mnemonic_seed,
        NetworkType::Mainnet,
        0,              // Restore from the beginning of the blockchain.
        1,              // Default KDF rounds.
        "".to_string(), // No seed offset.
    );
    assert!(
        wallet.is_ok(),
        "Failed to restore wallet: {:?}",
        wallet.err()
    );

    // Clean up wallet files.
    teardown(&temp_dir).expect("Failed to clean up after test");
}
// TODO: Test with offset.

#[test]
fn test_restore_polyseed_success() {
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path
        .to_str()
        .expect("Failed to convert wallet path to string")
        .to_string();
    let polyseed = "capital chief route liar question fix clutch water outside pave hamster occur always learn license knife".to_string();

    let restored_wallet = manager.restore_polyseed(
        wallet_str.clone(),
        "password".to_string(),
        polyseed.clone(),
        NetworkType::Mainnet,
        0,              // Restore from the beginning of the blockchain.
        1,              // Default KDF rounds.
        "".to_string(), // No seed offset.
        true,           // Create a new wallet.
    );
    assert!(
        restored_wallet.is_ok(),
        "Failed to restore wallet from polyseed: {:?}",
        restored_wallet.err()
    );

    // Clean up wallet files.
    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_generate_from_keys_unit() {
    println!("Running unit test: test_generate_from_keys_unit");
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    let wallet_path = temp_dir.path().join("generated_wallet_unit");
    let wallet_str = wallet_path
        .to_str()
        .expect("Failed to convert wallet path to string");

    // Test parameters.
    //
    // TODO add functions to get spend and view keys.
    let address = "45wsWad9EwZgF3VpxQumrUCRaEtdyyh6NG8sVD3YRVVJbK1jkpJ3zq8WHLijVzodQ22LxwkdWx7fS2a6JzaRGzkNU8K2Dhi";
    let spendkey = "29adefc8f67515b4b4bf48031780ab9d071d24f8a674b879ce7f245c37523807";
    let viewkey = "3bc0b202cde92fe5719c3cc0a16aa94f88a5d19f8c515d4e35fae361f6f2120e";
    let restore_height = 0;
    let password = "password";
    let language = "English";
    let network_type = NetworkType::Mainnet;
    let kdf_rounds = 1;

    let result = manager.generate_from_keys(
        wallet_str.to_string(),
        address.to_string(),
        spendkey.to_string(),
        viewkey.to_string(),
        restore_height,
        password.to_string(),
        language.to_string(),
        network_type,
        kdf_rounds,
    );
    assert!(
        result.is_ok(),
        "Failed to generate wallet from keys: {:?}",
        result.err()
    );

    // Clean up wallet files.
    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_multiple_address_generation() {
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path
        .to_str()
        .expect("Failed to convert wallet path to string");

    let wallet = manager
        .create_wallet(wallet_str, "password", "English", NetworkType::Mainnet)
        .expect("Failed to create wallet");

    for i in 0..5 {
        let result = wallet.get_address(0, i);
        assert!(
            result.is_ok(),
            "Failed to get address {}: {:?}",
            i,
            result.err()
        );
        assert!(!result.unwrap().is_empty(), "Address {} is empty", i);
    }

    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_wallet_error_display() {
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
        }
        _ => panic!("Expected WalletErrorCode variant"),
    }
}

#[test]
fn test_wallet_status() {
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path
        .to_str()
        .expect("Failed to convert wallet path to string");

    // Create a wallet to use for status checking
    let wallet = manager
        .create_wallet(wallet_str, "password", "English", NetworkType::Mainnet)
        .expect("Failed to create wallet");

    // Check the status of the wallet, expecting it to be OK
    let status_result = manager.get_status(wallet.ptr.as_ptr());
    assert!(
        status_result.is_ok(),
        "Failed to get status: {:?}",
        status_result.err()
    );

    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_open_wallet() {
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path
        .to_str()
        .expect("Failed to convert wallet path to string");

    // Create a wallet to be opened later
    let wallet = manager
        .create_wallet(wallet_str, "password", "English", NetworkType::Mainnet)
        .expect("Failed to create wallet");

    // Drop the wallet so it can be opened later
    drop(wallet);

    // Try to open the wallet
    let open_result = manager.open_wallet(wallet_str, "password", NetworkType::Mainnet);
    assert!(
        open_result.is_ok(),
        "Failed to open wallet: {:?}",
        open_result.err()
    );

    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_get_balance() {
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path
        .to_str()
        .expect("Failed to convert wallet path to string");

    let wallet = manager
        .create_wallet(wallet_str, "password", "English", NetworkType::Mainnet)
        .unwrap();

    let balance_result = wallet.get_balance(0);
    assert!(
        balance_result.is_ok(),
        "Failed to get balance: {:?}",
        balance_result.err()
    );

    let _balance = balance_result.unwrap();
    // assert!(_balance.balance >= 0, "Balance should be non-negative");
    // assert!(_balance.unlocked_balance >= 0, "Unlocked balance should be non-negative");
    // These assertions are meaningless with the constraints of the type.

    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_create_account() {
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path
        .to_str()
        .expect("Failed to convert wallet path to string");

    // Create a wallet.
    let wallet = manager
        .create_wallet(wallet_str, "password", "English", NetworkType::Mainnet)
        .expect("Failed to create wallet");

    // Create a new account.
    let result = wallet.create_account("Test Account");
    assert!(
        result.is_ok(),
        "Failed to create account: {:?}",
        result.err()
    );

    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_get_accounts() {
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path
        .to_str()
        .expect("Failed to convert wallet path to string");

    let wallet = manager
        .create_wallet(wallet_str, "password", "English", NetworkType::Mainnet)
        .expect("Failed to create wallet");

    // Add two accounts for testing
    wallet
        .create_account("Test Account 1")
        .expect("Failed to create account 1");
    wallet
        .create_account("Test Account 2")
        .expect("Failed to create account 2");

    // Retrieve all accounts
    let accounts = wallet.get_accounts().expect("Failed to retrieve accounts");
    assert_eq!(accounts.accounts.len(), 3); // Including the primary account

    // Check account names
    assert_eq!(accounts.accounts[0].label, "Primary account");
    assert_eq!(accounts.accounts[1].label, "Test Account 1");
    assert_eq!(accounts.accounts[2].label, "Test Account 2");

    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_close_wallet() {
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path
        .to_str()
        .expect("Failed to convert wallet path to string");

    // Create a wallet.
    let mut wallet = manager
        .create_wallet(wallet_str, "password", "English", NetworkType::Mainnet)
        .expect("Failed to create wallet");

    // Close the wallet.
    let close_result = wallet.close_wallet();
    assert!(
        close_result.is_ok(),
        "Failed to close wallet: {:?}",
        close_result.err()
    );

    // Attempt to close the wallet again.
    let close_again_result = wallet.close_wallet();
    assert!(
        close_again_result.is_ok(),
        "Failed to close wallet a second time: {:?}",
        close_again_result.err()
    );

    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_height_success() {
        let manager = WalletManager::new().unwrap();
        let height = manager.get_height().unwrap();
        // assert!(height > 0, "Blockchain height should be greater than 0");
        // The test should not assume network connectivity/any syncing progress, so:
        assert!(height == 0, "Blockchain height should be equal to 0");
    }
}

#[test]
fn test_init_success() {
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path
        .to_str()
        .expect("Failed to convert wallet path to string");

    // Create a wallet.
    let wallet = manager
        .create_wallet(wallet_str, "password", "English", NetworkType::Mainnet)
        .expect("Failed to create wallet");

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

    // Initialize the wallet.
    let init_result = wallet.init(config);
    assert!(
        init_result.is_ok(),
        "Failed to initialize wallet: {:?}",
        init_result.err()
    );

    // Clean up wallet files.
    fs::remove_file(wallet_str).expect("Failed to delete test wallet");
    fs::remove_file(format!("{}.keys", wallet_str)).expect("Failed to delete test wallet keys");

    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_refresh_success() {
    println!("Running test_refresh_success");
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    // Construct the full path for the wallet within temp_dir.
    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path
        .to_str()
        .expect("Failed to convert wallet path to string");

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
    let init_result = wallet.init(config);

    assert!(
        init_result.is_ok(),
        "Failed to initialize wallet: {:?}",
        init_result.err()
    );

    // Perform a refresh operation after initialization.
    println!("Refreshing the wallet...");
    let refresh_result = wallet.refresh();
    assert!(
        refresh_result.is_ok(),
        "Failed to refresh wallet: {:?}",
        refresh_result.err()
    );

    // Clean up wallet files.
    fs::remove_file(wallet_str).expect("Failed to delete test wallet");
    fs::remove_file(format!("{}.keys", wallet_str)).expect("Failed to delete test wallet keys");

    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_set_seed_language() {
    let (manager, temp_dir) = setup().expect("Failed to set up test environment");

    let wallet_path = temp_dir.path().join("test_wallet_set_seed_language");
    let wallet_str = wallet_path
        .to_str()
        .expect("Failed to convert wallet path to string");

    // Create a new wallet.
    let wallet = manager
        .create_wallet(wallet_str, "password", "English", NetworkType::Mainnet)
        .expect("Failed to create wallet");

    // Set the seed language to Spanish.
    let result = wallet.set_seed_language("Spanish");
    assert!(
        result.is_ok(),
        "Failed to set seed language: {:?}",
        result.err()
    );

    // Optionally, retrieve the seed language to verify it was set correctly.
    // This requires implementing a corresponding `get_seed_language` method.
    // For now, we'll assume that if no error was returned, the operation was successful.

    teardown(&temp_dir).expect("Failed to clean up after test");
}

#[test]
fn test_check_tx_key() {
    let temp_dir = TempDir::new().expect("Failed to create temporary directory");
    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path
        .to_str()
        .expect("Failed to convert wallet path to string")
        .to_string();

    let mnemonic_seed = "capital chief route liar question fix clutch water outside pave hamster occur always learn license knife".to_string();
    let passphrase = "".to_string();

    // Restore the wallet using polyseed.
    let wallet = WalletManager::new()
        .expect("Failed to create WalletManager")
        .restore_polyseed(
            wallet_str.clone(),
            "password".to_string(),
            mnemonic_seed.clone(),
            NetworkType::Mainnet,
            0,
            1,
            passphrase.clone(),
            true,
        )
        .expect("Failed to restore wallet from polyseed");

    // Print the primary address.
    println!(
        "Primary address: {}",
        wallet.get_address(0, 0).expect("Failed to get address")
    );

    // Valid transaction details.
    let valid_txid = "b3f1b71f5127f9d655e58f7a2b324a64bfbc5a3ea1ce8846a0f4c51cbcb87ea6".to_string();
    let valid_tx_key =
        "48ef9d8b772c4f5097e29a4ba413605497d978c74e879fda67545dddff312b0a".to_string();
    let valid_address = "465cUW8wTMSCV8oVVh7CuWWHs7yeB1oxhNPrsEM5FKSqadTXmobLqsNEtRnyGsbN1rbDuBtWdtxtXhTJda1Lm9vcH2ZdrD1".to_string();

    // Check the transaction key.
    let valid_check = wallet.check_tx_key(
        valid_txid.clone(),
        valid_tx_key.clone(),
        valid_address.clone(),
        Some(1),
        Some(false),
        Some(10),
    );

    match valid_check {
        Ok(check) => {
            assert!(check.valid, "Valid transaction key should be valid.");
            assert!(
                check.error.is_none(),
                "There should be no error for valid transaction key."
            );
            println!("Valid transaction key check passed.");
        }
        Err(e) => {
            panic!("Error checking valid transaction key: {:?}", e);
        }
    }

    // Clean up wallet files.
    fs::remove_file(&wallet_path).expect("Failed to delete test wallet");
    fs::remove_file(format!("{}.keys", wallet_path.display()))
        .expect("Failed to delete test wallet keys");
}

#[test]
fn test_invalid_check_tx_key() {
    let temp_dir = TempDir::new().expect("Failed to create temporary directory");
    let wallet_path = temp_dir.path().join("test_wallet");
    let wallet_str = wallet_path
        .to_str()
        .expect("Failed to convert wallet path to string")
        .to_string();

    let mnemonic_seed = "capital chief route liar question fix clutch water outside pave hamster occur always learn license knife".to_string();
    let passphrase = "".to_string();

    // Restore the wallet using polyseed.
    let wallet = WalletManager::new()
        .expect("Failed to create WalletManager")
        .restore_polyseed(
            wallet_str.clone(),
            "password".to_string(),
            mnemonic_seed.clone(),
            NetworkType::Mainnet,
            0,
            1,
            passphrase.clone(),
            true,
        )
        .expect("Failed to restore wallet from polyseed");

    // Print the primary address.
    println!(
        "Primary address: {}",
        wallet.get_address(0, 0).expect("Failed to get address")
    );

    // Invalid transaction details.
    let invalid_txid = "invalid_tx_id".to_string();
    let invalid_tx_key = "invalid_tx_key".to_string();
    let invalid_address = "invalid_address".to_string();

    // Check the invalid transaction key.
    let invalid_check = wallet.check_tx_key(
        invalid_txid.clone(),
        invalid_tx_key.clone(),
        invalid_address.clone(),
        Some(1),
        Some(false),
        Some(10),
    );

    match invalid_check {
        Ok(check) => {
            assert!(!check.valid, "Invalid transaction key should be invalid.");
            assert!(
                check.error.is_some(),
                "There should be an error message for invalid transaction key."
            );
            println!("Invalid transaction key check correctly identified as invalid.");
        }
        Err(e) => {
            println!("Expected error for invalid transaction key: {:?}", e);
        }
    }

    // Clean up wallet files.
    fs::remove_file(&wallet_path).expect("Failed to delete test wallet");
    fs::remove_file(format!("{}.keys", wallet_path.display()))
        .expect("Failed to delete test wallet keys");
}
