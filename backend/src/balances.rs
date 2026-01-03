//! Balance Manager
//! 
//! Tracks and syncs balances between wallet and on-chain Miner account.
//! Handles both wallet SOL/ORE and unclaimed Miner account balances.

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

/// Unclaimed balances (in Miner account, not yet claimed)
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

/// ORE token decimals (11)
const ORE_DECIMALS: f64 = 100_000_000_000.0;

/// SOL decimals (9)
const SOL_DECIMALS: f64 = 1_000_000_000.0;

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
    
    /// Get all balances for a wallet (on-chain)
    pub async fn get_all_balances(&self, wallet: &str) -> Result<AllBalances> {
        let wallet_pubkey: Pubkey = wallet.parse()
            .context("Invalid wallet address")?;
        
        // Fetch wallet balances
        let sol_balance = self.ore_client.get_sol_balance(&wallet_pubkey).await?;
        let ore_token_balance = self.ore_client.get_ore_token_balance(&wallet_pubkey).await?;
        
        // Fetch Miner account balances
        let miner_data = self.ore_client.get_miner_data(&wallet_pubkey).await?;
        
        // Convert to human-readable units
        let wallet_sol = sol_balance as f64 / SOL_DECIMALS;
        let wallet_ore = ore_token_balance as f64 / ORE_DECIMALS;
        
        // Get unclaimed from miner account
        let (unclaimed_sol, unclaimed_ore, refined_ore) = match miner_data {
            Some(miner) => (
                miner.rewards_sol as f64 / SOL_DECIMALS,
                miner.rewards_ore as f64 / ORE_DECIMALS,
                miner.refined_ore as f64 / ORE_DECIMALS,
            ),
            None => (0.0, 0.0, 0.0),
        };
        
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
        
        // Update database with new balances (convert to lamports i64)
        db.update_unclaimed_balance(
            wallet,
            (balances.unclaimed.sol * SOL_DECIMALS) as i64,
            (balances.unclaimed.ore * ORE_DECIMALS) as i64,
            (balances.unclaimed.refined_ore * ORE_DECIMALS) as i64,
        ).await?;
        
        debug!(
            "Synced balances for {}: wallet_sol={:.4}, wallet_ore={:.4}, unclaimed_sol={:.4}, unclaimed_ore={:.4}",
            wallet, balances.wallet.sol, balances.wallet.ore, balances.unclaimed.sol, balances.unclaimed.ore
        );
        
        Ok(balances)
    }
    
    /// Get just the wallet SOL balance
    pub async fn get_sol_balance(&self, wallet: &str) -> Result<f64> {
        let wallet_pubkey: Pubkey = wallet.parse()
            .context("Invalid wallet address")?;
        
        let balance = self.ore_client.get_sol_balance(&wallet_pubkey).await?;
        Ok(balance as f64 / SOL_DECIMALS)
    }
    
    /// Get just the wallet ORE token balance
    pub async fn get_ore_balance(&self, wallet: &str) -> Result<f64> {
        let wallet_pubkey: Pubkey = wallet.parse()
            .context("Invalid wallet address")?;
        
        let balance = self.ore_client.get_ore_token_balance(&wallet_pubkey).await?;
        Ok(balance as f64 / ORE_DECIMALS)
    }
    
    /// Check if wallet has enough SOL for a transaction
    pub async fn has_sufficient_sol(&self, wallet: &str, required: f64) -> Result<bool> {
        let balance = self.get_sol_balance(wallet).await?;
        Ok(balance >= required)
    }
    
    /// Get miner account stats for a wallet
    pub async fn get_miner_stats(&self, wallet: &str) -> Result<Option<MinerStats>> {
        let wallet_pubkey: Pubkey = wallet.parse()
            .context("Invalid wallet address")?;
        
        let miner_data = self.ore_client.get_miner_data(&wallet_pubkey).await?;
        
        Ok(miner_data.map(|m| MinerStats {
            current_round_id: m.round_id,
            lifetime_deployed: m.lifetime_deployed as f64 / SOL_DECIMALS,
            lifetime_rewards_sol: m.lifetime_rewards_sol as f64 / SOL_DECIMALS,
            lifetime_rewards_ore: m.lifetime_rewards_ore as f64 / ORE_DECIMALS,
        }))
    }
}

/// Miner lifetime stats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinerStats {
    pub current_round_id: u64,
    pub lifetime_deployed: f64,
    pub lifetime_rewards_sol: f64,
    pub lifetime_rewards_ore: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_fee_calculation() {
        let amount = 1.0;
        let claimable = amount * (1.0 - CLAIM_FEE_PERCENT);
        assert!((claimable - 0.9).abs() < 0.0001);
    }
}
