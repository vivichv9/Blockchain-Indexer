use std::future::Future;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

use serde::Deserialize;
use serde_json::Value;
use sqlx::{Executor, FromRow, PgConnection, PgPool, Postgres, Row};
use thiserror::Error;

use crate::modules::metrics::MetricsService;
use crate::modules::storage::repo::{
    AddressBalancesRepo, AddressLookupRepo, BlockRecord, BlocksRepo, TransactionRecord,
    TransactionsRepo, TxInputRecord, TxInputsRepo, TxOutputRecord, TxOutputsRepo, UtxoCreateRecord,
    UtxosRepo,
};

#[derive(Debug, Deserialize, serde::Serialize)]
pub struct RpcBlock {
    pub hash: String,
    pub height: i32,
    #[serde(rename = "previousblockhash")]
    pub prev_hash: Option<String>,
    pub time: i64,
    pub tx: Vec<RpcTransaction>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
pub struct RpcTransaction {
    pub txid: String,
    pub vin: Vec<RpcVin>,
    pub vout: Vec<RpcVout>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
pub struct RpcVin {
    pub txid: Option<String>,
    pub vout: Option<i32>,
    pub sequence: i64,
}

#[derive(Debug, Deserialize, serde::Serialize)]
pub struct RpcVout {
    pub n: i32,
    pub value: f64,
    #[serde(rename = "scriptPubKey")]
    pub script_pub_key: RpcScriptPubKey,
}

#[derive(Debug, Deserialize, serde::Serialize)]
pub struct RpcScriptPubKey {
    #[serde(rename = "type")]
    pub script_type: String,
    pub hex: String,
    pub address: Option<String>,
    pub addresses: Option<Vec<String>>,
}

pub struct IndexerPipeline<'a> {
    pool: &'a PgPool,
    metrics: MetricsService,
}

const CHAIN_STATE_LOCK_KEY: i64 = -1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistBlockOutcome {
    Indexed,
    AlreadyIndexed,
    WaitingForPreviousHeight,
}

impl<'a> IndexerPipeline<'a> {
    pub fn new(pool: &'a PgPool, metrics: MetricsService) -> Self {
        Self { pool, metrics }
    }

