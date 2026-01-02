//! Database Module
//! 
//! PostgreSQL database operations using sqlx.
//! Handles sessions, transactions, balances, and claims.

use anyhow::{Result, Context};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPool, FromRow};
use uuid::Uuid;
use tracing::{debug, info};

use crate::Strategy;

/// Database wrapper
#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

// =============================================================================
// Data Models
// =============================================================================

/// Mining session record
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub user_wallet: String,
    pub strategy: String,
    pub max_tip: Decimal,
    pub deploy_amount: Decimal,
    pub budget: Decimal,
    pub rounds_played: i64,
    pub rounds_skipped: i64,
    pub total_deployed: Decimal,
    pub total_tips: Decimal,
    pub total_won: Decimal,
    pub net_pnl: Decimal,
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
    pub deploy_amount: Decimal,
    pub tip_amount: Decimal,
    pub expected_ev: Decimal,
    pub actual_reward: Option<Decimal>,
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
    pub unclaimed_sol: Decimal,
    pub unclaimed_ore: Decimal,
    pub refined_ore: Decimal,
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
    pub gross_amount: Decimal,
    pub fee_amount: Decimal,
    pub net_amount: Decimal,
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
    pub change_amount: Decimal,
    pub reason: String,
    pub reference_id: Option<Uuid>,
    pub balance_before: Decimal,
    pub balance_after: Decimal,
    pub created_at: DateTime<Utc>,
}

