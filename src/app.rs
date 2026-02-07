use anyhow::Result;
use tracing::info;

use crate::modules::api::{self, ApiAuth};
use crate::modules::config::AppConfig;

pub struct App {
    bind_addr: String,
    auth: ApiAuth,
}

impl App {
    pub async fn bootstrap() -> Result<Self> {
        info!(component = "app", message = "bootstrap started");

        let config = AppConfig::load()?;
        let bind_addr = format!("{}:{}", config.server.bind_host, config.server.bind_port);

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
        })
    }

    pub async fn run(self) -> Result<()> {
        let listener = tokio::net::TcpListener::bind(&self.bind_addr).await?;
        info!(
            component = "api",
            bind_addr = %self.bind_addr,
            message = "http server listening"
        );

        axum::serve(listener, api::router(self.auth)).await?;
        Ok(())
    }
}
