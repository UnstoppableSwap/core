//! An incomplete async bitcoind rpc client that supports multi-wallet features

use ::bitcoin::{hashes::hex::FromHex, Address, Amount, Network, Transaction, Txid};
use bitcoincore_rpc_json::{FinalizePsbtResult, GetAddressInfoResult};
use jsonrpc_client::{JsonRpcError, ResponsePayload, SendRequest};
use reqwest::Url;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::collections::HashMap;
use std::str::FromStr;

pub use crate::bitcoind_rpc_api::*;
pub use jsonrpc_client;

pub type Result<T> = std::result::Result<T, Error>;

pub const JSONRPC_VERSION: &str = "1.0";

#[jsonrpc_client::implement(BitcoindRpcApi)]
#[derive(Debug, Clone)]
pub struct Client {
    inner: reqwest::Client,
    base_url: reqwest::Url,
}

impl Client {
    pub fn new(url: Url) -> Self {
        Client {
            inner: reqwest::Client::new(),
            base_url: url,
        }
    }

    pub fn with_wallet(&self, wallet_name: &str) -> Result<Self> {
        Ok(Self {
            base_url: self
                .base_url
                .join(format!("/wallet/{}", wallet_name).as_str())?,
            ..self.clone()
        })
    }

    pub async fn network(&self) -> Result<Network> {
        let blockchain_info = self.getblockchaininfo().await?;

        Ok(blockchain_info.chain)
    }

    pub async fn median_time(&self) -> Result<u64> {
        let blockchain_info = self.getblockchaininfo().await?;

        Ok(blockchain_info.median_time)
    }

    pub async fn set_hd_seed(
        &self,
        wallet_name: &str,
        new_key_pool: Option<bool>,
        wif_private_key: Option<String>,
    ) -> Result<()> {
        self.with_wallet(wallet_name)?
            .sethdseed(new_key_pool, wif_private_key)
            .await?;

        Ok(())
    }

    pub async fn send_to_address(
        &self,
        wallet_name: &str,
        address: Address,
        amount: Amount,
    ) -> Result<Txid> {
        let txid = self
            .with_wallet(wallet_name)?
            .sendtoaddress(address, amount.to_btc())
            .await?;

        Ok(Txid::from_str(&txid)?)
    }

    pub async fn get_raw_transaction(&self, txid: Txid) -> Result<Transaction> {
        let hex: String = self.get_raw_transaction_rpc(txid, false).await?;
        let bytes: Vec<u8> = FromHex::from_hex(&hex)?;
        let transaction = bitcoin::consensus::encode::deserialize(&bytes)?;

        Ok(transaction)
    }

    pub async fn get_raw_transaction_verbose(
        &self,
        txid: Txid,
    ) -> Result<bitcoincore_rpc_json::GetRawTransactionResult> {
        let res = self.get_raw_transaction_rpc(txid, true).await?;

        Ok(res)
    }

    async fn get_raw_transaction_rpc<R>(&self, txid: Txid, verbose: bool) -> Result<R>
    where
        R: std::fmt::Debug + DeserializeOwned,
    {
        let body = jsonrpc_client::Request::new_v2("getrawtransaction")
            .with_argument("txid".into(), txid)?
            .with_argument("verbose".into(), verbose)?
            .serialize()?;

        let payload: ResponsePayload<R> = self
            .inner
            .send_request::<R>(self.base_url.clone(), body)
            .await
            .map_err(::jsonrpc_client::Error::Client)?
            .payload;
        let response: std::result::Result<R, JsonRpcError> = payload.into();

        Ok(response.map_err(::jsonrpc_client::Error::JsonRpc)?)
    }

    pub async fn fund_psbt(
        &self,
        wallet_name: &str,
        inputs: &[bitcoincore_rpc_json::CreateRawTransactionInput],
        address: Address,
        amount: Amount,
    ) -> Result<String> {
        let mut outputs_converted = HashMap::new();
        outputs_converted.insert(address.to_string(), amount.to_btc());
        let psbt = self
            .with_wallet(wallet_name)?
            .walletcreatefundedpsbt(inputs, outputs_converted)
            .await?;
        Ok(psbt.psbt)
    }

