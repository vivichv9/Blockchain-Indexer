use anyhow::Result;
use tracing::info;

use crate::modules::api::{self, ApiAuth, AppState};
use crate::modules::config::AppConfig;
use crate::modules::jobs::JobsService;
use crate::modules::storage::Storage;

pub struct App {
    bind_addr: String,
    auth: ApiAuth,
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
            state: AppState { jobs: jobs_service },
        })
    }

    pub async fn run(self) -> Result<()> {
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
