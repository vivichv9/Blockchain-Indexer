use serde_json::Value;
use sqlx::PgPool;

#[derive(Debug, Clone)]
pub struct BlockRecord {
    pub height: i32,
    pub hash: String,
    pub prev_hash: String,
    pub time: i64,
    pub status: String,
    pub meta: Value,
}

#[derive(Debug, Clone)]
pub struct TransactionRecord {
    pub txid: String,
    pub block_height: Option<i32>,
    pub block_hash: Option<String>,
    pub time: i64,
    pub status: String,
    pub decoded: Value,
}

#[derive(Debug, Clone)]
pub struct TxOutputRecord {
    pub txid: String,
    pub vout: i32,
    pub value_sats: i64,
    pub script_type: String,
    pub address: Option<String>,
    pub script_hex: String,
}

#[derive(Debug, Clone)]
pub struct TxInputRecord {
    pub txid: String,
    pub vin: i32,
    pub prev_txid: String,
    pub prev_vout: i32,
    pub sequence: i64,
}

pub struct BlocksRepo<'a> {
    pool: &'a PgPool,
}

impl<'a> BlocksRepo<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn upsert(&self, block: &BlockRecord) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO blocks (height, hash, prev_hash, time, status, meta)\
             VALUES ($1, $2, $3, $4, $5, $6)\
             ON CONFLICT (hash) DO UPDATE SET\
               height = EXCLUDED.height,\
               prev_hash = EXCLUDED.prev_hash,\
               time = EXCLUDED.time,\
               status = EXCLUDED.status,\
               meta = EXCLUDED.meta",
        )
        .bind(block.height)
        .bind(&block.hash)
        .bind(&block.prev_hash)
        .bind(block.time)
        .bind(&block.status)
        .bind(&block.meta)
        .execute(self.pool)
        .await?;

        Ok(())
    }
}

pub struct TransactionsRepo<'a> {
    pool: &'a PgPool,
}

impl<'a> TransactionsRepo<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn upsert(&self, tx: &TransactionRecord) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO transactions (txid, block_height, block_hash, time, status, decoded)\
             VALUES ($1, $2, $3, $4, $5, $6)\
             ON CONFLICT (txid) DO UPDATE SET\
               block_height = EXCLUDED.block_height,\
               block_hash = EXCLUDED.block_hash,\
               time = EXCLUDED.time,\
               status = EXCLUDED.status,\
               decoded = EXCLUDED.decoded",
        )
        .bind(&tx.txid)
        .bind(tx.block_height)
        .bind(&tx.block_hash)
        .bind(tx.time)
        .bind(&tx.status)
        .bind(&tx.decoded)
        .execute(self.pool)
        .await?;

        Ok(())
    }
}

pub struct TxOutputsRepo<'a> {
    pool: &'a PgPool,
}

impl<'a> TxOutputsRepo<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, output: &TxOutputRecord) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO tx_outputs (txid, vout, value_sats, script_type, address, script_hex)\
             VALUES ($1, $2, $3, $4, $5, $6)\
             ON CONFLICT (txid, vout) DO NOTHING",
        )
        .bind(&output.txid)
        .bind(output.vout)
        .bind(output.value_sats)
        .bind(&output.script_type)
        .bind(&output.address)
        .bind(&output.script_hex)
        .execute(self.pool)
        .await?;

        Ok(())
    }
}

pub struct TxInputsRepo<'a> {
    pool: &'a PgPool,
}

impl<'a> TxInputsRepo<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, input: &TxInputRecord) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO tx_inputs (txid, vin, prev_txid, prev_vout, sequence)\
             VALUES ($1, $2, $3, $4, $5)\
             ON CONFLICT (txid, vin) DO NOTHING",
        )
        .bind(&input.txid)
        .bind(input.vin)
        .bind(&input.prev_txid)
        .bind(input.prev_vout)
        .bind(input.sequence)
        .execute(self.pool)
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{BlockRecord, TransactionRecord};

    #[test]
    fn block_record_is_sendable() {
        let block = BlockRecord {
            height: 1,
            hash: "h".to_string(),
            prev_hash: "p".to_string(),
            time: 0,
            status: "canonical".to_string(),
            meta: serde_json::json!({}),
        };

        let _ = block.clone();
    }

    #[test]
    fn tx_record_is_sendable() {
        let tx = TransactionRecord {
            txid: "t".to_string(),
            block_height: Some(1),
            block_hash: Some("h".to_string()),
            time: 0,
            status: "confirmed".to_string(),
            decoded: serde_json::json!({}),
        };

        let _ = tx.clone();
    }
}
