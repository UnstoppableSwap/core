-- This migration adds the lock_transfer_proof field to Bob's State4, State5, and State6
-- The lock_transfer_proof is copied from the XmrLockProofReceived state when available
-- For State6, the field can be null if XmrLockProofReceived was never reached

-- Bob: Add lock_transfer_proof to State4 in XmrLocked state
UPDATE swap_states SET
    state = json_insert(
        state,
        '$.Bob.XmrLocked.state4.lock_transfer_proof',
        (
            SELECT json_extract(states.state, '$.Bob.XmrLockProofReceived.lock_transfer_proof')
            FROM swap_states AS states
            WHERE
                states.swap_id = swap_states.swap_id
                AND json_extract(states.state, '$.Bob.XmrLockProofReceived') IS NOT NULL
        )
    )
WHERE json_extract(state, '$.Bob.XmrLocked') IS NOT NULL;

-- Bob: Add lock_transfer_proof to State4 in EncSigSent state
UPDATE swap_states SET
    state = json_insert(
        state,
        '$.Bob.EncSigSent.state4.lock_transfer_proof',
        (
            SELECT json_extract(states.state, '$.Bob.XmrLockProofReceived.lock_transfer_proof')
            FROM swap_states AS states
            WHERE
                states.swap_id = swap_states.swap_id
                AND json_extract(states.state, '$.Bob.XmrLockProofReceived') IS NOT NULL
        )
    )
WHERE json_extract(state, '$.Bob.EncSigSent') IS NOT NULL;

-- Bob: Add lock_transfer_proof to State5 in BtcRedeemed state
UPDATE swap_states SET
    state = json_insert(
        state,
        '$.Bob.BtcRedeemed.lock_transfer_proof',
        (
            SELECT json_extract(states.state, '$.Bob.XmrLockProofReceived.lock_transfer_proof')
            FROM swap_states AS states
            WHERE
                states.swap_id = swap_states.swap_id
                AND json_extract(states.state, '$.Bob.XmrLockProofReceived') IS NOT NULL
        )
    )
WHERE json_extract(state, '$.Bob.BtcRedeemed') IS NOT NULL;

-- Bob: Add lock_transfer_proof to State6 in CancelTimelockExpired state
UPDATE swap_states SET
    state = json_insert(
        state,
        '$.Bob.CancelTimelockExpired.lock_transfer_proof',
        (
            SELECT json_extract(states.state, '$.Bob.XmrLockProofReceived.lock_transfer_proof')
            FROM swap_states AS states
            WHERE
                states.swap_id = swap_states.swap_id
                AND json_extract(states.state, '$.Bob.XmrLockProofReceived') IS NOT NULL
        )
    )
WHERE json_extract(state, '$.Bob.CancelTimelockExpired') IS NOT NULL;

-- Bob: Add lock_transfer_proof to State6 in BtcCancelled state
UPDATE swap_states SET
    state = json_insert(
        state,
        '$.Bob.BtcCancelled.lock_transfer_proof',
        (
            SELECT json_extract(states.state, '$.Bob.XmrLockProofReceived.lock_transfer_proof')
            FROM swap_states AS states
            WHERE
                states.swap_id = swap_states.swap_id
                AND json_extract(states.state, '$.Bob.XmrLockProofReceived') IS NOT NULL
        )
    )
WHERE json_extract(state, '$.Bob.BtcCancelled') IS NOT NULL;

-- Bob: Add lock_transfer_proof to State6 in BtcRefunded state (Done variant)
UPDATE swap_states SET
    state = json_insert(
        state,
        '$.Bob.Done.BtcRefunded.lock_transfer_proof',
        (
            SELECT json_extract(states.state, '$.Bob.XmrLockProofReceived.lock_transfer_proof')
            FROM swap_states AS states
            WHERE
                states.swap_id = swap_states.swap_id
                AND json_extract(states.state, '$.Bob.XmrLockProofReceived') IS NOT NULL
        )
    )
WHERE json_extract(state, '$.Bob.Done.BtcRefunded') IS NOT NULL;

-- Bob: Add lock_transfer_proof to State6 in BtcPunished state
UPDATE swap_states SET
    state = json_insert(
        state,
        '$.Bob.BtcPunished.state.lock_transfer_proof',
        (
            SELECT json_extract(states.state, '$.Bob.XmrLockProofReceived.lock_transfer_proof')
            FROM swap_states AS states
            WHERE
                states.swap_id = swap_states.swap_id
                AND json_extract(states.state, '$.Bob.XmrLockProofReceived') IS NOT NULL
        )
    )
WHERE json_extract(state, '$.Bob.BtcPunished') IS NOT NULL;

