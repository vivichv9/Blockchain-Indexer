use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Json, Router, routing::post};
use bitcoin_blockchain_indexer::modules::config::{BasicAuthResolved, RpcConfig, RpcTimeouts};
use bitcoin_blockchain_indexer::modules::indexer::{
    IndexerPipeline, IndexerService, RpcBlock, RpcScriptPubKey, RpcTransaction, RpcVin, RpcVout,
};
use bitcoin_blockchain_indexer::modules::mempool::MempoolRunner;
use bitcoin_blockchain_indexer::modules::metrics::MetricsService;
use bitcoin_blockchain_indexer::modules::rpc::RpcClient;
use bitcoin_blockchain_indexer::modules::storage::Storage;
use sqlx::{PgPool, Row};
use testcontainers::core::WaitFor;
use testcontainers::{GenericImage, clients::Cli};

fn docker_available() -> bool {
    std::process::Command::new("docker")
        .arg("info")
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

async fn setup_db() -> Option<PgPool> {
    if !docker_available() {
        eprintln!("Docker is not available, skipping integration test.");
        return None;
    }

    let docker = Box::leak(Box::new(Cli::default()));
    let image = GenericImage::new("postgres", "16")
        .with_env_var("POSTGRES_DB", "postgres")
        .with_env_var("POSTGRES_USER", "postgres")
        .with_env_var("POSTGRES_PASSWORD", "postgres")
        .with_exposed_port(5432)
        .with_wait_for(WaitFor::message_on_stdout(
            "database system is ready to accept connections",
        ));
    let node = Box::leak(Box::new(docker.run(image)));
    let port = node.get_host_port_ipv4(5432);

    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
    std::env::set_var("DATABASE_URL", &database_url);
    std::env::set_var("MIGRATIONS_PATH", "migrations");

    let storage = Storage::connect().await.expect("connect storage");
    storage
        .apply_migrations()
        .await
        .expect("apply migrations");

    Some(storage.pool().clone())
}

#[derive(Clone)]
struct MockRpcState {
    block_count: u64,
    block_hashes: HashMap<u32, String>,
    mempool_sequences: VecDeque<Vec<String>>,
    transactions: HashMap<String, RpcTransaction>,
}

#[derive(Clone)]
struct MockRpcServer {
    state: Arc<Mutex<MockRpcState>>,
}

impl MockRpcServer {
    fn new(state: MockRpcState) -> Self {
        Self {
            state: Arc::new(Mutex::new(state)),
        }
    }

    async fn start(self) -> String {
        let router = Router::new()
            .route("/", post(mock_rpc_handler))
            .with_state(self.state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock rpc");
        let addr = listener.local_addr().expect("local addr");

        tokio::spawn(async move {
            axum::serve(listener, router).await.expect("serve mock rpc");
        });

        format!("http://{}", addr)
    }
}

async fn mock_rpc_handler(
    State(state): State<Arc<Mutex<MockRpcState>>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let method = body
        .get("method")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let params = body
        .get("params")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let id = body.get("id").cloned().unwrap_or(serde_json::Value::Null);

    let result = {
        let mut guard = state.lock().expect("mock rpc mutex poisoned");
        match method {
            "getblockcount" => Some(serde_json::json!(guard.block_count)),
            "getblockhash" => {
                let height = params.first().and_then(|value| value.as_u64()).unwrap_or_default() as u32;
                guard
                    .block_hashes
                    .get(&height)
                    .cloned()
                    .map(serde_json::Value::String)
            }
            "getrawmempool" => {
                let response = guard
                    .mempool_sequences
                    .pop_front()
                    .or_else(|| guard.mempool_sequences.back().cloned())
                    .unwrap_or_default();
                Some(serde_json::json!(response))
            }
            "getrawtransaction" => {
                let txid = params.first().and_then(|value| value.as_str()).unwrap_or_default();
                guard
                    .transactions
                    .get(txid)
                    .cloned()
                    .map(|tx| serde_json::to_value(tx).expect("serialize transaction"))
            }
            _ => None,
        }
    };

    match result {
        Some(result) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "result": result,
                "error": null,
                "id": id
            })),
        )
            .into_response(),
        None => (
            StatusCode::OK,
            Json(serde_json::json!({
                "result": null,
                "error": { "message": format!("unsupported method: {method}") },
                "id": id
            })),
        )
            .into_response(),
    }
}

