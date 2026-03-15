use std::collections::HashMap;

use serde::Serialize;
use sqlx::{PgPool, Postgres, QueryBuilder, Row};
use thiserror::Error;
use utoipa::ToSchema;

#[derive(Debug, Error)]
pub enum DataError {
    #[error("address is not indexed")]
    AddressNotIndexed,
    #[error("validation error: {0}")]
    Validation(String),
    #[error("storage error: {0}")]
    Storage(#[from] sqlx::Error),
}

#[derive(Debug, Clone)]
pub struct DataService {
    pool: PgPool,
}

#[derive(Debug, Clone, Copy, ToSchema)]
pub struct Pagination {
    pub offset: i64,
    pub limit: i64,
}

#[derive(Debug, Clone, Default)]
pub struct BalanceFilter {
    pub from_time: Option<i64>,
    pub to_time: Option<i64>,
    pub from_height: Option<i32>,
    pub to_height: Option<i32>,
}

#[derive(Debug, Clone, Default)]
pub struct TransactionsFilter {
    pub from_height: Option<i32>,
    pub to_height: Option<i32>,
    pub from_time: Option<i64>,
    pub to_time: Option<i64>,
    pub address: Option<String>,
    pub txid: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct BlocksFilter {
    pub from_height: Option<i32>,
    pub to_height: Option<i32>,
    pub from_time: Option<i64>,
    pub to_time: Option<i64>,
    pub block_hash: Option<String>,
    pub has_txid: Option<String>,
    pub address: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BalanceResponse {
    pub address: String,
    pub balance_sats: i64,
    pub as_of: BalanceAsOf,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BalanceAsOf {
    pub block_height: Option<i32>,
    pub time: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BalanceHistoryItem {
    pub block_height: i32,
    pub time: i64,
    pub balance_sats: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BalanceHistoryPage {
    pub address: String,
    pub items: Vec<BalanceHistoryItem>,
    pub offset: i64,
    pub limit: i64,
    pub total: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UtxoItem {
    pub out_txid: String,
    pub out_vout: i32,
    pub value_sats: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UtxosResponse {
    pub address: String,
    pub items: Vec<UtxoItem>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TransactionIo {
    pub txid: Option<String>,
    pub vout: Option<i32>,
    pub address: Option<String>,
    pub value_sats: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TransactionItem {
    pub txid: String,
    pub status: String,
    pub block_height: Option<i32>,
    pub block_hash: Option<String>,
    pub time: i64,
    pub inputs: Vec<TransactionIo>,
    pub outputs: Vec<TransactionIo>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TransactionsPage {
    pub items: Vec<TransactionItem>,
    pub offset: i64,
    pub limit: i64,
    pub total: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BlockItem {
    pub height: i32,
    pub hash: String,
    pub prev_hash: String,
    pub time: i64,
    pub status: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BlocksPage {
    pub items: Vec<BlockItem>,
    pub offset: i64,
    pub limit: i64,
    pub total: i64,
}

impl DataService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn ensure_address_indexed(&self, address: &str) -> Result<(), DataError> {
        let indexed = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(
                SELECT 1
                FROM jobs
                WHERE mode = 'all_addresses'
                  AND COALESCE((config_snapshot ->> 'enabled')::boolean, false) = true
            ) OR EXISTS(
                SELECT 1
                FROM job_addresses ja
                JOIN jobs j ON j.job_id = ja.job_id
                WHERE ja.address = $1
                  AND COALESCE((j.config_snapshot ->> 'enabled')::boolean, false) = true
            )",
        )
        .bind(address)
        .fetch_one(&self.pool)
        .await?;

        if indexed {
            Ok(())
        } else {
            Err(DataError::AddressNotIndexed)
        }
    }

    pub fn validate_pagination(offset: Option<i64>, limit: Option<i64>) -> Result<Pagination, DataError> {
        let offset = offset.unwrap_or(0);
        let limit = limit.unwrap_or(100);

        if offset < 0 {
            return Err(DataError::Validation("offset MUST be >= 0".to_string()));
        }

        if !(1..=1000).contains(&limit) {
            return Err(DataError::Validation("limit MUST be between 1 and 1000".to_string()));
        }

        Ok(Pagination { offset, limit })
    }

    pub async fn get_balance(&self, address: &str, filter: BalanceFilter) -> Result<BalanceResponse, DataError> {
        self.ensure_address_indexed(address).await?;

        let current_query = filter.from_time.is_none()
            && filter.to_time.is_none()
            && filter.from_height.is_none()
            && filter.to_height.is_none();

        if current_query {
            let balance_sats = sqlx::query_scalar::<_, i64>(
                "SELECT COALESCE(
                    (SELECT balance_sats FROM address_balance_current WHERE address = $1),
                    0
                )",
            )
            .bind(address)
            .fetch_one(&self.pool)
            .await?;

            let tip = sqlx::query(
                "SELECT height, time
                 FROM blocks
                 WHERE status = 'canonical'
                 ORDER BY height DESC
                 LIMIT 1",
            )
            .fetch_optional(&self.pool)
            .await?;

            return Ok(BalanceResponse {
                address: address.to_string(),
                balance_sats,
                as_of: BalanceAsOf {
                    block_height: tip.as_ref().map(|row| row.get::<i32, _>("height")),
                    time: tip.as_ref().map(|row| row.get::<i64, _>("time")),
                },
            });
        }

        let mut tip_query: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT height, time
             FROM blocks
             WHERE status = 'canonical'",
        );
        apply_block_bounds(
            &mut tip_query,
            "blocks",
            filter.from_height,
            filter.to_height,
            filter.from_time,
            filter.to_time,
        );
        tip_query.push(" ORDER BY height DESC LIMIT 1");

        let tip = tip_query.build().fetch_optional(&self.pool).await?;
        let Some(tip) = tip else {
            return Ok(BalanceResponse {
                address: address.to_string(),
                balance_sats: 0,
                as_of: BalanceAsOf {
                    block_height: None,
                    time: None,
                },
            });
        };

        let tip_height = tip.get::<i32, _>("height");
        let tip_time = tip.get::<i64, _>("time");

        let balance_row = sqlx::query(
            "SELECT balance_sats
             FROM address_balance_history
             WHERE address = $1 AND block_height <= $2
             ORDER BY block_height DESC
             LIMIT 1",
        )
        .bind(address)
        .bind(tip_height)
        .fetch_optional(&self.pool)
        .await?;

        Ok(BalanceResponse {
            address: address.to_string(),
            balance_sats: balance_row
                .map(|row| row.get::<i64, _>("balance_sats"))
                .unwrap_or(0),
            as_of: BalanceAsOf {
                block_height: Some(tip_height),
                time: Some(tip_time),
            },
        })
    }

    pub async fn get_utxos(&self, address: &str) -> Result<UtxosResponse, DataError> {
        self.ensure_address_indexed(address).await?;

        let rows = sqlx::query(
            "SELECT out_txid, out_vout, value_sats
             FROM utxos_current
             WHERE address = $1 AND status = 'unspent'
             ORDER BY out_txid, out_vout",
        )
        .bind(address)
        .fetch_all(&self.pool)
        .await?;

        Ok(UtxosResponse {
            address: address.to_string(),
            items: rows
                .into_iter()
                .map(|row| UtxoItem {
                    out_txid: row.get::<String, _>("out_txid"),
                    out_vout: row.get::<i32, _>("out_vout"),
                    value_sats: row.get::<i64, _>("value_sats"),
                })
                .collect(),
        })
    }

    pub async fn get_balance_history(
        &self,
        address: &str,
        filter: BalanceFilter,
        pagination: Pagination,
    ) -> Result<BalanceHistoryPage, DataError> {
        self.ensure_address_indexed(address).await?;

        let mut count_builder = QueryBuilder::<Postgres>::new(
            "SELECT COUNT(*) AS total
             FROM address_balance_history abh
             WHERE abh.address = ",
        );
        count_builder.push_bind(address);
        append_balance_history_filters(
            &mut count_builder,
            filter.from_height,
            filter.to_height,
            filter.from_time,
            filter.to_time,
        );
        let total = count_builder
            .build()
            .fetch_one(&self.pool)
            .await?
            .get::<i64, _>("total");

        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT abh.block_height, abh.time, abh.balance_sats
             FROM address_balance_history abh
             WHERE abh.address = ",
        );
        builder.push_bind(address);
        append_balance_history_filters(
            &mut builder,
            filter.from_height,
            filter.to_height,
            filter.from_time,
            filter.to_time,
        );
        builder.push(" ORDER BY abh.block_height DESC, abh.time DESC");
        builder.push(" OFFSET ");
        builder.push_bind(pagination.offset);
        builder.push(" LIMIT ");
        builder.push_bind(pagination.limit);

        let rows = builder.build().fetch_all(&self.pool).await?;
        let items = rows
            .into_iter()
            .map(|row| BalanceHistoryItem {
                block_height: row.get::<i32, _>("block_height"),
                time: row.get::<i64, _>("time"),
                balance_sats: row.get::<i64, _>("balance_sats"),
            })
            .collect();

        Ok(BalanceHistoryPage {
            address: address.to_string(),
            items,
            offset: pagination.offset,
            limit: pagination.limit,
            total,
        })
    }

    pub async fn list_mempool_transactions(
        &self,
        address: Option<&str>,
        pagination: Pagination,
    ) -> Result<TransactionsPage, DataError> {
        if let Some(address) = address {
            self.ensure_address_indexed(address).await?;
        }

        self.list_transactions_by_status("mempool", address, None, pagination).await
    }

    pub async fn list_transactions(
        &self,
        filter: TransactionsFilter,
        pagination: Pagination,
    ) -> Result<TransactionsPage, DataError> {
        if let Some(address) = filter.address.as_deref() {
            self.ensure_address_indexed(address).await?;
        }

        let mut count_builder = QueryBuilder::<Postgres>::new(
            "SELECT COUNT(DISTINCT t.txid) AS total
             FROM transactions t",
        );
        append_transaction_joins(&mut count_builder, filter.address.as_deref());
        count_builder.push(" WHERE t.status = 'confirmed'");
        append_transaction_filters(
            &mut count_builder,
            filter.address.as_deref(),
            filter.txid.as_deref(),
            filter.from_height,
            filter.to_height,
            filter.from_time,
            filter.to_time,
        );
        let total = count_builder
            .build()
            .fetch_one(&self.pool)
            .await?
            .get::<i64, _>("total");

        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT DISTINCT t.txid, t.status, t.block_height, t.block_hash, t.time
             FROM transactions t",
        );
        append_transaction_joins(&mut builder, filter.address.as_deref());
        builder.push(" WHERE t.status = 'confirmed'");
        append_transaction_filters(
            &mut builder,
            filter.address.as_deref(),
            filter.txid.as_deref(),
            filter.from_height,
            filter.to_height,
            filter.from_time,
            filter.to_time,
        );
        builder.push(" ORDER BY t.block_height DESC NULLS LAST, t.position_in_block DESC, t.txid DESC");
        builder.push(" OFFSET ");
        builder.push_bind(pagination.offset);
        builder.push(" LIMIT ");
        builder.push_bind(pagination.limit);

        let rows = builder.build().fetch_all(&self.pool).await?;
        let items = self.load_transaction_items(rows).await?;

        Ok(TransactionsPage {
            items,
            offset: pagination.offset,
            limit: pagination.limit,
            total,
        })
    }

    pub async fn list_blocks(
        &self,
        filter: BlocksFilter,
        pagination: Pagination,
    ) -> Result<BlocksPage, DataError> {
        if let Some(address) = filter.address.as_deref() {
            self.ensure_address_indexed(address).await?;
        }

        let mut count_builder = QueryBuilder::<Postgres>::new(
            "SELECT COUNT(DISTINCT b.hash) AS total
             FROM blocks b",
        );
        append_block_joins(&mut count_builder, filter.has_txid.as_deref(), filter.address.as_deref());
        count_builder.push(" WHERE b.status = 'canonical'");
        append_block_filters(
            &mut count_builder,
            filter.from_height,
            filter.to_height,
            filter.from_time,
            filter.to_time,
            filter.block_hash.as_deref(),
            filter.has_txid.as_deref(),
            filter.address.as_deref(),
        );
        let total = count_builder
            .build()
            .fetch_one(&self.pool)
            .await?
            .get::<i64, _>("total");

        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT DISTINCT b.height, b.hash, b.prev_hash, b.time, b.status
             FROM blocks b",
        );
        append_block_joins(&mut builder, filter.has_txid.as_deref(), filter.address.as_deref());
        builder.push(" WHERE b.status = 'canonical'");
        append_block_filters(
            &mut builder,
            filter.from_height,
            filter.to_height,
            filter.from_time,
            filter.to_time,
            filter.block_hash.as_deref(),
            filter.has_txid.as_deref(),
            filter.address.as_deref(),
        );
        builder.push(" ORDER BY b.height DESC, b.hash DESC");
        builder.push(" OFFSET ");
        builder.push_bind(pagination.offset);
        builder.push(" LIMIT ");
        builder.push_bind(pagination.limit);

        let rows = builder.build().fetch_all(&self.pool).await?;
        let items = rows
            .into_iter()
            .map(|row| BlockItem {
                height: row.get::<i32, _>("height"),
                hash: row.get::<String, _>("hash"),
                prev_hash: row.get::<String, _>("prev_hash"),
                time: row.get::<i64, _>("time"),
                status: row.get::<String, _>("status"),
            })
            .collect();

        Ok(BlocksPage {
            items,
            offset: pagination.offset,
            limit: pagination.limit,
            total,
        })
    }

    async fn list_transactions_by_status(
        &self,
        status: &str,
        address: Option<&str>,
        txid: Option<&str>,
        pagination: Pagination,
    ) -> Result<TransactionsPage, DataError> {
        let mut count_builder = QueryBuilder::<Postgres>::new(
            "SELECT COUNT(DISTINCT t.txid) AS total
             FROM transactions t",
        );
        append_transaction_joins(&mut count_builder, address);
        count_builder.push(" WHERE t.status = ");
        count_builder.push_bind(status);
        append_transaction_filters(&mut count_builder, address, txid, None, None, None, None);
        let total = count_builder
            .build()
            .fetch_one(&self.pool)
            .await?
            .get::<i64, _>("total");

        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT DISTINCT t.txid, t.status, t.block_height, t.block_hash, t.time
             FROM transactions t",
        );
        append_transaction_joins(&mut builder, address);
        builder.push(" WHERE t.status = ");
        builder.push_bind(status);
        append_transaction_filters(&mut builder, address, txid, None, None, None, None);
        builder.push(" ORDER BY t.time DESC, t.txid DESC");
        builder.push(" OFFSET ");
        builder.push_bind(pagination.offset);
        builder.push(" LIMIT ");
        builder.push_bind(pagination.limit);

        let rows = builder.build().fetch_all(&self.pool).await?;
        let items = self.load_transaction_items(rows).await?;

        Ok(TransactionsPage {
            items,
            offset: pagination.offset,
            limit: pagination.limit,
            total,
        })
    }

    async fn load_transaction_items(&self, rows: Vec<sqlx::postgres::PgRow>) -> Result<Vec<TransactionItem>, DataError> {
        let txids: Vec<String> = rows.iter().map(|row| row.get::<String, _>("txid")).collect();
        if txids.is_empty() {
            return Ok(Vec::new());
        }

        let inputs_rows = sqlx::query(
            "SELECT i.txid, i.prev_txid, i.prev_vout, prev_o.address, prev_o.value_sats
             FROM tx_inputs i
             LEFT JOIN tx_outputs prev_o ON prev_o.txid = i.prev_txid AND prev_o.vout = i.prev_vout
             WHERE i.txid = ANY($1)
             ORDER BY i.txid, i.vin",
        )
        .bind(&txids)
        .fetch_all(&self.pool)
        .await?;

        let outputs_rows = sqlx::query(
            "SELECT txid, vout, address, value_sats
             FROM tx_outputs
             WHERE txid = ANY($1)
             ORDER BY txid, vout",
        )
        .bind(&txids)
        .fetch_all(&self.pool)
        .await?;

        let mut inputs_map: HashMap<String, Vec<TransactionIo>> = HashMap::new();
        for row in inputs_rows {
            inputs_map
                .entry(row.get::<String, _>("txid"))
                .or_default()
                .push(TransactionIo {
                    txid: Some(row.get::<String, _>("prev_txid")),
                    vout: Some(row.get::<i32, _>("prev_vout")),
                    address: row.try_get::<String, _>("address").ok(),
                    value_sats: row.try_get::<i64, _>("value_sats").ok(),
                });
        }

        let mut outputs_map: HashMap<String, Vec<TransactionIo>> = HashMap::new();
        for row in outputs_rows {
            outputs_map
                .entry(row.get::<String, _>("txid"))
                .or_default()
                .push(TransactionIo {
                    txid: None,
                    vout: Some(row.get::<i32, _>("vout")),
                    address: row.try_get::<String, _>("address").ok(),
                    value_sats: Some(row.get::<i64, _>("value_sats")),
                });
        }

        Ok(rows
            .into_iter()
            .map(|row| {
                let txid = row.get::<String, _>("txid");
                TransactionItem {
                    inputs: inputs_map.remove(&txid).unwrap_or_default(),
                    outputs: outputs_map.remove(&txid).unwrap_or_default(),
                    status: row.get::<String, _>("status"),
                    block_height: row.try_get::<i32, _>("block_height").ok(),
                    block_hash: row.try_get::<String, _>("block_hash").ok(),
                    time: row.get::<i64, _>("time"),
                    txid,
                }
            })
            .collect())
    }
}

fn append_transaction_joins(builder: &mut QueryBuilder<'_, Postgres>, address: Option<&str>) {
    if address.is_some() {
        builder.push(
            " LEFT JOIN tx_outputs o ON o.txid = t.txid
              LEFT JOIN tx_inputs i ON i.txid = t.txid
              LEFT JOIN tx_outputs prev_o ON prev_o.txid = i.prev_txid AND prev_o.vout = i.prev_vout",
        );
    }
}

fn append_transaction_filters<'a>(
    builder: &mut QueryBuilder<'a, Postgres>,
    address: Option<&'a str>,
    txid: Option<&'a str>,
    from_height: Option<i32>,
    to_height: Option<i32>,
    from_time: Option<i64>,
    to_time: Option<i64>,
) {
    if let Some(address) = address {
        builder.push(" AND (o.address = ");
        builder.push_bind(address);
        builder.push(" OR prev_o.address = ");
        builder.push_bind(address);
        builder.push(")");
    }

    if let Some(txid) = txid {
        builder.push(" AND t.txid = ");
        builder.push_bind(txid);
    }

    if let Some(from_height) = from_height {
        builder.push(" AND t.block_height >= ");
        builder.push_bind(from_height);
    }

    if let Some(to_height) = to_height {
        builder.push(" AND t.block_height <= ");
        builder.push_bind(to_height);
    }

    if let Some(from_time) = from_time {
        builder.push(" AND t.time >= ");
        builder.push_bind(from_time);
    }

    if let Some(to_time) = to_time {
        builder.push(" AND t.time <= ");
        builder.push_bind(to_time);
    }
}

fn append_block_joins(
    builder: &mut QueryBuilder<'_, Postgres>,
    has_txid: Option<&str>,
    address: Option<&str>,
) {
    if has_txid.is_some() || address.is_some() {
        builder.push(" LEFT JOIN transactions t ON t.block_hash = b.hash AND t.status = 'confirmed'");
    }

    if address.is_some() {
        builder.push(
            " LEFT JOIN tx_outputs o ON o.txid = t.txid
              LEFT JOIN tx_inputs i ON i.txid = t.txid
              LEFT JOIN tx_outputs prev_o ON prev_o.txid = i.prev_txid AND prev_o.vout = i.prev_vout",
        );
    }
}

fn append_block_filters<'a>(
    builder: &mut QueryBuilder<'a, Postgres>,
    from_height: Option<i32>,
    to_height: Option<i32>,
    from_time: Option<i64>,
    to_time: Option<i64>,
    block_hash: Option<&'a str>,
    has_txid: Option<&'a str>,
    address: Option<&'a str>,
) {
    apply_block_bounds(builder, "b", from_height, to_height, from_time, to_time);

    if let Some(block_hash) = block_hash {
        builder.push(" AND b.hash = ");
        builder.push_bind(block_hash);
    }

    if let Some(has_txid) = has_txid {
        builder.push(" AND t.txid = ");
        builder.push_bind(has_txid);
    }

    if let Some(address) = address {
        builder.push(" AND (o.address = ");
        builder.push_bind(address);
        builder.push(" OR prev_o.address = ");
        builder.push_bind(address);
        builder.push(")");
    }
}

fn append_balance_history_filters(
    builder: &mut QueryBuilder<'_, Postgres>,
    from_height: Option<i32>,
    to_height: Option<i32>,
    from_time: Option<i64>,
    to_time: Option<i64>,
) {
    if let Some(from_height) = from_height {
        builder.push(" AND abh.block_height >= ");
        builder.push_bind(from_height);
    }

    if let Some(to_height) = to_height {
        builder.push(" AND abh.block_height <= ");
        builder.push_bind(to_height);
    }

    if let Some(from_time) = from_time {
        builder.push(" AND abh.time >= ");
        builder.push_bind(from_time);
    }

    if let Some(to_time) = to_time {
        builder.push(" AND abh.time <= ");
        builder.push_bind(to_time);
    }
}

fn apply_block_bounds(
    builder: &mut QueryBuilder<'_, Postgres>,
    table_ref: &str,
    from_height: Option<i32>,
    to_height: Option<i32>,
    from_time: Option<i64>,
    to_time: Option<i64>,
) {
    if let Some(from_height) = from_height {
        builder.push(" AND ");
        builder.push(table_ref);
        builder.push(".height >= ");
        builder.push_bind(from_height);
    }

    if let Some(to_height) = to_height {
        builder.push(" AND ");
        builder.push(table_ref);
        builder.push(".height <= ");
        builder.push_bind(to_height);
    }

    if let Some(from_time) = from_time {
        builder.push(" AND ");
        builder.push(table_ref);
        builder.push(".time >= ");
        builder.push_bind(from_time);
    }

    if let Some(to_time) = to_time {
        builder.push(" AND ");
        builder.push(table_ref);
        builder.push(".time <= ");
        builder.push_bind(to_time);
    }
}
