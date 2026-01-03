//! Jito Bundle Submission Client
//! 
//! Handles bundle submission to Jito block engine for MEV-protected transactions.
//! Uses Jito's JSON-RPC API for bundle submission.
//! Block Engine: ny.mainnet.block-engine.jito.wtf

use std::time::Duration;

use anyhow::{Result, Context};
use solana_sdk::{
    instruction::Instruction,
    pubkey::Pubkey,
    signature::Signature,
    transaction::Transaction,
    system_instruction,
};
use tracing::{debug, info, warn, error};

/// Jito tip account addresses (rotate for load balancing)
const JITO_TIP_ACCOUNTS: [&str; 8] = [
    "96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5",
    "HFqU5x63VTqvQss8hp11i4wVV8bD44PvwucfZ2bU7gRe",
    "Cw8CFyM9FkoMi7K7Crf6HNQqf4uEMzpKw6QNghXLvLkY",
    "ADaUMid9yfUytqMBgopwjb2DTLSokTSzL1zt6iGPaS49",
    "DfXygSm4jCyNCybVYYK6DwvWqjKee8pbDmJGcLWNDXjh",
    "ADuUkR4vqLUMWXxW9gh6D6L8pMSawimctcNZ5pGwDcEt",
    "DttWaMuVvTiduZRnguLF7jNxTgiMBZ1hyAumKUiL2KRL",
    "3AVi9Tg9Uo68tJfuvoKvqKNWKkC5wPdSSdeBnizKZ6jT",
];

/// Jito Block Engine RPC endpoints
const JITO_MAINNET_RPC: &str = "https://mainnet.block-engine.jito.wtf/api/v1/bundles";

/// Bundle status returned by Jito
#[derive(Debug, Clone)]
pub enum BundleStatus {
    Pending,
    Landed { slot: u64 },
    Failed { reason: String },
    Dropped,
}

/// Result of bundle submission
#[derive(Debug, Clone)]
pub struct BundleResult {
    pub bundle_id: String,
    pub status: BundleStatus,
    pub tip_amount: u64,
    pub signatures: Vec<Signature>,
}

/// Jito client for bundle submission
#[derive(Clone)]
pub struct JitoClient {
    block_engine_url: String,
}

impl JitoClient {
    /// Create a new Jito client
    pub async fn new(block_engine_url: &str) -> Result<Self> {
        let url = if block_engine_url.contains("block-engine") {
            format!("https://{}/api/v1/bundles", block_engine_url.trim_start_matches("https://").trim_start_matches("http://"))
        } else {
            JITO_MAINNET_RPC.to_string()
        };
        
        info!("Initializing Jito client for: {}", url);
        
        Ok(Self {
            block_engine_url: url,
        })
    }
    
    /// Get a random tip account for load balancing
    pub fn get_tip_account(&self) -> Pubkey {
        use rand::Rng;
        let idx = rand::thread_rng().gen_range(0..JITO_TIP_ACCOUNTS.len());
        JITO_TIP_ACCOUNTS[idx].parse().expect("Invalid tip account")
    }
    
    /// Build a tip instruction
    pub fn build_tip_instruction(
        &self,
        payer: &Pubkey,
        tip_amount: u64,
    ) -> Instruction {
        let tip_account = self.get_tip_account();
        system_instruction::transfer(payer, &tip_account, tip_amount)
    }
    
    /// Build a transaction bundle with tip
    pub fn build_bundle(
        &self,
        instructions: Vec<Instruction>,
        payer: &Pubkey,
        tip_amount: u64,
        recent_blockhash: solana_sdk::hash::Hash,
    ) -> Result<Transaction> {
        // Add tip instruction at the end
        let mut all_instructions = instructions;
        all_instructions.push(self.build_tip_instruction(payer, tip_amount));
        
        // Build transaction (will need to be signed by wallet)
        let tx = Transaction::new_with_payer(&all_instructions, Some(payer));
        
        Ok(tx)
    }
    
