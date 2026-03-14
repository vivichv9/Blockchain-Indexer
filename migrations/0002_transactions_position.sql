ALTER TABLE transactions
    ADD COLUMN IF NOT EXISTS position_in_block INT NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_transactions_block_height_position
    ON transactions(block_height, position_in_block);