    pub async fn join_psbts(&self, wallet_name: &str, psbts: &[String]) -> Result<PsbtBase64> {
        let psbt = self.with_wallet(wallet_name)?.joinpsbts(psbts).await?;
        Ok(psbt)
    }
    pub async fn wallet_process_psbt(
        &self,
        wallet_name: &str,
        psbt: PsbtBase64,
    ) -> Result<WalletProcessPsbtResponse> {
        let psbt = self
            .with_wallet(wallet_name)?
            .walletprocesspsbt(psbt)
            .await?;
        Ok(psbt)
    }

    pub async fn finalize_psbt(
        &self,
        wallet_name: &str,
        psbt: PsbtBase64,
    ) -> Result<FinalizePsbtResult> {
        let psbt = self.with_wallet(wallet_name)?.finalizepsbt(psbt).await?;
        Ok(psbt)
    }

    pub async fn address_info(
        &self,
        wallet_name: &str,
        address: &Address,
    ) -> Result<GetAddressInfoResult> {
        let address_info = self
            .with_wallet(wallet_name)?
            .getaddressinfo(address)
            .await?;
        Ok(address_info)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("JSON Rpc Client: ")]
    JsonRpcClient(#[from] jsonrpc_client::Error<reqwest::Error>),
    #[error("Serde JSON: ")]
    SerdeJson(#[from] serde_json::Error),
    #[error("Parse amount: ")]
    ParseAmount(#[from] bitcoin::amount::ParseAmountError),
    #[error("Hex decode: ")]
    Hex(#[from] bitcoin::hashes::hex::error::HexToBytesError),
    #[error("Bitcoin decode: ")]
    BitcoinDecode(#[from] bitcoin::consensus::encode::Error),
    #[error("Txid Parsing: ")]
    TxidParseError(#[from] bitcoin::hashes::hex::error::HexToArrayError),
    #[error("Address Parsing: ")]
    AddressParseError(#[from] bitcoin::address::ParseError),
    // TODO: add more info to error
    #[error("Unexpected response: ")]
    UnexpectedResponse,
    #[error("Parse url: ")]
    ParseUrl(#[from] url::ParseError),
}

/// Response to the RPC command `getrawtransaction`, when the second
/// argument is set to `true`.
///
/// It only defines one field, but can be expanded to include all the
/// fields returned by `bitcoind` (see:
/// https://bitcoincore.org/en/doc/0.19.0/rpc/rawtransactions/getrawtransaction/)
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct GetRawTransactionVerboseResponse {
    #[serde(rename = "blockhash")]
    pub block_hash: Option<bitcoin::BlockHash>,
}

/// Response to the RPC command `getblock`.
///
/// It only defines one field, but can be expanded to include all the
/// fields returned by `bitcoind` (see:
/// https://bitcoincore.org/en/doc/0.19.0/rpc/blockchain/getblock/)
#[derive(Copy, Clone, Debug, Deserialize)]
pub struct GetBlockResponse {
    pub height: u32,
}

#[cfg(all(test, feature = "test-docker"))]
mod test {
    use super::*;
    use crate::Bitcoind;
    use std::time::Duration;
    use testcontainers::clients;
    use tokio::time::sleep;

    #[tokio::test]
    async fn get_network_info() {
        let tc_client = clients::Cli::default();
        let (client, _container) = {
            let blockchain = Bitcoind::new(&tc_client).unwrap();

            (Client::new(blockchain.node_url.clone()), blockchain)
        };

        let network = client.network().await.unwrap();

        assert_eq!(network, Network::Regtest)
    }

    #[tokio::test]
    async fn get_median_time() {
        let tc_client = clients::Cli::default();
        let (client, _container) = {
            let blockchain = Bitcoind::new(&tc_client).unwrap();

            (Client::new(blockchain.node_url.clone()), blockchain)
        };

        let _mediant_time = client.median_time().await.unwrap();
    }

    #[tokio::test]
    async fn blockcount() {
        let tc_client = testcontainers::clients::Cli::default();
        let bitcoind = Bitcoind::new(&tc_client).unwrap();
        bitcoind.init(5).await.unwrap();

        let client = Client::new(bitcoind.node_url.clone());

        let height_0 = client.getblockcount().await.unwrap();
        sleep(Duration::from_secs(2)).await;

        let height_1 = client.getblockcount().await.unwrap();

        assert!(height_1 > height_0)
    }
}
