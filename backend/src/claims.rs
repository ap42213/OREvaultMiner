//! Claims Processor
//! 
//! Handles claiming SOL and ORE from the on-chain ORE account to wallet.
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
    
    /// Build a transaction to claim SOL from ORE account
    /// Returns transaction for wallet to sign
    pub async fn build_claim_sol_tx(
        &self,
        wallet: &str,
        amount: Option<f64>,
    ) -> Result<ClaimTxData> {
        let wallet_pubkey: Pubkey = wallet.parse()
            .context("Invalid wallet address")?;
        
        // Get available balance
        let ore_balance = self.ore_client.get_ore_account_balance(&wallet_pubkey).await?;
        let available_lamports = ore_balance.unclaimed_sol;
        
        if available_lamports == 0 {
            anyhow::bail!("No SOL available to claim");
        }
        
        // Determine claim amount
        let claim_lamports = match amount {
            Some(sol) => {
                let lamports = (sol * 1_000_000_000.0) as u64;
                if lamports > available_lamports {
                    anyhow::bail!(
                        "Requested {} SOL but only {} SOL available",
                        sol,
                        available_lamports as f64 / 1_000_000_000.0
                    );
                }
                lamports
            }
            None => available_lamports, // Claim all
        };
        
        // Calculate fees
        let gross_sol = claim_lamports as f64 / 1_000_000_000.0;
        let fee_sol = gross_sol * CLAIM_FEE_PERCENT;
        let net_sol = gross_sol - fee_sol;
        
        // Build claim instruction
        let claim_ix = self.ore_client.build_claim_sol_instruction(
            &wallet_pubkey,
            Some(claim_lamports),
        )?;
        
        // Get recent blockhash
        let blockhash = self.ore_client.get_latest_blockhash().await?;
        
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
    
    /// Build a transaction to claim ORE from ORE account
    /// Returns transaction for wallet to sign
    pub async fn build_claim_ore_tx(
        &self,
        wallet: &str,
        amount: Option<f64>,
    ) -> Result<ClaimTxData> {
        let wallet_pubkey: Pubkey = wallet.parse()
            .context("Invalid wallet address")?;
        
        // Get available balance
        let ore_balance = self.ore_client.get_ore_account_balance(&wallet_pubkey).await?;
        let available_ore = ore_balance.unclaimed_ore;
        
        if available_ore == 0 {
            anyhow::bail!("No ORE available to claim");
        }
        
        // Determine claim amount (ORE has 9 decimals like SOL)
        let claim_amount = match amount {
            Some(ore) => {
                let base_units = (ore * 1_000_000_000.0) as u64;
                if base_units > available_ore {
                    anyhow::bail!(
                        "Requested {} ORE but only {} ORE available",
                        ore,
                        available_ore as f64 / 1_000_000_000.0
                    );
                }
                base_units
            }
            None => available_ore, // Claim all
        };
        
        // Calculate fees
        let gross_ore = claim_amount as f64 / 1_000_000_000.0;
        let fee_ore = gross_ore * CLAIM_FEE_PERCENT;
        let net_ore = gross_ore - fee_ore;
        
        // Build claim instruction
        let claim_ix = self.ore_client.build_claim_ore_instruction(
            &wallet_pubkey,
            Some(claim_amount),
        )?;
        
        // Get recent blockhash
        let blockhash = self.ore_client.get_latest_blockhash().await?;
        
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
        
        let ore_balance = self.ore_client.get_ore_account_balance(&wallet_pubkey).await?;
        
        let unclaimed_sol = ore_balance.unclaimed_sol as f64 / 1_000_000_000.0;
        let unclaimed_ore = ore_balance.unclaimed_ore as f64 / 1_000_000_000.0;
        
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
        let processor = ClaimsProcessor {
            ore_client: unsafe { std::mem::zeroed() }, // Just for testing fee calc
        };
        
        let (fee, net) = processor.calculate_fee(1.0);
        assert!((fee - 0.1).abs() < 0.0001, "Fee should be 10%");
        assert!((net - 0.9).abs() < 0.0001, "Net should be 90%");
        
        let (fee, net) = processor.calculate_fee(10.0);
        assert!((fee - 1.0).abs() < 0.0001, "Fee should be 1.0 SOL");
        assert!((net - 9.0).abs() < 0.0001, "Net should be 9.0 SOL");
    }
}
