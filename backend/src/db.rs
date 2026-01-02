//! Database Module
//! 
//! PostgreSQL database operations using sqlx.
//! Handles sessions, transactions, balances, and claims.
//! All amounts stored as i64 (lamports for SOL, raw units for ORE).

use anyhow::{Result, Context};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPool, FromRow, Postgres};
use uuid::Uuid;
use tracing::{debug, info};

use crate::Strategy;

/// Database wrapper
#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

// =============================================================================
// Data Models (using i64 for all amounts - stored as lamports)
// =============================================================================

/// Mining session record
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub user_wallet: String,
    pub strategy: String,
    pub max_tip: i64,
    pub deploy_amount: i64,
    pub budget: i64,
    pub rounds_played: i64,
    pub rounds_skipped: i64,
    pub total_deployed: i64,
    pub total_tips: i64,
    pub total_won: i64,
    pub net_pnl: i64,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Transaction record
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Transaction {
    pub id: Uuid,
    pub user_wallet: String,
    pub session_id: Option<Uuid>,
    pub round_id: i64,
    pub tx_signature: Option<String>,
    pub block_index: i16,
    pub deploy_amount: i64,
    pub tip_amount: i64,
    pub expected_ev: i64,
    pub actual_reward: Option<i64>,
    pub status: String,
    pub strategy: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Transaction status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TxStatus {
    Pending,
    Won,
    Lost,
    Skipped,
    Failed,
}

impl TxStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TxStatus::Pending => "pending",
            TxStatus::Won => "won",
            TxStatus::Lost => "lost",
            TxStatus::Skipped => "skipped",
            TxStatus::Failed => "failed",
        }
    }
}

