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

    let (state3, transfer_proof) = match state {
        // Punish potentially possible (no knowledge of cancel transaction)
        AliceState::XmrLockTransactionSent {state3, transfer_proof, ..}
        | AliceState::XmrLocked {state3, transfer_proof, ..}
        | AliceState::XmrLockTransferProofSent {state3, transfer_proof, ..}
        | AliceState::EncSigLearned {state3, transfer_proof, ..}
        | AliceState::CancelTimelockExpired {state3, transfer_proof, ..}
        // Punish possible due to cancel transaction already being published
        | AliceState::BtcCancelled {state3, transfer_proof, ..}
        | AliceState::BtcPunishable {state3, transfer_proof, ..}
        // The state machine is in a state where punish is theoretically impossible but we try and punish anyway as this is what the user wants
        | AliceState::BtcRedeemTransactionPublished { state3, transfer_proof, .. }
        | AliceState::BtcRefunded { state3, transfer_proof,.. } => { (state3, transfer_proof) }

        // Alice already in final state or at the start of the swap so we can't punish
        | AliceState::Started { .. }
        | AliceState::BtcLockTransactionSeen { .. }
        | AliceState::BtcLocked { .. }
        | AliceState::BtcRedeemed { .. }
        | AliceState::XmrRefunded
        | AliceState::BtcPunished { .. }
        | AliceState::SafelyAborted => bail!(Error::SwapNotPunishable(state)),
    };

    tracing::info!(%swap_id, "Trying to manually punish swap");

    let txid = state3.punish_btc(&bitcoin_wallet).await?;

    // Problem: we need the lock transfer proof in BtcPunished.
    // For that, we need to pass it to the state.
    // We want to be able to punish from any state.
    // But: not every state has the transfer proof saved.

    // So we kinda have to get the transfer proof from the database.

    let state = AliceState::BtcPunished {
        state3: state3.clone(),
        transfer_proof,
    };
    db.insert_latest_state(swap_id, state.clone().into())
        .await?;

    Ok((txid, state))
}