    pub async fn persist_block(&self, block: &RpcBlock) -> Result<PersistBlockOutcome, sqlx::Error> {
        let mut db_tx = self.pool.begin().await?;
        acquire_chain_state_lock(&mut *db_tx).await?;
        acquire_height_lock(&mut *db_tx, block.height).await?;

        if let Some(existing_hash) = canonical_block_hash_at_height(&mut *db_tx, block.height).await? {
            db_tx.commit().await?;
            if existing_hash == block.hash {
                return Ok(PersistBlockOutcome::AlreadyIndexed);
            }

            return Err(sqlx::Error::Protocol(format!(
                "height {} is already occupied by canonical block {}",
                block.height, existing_hash
            )));
        }

        if block.height > 0 && canonical_block_hash_at_height(&mut *db_tx, block.height - 1).await?.is_none() {
            db_tx.commit().await?;
            return Ok(PersistBlockOutcome::WaitingForPreviousHeight);
        }

        let blocks = BlocksRepo::new(self.pool);
        let txs = TransactionsRepo::new(self.pool);
        let inputs = TxInputsRepo::new(self.pool);
        let outputs = TxOutputsRepo::new(self.pool);
        let utxos = UtxosRepo::new(self.pool);
        let address_balances = AddressBalancesRepo::new(self.pool);
        let address_lookup = AddressLookupRepo::new(self.pool);
        let mut address_deltas: HashMap<String, i64> = HashMap::new();
        let mut touched_addresses: HashSet<String> = HashSet::new();

        let block_record = BlockRecord {
            height: block.height,
            hash: block.hash.clone(),
            prev_hash: block.prev_hash.clone().unwrap_or_default(),
            time: block.time,
            status: "canonical".to_string(),
            meta: serde_json::json!({}),
        };
        observe_db_write(&self.metrics, "blocks", blocks.upsert(&mut *db_tx, &block_record)).await?;

        for (tx_position, tx) in block.tx.iter().enumerate() {
            let tx_record = TransactionRecord {
                txid: tx.txid.clone(),
                block_height: Some(block.height),
                block_hash: Some(block.hash.clone()),
                position_in_block: tx_position as i32,
                time: block.time,
                status: "confirmed".to_string(),
                decoded: serde_json::to_value(tx).unwrap_or(Value::Null),
            };
            observe_db_write(&self.metrics, "transactions", txs.upsert(&mut *db_tx, &tx_record)).await?;

            for (idx, vin) in tx.vin.iter().enumerate() {
                if let (Some(prev_txid), Some(prev_vout)) = (vin.txid.as_ref(), vin.vout) {
                    let input = TxInputRecord {
                        txid: tx.txid.clone(),
                        vin: idx as i32,
                        prev_txid: prev_txid.clone(),
                        prev_vout,
                        sequence: vin.sequence,
                    };
                    observe_db_write(&self.metrics, "tx_inputs", inputs.insert(&mut *db_tx, &input)).await?;

                    if let Some((address, value_sats)) =
                        address_lookup
                            .output_address_value(&mut *db_tx, prev_txid, prev_vout)
                            .await?
                    {
                        let spent = observe_db_write(
                            &self.metrics,
                            "utxos_current",
                            utxos.mark_spent_if_unspent(&mut *db_tx, prev_txid, prev_vout, &tx.txid),
                        )
                        .await?;
                        if spent {
                            *address_deltas.entry(address.clone()).or_insert(0) -= value_sats;
                            touched_addresses.insert(address);
                        }
                    }
                }
            }

            for vout in &tx.vout {
                let address = vout
                    .script_pub_key
                    .address
                    .clone()
                    .or_else(|| vout.script_pub_key.addresses.as_ref().and_then(|list| list.first().cloned()));

                let output = TxOutputRecord {
                    txid: tx.txid.clone(),
                    vout: vout.n,
                    value_sats: btc_to_sats(vout.value),
                    script_type: vout.script_pub_key.script_type.clone(),
                    address,
                    script_hex: vout.script_pub_key.hex.clone(),
                };
                observe_db_write(&self.metrics, "tx_outputs", outputs.insert(&mut *db_tx, &output)).await?;

                if let Some(output_address) = output.address.as_ref() {
                    let created = observe_db_write(
                        &self.metrics,
                        "utxos_current",
                        utxos.insert_unspent_if_absent(&mut *db_tx, &UtxoCreateRecord {
                            out_txid: output.txid.clone(),
                            out_vout: output.vout,
                            address: output_address.clone(),
                            value_sats: output.value_sats,
                            created_in_txid: output.txid.clone(),
                        }),
                    )
                    .await?;
                    if created {
                        *address_deltas.entry(output_address.clone()).or_insert(0) += output.value_sats;
                        touched_addresses.insert(output_address.clone());
                    }
                }
            }
        }

        for (address, delta) in address_deltas {
            if delta != 0 {
                observe_db_write(
                    &self.metrics,
                    "address_balance_current",
                    address_balances.add_delta(&mut *db_tx, &address, delta),
                )
                .await?;
            }
        }

        for address in touched_addresses {
            if let Some(balance_sats) = address_balances
                .current_balance(&mut *db_tx, &address)
                .await?
            {
                observe_db_write(
                    &self.metrics,
                    "address_balance_history",
                    address_balances.upsert_history_snapshot(
                        &mut *db_tx,
                        &address,
                        block.height,
                        block.time,
                        balance_sats,
                    )
                )
                .await?;
            }
        }

        db_tx.commit().await?;
        Ok(PersistBlockOutcome::Indexed)
    }
}

#[derive(Debug, Error)]
pub enum IndexerError {
    #[error("rpc error: {0}")]
    Rpc(#[from] crate::modules::rpc::RpcError),
    #[error("storage error: {0}")]
    Storage(#[from] sqlx::Error),
}

#[derive(Clone)]
pub struct IndexerService {
    rpc: crate::modules::rpc::RpcClient,
    pool: PgPool,
    metrics: MetricsService,
}

impl IndexerService {
    pub fn new(rpc: crate::modules::rpc::RpcClient, pool: PgPool, metrics: MetricsService) -> Self {
        Self { rpc, pool, metrics }
    }

    pub async fn has_canonical_block(&self, height: i32) -> Result<bool, IndexerError> {
        Ok(canonical_block_hash_at_height(&self.pool, height).await?.is_some())
    }

    pub async fn index_height(&self, height: u32) -> Result<IndexHeightResult, IndexerError> {
        let hash = self.rpc.get_block_hash(height).await?;
        let block = self.rpc.get_block_verbose2(&hash).await?;
        let tx_count = block.tx.len() as u64;

        let pipeline = IndexerPipeline::new(&self.pool, self.metrics.clone());
        let outcome = pipeline.persist_block(&block).await?;
        Ok(IndexHeightResult { outcome, tx_count })
    }

    pub async fn reconcile_chain(&self, reorg_depth: u32) -> Result<Option<i32>, IndexerError> {
        let Some(db_tip) = canonical_tip_height(&self.pool).await? else {
            return Ok(None);
        };

        let node_tip = i32::try_from(self.rpc.get_block_count().await?)
            .map_err(|_| sqlx::Error::Protocol("node tip exceeds i32 range".into()))?;
        let compare_tip = std::cmp::min(db_tip, node_tip);
        let compare_depth = i32::try_from(reorg_depth).unwrap_or(i32::MAX).max(1);
        let start_height = std::cmp::max(0, compare_tip.saturating_sub(compare_depth).saturating_add(1));

        for height in start_height..=compare_tip {
            let Some(db_hash) = canonical_block_hash_at_height(&self.pool, height).await? else {
                continue;
            };
            let node_hash = self.rpc.get_block_hash(height as u32).await?;

            if db_hash != node_hash {
                self.metrics.increment_error("reorg");
                self.apply_reorg(height).await?;
                return Ok(Some(height));
            }
        }

        Ok(None)
    }

    async fn apply_reorg(&self, divergence_height: i32) -> Result<(), IndexerError> {
        let mut db_tx = self.pool.begin().await?;
        acquire_chain_state_lock(&mut *db_tx).await?;

        sqlx::query(
            "UPDATE blocks \
             SET status = 'orphaned' \
             WHERE height >= $1 AND status = 'canonical'",
        )
        .bind(divergence_height)
        .execute(&mut *db_tx)
        .await?;

        sqlx::query(
            "UPDATE transactions \
             SET status = 'orphaned' \
             WHERE block_height >= $1 AND status = 'confirmed'",
        )
        .bind(divergence_height)
        .execute(&mut *db_tx)
        .await?;

        sqlx::query("DELETE FROM utxos_current")
            .execute(&mut *db_tx)
            .await?;
        sqlx::query("DELETE FROM address_balance_current")
            .execute(&mut *db_tx)
            .await?;
        sqlx::query("DELETE FROM address_balance_history")
            .execute(&mut *db_tx)
            .await?;

        let canonical_blocks: Vec<CanonicalBlockRow> = sqlx::query_as(
            "SELECT height, hash, time \
             FROM blocks \
             WHERE status = 'canonical' \
             ORDER BY height ASC",
        )
        .fetch_all(&mut *db_tx)
        .await?;

        for block in canonical_blocks {
            let txs: Vec<CanonicalTxRow> = sqlx::query_as(
                "SELECT txid, position_in_block \
                 FROM transactions \
                 WHERE block_height = $1 AND status = 'confirmed' \
                 ORDER BY position_in_block ASC, txid ASC",
            )
            .bind(block.height)
            .fetch_all(&mut *db_tx)
            .await?;

            replay_canonical_block(&mut *db_tx, &block, &txs).await?;
        }

        db_tx.commit().await?;
        Ok(())
    }
}

pub struct IndexHeightResult {
    pub outcome: PersistBlockOutcome,
    pub tx_count: u64,
}

#[derive(Debug, FromRow)]
struct CanonicalBlockRow {
    height: i32,
    hash: String,
    time: i64,
}

#[derive(Debug, FromRow)]
struct CanonicalTxRow {
    txid: String,
    position_in_block: i32,
}

#[derive(Debug, FromRow)]
struct ReplayInputRow {
    prev_txid: String,
    prev_vout: i32,
}

#[derive(Debug, FromRow)]
struct ReplayOutputRow {
    txid: String,
    vout: i32,
    address: Option<String>,
    value_sats: i64,
}

async fn replay_canonical_block(
    executor: &mut PgConnection,
    block: &CanonicalBlockRow,
    txs: &[CanonicalTxRow],
) -> Result<(), sqlx::Error> {
    let mut address_deltas: HashMap<String, i64> = HashMap::new();
    let mut touched_addresses: HashSet<String> = HashSet::new();

    for tx in txs {
        let inputs: Vec<ReplayInputRow> = sqlx::query_as(
            "SELECT prev_txid, prev_vout \
             FROM tx_inputs \
             WHERE txid = $1 \
             ORDER BY vin ASC",
        )
        .bind(&tx.txid)
        .fetch_all(&mut *executor)
        .await?;

        for input in inputs {
            let spent_output = sqlx::query(
                "SELECT address, value_sats \
                 FROM tx_outputs \
                 WHERE txid = $1 AND vout = $2 AND address IS NOT NULL",
            )
            .bind(&input.prev_txid)
            .bind(input.prev_vout)
            .fetch_optional(&mut *executor)
            .await?;

            if let Some(row) = spent_output {
                let address = row.get::<String, _>("address");
                let value_sats = row.get::<i64, _>("value_sats");
                let spent = sqlx::query(
                    "UPDATE utxos_current \
                     SET spent_in_txid = $3, status = 'spent' \
                     WHERE out_txid = $1 AND out_vout = $2 AND status = 'unspent'",
                )
                .bind(&input.prev_txid)
                .bind(input.prev_vout)
                .bind(&tx.txid)
                .execute(&mut *executor)
                .await?
                .rows_affected()
                    == 1;
                if spent {
                    *address_deltas.entry(address.clone()).or_insert(0) -= value_sats;
                    touched_addresses.insert(address);
                }
            }
        }

        let outputs: Vec<ReplayOutputRow> = sqlx::query_as(
            "SELECT txid, vout, address, value_sats \
             FROM tx_outputs \
             WHERE txid = $1 \
             ORDER BY vout ASC",
        )
        .bind(&tx.txid)
        .fetch_all(&mut *executor)
        .await?;

        for output in outputs {
            if let Some(output_address) = output.address.as_ref() {
                let created = sqlx::query(
                    "INSERT INTO utxos_current \
                     (out_txid, out_vout, address, value_sats, created_in_txid, spent_in_txid, status) \
                     VALUES ($1, $2, $3, $4, $5, NULL, 'unspent') \
                     ON CONFLICT (out_txid, out_vout) DO NOTHING",
                )
                .bind(&output.txid)
                .bind(output.vout)
                .bind(output_address)
                .bind(output.value_sats)
                .bind(&output.txid)
                .execute(&mut *executor)
                .await?
                .rows_affected()
                    == 1;
                if created {
                    *address_deltas.entry(output_address.clone()).or_insert(0) += output.value_sats;
                    touched_addresses.insert(output_address.clone());
                }
            }
        }
    }

    for (address, delta) in address_deltas {
        if delta != 0 {
            sqlx::query(
                "INSERT INTO address_balance_current (address, balance_sats, updated_at) \
                 VALUES ($1, $2, NOW()) \
                 ON CONFLICT (address) DO UPDATE SET \
                   balance_sats = address_balance_current.balance_sats + EXCLUDED.balance_sats, \
                   updated_at = NOW()",
            )
            .bind(&address)
            .bind(delta)
            .execute(&mut *executor)
            .await?;
        }
    }

    for address in touched_addresses {
        let balance_row = sqlx::query(
            "SELECT balance_sats \
             FROM address_balance_current \
             WHERE address = $1",
        )
        .bind(&address)
        .fetch_optional(&mut *executor)
        .await?;

        if let Some(balance_row) = balance_row {
            sqlx::query(
                "INSERT INTO address_balance_history (address, block_height, time, balance_sats) \
                 VALUES ($1, $2, $3, $4) \
                 ON CONFLICT (address, block_height) DO UPDATE SET \
                   time = EXCLUDED.time, \
                   balance_sats = EXCLUDED.balance_sats",
            )
            .bind(&address)
            .bind(block.height)
            .bind(block.time)
            .bind(balance_row.get::<i64, _>("balance_sats"))
            .execute(&mut *executor)
            .await?;
        }
    }

    Ok(())
}

async fn canonical_tip_height(pool: &PgPool) -> Result<Option<i32>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT MAX(height) \
         FROM blocks \
         WHERE status = 'canonical'",
    )
    .fetch_one(pool)
    .await
}

async fn acquire_chain_state_lock<'e, E>(executor: E) -> Result<(), sqlx::Error>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(CHAIN_STATE_LOCK_KEY)
        .execute(executor)
        .await?;

