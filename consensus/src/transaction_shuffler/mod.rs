// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use aptos_types::transaction::SignedTransaction;

mod use_case_aware;
// re-export use case aware shuffler for fuzzer.
#[cfg(feature = "fuzzing")]
pub mod transaction_shuffler_fuzzing {
    pub mod use_case_aware {
        pub use crate::transaction_shuffler::use_case_aware::{Config, UseCaseAwareShuffler};
    }
}

/// Interface to shuffle transactions
pub trait TransactionShuffler: Send + Sync {
    fn shuffle(&self, txns: Vec<SignedTransaction>) -> Vec<SignedTransaction>;

    /// Given a configuration and a vector of SignedTransactions, return an iterator that
    /// produces them in a particular shuffled order.
    fn signed_transaction_iterator(
        &self,
        txns: Vec<SignedTransaction>,
    ) -> Box<dyn Iterator<Item = SignedTransaction> + 'static>;
}

/// No Op Shuffler to maintain backward compatibility
pub struct NoOpShuffler {}

impl TransactionShuffler for NoOpShuffler {
    fn shuffle(&self, txns: Vec<SignedTransaction>) -> Vec<SignedTransaction> {
        txns
    }

    fn signed_transaction_iterator(
        &self,
        txns: Vec<SignedTransaction>,
    ) -> Box<dyn Iterator<Item = SignedTransaction>> {
        Box::new(txns.into_iter())
    }
}
