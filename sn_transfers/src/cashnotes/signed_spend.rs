// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{Hash, NanoTokens, Transaction, UniquePubkey};
use crate::{Error, Result, Signature};

use custom_debug::Debug;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// SignedSpend's are constructed when a CashNote is logged to the spentbook.
#[derive(Debug, Clone, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SignedSpend {
    /// The Spend, which together with signature over it, constitutes the SignedSpend.
    pub spend: Spend,
    /// The DerivedSecretKey's signature over (the hash of) Spend, confirming that the CashNote was intended to be spent.
    #[debug(skip)]
    pub derived_key_sig: Signature,
}

impl SignedSpend {
    /// Get public key of input CashNote.
    pub fn unique_pubkey(&self) -> &UniquePubkey {
        &self.spend.unique_pubkey
    }

    /// Get the hash of the transaction this CashNote is spent in
    pub fn spent_tx_hash(&self) -> Hash {
        self.spend.spent_tx.hash()
    }

    /// Get the transaction this CashNote is spent in
    pub fn spent_tx(&self) -> Transaction {
        self.spend.spent_tx.clone()
    }

    /// Get the hash of the transaction this CashNote was created in
    pub fn cashnote_creation_tx_hash(&self) -> Hash {
        self.spend.cashnote_creation_tx.hash()
    }

    /// Get Nano
    pub fn token(&self) -> &NanoTokens {
        &self.spend.token
    }

    /// Get reason.
    pub fn reason(&self) -> Hash {
        self.spend.reason
    }

    /// Represent this SignedSpend as bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes: Vec<u8> = Default::default();
        bytes.extend(self.spend.to_bytes());
        bytes.extend(self.derived_key_sig.to_bytes());
        bytes
    }

    /// Verify this SignedSpend
    ///
    /// Checks that the provided spent_tx_hash equals the input dst tx hash that was
    /// signed by the DerivedSecretKey. Also verifies that that signature is
    /// valid for this SignedSpend.
    pub fn verify(&self, spent_tx_hash: Hash) -> Result<()> {
        // Verify that input spent_tx_hash matches self.spent_tx_hash which was signed by the DerivedSecretKey of the input.
        if spent_tx_hash != self.spent_tx_hash() {
            return Err(Error::InvalidTransactionHash);
        }

        // The spend is signed by the DerivedSecretKey
        // corresponding to the UniquePubkey of the CashNote being spent.
        if self
            .spend
            .unique_pubkey
            .verify(&self.derived_key_sig, self.spend.to_bytes())
        {
            Ok(())
        } else {
            Err(Error::InvalidSpendSignature(*self.unique_pubkey()))
        }
    }
}

// Impl manually to avoid clippy complaint about Hash conflict.
impl PartialEq for SignedSpend {
    fn eq(&self, other: &Self) -> bool {
        self.spend == other.spend && self.derived_key_sig == other.derived_key_sig
    }
}

impl Eq for SignedSpend {}

impl std::hash::Hash for SignedSpend {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let bytes = self.to_bytes();
        bytes.hash(state);
    }
}

/// Represents the data to be signed by the DerivedSecretKey of the CashNote being spent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Spend {
    /// UniquePubkey of input CashNote that this SignedSpend is proving to be spent.
    pub unique_pubkey: UniquePubkey,
    /// The transaction that the input CashNote is being spent in.
    #[debug(skip)]
    pub spent_tx: Transaction,
    /// Reason why this CashNote was spent.
    #[debug(skip)]
    pub reason: Hash,
    /// The amount of the input CashNote.
    #[debug(skip)]
    pub token: NanoTokens,
    /// The transaction that the input CashNote was created in.
    #[debug(skip)]
    pub cashnote_creation_tx: Transaction,
}

impl Spend {
    /// Represent this Spend as bytes.
    /// There is no from_bytes, because this function is not symetric as it uses hashes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes: Vec<u8> = Default::default();
        bytes.extend(self.unique_pubkey.to_bytes());
        bytes.extend(self.spent_tx.hash().as_ref());
        bytes.extend(self.reason.as_ref());
        bytes.extend(self.token.to_bytes());
        bytes.extend(self.cashnote_creation_tx.hash().as_ref());
        bytes
    }

    /// represent this Spend as a Hash
    pub fn hash(&self) -> Hash {
        Hash::hash(&self.to_bytes())
    }
}

impl PartialOrd for Spend {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Spend {
    fn cmp(&self, other: &Self) -> Ordering {
        self.unique_pubkey.cmp(&other.unique_pubkey)
    }
}
