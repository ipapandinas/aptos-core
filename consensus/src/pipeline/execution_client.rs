// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    error::StateSyncError,
    network::{IncomingCommitRequest, IncomingRandGenRequest},
    payload_manager::TPayloadManager,
    pipeline::{
        buffer_manager::{OrderedBlocks, ResetRequest},
        pipeline_builder::PipelineBuilder,
        signing_phase::CommitSignerProvider,
    },
    rand::rand_gen::types::RandConfig,
};
use anyhow::{anyhow, Result};
use aptos_channels::aptos_channel;
use aptos_consensus_types::{
    common::Round,
    pipelined_block::PipelinedBlock,
    wrapped_ledger_info::WrappedLedgerInfo,
};
use aptos_crypto::bls12381::PrivateKey;
use aptos_executor_types::ExecutorResult;
use aptos_types::{
    epoch_state::EpochState,
    ledger_info::LedgerInfoWithSignatures,
    on_chain_config::{OnChainConsensusConfig, OnChainExecutionConfig, OnChainRandomnessConfig},
    validator_signer::ValidatorSigner,
};
use futures::channel::mpsc::UnboundedSender;
use move_core_types::account_address::AccountAddress;
use std::{sync::Arc, time::Duration};

#[async_trait::async_trait]
pub trait TExecutionClient: Send + Sync {
    /// Initialize the execution phase for a new epoch.
    async fn start_epoch(
        &self,
        maybe_consensus_key: Arc<PrivateKey>,
        epoch_state: Arc<EpochState>,
        commit_signer_provider: Arc<dyn CommitSignerProvider>,
        payload_manager: Arc<dyn TPayloadManager>,
        onchain_consensus_config: &OnChainConsensusConfig,
        onchain_execution_config: &OnChainExecutionConfig,
        onchain_randomness_config: &OnChainRandomnessConfig,
        rand_config: Option<RandConfig>,
        fast_rand_config: Option<RandConfig>,
        rand_msg_rx: aptos_channel::Receiver<AccountAddress, IncomingRandGenRequest>,
        highest_committed_round: Round,
    );

    /// This is needed for some DAG tests. Clean this up as a TODO.
    fn get_execution_channel(&self) -> Option<UnboundedSender<OrderedBlocks>>;

    /// Send ordered blocks to the real execution phase through the channel.
    async fn finalize_order(
        &self,
        blocks: Vec<Arc<PipelinedBlock>>,
        ordered_proof: WrappedLedgerInfo,
    ) -> ExecutorResult<()>;

    fn send_commit_msg(
        &self,
        peer_id: AccountAddress,
        commit_msg: IncomingCommitRequest,
    ) -> Result<()>;

    /// Synchronizes for the specified duration and returns the latest synced
    /// ledger info. Note: it is possible that state sync may run longer than
    /// the specified duration (e.g., if the node is very far behind).
    async fn sync_for_duration(
        &self,
        duration: Duration,
    ) -> Result<LedgerInfoWithSignatures, StateSyncError>;

    /// Synchronize to a commit that is not present locally.
    async fn sync_to_target(&self, target: LedgerInfoWithSignatures) -> Result<(), StateSyncError>;

    /// Resets the internal state of the rand and buffer managers.
    async fn reset(&self, target: &LedgerInfoWithSignatures) -> Result<()>;

    /// Shutdown the current processor at the end of the epoch.
    async fn end_epoch(&self);

    /// Returns a pipeline builder for the current epoch.
    fn pipeline_builder(&self, signer: Arc<ValidatorSigner>) -> PipelineBuilder;
}

struct BufferManagerHandle {
    pub execute_tx: Option<UnboundedSender<OrderedBlocks>>,
    pub commit_tx:
        Option<aptos_channel::Sender<AccountAddress, (AccountAddress, IncomingCommitRequest)>>,
    pub reset_tx_to_buffer_manager: Option<UnboundedSender<ResetRequest>>,
    pub reset_tx_to_rand_manager: Option<UnboundedSender<ResetRequest>>,
}

impl BufferManagerHandle {
    pub fn new() -> Self {
        Self {
            execute_tx: None,
            commit_tx: None,
            reset_tx_to_buffer_manager: None,
            reset_tx_to_rand_manager: None,
        }
    }

    pub fn init(
        &mut self,
        execute_tx: UnboundedSender<OrderedBlocks>,
        commit_tx: aptos_channel::Sender<AccountAddress, (AccountAddress, IncomingCommitRequest)>,
        reset_tx_to_buffer_manager: UnboundedSender<ResetRequest>,
        reset_tx_to_rand_manager: Option<UnboundedSender<ResetRequest>>,
    ) {
        self.execute_tx = Some(execute_tx);
        self.commit_tx = Some(commit_tx);
        self.reset_tx_to_buffer_manager = Some(reset_tx_to_buffer_manager);
        self.reset_tx_to_rand_manager = reset_tx_to_rand_manager;
    }

    pub fn reset(
        &mut self,
    ) -> (
        Option<UnboundedSender<ResetRequest>>,
        Option<UnboundedSender<ResetRequest>>,
    ) {
        let reset_tx_to_rand_manager = self.reset_tx_to_rand_manager.take();
        let reset_tx_to_buffer_manager = self.reset_tx_to_buffer_manager.take();
        self.execute_tx = None;
        self.commit_tx = None;
        (reset_tx_to_rand_manager, reset_tx_to_buffer_manager)
    }
}

pub struct DummyExecutionClient;

#[async_trait::async_trait]
impl TExecutionClient for DummyExecutionClient {
    async fn start_epoch(
        &self,
        _maybe_consensus_key: Arc<PrivateKey>,
        _epoch_state: Arc<EpochState>,
        _commit_signer_provider: Arc<dyn CommitSignerProvider>,
        _payload_manager: Arc<dyn TPayloadManager>,
        _onchain_consensus_config: &OnChainConsensusConfig,
        _onchain_execution_config: &OnChainExecutionConfig,
        _onchain_randomness_config: &OnChainRandomnessConfig,
        _rand_config: Option<RandConfig>,
        _fast_rand_config: Option<RandConfig>,
        _rand_msg_rx: aptos_channel::Receiver<AccountAddress, IncomingRandGenRequest>,
        _highest_committed_round: Round,
    ) {
    }

    fn get_execution_channel(&self) -> Option<UnboundedSender<OrderedBlocks>> {
        None
    }

    async fn finalize_order(
        &self,
        _: Vec<Arc<PipelinedBlock>>,
        _: WrappedLedgerInfo,
    ) -> ExecutorResult<()> {
        Ok(())
    }

    fn send_commit_msg(&self, _: AccountAddress, _: IncomingCommitRequest) -> Result<()> {
        Ok(())
    }

    async fn sync_for_duration(
        &self,
        _: Duration,
    ) -> Result<LedgerInfoWithSignatures, StateSyncError> {
        Err(StateSyncError::from(anyhow!(
            "sync_for_duration() is not supported by the DummyExecutionClient!"
        )))
    }

    async fn sync_to_target(&self, _: LedgerInfoWithSignatures) -> Result<(), StateSyncError> {
        Ok(())
    }

    async fn reset(&self, _: &LedgerInfoWithSignatures) -> Result<()> {
        Ok(())
    }

    async fn end_epoch(&self) {}

    fn pipeline_builder(&self, _signer: Arc<ValidatorSigner>) -> PipelineBuilder {
        todo!()
    }
}
