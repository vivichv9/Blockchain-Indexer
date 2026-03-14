use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use thiserror::Error;
use tokio::sync::{Mutex, Semaphore};
use tracing::{error, warn};

use crate::modules::config::JobConfig;
use crate::modules::indexer::{IndexerError, IndexerService};
use crate::modules::rpc::{RpcClient, RpcError};

#[derive(Debug, Clone, Serialize)]
pub struct JobSummary {
    pub job_id: String,
    pub mode: String,
    pub status: String,
    pub progress_height: i32,
    pub tip_height: Option<i32>,
    pub updated_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JobDetails {
    pub job_id: String,
    pub mode: String,
    pub status: String,
    pub progress_height: i32,
    pub updated_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub config_snapshot: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JobActionRequest {
    pub _empty: Option<String>,
}

#[derive(Debug, Error)]
pub enum JobsError {
    #[error("job not found")]
    NotFound,
    #[error("invalid transition from '{0}'")]
    InvalidTransition(String),
    #[error("storage error: {0}")]
    Storage(#[from] sqlx::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Debug, Error)]
enum JobExecutionError {
    #[error(transparent)]
    Jobs(#[from] JobsError),
    #[error(transparent)]
    Rpc(#[from] RpcError),
    #[error(transparent)]
    Indexer(#[from] IndexerError),
    #[error("tip height exceeds i32 range")]
    TipOverflow,
}

#[derive(Debug, Clone, Copy)]
enum JobAction {
    Start,
    Stop,
    Pause,
    Resume,
    Retry,
}

#[derive(Debug, Clone)]
pub struct JobsService {
    pool: Arc<PgPool>,
}

#[derive(Debug, Clone)]
pub struct JobsRunnerConfig {
    pub max_jobs: usize,
    pub poll_interval: Duration,
    pub blocks_per_batch: u32,
}

#[derive(Clone)]
pub struct JobsRunner {
    jobs: JobsService,
    rpc: RpcClient,
    indexer: IndexerService,
    config: JobsRunnerConfig,
    active_jobs: Arc<Mutex<HashSet<String>>>,
}

impl JobsService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool: Arc::new(pool),
        }
    }

    pub async fn sync_from_config(&self, jobs: &[JobConfig]) -> Result<(), JobsError> {
        for job in jobs {
            let snapshot = serde_json::to_value(job)?;
            sqlx::query(
                "INSERT INTO jobs (job_id, mode, status, progress_height, config_snapshot, updated_at) \
                 VALUES ($1, $2, 'created', 0, $3, NOW()) \
                 ON CONFLICT (job_id) DO UPDATE SET \
                   mode = EXCLUDED.mode, \
                   config_snapshot = EXCLUDED.config_snapshot, \
                   updated_at = NOW()",
            )
            .bind(&job.job_id)
            .bind(&job.mode)
            .bind(snapshot)
            .execute(self.pool.as_ref())
            .await?;
        }

        Ok(())
    }

    pub async fn list(&self) -> Result<Vec<JobSummary>, JobsError> {
        let rows: Vec<JobRow> = sqlx::query_as(
            "SELECT job_id, mode, status, progress_height, updated_at, last_error \
             FROM jobs \
             ORDER BY job_id",
        )
        .fetch_all(self.pool.as_ref())
        .await?;

        Ok(rows.into_iter().map(JobSummary::from).collect())
    }

    pub async fn get(&self, job_id: &str) -> Result<JobDetails, JobsError> {
        let row: JobDetailsRow = sqlx::query_as(
            "SELECT job_id, mode, status, progress_height, updated_at, last_error, config_snapshot \
             FROM jobs \
             WHERE job_id = $1",
        )
        .bind(job_id)
        .fetch_optional(self.pool.as_ref())
        .await?
        .ok_or(JobsError::NotFound)?;

        Ok(JobDetails {
            job_id: row.job_id,
            mode: row.mode,
            status: row.status,
            progress_height: row.progress_height,
            updated_at: row.updated_at,
            last_error: row.last_error,
            config_snapshot: row.config_snapshot,
        })
    }

    pub async fn start(&self, job_id: &str) -> Result<JobDetails, JobsError> {
        self.transition(job_id, JobAction::Start).await
    }

    pub async fn stop(&self, job_id: &str) -> Result<JobDetails, JobsError> {
        self.transition(job_id, JobAction::Stop).await
    }

    pub async fn pause(&self, job_id: &str) -> Result<JobDetails, JobsError> {
        self.transition(job_id, JobAction::Pause).await
    }

    pub async fn resume(&self, job_id: &str) -> Result<JobDetails, JobsError> {
        self.transition(job_id, JobAction::Resume).await
    }

    pub async fn retry(&self, job_id: &str) -> Result<JobDetails, JobsError> {
        self.transition(job_id, JobAction::Retry).await
    }

    pub async fn running_job_ids(&self) -> Result<Vec<String>, JobsError> {
        let rows: Vec<JobIdRow> = sqlx::query_as(
            "SELECT job_id \
             FROM jobs \
             WHERE status = 'running' \
             ORDER BY job_id",
        )
        .fetch_all(self.pool.as_ref())
        .await?;

        Ok(rows.into_iter().map(|row| row.job_id).collect())
    }

    pub async fn is_running(&self, job_id: &str) -> Result<bool, JobsError> {
        let row = sqlx::query_scalar::<_, String>("SELECT status FROM jobs WHERE job_id = $1")
            .bind(job_id)
            .fetch_optional(self.pool.as_ref())
            .await?
            .ok_or(JobsError::NotFound)?;

        Ok(row == "running")
    }

    pub async fn update_progress(&self, job_id: &str, height: i32) -> Result<(), JobsError> {
        sqlx::query(
            "UPDATE jobs \
             SET progress_height = GREATEST(progress_height, $2), updated_at = NOW(), last_error = NULL \
             WHERE job_id = $1",
        )
        .bind(job_id)
        .bind(height)
        .execute(self.pool.as_ref())
        .await?;

        Ok(())
    }

    pub async fn mark_failed(&self, job_id: &str, message: &str) -> Result<(), JobsError> {
        sqlx::query(
            "UPDATE jobs \
             SET status = 'failed', last_error = $2, updated_at = NOW() \
             WHERE job_id = $1",
        )
        .bind(job_id)
        .bind(message)
        .execute(self.pool.as_ref())
        .await?;

        Ok(())
    }

    async fn transition(&self, job_id: &str, action: JobAction) -> Result<JobDetails, JobsError> {
        let row: JobRow = sqlx::query_as(
            "SELECT job_id, mode, status, progress_height, updated_at, last_error \
             FROM jobs \
             WHERE job_id = $1",
        )
        .bind(job_id)
        .fetch_optional(self.pool.as_ref())
        .await?
        .ok_or(JobsError::NotFound)?;

        let next = transition_target(action, &row.status)?;

        let last_error = if matches!(action, JobAction::Start | JobAction::Resume | JobAction::Retry) {
            None::<String>
        } else {
            row.last_error.clone()
        };

        sqlx::query(
            "UPDATE jobs \
             SET status = $2, updated_at = NOW(), last_error = $3 \
             WHERE job_id = $1",
        )
        .bind(job_id)
        .bind(next)
        .bind(last_error)
        .execute(self.pool.as_ref())
        .await?;

        self.get(job_id).await
    }
}

impl JobsRunner {
    pub fn new(
        jobs: JobsService,
        rpc: RpcClient,
        indexer: IndexerService,
        config: JobsRunnerConfig,
    ) -> Self {
        Self {
            jobs,
            rpc,
            indexer,
            config,
            active_jobs: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn start(&self) {
        let jobs = self.jobs.clone();
        let rpc = self.rpc.clone();
        let indexer = self.indexer.clone();
        let active_jobs = self.active_jobs.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            let semaphore = Arc::new(Semaphore::new(config.max_jobs.max(1)));

            loop {
                if let Err(err) = schedule_running_jobs(
                    &jobs,
                    &rpc,
                    &indexer,
                    &active_jobs,
                    &semaphore,
                    config.blocks_per_batch,
                )
                .await
                {
                    warn!(component = "jobs", error = %err, message = "job scheduler iteration failed");
                }

                tokio::time::sleep(config.poll_interval).await;
            }
        });
    }
}

async fn schedule_running_jobs(
    jobs: &JobsService,
    rpc: &RpcClient,
    indexer: &IndexerService,
    active_jobs: &Arc<Mutex<HashSet<String>>>,
    semaphore: &Arc<Semaphore>,
    blocks_per_batch: u32,
) -> Result<(), JobsError> {
    for job_id in jobs.running_job_ids().await? {
        let permit = match semaphore.clone().try_acquire_owned() {
            Ok(permit) => permit,
            Err(_) => break,
        };

        let should_spawn = {
            let mut active = active_jobs.lock().await;
            active.insert(job_id.clone())
        };

        if !should_spawn {
            drop(permit);
            continue;
        }

        let jobs = jobs.clone();
        let rpc = rpc.clone();
        let indexer = indexer.clone();
        let active_jobs = active_jobs.clone();

        tokio::spawn(async move {
            let _permit = permit;

            if let Err(err) = execute_job_batch(&jobs, &rpc, &indexer, &job_id, blocks_per_batch).await {
                error!(component = "jobs", job_id = %job_id, error = %err, message = "job batch failed");

                if let Err(mark_err) = jobs.mark_failed(&job_id, &err.to_string()).await {
                    error!(
                        component = "jobs",
                        job_id = %job_id,
                        error = %mark_err,
                        message = "failed to mark job as failed"
                    );
                }
            }

            let mut active = active_jobs.lock().await;
            active.remove(&job_id);
        });
    }

    Ok(())
}

async fn execute_job_batch(
    jobs: &JobsService,
    rpc: &RpcClient,
    indexer: &IndexerService,
    job_id: &str,
    blocks_per_batch: u32,
) -> Result<(), JobExecutionError> {
    if !jobs.is_running(job_id).await? {
        return Ok(());
    }

    let details = jobs.get(job_id).await?;
    let tip_height = i32::try_from(rpc.get_block_count().await?).map_err(|_| JobExecutionError::TipOverflow)?;
    let next_height = details.progress_height.saturating_add(1);

    if next_height > tip_height {
        return Ok(());
    }

    let batch_size = i32::try_from(blocks_per_batch.max(1)).unwrap_or(i32::MAX);
    let target_height = std::cmp::min(
        details.progress_height.saturating_add(batch_size),
        tip_height,
    );

    for height in next_height..=target_height {
        if !jobs.is_running(job_id).await? {
            break;
        }

        indexer.index_height(height as u32).await?;
        jobs.update_progress(job_id, height).await?;
    }

    Ok(())
}

fn transition_target(action: JobAction, current: &str) -> Result<&'static str, JobsError> {
    match (action, current) {
        (JobAction::Start, "created") => Ok("running"),
        (JobAction::Stop, "running") => Ok("created"),
        (JobAction::Stop, "paused") => Ok("created"),
        (JobAction::Stop, "failed") => Ok("created"),
        (JobAction::Pause, "running") => Ok("paused"),
        (JobAction::Resume, "paused") => Ok("running"),
        (JobAction::Retry, "failed") => Ok("running"),
        _ => Err(JobsError::InvalidTransition(current.to_string())),
    }
}

impl From<JobRow> for JobSummary {
    fn from(row: JobRow) -> Self {
        Self {
            job_id: row.job_id,
            mode: row.mode,
            status: row.status,
            progress_height: row.progress_height,
            tip_height: None,
            updated_at: row.updated_at,
            last_error: row.last_error,
        }
    }
}

#[derive(Debug, FromRow)]
struct JobRow {
    job_id: String,
    mode: String,
    status: String,
    progress_height: i32,
    updated_at: Option<DateTime<Utc>>,
    last_error: Option<String>,
}

#[derive(Debug, FromRow)]
struct JobDetailsRow {
    job_id: String,
    mode: String,
    status: String,
    progress_height: i32,
    updated_at: Option<DateTime<Utc>>,
    last_error: Option<String>,
    config_snapshot: serde_json::Value,
}

#[derive(Debug, FromRow)]
struct JobIdRow {
    job_id: String,
}

#[cfg(test)]
mod tests {
    use super::{transition_target, JobAction};

    #[test]
    fn validates_transitions() {
        assert_eq!(transition_target(JobAction::Start, "created").unwrap(), "running");
        assert_eq!(transition_target(JobAction::Stop, "running").unwrap(), "created");
        assert!(transition_target(JobAction::Stop, "created").is_err());
        assert_eq!(transition_target(JobAction::Pause, "running").unwrap(), "paused");
        assert!(transition_target(JobAction::Pause, "created").is_err());
        assert_eq!(transition_target(JobAction::Resume, "paused").unwrap(), "running");
        assert!(transition_target(JobAction::Resume, "running").is_err());
        assert_eq!(transition_target(JobAction::Retry, "failed").unwrap(), "running");
        assert!(transition_target(JobAction::Retry, "running").is_err());
    }
}
