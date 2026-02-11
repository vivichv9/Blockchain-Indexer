use std::collections::HashSet;
use std::env;
use std::fs::File;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

const DEFAULT_CONFIG_PATH: &str = "config/indexer.yaml";

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config '{path}': {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse yaml config: {0}")]
    Parse(#[from] serde_yaml::Error),
    #[error("validation error: {0}")]
    Validation(String),
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub rpc: RpcConfig,
    pub indexer: IndexerConfig,
    pub jobs: Vec<JobConfig>,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub bind_host: String,
    pub bind_port: u16,
    pub tls: TlsConfig,
    pub auth: BasicAuthResolved,
}

#[derive(Debug, Clone)]
pub struct TlsConfig {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct BasicAuthResolved {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone)]
pub struct RpcConfig {
    pub node_id: String,
    pub url: String,
    pub auth: BasicAuthResolved,
    pub mtls: Option<MtlsConfig>,
    pub timeouts: RpcTimeouts,
}

#[derive(Debug, Clone)]
pub struct MtlsConfig {
    pub ca_path: PathBuf,
    pub client_cert_path: PathBuf,
    pub client_key_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct RpcTimeouts {
    pub connect_ms: u64,
    pub request_ms: u64,
}

#[derive(Debug, Clone)]
pub struct IndexerConfig {
    pub chain: String,
    pub network: String,
    pub reorg_depth: u32,
    pub poll: PollConfig,
    pub concurrency: ConcurrencyConfig,
    pub batching: BatchingConfig,
}

#[derive(Debug, Clone)]
pub struct PollConfig {
    pub tip_interval_ms: u64,
    pub mempool_interval_ms: u64,
}

#[derive(Debug, Clone)]
pub struct ConcurrencyConfig {
    pub max_jobs: u8,
    pub rpc_parallelism: u16,
    pub db_writer_parallelism: u16,
}

#[derive(Debug, Clone)]
pub struct BatchingConfig {
    pub blocks_per_batch: u32,
    pub txs_per_batch: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobConfig {
    pub job_id: String,
    pub mode: String,
    pub enabled: bool,
    pub addresses: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawAppConfig {
    server: RawServerConfig,
    rpc: RawRpcConfig,
    indexer: RawIndexerConfig,
    jobs: Vec<RawJobConfig>,
}

#[derive(Debug, Deserialize)]
struct RawServerConfig {
    bind_host: String,
    bind_port: u16,
    tls: RawTlsConfig,
    auth: RawAuthConfig,
}

#[derive(Debug, Deserialize)]
struct RawTlsConfig {
    cert_path: String,
    key_path: String,
}

#[derive(Debug, Deserialize)]
struct RawAuthConfig {
    basic: RawBasicAuth,
}

#[derive(Debug, Deserialize)]
struct RawBasicAuth {
    username: String,
    password_env: String,
}

#[derive(Debug, Deserialize)]
struct RawRpcConfig {
    node_id: String,
    url: String,
    auth: RawAuthConfig,
    mtls: Option<RawMtlsConfig>,
    timeouts: RawRpcTimeouts,
}

#[derive(Debug, Deserialize)]
struct RawMtlsConfig {
    enabled: Option<bool>,
    ca_path: String,
    client_cert_path: String,
    client_key_path: String,
}

#[derive(Debug, Deserialize)]
struct RawRpcTimeouts {
    connect_ms: u64,
    request_ms: u64,
}

#[derive(Debug, Deserialize)]
struct RawIndexerConfig {
    chain: String,
    network: String,
    reorg_depth: i64,
    poll: RawPollConfig,
    concurrency: RawConcurrencyConfig,
    batching: RawBatchingConfig,
}

#[derive(Debug, Deserialize)]
struct RawPollConfig {
    tip_interval_ms: u64,
    mempool_interval_ms: u64,
}

#[derive(Debug, Deserialize)]
struct RawConcurrencyConfig {
    max_jobs: u8,
    rpc_parallelism: u16,
    db_writer_parallelism: u16,
}

#[derive(Debug, Deserialize)]
struct RawBatchingConfig {
    blocks_per_batch: u32,
    txs_per_batch: u32,
}

#[derive(Debug, Deserialize)]
struct RawJobConfig {
    job_id: String,
    mode: String,
    enabled: bool,
    addresses: Option<Vec<String>>,
}

impl AppConfig {
    pub fn load() -> Result<Self, ConfigError> {
        let path = env::var("INDEXER_CONFIG_PATH").unwrap_or_else(|_| DEFAULT_CONFIG_PATH.to_string());
        Self::load_from_path(Path::new(&path))
    }

    pub fn load_from_path(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.display().to_string(),
            source,
        })?;

        let raw: RawAppConfig = serde_yaml::from_str(&content)?;
        Self::from_raw(raw)
    }

    fn from_raw(raw: RawAppConfig) -> Result<Self, ConfigError> {
        validate_readable_file(&raw.server.tls.cert_path)?;
        validate_readable_file(&raw.server.tls.key_path)?;

        let mtls = match raw.rpc.mtls {
            Some(mtls) => {
                let enabled = mtls.enabled.unwrap_or(true);
                if enabled {
                    validate_readable_file(&mtls.ca_path)?;
                    validate_readable_file(&mtls.client_cert_path)?;
                    validate_readable_file(&mtls.client_key_path)?;
                    Some(MtlsConfig {
                        ca_path: PathBuf::from(mtls.ca_path),
                        client_cert_path: PathBuf::from(mtls.client_cert_path),
                        client_key_path: PathBuf::from(mtls.client_key_path),
                    })
                } else {
                    None
                }
            }
            None => None,
        };

        let server_auth = resolve_basic_auth(&raw.server.auth.basic)?;
        let rpc_auth = resolve_basic_auth(&raw.rpc.auth.basic)?;

        if raw.indexer.reorg_depth < 0 {
            return Err(ConfigError::Validation(
                "indexer.reorg_depth MUST be >= 0".to_string(),
            ));
        }

        if !matches!(
            raw.indexer.network.as_str(),
            "mainnet" | "testnet" | "signet" | "regtest"
        ) {
            return Err(ConfigError::Validation(
                "indexer.network MUST be one of: mainnet|testnet|signet|regtest".to_string(),
            ));
        }

        let mut seen_job_ids = HashSet::new();
        let mut jobs = Vec::with_capacity(raw.jobs.len());

        for job in raw.jobs {
            if !seen_job_ids.insert(job.job_id.clone()) {
                return Err(ConfigError::Validation(format!(
                    "jobs[*].job_id MUST be unique: {}",
                    job.job_id
                )));
            }

            if !matches!(job.mode.as_str(), "all_addresses" | "address_list") {
                return Err(ConfigError::Validation(format!(
                    "jobs[*].mode has unsupported value: {}",
                    job.mode
                )));
            }

            let addresses = job.addresses.unwrap_or_default();
            if job.mode == "address_list" && addresses.is_empty() {
                return Err(ConfigError::Validation(format!(
                    "jobs[{job_id}].addresses MUST be non-empty for address_list mode",
                    job_id = job.job_id
                )));
            }

            jobs.push(JobConfig {
                job_id: job.job_id,
                mode: job.mode,
                enabled: job.enabled,
                addresses,
            });
        }

        Ok(AppConfig {
            server: ServerConfig {
                bind_host: raw.server.bind_host,
                bind_port: raw.server.bind_port,
                tls: TlsConfig {
                    cert_path: PathBuf::from(raw.server.tls.cert_path),
                    key_path: PathBuf::from(raw.server.tls.key_path),
                },
                auth: server_auth,
            },
            rpc: RpcConfig {
                node_id: raw.rpc.node_id,
                url: raw.rpc.url,
                auth: rpc_auth,
                mtls,
                timeouts: RpcTimeouts {
                    connect_ms: raw.rpc.timeouts.connect_ms,
                    request_ms: raw.rpc.timeouts.request_ms,
                },
            },
            indexer: IndexerConfig {
                chain: raw.indexer.chain,
                network: raw.indexer.network,
                reorg_depth: raw.indexer.reorg_depth as u32,
                poll: PollConfig {
                    tip_interval_ms: raw.indexer.poll.tip_interval_ms,
                    mempool_interval_ms: raw.indexer.poll.mempool_interval_ms,
                },
                concurrency: ConcurrencyConfig {
                    max_jobs: raw.indexer.concurrency.max_jobs,
                    rpc_parallelism: raw.indexer.concurrency.rpc_parallelism,
                    db_writer_parallelism: raw.indexer.concurrency.db_writer_parallelism,
                },
                batching: BatchingConfig {
                    blocks_per_batch: raw.indexer.batching.blocks_per_batch,
                    txs_per_batch: raw.indexer.batching.txs_per_batch,
                },
            },
            jobs,
        })
    }
}

fn validate_readable_file(path: &str) -> Result<(), ConfigError> {
    File::open(path).map_err(|err| {
        ConfigError::Validation(format!("file '{path}' MUST exist and be readable: {err}"))
    })?;
    Ok(())
}

fn resolve_basic_auth(raw: &RawBasicAuth) -> Result<BasicAuthResolved, ConfigError> {
    if raw.password_env.trim().is_empty() {
        return Err(ConfigError::Validation(
            "password_env MUST be non-empty".to_string(),
        ));
    }

    let password = env::var(&raw.password_env).map_err(|_| {
        ConfigError::Validation(format!(
            "env variable '{}' MUST be set",
            raw.password_env
        ))
    })?;

    Ok(BasicAuthResolved {
        username: raw.username.clone(),
        password,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::AppConfig;

    fn write_file(path: &std::path::Path) {
        fs::write(path, b"x").expect("write file");
    }

    fn make_yaml(paths: &[(&str, String)], jobs: &str, reorg_depth: i64) -> String {
        let mut p = std::collections::HashMap::new();
        for (k, v) in paths {
            p.insert(*k, v.clone());
        }

        format!(
            r#"
server:
  bind_host: "0.0.0.0"
  bind_port: 8443
  tls:
    cert_path: "{server_cert}"
    key_path: "{server_key}"
  auth:
    basic:
      username: "admin"
      password_env: "INDEXER_API_PASSWORD"
rpc:
  node_id: "btc-mainnet-1"
  url: "https://nginx-rpc:443"
  auth:
    basic:
      username: "rpcuser"
      password_env: "BITCOIN_RPC_PASSWORD"
  mtls:
    ca_path: "{ca}"
    client_cert_path: "{client_cert}"
    client_key_path: "{client_key}"
  timeouts:
    connect_ms: 5000
    request_ms: 30000
indexer:
  chain: "bitcoin"
  network: "mainnet"
  reorg_depth: {reorg_depth}
  poll:
    tip_interval_ms: 5000
    mempool_interval_ms: 3000
  concurrency:
    max_jobs: 5
    rpc_parallelism: 8
    db_writer_parallelism: 4
  batching:
    blocks_per_batch: 50
    txs_per_batch: 5000
jobs:
{jobs}
"#,
            server_cert = p["server_cert"],
            server_key = p["server_key"],
            ca = p["ca"],
            client_cert = p["client_cert"],
            client_key = p["client_key"],
            reorg_depth = reorg_depth,
            jobs = jobs
        )
    }

    #[test]
    fn loads_valid_config() {
        let dir = tempdir().expect("tempdir");

        let server_cert = dir.path().join("server.crt");
        let server_key = dir.path().join("server.key");
        let ca = dir.path().join("ca.crt");
        let client_cert = dir.path().join("client.crt");
        let client_key = dir.path().join("client.key");

        write_file(&server_cert);
        write_file(&server_key);
        write_file(&ca);
        write_file(&client_cert);
        write_file(&client_key);

        let yaml = make_yaml(
            &[
                ("server_cert", server_cert.display().to_string()),
                ("server_key", server_key.display().to_string()),
                ("ca", ca.display().to_string()),
                ("client_cert", client_cert.display().to_string()),
                ("client_key", client_key.display().to_string()),
            ],
            "  - job_id: \"full-sync\"\n    mode: \"all_addresses\"\n    enabled: true\n",
            12,
        );

        let yaml_path = dir.path().join("indexer.yaml");
        fs::write(&yaml_path, yaml).expect("write yaml");

        std::env::set_var("INDEXER_API_PASSWORD", "api-pass");
        std::env::set_var("BITCOIN_RPC_PASSWORD", "rpc-pass");

        let cfg = AppConfig::load_from_path(&yaml_path).expect("config should load");
        assert_eq!(cfg.server.auth.username, "admin");
        assert_eq!(cfg.rpc.auth.username, "rpcuser");
        assert_eq!(cfg.jobs.len(), 1);
    }

    #[test]
    fn rejects_negative_reorg_depth() {
        let dir = tempdir().expect("tempdir");

        let server_cert = dir.path().join("server.crt");
        let server_key = dir.path().join("server.key");
        let ca = dir.path().join("ca.crt");
        let client_cert = dir.path().join("client.crt");
        let client_key = dir.path().join("client.key");

        write_file(&server_cert);
        write_file(&server_key);
        write_file(&ca);
        write_file(&client_cert);
        write_file(&client_key);

        let yaml = make_yaml(
            &[
                ("server_cert", server_cert.display().to_string()),
                ("server_key", server_key.display().to_string()),
                ("ca", ca.display().to_string()),
                ("client_cert", client_cert.display().to_string()),
                ("client_key", client_key.display().to_string()),
            ],
            "  - job_id: \"full-sync\"\n    mode: \"all_addresses\"\n    enabled: true\n",
            -1,
        );

        let yaml_path = dir.path().join("indexer.yaml");
        fs::write(&yaml_path, yaml).expect("write yaml");

        std::env::set_var("INDEXER_API_PASSWORD", "api-pass");
        std::env::set_var("BITCOIN_RPC_PASSWORD", "rpc-pass");

        let err = AppConfig::load_from_path(&yaml_path).expect_err("should fail");
        assert!(err.to_string().contains("reorg_depth"));
    }

    #[test]
    fn rejects_invalid_network() {
        let dir = tempdir().expect("tempdir");

        let server_cert = dir.path().join("server.crt");
        let server_key = dir.path().join("server.key");
        let ca = dir.path().join("ca.crt");
        let client_cert = dir.path().join("client.crt");
        let client_key = dir.path().join("client.key");

        write_file(&server_cert);
        write_file(&server_key);
        write_file(&ca);
        write_file(&client_cert);
        write_file(&client_key);

        let mut yaml = make_yaml(
            &[
                ("server_cert", server_cert.display().to_string()),
                ("server_key", server_key.display().to_string()),
                ("ca", ca.display().to_string()),
                ("client_cert", client_cert.display().to_string()),
                ("client_key", client_key.display().to_string()),
            ],
            "  - job_id: \"full-sync\"\n    mode: \"all_addresses\"\n    enabled: true\n",
            12,
        );

        yaml = yaml.replace("network: \"mainnet\"", "network: \"unknown\"");

        let yaml_path = dir.path().join("indexer.yaml");
        fs::write(&yaml_path, yaml).expect("write yaml");

        std::env::set_var("INDEXER_API_PASSWORD", "api-pass");
        std::env::set_var("BITCOIN_RPC_PASSWORD", "rpc-pass");

        let err = AppConfig::load_from_path(&yaml_path).expect_err("should fail");
        assert!(err.to_string().contains("indexer.network"));
    }

    #[test]
    fn rejects_duplicate_job_ids() {
        let dir = tempdir().expect("tempdir");

        let server_cert = dir.path().join("server.crt");
        let server_key = dir.path().join("server.key");
        let ca = dir.path().join("ca.crt");
        let client_cert = dir.path().join("client.crt");
        let client_key = dir.path().join("client.key");

        write_file(&server_cert);
        write_file(&server_key);
        write_file(&ca);
        write_file(&client_cert);
        write_file(&client_key);

        let jobs = "  - job_id: \"full-sync\"\n    mode: \"all_addresses\"\n    enabled: true\n  - job_id: \"full-sync\"\n    mode: \"all_addresses\"\n    enabled: true\n";

        let yaml = make_yaml(
            &[
                ("server_cert", server_cert.display().to_string()),
                ("server_key", server_key.display().to_string()),
                ("ca", ca.display().to_string()),
                ("client_cert", client_cert.display().to_string()),
                ("client_key", client_key.display().to_string()),
            ],
            jobs,
            12,
        );

        let yaml_path = dir.path().join("indexer.yaml");
        fs::write(&yaml_path, yaml).expect("write yaml");

        std::env::set_var("INDEXER_API_PASSWORD", "api-pass");
        std::env::set_var("BITCOIN_RPC_PASSWORD", "rpc-pass");

        let err = AppConfig::load_from_path(&yaml_path).expect_err("should fail");
        assert!(err.to_string().contains("job_id MUST be unique"));
    }

    #[test]
    fn rejects_empty_address_list() {
        let dir = tempdir().expect("tempdir");

        let server_cert = dir.path().join("server.crt");
        let server_key = dir.path().join("server.key");
        let ca = dir.path().join("ca.crt");
        let client_cert = dir.path().join("client.crt");
        let client_key = dir.path().join("client.key");

        write_file(&server_cert);
        write_file(&server_key);
        write_file(&ca);
        write_file(&client_cert);
        write_file(&client_key);

        let jobs = "  - job_id: \"watchlist\"\n    mode: \"address_list\"\n    enabled: true\n";

        let yaml = make_yaml(
            &[
                ("server_cert", server_cert.display().to_string()),
                ("server_key", server_key.display().to_string()),
                ("ca", ca.display().to_string()),
                ("client_cert", client_cert.display().to_string()),
                ("client_key", client_key.display().to_string()),
            ],
            jobs,
            12,
        );

        let yaml_path = dir.path().join("indexer.yaml");
        fs::write(&yaml_path, yaml).expect("write yaml");

        std::env::set_var("INDEXER_API_PASSWORD", "api-pass");
        std::env::set_var("BITCOIN_RPC_PASSWORD", "rpc-pass");

        let err = AppConfig::load_from_path(&yaml_path).expect_err("should fail");
        assert!(err.to_string().contains("addresses MUST be non-empty"));
    }

    #[test]
    fn rejects_missing_password_env() {
        let dir = tempdir().expect("tempdir");

        let server_cert = dir.path().join("server.crt");
        let server_key = dir.path().join("server.key");
        let ca = dir.path().join("ca.crt");
        let client_cert = dir.path().join("client.crt");
        let client_key = dir.path().join("client.key");

        write_file(&server_cert);
        write_file(&server_key);
        write_file(&ca);
        write_file(&client_cert);
        write_file(&client_key);

        let mut yaml = make_yaml(
            &[
                ("server_cert", server_cert.display().to_string()),
                ("server_key", server_key.display().to_string()),
                ("ca", ca.display().to_string()),
                ("client_cert", client_cert.display().to_string()),
                ("client_key", client_key.display().to_string()),
            ],
            "  - job_id: \"full-sync\"\n    mode: \"all_addresses\"\n    enabled: true\n",
            12,
        );

        yaml = yaml.replace("password_env: \"INDEXER_API_PASSWORD\"", "password_env: \"MISSING_ENV\"");

        let yaml_path = dir.path().join("indexer.yaml");
        fs::write(&yaml_path, yaml).expect("write yaml");

        std::env::remove_var("MISSING_ENV");
        std::env::set_var("BITCOIN_RPC_PASSWORD", "rpc-pass");

        let err = AppConfig::load_from_path(&yaml_path).expect_err("should fail");
        assert!(err.to_string().contains("MISSING_ENV"));
    }

    #[test]
    fn rejects_missing_files() {
        let dir = tempdir().expect("tempdir");

        let server_cert = dir.path().join("server.crt");
        let server_key = dir.path().join("server.key");
        let ca = dir.path().join("ca.crt");
        let client_cert = dir.path().join("client.crt");
        let client_key = dir.path().join("client.key");

        write_file(&server_cert);
        write_file(&server_key);
        write_file(&ca);
        write_file(&client_cert);
        // client_key intentionally missing

        let yaml = make_yaml(
            &[
                ("server_cert", server_cert.display().to_string()),
                ("server_key", server_key.display().to_string()),
                ("ca", ca.display().to_string()),
                ("client_cert", client_cert.display().to_string()),
                ("client_key", client_key.display().to_string()),
            ],
            "  - job_id: \"full-sync\"\n    mode: \"all_addresses\"\n    enabled: true\n",
            12,
        );

        let yaml_path = dir.path().join("indexer.yaml");
        fs::write(&yaml_path, yaml).expect("write yaml");

        std::env::set_var("INDEXER_API_PASSWORD", "api-pass");
        std::env::set_var("BITCOIN_RPC_PASSWORD", "rpc-pass");

        let err = AppConfig::load_from_path(&yaml_path).expect_err("should fail");
        assert!(err.to_string().contains("client.key"));
    }

    #[test]
    fn allows_mtls_disabled_without_files() {
        let dir = tempdir().expect("tempdir");

        let server_cert = dir.path().join("server.crt");
        let server_key = dir.path().join("server.key");
        let ca = dir.path().join("ca.crt");
        let client_cert = dir.path().join("client.crt");
        let client_key = dir.path().join("client.key");

        write_file(&server_cert);
        write_file(&server_key);
        write_file(&ca);
        write_file(&client_cert);
        write_file(&client_key);

        let mut yaml = make_yaml(
            &[
                ("server_cert", server_cert.display().to_string()),
                ("server_key", server_key.display().to_string()),
                ("ca", ca.display().to_string()),
                ("client_cert", client_cert.display().to_string()),
                ("client_key", client_key.display().to_string()),
            ],
            "  - job_id: \"full-sync\"\n    mode: \"all_addresses\"\n    enabled: true\n",
            12,
        );

        yaml = yaml.replace(
            "mtls:\n    ca_path:",
            "mtls:\n    enabled: false\n    ca_path:",
        );

        let yaml_path = dir.path().join("indexer.yaml");
        fs::write(&yaml_path, yaml).expect("write yaml");

        std::env::set_var("INDEXER_API_PASSWORD", "api-pass");
        std::env::set_var("BITCOIN_RPC_PASSWORD", "rpc-pass");

        let cfg = AppConfig::load_from_path(&yaml_path).expect("config should load");
        assert!(cfg.rpc.mtls.is_none());
    }
}
