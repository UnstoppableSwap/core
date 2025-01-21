use bitcoin::{address::NetworkUnchecked, Address, BlockHash, Transaction, Txid};
use bitcoincore_rpc_json::{
    FinalizePsbtResult, GetAddressInfoResult, GetBlockResult, GetBlockchainInfoResult,
    GetDescriptorInfoResult, GetTransactionResult, GetWalletInfoResult, ListUnspentResultEntry,
    LoadWalletResult, WalletCreateFundedPsbtResult,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[jsonrpc_client::api]
pub trait BitcoindRpcApi {
    async fn createwallet(
        &self,
        wallet_name: &str,
        disable_private_keys: Option<bool>,
        blank: Option<bool>,
        passphrase: Option<String>,
        avoid_reuse: Option<bool>,
    ) -> LoadWalletResult;

    async fn deriveaddresses(&self, descriptor: &str, range: Option<[u64; 2]>) -> Vec<Address<NetworkUnchecked>>;

    async fn dumpwallet(&self, filename: &std::path::Path) -> DumpWalletResponse;

    async fn finalizepsbt(&self, psbt: PsbtBase64) -> FinalizePsbtResult;

    async fn generatetoaddress(&self, nblocks: u32, address: Address) -> Vec<BlockHash>;

    async fn getaddressinfo(&self, address: &Address) -> GetAddressInfoResult;

    async fn getbalance(
        &self,
        minconf: Option<u32>,
        include_watchonly: Option<bool>,
        avoid_reuse: Option<bool>,
    ) -> f64;

    async fn getblock(&self, blockhash: &bitcoin::BlockHash) -> GetBlockResult;

    async fn getblockchaininfo(&self) -> GetBlockchainInfoResult;

    async fn getblockcount(&self) -> u32;

    async fn getdescriptorinfo(&self, descriptor: &str) -> GetDescriptorInfoResult;

    async fn getnewaddress(&self, label: Option<String>, address_type: Option<String>) -> Address<NetworkUnchecked>;

    async fn gettransaction(&self, txid: Txid) -> GetTransactionResult;

    async fn getwalletinfo(&self) -> GetWalletInfoResult;

    async fn joinpsbts(&self, txs: &[String]) -> PsbtBase64;

    async fn listunspent(
        &self,
        minconf: Option<u32>,
        maxconf: Option<u32>,
        addresses: Option<Vec<Address>>,
        include_unsafe: Option<bool>,
    ) -> Vec<ListUnspentResultEntry>;

    async fn listwallets(&self) -> Vec<String>;

    async fn sendrawtransaction(&self, hexstring: TransactionHex) -> String;

    /// amount is btc
    async fn sendtoaddress(&self, address: Address, amount: f64) -> String;

    async fn sethdseed(&self, new_key_pool: Option<bool>, wif_private_key: Option<String>) -> ();

    /// Outputs are {address, btc amount}
    async fn walletcreatefundedpsbt(
        &self,
        inputs: &[bitcoincore_rpc_json::CreateRawTransactionInput],
        outputs: HashMap<String, f64>,
    ) -> WalletCreateFundedPsbtResult;

    async fn walletprocesspsbt(&self, psbt: PsbtBase64) -> WalletProcessPsbtResponse;
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DumpWalletResponse {
    pub filename: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PsbtBase64(pub String);

#[derive(Debug, Deserialize, Serialize)]
pub struct WalletProcessPsbtResponse {
    psbt: String,
    complete: bool,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename = "*")]
pub struct Account;

impl From<WalletProcessPsbtResponse> for PsbtBase64 {
    fn from(processed_psbt: WalletProcessPsbtResponse) -> Self {
        Self(processed_psbt.psbt)
    }
}

impl From<String> for PsbtBase64 {
    fn from(base64_string: String) -> Self {
        Self(base64_string)
    }
}

#[derive(Debug, Serialize)]
pub struct TransactionHex(String);

impl From<Transaction> for TransactionHex {
    fn from(tx: Transaction) -> Self {
        Self(bitcoin::consensus::encode::serialize_hex(&tx))
    }
}
