// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    consensus_observer::publisher::consensus_publisher::ConsensusPublisher,
    counters,
    epoch_manager::EpochManager,
    network::NetworkTask,
    network_interface::{ConsensusMsg, ConsensusNetworkClient},
    persistent_liveness_storage::StorageWriteProxy,
    quorum_store::quorum_store_db::QuorumStoreDB,
    rand::rand_gen::storage::db::RandDb,
    util::time_service::ClockTimeService,
};
use aptos_bounded_executor::BoundedExecutor;
use aptos_config::config::NodeConfig;
use aptos_consensus_notifications::ConsensusNotificationSender;
use aptos_event_notifications::{DbBackedOnChainConfig, ReconfigNotificationListener};
use aptos_logger::prelude::*;
use aptos_mempool::QuorumStoreRequest;
use aptos_network::application::interface::{NetworkClient, NetworkServiceEvents};
use aptos_storage_interface::DbReaderWriter;
use aptos_validator_transaction_pool::VTxnPoolState;
use futures::channel::mpsc;
use std::sync::Arc;
use tokio::runtime::Runtime;
use crate::pipeline::execution_client::DummyExecutionClient;

/// Helper function to start consensus based on configuration and return the runtime
#[allow(clippy::unwrap_used)]
pub fn start_consensus(
    node_config: &NodeConfig,
    network_client: NetworkClient<ConsensusMsg>,
    network_service_events: NetworkServiceEvents<ConsensusMsg>,
    _state_sync_notifier: Arc<dyn ConsensusNotificationSender>,
    consensus_to_mempool_sender: mpsc::Sender<QuorumStoreRequest>,
    aptos_db: DbReaderWriter,
    reconfig_events: ReconfigNotificationListener<DbBackedOnChainConfig>,
    vtxn_pool: VTxnPoolState,
    consensus_publisher: Option<Arc<ConsensusPublisher>>,
) -> (Runtime, Arc<StorageWriteProxy>, Arc<QuorumStoreDB>) {
    let runtime = aptos_runtimes::spawn_named_runtime("consensus".into(), None);
    let storage = Arc::new(StorageWriteProxy::new(node_config, aptos_db.reader.clone()));
    let quorum_store_db = Arc::new(QuorumStoreDB::new(node_config.storage.dir()));

    // let txn_notifier = Arc::new(MempoolNotifier::new(
    //     consensus_to_mempool_sender.clone(),
    //     node_config.consensus.mempool_executed_txn_timeout_ms,
    // ));

    // let execution_proxy = ExecutionProxy::new(
    //     Arc::new(DummyExecutionClient),
    //     txn_notifier,
    //     state_sync_notifier,
    //     node_config.transaction_filters.execution_filter.clone(),
    //     node_config.consensus.enable_pre_commit,
    // );

    let time_service = Arc::new(ClockTimeService::new(runtime.handle().clone()));

    let (timeout_sender, timeout_receiver) =
        aptos_channels::new(1_024, &counters::PENDING_ROUND_TIMEOUTS);
    let (self_sender, self_receiver) =
        aptos_channels::new_unbounded(&counters::PENDING_SELF_MESSAGES);
    let consensus_network_client = ConsensusNetworkClient::new(network_client);
    let bounded_executor = BoundedExecutor::new(
        node_config.consensus.num_bounded_executor_tasks as usize,
        runtime.handle().clone(),
    );
    let rand_storage = Arc::new(RandDb::new(node_config.storage.dir()));

    // let execution_client = Arc::new(ExecutionProxyClient::new(
    //     node_config.consensus.clone(),
    //     Arc::new(execution_proxy),
    //     node_config.validator_network.as_ref().unwrap().peer_id(),
    //     self_sender.clone(),
    //     consensus_network_client.clone(),
    //     bounded_executor.clone(),
    //     rand_storage.clone(),
    //     node_config.consensus_observer,
    //     consensus_publisher.clone(),
    // ));

    let execution_client = Arc::new(DummyExecutionClient);

    let epoch_mgr = EpochManager::new(
        node_config,
        time_service,
        self_sender,
        consensus_network_client,
        timeout_sender,
        consensus_to_mempool_sender,
        execution_client,
        storage.clone(),
        quorum_store_db.clone(),
        reconfig_events,
        bounded_executor,
        aptos_time_service::TimeService::real(),
        vtxn_pool,
        rand_storage,
        consensus_publisher,
    );

    let (network_task, network_receiver) = NetworkTask::new(network_service_events, self_receiver);

    runtime.spawn(network_task.start());
    runtime.spawn(epoch_mgr.start(timeout_receiver, network_receiver));

    debug!("Consensus started.");
    (runtime, storage, quorum_store_db)
}
