#![warn(
    unused_extern_crates,
    missing_debug_implementations,
    missing_copy_implementations,
    rust_2018_idioms,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::fallible_impl_from,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap,
    clippy::dbg_macro
)]
#![forbid(unsafe_code)]

//! # bitcoin-harness
//! A simple lib to start a bitcoind container, generate blocks and funds addresses.
//! Note: It uses tokio.
//!
//! # Examples
//!
//! ## Just connect to bitcoind and get the network
//!
//! ```rust
//! use bitcoin_harness::{Bitcoind, bitcoind_rpc, Client};
//!
//! # #[tokio::main]
//! # async fn main() {
//! let tc_client = testcontainers::clients::Cli::default();
//! let bitcoind = Bitcoind::new(&tc_client).unwrap();
//! let client = Client::new(bitcoind.node_url);
//! let network = client.network().await.unwrap();
//!
//! assert_eq!(network, bitcoin::Network::Regtest)
//! # }
//! ```
//!
//! ## Create a wallet, fund it and get a UTXO
//!
//! ```rust
//! use bitcoin_harness::{Bitcoind, bitcoind_rpc, Client, Wallet};
//!
//! # #[tokio::main]
//! # async fn main() {
//! let tc_client = testcontainers::clients::Cli::default();
//! let bitcoind = Bitcoind::new(&tc_client).unwrap();
//! let client = Client::new(bitcoind.node_url.clone());
//!
//! bitcoind.init(5).await.unwrap();
//!
//! let wallet = Wallet::new("my_wallet", bitcoind.node_url.clone()).await.unwrap();
//! let address = wallet.new_address().await.unwrap();
//! let amount = bitcoin::Amount::from_btc(3.0).unwrap();
//!
//! bitcoind.mint(address, amount).await.unwrap();
//!
//! let balance = wallet.balance().await.unwrap();
//!
//! assert_eq!(balance, amount);
//!
//! let utxos = wallet.list_unspent().await.unwrap();
//!
//! assert_eq!(utxos.get(0).unwrap().amount, amount);
//! # }
//! ```

pub mod bitcoind_rpc;
pub mod bitcoind_rpc_api;
pub mod img;
pub mod wallet;

use reqwest::Url;
use std::time::Duration;
use testcontainers::{clients, Container};

pub use crate::bitcoind_rpc::Client;
pub use crate::bitcoind_rpc_api::BitcoindRpcApi;
pub use crate::img::BitcoinCore;
pub use crate::wallet::Wallet;

pub type Result<T> = std::result::Result<T, Error>;

const BITCOIND_RPC_PORT: u16 = 18443;

#[derive(Debug)]
pub struct Bitcoind<'c> {
    pub container: Container<'c, BitcoinCore>,
    pub node_url: Url,
    pub wallet_name: String,
}

impl<'c> Bitcoind<'c> {
    /// Starts a new regtest bitcoind container
    pub fn new(client: &'c clients::Cli) -> Result<Self> {
        let container = client.run(BitcoinCore);
        let port = container.get_host_port_ipv4(BITCOIND_RPC_PORT);

        let auth = &container.image_args().rpc_auth;
        let url = format!(
            "http://{}:{}@localhost:{}",
            auth.username(),
            auth.password(),
            port
        );
        let url = Url::parse(&url)?;

        let wallet_name = String::from("testwallet");

        Ok(Self {
            container,
            node_url: url,
            wallet_name,
        })
    }

    /// Create a test wallet, generate enough block to fund it and activate segwit.
    /// Generate enough blocks to make the passed `spendable_quantity` spendable.
    /// Spawn a tokio thread to mine a new block every second.
    pub async fn init(&self, spendable_quantity: u32) -> Result<()> {
        let bitcoind_client = Client::new(self.node_url.clone());

        bitcoind_client
            .createwallet(&self.wallet_name, None, None, None, None)
            .await?;

        let reward_address = bitcoind_client
            .with_wallet(&self.wallet_name)?
            .getnewaddress(None, None)
            .await?;

        let reward_address = reward_address
            .require_network(bitcoind_client.network().await?)?;

        bitcoind_client
            .generatetoaddress(101 + spendable_quantity, reward_address.clone())
            .await?;
        let _ = tokio::spawn(mine(bitcoind_client, reward_address));

        Ok(())
    }

    /// Send Bitcoin to the specified address, limited to the spendable bitcoin quantity.
    pub async fn mint(&self, address: bitcoin::Address, amount: bitcoin::Amount) -> Result<()> {
        let bitcoind_client = Client::new(self.node_url.clone());

        bitcoind_client
            .send_to_address(&self.wallet_name, address.clone(), amount)
            .await?;

        // Confirm the transaction
        let reward_address = bitcoind_client
            .with_wallet(&self.wallet_name)?
            .getnewaddress(None, None)
            .await?;

        let reward_address = reward_address
            .require_network(bitcoind_client.network().await?)?; 

        bitcoind_client.generatetoaddress(1, reward_address).await?;

        Ok(())
    }

    pub fn container_id(&self) -> &str {
        self.container.id()
    }
}

async fn mine(bitcoind_client: Client, reward_address: bitcoin::Address) -> Result<()> {
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        bitcoind_client
            .generatetoaddress(1, reward_address.clone())
            .await?;
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Bitcoin Rpc: ")]
    BitcoindRpc(#[from] bitcoind_rpc::Error),
    #[error("Json Rpc: ")]
    JsonRpc(#[from] jsonrpc_client::Error<reqwest::Error>),
    #[error("Url Parsing: ")]
    UrlParseError(#[from] url::ParseError),
    #[error("Address Parsing: ")]
    AddressParseError(#[from] bitcoin::address::ParseError),
    #[error("Txid Parsing: ")]
    TxidParseError(#[from] bitcoin::hashes::hex::error::HexToArrayError),
    #[error("Docker port not exposed: ")]
    PortNotExposed(u16),
}
