use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use reqwest::{Certificate, Client, Identity};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::modules::config::RpcConfig;
use crate::modules::indexer::RpcBlock;

#[derive(Debug, Error)]
pub enum RpcError {
    #[error("failed to read rpc certificate: {0}")]
    Certificate(std::io::Error),
    #[error("invalid rpc certificate: {0}")]
    InvalidCertificate(reqwest::Error),
    #[error("invalid rpc identity: {0}")]
    InvalidIdentity(reqwest::Error),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
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
}

impl RpcClient {
    pub fn from_config(config: &RpcConfig) -> Result<Self, RpcError> {
        let mut builder = Client::builder()
            .connect_timeout(Duration::from_millis(config.timeouts.connect_ms))
            .timeout(Duration::from_millis(config.timeouts.request_ms));

        if let Some(mtls) = &config.mtls {
            let ca_pem = std::fs::read(&mtls.ca_path).map_err(RpcError::Certificate)?;
            let client_cert = std::fs::read(&mtls.client_cert_path).map_err(RpcError::Certificate)?;
            let client_key = std::fs::read(&mtls.client_key_path).map_err(RpcError::Certificate)?;

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
            url: config.url.clone(),
            username: config.auth.username.clone(),
            password: config.auth.password.clone(),
            id: Arc::new(AtomicU64::new(1)),
        })
    }

    pub async fn call<T>(&self, method: &str, params: Value) -> Result<T, RpcError>
    where
        T: DeserializeOwned,
    {
        let id = self.id.fetch_add(1, Ordering::Relaxed);
        let request = RpcRequest {
            jsonrpc: "1.0",
            id,
            method,
            params,
        };

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
            return Err(RpcError::Rpc(format!("{}", error.message)));
        }

        payload
            .result
            .ok_or_else(|| RpcError::Rpc("missing result".to_string()))
    }

    pub async fn get_block_hash(&self, height: u32) -> Result<String, RpcError> {
        self.call("getblockhash", serde_json::json!([height]))
            .await
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
