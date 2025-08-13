// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use crate::error::MempoolError;
use anyhow::Result;
use aptos_types::transaction::{SignedTransaction, TransactionStatus};

/// Notification of failed transactions.
#[async_trait::async_trait]
pub trait TxnNotifier: Send + Sync {
    /// Notification of txns which failed execution. (Committed txns is notified by
    /// state sync.)
    async fn notify_failed_txn(
        &self,
        txns: &[SignedTransaction],
        statuses: &[TransactionStatus],
    ) -> Result<(), MempoolError>;
}