fn rpc_client(url: String) -> RpcClient {
    RpcClient::from_config(&RpcConfig {
        node_id: "mock-node".to_string(),
        url,
        auth: BasicAuthResolved {
            username: "rpcuser".to_string(),
            password: "rpcpass".to_string(),
        },
        mtls: None,
        timeouts: RpcTimeouts {
            connect_ms: 5_000,
            request_ms: 5_000,
        },
    })
    .expect("build rpc client")
}

fn canonical_block_zero() -> RpcBlock {
    RpcBlock {
        hash: "blockhash0".to_string(),
        height: 0,
        prev_hash: None,
        time: 1_700_000_000,
        tx: vec![RpcTransaction {
            txid: "coinbase0".to_string(),
            vin: vec![RpcVin {
                txid: None,
                vout: None,
                sequence: 0,
            }],
            vout: vec![RpcVout {
                n: 0,
                value: 50.0,
                script_pub_key: RpcScriptPubKey {
                    script_type: "pubkeyhash".to_string(),
                    hex: "0014coinbase0".to_string(),
                    address: Some("addr1".to_string()),
                    addresses: None,
                },
            }],
        }],
    }
}

fn canonical_block_one(hash: &str) -> RpcBlock {
    RpcBlock {
        hash: hash.to_string(),
        height: 1,
        prev_hash: Some("blockhash0".to_string()),
        time: 1_700_000_060,
        tx: vec![RpcTransaction {
            txid: format!("spend-{hash}"),
            vin: vec![RpcVin {
                txid: Some("coinbase0".to_string()),
                vout: Some(0),
                sequence: 1,
            }],
            vout: vec![
                RpcVout {
                    n: 0,
                    value: 20.0,
                    script_pub_key: RpcScriptPubKey {
                        script_type: "pubkeyhash".to_string(),
                        hex: "0014addr1".to_string(),
                        address: Some("addr1".to_string()),
                        addresses: None,
                    },
                },
                RpcVout {
                    n: 1,
                    value: 30.0,
                    script_pub_key: RpcScriptPubKey {
                        script_type: "pubkeyhash".to_string(),
                        hex: "0014addr2".to_string(),
                        address: Some("addr2".to_string()),
                        addresses: None,
                    },
                },
            ],
        }],
    }
}

fn mempool_transaction() -> RpcTransaction {
    RpcTransaction {
        txid: "mempooltx".to_string(),
        vin: vec![RpcVin {
            txid: Some("confirmed-prev".to_string()),
            vout: Some(0),
            sequence: 1,
        }],
        vout: vec![RpcVout {
            n: 0,
            value: 0.00003,
            script_pub_key: RpcScriptPubKey {
                script_type: "pubkeyhash".to_string(),
                hex: "0014mempool".to_string(),
                address: Some("addr1".to_string()),
                addresses: None,
            },
        }],
    }
}

