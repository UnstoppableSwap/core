use std::sync::Arc;
use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;
use uuid::Uuid;

use crate::bitcoin::wallet::ScriptStatus;
use crate::bitcoin::{ExpiredTimelocks, TxLock, Wallet};
use crate::cli::api::tauri_bindings::TauriHandle;
use crate::protocol::bob::BobState;
use crate::protocol::{Database, State};

use super::api::tauri_bindings::TauriEmitter;

/// A long running task which watches for changes to timelocks and the number of confirmations.
#[derive(Clone)]
pub struct Watcher {
    wallet: Arc<Wallet>,
    database: Arc<dyn Database + Send + Sync>,
    /// This saves for every running swap the expired timelocks as well as
    /// the [`ScriptStatus`] of [`TxLock`].
    current_swaps: HashMap<(Uuid, TxLock), (ExpiredTimelocks, ScriptStatus)>,
    tauri: Option<TauriHandle>,
}

impl Watcher {
    /// How often to check for changes (in seconds)
    const CHECK_INTERVAL: u64 = 3;

    /// Create a new Watcher
    pub fn new(wallet: Arc<Wallet>, database: Arc<dyn Database + Send + Sync>, tauri: Option<TauriHandle>) -> Self {
        Self {
            wallet,
            database,
            current_swaps: HashMap::new(),
            tauri,
        }
    }

    /// Start running the watcher event loop. 
    /// Should be done in a new task using [`tokio::spawn`].
    pub async fn run(mut self) {
        // Note: since this is de-facto a daemon, we have to gracefully handle errors
        // (which in our case means logging the error message and trying again later)
        loop {
            // Fetch current transactions and timelocks
            let current_swaps = match self.get_current_swaps().await {
                Ok(val) => val,
                Err(e) => {
                    tracing::error!(error=%e, "Failed to fetch current transactions, retrying later");
                    continue;
                }
            };

            // Check for changes for every current swap
            for (uuid, state) in current_swaps {
                // Check if the timelock has expired
                let new_timelock_status = match state.expired_timelocks(self.wallet.clone()).await {
                    Ok(Some(val)) => val,
                    Ok(None) => continue, // ignore finished swaps
                    Err(e) => {
                        tracing::error!(error=%e, "Failed to fetch expired timelocks, retrying later");
                        continue;
                    }
                };
                let new_confirmation_status = match self.wallet.status_of_script(state.tx_lock()) {
                    
                }
                // Check if the status changed
                if let Some(old_status) = self.current_swaps.get(&uuid) {
                    // And send a tauri event if it did
                    if *old_status != new_timelock_status {
                        self.tauri.emit_timelock_change_event(uuid);
                    }
                } else {
                    // If this is the first time we see this swap, send a tauri event, too
                    self.tauri.emit_timelock_change_event(uuid);
                }

                // Insert new status
                self.current_swaps.insert(uuid, new_timelock_status);
            }

            // Sleep and check again later
            tokio::time::sleep(Duration::from_secs(Watcher::CHECK_INTERVAL)).await;
        }
    }

    /// Helper function for fetching the current list of swaps
    async fn get_current_swaps(&self) -> Result<Vec<(Uuid, BobState)>> {
        Ok(self.database
            .all()
            .await?
            .into_iter()
            // Filter for BobState
            .filter_map(|(uuid, state)| {
                match state {
                    State::Bob(bob_state) => Some((uuid, bob_state)),
                    _ => None
                }
            }).collect())
    }
}