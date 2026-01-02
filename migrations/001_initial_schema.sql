-- OreVault Database Migrations
-- Migration 001: Initial Schema

-- Sessions table - tracks mining sessions
CREATE TABLE IF NOT EXISTS sessions (
    id UUID PRIMARY KEY,
    user_wallet VARCHAR(64) NOT NULL,
    strategy VARCHAR(32) NOT NULL,
    max_tip DECIMAL(20, 9) NOT NULL,
    deploy_amount DECIMAL(20, 9) NOT NULL,
    budget DECIMAL(20, 9) NOT NULL,
    rounds_played BIGINT NOT NULL DEFAULT 0,
    rounds_skipped BIGINT NOT NULL DEFAULT 0,
    total_deployed DECIMAL(20, 9) NOT NULL DEFAULT 0,
    total_tips DECIMAL(20, 9) NOT NULL DEFAULT 0,
    total_won DECIMAL(20, 9) NOT NULL DEFAULT 0,
    net_pnl DECIMAL(20, 9) NOT NULL DEFAULT 0,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_sessions_wallet ON sessions(user_wallet);
CREATE INDEX IF NOT EXISTS idx_sessions_active ON sessions(user_wallet, is_active);

-- Transactions table - tracks all deploy transactions
CREATE TABLE IF NOT EXISTS transactions (
    id UUID PRIMARY KEY,
    user_wallet VARCHAR(64) NOT NULL,
    session_id UUID REFERENCES sessions(id),
    round_id BIGINT NOT NULL,
    tx_signature VARCHAR(128),
    block_index SMALLINT NOT NULL CHECK (block_index >= 0 AND block_index < 25),
    deploy_amount DECIMAL(20, 9) NOT NULL,
    tip_amount DECIMAL(20, 9) NOT NULL,
    expected_ev DECIMAL(20, 9) NOT NULL,
    actual_reward DECIMAL(20, 9),
    status VARCHAR(32) NOT NULL DEFAULT 'pending',
    strategy VARCHAR(32) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_transactions_wallet ON transactions(user_wallet);
CREATE INDEX IF NOT EXISTS idx_transactions_session ON transactions(session_id);
CREATE INDEX IF NOT EXISTS idx_transactions_round ON transactions(round_id);
CREATE INDEX IF NOT EXISTS idx_transactions_status ON transactions(status);

-- Unclaimed balances table - tracks ORE account balances
CREATE TABLE IF NOT EXISTS unclaimed_balances (
    id UUID PRIMARY KEY,
    user_wallet VARCHAR(64) NOT NULL UNIQUE,
    unclaimed_sol DECIMAL(20, 9) NOT NULL DEFAULT 0,
    unclaimed_ore DECIMAL(20, 9) NOT NULL DEFAULT 0,
    refined_ore DECIMAL(20, 9) NOT NULL DEFAULT 0,
    last_synced TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_unclaimed_wallet ON unclaimed_balances(user_wallet);

-- Claims table - tracks all claim transactions
CREATE TABLE IF NOT EXISTS claims (
    id UUID PRIMARY KEY,
    user_wallet VARCHAR(64) NOT NULL,
    claim_type VARCHAR(16) NOT NULL CHECK (claim_type IN ('sol', 'ore')),
    gross_amount DECIMAL(20, 9) NOT NULL,
    fee_amount DECIMAL(20, 9) NOT NULL,
    net_amount DECIMAL(20, 9) NOT NULL,
    tx_signature VARCHAR(128),
    status VARCHAR(32) NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_claims_wallet ON claims(user_wallet);
CREATE INDEX IF NOT EXISTS idx_claims_status ON claims(status);
CREATE INDEX IF NOT EXISTS idx_claims_type ON claims(claim_type);

-- Balance history table - audit trail for balance changes
CREATE TABLE IF NOT EXISTS balance_history (
    id UUID PRIMARY KEY,
    user_wallet VARCHAR(64) NOT NULL,
    balance_type VARCHAR(32) NOT NULL,
    change_amount DECIMAL(20, 9) NOT NULL,
    reason VARCHAR(128) NOT NULL,
    reference_id UUID,
    balance_before DECIMAL(20, 9) NOT NULL,
    balance_after DECIMAL(20, 9) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_balance_history_wallet ON balance_history(user_wallet);
CREATE INDEX IF NOT EXISTS idx_balance_history_type ON balance_history(balance_type);
CREATE INDEX IF NOT EXISTS idx_balance_history_created ON balance_history(created_at);

-- Add comments for documentation
COMMENT ON TABLE sessions IS 'Mining sessions for automated ORE v3 mining';
COMMENT ON TABLE transactions IS 'Deploy transactions for each round';
COMMENT ON TABLE unclaimed_balances IS 'Cached unclaimed balances from ORE account';
COMMENT ON TABLE claims IS 'SOL and ORE claim transactions';
COMMENT ON TABLE balance_history IS 'Audit trail for all balance changes';
