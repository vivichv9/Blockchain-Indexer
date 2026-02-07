CREATE TABLE IF NOT EXISTS blocks (
    id BIGSERIAL PRIMARY KEY,
    height INT NOT NULL,
    hash TEXT NOT NULL UNIQUE,
    prev_hash TEXT NOT NULL,
    time BIGINT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('canonical', 'orphaned')),
    meta JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE INDEX IF NOT EXISTS idx_blocks_height ON blocks(height);
CREATE INDEX IF NOT EXISTS idx_blocks_status_height ON blocks(status, height);

CREATE TABLE IF NOT EXISTS transactions (
    id BIGSERIAL PRIMARY KEY,
    txid TEXT NOT NULL UNIQUE,
    block_height INT NULL,
    block_hash TEXT NULL,
    time BIGINT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('confirmed', 'mempool', 'dropped', 'orphaned')),
    decoded JSONB NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_transactions_status ON transactions(status);
CREATE INDEX IF NOT EXISTS idx_transactions_block_height ON transactions(block_height);
CREATE INDEX IF NOT EXISTS idx_transactions_time ON transactions(time);

CREATE TABLE IF NOT EXISTS tx_outputs (
    txid TEXT NOT NULL,
    vout INT NOT NULL,
    value_sats BIGINT NOT NULL,
    script_type TEXT NOT NULL,
    address TEXT NULL,
    script_hex TEXT NOT NULL,
    PRIMARY KEY (txid, vout),
    CONSTRAINT fk_tx_outputs_txid FOREIGN KEY (txid) REFERENCES transactions(txid) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_tx_outputs_address ON tx_outputs(address);

CREATE TABLE IF NOT EXISTS tx_inputs (
    txid TEXT NOT NULL,
    vin INT NOT NULL,
    prev_txid TEXT NOT NULL,
    prev_vout INT NOT NULL,
    sequence BIGINT NOT NULL,
    PRIMARY KEY (txid, vin),
    CONSTRAINT fk_tx_inputs_txid FOREIGN KEY (txid) REFERENCES transactions(txid) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_tx_inputs_prev ON tx_inputs(prev_txid, prev_vout);

CREATE TABLE IF NOT EXISTS utxos_current (
    out_txid TEXT NOT NULL,
    out_vout INT NOT NULL,
    address TEXT NOT NULL,
    value_sats BIGINT NOT NULL,
    created_in_txid TEXT NOT NULL,
    spent_in_txid TEXT NULL,
    status TEXT NOT NULL CHECK (status IN ('unspent', 'spent')),
    PRIMARY KEY (out_txid, out_vout)
);

CREATE INDEX IF NOT EXISTS idx_utxos_current_address_status ON utxos_current(address, status);

CREATE TABLE IF NOT EXISTS address_balance_current (
    address TEXT PRIMARY KEY,
    balance_sats BIGINT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS address_balance_history (
    address TEXT NOT NULL,
    block_height INT NOT NULL,
    time BIGINT NOT NULL,
    balance_sats BIGINT NOT NULL,
    PRIMARY KEY (address, block_height)
);

CREATE INDEX IF NOT EXISTS idx_address_balance_history_address_time
    ON address_balance_history(address, time);

CREATE TABLE IF NOT EXISTS jobs (
    job_id TEXT PRIMARY KEY,
    mode TEXT NOT NULL CHECK (mode IN ('all_addresses', 'address_list')),
    status TEXT NOT NULL CHECK (status IN ('created', 'running', 'paused', 'failed', 'completed')),
    progress_height INT NOT NULL DEFAULT 0,
    last_error TEXT NULL,
    started_at TIMESTAMPTZ NULL,
    updated_at TIMESTAMPTZ NULL,
    config_snapshot JSONB NOT NULL
);

CREATE TABLE IF NOT EXISTS job_addresses (
    job_id TEXT NOT NULL,
    address TEXT NOT NULL,
    PRIMARY KEY (job_id, address),
    CONSTRAINT fk_job_addresses_job_id FOREIGN KEY (job_id) REFERENCES jobs(job_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS node_health (
    node_id TEXT PRIMARY KEY,
    last_seen_at TIMESTAMPTZ NOT NULL,
    tip_height INT NOT NULL,
    tip_hash TEXT NOT NULL,
    rpc_latency_ms INT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('ok', 'degraded', 'down')),
    details JSONB NOT NULL DEFAULT '{}'::jsonb
);
