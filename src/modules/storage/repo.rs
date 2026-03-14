use serde_json::Value;
use sqlx::{Executor, PgPool, Postgres, Row};

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
    pub position_in_block: i32,
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

pub struct BlocksRepo;

impl BlocksRepo {
    pub fn new(_pool: &PgPool) -> Self {
        Self
    }

    pub async fn upsert<'e, E>(&self, executor: E, block: &BlockRecord) -> Result<(), sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
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
        .execute(executor)
        .await?;

        Ok(())
    }
}

pub struct TransactionsRepo;

impl TransactionsRepo {
    pub fn new(_pool: &PgPool) -> Self {
        Self
    }

    pub async fn upsert<'e, E>(&self, executor: E, tx: &TransactionRecord) -> Result<(), sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        sqlx::query(
            "INSERT INTO transactions (txid, block_height, block_hash, position_in_block, time, status, decoded)\
             VALUES ($1, $2, $3, $4, $5, $6, $7)\
             ON CONFLICT (txid) DO UPDATE SET\
               block_height = EXCLUDED.block_height,\
               block_hash = EXCLUDED.block_hash,\
               position_in_block = EXCLUDED.position_in_block,\
               time = EXCLUDED.time,\
               status = EXCLUDED.status,\
               decoded = EXCLUDED.decoded",
        )
        .bind(&tx.txid)
        .bind(tx.block_height)
        .bind(&tx.block_hash)
        .bind(tx.position_in_block)
        .bind(tx.time)
        .bind(&tx.status)
        .bind(&tx.decoded)
        .execute(executor)
        .await?;

        Ok(())
    }
}

pub struct TxOutputsRepo;

impl TxOutputsRepo {
    pub fn new(_pool: &PgPool) -> Self {
        Self
    }

    pub async fn insert<'e, E>(&self, executor: E, output: &TxOutputRecord) -> Result<(), sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
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
        .execute(executor)
        .await?;

        Ok(())
    }
}

pub struct TxInputsRepo;

impl TxInputsRepo {
    pub fn new(_pool: &PgPool) -> Self {
        Self
    }

    pub async fn insert<'e, E>(&self, executor: E, input: &TxInputRecord) -> Result<(), sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
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
        .execute(executor)
        .await?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct UtxoCreateRecord {
    pub out_txid: String,
    pub out_vout: i32,
    pub address: String,
    pub value_sats: i64,
    pub created_in_txid: String,
}

pub struct UtxosRepo;

impl UtxosRepo {
    pub fn new(_pool: &PgPool) -> Self {
        Self
    }

    pub async fn insert_unspent_if_absent<'e, E>(
        &self,
        executor: E,
        utxo: &UtxoCreateRecord,
    ) -> Result<bool, sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let result = sqlx::query(
            "INSERT INTO utxos_current \
             (out_txid, out_vout, address, value_sats, created_in_txid, spent_in_txid, status) \
             VALUES ($1, $2, $3, $4, $5, NULL, 'unspent') \
             ON CONFLICT (out_txid, out_vout) DO NOTHING",
        )
        .bind(&utxo.out_txid)
        .bind(utxo.out_vout)
        .bind(&utxo.address)
        .bind(utxo.value_sats)
        .bind(&utxo.created_in_txid)
        .execute(executor)
        .await?;

        Ok(result.rows_affected() == 1)
    }

    pub async fn mark_spent_if_unspent(
        &self,
        executor: impl Executor<'_, Database = Postgres>,
        out_txid: &str,
        out_vout: i32,
        spent_in_txid: &str,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE utxos_current \
             SET spent_in_txid = $3, status = 'spent' \
             WHERE out_txid = $1 AND out_vout = $2 AND status = 'unspent'",
        )
        .bind(out_txid)
        .bind(out_vout)
        .bind(spent_in_txid)
        .execute(executor)
        .await?;

        Ok(result.rows_affected() == 1)
    }
}

pub struct AddressBalancesRepo;

impl AddressBalancesRepo {
    pub fn new(_pool: &PgPool) -> Self {
        Self
    }

    pub async fn add_delta<'e, E>(
        &self,
        executor: E,
        address: &str,
        delta_sats: i64,
    ) -> Result<(), sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        sqlx::query(
            "INSERT INTO address_balance_current (address, balance_sats, updated_at) \
             VALUES ($1, $2, NOW()) \
             ON CONFLICT (address) DO UPDATE SET \
               balance_sats = address_balance_current.balance_sats + EXCLUDED.balance_sats, \
               updated_at = NOW()",
        )
        .bind(address)
        .bind(delta_sats)
        .execute(executor)
        .await?;

        Ok(())
    }

    pub async fn current_balance<'e, E>(&self, executor: E, address: &str) -> Result<Option<i64>, sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let row = sqlx::query("SELECT balance_sats FROM address_balance_current WHERE address = $1")
            .bind(address)
            .fetch_optional(executor)
            .await?;

        Ok(row.map(|r| r.get::<i64, _>("balance_sats")))
    }

    pub async fn upsert_history_snapshot(
        &self,
        executor: impl Executor<'_, Database = Postgres>,
        address: &str,
        block_height: i32,
        time: i64,
        balance_sats: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO address_balance_history (address, block_height, time, balance_sats) \
             VALUES ($1, $2, $3, $4) \
             ON CONFLICT (address, block_height) DO UPDATE SET \
               time = EXCLUDED.time, \
               balance_sats = EXCLUDED.balance_sats",
        )
        .bind(address)
        .bind(block_height)
        .bind(time)
        .bind(balance_sats)
        .execute(executor)
        .await?;

        Ok(())
    }
}

pub struct AddressLookupRepo;

impl AddressLookupRepo {
    pub fn new(_pool: &PgPool) -> Self {
        Self
    }

    pub async fn output_address_value(
        &self,
        executor: impl Executor<'_, Database = Postgres>,
        txid: &str,
        vout: i32,
    ) -> Result<Option<(String, i64)>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT address, value_sats \
             FROM tx_outputs \
             WHERE txid = $1 AND vout = $2 AND address IS NOT NULL",
        )
        .bind(txid)
        .bind(vout)
        .fetch_optional(executor)
        .await?;

        Ok(row.map(|r| (r.get::<String, _>("address"), r.get::<i64, _>("value_sats"))))
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
            position_in_block: 0,
            time: 0,
            status: "confirmed".to_string(),
            decoded: serde_json::json!({}),
        };

        let _ = tx.clone();
    }
}
