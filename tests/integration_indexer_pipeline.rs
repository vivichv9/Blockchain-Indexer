use bitcoin_blockchain_indexer::modules::indexer::{
    IndexerPipeline, PersistBlockOutcome, RpcBlock, RpcScriptPubKey, RpcTransaction, RpcVin, RpcVout,
};
use bitcoin_blockchain_indexer::modules::mempool::list_mempool_txids_for_address;
use bitcoin_blockchain_indexer::modules::metrics::MetricsService;
use bitcoin_blockchain_indexer::modules::storage::Storage;
use sqlx::{PgPool, Row};
use testcontainers::core::WaitFor;
use testcontainers::{clients::Cli, GenericImage};

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

fn block_zero() -> RpcBlock {
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

fn block_one() -> RpcBlock {
    RpcBlock {
        hash: "blockhash1".to_string(),
        height: 1,
        prev_hash: Some("blockhash0".to_string()),
        time: 1_700_000_060,
        tx: vec![RpcTransaction {
            txid: "spend1".to_string(),
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
                        hex: "0014change1".to_string(),
                        address: Some("addr1".to_string()),
                        addresses: None,
                    },
                },
                RpcVout {
                    n: 1,
                    value: 30.0,
                    script_pub_key: RpcScriptPubKey {
                        script_type: "pubkeyhash".to_string(),
                        hex: "0014pay1".to_string(),
                        address: Some("addr2".to_string()),
                        addresses: None,
                    },
                },
            ],
        }],
    }
}

#[tokio::test]
#[ignore]
async fn indexer_pipeline_persists_blocks_utxos_and_balances() {
    let Some(pool) = setup_db().await else {
        return;
    };

    let pipeline = IndexerPipeline::new(&pool, MetricsService::new());

    assert_eq!(
        pipeline.persist_block(&block_zero()).await.expect("persist block 0"),
        PersistBlockOutcome::Indexed
    );
    assert_eq!(
        pipeline.persist_block(&block_one()).await.expect("persist block 1"),
        PersistBlockOutcome::Indexed
    );

    let canonical_tip = sqlx::query_scalar::<_, Option<i32>>(
        "SELECT MAX(height) FROM blocks WHERE status = 'canonical'",
    )
    .fetch_one(&pool)
    .await
    .expect("load tip");
    assert_eq!(canonical_tip, Some(1));

    let spent_status = sqlx::query(
        "SELECT status, spent_in_txid
         FROM utxos_current
         WHERE out_txid = 'coinbase0' AND out_vout = 0",
    )
    .fetch_one(&pool)
    .await
    .expect("load spent utxo");
    assert_eq!(spent_status.get::<String, _>("status"), "spent");
    assert_eq!(spent_status.get::<String, _>("spent_in_txid"), "spend1");

    let unspent_rows = sqlx::query(
        "SELECT address, value_sats
         FROM utxos_current
         WHERE status = 'unspent'
         ORDER BY address",
    )
    .fetch_all(&pool)
    .await
    .expect("load unspent utxos");
    assert_eq!(unspent_rows.len(), 2);
    assert_eq!(unspent_rows[0].get::<String, _>("address"), "addr1");
    assert_eq!(unspent_rows[0].get::<i64, _>("value_sats"), 2_000_000_000);
    assert_eq!(unspent_rows[1].get::<String, _>("address"), "addr2");
    assert_eq!(unspent_rows[1].get::<i64, _>("value_sats"), 3_000_000_000);

    let current_balances = sqlx::query(
        "SELECT address, balance_sats
         FROM address_balance_current
         ORDER BY address",
    )
    .fetch_all(&pool)
    .await
    .expect("load current balances");
    assert_eq!(current_balances.len(), 2);
    assert_eq!(current_balances[0].get::<String, _>("address"), "addr1");
    assert_eq!(current_balances[0].get::<i64, _>("balance_sats"), 2_000_000_000);
    assert_eq!(current_balances[1].get::<String, _>("address"), "addr2");
    assert_eq!(current_balances[1].get::<i64, _>("balance_sats"), 3_000_000_000);

    let history_rows = sqlx::query(
        "SELECT address, block_height, balance_sats
         FROM address_balance_history
         ORDER BY block_height, address",
    )
    .fetch_all(&pool)
    .await
    .expect("load balance history");
    assert_eq!(history_rows.len(), 3);
    assert_eq!(history_rows[0].get::<String, _>("address"), "addr1");
    assert_eq!(history_rows[0].get::<i32, _>("block_height"), 0);
    assert_eq!(history_rows[0].get::<i64, _>("balance_sats"), 5_000_000_000);
    assert_eq!(history_rows[1].get::<String, _>("address"), "addr1");
    assert_eq!(history_rows[1].get::<i32, _>("block_height"), 1);
    assert_eq!(history_rows[1].get::<i64, _>("balance_sats"), 2_000_000_000);
    assert_eq!(history_rows[2].get::<String, _>("address"), "addr2");
    assert_eq!(history_rows[2].get::<i32, _>("block_height"), 1);
    assert_eq!(history_rows[2].get::<i64, _>("balance_sats"), 3_000_000_000);
}

