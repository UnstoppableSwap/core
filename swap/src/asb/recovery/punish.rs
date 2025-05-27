use crate::bitcoin::{self, Txid};
use crate::protocol::alice::AliceState;
use crate::protocol::Database;
use anyhow::{bail, Result};
use std::convert::TryInto;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Cannot punish swap because it is in state {0} which is not punishable")]
    SwapNotPunishable(AliceState),
}

pub async fn punish(
    swap_id: Uuid,
    bitcoin_wallet: Arc<bitcoin::Wallet>,
    db: Arc<dyn Database>,
) -> Result<(Txid, AliceState)> {
    let state = db.get_state(swap_id).await?.try_into()?;

    let (state3, monero_wallet_restore_blockheight, transfer_proof) = match state {
        // We haven't locked our Monero yet, so do don't punish
        AliceState::Started { .. }
        | AliceState::BtcLockTransactionSeen { .. }
        | AliceState::BtcLocked { .. } => bail!(Error::SwapNotPunishable(state.clone())),

        // Punish might be possible, because:
        // - We have locked our Monero
        // - Haven't seen proof of the cancel transaction yet
        | AliceState::XmrLockTransactionSent {state3, monero_wallet_restore_blockheight, transfer_proof}
        | AliceState::XmrLocked {state3, monero_wallet_restore_blockheight, transfer_proof}
        | AliceState::XmrLockTransferProofSent {state3, monero_wallet_restore_blockheight, transfer_proof}
        | AliceState::EncSigLearned {state3, monero_wallet_restore_blockheight, transfer_proof, ..}
        | AliceState::CancelTimelockExpired {state3, monero_wallet_restore_blockheight, transfer_proof}

        // Punish possible due to cancel transaction already being published
        | AliceState::BtcCancelled {state3, monero_wallet_restore_blockheight, transfer_proof}
        | AliceState::BtcRefunded {state3, monero_wallet_restore_blockheight, transfer_proof, ..}
        | AliceState::BtcPunishable {state3, monero_wallet_restore_blockheight, transfer_proof}
        | AliceState::BtcPunished {state3, monero_wallet_restore_blockheight, transfer_proof} => { (state3, monero_wallet_restore_blockheight, transfer_proof) }

        // The state machine is in a state where punish is theoretically impossible but we try and punish anyway as this is what the user wants
        AliceState::BtcRedeemTransactionPublished { .. }
        // Alice already in final state
        | AliceState::BtcRedeemed
        | AliceState::XmrRefunded
        | AliceState::SafelyAborted => bail!(Error::SwapNotPunishable(state)),
    };

    tracing::info!(%swap_id, "Trying to manually punish swap");

    // Attempt to publish the punish Bitcoin transaction
    let txid = state3.punish_btc(&bitcoin_wallet).await?;

    // Construct new state
    let state = AliceState::BtcPunished {
        state3,
        transfer_proof,
        monero_wallet_restore_blockheight,
    };

    // Insert new state into database
    db.insert_latest_state(swap_id, state.clone().into())
        .await?;

    Ok((txid, state))
}
