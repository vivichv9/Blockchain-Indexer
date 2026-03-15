use std::time::Duration;

use reqwest::StatusCode;
use serde_json::Value;
use sqlx::PgPool;
use testcontainers::core::WaitFor;
use testcontainers::{clients::Cli, GenericImage};
use tokio::time::sleep;

use bitcoin_blockchain_indexer::modules::api::{self, ApiAuth, AppState};
use bitcoin_blockchain_indexer::modules::config::JobConfig;
use bitcoin_blockchain_indexer::modules::data::DataService;
use bitcoin_blockchain_indexer::modules::jobs::JobsService;
use bitcoin_blockchain_indexer::modules::metrics::MetricsService;
use bitcoin_blockchain_indexer::modules::nodes::NodesService;
use bitcoin_blockchain_indexer::modules::storage::Storage;

async fn start_api(bind_addr: &str, auth: ApiAuth, state: AppState) {
    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .expect("bind listener");

    tokio::spawn(async move {
        axum::serve(listener, api::router(auth, state))
            .await
            .expect("server");
    });
}

fn docker_available() -> bool {
    std::process::Command::new("docker")
        .arg("info")
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

async fn setup() -> Option<(String, ApiAuth, PgPool)> {
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

    let jobs = vec![JobConfig {
        job_id: "full-sync".to_string(),
        mode: "all_addresses".to_string(),
        enabled: true,
        addresses: vec![],
    }];

    let jobs_service = JobsService::new(storage.pool().clone());
    jobs_service
        .sync_from_config(&jobs)
        .await
        .expect("sync jobs");

    let auth = ApiAuth {
        username: "admin".to_string(),
        password: "pass".to_string(),
    };

    let state = AppState {
        jobs: jobs_service,
        data: DataService::new(storage.pool().clone()),
        metrics: MetricsService::new(),
        nodes: NodesService::new(storage.pool().clone()),
    };
    let bind_addr = "127.0.0.1:18080".to_string();
    start_api(&bind_addr, auth.clone(), state).await;
    sleep(Duration::from_millis(150)).await;

    Some((bind_addr, auth, storage.pool().clone()))
}

async fn seed_data_api_fixture(pool: &PgPool) {
    sqlx::query(
        "INSERT INTO blocks (height, hash, prev_hash, time, status, meta)
         VALUES
           (100, 'blockhash100', 'blockhash099', 1700000000, 'canonical', '{}'::jsonb),
           (101, 'blockhash101', 'blockhash100', 1700000060, 'canonical', '{}'::jsonb)",
    )
    .execute(pool)
    .await
    .expect("seed blocks");

    sqlx::query(
        "INSERT INTO transactions (txid, block_height, block_hash, position_in_block, time, status, decoded)
         VALUES
           ('prevtx', 100, 'blockhash100', 0, 1700000000, 'confirmed', '{}'::jsonb),
           ('confirmedtx', 101, 'blockhash101', 0, 1700000060, 'confirmed', '{}'::jsonb),
           ('mempooltx', NULL, NULL, 0, 1700000120, 'mempool', '{}'::jsonb)",
    )
    .execute(pool)
    .await
    .expect("seed transactions");

    sqlx::query(
        "INSERT INTO tx_outputs (txid, vout, value_sats, script_type, address, script_hex)
         VALUES
           ('prevtx', 0, 7000, 'pubkeyhash', 'addr1', '0014prev'),
           ('confirmedtx', 0, 5000, 'pubkeyhash', 'addr1', '0014confirmed'),
           ('confirmedtx', 1, 2000, 'pubkeyhash', 'addr2', '0014change'),
           ('mempooltx', 0, 4000, 'pubkeyhash', 'addr1', '0014mempool')",
    )
    .execute(pool)
    .await
    .expect("seed outputs");

    sqlx::query(
        "INSERT INTO tx_inputs (txid, vin, prev_txid, prev_vout, sequence)
         VALUES
           ('confirmedtx', 0, 'prevtx', 0, 1),
           ('mempooltx', 0, 'confirmedtx', 0, 1)",
    )
    .execute(pool)
    .await
    .expect("seed inputs");

    sqlx::query(
        "INSERT INTO utxos_current (out_txid, out_vout, address, value_sats, created_in_txid, spent_in_txid, status)
         VALUES
           ('confirmedtx', 0, 'addr1', 5000, 'confirmedtx', NULL, 'unspent'),
           ('confirmedtx', 1, 'addr2', 2000, 'confirmedtx', NULL, 'unspent')",
    )
    .execute(pool)
    .await
    .expect("seed utxos");

    sqlx::query(
        "INSERT INTO address_balance_current (address, balance_sats, updated_at)
         VALUES
           ('addr1', 5000, NOW()),
           ('addr2', 2000, NOW())",
    )
    .execute(pool)
    .await
    .expect("seed address current balances");

    sqlx::query(
        "INSERT INTO address_balance_history (address, block_height, time, balance_sats)
         VALUES
           ('addr1', 100, 1700000000, 7000),
           ('addr1', 101, 1700000060, 5000),
           ('addr2', 101, 1700000060, 2000)",
    )
    .execute(pool)
    .await
    .expect("seed address balance history");
}

#[tokio::test]
#[ignore]
async fn jobs_lifecycle_api() {
    let Some((bind_addr, auth, _pool)) = setup().await else {
        return;
    };

    let client = reqwest::Client::new();

    let list_resp = client
        .get(format!("http://{bind_addr}/v1/jobs"))
        .basic_auth(&auth.username, Some(&auth.password))
        .send()
        .await
        .expect("list jobs");

    assert_eq!(list_resp.status(), StatusCode::OK);

    let list_body: Value = list_resp.json().await.expect("list body");
    assert_eq!(list_body["items"].as_array().unwrap().len(), 1);

    let start_resp = client
        .post(format!("http://{bind_addr}/v1/jobs/full-sync/start"))
        .basic_auth(&auth.username, Some(&auth.password))
        .send()
        .await
        .expect("start job");

    assert_eq!(start_resp.status(), StatusCode::OK);

    let start_body: Value = start_resp.json().await.expect("start body");
    assert_eq!(start_body["item"]["status"], "running");

    let pause_resp = client
        .post(format!("http://{bind_addr}/v1/jobs/full-sync/pause"))
        .basic_auth(&auth.username, Some(&auth.password))
        .send()
        .await
        .expect("pause job");

    assert_eq!(pause_resp.status(), StatusCode::OK);

    let pause_body: Value = pause_resp.json().await.expect("pause body");
    assert_eq!(pause_body["item"]["status"], "paused");

    let resume_resp = client
        .post(format!("http://{bind_addr}/v1/jobs/full-sync/resume"))
        .basic_auth(&auth.username, Some(&auth.password))
        .send()
        .await
        .expect("resume job");

    assert_eq!(resume_resp.status(), StatusCode::OK);

    let resume_body: Value = resume_resp.json().await.expect("resume body");
    assert_eq!(resume_body["item"]["status"], "running");

    let stop_resp = client
        .post(format!("http://{bind_addr}/v1/jobs/full-sync/stop"))
        .basic_auth(&auth.username, Some(&auth.password))
        .send()
        .await
        .expect("stop job");

    assert_eq!(stop_resp.status(), StatusCode::OK);

    let stop_body: Value = stop_resp.json().await.expect("stop body");
    assert_eq!(stop_body["item"]["status"], "created");
}

#[tokio::test]
#[ignore]
async fn jobs_can_be_created_via_api_without_restart() {
    let Some((bind_addr, auth, _pool)) = setup().await else {
        return;
    };

    let client = reqwest::Client::new();

    let create_resp = client
        .post(format!("http://{bind_addr}/v1/jobs"))
        .basic_auth(&auth.username, Some(&auth.password))
        .json(&serde_json::json!({
            "job_id": "watchlist-runtime",
            "mode": "address_list",
            "enabled": true,
            "addresses": ["addr1", "addr2"]
        }))
        .send()
        .await
        .expect("create job");

    assert_eq!(create_resp.status(), StatusCode::CREATED);
    let create_body: Value = create_resp.json().await.expect("create body");
    assert_eq!(create_body["item"]["job_id"], "watchlist-runtime");
    assert_eq!(create_body["item"]["mode"], "address_list");
    assert_eq!(create_body["item"]["status"], "running");
    assert_eq!(create_body["item"]["config_snapshot"]["addresses"][0], "addr1");

    let list_resp = client
        .get(format!("http://{bind_addr}/v1/jobs"))
        .basic_auth(&auth.username, Some(&auth.password))
        .send()
        .await
        .expect("list jobs");

    assert_eq!(list_resp.status(), StatusCode::OK);
    let list_body: Value = list_resp.json().await.expect("list body");
    let items = list_body["items"].as_array().expect("job items");
    assert!(items.iter().any(|item| item["job_id"] == "watchlist-runtime"));
}

#[tokio::test]
#[ignore]
async fn jobs_requires_auth() {
    let Some((bind_addr, auth, _pool)) = setup().await else {
        return;
    };
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("http://{bind_addr}/v1/jobs"))
        .send()
        .await
        .expect("request");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let resp = client
        .get(format!("http://{bind_addr}/v1/jobs"))
        .basic_auth(&auth.username, Some("wrong"))
        .send()
        .await
        .expect("request");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore]