/// Session statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStats {
    pub rounds_played: i64,
    pub rounds_skipped: i64,
    pub rounds_won: i64,
    pub rounds_lost: i64,
    pub total_deployed: f64,
    pub total_tips: f64,
    pub total_won: f64,
    pub net_pnl: f64,
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
    pub async fn create_session(
        &self,
        wallet: &str,
        strategy: Strategy,
        max_tip: f64,
        deploy_amount: f64,
        budget: f64,
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
        .bind(Decimal::try_from(max_tip).unwrap_or_default())
        .bind(Decimal::try_from(deploy_amount).unwrap_or_default())
        .bind(Decimal::try_from(budget).unwrap_or_default())
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
    
    /// Update session statistics
    pub async fn update_session_stats(
        &self,
        session_id: Uuid,
        deployed: f64,
        tip: f64,
        reward: Option<f64>,
        is_skip: bool,
    ) -> Result<()> {
        let won = reward.unwrap_or(0.0);
        let deployed_dec = Decimal::try_from(deployed).unwrap_or_default();
        let tip_dec = Decimal::try_from(tip).unwrap_or_default();
        let won_dec = Decimal::try_from(won).unwrap_or_default();
        
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
            sqlx::query(
                r#"
                UPDATE sessions SET
                    rounds_played = rounds_played + 1,
                    total_deployed = total_deployed + $2,
                    total_tips = total_tips + $3,
                    total_won = total_won + $4,
                    net_pnl = total_won - total_deployed - total_tips,
                    updated_at = NOW()
                WHERE id = $1
                "#,
            )
            .bind(session_id)
            .bind(deployed_dec)
            .bind(tip_dec)
            .bind(won_dec)
            .execute(&self.pool)
            .await?;
        }
        
        Ok(())
    }
    
    /// Get session statistics
    pub async fn get_session_stats(&self, wallet: &str) -> Result<SessionStats> {
        let result = sqlx::query_as::<_, (i64, i64, Decimal, Decimal, Decimal, Decimal)>(
            r#"
            SELECT 
                COALESCE(SUM(rounds_played), 0),
                COALESCE(SUM(rounds_skipped), 0),
                COALESCE(SUM(total_deployed), 0),
                COALESCE(SUM(total_tips), 0),
                COALESCE(SUM(total_won), 0),
                COALESCE(SUM(net_pnl), 0)
            FROM sessions
            WHERE user_wallet = $1
            "#,
        )
        .bind(wallet)
        .fetch_one(&self.pool)
        .await
        .context("Failed to get session stats")?;
        
        // Count wins and losses from transactions
        let (wins, losses): (i64, i64) = sqlx::query_as(
            r#"
            SELECT 
                COALESCE(SUM(CASE WHEN status = 'won' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status = 'lost' THEN 1 ELSE 0 END), 0)
            FROM transactions
            WHERE user_wallet = $1
            "#,
        )
        .bind(wallet)
        .fetch_one(&self.pool)
        .await
        .unwrap_or((0, 0));
        
        let rounds_played = result.0;
        let win_rate = if rounds_played > 0 {
            wins as f64 / rounds_played as f64 * 100.0
        } else {
            0.0
        };
        
        Ok(SessionStats {
            rounds_played: result.0,
            rounds_skipped: result.1,
            rounds_won: wins,
            rounds_lost: losses,
            total_deployed: result.2.try_into().unwrap_or(0.0),
            total_tips: result.3.try_into().unwrap_or(0.0),
            total_won: result.4.try_into().unwrap_or(0.0),
            net_pnl: result.5.try_into().unwrap_or(0.0),
            win_rate,
        })
    }
    
    // =========================================================================
    // Transaction Operations
    // =========================================================================
    
    /// Record a new transaction
    pub async fn create_transaction(
        &self,
        wallet: &str,
        session_id: Option<Uuid>,
        round_id: i64,
        block_index: i16,
        deploy_amount: f64,
        tip_amount: f64,
        expected_ev: f64,
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
        .bind(Decimal::try_from(deploy_amount).unwrap_or_default())
        .bind(Decimal::try_from(tip_amount).unwrap_or_default())
        .bind(Decimal::try_from(expected_ev).unwrap_or_default())
        .bind(strategy)
        .fetch_one(&self.pool)
        .await
        .context("Failed to create transaction")?;
        
        Ok(tx)
    }
    
    /// Update transaction with result
    pub async fn update_transaction(
        &self,
        tx_id: Uuid,
        signature: &str,
        status: TxStatus,
        reward: Option<f64>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE transactions SET
                tx_signature = $2,
                status = $3,
                actual_reward = $4,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(tx_id)
        .bind(signature)
        .bind(status.as_str())
        .bind(reward.map(|r| Decimal::try_from(r).unwrap_or_default()))
        .execute(&self.pool)
        .await
        .context("Failed to update transaction")?;
        
        Ok(())
    }
    
    /// Get transaction history for wallet
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
    
    /// Update unclaimed balances
    pub async fn update_unclaimed_balances(
        &self,
        wallet: &str,
        unclaimed_sol: f64,
        unclaimed_ore: f64,
        refined_ore: f64,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO unclaimed_balances (
                id, user_wallet, unclaimed_sol, unclaimed_ore, refined_ore,
                last_synced, created_at, updated_at
            )
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
        .bind(Decimal::try_from(unclaimed_sol).unwrap_or_default())
        .bind(Decimal::try_from(unclaimed_ore).unwrap_or_default())
        .bind(Decimal::try_from(refined_ore).unwrap_or_default())
        .execute(&self.pool)
        .await
        .context("Failed to update unclaimed balances")?;
        
        Ok(())
    }
    
    /// Get unclaimed balances
    pub async fn get_unclaimed_balances(&self, wallet: &str) -> Result<Option<UnclaimedBalance>> {
        let balance = sqlx::query_as::<_, UnclaimedBalance>(
            "SELECT * FROM unclaimed_balances WHERE user_wallet = $1"
        )
        .bind(wallet)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch unclaimed balances")?;
        
        Ok(balance)
    }
    
    // =========================================================================
    // Claims Operations
    // =========================================================================
    
    /// Record a new claim
    pub async fn create_claim(
        &self,
        wallet: &str,
        claim_type: &str,
        gross_amount: f64,
        fee_amount: f64,
        net_amount: f64,
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
        .bind(Decimal::try_from(gross_amount).unwrap_or_default())
        .bind(Decimal::try_from(fee_amount).unwrap_or_default())
        .bind(Decimal::try_from(net_amount).unwrap_or_default())
        .fetch_one(&self.pool)
        .await
        .context("Failed to create claim")?;
        
        Ok(claim)
    }
    
    /// Update claim with signature and status
    pub async fn update_claim(
        &self,
        claim_id: Uuid,
        signature: &str,
        status: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE claims SET
                tx_signature = $2,
                status = $3,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(claim_id)
        .bind(signature)
        .bind(status)
        .execute(&self.pool)
        .await
        .context("Failed to update claim")?;
        
        Ok(())
    }
    
    /// Get claims history
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
    // Balance History Operations
    // =========================================================================
    
    /// Record balance change in audit log
    pub async fn record_balance_change(
        &self,
        wallet: &str,
        balance_type: &str,
        change_amount: f64,
        reason: &str,
        reference_id: Option<Uuid>,
        balance_before: f64,
        balance_after: f64,
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
        .bind(Decimal::try_from(change_amount).unwrap_or_default())
        .bind(reason)
        .bind(reference_id)
        .bind(Decimal::try_from(balance_before).unwrap_or_default())
        .bind(Decimal::try_from(balance_after).unwrap_or_default())
        .execute(&self.pool)
        .await
        .context("Failed to record balance change")?;
        
        Ok(())
    }
}
