use std::sync::{Arc, Mutex};

use futures::future::join_all;
use tokio::task::spawn_blocking;

use bdk_electrum::electrum_client::{
    Client, ConfigBuilder, ElectrumApi, Error,
};
use bitcoin::Transaction;
use tracing::{debug, error, info, instrument, trace, warn};

/// Round-robin load balancer for Electrum connections.
///
/// The balancer will try each Electrum node until the provided
/// closure succeeds or all nodes have returned an I/O error.
/// Any non I/O error is immediately returned to the caller.
///
/// Clients are created lazily on first use to avoid blocking during initialization.
pub struct ElectrumBalancer {
    urls: Vec<String>,
    clients: Mutex<Vec<Option<Arc<Client>>>>,
    next: Mutex<usize>,
}

impl ElectrumBalancer {
    /// Create a single electrum client with timeout
    fn create_client_with_timeout(url: &str) -> Result<Client, Error> {
        // Configure client with short timeout to prevent hanging on unresponsive servers
        let config = ConfigBuilder::new()
            .timeout(Some(5)) // 5 second timeout
            .retry(0) // Retrying is done at the caller level (ElectrumBalancer)
            .build();

        Client::from_config(url, config)
    }

    /// Create a new balancer from a list of Electrum URLs.
    /// All clients are created at startup - this may take time but eliminates delays during use.
    #[instrument(level = "info", fields(num_urls = urls.len()))]
    pub fn new(urls: Vec<String>) -> Result<Self, Error> {
        if urls.is_empty() {
            error!("No Electrum URLs provided");
            return Err(Error::Protocol("No Electrum URLs provided".into()));
        }

        info!(
            "Initializing Electrum load balancer with {} servers",
            urls.len()
        );

        // Create all clients at startup
        let clients: Vec<Option<Arc<Client>>> = urls
            .iter()
            .map(|url| {
                Self::create_client_with_timeout(url)
                    .map(|client| Arc::new(client))
                    .ok()
            })
            .collect();

        Ok(Self {
            urls,
            clients: Mutex::new(clients),
            next: Mutex::new(0),
        })
    }