async fn jobs_not_found() {
    let Some((bind_addr, auth, _pool)) = setup().await else {
        return;
    };
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("http://{bind_addr}/v1/jobs/missing"))
        .basic_auth(&auth.username, Some(&auth.password))
        .send()
        .await
        .expect("request");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore]
async fn jobs_invalid_transition() {
    let Some((bind_addr, auth, _pool)) = setup().await else {
        return;
    };
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("http://{bind_addr}/v1/jobs/full-sync/pause"))
        .basic_auth(&auth.username, Some(&auth.password))
        .send()
        .await
        .expect("request");

    assert_eq!(resp.status(), StatusCode::CONFLICT);
}

#[tokio::test]
#[ignore]
async fn nodes_list_and_details_api() {
    let Some((bind_addr, auth, pool)) = setup().await else {
        return;
    };

    sqlx::query(
        "INSERT INTO nodes_registry
         (node_id, url, username, password, insecure_skip_verify, enabled)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind("btc-mainnet-1")
    .bind("https://rpc.example.com")
    .bind("user")
    .bind("pass")
    .bind(false)
    .bind(true)
    .execute(&pool)
    .await
    .expect("seed node registry");

    sqlx::query(
        "INSERT INTO node_health
         (node_id, last_seen_at, tip_height, tip_hash, rpc_latency_ms, status, details)
         VALUES ($1, NOW(), $2, $3, $4, $5, $6)",
    )
    .bind("btc-mainnet-1")
    .bind(875_000_i32)
    .bind("000000000000000000testhash")
    .bind(42_i32)
    .bind("ok")
    .bind(serde_json::json!({ "source": "integration-test" }))
    .execute(&pool)
    .await
    .expect("seed node health");

    let client = reqwest::Client::new();

    let list_resp = client
        .get(format!("http://{bind_addr}/v1/nodes"))
        .basic_auth(&auth.username, Some(&auth.password))
        .send()
        .await
        .expect("list nodes");

    assert_eq!(list_resp.status(), StatusCode::OK);

    let list_body: Value = list_resp.json().await.expect("list body");
    let items = list_body["items"].as_array().expect("nodes array");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["node_id"], "btc-mainnet-1");
    assert_eq!(items[0]["status"], "ok");
    assert_eq!(items[0]["tip_height"], 875000);

    let detail_resp = client
        .get(format!("http://{bind_addr}/v1/nodes/btc-mainnet-1/health"))
        .basic_auth(&auth.username, Some(&auth.password))
        .send()
        .await
        .expect("get node health");

    assert_eq!(detail_resp.status(), StatusCode::OK);

    let detail_body: Value = detail_resp.json().await.expect("detail body");
    assert_eq!(detail_body["item"]["node_id"], "btc-mainnet-1");
    assert_eq!(detail_body["item"]["tip_hash"], "000000000000000000testhash");
    assert_eq!(detail_body["item"]["rpc_latency_ms"], 42);
    assert_eq!(detail_body["item"]["details"]["source"], "integration-test");
}