#[tokio::test]
#[ignore]
async fn mempool_runner_syncs_new_and_dropped_transactions_via_rpc() {
    let Some(pool) = setup_db().await else {
        return;
    };

    sqlx::query(
        "INSERT INTO transactions (txid, block_height, block_hash, position_in_block, time, status, decoded)
         VALUES ('confirmed-prev', 10, 'blockhash10', 0, 1700001000, 'confirmed', '{}'::jsonb)",
    )
    .execute(&pool)
    .await
    .expect("seed prev transaction");

    sqlx::query(
        "INSERT INTO tx_outputs (txid, vout, value_sats, script_type, address, script_hex)
         VALUES ('confirmed-prev', 0, 1500, 'pubkeyhash', 'addr1', '0014prev')",
    )
    .execute(&pool)
    .await
    .expect("seed prev output");

    let rpc_url = MockRpcServer::new(MockRpcState {
        block_count: 10,
        block_hashes: HashMap::new(),
        mempool_sequences: VecDeque::from(vec![vec!["mempooltx".to_string()], vec![]]),
        transactions: HashMap::from([(String::from("mempooltx"), mempool_transaction())]),
    })
    .start()
    .await;

    let runner = MempoolRunner::new(
        rpc_client(rpc_url),
        pool.clone(),
        bitcoin_blockchain_indexer::modules::mempool::MempoolRunnerConfig {
            poll_interval: Duration::from_secs(1),
        },
    );

    runner.sync_once().await.expect("first sync");

    let saved_row = sqlx::query(
        "SELECT status
         FROM transactions
         WHERE txid = 'mempooltx'",
    )
    .fetch_one(&pool)
    .await
    .expect("load mempool tx");
    assert_eq!(saved_row.get::<String, _>("status"), "mempool");

    let output_row = sqlx::query(
        "SELECT address, value_sats
         FROM tx_outputs
         WHERE txid = 'mempooltx' AND vout = 0",
    )
    .fetch_one(&pool)
    .await
    .expect("load mempool output");
    assert_eq!(output_row.get::<String, _>("address"), "addr1");
    assert_eq!(output_row.get::<i64, _>("value_sats"), 3_000);

    runner.sync_once().await.expect("second sync");

    let dropped_row = sqlx::query(
        "SELECT status
         FROM transactions
         WHERE txid = 'mempooltx'",
    )
    .fetch_one(&pool)
    .await
    .expect("load dropped tx");
    assert_eq!(dropped_row.get::<String, _>("status"), "dropped");
}

#[tokio::test]
#[ignore]
async fn indexer_service_reconcile_chain_marks_orphans_and_rebuilds_balances() {
    let Some(pool) = setup_db().await else {
        return;
    };

    let pipeline = IndexerPipeline::new(&pool, MetricsService::new());
    pipeline
        .persist_block(&canonical_block_zero())
        .await
        .expect("persist block 0");
    pipeline
        .persist_block(&canonical_block_one("oldhash1"))
        .await
        .expect("persist old block 1");

    let rpc_url = MockRpcServer::new(MockRpcState {
        block_count: 1,
        block_hashes: HashMap::from([(0_u32, "blockhash0".to_string()), (1_u32, "newhash1".to_string())]),
        mempool_sequences: VecDeque::new(),
        transactions: HashMap::new(),
    })
    .start()
    .await;

    let indexer = IndexerService::new(rpc_client(rpc_url), pool.clone(), MetricsService::new());
    let divergence = indexer
        .reconcile_chain(5)
        .await
        .expect("reconcile chain");

    assert_eq!(divergence, Some(1));

    let orphaned_block = sqlx::query(
        "SELECT status
         FROM blocks
         WHERE hash = 'oldhash1'",
    )
    .fetch_one(&pool)
    .await
    .expect("load orphaned block");
    assert_eq!(orphaned_block.get::<String, _>("status"), "orphaned");

    let orphaned_tx = sqlx::query(
        "SELECT status
         FROM transactions
         WHERE txid = 'spend-oldhash1'",
    )
    .fetch_one(&pool)
    .await
    .expect("load orphaned tx");
    assert_eq!(orphaned_tx.get::<String, _>("status"), "orphaned");

    let current_balances = sqlx::query(
        "SELECT address, balance_sats
         FROM address_balance_current
         ORDER BY address",
    )
    .fetch_all(&pool)
    .await
    .expect("load current balances");
    assert_eq!(current_balances.len(), 1);
    assert_eq!(current_balances[0].get::<String, _>("address"), "addr1");
    assert_eq!(current_balances[0].get::<i64, _>("balance_sats"), 5_000_000_000);

    let history_rows = sqlx::query(
        "SELECT address, block_height, balance_sats
         FROM address_balance_history
         ORDER BY block_height, address",
    )
    .fetch_all(&pool)
    .await
    .expect("load history");
    assert_eq!(history_rows.len(), 1);
    assert_eq!(history_rows[0].get::<String, _>("address"), "addr1");
    assert_eq!(history_rows[0].get::<i32, _>("block_height"), 0);
    assert_eq!(history_rows[0].get::<i64, _>("balance_sats"), 5_000_000_000);
}
