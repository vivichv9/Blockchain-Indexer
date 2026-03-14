use std::collections::HashSet;
use std::time::Duration;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgPool, Row};
use thiserror::Error;
use tracing::warn;

use crate::modules::indexer::RpcTransaction;
use crate::modules::rpc::{RpcClient, RpcError};
use crate::modules::storage::repo::{
    TransactionRecord, TransactionsRepo, TxInputRecord, TxInputsRepo, TxOutputRecord, TxOutputsRepo,
};

#[derive(Debug, Error)]
pub enum MempoolError {
    #[error(transparent)]
    Rpc(#[from] RpcError),
    #[error(transparent)]
    Storage(#[from] sqlx::Error),
}

#[derive(Debug, Clone)]
pub struct MempoolRunnerConfig {
    pub poll_interval: Duration,
}

#[derive(Clone)]
pub struct MempoolRunner {
    rpc: RpcClient,
    pool: PgPool,
    config: MempoolRunnerConfig,
}

impl MempoolRunner {
    pub fn new(rpc: RpcClient, pool: PgPool, config: MempoolRunnerConfig) -> Self {
        Self { rpc, pool, config }
    }

    pub fn start(&self) {
        let runner = self.clone();

        tokio::spawn(async move {
            loop {
                if let Err(err) = runner.sync_once().await {
                    warn!(component = "mempool", error = %err, message = "mempool sync failed");
                }

                tokio::time::sleep(runner.config.poll_interval).await;
            }
        });
    }

    pub async fn sync_once(&self) -> Result<(), MempoolError> {
        let current_txids = self.rpc.get_raw_mempool().await?;
        let current_set: HashSet<String> = current_txids.iter().cloned().collect();
        let known_set = self.list_known_mempool_txids().await?;

        let new_txids = diff_new_txids(&current_set, &known_set);
        let dropped_txids = diff_dropped_txids(&current_set, &known_set);

        for txid in new_txids {
            match self.rpc.get_raw_transaction_verbose(&txid).await {
                Ok(tx) => self.persist_mempool_transaction(&tx).await?,
                Err(err) => {
                    warn!(
                        component = "mempool",
                        txid = %txid,
                        error = %err,
                        message = "failed to fetch mempool transaction"
                    );
                }
            }
        }

        if !dropped_txids.is_empty() {
            self.mark_dropped(&dropped_txids).await?;
        }

        Ok(())
    }

    async fn list_known_mempool_txids(&self) -> Result<HashSet<String>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT txid \
             FROM transactions \
             WHERE status = 'mempool'",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| row.get::<String, _>("txid"))
            .collect())
    }

    async fn persist_mempool_transaction(&self, tx: &RpcTransaction) -> Result<(), MempoolError> {
        let mut db_tx = self.pool.begin().await?;

        let existing_status = sqlx::query_scalar::<_, String>(
            "SELECT status \
             FROM transactions \
             WHERE txid = $1",
        )
        .bind(&tx.txid)
        .fetch_optional(&mut *db_tx)
        .await?;

        if matches!(existing_status.as_deref(), Some("confirmed")) {
            db_tx.commit().await?;
            return Ok(());
        }

        let tx_repo = TransactionsRepo::new(&self.pool);
        let inputs_repo = TxInputsRepo::new(&self.pool);
        let outputs_repo = TxOutputsRepo::new(&self.pool);
        let now = Utc::now().timestamp();

        tx_repo
            .upsert(
                &mut *db_tx,
                &TransactionRecord {
                    txid: tx.txid.clone(),
                    block_height: None,
                    block_hash: None,
                    position_in_block: 0,
                    time: now,
                    status: "mempool".to_string(),
                    decoded: serde_json::to_value(tx).unwrap_or(Value::Null),
                },
            )
            .await?;

        for (idx, vin) in tx.vin.iter().enumerate() {
            if let (Some(prev_txid), Some(prev_vout)) = (vin.txid.as_ref(), vin.vout) {
                inputs_repo
                    .insert(
                        &mut *db_tx,
                        &TxInputRecord {
                            txid: tx.txid.clone(),
                            vin: idx as i32,
                            prev_txid: prev_txid.clone(),
                            prev_vout,
                            sequence: vin.sequence,
                        },
                    )
                    .await?;
            }
        }

        for vout in &tx.vout {
            let address = vout
                .script_pub_key
                .address
                .clone()
                .or_else(|| vout.script_pub_key.addresses.as_ref().and_then(|list| list.first().cloned()));

            outputs_repo
                .insert(
                    &mut *db_tx,
                    &TxOutputRecord {
                        txid: tx.txid.clone(),
                        vout: vout.n,
                        value_sats: btc_to_sats(vout.value),
                        script_type: vout.script_pub_key.script_type.clone(),
                        address,
                        script_hex: vout.script_pub_key.hex.clone(),
                    },
                )
                .await?;
        }

        db_tx.commit().await?;
        Ok(())
    }

    async fn mark_dropped(&self, dropped_txids: &[String]) -> Result<(), sqlx::Error> {
        for txid in dropped_txids {
            sqlx::query(
                "UPDATE transactions \
                 SET status = 'dropped' \
                 WHERE txid = $1 AND status = 'mempool'",
            )
            .bind(txid)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }
}

fn diff_new_txids(current: &HashSet<String>, known: &HashSet<String>) -> Vec<String> {
    let mut values: Vec<String> = current.difference(known).cloned().collect();
    values.sort();
    values
}

fn diff_dropped_txids(current: &HashSet<String>, known: &HashSet<String>) -> Vec<String> {
    let mut values: Vec<String> = known.difference(current).cloned().collect();
    values.sort();
    values
}

fn btc_to_sats(value: f64) -> i64 {
    (value * 100_000_000.0).round() as i64
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MempoolAddressMatch {
    pub txid: String,
    pub addresses: Vec<String>,
}

pub async fn list_mempool_txids_for_address(
    pool: &PgPool,
    address: &str,
) -> Result<Vec<MempoolAddressMatch>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT DISTINCT t.txid \
         FROM transactions t \
         LEFT JOIN tx_outputs o ON o.txid = t.txid AND o.address = $1 \
         LEFT JOIN tx_inputs i ON i.txid = t.txid \
         LEFT JOIN tx_outputs prev_o ON prev_o.txid = i.prev_txid AND prev_o.vout = i.prev_vout AND prev_o.address = $1 \
         WHERE t.status = 'mempool' AND (o.address IS NOT NULL OR prev_o.address IS NOT NULL) \
         ORDER BY t.txid",
    )
    .bind(address)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| MempoolAddressMatch {
            txid: row.get::<String, _>("txid"),
            addresses: vec![address.to_string()],
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{btc_to_sats, diff_dropped_txids, diff_new_txids};

    #[test]
    fn detects_new_txids() {
        let current = HashSet::from(["a".to_string(), "b".to_string()]);
        let known = HashSet::from(["b".to_string(), "c".to_string()]);

        assert_eq!(diff_new_txids(&current, &known), vec!["a".to_string()]);
    }

    #[test]
    fn detects_dropped_txids() {
        let current = HashSet::from(["a".to_string(), "b".to_string()]);
        let known = HashSet::from(["b".to_string(), "c".to_string()]);

        assert_eq!(diff_dropped_txids(&current, &known), vec!["c".to_string()]);
    }

    #[test]
    fn converts_btc_to_sats() {
        assert_eq!(btc_to_sats(0.00000001), 1);
        assert_eq!(btc_to_sats(1.5), 150_000_000);
    }
}
