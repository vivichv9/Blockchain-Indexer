use serde::Deserialize;
use serde_json::Value;
use sqlx::PgPool;
use thiserror::Error;

use crate::modules::storage::repo::{
    BlockRecord, BlocksRepo, TransactionRecord, TransactionsRepo, TxInputRecord, TxInputsRepo,
    TxOutputRecord, TxOutputsRepo,
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
}

impl<'a> IndexerPipeline<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn persist_block(&self, block: &RpcBlock) -> Result<(), sqlx::Error> {
        let blocks = BlocksRepo::new(self.pool);
        let txs = TransactionsRepo::new(self.pool);
        let inputs = TxInputsRepo::new(self.pool);
        let outputs = TxOutputsRepo::new(self.pool);

        let block_record = BlockRecord {
            height: block.height,
            hash: block.hash.clone(),
            prev_hash: block.prev_hash.clone().unwrap_or_default(),
            time: block.time,
            status: "canonical".to_string(),
            meta: serde_json::json!({}),
        };
        blocks.upsert(&block_record).await?;

        for tx in &block.tx {
            let tx_record = TransactionRecord {
                txid: tx.txid.clone(),
                block_height: Some(block.height),
                block_hash: Some(block.hash.clone()),
                time: block.time,
                status: "confirmed".to_string(),
                decoded: serde_json::to_value(tx).unwrap_or(Value::Null),
            };
            txs.upsert(&tx_record).await?;

            for (idx, vin) in tx.vin.iter().enumerate() {
                if let (Some(prev_txid), Some(prev_vout)) = (vin.txid.as_ref(), vin.vout) {
                    let input = TxInputRecord {
                        txid: tx.txid.clone(),
                        vin: idx as i32,
                        prev_txid: prev_txid.clone(),
                        prev_vout,
                        sequence: vin.sequence,
                    };
                    inputs.insert(&input).await?;
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
                outputs.insert(&output).await?;
            }
        }

        Ok(())
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
}

impl IndexerService {
    pub fn new(rpc: crate::modules::rpc::RpcClient, pool: PgPool) -> Self {
        Self { rpc, pool }
    }

    pub async fn index_height(&self, height: u32) -> Result<(), IndexerError> {
        let hash = self.rpc.get_block_hash(height).await?;
        let block = self.rpc.get_block_verbose2(&hash).await?;

        let pipeline = IndexerPipeline::new(&self.pool);
        pipeline.persist_block(&block).await?;
        Ok(())
    }
}

fn btc_to_sats(value: f64) -> i64 {
    (value * 100_000_000.0).round() as i64
}

#[cfg(test)]
mod tests {
    use super::{btc_to_sats, RpcBlock};

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
}