    /// Submit a bundle to Jito via JSON-RPC
    pub async fn send_bundle(
        &self,
        transactions: Vec<Transaction>,
    ) -> Result<BundleResult> {
        // Serialize transactions to base58 (Jito expects base58-encoded serialized tx bytes)
        let serialized_txs: Vec<String> = transactions.iter()
            .map(|tx| {
                let bytes = bincode::serialize(tx).unwrap();
                bs58::encode(&bytes).into_string()
            })
            .collect();
        
        // Collect signatures
        let signatures: Vec<Signature> = transactions.iter()
            .flat_map(|tx| tx.signatures.clone())
            .collect();
        
        // Generate bundle ID
        let bundle_id = format!("bundle_{}", uuid::Uuid::new_v4());
        
        info!(
            "Submitting bundle {} with {} transaction(s) to Jito",
            bundle_id,
            transactions.len()
        );
        
        // Build JSON-RPC request
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "sendBundle",
            "params": [serialized_txs]
        });
        
        // Create HTTP client
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;
        
        // Submit via HTTP
        match client
            .post(&self.block_engine_url)
            .json(&request)
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    let result: serde_json::Value = response.json().await
                        .unwrap_or_else(|_| serde_json::json!({}));
                    
                    if let Some(error) = result.get("error") {
                        error!("Bundle {} failed: {:?}", bundle_id, error);
                        Ok(BundleResult {
                            bundle_id,
                            status: BundleStatus::Failed { 
                                reason: error.to_string() 
                            },
                            tip_amount: 0,
                            signatures,
                        })
                    } else {
                        info!("Bundle {} submitted successfully", bundle_id);
                        Ok(BundleResult {
                            bundle_id,
                            status: BundleStatus::Pending,
                            tip_amount: self.extract_tip_amount(&transactions),
                            signatures,
                        })
                    }
                } else {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    error!("Bundle {} HTTP error {}: {}", bundle_id, status, body);
                    Ok(BundleResult {
                        bundle_id,
                        status: BundleStatus::Failed { 
                            reason: format!("HTTP {}: {}", status, body) 
                        },
                        tip_amount: 0,
                        signatures,
                    })
                }
            }
            Err(e) => {
                error!("Bundle {} network error: {}", bundle_id, e);
                Ok(BundleResult {
                    bundle_id,
                    status: BundleStatus::Failed { reason: e.to_string() },
                    tip_amount: 0,
                    signatures,
                })
            }
        }
    }
    
    /// Submit a single transaction as a bundle
    pub async fn send_bundle_single(&self, tx: Transaction) -> Result<BundleResult> {
        self.send_bundle(vec![tx]).await
    }
    
    /// Extract total tip amount from transactions
    fn extract_tip_amount(&self, transactions: &[Transaction]) -> u64 {
        let tip_accounts: std::collections::HashSet<Pubkey> = JITO_TIP_ACCOUNTS
            .iter()
            .map(|s| s.parse().unwrap())
            .collect();
        
        let mut total_tip = 0u64;
        
        for tx in transactions {
            for ix in &tx.message.instructions {
                if tx.message.account_keys.get(ix.program_id_index as usize)
                    .map(|k| *k == solana_sdk::system_program::id())
                    .unwrap_or(false)
                {
                    if ix.data.len() >= 12 && ix.data[0..4] == [2, 0, 0, 0] {
                        let lamports = u64::from_le_bytes(ix.data[4..12].try_into().unwrap_or([0; 8]));
                        if let Some(dest_idx) = ix.accounts.get(1) {
                            if let Some(dest) = tx.message.account_keys.get(*dest_idx as usize) {
                                if tip_accounts.contains(dest) {
                                    total_tip += lamports;
                                }
                            }
                        }
                    }
                }
            }
        }
        
        total_tip
    }
    
    /// Get bundle status (placeholder - would query Jito API)
    pub async fn get_bundle_status(&self, _bundle_id: &str) -> Result<BundleStatus> {
        Ok(BundleStatus::Pending)
    }
    
    /// Wait for bundle confirmation with timeout
    pub async fn wait_for_confirmation(
        &self,
        bundle_id: &str,
        timeout_secs: u64,
    ) -> Result<BundleStatus> {
        let result = tokio::time::timeout(
            Duration::from_secs(timeout_secs),
            self.poll_bundle_status(bundle_id),
        ).await;
        
        match result {
            Ok(Ok(status)) => Ok(status),
            Ok(Err(e)) => Err(e),
            Err(_) => Ok(BundleStatus::Dropped),
        }
    }
    
    /// Poll bundle status until confirmed or failed
    async fn poll_bundle_status(&self, bundle_id: &str) -> Result<BundleStatus> {
        loop {
            let status = self.get_bundle_status(bundle_id).await?;
            match status {
                BundleStatus::Pending => {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                _ => return Ok(status),
            }
        }
    }
    
    /// Calculate recommended tip based on recent bundles
    pub async fn get_recommended_tip(&self) -> Result<u64> {
        // Default tip: 0.001 SOL = 1_000_000 lamports
        Ok(1_000_000)
    }
    
    /// Get current tip floor from Jito
    pub async fn get_tip_floor(&self) -> Result<u64> {
        // Minimum tip: 0.0005 SOL
        Ok(500_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tip_accounts_valid() {
        for account in JITO_TIP_ACCOUNTS {
            assert!(account.parse::<Pubkey>().is_ok(), "Invalid tip account: {}", account);
        }
    }
}