#[tokio::test]
#[ignore]
async fn nodes_can_be_created_via_api_without_restart() {
    let Some((bind_addr, auth, _pool)) = setup().await else {
        return;
    };

    let client = reqwest::Client::new();

    let create_resp = client
        .post(format!("http://{bind_addr}/v1/nodes"))
        .basic_auth(&auth.username, Some(&auth.password))
        .json(&serde_json::json!({
            "node_id": "btc-testnet-2",
            "url": "https://rpc.testnet.example.com",
            "username": "user",
            "password": "pass",
            "insecure_skip_verify": true,
            "enabled": true
        }))
        .send()
        .await
        .expect("create node");

    assert_eq!(create_resp.status(), StatusCode::CREATED);
    let create_body: Value = create_resp.json().await.expect("create node body");
    assert_eq!(create_body["item"]["node_id"], "btc-testnet-2");
    assert_eq!(create_body["item"]["status"], "unknown");
    assert_eq!(create_body["item"]["details"]["url"], "https://rpc.testnet.example.com");

    let list_resp = client
        .get(format!("http://{bind_addr}/v1/nodes"))
        .basic_auth(&auth.username, Some(&auth.password))
        .send()
        .await
        .expect("list nodes");

    assert_eq!(list_resp.status(), StatusCode::OK);
    let list_body: Value = list_resp.json().await.expect("list nodes body");
    let items = list_body["items"].as_array().expect("node items");
    assert!(items.iter().any(|item| item["node_id"] == "btc-testnet-2"));
}

