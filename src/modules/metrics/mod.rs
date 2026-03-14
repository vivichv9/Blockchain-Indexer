use std::collections::HashMap;
use std::fmt::Write;
use std::sync::{Arc, Mutex};

use sqlx::{FromRow, PgPool};

const HISTOGRAM_BUCKETS: [f64; 11] = [0.001, 0.005, 0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0];

#[derive(Debug, Clone, Default)]
pub struct MetricsService {
    inner: Arc<MetricsInner>,
}

#[derive(Debug, Default)]
struct MetricsInner {
    rpc_requests_total: Mutex<HashMap<String, u64>>,
    rpc_request_duration_seconds: Mutex<HashMap<String, Histogram>>,
    db_write_duration_seconds: Mutex<HashMap<String, Histogram>>,
    errors_total: Mutex<HashMap<String, u64>>,
    blocks_processed_total: Mutex<HashMap<String, u64>>,
    txs_processed_total: Mutex<HashMap<String, u64>>,
}

#[derive(Debug, Clone)]
struct Histogram {
    buckets: Vec<u64>,
    count: u64,
    sum: f64,
}

#[derive(Debug, FromRow)]
struct JobMetricsRow {
    job_id: String,
    progress_height: i32,
}

impl MetricsService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn increment_rpc_request(&self, method: &str) {
        increment_counter(&self.inner.rpc_requests_total, method, 1);
    }

    pub fn observe_rpc_request_duration(&self, method: &str, seconds: f64) {
        observe_histogram(&self.inner.rpc_request_duration_seconds, method, seconds);
    }

    pub fn observe_db_write_duration(&self, table: &str, seconds: f64) {
        observe_histogram(&self.inner.db_write_duration_seconds, table, seconds);
    }

    pub fn increment_error(&self, error_type: &str) {
        increment_counter(&self.inner.errors_total, error_type, 1);
    }

    pub fn increment_blocks_processed(&self, job_id: &str, count: u64) {
        increment_counter(&self.inner.blocks_processed_total, job_id, count);
    }

    pub fn increment_txs_processed(&self, job_id: &str, count: u64) {
        increment_counter(&self.inner.txs_processed_total, job_id, count);
    }

    pub async fn render(&self, pool: &PgPool) -> Result<String, sqlx::Error> {
        let tip_height = sqlx::query_scalar::<_, i32>(
            "SELECT tip_height
             FROM node_health
             WHERE status = 'ok'
             ORDER BY last_seen_at DESC
             LIMIT 1",
        )
        .fetch_optional(pool)
        .await?;
        let jobs: Vec<JobMetricsRow> = sqlx::query_as(
            "SELECT job_id, progress_height
             FROM jobs
             ORDER BY job_id",
        )
        .fetch_all(pool)
        .await?;

        let mut output = String::new();

        output.push_str("# HELP indexer_tip_height Latest canonical tip height observed from a healthy node.\n");
        output.push_str("# TYPE indexer_tip_height gauge\n");
        let tip_value = tip_height.unwrap_or_default();
        let _ = writeln!(output, "indexer_tip_height {}", tip_value);

        output.push_str("# HELP indexer_progress_height Indexed progress height by job.\n");
        output.push_str("# TYPE indexer_progress_height gauge\n");
        for job in &jobs {
            let _ = writeln!(
                output,
                "indexer_progress_height{{job_id=\"{}\"}} {}",
                escape_label_value(&job.job_id),
                job.progress_height
            );
        }

        output.push_str("# HELP indexer_lag_blocks Lag in blocks between node tip and job progress.\n");
        output.push_str("# TYPE indexer_lag_blocks gauge\n");
        for job in &jobs {
            let lag = tip_height
                .map(|tip| (tip.saturating_sub(job.progress_height)).max(0))
                .unwrap_or(0);
            let _ = writeln!(
                output,
                "indexer_lag_blocks{{job_id=\"{}\"}} {}",
                escape_label_value(&job.job_id),
                lag
            );
        }

        render_counter_family(
            &mut output,
            "indexer_blocks_processed_total",
            "Total number of canonical blocks persisted by job.",
            "job_id",
            snapshot_counters(&self.inner.blocks_processed_total),
        );
        render_counter_family(
            &mut output,
            "indexer_txs_processed_total",
            "Total number of confirmed transactions persisted by job.",
            "job_id",
            snapshot_counters(&self.inner.txs_processed_total),
        );
        render_counter_family(
            &mut output,
            "indexer_rpc_requests_total",
            "Total number of RPC requests by method.",
            "method",
            snapshot_counters(&self.inner.rpc_requests_total),
        );
        render_counter_family(
            &mut output,
            "indexer_errors_total",
            "Total number of indexer errors by type.",
            "type",
            snapshot_counters(&self.inner.errors_total),
        );
        render_histogram_family(
            &mut output,
            "indexer_rpc_request_duration_seconds",
            "RPC request duration in seconds by method.",
            "method",
            snapshot_histograms(&self.inner.rpc_request_duration_seconds),
        );
        render_histogram_family(
            &mut output,
            "indexer_db_write_duration_seconds",
            "Database write duration in seconds by table.",
            "table",
            snapshot_histograms(&self.inner.db_write_duration_seconds),
        );

        Ok(output)
    }
}

