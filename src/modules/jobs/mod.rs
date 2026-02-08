use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use thiserror::Error;

use crate::modules::config::JobConfig;

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

#[derive(Debug, Clone)]
pub struct JobsService {
    pool: Arc<PgPool>,
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
            "SELECT job_id, mode, status, progress_height, updated_at, last_error\
             FROM jobs\
             ORDER BY job_id",
        )
        .fetch_all(self.pool.as_ref())
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| JobSummary {
                job_id: row.job_id,
                mode: row.mode,
                status: row.status,
                progress_height: row.progress_height,
                tip_height: None,
                updated_at: row.updated_at,
                last_error: row.last_error,
            })
            .collect())
    }

    pub async fn get(&self, job_id: &str) -> Result<JobDetails, JobsError> {
        let row: JobDetailsRow = sqlx::query_as(
            "SELECT job_id, mode, status, progress_height, updated_at, last_error, config_snapshot\
             FROM jobs\
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
        self.transition(job_id, &["created"], "running").await
    }

    pub async fn stop(&self, job_id: &str) -> Result<JobDetails, JobsError> {
        self.transition(job_id, &["running", "paused", "failed"], "created")
            .await
    }

    pub async fn pause(&self, job_id: &str) -> Result<JobDetails, JobsError> {
        self.transition(job_id, &["running"], "paused").await
    }

    pub async fn resume(&self, job_id: &str) -> Result<JobDetails, JobsError> {
        self.transition(job_id, &["paused"], "running").await
    }

    pub async fn retry(&self, job_id: &str) -> Result<JobDetails, JobsError> {
        self.transition(job_id, &["failed"], "running").await
    }

    async fn transition(
        &self,
        job_id: &str,
        allowed: &[&str],
        next: &str,
    ) -> Result<JobDetails, JobsError> {
        let row: JobRow = sqlx::query_as(
            "SELECT job_id, mode, status, progress_height, updated_at, last_error\
             FROM jobs\
             WHERE job_id = $1",
        )
        .bind(job_id)
        .fetch_optional(self.pool.as_ref())
        .await?
        .ok_or(JobsError::NotFound)?;

        if !allowed.iter().any(|status| *status == row.status) {
            return Err(JobsError::InvalidTransition(row.status));
        }

        sqlx::query(
            "UPDATE jobs\
             SET status = $2, updated_at = NOW()\
             WHERE job_id = $1",
        )
        .bind(job_id)
        .bind(next)
        .execute(self.pool.as_ref())
        .await?;

        self.get(job_id).await
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