/// Unclaimed balance record
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct UnclaimedBalance {
    pub id: Uuid,
    pub user_wallet: String,
    pub unclaimed_sol: i64,
    pub unclaimed_ore: i64,
    pub refined_ore: i64,
    pub last_synced: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Claim record
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Claim {
    pub id: Uuid,
    pub user_wallet: String,
    pub claim_type: String,
    pub gross_amount: i64,
    pub fee_amount: i64,
    pub net_amount: i64,
    pub tx_signature: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Balance history record for audit
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct BalanceHistory {
    pub id: Uuid,
    pub user_wallet: String,
    pub balance_type: String,
    pub change_amount: i64,
    pub reason: String,
    pub reference_id: Option<Uuid>,
    pub balance_before: i64,
    pub balance_after: i64,
    pub created_at: DateTime<Utc>,
}

/// Session statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStats {
    pub rounds_played: i64,
    pub rounds_skipped: i64,
    pub rounds_won: i64,
    pub rounds_lost: i64,
    pub total_deployed: i64,
    pub total_tips: i64,
    pub total_won: i64,
    pub net_pnl: i64,
    pub win_rate: f64,
}

// =============================================================================
// Database Implementation
// =============================================================================

impl Database {
    /// Create a new database instance
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    
    /// Get the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
    
    // =========================================================================
    // Session Operations
    // =========================================================================
    
    /// Create a new mining session
    /// Amounts are in lamports (1 SOL = 1_000_000_000 lamports)
    pub async fn create_session(
        &self,
        wallet: &str,
        strategy: Strategy,
        max_tip: i64,
        deploy_amount: i64,
        budget: i64,
    ) -> Result<Session> {
        let strategy_str = match strategy {
            Strategy::BestEv => "best_ev",
            Strategy::Conservative => "conservative",
            Strategy::Aggressive => "aggressive",
        };
        
        let session = sqlx::query_as::<_, Session>(
            r#"
            INSERT INTO sessions (
                id, user_wallet, strategy, max_tip, deploy_amount, budget,
                rounds_played, rounds_skipped, total_deployed, total_tips,
                total_won, net_pnl, is_active, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, 0, 0, 0, 0, 0, 0, true, NOW(), NOW())
            RETURNING *
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(wallet)
        .bind(strategy_str)
        .bind(max_tip)
        .bind(deploy_amount)
        .bind(budget)
        .fetch_one(&self.pool)
        .await
        .context("Failed to create session")?;
        
        info!("Created session {} for wallet {}", session.id, wallet);
        Ok(session)
    }
    
    /// End a mining session
    pub async fn end_session(&self, wallet: &str) -> Result<()> {
        sqlx::query(
            "UPDATE sessions SET is_active = false, updated_at = NOW() WHERE user_wallet = $1 AND is_active = true"
        )
        .bind(wallet)
        .execute(&self.pool)
        .await
        .context("Failed to end session")?;
        
        Ok(())
    }
    
    /// Get active session for wallet
    pub async fn get_active_session(&self, wallet: &str) -> Result<Option<Session>> {
        let session = sqlx::query_as::<_, Session>(
            "SELECT * FROM sessions WHERE user_wallet = $1 AND is_active = true ORDER BY created_at DESC LIMIT 1"
        )
        .bind(wallet)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch session")?;
        
        Ok(session)
    }
    
    /// Update session statistics (amounts in lamports)
    pub async fn update_session_stats(
        &self,
        session_id: Uuid,
        deployed: i64,
        tip: i64,
        reward: Option<i64>,
        is_skip: bool,
    ) -> Result<()> {
        let won = reward.unwrap_or(0);
        
        if is_skip {
            sqlx::query(
                r#"
                UPDATE sessions SET
                    rounds_skipped = rounds_skipped + 1,
                    updated_at = NOW()
                WHERE id = $1
                "#,
            )
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        } else {
            let net = won - deployed - tip;
            sqlx::query(
                r#"
                UPDATE sessions SET
                    rounds_played = rounds_played + 1,
                    total_deployed = total_deployed + $2,
                    total_tips = total_tips + $3,
                    total_won = total_won + $4,
                    net_pnl = net_pnl + $5,
                    updated_at = NOW()
                WHERE id = $1
                "#,
            )
            .bind(session_id)
            .bind(deployed)
            .bind(tip)
            .bind(won)
            .bind(net)
            .execute(&self.pool)
            .await?;
        }
        
        Ok(())
    }
    
    /// Get session stats
    pub async fn get_session_stats(&self, session_id: Uuid) -> Result<SessionStats> {
        let session = sqlx::query_as::<_, Session>(
            "SELECT * FROM sessions WHERE id = $1"
        )
        .bind(session_id)
        .fetch_one(&self.pool)
        .await
        .context("Session not found")?;
        
        let rounds_won = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM transactions WHERE session_id = $1 AND status = 'won'"
        )
        .bind(session_id)
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0);
        
        let rounds_lost = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM transactions WHERE session_id = $1 AND status = 'lost'"
        )
        .bind(session_id)
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0);
        
        let total_rounds = rounds_won + rounds_lost;
        let win_rate = if total_rounds > 0 {
            rounds_won as f64 / total_rounds as f64
        } else {
            0.0
        };
        
        Ok(SessionStats {
            rounds_played: session.rounds_played,
            rounds_skipped: session.rounds_skipped,
            rounds_won,
            rounds_lost,
            total_deployed: session.total_deployed,
            total_tips: session.total_tips,
            total_won: session.total_won,
            net_pnl: session.net_pnl,
            win_rate,
        })
    }
    
    // =========================================================================
    // Transaction Operations
    // =========================================================================
    
    /// Record a new transaction (amounts in lamports)
    pub async fn record_transaction(
        &self,
        wallet: &str,
        session_id: Option<Uuid>,
        round_id: i64,
        block_index: i16,
        deploy_amount: i64,
        tip_amount: i64,
        expected_ev: i64,
        strategy: &str,
    ) -> Result<Transaction> {
        let tx = sqlx::query_as::<_, Transaction>(
            r#"
            INSERT INTO transactions (
                id, user_wallet, session_id, round_id, block_index,
                deploy_amount, tip_amount, expected_ev, status, strategy,
                created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'pending', $9, NOW(), NOW())
            RETURNING *
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(wallet)
        .bind(session_id)
        .bind(round_id)
        .bind(block_index)
        .bind(deploy_amount)
        .bind(tip_amount)
        .bind(expected_ev)
        .bind(strategy)
        .fetch_one(&self.pool)
        .await
        .context("Failed to record transaction")?;
        
        debug!("Recorded transaction {} for round {}", tx.id, round_id);
        Ok(tx)
    }
    
    /// Update transaction status
    pub async fn update_transaction_status(
        &self,
        tx_id: Uuid,
        status: TxStatus,
        signature: Option<&str>,
        reward: Option<i64>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE transactions SET
                status = $2,
                tx_signature = COALESCE($3, tx_signature),
                actual_reward = $4,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(tx_id)
        .bind(status.as_str())
        .bind(signature)
        .bind(reward)
        .execute(&self.pool)
        .await
        .context("Failed to update transaction")?;
        
        Ok(())
    }
    
    /// Get transactions for wallet
    pub async fn get_transactions(
        &self,
        wallet: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Transaction>> {
        let transactions = sqlx::query_as::<_, Transaction>(
            r#"
            SELECT * FROM transactions
            WHERE user_wallet = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(wallet)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch transactions")?;
        
        Ok(transactions)
    }
    
    // =========================================================================
    // Balance Operations
    // =========================================================================
    
    /// Update unclaimed balance (amounts in lamports/raw units)
    pub async fn update_unclaimed_balance(
        &self,
        wallet: &str,
        unclaimed_sol: i64,
        unclaimed_ore: i64,
        refined_ore: i64,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO unclaimed_balances (id, user_wallet, unclaimed_sol, unclaimed_ore, refined_ore, last_synced, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, NOW(), NOW(), NOW())
            ON CONFLICT (user_wallet) DO UPDATE SET
                unclaimed_sol = $3,
                unclaimed_ore = $4,
                refined_ore = $5,
                last_synced = NOW(),
                updated_at = NOW()
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(wallet)
        .bind(unclaimed_sol)
        .bind(unclaimed_ore)
        .bind(refined_ore)
        .execute(&self.pool)
        .await
        .context("Failed to update unclaimed balance")?;
        
        Ok(())
    }
    
    /// Get unclaimed balance for wallet
    pub async fn get_unclaimed_balance(&self, wallet: &str) -> Result<Option<UnclaimedBalance>> {
        let balance = sqlx::query_as::<_, UnclaimedBalance>(
            "SELECT * FROM unclaimed_balances WHERE user_wallet = $1"
        )
        .bind(wallet)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch unclaimed balance")?;
        
        Ok(balance)
    }
    
    // =========================================================================
    // Claims Operations
    // =========================================================================
    
    /// Record a claim (amounts in lamports/raw units)
    pub async fn record_claim(
        &self,
        wallet: &str,
        claim_type: &str,
        gross_amount: i64,
        fee_amount: i64,
        net_amount: i64,
    ) -> Result<Claim> {
        let claim = sqlx::query_as::<_, Claim>(
            r#"
            INSERT INTO claims (
                id, user_wallet, claim_type, gross_amount, fee_amount,
                net_amount, status, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, 'pending', NOW(), NOW())
            RETURNING *
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(wallet)
        .bind(claim_type)
        .bind(gross_amount)
        .bind(fee_amount)
        .bind(net_amount)
        .fetch_one(&self.pool)
        .await
        .context("Failed to record claim")?;
        
        info!("Recorded claim {} for wallet {}: {} {}", claim.id, wallet, gross_amount, claim_type);
        Ok(claim)
    }
    
    /// Update claim status
    pub async fn update_claim_status(
        &self,
        claim_id: Uuid,
        status: &str,
        signature: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE claims SET
                status = $2,
                tx_signature = COALESCE($3, tx_signature),
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(claim_id)
        .bind(status)
        .bind(signature)
        .execute(&self.pool)
        .await
        .context("Failed to update claim")?;
        
        Ok(())
    }
    
    /// Get claims for wallet
    pub async fn get_claims(
        &self,
        wallet: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Claim>> {
        let claims = sqlx::query_as::<_, Claim>(
            r#"
            SELECT * FROM claims
            WHERE user_wallet = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(wallet)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch claims")?;
        
        Ok(claims)
    }
    
    // =========================================================================
    // Balance History
    // =========================================================================
    
    /// Record balance change for audit
    pub async fn record_balance_history(
        &self,
        wallet: &str,
        balance_type: &str,
        change_amount: i64,
        reason: &str,
        reference_id: Option<Uuid>,
        balance_before: i64,
        balance_after: i64,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO balance_history (
                id, user_wallet, balance_type, change_amount, reason,
                reference_id, balance_before, balance_after, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW())
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(wallet)
        .bind(balance_type)
        .bind(change_amount)
        .bind(reason)
        .bind(reference_id)
        .bind(balance_before)
        .bind(balance_after)
        .execute(&self.pool)
        .await
        .context("Failed to record balance history")?;
        
        Ok(())
    }
    
    /// Get balance history for wallet
    pub async fn get_balance_history(
        &self,
        wallet: &str,
        limit: i64,
    ) -> Result<Vec<BalanceHistory>> {
        let history = sqlx::query_as::<_, BalanceHistory>(
            r#"
            SELECT * FROM balance_history
            WHERE user_wallet = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(wallet)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch balance history")?;
        
        Ok(history)
    }
}