impl Histogram {
    fn new() -> Self {
        Self {
            buckets: vec![0; HISTOGRAM_BUCKETS.len() + 1],
            count: 0,
            sum: 0.0,
        }
    }

    fn observe(&mut self, value: f64) {
        let bucket_index = HISTOGRAM_BUCKETS
            .iter()
            .position(|bound| value <= *bound)
            .unwrap_or(HISTOGRAM_BUCKETS.len());
        self.buckets[bucket_index] += 1;
        self.count += 1;
        self.sum += value;
    }
}

fn increment_counter(map: &Mutex<HashMap<String, u64>>, key: &str, count: u64) {
    let mut guard = map.lock().expect("metrics counter mutex poisoned");
    *guard.entry(key.to_string()).or_insert(0) += count;
}

fn observe_histogram(map: &Mutex<HashMap<String, Histogram>>, key: &str, value: f64) {
    let mut guard = map.lock().expect("metrics histogram mutex poisoned");
    guard
        .entry(key.to_string())
        .or_insert_with(Histogram::new)
        .observe(value);
}

fn snapshot_counters(map: &Mutex<HashMap<String, u64>>) -> Vec<(String, u64)> {
    let guard = map.lock().expect("metrics counter mutex poisoned");
    let mut items: Vec<_> = guard.iter().map(|(key, value)| (key.clone(), *value)).collect();
    items.sort_by(|left, right| left.0.cmp(&right.0));
    items
}

fn snapshot_histograms(map: &Mutex<HashMap<String, Histogram>>) -> Vec<(String, Histogram)> {
    let guard = map.lock().expect("metrics histogram mutex poisoned");
    let mut items: Vec<_> = guard.iter().map(|(key, value)| (key.clone(), value.clone())).collect();
    items.sort_by(|left, right| left.0.cmp(&right.0));
    items
}

fn render_counter_family(
    output: &mut String,
    metric: &str,
    help: &str,
    label_name: &str,
    items: Vec<(String, u64)>,
) {
    let _ = writeln!(output, "# HELP {} {}", metric, help);
    let _ = writeln!(output, "# TYPE {} counter", metric);
    for (label_value, value) in items {
        let _ = writeln!(
            output,
            "{}{{{}=\"{}\"}} {}",
            metric,
            label_name,
            escape_label_value(&label_value),
            value
        );
    }
}

fn render_histogram_family(
    output: &mut String,
    metric: &str,
    help: &str,
    label_name: &str,
    items: Vec<(String, Histogram)>,
) {
    let _ = writeln!(output, "# HELP {} {}", metric, help);
    let _ = writeln!(output, "# TYPE {} histogram", metric);
    for (label_value, histogram) in items {
        let escaped = escape_label_value(&label_value);
        let mut cumulative = 0_u64;
        for (idx, upper_bound) in HISTOGRAM_BUCKETS.iter().enumerate() {
            cumulative += histogram.buckets[idx];
            let _ = writeln!(
                output,
                "{}_bucket{{{}=\"{}\",le=\"{}\"}} {}",
                metric,
                label_name,
                escaped,
                upper_bound,
                cumulative
            );
        }
        cumulative += histogram.buckets[HISTOGRAM_BUCKETS.len()];
        let _ = writeln!(
            output,
            "{}_bucket{{{}=\"{}\",le=\"+Inf\"}} {}",
            metric,
            label_name,
            escaped,
            cumulative
        );
        let _ = writeln!(
            output,
            "{}_sum{{{}=\"{}\"}} {}",
            metric,
            label_name,
            escaped,
            histogram.sum
        );
        let _ = writeln!(
            output,
            "{}_count{{{}=\"{}\"}} {}",
            metric,
            label_name,
            escaped,
            histogram.count
        );
    }
}

fn escape_label_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
