use ::monero::Network;
use anyhow::{bail, Context, Error, Result};
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::time::Duration;

// See: https://www.moneroworld.com/#nodes, https://monero.fail
// We don't need any testnet nodes because we don't support testnet at all
static MONERO_DAEMONS: Lazy<[MoneroDaemon; 12]> = Lazy::new(|| {
    [
        MoneroDaemon::new("xmr-node.cakewallet.com", 18081, Network::Mainnet),
        MoneroDaemon::new("nodex.monerujo.io", 18081, Network::Mainnet),
        MoneroDaemon::new("nodes.hashvault.pro", 18081, Network::Mainnet),
        MoneroDaemon::new("p2pmd.xmrvsbeast.com", 18081, Network::Mainnet),
        MoneroDaemon::new("node.monerodevs.org", 18089, Network::Mainnet),
        MoneroDaemon::new("xmr-node-uk.cakewallet.com", 18081, Network::Mainnet),
        MoneroDaemon::new("xmr.litepay.ch", 18081, Network::Mainnet),
        MoneroDaemon::new("stagenet.xmr-tw.org", 38081, Network::Stagenet),
        MoneroDaemon::new("node.monerodevs.org", 38089, Network::Stagenet),
        MoneroDaemon::new("singapore.node.xmr.pm", 38081, Network::Stagenet),
        MoneroDaemon::new("xmr-lux.boldsuck.org", 38081, Network::Stagenet),
        MoneroDaemon::new("stagenet.community.rino.io", 38081, Network::Stagenet),
    ]
});

#[derive(Debug, Clone)]
pub struct MoneroDaemon {
    address: String,
    port: u16,
    network: Network,
}

impl MoneroDaemon {
    pub fn new(address: impl Into<String>, port: u16, network: Network) -> MoneroDaemon {
        MoneroDaemon {
            address: address.into(),
            port,
            network,
        }
    }

    pub fn from_str(address: impl Into<String>, network: Network) -> Result<MoneroDaemon, Error> {
        let (address, port) = extract_host_and_port(address.into())?;

        Ok(MoneroDaemon {
            address,
            port,
            network,
        })
    }

    /// Checks if the Monero daemon is available by sending a request to its `get_info` endpoint.
    pub async fn is_available(&self, client: &reqwest::Client) -> Result<bool, Error> {
        let url = format!("http://{}:{}/get_info", self.address, self.port);
        let res = client
            .get(url)
            .send()
            .await
            .context("Failed to send request to get_info endpoint")?;

        let json: MoneroDaemonGetInfoResponse = res
            .json()
            .await
            .context("Failed to deserialize daemon get_info response")?;

        let is_status_ok = json.status == "OK";
        let is_synchronized = json.synchronized;
        let is_correct_network = match self.network {
            Network::Mainnet => json.mainnet,
            Network::Stagenet => json.stagenet,
            Network::Testnet => json.testnet,
        };

        Ok(is_status_ok && is_synchronized && is_correct_network)
    }
}

impl Display for MoneroDaemon {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.address, self.port)
    }
}

#[derive(Deserialize)]
struct MoneroDaemonGetInfoResponse {
    status: String,
    synchronized: bool,
    mainnet: bool,
    stagenet: bool,
    testnet: bool,
}

/// Chooses an available Monero daemon based on the specified network.
async fn choose_monero_daemon(network: Network) -> Result<MoneroDaemon, Error> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .https_only(false)
        .build()?;

    // We only want to check for daemons that match the specified network
    let network_matching_daemons = MONERO_DAEMONS
        .iter()
        .filter(|daemon| daemon.network == network);

    for daemon in network_matching_daemons {
        match daemon.is_available(&client).await {
            Ok(true) => {
                tracing::debug!(%daemon, "Found available Monero daemon");
                return Ok(daemon.clone());
            }
            Err(err) => {
                tracing::debug!(%err, %daemon, "Failed to connect to Monero daemon");
                continue;
            }
            Ok(false) => continue,
        }
    }

    bail!("No Monero daemon could be found. Please specify one manually or try again later.")
}

/// Public wrapper around [`choose_monero_daemon`].
pub async fn choose_monero_node(network: Network) -> Result<MoneroDaemon, Error> {
    choose_monero_daemon(network).await
}

fn extract_host_and_port(address: String) -> Result<(String, u16), Error> {
    // Strip the protocol (anything before "://")
    let stripped_address = if let Some(pos) = address.find("://") {
        address[(pos + 3)..].to_string()
    } else {
        address
    };

    // Split the remaining address into parts (host and port)
    let parts: Vec<&str> = stripped_address.split(':').collect();

    if parts.len() == 2 {
        let host = parts[0].to_string();
        let port = parts[1].parse::<u16>()?;

        return Ok((host, port));
    }

    bail!(
        "Could not extract host and port from address: {}",
        stripped_address
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_is_daemon_available_success() {
        let mut server = mockito::Server::new_async().await;

        let _ = server
            .mock("GET", "/get_info")
            .with_status(200)
            .with_body(
                r#"
                {
                    "status": "OK",
                    "synchronized": true,
                    "mainnet": true,
                    "stagenet": false,
                    "testnet": false
                }
                "#,
            )
            .create();

        let (host, port) = extract_host_and_port(server.host_with_port()).unwrap();

        let client = reqwest::Client::new();
        let result = MoneroDaemon::new(host, port, Network::Mainnet)
            .is_available(&client)
            .await;

        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_is_daemon_available_wrong_network_failure() {
        let mut server = mockito::Server::new_async().await;

        let _ = server
            .mock("GET", "/get_info")
            .with_status(200)
            .with_body(
                r#"
                {
                    "status": "OK",
                    "synchronized": true,
                    "mainnet": true,
                    "stagenet": false,
                    "testnet": false
                }
                "#,
            )
            .create();

        let (host, port) = extract_host_and_port(server.host_with_port()).unwrap();

        let client = reqwest::Client::new();
        let result = MoneroDaemon::new(host, port, Network::Stagenet)
            .is_available(&client)
            .await;

        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_is_daemon_available_not_synced_failure() {
        let mut server = mockito::Server::new_async().await;

        let _ = server
            .mock("GET", "/get_info")
            .with_status(200)
            .with_body(
                r#"
                {
                    "status": "OK",
                    "synchronized": false,
                    "mainnet": true,
                    "stagenet": false,
                    "testnet": false
                }
                "#,
            )
            .create();

        let (host, port) = extract_host_and_port(server.host_with_port()).unwrap();

        let client = reqwest::Client::new();
        let result = MoneroDaemon::new(host, port, Network::Mainnet)
            .is_available(&client)
            .await;

        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_is_daemon_available_network_error_failure() {
        let client = reqwest::Client::new();
        let result = MoneroDaemon::new("does.not.exist.com", 18081, Network::Mainnet)
            .is_available(&client)
            .await;

        assert!(result.is_err());
    }
}