    Ok(())
}

async fn acquire_height_lock<'e, E>(executor: E, height: i32) -> Result<(), sqlx::Error>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(i64::from(height))
        .execute(executor)
        .await?;

    Ok(())
}

async fn canonical_block_hash_at_height<'e, E>(
    executor: E,
    height: i32,
) -> Result<Option<String>, sqlx::Error>
where
    E: Executor<'e, Database = Postgres>,
{
    let row = sqlx::query(
        "SELECT hash \
         FROM blocks \
         WHERE height = $1 AND status = 'canonical' \
         LIMIT 1",
    )
    .bind(height)
    .fetch_optional(executor)
    .await?;

    Ok(row.map(|row| row.get::<String, _>("hash")))
}

fn btc_to_sats(value: f64) -> i64 {
    (value * 100_000_000.0).round() as i64
}

async fn observe_db_write<F, T>(
    metrics: &MetricsService,
    table: &str,
    future: F,
) -> Result<T, sqlx::Error>
where
    F: Future<Output = Result<T, sqlx::Error>>,
{
    let started = Instant::now();
    let result = future.await;
    metrics.observe_db_write_duration(table, started.elapsed().as_secs_f64());
    if result.is_err() {
        metrics.increment_error("db_write");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{btc_to_sats, PersistBlockOutcome, RpcBlock};

    #[test]
    fn converts_btc_to_sats() {
        assert_eq!(btc_to_sats(0.0), 0);
        assert_eq!(btc_to_sats(1.0), 100_000_000);
        assert_eq!(btc_to_sats(0.00000001), 1);
    }

    #[test]
    fn parses_block_json() {
        let json = r#"
        {
          "hash": "blockhash",
          "height": 1,
          "previousblockhash": "prevhash",
          "time": 1700000000,
          "tx": [
            {
              "txid": "tx1",
              "vin": [{"txid": "prevtx", "vout": 0, "sequence": 1}],
              "vout": [
                {"n": 0, "value": 0.5, "scriptPubKey": {"type": "pubkeyhash", "hex": "00", "address": "addr1"}}
              ]
            }
          ]
        }
        "#;

        let block: RpcBlock = serde_json::from_str(json).expect("parse block");
        assert_eq!(block.height, 1);
        assert_eq!(block.tx.len(), 1);
    }

    #[test]
    fn persist_block_outcome_is_comparable() {
        assert_eq!(PersistBlockOutcome::Indexed, PersistBlockOutcome::Indexed);
        assert_ne!(
            PersistBlockOutcome::Indexed,
            PersistBlockOutcome::WaitingForPreviousHeight
        );
    }
}