#[tokio::test]
#[ignore]
async fn nodes_not_found() {
    let Some((bind_addr, auth, _pool)) = setup().await else {
        return;
    };

    let client = reqwest::Client::new();

    let resp = client
        .get(format!("http://{bind_addr}/v1/nodes/missing/health"))
        .basic_auth(&auth.username, Some(&auth.password))
        .send()
        .await
        .expect("request");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore]
async fn data_api_returns_balance_utxos_transactions_mempool_and_blocks() {
    let Some((bind_addr, auth, pool)) = setup().await else {
        return;
    };
    seed_data_api_fixture(&pool).await;

    let client = reqwest::Client::new();

    let balance_resp = client
        .get(format!("http://{bind_addr}/v1/data/addresses/addr1/balance"))
        .basic_auth(&auth.username, Some(&auth.password))
        .send()
        .await
        .expect("get balance");
    assert_eq!(balance_resp.status(), StatusCode::OK);
    let balance_body: Value = balance_resp.json().await.expect("balance body");
    assert_eq!(balance_body["address"], "addr1");
    assert_eq!(balance_body["balance_sats"], 5000);
    assert_eq!(balance_body["as_of"]["block_height"], 101);

    let historical_balance_resp = client
        .get(format!(
            "http://{bind_addr}/v1/data/addresses/addr1/balance?to_height=100"
        ))
        .basic_auth(&auth.username, Some(&auth.password))
        .send()
        .await
        .expect("get historical balance");
    assert_eq!(historical_balance_resp.status(), StatusCode::OK);
    let historical_balance_body: Value = historical_balance_resp
        .json()
        .await
        .expect("historical balance body");
    assert_eq!(historical_balance_body["balance_sats"], 7000);
    assert_eq!(historical_balance_body["as_of"]["block_height"], 100);

    let balance_history_resp = client
        .get(format!(
            "http://{bind_addr}/v1/data/addresses/addr1/balance/history?from_height=100&to_height=101&limit=10"
        ))
        .basic_auth(&auth.username, Some(&auth.password))
        .send()
        .await
        .expect("get balance history");
    assert_eq!(balance_history_resp.status(), StatusCode::OK);
    let balance_history_body: Value = balance_history_resp
        .json()
        .await
        .expect("balance history body");
    let balance_history_items = balance_history_body["items"]
        .as_array()
        .expect("balance history items");
    assert_eq!(balance_history_body["address"], "addr1");
    assert_eq!(balance_history_body["total"], 2);
    assert_eq!(balance_history_items.len(), 2);
    assert_eq!(balance_history_items[0]["block_height"], 101);
    assert_eq!(balance_history_items[0]["balance_sats"], 5000);
    assert_eq!(balance_history_items[1]["block_height"], 100);
    assert_eq!(balance_history_items[1]["balance_sats"], 7000);

    let utxos_resp = client
        .get(format!("http://{bind_addr}/v1/data/addresses/addr1/utxos"))
        .basic_auth(&auth.username, Some(&auth.password))
        .send()
        .await
        .expect("get utxos");
    assert_eq!(utxos_resp.status(), StatusCode::OK);
    let utxos_body: Value = utxos_resp.json().await.expect("utxos body");
    let utxo_items = utxos_body["items"].as_array().expect("utxo items");
    assert_eq!(utxo_items.len(), 1);
    assert_eq!(utxo_items[0]["out_txid"], "confirmedtx");
    assert_eq!(utxo_items[0]["value_sats"], 5000);

    let txs_resp = client
        .get(format!(
            "http://{bind_addr}/v1/data/transactions?address=addr1&limit=10"
        ))
        .basic_auth(&auth.username, Some(&auth.password))
        .send()
        .await
        .expect("list transactions");
    assert_eq!(txs_resp.status(), StatusCode::OK);
    let txs_body: Value = txs_resp.json().await.expect("transactions body");
    let tx_items = txs_body["items"].as_array().expect("transaction items");
    assert_eq!(txs_body["total"], 2);
    assert_eq!(tx_items.len(), 2);
    assert_eq!(tx_items[0]["txid"], "confirmedtx");
    assert_eq!(tx_items[0]["inputs"][0]["txid"], "prevtx");
    assert_eq!(tx_items[0]["outputs"][0]["address"], "addr1");

    let mempool_resp = client
        .get(format!(
            "http://{bind_addr}/v1/data/transactions/mempool?address=addr1"
        ))
        .basic_auth(&auth.username, Some(&auth.password))
        .send()
        .await
        .expect("list mempool transactions");
    assert_eq!(mempool_resp.status(), StatusCode::OK);
    let mempool_body: Value = mempool_resp.json().await.expect("mempool body");
    let mempool_items = mempool_body["items"].as_array().expect("mempool items");
    assert_eq!(mempool_body["total"], 1);
    assert_eq!(mempool_items.len(), 1);
    assert_eq!(mempool_items[0]["txid"], "mempooltx");
    assert_eq!(mempool_items[0]["status"], "mempool");

    let blocks_resp = client
        .get(format!(
            "http://{bind_addr}/v1/data/blocks?address=addr1&has_txid=confirmedtx"
        ))
        .basic_auth(&auth.username, Some(&auth.password))
        .send()
        .await
        .expect("list blocks");
    assert_eq!(blocks_resp.status(), StatusCode::OK);
    let blocks_body: Value = blocks_resp.json().await.expect("blocks body");
    let block_items = blocks_body["items"].as_array().expect("block items");
    assert_eq!(blocks_body["total"], 1);
    assert_eq!(block_items.len(), 1);
    assert_eq!(block_items[0]["height"], 101);
    assert_eq!(block_items[0]["hash"], "blockhash101");
}

