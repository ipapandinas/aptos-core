// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use crate::{
    error::QuorumStoreError,
    payload_client::{user::UserPayloadClient, PayloadClient},
};
use aptos_consensus_types::{
    common::Payload, payload_pull_params::PayloadPullParameters, utils::PayloadTxnsSize,
};
use aptos_logger::debug;
use aptos_types::{on_chain_config::ValidatorTxnConfig, validator_txn::ValidatorTransaction};
use aptos_validator_transaction_pool::TransactionFilter;
use fail::fail_point;
use std::{cmp::min, sync::Arc, time::Instant};

pub struct MixedPayloadClient {
    validator_txn_config: ValidatorTxnConfig,
    validator_txn_pool_client: Arc<dyn crate::payload_client::validator::ValidatorTxnPayloadClient>,
    user_payload_client: Arc<dyn UserPayloadClient>,
}

impl MixedPayloadClient {
    pub fn new(
        validator_txn_config: ValidatorTxnConfig,
        validator_txn_pool_client: Arc<
            dyn crate::payload_client::validator::ValidatorTxnPayloadClient,
        >,
        user_payload_client: Arc<dyn UserPayloadClient>,
    ) -> Self {
        Self {
            validator_txn_config,
            validator_txn_pool_client,
            user_payload_client,
        }
    }

    /// When enabled in smoke tests, generate 2 random validator transactions, 1 valid, 1 invalid.
    fn extra_test_only_vtxns(&self) -> Vec<ValidatorTransaction> {
        fail_point!("mixed_payload_client::extra_test_only_vtxns", |_| {
            use aptos_types::dkg::{DKGTranscript, DKGTranscriptMetadata};
            use move_core_types::account_address::AccountAddress;

            vec![ValidatorTransaction::DKGResult(DKGTranscript {
                metadata: DKGTranscriptMetadata {
                    epoch: 999,
                    author: AccountAddress::ZERO,
                },
                transcript_bytes: vec![],
            })]
        });
        vec![]
    }
}

#[async_trait::async_trait]
impl PayloadClient for MixedPayloadClient {
    async fn pull_payload(
        &self,
        params: PayloadPullParameters,
        validator_txn_filter: TransactionFilter,
    ) -> anyhow::Result<(Vec<ValidatorTransaction>, Payload), QuorumStoreError> {
        // Pull validator txns first.
        let validator_txn_pull_timer = Instant::now();
        let mut validator_txns = self
            .validator_txn_pool_client
            .pull(
                params.max_poll_time,
                min(
                    params.max_txns.count(),
                    self.validator_txn_config.per_block_limit_txn_count(),
                ),
                min(
                    params.max_txns.size_in_bytes(),
                    self.validator_txn_config.per_block_limit_total_bytes(),
                ),
                validator_txn_filter,
            )
            .await;
        let vtxn_size = PayloadTxnsSize::new(
            validator_txns.len() as u64,
            validator_txns
                .iter()
                .map(|txn| txn.size_in_bytes())
                .sum::<usize>() as u64,
        );

        validator_txns.extend(self.extra_test_only_vtxns());

        debug!("num_validator_txns={}", validator_txns.len());
        // Update constraints with validator txn pull results.
        let mut user_txn_pull_params = params;
        user_txn_pull_params.max_txns -= vtxn_size;
        user_txn_pull_params.max_txns_after_filtering -= validator_txns.len() as u64;
        user_txn_pull_params.soft_max_txns_after_filtering -= validator_txns.len() as u64;
        user_txn_pull_params.max_poll_time = user_txn_pull_params
            .max_poll_time
            .saturating_sub(validator_txn_pull_timer.elapsed());

        // Pull user payload.
        let user_payload = self.user_payload_client.pull(user_txn_pull_params).await?;

        Ok((validator_txns, user_payload))
    }
}
