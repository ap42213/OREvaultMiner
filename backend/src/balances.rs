//! Balance Manager
//! 
//! Tracks and syncs balances between wallet and on-chain ORE account.
//! Handles both wallet SOL/ORE and unclaimed ORE account balances.

use anyhow::{Result, Context};
use chrono::{DateTime, Utc};
use solana_sdk::pubkey::Pubkey;
use serde::{Deserialize, Serialize};
use tracing::{info, debug};

use crate::ore::OreClient;
use crate::db::Database;

/// Complete balance information for a user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllBalances {
    pub wallet: WalletBalances,
    pub unclaimed: UnclaimedBalances,
    pub claimable: ClaimableBalances,
    pub last_synced: DateTime<Utc>,
}

/// Wallet balances (directly in wallet)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletBalances {
    /// SOL balance in wallet
    pub sol: f64,
    /// ORE token balance in wallet
    pub ore: f64,
}

/// Unclaimed balances (in ORE account, not yet claimed)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnclaimedBalances {
    /// Unclaimed SOL from winnings
    pub sol: f64,
    /// Unclaimed ORE tokens
    pub ore: f64,
    /// Refined ORE (accrues while holding)
    pub refined_ore: f64,
}

/// Claimable amounts after 10% fee
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimableBalances {
    /// Net SOL after 10% fee
    pub sol: f64,
    /// Net ORE after 10% fee
    pub ore: f64,
}

/// Fee percentage
const CLAIM_FEE_PERCENT: f64 = 0.10;

/// Balance manager for tracking user balances
#[derive(Clone)]
pub struct BalanceManager {
    ore_client: OreClient,
}

impl BalanceManager {
    /// Create a new balance manager
    pub fn new(ore_client: OreClient) -> Self {
        Self { ore_client }
    }
    
    /// Get all balances for a wallet (cached + on-chain)
    pub async fn get_all_balances(&self, wallet: &str) -> Result<AllBalances> {
        let wallet_pubkey: Pubkey = wallet.parse()
            .context("Invalid wallet address")?;
        
        // Fetch wallet balances
        let sol_balance = self.ore_client.get_sol_balance(&wallet_pubkey).await?;
        let ore_token_balance = self.ore_client.get_ore_token_balance(&wallet_pubkey).await?;
        
        // Fetch ORE account balances
        let ore_account = self.ore_client.get_ore_account_balance(&wallet_pubkey).await?;
        
        // Convert to human-readable units
        let wallet_sol = sol_balance as f64 / 1_000_000_000.0;
        let wallet_ore = ore_token_balance as f64 / 1_000_000_000.0;
        let unclaimed_sol = ore_account.unclaimed_sol as f64 / 1_000_000_000.0;
        let unclaimed_ore = ore_account.unclaimed_ore as f64 / 1_000_000_000.0;
        let refined_ore = ore_account.refined_ore as f64 / 1_000_000_000.0;
        
        // Calculate claimable after fee
        let claimable_sol = unclaimed_sol * (1.0 - CLAIM_FEE_PERCENT);
        let claimable_ore = unclaimed_ore * (1.0 - CLAIM_FEE_PERCENT);
        
        Ok(AllBalances {
            wallet: WalletBalances {
                sol: wallet_sol,
                ore: wallet_ore,
            },
            unclaimed: UnclaimedBalances {
                sol: unclaimed_sol,
                ore: unclaimed_ore,
                refined_ore,
            },
            claimable: ClaimableBalances {
                sol: claimable_sol,
                ore: claimable_ore,
            },
            last_synced: Utc::now(),
        })
    }
    
    /// Sync balances from on-chain and update database
    pub async fn sync_from_chain(&self, wallet: &str, db: &Database) -> Result<AllBalances> {
        let balances = self.get_all_balances(wallet).await?;
        
        // Update database with new balances (convert f64 to lamports i64)
        db.update_unclaimed_balance(
            wallet,
            (balances.unclaimed.sol * 1_000_000_000.0) as i64,
            (balances.unclaimed.ore * 1_000_000_000.0) as i64,
            (balances.unclaimed.refined_ore * 1_000_000_000.0) as i64,
        ).await?;
        
        info!(
            "Synced balances for {}: wallet={:.4} SOL, unclaimed={:.4} SOL",
            wallet, balances.wallet.sol, balances.unclaimed.sol
        );
        
        Ok(balances)
    }
    
    /// Get just the wallet SOL balance
    pub async fn get_wallet_sol(&self, wallet: &str) -> Result<f64> {
        let wallet_pubkey: Pubkey = wallet.parse()
            .context("Invalid wallet address")?;
        
        let balance = self.ore_client.get_sol_balance(&wallet_pubkey).await?;
        Ok(balance as f64 / 1_000_000_000.0)
    }
    
    /// Check if wallet has enough SOL for a deployment
    pub async fn has_sufficient_balance(
        &self,
        wallet: &str,
        deploy_amount: f64,
        tip_amount: f64,
    ) -> Result<bool> {
        let balance = self.get_wallet_sol(wallet).await?;
        let required = deploy_amount + tip_amount + 0.001; // Add buffer for tx fees
        Ok(balance >= required)
    }
    
    /// Format balance for display
    pub fn format_balance(amount: f64, decimals: usize) -> String {
        format!("{:.decimals$}", amount, decimals = decimals)
    }
    
    /// Get balance summary string
    pub fn format_summary(balances: &AllBalances) -> String {
        format!(
            "Wallet: {:.4} SOL / {:.2} ORE | Unclaimed: {:.4} SOL / {:.2} ORE | Refined: {:.2} ORE",
            balances.wallet.sol,
            balances.wallet.ore,
            balances.unclaimed.sol,
            balances.unclaimed.ore,
            balances.unclaimed.refined_ore,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_format_balance() {
        assert_eq!(BalanceManager::format_balance(1.234567, 4), "1.2346");
        assert_eq!(BalanceManager::format_balance(0.1, 2), "0.10");
        assert_eq!(BalanceManager::format_balance(100.0, 0), "100");
    }
    
    #[test]
    fn test_claimable_calculation() {
        let unclaimed = 1.0;
        let claimable = unclaimed * (1.0 - CLAIM_FEE_PERCENT);
        assert!((claimable - 0.9).abs() < 0.0001);
    }
}
