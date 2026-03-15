use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgPool};
use thiserror::Error;
use tracing::warn;
use utoipa::ToSchema;

use crate::modules::metrics::MetricsService;
use crate::modules::rpc::{RpcClient, RpcError};

#[derive(Debug, Error)]
pub enum NodesError {
    #[error("node not found")]
    NotFound,
    #[error(transparent)]
    Rpc(#[from] RpcError),
    #[error(transparent)]
    Storage(#[from] sqlx::Error),
}

#[derive(Debug, Clone)]
pub struct NodeHealthRunnerConfig {
    pub poll_interval: Duration,
    pub node_id: String,
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
pub struct NodeHealthRunner {
    rpc: RpcClient,
    pool: PgPool,
    metrics: MetricsService,
    config: NodeHealthRunnerConfig,
}

impl NodesService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list(&self) -> Result<Vec<NodeSummary>, NodesError> {
        let rows: Vec<NodeSummaryRow> = sqlx::query_as(
            "SELECT node_id, status, tip_height, rpc_latency_ms, last_seen_at
             FROM node_health
             ORDER BY node_id",
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
        let row: NodeHealthRow = sqlx::query_as(
            "SELECT node_id, status, tip_height, tip_hash, rpc_latency_ms, last_seen_at, details
             FROM node_health
             WHERE node_id = $1",
        )
        .bind(node_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(NodesError::NotFound)?;

        Ok(NodeHealthDetails {
            node_id: row.node_id,
            status: row.status,
            tip_height: row.tip_height,
            tip_hash: row.tip_hash,
            rpc_latency_ms: row.rpc_latency_ms,
            last_seen_at: row.last_seen_at,
            details: row.details,
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
}

impl NodeHealthRunner {
    pub fn new(rpc: RpcClient, pool: PgPool, metrics: MetricsService, config: NodeHealthRunnerConfig) -> Self {
        Self {
            rpc,
            pool,
            metrics,
            config,
        }
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
        let started = Instant::now();
        let tip_result = async {
            let raw_tip_height = self.rpc.get_block_count().await?;
            let block_height = u32::try_from(raw_tip_height)
                .map_err(|_| RpcError::Rpc("tip height exceeds u32 range".to_string()))?;
            let tip_hash = self.rpc.get_block_hash(block_height).await?;
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
                .bind(&self.config.node_id)
                .bind(now)
                .bind(tip_height)
                .bind(tip_hash)
                .bind(latency_ms)
                .bind(serde_json::json!({ "checked_at": now }))
                .execute(&self.pool)
                .await?;
                self.metrics.observe_db_write_duration("node_health", started.elapsed().as_secs_f64());
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
                .bind(&self.config.node_id)
                .bind(now)
                .bind(latency_ms)
                .bind(serde_json::json!({ "error": err.to_string(), "checked_at": now }))
                .execute(&self.pool)
                .await?;
                self.metrics
                    .observe_db_write_duration("node_health", write_started.elapsed().as_secs_f64());

                return Err(NodesError::Rpc(err));
            }
        }

        Ok(())
    }
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
struct NodeHealthRow {
    node_id: String,
    status: String,
    tip_height: i32,
    tip_hash: String,
    rpc_latency_ms: i32,
    last_seen_at: DateTime<Utc>,
    details: serde_json::Value,
}
