// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use aptos_types::transaction::SignedTransaction;

/// Interface to dedup transactions. The dedup filters duplicate transactions within a block.
pub trait TransactionDeduper: Send + Sync {
    fn dedup(&self, txns: Vec<SignedTransaction>) -> Vec<SignedTransaction>;
}

/// No Op Deduper to maintain backward compatibility
pub struct NoOpDeduper {}

impl TransactionDeduper for NoOpDeduper {
    fn dedup(&self, txns: Vec<SignedTransaction>) -> Vec<SignedTransaction> {
        txns
    }
}