    /// Get a client for the given index.
    async fn get_or_create_client(&self, idx: usize) -> Result<Arc<Client>, Error> {
        let clients = self.clients.lock().expect("mutex poisoned");

        if let Some(ref client) = clients[idx] {
            Ok(client.clone())
        } else {
            Err(Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Client {} is not available", self.urls[idx]),
            )))
        }
    }

    /// Helper function to determine if an error should trigger failover
    fn should_retry_on_error(error: &Error) -> bool {
        // Check if this is a transaction not found error - these should NOT be retried
        let error_str = format!("{:?}", error);
        if error_str.contains("\"code\": Number(-5)")
            || error_str.contains("No such mempool or blockchain transaction")
            || error_str.contains("missing transaction")
        {
            return false;
        }

        // For all other errors, retry by default
        true
    }

    /// Execute the given closure using one of the Electrum clients asynchronously.
    ///
    ///
    /// If the closure returns an I/O error or certificate error the balancer will try the next
    /// node until all nodes have been exhausted. The last encountered error
    /// is returned in that case.
    #[instrument(level = "debug", skip(self, f), fields(total_urls = self.urls.len()))]
    pub async fn call_async<F, T>(&self, f: F) -> Result<T, Error>
    where
        F: Fn(&Client) -> Result<T, Error> + Send + Sync + Clone + 'static,
        T: Send + 'static,
    {
        let operation_id = uuid::Uuid::new_v4();
        let num_urls = self.urls.len();
        let mut last_error = None;

        for _attempt in 0..num_urls {
            let idx = {
                let mut next = self.next.lock().expect("mutex poisoned");
                let idx = *next;
                *next = (*next + 1) % num_urls;
                idx
            };

            // Get or create client for this index
            let client = match self.get_or_create_client(idx).await {
                Ok(client) => client,
                Err(e) => {
                    last_error = Some(e);
                    continue;
                }
            };

            // Execute the request in spawn_blocking to prevent blocking the async runtime
            let f_clone = f.clone();
            let request_result = spawn_blocking(move || f_clone(&client)).await;

            match request_result {
                Ok(Ok(res)) => {
                    debug!(%operation_id, "Request successful on {}", self.urls[idx]);
                    return Ok(res);
                }
                Ok(Err(e)) => {
                    if Self::should_retry_on_error(&e) {
                        debug!(
                            %operation_id,
                            "Request failed on {}: {:?}, trying next server",
                            self.urls[idx], e
                        );
                        last_error = Some(e);
                        continue;
                    } else {
                        debug!(
                            %operation_id,
                            "Non-retryable error on {}: {:?}",
                            self.urls[idx], e
                        );
                        return Err(e);
                    }
                }
                Err(join_err) => {
                    let error = Error::IOError(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("spawn_blocking failed: {}", join_err),
                    ));
                    debug!(
                        %operation_id,
                        "Request execution failed on {}: {:?}, trying next server",
                        self.urls[idx], join_err
                    );
                    last_error = Some(error);
                    continue;
                }
            }
        }

        trace!(
            %operation_id,
            "All {} electrum servers failed or could not be created",
            num_urls
        );

        Err(last_error.unwrap_or_else(|| {
            Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                "all electrum nodes failed",
            ))
        }))
    }

    /// Execute the given closure using one of the Electrum clients synchronously.
    ///
    /// This version blocks for client creation if needed but executes the request synchronously.
    /// Used for implementing the ElectrumApi trait.
    ///
    /// If the closure returns an I/O error or certificate error the balancer will try the next
    /// node until all nodes have been exhausted. The last encountered error
    /// is returned in that case.
    #[instrument(level = "debug", skip(self, f), fields(total_urls = self.urls.len()))]
    pub fn call<F, T>(&self, mut f: F) -> Result<T, Error>
    where
        F: FnMut(&Client) -> Result<T, Error>,
    {
        let num_urls = self.urls.len();
        let mut last_error = None;

        for _attempt in 0..num_urls {
            let idx = {
                let mut next = self.next.lock().expect("mutex poisoned");
                let idx = *next;
                *next = (*next + 1) % num_urls;
                idx
            };

            // Get client for this index
            let client = {
                let clients = self.clients.lock().expect("mutex poisoned");
                match &clients[idx] {
                    Some(client) => client.clone(),
                    None => {
                        last_error = Some(Error::IOError(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("Client {} is not available", self.urls[idx]),
                        )));
                        continue;
                    }
                }
            };

            // Execute the request synchronously
            match f(&client) {
                Ok(res) => {
                    trace!("Request successful on {}", self.urls[idx]);
                    return Ok(res);
                }
                Err(e) => {
                    if Self::should_retry_on_error(&e) {
                        warn!(
                            "Request failed on {}: {:?}, trying next server",
                            self.urls[idx], e
                        );
                        last_error = Some(e);
                        continue;
                    } else {
                        debug!("Non-retryable error on {}: {:?}", self.urls[idx], e);
                        return Err(e);
                    }
                }
            }
        }

        error!(
            "All {} electrum servers failed or could not be created",
            num_urls
        );
        Err(last_error.unwrap_or_else(|| {
            Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                "all electrum nodes failed",
            ))
        }))
    }

    /// Execute the given closure on **all** Electrum nodes in parallel.
    ///
    /// The closure is executed in a blocking task for each client.
    /// The resulting `Result`s are collected and returned in the same
    /// order as the nodes were provided during construction.
    #[instrument(level = "debug", skip(self, f), fields(num_urls = self.urls.len()))]
    pub async fn join_all<F, T>(&self, f: F) -> Vec<Result<T, Error>>
    where
        F: Fn(Arc<Client>) -> Result<T, Error> + Send + Sync + Clone + 'static,
        T: Send + 'static,
    {
        info!(
            "Executing parallel requests on {} electrum servers",
            self.urls.len()
        );

        // Pre-create all clients asynchronously
        let mut all_clients = Vec::new();
        for idx in 0..self.urls.len() {
            match self.get_or_create_client(idx).await {
                Ok(client) => all_clients.push(Some(client)),
                Err(e) => {
                    warn!("Failed to create client #{}: {:?}", idx, e);
                    all_clients.push(None);
                }
            }
        }

        let tasks = all_clients
            .into_iter()
            .enumerate()
            .map(|(idx, client_opt)| {
                let f = f.clone();
                spawn_blocking(move || {
                    debug!("Starting parallel request on server #{}", idx);

                    let client = match client_opt {
                        Some(client) => client,
                        None => {
                            debug!("No client available for server #{}", idx);
                            return Err(Error::IOError(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                format!("client #{} unavailable", idx),
                            )));
                        }
                    };

                    let result = f(client);
                    match &result {
                        Ok(_) => debug!("Parallel request succeeded on server #{}", idx),
                        Err(e) => debug!("Parallel request failed on server #{}: {:?}", idx, e),
                    }
                    result
                })
            });

        let results = join_all(tasks)
            .await
            .into_iter()
            .enumerate()
            .map(|(idx, res)| match res {
                Ok(r) => r,
                Err(e) => {
                    error!("Spawn error for server #{}: {}", idx, e);
                    Err(Error::IOError(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("spawn error: {e}"),
                    )))
                }
            })
            .collect::<Vec<_>>();

        let success_count = results.iter().filter(|r| r.is_ok()).count();
        let failure_count = results.len() - success_count;

        if failure_count > 0 {
            warn!(
                "Parallel execution completed: {}/{} succeeded, {}/{} failed",
                success_count,
                results.len(),
                failure_count,
                results.len()
            );
        } else {
            info!(
                "Parallel execution completed: all {}/{} requests succeeded",
                success_count,
                results.len()
            );
        }

        results
    }

    /// Broadcast the given transaction to all Electrum nodes in parallel.
    ///
    /// The method returns a list of results in the same order as the
    /// configured nodes. Errors for individual nodes do not abort the
    /// others.
    #[instrument(level = "info", skip(self, tx), fields(txid = %tx.compute_txid(), num_servers = self.urls.len()))]
    pub async fn broadcast_all(&self, tx: Transaction) -> Vec<Result<bitcoin::Txid, Error>> {
        let txid = tx.compute_txid();
        info!(
            "Broadcasting transaction {} to {} electrum servers",
            txid,
            self.urls.len()
        );

        let results = self
            .join_all(move |client| client.transaction_broadcast(&tx))
            .await;

        let success_count = results.iter().filter(|r| r.is_ok()).count();

        if success_count > 0 {
            info!(
                "Transaction {} broadcast successful on {}/{} servers",
                txid,
                success_count,
                results.len()
            );
        } else {
            error!(
                "Transaction {} broadcast failed on all {} servers",
                txid,
                results.len()
            );
        }

        results
    }

    /// Get the URLs used by this balancer
    pub fn urls(&self) -> &Vec<String> {
        &self.urls
    }

    /// Populate the transaction cache for all clients.
    /// Note: This is not implemented for the load balancer as the underlying clients 
    /// don't support transaction caching.
    pub fn populate_tx_cache(
        &self,
        _txs: impl IntoIterator<Item = impl Into<Arc<Transaction>>>,
    ) {
        // No-op: The raw electrum clients don't support transaction caching
        tracing::debug!("populate_tx_cache called on ElectrumBalancer - this is a no-op");
    }
}

