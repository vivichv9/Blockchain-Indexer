use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use thiserror::Error;
use tracing::warn;
use utoipa::ToSchema;

use crate::modules::config::RpcConfig;
use crate::modules::metrics::MetricsService;
use crate::modules::rpc::{RpcClient, RpcError};

const DEFAULT_CONNECT_TIMEOUT_MS: u64 = 5_000;
const DEFAULT_REQUEST_TIMEOUT_MS: u64 = 30_000;

#[derive(Debug, Error)]
pub enum NodesError {
    #[error("node not found")]
    NotFound,
    #[error("node already exists")]
    AlreadyExists,
    #[error("validation error: {0}")]
    Validation(String),
    #[error(transparent)]
    Rpc(#[from] RpcError),
    #[error(transparent)]
    Storage(#[from] sqlx::Error),
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CreateNodeRequest {
    pub node_id: String,
    pub url: String,
    pub username: String,
    pub password: String,
    pub insecure_skip_verify: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct NodesRunnerConfig {
    pub poll_interval: Duration,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct NodeSummary {
    pub node_id: String,
    pub status: String,
    pub tip_height: i32,
    pub rpc_latency_ms: i32,
    pub last_seen_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct NodeHealthDetails {
    pub node_id: String,
    pub status: String,
    pub tip_height: i32,
    pub tip_hash: String,
    pub rpc_latency_ms: i32,
    pub last_seen_at: DateTime<Utc>,
    pub details: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct NodesService {
    pool: PgPool,
}

#[derive(Clone)]
pub struct NodesRunner {
    pool: PgPool,
    metrics: MetricsService,
    config: NodesRunnerConfig,
}

impl NodesService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn ensure_primary_node(&self, rpc: &RpcConfig) -> Result<(), NodesError> {
        sqlx::query(
            "INSERT INTO nodes_registry
             (node_id, url, username, password, insecure_skip_verify, enabled, updated_at)
             VALUES ($1, $2, $3, $4, $5, TRUE, NOW())
             ON CONFLICT (node_id) DO UPDATE SET
               url = EXCLUDED.url,
               username = EXCLUDED.username,
               password = EXCLUDED.password,
               insecure_skip_verify = EXCLUDED.insecure_skip_verify,
               enabled = TRUE,
               updated_at = NOW()",
        )
        .bind(&rpc.node_id)
        .bind(&rpc.url)
        .bind(&rpc.auth.username)
        .bind(&rpc.auth.password)
        .bind(rpc.insecure_skip_verify)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn create(&self, request: CreateNodeRequest) -> Result<NodeHealthDetails, NodesError> {
        let node = normalize_node_request(request)?;
        let inserted = sqlx::query(
            "INSERT INTO nodes_registry
             (node_id, url, username, password, insecure_skip_verify, enabled, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, NOW())
             ON CONFLICT (node_id) DO NOTHING",
        )
        .bind(&node.node_id)
        .bind(&node.url)
        .bind(&node.username)
        .bind(&node.password)
        .bind(node.insecure_skip_verify)
        .bind(node.enabled)
        .execute(&self.pool)
        .await?
        .rows_affected();

        if inserted == 0 {
            return Err(NodesError::AlreadyExists);
        }

        self.get(&node.node_id).await
    }

    pub async fn list(&self) -> Result<Vec<NodeSummary>, NodesError> {
        let rows: Vec<NodeSummaryRow> = sqlx::query_as(
            "SELECT nr.node_id,
                    COALESCE(nh.status, CASE WHEN nr.enabled THEN 'unknown' ELSE 'disabled' END) AS status,
                    COALESCE(nh.tip_height, 0) AS tip_height,
                    COALESCE(nh.rpc_latency_ms, 0) AS rpc_latency_ms,
                    COALESCE(nh.last_seen_at, nr.created_at) AS last_seen_at
             FROM nodes_registry nr
             LEFT JOIN node_health nh ON nh.node_id = nr.node_id
             ORDER BY nr.node_id",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| NodeSummary {
                node_id: row.node_id,
                status: row.status,
                tip_height: row.tip_height,
                rpc_latency_ms: row.rpc_latency_ms,
                last_seen_at: row.last_seen_at,
            })
            .collect())
    }

    pub async fn get(&self, node_id: &str) -> Result<NodeHealthDetails, NodesError> {
        let row: NodeDetailsRow = sqlx::query_as(
            "SELECT nr.node_id,
                    nr.url,
                    nr.enabled,
                    nr.insecure_skip_verify,
                    COALESCE(nh.status, CASE WHEN nr.enabled THEN 'unknown' ELSE 'disabled' END) AS status,
                    COALESCE(nh.tip_height, 0) AS tip_height,
                    COALESCE(nh.tip_hash, '') AS tip_hash,
                    COALESCE(nh.rpc_latency_ms, 0) AS rpc_latency_ms,
                    COALESCE(nh.last_seen_at, nr.created_at) AS last_seen_at,
                    nh.details AS health_details
             FROM nodes_registry nr
             LEFT JOIN node_health nh ON nh.node_id = nr.node_id
             WHERE nr.node_id = $1",
        )
        .bind(node_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(NodesError::NotFound)?;

        let mut details = serde_json::json!({
            "url": row.url,
            "enabled": row.enabled,
            "insecure_skip_verify": row.insecure_skip_verify,
        });

        if let Some(health_details) = row.health_details {
            details["health"] = health_details;
        }

        Ok(NodeHealthDetails {
            node_id: row.node_id,
            status: row.status,
            tip_height: row.tip_height,
            tip_hash: row.tip_hash,
            rpc_latency_ms: row.rpc_latency_ms,
            last_seen_at: row.last_seen_at,
            details,
        })
    }

    pub async fn tip_height(&self) -> Result<Option<i32>, NodesError> {
        let value = sqlx::query_scalar::<_, i32>(
            "SELECT tip_height
             FROM node_health
             WHERE status = 'ok'
             ORDER BY last_seen_at DESC
             LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(value)
    }

    async fn enabled_nodes(&self) -> Result<Vec<NodeRuntimeConfig>, NodesError> {
        let rows: Vec<NodeRuntimeConfig> = sqlx::query_as(
            "SELECT node_id, url, username, password, insecure_skip_verify
             FROM nodes_registry
             WHERE enabled = TRUE
             ORDER BY node_id",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }
}

impl NodesRunner {
    pub fn new(pool: PgPool, metrics: MetricsService, config: NodesRunnerConfig) -> Self {
        Self { pool, metrics, config }
    }

    pub fn start(&self) {
        let runner = self.clone();

        tokio::spawn(async move {
            loop {
                if let Err(err) = runner.sync_once().await {
                    runner.metrics.increment_error("node_health");
                    warn!(component = "nodes", error = %err, message = "node health sync failed");
                }

                tokio::time::sleep(runner.config.poll_interval).await;
            }
        });
    }

    pub async fn sync_once(&self) -> Result<(), NodesError> {
        let nodes_service = NodesService::new(self.pool.clone());
        for node in nodes_service.enabled_nodes().await? {
            if let Err(err) = sync_node_once(&self.pool, &self.metrics, &node).await {
                self.metrics.increment_error("node_health");
                warn!(
                    component = "nodes",
                    node_id = %node.node_id,
                    error = %err,
                    message = "node health sync failed"
                );
            }
        }

        Ok(())
    }
}

async fn sync_node_once(
    pool: &PgPool,
    metrics: &MetricsService,
    node: &NodeRuntimeConfig,
) -> Result<(), NodesError> {
    let rpc = RpcClient::new(
        &node.url,
        &node.username,
        &node.password,
        node.insecure_skip_verify,
        DEFAULT_CONNECT_TIMEOUT_MS,
        DEFAULT_REQUEST_TIMEOUT_MS,
        None,
    )?;

    let started = Instant::now();
    let tip_result = async {
        let raw_tip_height = rpc.get_block_count().await?;
        let block_height = u32::try_from(raw_tip_height)
            .map_err(|_| RpcError::Rpc("tip height exceeds u32 range".to_string()))?;
        let tip_hash = rpc.get_block_hash(block_height).await?;
        Ok::<(u64, String), RpcError>((raw_tip_height, tip_hash))
    }
    .await;
    let latency_ms = started.elapsed().as_millis().min(i32::MAX as u128) as i32;
    let now = Utc::now();

    match tip_result {
        Ok((raw_tip_height, tip_hash)) => {
            let tip_height = i32::try_from(raw_tip_height)
                .map_err(|_| NodesError::Storage(sqlx::Error::Protocol("tip height exceeds i32 range".into())))?;

            sqlx::query(
                "INSERT INTO node_health
                 (node_id, last_seen_at, tip_height, tip_hash, rpc_latency_ms, status, details)
                 VALUES ($1, $2, $3, $4, $5, 'ok', $6)
                 ON CONFLICT (node_id) DO UPDATE SET
                   last_seen_at = EXCLUDED.last_seen_at,
                   tip_height = EXCLUDED.tip_height,
                   tip_hash = EXCLUDED.tip_hash,
                   rpc_latency_ms = EXCLUDED.rpc_latency_ms,
                   status = EXCLUDED.status,
                   details = EXCLUDED.details",
            )
            .bind(&node.node_id)
            .bind(now)
            .bind(tip_height)
            .bind(tip_hash)
            .bind(latency_ms)
            .bind(serde_json::json!({ "checked_at": now }))
            .execute(pool)
            .await?;
            metrics.observe_db_write_duration("node_health", started.elapsed().as_secs_f64());
        }
        Err(err) => {
            let write_started = Instant::now();
            sqlx::query(
                "INSERT INTO node_health
                 (node_id, last_seen_at, tip_height, tip_hash, rpc_latency_ms, status, details)
                 VALUES ($1, $2, 0, '', $3, 'down', $4)
                 ON CONFLICT (node_id) DO UPDATE SET
                   last_seen_at = EXCLUDED.last_seen_at,
                   tip_height = EXCLUDED.tip_height,
                   tip_hash = EXCLUDED.tip_hash,
                   rpc_latency_ms = EXCLUDED.rpc_latency_ms,
                   status = EXCLUDED.status,
                   details = EXCLUDED.details",
            )
            .bind(&node.node_id)
            .bind(now)
            .bind(latency_ms)
            .bind(serde_json::json!({ "error": err.to_string(), "checked_at": now }))
            .execute(pool)
            .await?;
            metrics.observe_db_write_duration("node_health", write_started.elapsed().as_secs_f64());

            return Err(NodesError::Rpc(err));
        }
    }

    Ok(())
}

fn normalize_node_request(request: CreateNodeRequest) -> Result<CreateNodeRequest, NodesError> {
    if request.node_id.trim().is_empty() {
        return Err(NodesError::Validation("node_id MUST be non-empty".to_string()));
    }

    if request.url.trim().is_empty() {
        return Err(NodesError::Validation("url MUST be non-empty".to_string()));
    }

    if request.username.trim().is_empty() {
        return Err(NodesError::Validation("username MUST be non-empty".to_string()));
    }

    if request.password.is_empty() {
        return Err(NodesError::Validation("password MUST be non-empty".to_string()));
    }

    Ok(CreateNodeRequest {
        node_id: request.node_id.trim().to_string(),
        url: request.url.trim().to_string(),
        username: request.username.trim().to_string(),
        password: request.password,
        insecure_skip_verify: request.insecure_skip_verify,
        enabled: request.enabled,
    })
}

#[derive(Debug, FromRow)]
struct NodeSummaryRow {
    node_id: String,
    status: String,
    tip_height: i32,
    rpc_latency_ms: i32,
    last_seen_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct NodeDetailsRow {
    node_id: String,
    url: String,
    enabled: bool,
    insecure_skip_verify: bool,
    status: String,
    tip_height: i32,
    tip_hash: String,
    rpc_latency_ms: i32,
    last_seen_at: DateTime<Utc>,
    health_details: Option<serde_json::Value>,
}

#[derive(Debug, FromRow)]
struct NodeRuntimeConfig {
    node_id: String,
    url: String,
    username: String,
    password: String,
    insecure_skip_verify: bool,
}

#[cfg(test)]
mod tests {
    use super::{normalize_node_request, CreateNodeRequest};

    #[test]
    fn validates_runtime_node_request() {
        let err = normalize_node_request(CreateNodeRequest {
            node_id: " ".to_string(),
            url: "https://example.com".to_string(),
            username: "user".to_string(),
            password: "pass".to_string(),
            insecure_skip_verify: false,
            enabled: true,
        })
        .expect_err("empty node_id should fail");
        assert!(err.to_string().contains("node_id"));
    }
}
