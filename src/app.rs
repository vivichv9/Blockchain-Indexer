use anyhow::Result;
use tracing::info;

use crate::modules::api::{self, ApiAuth, AppState};
use crate::modules::config::AppConfig;
use crate::modules::data::DataService;
use crate::modules::indexer::IndexerService;
use crate::modules::jobs::{JobsRunner, JobsRunnerConfig, JobsService};
use crate::modules::mempool::{MempoolRunner, MempoolRunnerConfig};
use crate::modules::metrics::MetricsService;
use crate::modules::nodes::{NodesRunner, NodesRunnerConfig, NodesService};
use crate::modules::rpc::RpcClient;
use crate::modules::storage::Storage;

pub struct App {
    bind_addr: String,
    auth: ApiAuth,
    jobs_runner: JobsRunner,
    mempool_runner: MempoolRunner,
    nodes_runner: NodesRunner,
    state: AppState,
}

impl App {
    pub async fn bootstrap() -> Result<Self> {
        info!(component = "app", message = "bootstrap started");

        let config = AppConfig::load()?;
        let bind_addr = format!("{}:{}", config.server.bind_host, config.server.bind_port);

        let storage = Storage::connect().await?;
        storage.apply_migrations().await?;
        let jobs_service = JobsService::new(storage.pool().clone());
        jobs_service.sync_from_config(&config.jobs).await?;
        jobs_service.activate_enabled_jobs(&config.jobs).await?;
        let metrics = MetricsService::new();
        let nodes_service = NodesService::new(storage.pool().clone());
        nodes_service.ensure_primary_node(&config.rpc).await?;
        let rpc = RpcClient::from_config(&config.rpc)?.with_metrics(metrics.clone());
        let indexer = IndexerService::new(rpc.clone(), storage.pool().clone(), metrics.clone());
        let mempool_runner = MempoolRunner::new(
            rpc.clone(),
            storage.pool().clone(),
            MempoolRunnerConfig {
                poll_interval: std::time::Duration::from_millis(config.indexer.poll.mempool_interval_ms),
            },
        );
        let nodes_runner = NodesRunner::new(
            storage.pool().clone(),
            metrics.clone(),
            NodesRunnerConfig {
                poll_interval: std::time::Duration::from_millis(config.indexer.poll.tip_interval_ms),
            },
        );
        let jobs_runner = JobsRunner::new(
            jobs_service.clone(),
            rpc,
            indexer,
            metrics.clone(),
            JobsRunnerConfig {
                max_jobs: config.indexer.concurrency.max_jobs as usize,
                poll_interval: std::time::Duration::from_millis(config.indexer.poll.tip_interval_ms),
                blocks_per_batch: config.indexer.batching.blocks_per_batch,
                reorg_depth: config.indexer.reorg_depth,
            },
        );

        info!(
            component = "config",
            network = %config.indexer.network,
            jobs_count = config.jobs.len(),
            message = "configuration loaded"
        );

        Ok(Self {
            bind_addr,
            auth: ApiAuth {
                username: config.server.auth.username,
                password: config.server.auth.password,
            },
            jobs_runner,
            mempool_runner,
            nodes_runner,
            state: AppState {
                jobs: jobs_service,
                data: DataService::new(storage.pool().clone()),
                metrics,
                nodes: nodes_service,
            },
        })
    }

    pub async fn run(self) -> Result<()> {
        self.jobs_runner.start();
        self.mempool_runner.start();
        self.nodes_runner.start();
        let listener = tokio::net::TcpListener::bind(&self.bind_addr).await?;
        info!(
            component = "api",
            bind_addr = %self.bind_addr,
            message = "http server listening"
        );

        axum::serve(listener, api::router(self.auth, self.state)).await?;
        Ok(())
    }
}
