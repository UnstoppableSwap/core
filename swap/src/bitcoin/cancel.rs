use crate::bitcoin;
use crate::bitcoin::wallet::Watchable;
use crate::bitcoin::{
    build_shared_output_descriptor, Address, Amount, BlockHeight, PublicKey, Transaction, TxLock,
};
use ::bitcoin::sighash::SighashCache;
use ::bitcoin::transaction::Version;
use ::bitcoin::Weight;
use ::bitcoin::{
    locktime::absolute::LockTime as PackedLockTime, secp256k1, sighash::SegwitV0Sighash as Sighash,
    EcdsaSighashType, OutPoint, ScriptBuf, Sequence, TxIn, TxOut, Txid,
};
use anyhow::Result;
use bdk_wallet::miniscript::Descriptor;
use ecdsa_fun::Signature;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::ops::Add;
use typeshare::typeshare;

/// Represent a timelock, expressed in relative block height as defined in
/// [BIP68](https://github.com/bitcoin/bips/blob/master/bip-0068.mediawiki).
/// E.g. The timelock expires 10 blocks after the reference transaction is
/// mined.
#[derive(Debug, Copy, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(transparent)]
#[typeshare]
pub struct CancelTimelock(u32);

impl From<CancelTimelock> for u32 {
    fn from(cancel_timelock: CancelTimelock) -> Self {
        cancel_timelock.0
    }
}

impl From<u32> for CancelTimelock {
    fn from(number_of_blocks: u32) -> Self {
        Self(number_of_blocks)
    }
}

impl CancelTimelock {
    pub const fn new(number_of_blocks: u32) -> Self {
        Self(number_of_blocks)
    }

    pub fn half(&self) -> CancelTimelock {
        Self(self.0 / 2)
    }
}

impl Add<CancelTimelock> for BlockHeight {
    type Output = BlockHeight;

    fn add(self, rhs: CancelTimelock) -> Self::Output {
        self + rhs.0
    }
}

impl PartialOrd<CancelTimelock> for u32 {
    fn partial_cmp(&self, other: &CancelTimelock) -> Option<Ordering> {
        self.partial_cmp(&other.0)
    }
}

impl PartialEq<CancelTimelock> for u32 {
    fn eq(&self, other: &CancelTimelock) -> bool {
        self.eq(&other.0)
    }
}

impl fmt::Display for CancelTimelock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} blocks", self.0)
    }
}

/// Represent a timelock, expressed in relative block height as defined in
/// [BIP68](https://github.com/bitcoin/bips/blob/master/bip-0068.mediawiki).
/// E.g. The timelock expires 10 blocks after the reference transaction is
/// mined.
#[derive(Debug, Copy, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(transparent)]
#[typeshare]
pub struct PunishTimelock(u32);

impl From<PunishTimelock> for u32 {
    fn from(punish_timelock: PunishTimelock) -> Self {
        punish_timelock.0
    }
}

impl From<u32> for PunishTimelock {
    fn from(number_of_blocks: u32) -> Self {
        Self(number_of_blocks)
    }
}

impl PunishTimelock {
    pub const fn new(number_of_blocks: u32) -> Self {
        Self(number_of_blocks)
    }
}

impl Add<PunishTimelock> for BlockHeight {
    type Output = BlockHeight;

    fn add(self, rhs: PunishTimelock) -> Self::Output {
        self + rhs.0
    }
}

impl PartialOrd<PunishTimelock> for u32 {
    fn partial_cmp(&self, other: &PunishTimelock) -> Option<Ordering> {
        self.partial_cmp(&other.0)
    }
}

impl PartialEq<PunishTimelock> for u32 {
    fn eq(&self, other: &PunishTimelock) -> bool {
        self.eq(&other.0)
    }
}

#[derive(Debug)]
pub struct TxCancel {
    inner: Transaction,
    digest: Sighash,
    pub(in crate::bitcoin) output_descriptor: Descriptor<::bitcoin::PublicKey>,
    lock_output_descriptor: Descriptor<::bitcoin::PublicKey>,
}