#[tokio::test]
#[ignore]
async fn indexer_pipeline_is_idempotent_and_waits_for_previous_height() {
    let Some(pool) = setup_db().await else {
        return;
    };

    let pipeline = IndexerPipeline::new(&pool, MetricsService::new());

    let waiting_block = RpcBlock {
        hash: "blockhash2".to_string(),
        height: 2,
        prev_hash: Some("blockhash1".to_string()),
        time: 1_700_000_120,
        tx: vec![],
    };

    assert_eq!(
        pipeline
            .persist_block(&waiting_block)
            .await
            .expect("wait for previous height"),
        PersistBlockOutcome::WaitingForPreviousHeight
    );

    assert_eq!(
        pipeline.persist_block(&block_zero()).await.expect("persist first time"),
        PersistBlockOutcome::Indexed
    );
    assert_eq!(
        pipeline.persist_block(&block_zero()).await.expect("persist second time"),
        PersistBlockOutcome::AlreadyIndexed
    );
}

#[tokio::test]
#[ignore]
async fn mempool_lookup_returns_transactions_matching_address_in_inputs_and_outputs() {
    let Some(pool) = setup_db().await else {
        return;
    };

    sqlx::query(
        "INSERT INTO transactions (txid, block_height, block_hash, position_in_block, time, status, decoded)
         VALUES
           ('confirmed-prev', 10, 'blockhash10', 0, 1700001000, 'confirmed', '{}'::jsonb),
           ('mempool-in', NULL, NULL, 0, 1700001010, 'mempool', '{}'::jsonb),
           ('mempool-out', NULL, NULL, 0, 1700001020, 'mempool', '{}'::jsonb)",
    )
    .execute(&pool)
    .await
    .expect("seed transactions");

    sqlx::query(
        "INSERT INTO tx_outputs (txid, vout, value_sats, script_type, address, script_hex)
         VALUES
           ('confirmed-prev', 0, 1500, 'pubkeyhash', 'addr1', '0014prev'),
           ('mempool-out', 0, 2500, 'pubkeyhash', 'addr1', '0014out')",
    )
    .execute(&pool)
    .await
    .expect("seed outputs");

    sqlx::query(
        "INSERT INTO tx_inputs (txid, vin, prev_txid, prev_vout, sequence)
         VALUES
           ('mempool-in', 0, 'confirmed-prev', 0, 1)",
    )
    .execute(&pool)
    .await
    .expect("seed inputs");

    let matches = list_mempool_txids_for_address(&pool, "addr1")
        .await
        .expect("lookup mempool txids");

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].txid, "mempool-in");
    assert_eq!(matches[0].addresses, vec!["addr1".to_string()]);
    assert_eq!(matches[1].txid, "mempool-out");
    assert_eq!(matches[1].addresses, vec!["addr1".to_string()]);
}
