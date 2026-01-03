//! Claims Processor
//! 
//! Handles claiming SOL and ORE from the on-chain Miner account to wallet.
//! All claims incur a 10% fee taken by the ORE protocol.

use anyhow::{Result, Context};
use solana_sdk::{
    pubkey::Pubkey,
    transaction::Transaction,
};
use tracing::{info, debug};

use crate::ore::OreClient;

/// Fee percentage for all claims (10%)
pub const CLAIM_FEE_PERCENT: f64 = 0.10;

/// Result of building a claim transaction
#[derive(Debug, Clone)]
pub struct ClaimTxData {
    /// Serialized transaction (base64) for wallet to sign
    pub serialized_tx: String,
    /// Gross amount being claimed
    pub gross_amount: f64,
    /// Fee amount (10%)
    pub fee_amount: f64,
    /// Net amount after fee
    pub net_amount: f64,
}

/// Type of claim
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimType {
    Sol,
    Ore,
}

impl ClaimType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ClaimType::Sol => "sol",
            ClaimType::Ore => "ore",
        }
    }
}

/// Claims processor for handling SOL and ORE claims
#[derive(Clone)]
pub struct ClaimsProcessor {
    ore_client: OreClient,
}

impl ClaimsProcessor {
    /// Create a new claims processor
    pub fn new(ore_client: OreClient) -> Self {
        Self { ore_client }
    }
    
    /// Build a transaction to claim SOL from Miner account
    /// Returns transaction for wallet to sign
    /// Note: ORE v3 claims all available at once
    pub async fn build_claim_sol_tx(
        &self,
        wallet: &str,
        _amount: Option<f64>, // Ignored - claims all
    ) -> Result<ClaimTxData> {
        let wallet_pubkey: Pubkey = wallet.parse()
            .context("Invalid wallet address")?;
        
        // Get available balance from Miner account
        let (rewards_sol, _rewards_ore) = self.ore_client.get_unclaimed_balances(&wallet_pubkey).await?;
        
        if rewards_sol == 0 {
            anyhow::bail!("No SOL available to claim");
        }
        
        // Calculate fees (ORE v3 claims all at once)
        let gross_sol = rewards_sol as f64 / 1_000_000_000.0;
        let fee_sol = gross_sol * CLAIM_FEE_PERCENT;
        let net_sol = gross_sol - fee_sol;
        
        // Build claim instruction using ore-api SDK
        let claim_ix = self.ore_client.build_claim_sol_instruction(&wallet_pubkey)?;
        
        // Build transaction
        let tx = Transaction::new_with_payer(
            &[claim_ix],
            Some(&wallet_pubkey),
        );
        
        // Serialize for wallet signing
        let serialized = bincode::serialize(&tx)
            .context("Failed to serialize transaction")?;
        let serialized_b64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            &serialized,
        );
        
        info!(
            "Built ClaimSOL tx: wallet={}, gross={:.4} SOL, fee={:.4} SOL, net={:.4} SOL",
            wallet, gross_sol, fee_sol, net_sol
        );
        
        Ok(ClaimTxData {
            serialized_tx: serialized_b64,
            gross_amount: gross_sol,
            fee_amount: fee_sol,
            net_amount: net_sol,
        })
    }
    
    /// Build a transaction to claim ORE from Miner account
    /// Returns transaction for wallet to sign
    /// Note: ORE v3 claims all available at once
    pub async fn build_claim_ore_tx(
        &self,
        wallet: &str,
        _amount: Option<f64>, // Ignored - claims all
    ) -> Result<ClaimTxData> {
        let wallet_pubkey: Pubkey = wallet.parse()
            .context("Invalid wallet address")?;
        
        // Get available balance from Miner account
        let (_rewards_sol, rewards_ore) = self.ore_client.get_unclaimed_balances(&wallet_pubkey).await?;
        
        if rewards_ore == 0 {
            anyhow::bail!("No ORE available to claim");
        }
        
        // Calculate fees (ORE has 11 decimals)
        let gross_ore = rewards_ore as f64 / 100_000_000_000.0; // 11 decimals
        let fee_ore = gross_ore * CLAIM_FEE_PERCENT;
        let net_ore = gross_ore - fee_ore;
        
        // Build claim instruction using ore-api SDK
        let claim_ix = self.ore_client.build_claim_ore_instruction(&wallet_pubkey)?;
        
        // Build transaction
        let tx = Transaction::new_with_payer(
            &[claim_ix],
            Some(&wallet_pubkey),
        );
        
        // Serialize for wallet signing
        let serialized = bincode::serialize(&tx)
            .context("Failed to serialize transaction")?;
        let serialized_b64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            &serialized,
        );
        
        info!(
            "Built ClaimORE tx: wallet={}, gross={:.4} ORE, fee={:.4} ORE, net={:.4} ORE",
            wallet, gross_ore, fee_ore, net_ore
        );
        
        Ok(ClaimTxData {
            serialized_tx: serialized_b64,
            gross_amount: gross_ore,
            fee_amount: fee_ore,
            net_amount: net_ore,
        })
    }
    
    /// Calculate fee preview without building transaction
    pub fn calculate_fee(&self, amount: f64) -> (f64, f64) {
        let fee = amount * CLAIM_FEE_PERCENT;
        let net = amount - fee;
        (fee, net)
    }
    
    /// Get claimable amounts after fee
    pub async fn get_claimable(&self, wallet: &str) -> Result<ClaimableBalances> {
        let wallet_pubkey: Pubkey = wallet.parse()
            .context("Invalid wallet address")?;
        
        let (rewards_sol, rewards_ore) = self.ore_client.get_unclaimed_balances(&wallet_pubkey).await?;
        
        let unclaimed_sol = rewards_sol as f64 / 1_000_000_000.0;
        let unclaimed_ore = rewards_ore as f64 / 100_000_000_000.0; // 11 decimals
        
        Ok(ClaimableBalances {
            sol_gross: unclaimed_sol,
            sol_fee: unclaimed_sol * CLAIM_FEE_PERCENT,
            sol_net: unclaimed_sol * (1.0 - CLAIM_FEE_PERCENT),
            ore_gross: unclaimed_ore,
            ore_fee: unclaimed_ore * CLAIM_FEE_PERCENT,
            ore_net: unclaimed_ore * (1.0 - CLAIM_FEE_PERCENT),
        })
    }
}

/// Claimable balance breakdown
#[derive(Debug, Clone)]
pub struct ClaimableBalances {
    pub sol_gross: f64,
    pub sol_fee: f64,
    pub sol_net: f64,
    pub ore_gross: f64,
    pub ore_fee: f64,
    pub ore_net: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_fee_calculation() {
        // Simple fee calculation test
        let fee_percent = CLAIM_FEE_PERCENT;
        let amount = 1.0;
        let fee = amount * fee_percent;
        let net = amount - fee;
        
        assert!((fee - 0.1).abs() < 0.0001, "Fee should be 10%");
        assert!((net - 0.9).abs() < 0.0001, "Net should be 90%");
    }
}
