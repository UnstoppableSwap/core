use std::path::Path;
use std::sync::Arc;

use arti_client::{
    config::{BridgeConfigBuilder, TorClientConfigBuilder},
    Error, TorClient,
};
use tor_rtcompat::tokio::TokioRustlsRuntime;

pub async fn init_tor_client(
    data_dir: &Path,
    bridges: Vec<String>,
) -> Result<Arc<TorClient<TokioRustlsRuntime>>, Error> {
    // We store the Tor state in the data directory
    let data_dir = data_dir.join("tor");
    let state_dir = data_dir.join("state");
    let cache_dir = data_dir.join("cache");

    // The client configuration describes how to connect to the Tor network,
    // and what directories to use for storing persistent state.
    let mut builder = TorClientConfigBuilder::from_directories(state_dir, cache_dir);

    // Append all known bridges to the configuration
    for bridge_line in bridges {
        match bridge_line.parse::<BridgeConfigBuilder>() {
            Ok(bridge) => {
                tracing::debug!(%bridge_line, "Using tor bridge");
                builder.bridges().bridges().push(bridge);
            }
            Err(err) => {
                tracing::error!(%err, %bridge_line, "Could not use tor bridge because we could not parse it")
            }
        }
    }

    let config = builder
        .build()
        .expect("We initialized the Tor client with all required attributes");

    // Start the Arti client, and let it bootstrap a connection to the Tor network.
    // (This takes a while to gather the necessary directory information.
    // It uses cached information when possible.)
    let runtime = TokioRustlsRuntime::current().expect("We are always running with tokio");

    tracing::debug!("Bootstrapping Tor client");

    let tor_client = TorClient::with_runtime(runtime)
        .config(config)
        .create_bootstrapped()
        .await?;

    Ok(Arc::new(tor_client))
}
