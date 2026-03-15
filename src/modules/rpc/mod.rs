use std::error::Error;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::{Certificate, Client, Identity};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::modules::config::RpcConfig;
use crate::modules::indexer::{RpcBlock, RpcTransaction};
use crate::modules::metrics::MetricsService;

#[derive(Debug, Error)]
pub enum RpcError {
    #[error("failed to read rpc certificate: {0}")]
    Certificate(std::io::Error),
    #[error("invalid rpc certificate: {0}")]
    InvalidCertificate(reqwest::Error),
    #[error("invalid rpc identity: {0}")]
    InvalidIdentity(reqwest::Error),
    #[error("http error: {0}")]
    Http(String),
    #[error("rpc error: {0}")]
    Rpc(String),
}

#[derive(Clone)]
pub struct RpcClient {
    client: Client,
    url: String,
    username: String,
    password: String,
    id: Arc<AtomicU64>,
    metrics: Option<MetricsService>,
}

impl RpcClient {
    pub fn from_config(config: &RpcConfig) -> Result<Self, RpcError> {
        Self::new(
            &config.url,
            &config.auth.username,
            &config.auth.password,
            config.insecure_skip_verify,
            config.timeouts.connect_ms,
            config.timeouts.request_ms,
            config.mtls
                .as_ref()
                .map(|mtls| {
                    (
                        mtls.ca_path.clone(),
                        mtls.client_cert_path.clone(),
                        mtls.client_key_path.clone(),
                    )
                }),
        )
    }

    pub fn new(
        url: &str,
        username: &str,
        password: &str,
        insecure_skip_verify: bool,
        connect_timeout_ms: u64,
        request_timeout_ms: u64,
        mtls_paths: Option<(std::path::PathBuf, std::path::PathBuf, std::path::PathBuf)>,
    ) -> Result<Self, RpcError> {
        let mut builder = Client::builder()
            .connect_timeout(Duration::from_millis(connect_timeout_ms))
            .timeout(Duration::from_millis(request_timeout_ms));

        if insecure_skip_verify {
            builder = builder.danger_accept_invalid_certs(true);
        }

        if let Some((ca_path, client_cert_path, client_key_path)) = mtls_paths {
            let ca_pem = std::fs::read(&ca_path).map_err(RpcError::Certificate)?;
            let client_cert = std::fs::read(&client_cert_path).map_err(RpcError::Certificate)?;
            let client_key = std::fs::read(&client_key_path).map_err(RpcError::Certificate)?;

            let mut identity_pem = Vec::with_capacity(client_cert.len() + client_key.len() + 1);
            identity_pem.extend_from_slice(&client_cert);
            if !client_cert.ends_with(b"\n") {
                identity_pem.push(b'\n');
            }
            identity_pem.extend_from_slice(&client_key);

            let ca_cert = Certificate::from_pem(&ca_pem).map_err(RpcError::InvalidCertificate)?;
            let identity = Identity::from_pem(&identity_pem).map_err(RpcError::InvalidIdentity)?;

            builder = builder.add_root_certificate(ca_cert).identity(identity);
        }

        let client = builder.build()?;

        Ok(Self {
            client,
            url: url.to_string(),
            username: username.to_string(),
            password: password.to_string(),
            id: Arc::new(AtomicU64::new(1)),
            metrics: None,
        })
    }

    pub fn with_metrics(mut self, metrics: MetricsService) -> Self {
        self.metrics = Some(metrics);
        self
    }

    pub async fn call<T>(&self, method: &str, params: Value) -> Result<T, RpcError>
    where
        T: DeserializeOwned,
    {
        let started = Instant::now();
        let id = self.id.fetch_add(1, Ordering::Relaxed);
        let request = RpcRequest {
            jsonrpc: "1.0",
            id,
            method,
            params,
        };

        let result = async {
            let response = self
                .client
                .post(&self.url)
                .basic_auth(&self.username, Some(&self.password))
                .json(&request)
                .send()
                .await?
                .error_for_status()?;

            let payload: RpcResponse<T> = response.json().await?;
            if let Some(error) = payload.error {
                return Err(RpcError::Rpc(error.message));
            }

            payload
                .result
                .ok_or_else(|| RpcError::Rpc("missing result".to_string()))
        }
        .await;

        if let Some(metrics) = &self.metrics {
            metrics.increment_rpc_request(method);
            metrics.observe_rpc_request_duration(method, started.elapsed().as_secs_f64());
            if result.is_err() {
                metrics.increment_error("rpc");
            }
        }

        result
    }

    pub async fn get_block_hash(&self, height: u32) -> Result<String, RpcError> {
        self.call("getblockhash", serde_json::json!([height]))
            .await
    }

    pub async fn get_block_count(&self) -> Result<u64, RpcError> {
        self.call("getblockcount", serde_json::json!([])).await
    }

    pub async fn get_block(&self, hash: &str, verbosity: u8) -> Result<Value, RpcError> {
        self.call("getblock", serde_json::json!([hash, verbosity]))
            .await
    }

    pub async fn get_block_verbose2(&self, hash: &str) -> Result<RpcBlock, RpcError> {
        self.call("getblock", serde_json::json!([hash, 2])).await
    }

    pub async fn get_raw_transaction(&self, txid: &str, verbose: bool) -> Result<Value, RpcError> {
        self.call("getrawtransaction", serde_json::json!([txid, verbose]))
            .await
    }

    pub async fn get_raw_transaction_verbose(&self, txid: &str) -> Result<RpcTransaction, RpcError> {
        self.call("getrawtransaction", serde_json::json!([txid, true]))
            .await
    }

    pub async fn get_raw_mempool(&self) -> Result<Vec<String>, RpcError> {
        self.call("getrawmempool", serde_json::json!([])).await
    }
}

#[derive(Debug, Serialize)]
struct RpcRequest<'a> {
    jsonrpc: &'static str,
    id: u64,
    method: &'a str,
    params: Value,
}

#[derive(Debug, Deserialize)]
struct RpcResponse<T> {
    result: Option<T>,
    error: Option<RpcResponseError>,
    id: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct RpcResponseError {
    message: String,
}

impl From<reqwest::Error> for RpcError {
    fn from(err: reqwest::Error) -> Self {
        RpcError::Http(describe_reqwest_error(&err))
    }
}

fn describe_reqwest_error(err: &reqwest::Error) -> String {
    let mut parts = Vec::new();

    parts.push(err.to_string());

    if let Some(url) = err.url() {
        parts.push(format!("url={url}"));
    }

    if let Some(status) = err.status() {
        parts.push(format!("status={status}"));
    }

    if err.is_timeout() {
        parts.push("kind=timeout".to_string());
    } else if err.is_connect() {
        parts.push("kind=connect".to_string());
    } else if err.is_request() {
        parts.push("kind=request".to_string());
    } else if err.is_body() {
        parts.push("kind=body".to_string());
    } else if err.is_decode() {
        parts.push("kind=decode".to_string());
    }

    let mut source = err.source();
    while let Some(inner) = source {
        parts.push(format!("source={inner}"));
        source = inner.source();
    }

    parts.join("; ")
}

#[cfg(test)]
mod tests {
    use super::RpcRequest;

    #[test]
    fn rpc_request_serializes() {
        let req = RpcRequest {
            jsonrpc: "1.0",
            id: 1,
            method: "getblockhash",
            params: serde_json::json!([1]),
        };

        let body = serde_json::to_string(&req).expect("serialize");
        assert!(body.contains("getblockhash"));
        assert!(body.contains("\"jsonrpc\":\"1.0\""));
    }
}