impl TxCancel {
    pub fn new(
        tx_lock: &TxLock,
        cancel_timelock: CancelTimelock,
        A: PublicKey,
        B: PublicKey,
        spending_fee: Amount,
    ) -> Result<Self> {
        let cancel_output_descriptor = build_shared_output_descriptor(A.0, B.0)?;

        let tx_in = TxIn {
            previous_output: tx_lock.as_outpoint(),
            script_sig: Default::default(),
            sequence: Sequence(cancel_timelock.0),
            witness: Default::default(),
        };

        let tx_out = TxOut {
            value: tx_lock.lock_amount() - spending_fee,
            script_pubkey: cancel_output_descriptor.script_pubkey(),
        };

        let transaction = Transaction {
            version: Version(2),
            lock_time: PackedLockTime::from_height(0).expect("0 to be below lock time threshold"),
            input: vec![tx_in],
            output: vec![tx_out],
        };

        let digest = SighashCache::new(&transaction)
            .p2wsh_signature_hash(
                0, // Only one input: lock_input (lock transaction)
                &tx_lock.output_descriptor.script_code().expect("scriptcode"),
                tx_lock.lock_amount(),
                EcdsaSighashType::All,
            )
            .expect("sighash");

        Ok(Self {
            inner: transaction,
            digest,
            output_descriptor: cancel_output_descriptor,
            lock_output_descriptor: tx_lock.output_descriptor.clone(),
        })
    }

    pub fn txid(&self) -> Txid {
        self.inner.compute_txid()
    }

    pub fn digest(&self) -> Sighash {
        self.digest
    }

    pub fn amount(&self) -> Amount {
        self.inner.output[0].value
    }

    pub fn as_outpoint(&self) -> OutPoint {
        OutPoint::new(self.inner.compute_txid(), 0)
    }

    pub fn complete_as_alice(
        self,
        a: bitcoin::SecretKey,
        B: bitcoin::PublicKey,
        tx_cancel_sig_B: bitcoin::Signature,
    ) -> Result<Transaction> {
        let sig_a = a.sign(self.digest());
        let sig_b = tx_cancel_sig_B;

        let tx_cancel = self
            .add_signatures((a.public(), sig_a), (B, sig_b))
            .expect("sig_{a,b} to be valid signatures for tx_cancel");

        Ok(tx_cancel)
    }

    pub fn complete_as_bob(
        self,
        A: bitcoin::PublicKey,
        b: bitcoin::SecretKey,
        tx_cancel_sig_A: bitcoin::Signature,
    ) -> Result<Transaction> {
        let sig_a = tx_cancel_sig_A;
        let sig_b = b.sign(self.digest());

        let tx_cancel = self
            .add_signatures((A, sig_a), (b.public(), sig_b))
            .expect("sig_{a,b} to be valid signatures for tx_cancel");

        Ok(tx_cancel)
    }

    fn add_signatures(
        self,
        (A, sig_a): (PublicKey, Signature),
        (B, sig_b): (PublicKey, Signature),
    ) -> Result<Transaction> {
        let satisfier = {
            let mut satisfier = HashMap::with_capacity(2);

            let A = ::bitcoin::PublicKey {
                compressed: true,
                inner: secp256k1::PublicKey::from_slice(&A.0.to_bytes())?,
            };
            let B = ::bitcoin::PublicKey {
                compressed: true,
                inner: secp256k1::PublicKey::from_slice(&B.0.to_bytes())?,
            };

            // The order in which these are inserted doesn't matter
            let sig_a = secp256k1::ecdsa::Signature::from_compact(&sig_a.to_bytes())?;
            let sig_b = secp256k1::ecdsa::Signature::from_compact(&sig_b.to_bytes())?;
            satisfier.insert(
                A,
                ::bitcoin::ecdsa::Signature {
                    signature: sig_a,
                    sighash_type: EcdsaSighashType::All,
                },
            );
            satisfier.insert(
                B,
                ::bitcoin::ecdsa::Signature {
                    signature: sig_b,
                    sighash_type: EcdsaSighashType::All,
                },
            );

            satisfier
        };

        let mut tx_cancel = self.inner;
        self.lock_output_descriptor
            .satisfy(&mut tx_cancel.input[0], satisfier)?;

        Ok(tx_cancel)
    }

    pub fn build_spend_transaction(
        &self,
        spend_address: &Address,
        sequence: Option<PunishTimelock>,
        spending_fee: Amount,
    ) -> Transaction {
        let previous_output = self.as_outpoint();

        let sequence = Sequence(sequence.map(|seq| seq.0).unwrap_or(0xFFFF_FFFF));
        let tx_in = TxIn {
            previous_output,
            script_sig: Default::default(),
            sequence,
            witness: Default::default(),
        };

        let tx_out = TxOut {
            value: self.amount() - spending_fee,
            script_pubkey: spend_address.script_pubkey(),
        };

        Transaction {
            version: Version(2),
            lock_time: PackedLockTime::from_height(0).expect("0 to be below lock time threshold"),
            input: vec![tx_in],
            output: vec![tx_out],
        }
    }

    pub fn weight() -> Weight {
        Weight::from_wu(596)
    }
}

impl Watchable for TxCancel {
    fn id(&self) -> Txid {
        self.txid()
    }

    fn script(&self) -> ScriptBuf {
        self.output_descriptor.script_pubkey()
    }
}