#[tokio::test]
#[ignore]
async fn data_api_validates_pagination_and_returns_empty_unknown_address_state() {
    let Some((bind_addr, auth, pool)) = setup().await else {
        return;
    };
    seed_data_api_fixture(&pool).await;

    let client = reqwest::Client::new();

    let invalid_pagination_resp = client
        .get(format!(
            "http://{bind_addr}/v1/data/transactions?offset=-1&limit=10"
        ))
        .basic_auth(&auth.username, Some(&auth.password))
        .send()
        .await
        .expect("invalid pagination request");
    assert_eq!(invalid_pagination_resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let invalid_pagination_body: Value = invalid_pagination_resp
        .json()
        .await
        .expect("invalid pagination body");
    assert_eq!(invalid_pagination_body["code"], "VALIDATION_ERROR");

    let empty_address_resp = client
        .get(format!("http://{bind_addr}/v1/data/addresses/unknown/balance"))
        .basic_auth(&auth.username, Some(&auth.password))
        .send()
        .await
        .expect("unknown address request");
    assert_eq!(empty_address_resp.status(), StatusCode::OK);
    let empty_address_body: Value = empty_address_resp.json().await.expect("unknown address body");
    assert_eq!(empty_address_body["address"], "unknown");
    assert_eq!(empty_address_body["balance_sats"], 0);
}
