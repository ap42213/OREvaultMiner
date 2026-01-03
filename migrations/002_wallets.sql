-- Wallet Management Table
-- Stores the mining wallet keypair for automine functionality
-- Critical: This wallet holds ORE rewards and SOL for fees!

-- Enable UUID extension (Supabase has this by default)
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE IF NOT EXISTS wallets (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    
    -- Wallet identification
    wallet_address TEXT NOT NULL UNIQUE,
    name TEXT DEFAULT 'Mining Wallet',
    
    -- Private key (base58 encoded)
    -- IMPORTANT: Export this to recover funds!
    private_key_b58 TEXT NOT NULL,
    
    -- Status
    is_active BOOLEAN DEFAULT true,
    
    -- Metadata
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    last_used_at TIMESTAMP WITH TIME ZONE
);

-- Index for quick lookups
CREATE INDEX IF NOT EXISTS idx_wallets_active ON wallets(is_active);

-- Add helpful comment
COMMENT ON TABLE wallets IS 'Mining wallet keypairs - BACKUP PRIVATE KEYS!';
