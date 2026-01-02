//! ORE v3 Program Client
//! 
//! Handles all interactions with the ORE v3 program on Solana mainnet.
//! Program ID: oreV3EG1i9BEgiAJ8b177Z2S2rMarzak4NMv1kULvWv

use std::str::FromStr;

use anyhow::{Result, Context};
use solana_client::rpc_client::RpcClient;
use solana_client::nonblocking::rpc_client::RpcClient as AsyncRpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Signature,
    transaction::Transaction,
    system_program,
    sysvar,
};
use tracing::{debug, info, warn};

/// ORE v3 Program ID on Mainnet
pub const ORE_PROGRAM_ID: &str = "oreV3EG1i9BEgiAJ8b177Z2S2rMarzak4NMv1kULvWv";

/// ORE v3 Instruction discriminators
#[repr(u8)]
pub enum OreInstruction {
    Deploy = 0,
    Reset = 1,
    ClaimOre = 2,
    ClaimSol = 3,
    Checkpoint = 4,
}

/// Block data from ORE grid (5x5 = 25 blocks)
#[derive(Debug, Clone)]
pub struct BlockData {
    pub index: u8,
    pub total_deployed: u64,
    pub deployers: Vec<DeployerData>,
}

#[derive(Debug, Clone)]
pub struct DeployerData {
    pub wallet: Pubkey,
    pub amount: u64,
}

/// Round state from on-chain
#[derive(Debug, Clone)]
pub struct RoundState {
    pub round_id: u64,
    pub start_time: i64,
    pub end_time: i64,
    pub total_pot: u64,
    pub blocks: [BlockData; 25],
}

/// User's ORE account balances (on-chain, unclaimed)
#[derive(Debug, Clone, Default)]
pub struct OreAccountBalance {
    pub unclaimed_sol: u64,   // lamports
    pub unclaimed_ore: u64,   // base units
    pub refined_ore: u64,     // base units
}

/// ORE v3 client for interacting with the program
#[derive(Clone)]
pub struct OreClient {
    rpc: AsyncRpcClient,
    program_id: Pubkey,
}

impl OreClient {
    /// Create a new ORE client
    pub fn new(rpc_url: &str) -> Result<Self> {
        let rpc = AsyncRpcClient::new_with_commitment(
            rpc_url.to_string(),
            CommitmentConfig::confirmed(),
        );
        
        let program_id = Pubkey::from_str(ORE_PROGRAM_ID)
            .context("Invalid ORE program ID")?;
        
        info!("ORE Client initialized for program: {}", program_id);
        
        Ok(Self { rpc, program_id })
    }
    
    /// Get the ORE program ID
    pub fn program_id(&self) -> &Pubkey {
        &self.program_id
    }
    
    /// Get current round state from on-chain
    pub async fn get_round_state(&self) -> Result<RoundState> {
        // Derive the round PDA
        let (round_pda, _bump) = Pubkey::find_program_address(
            &[b"round"],
            &self.program_id,
        );
        
        let account = self.rpc.get_account(&round_pda).await
            .context("Failed to fetch round account")?;
        
        // Parse round account data
        // The actual parsing depends on ore-api's account structure
        let round_state = self.parse_round_account(&account.data)?;
        
        Ok(round_state)
    }
    
    /// Get all 25 blocks for current round
    pub async fn get_all_blocks(&self) -> Result<[BlockData; 25]> {
        let round = self.get_round_state().await?;
        Ok(round.blocks)
    }
    
    /// Get specific block data
    pub async fn get_block(&self, index: u8) -> Result<BlockData> {
        if index >= 25 {
            anyhow::bail!("Block index must be 0-24, got {}", index);
        }
        
        let round = self.get_round_state().await?;
        Ok(round.blocks[index as usize].clone())
    }
    
    /// Get user's ORE account balance (unclaimed rewards)
    pub async fn get_ore_account_balance(&self, wallet: &Pubkey) -> Result<OreAccountBalance> {
        // Derive user's ORE account PDA
        let (user_account_pda, _bump) = Pubkey::find_program_address(
            &[b"user", wallet.as_ref()],
            &self.program_id,
        );
        
        match self.rpc.get_account(&user_account_pda).await {
            Ok(account) => {
                let balance = self.parse_user_account(&account.data)?;
                Ok(balance)
            }
            Err(_) => {
                // Account doesn't exist yet - user hasn't played
                Ok(OreAccountBalance::default())
            }
        }
    }
    
    /// Get wallet SOL balance
    pub async fn get_sol_balance(&self, wallet: &Pubkey) -> Result<u64> {
        let balance = self.rpc.get_balance(wallet).await
            .context("Failed to fetch SOL balance")?;
        Ok(balance)
    }
    
    /// Get wallet ORE token balance
    pub async fn get_ore_token_balance(&self, wallet: &Pubkey) -> Result<u64> {
        let ore_mint = self.get_ore_mint();
        let ata = spl_associated_token_account::get_associated_token_address(
            wallet,
            &ore_mint,
        );
        
        match self.rpc.get_token_account_balance(&ata).await {
            Ok(balance) => {
                let amount = balance.amount.parse::<u64>()
                    .unwrap_or(0);
                Ok(amount)
            }
            Err(_) => Ok(0),
        }
    }
    
    /// Get ORE token mint address
    pub fn get_ore_mint(&self) -> Pubkey {
        // ORE token mint - this should be fetched from ore-api
        // For now using a placeholder that would need to be verified
        Pubkey::find_program_address(
            &[b"mint"],
            &self.program_id,
        ).0
    }
    
    /// Build Deploy instruction
    pub fn build_deploy_instruction(
        &self,
        wallet: &Pubkey,
        block_index: u8,
        amount: u64,
    ) -> Result<Instruction> {
        if block_index >= 25 {
            anyhow::bail!("Block index must be 0-24");
        }
        
        let (round_pda, _) = Pubkey::find_program_address(
            &[b"round"],
            &self.program_id,
        );
        
        let (block_pda, _) = Pubkey::find_program_address(
            &[b"block", &[block_index]],
            &self.program_id,
        );
        
        let (user_account_pda, _) = Pubkey::find_program_address(
            &[b"user", wallet.as_ref()],
            &self.program_id,
        );
        
        // Instruction data: [discriminator, block_index, amount (8 bytes)]
        let mut data = vec![OreInstruction::Deploy as u8, block_index];
        data.extend_from_slice(&amount.to_le_bytes());
        
        let accounts = vec![
            AccountMeta::new(*wallet, true),           // payer/signer
            AccountMeta::new(round_pda, false),        // round account
            AccountMeta::new(block_pda, false),        // block account
            AccountMeta::new(user_account_pda, false), // user account
            AccountMeta::new_readonly(system_program::id(), false),
        ];
        
        Ok(Instruction {
            program_id: self.program_id,
            accounts,
            data,
        })
    }
    
    /// Build ClaimSOL instruction (10% fee applied on-chain)
    pub fn build_claim_sol_instruction(
        &self,
        wallet: &Pubkey,
        amount: Option<u64>, // None = claim all
    ) -> Result<Instruction> {
        let (user_account_pda, _) = Pubkey::find_program_address(
            &[b"user", wallet.as_ref()],
            &self.program_id,
        );
        
        let (treasury_pda, _) = Pubkey::find_program_address(
            &[b"treasury"],
            &self.program_id,
        );
        
        // Instruction data: [discriminator, amount (optional)]
        let mut data = vec![OreInstruction::ClaimSol as u8];
        if let Some(amt) = amount {
            data.extend_from_slice(&amt.to_le_bytes());
        }
        
        let accounts = vec![
            AccountMeta::new(*wallet, true),           // wallet/signer
            AccountMeta::new(user_account_pda, false), // user account
            AccountMeta::new(treasury_pda, false),     // treasury (for fee)
            AccountMeta::new_readonly(system_program::id(), false),
        ];
        
        Ok(Instruction {
            program_id: self.program_id,
            accounts,
            data,
        })
    }
    
    /// Build ClaimORE instruction (10% fee applied on-chain)
    pub fn build_claim_ore_instruction(
        &self,
        wallet: &Pubkey,
        amount: Option<u64>, // None = claim all
    ) -> Result<Instruction> {
        let ore_mint = self.get_ore_mint();
        
        let (user_account_pda, _) = Pubkey::find_program_address(
            &[b"user", wallet.as_ref()],
            &self.program_id,
        );
        
        let (treasury_pda, _) = Pubkey::find_program_address(
            &[b"treasury"],
            &self.program_id,
        );
        
        let wallet_ata = spl_associated_token_account::get_associated_token_address(
            wallet,
            &ore_mint,
        );
        
        let treasury_ata = spl_associated_token_account::get_associated_token_address(
            &treasury_pda,
            &ore_mint,
        );
        
        // Instruction data: [discriminator, amount (optional)]
        let mut data = vec![OreInstruction::ClaimOre as u8];
        if let Some(amt) = amount {
            data.extend_from_slice(&amt.to_le_bytes());
        }
        
        let accounts = vec![
            AccountMeta::new(*wallet, true),           // wallet/signer
            AccountMeta::new(user_account_pda, false), // user account
            AccountMeta::new(ore_mint, false),         // ore mint
            AccountMeta::new(wallet_ata, false),       // wallet token account
            AccountMeta::new(treasury_ata, false),     // treasury token account (for fee)
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(spl_associated_token_account::id(), false),
            AccountMeta::new_readonly(system_program::id(), false),
        ];
        
        Ok(Instruction {
            program_id: self.program_id,
            accounts,
            data,
        })
    }
    
    /// Build Checkpoint instruction
    pub fn build_checkpoint_instruction(
        &self,
        wallet: &Pubkey,
    ) -> Result<Instruction> {
        let (user_account_pda, _) = Pubkey::find_program_address(
            &[b"user", wallet.as_ref()],
            &self.program_id,
        );
        
        let (round_pda, _) = Pubkey::find_program_address(
            &[b"round"],
            &self.program_id,
        );
        
        let data = vec![OreInstruction::Checkpoint as u8];
        
        let accounts = vec![
            AccountMeta::new(*wallet, true),
            AccountMeta::new(user_account_pda, false),
            AccountMeta::new_readonly(round_pda, false),
        ];
        
        Ok(Instruction {
            program_id: self.program_id,
            accounts,
            data,
        })
    }
    
    /// Get time remaining in current round (seconds)
    pub async fn get_time_remaining(&self) -> Result<f64> {
        let round = self.get_round_state().await?;
        let now = chrono::Utc::now().timestamp();
        let remaining = (round.end_time - now) as f64;
        Ok(remaining.max(0.0))
    }
    
    /// Check if we're in the submission window (T-2.0s to T-0.0s)
    pub async fn in_submission_window(&self) -> Result<bool> {
        let remaining = self.get_time_remaining().await?;
        Ok(remaining <= 2.0 && remaining > 0.0)
    }
    
    // Private parsing methods
    
    fn parse_round_account(&self, data: &[u8]) -> Result<RoundState> {
        // This would use ore-api types for proper deserialization
        // Placeholder implementation - actual implementation depends on ore-api structure
        
        if data.len() < 32 {
            anyhow::bail!("Round account data too short");
        }
        
        // Parse header
        let round_id = u64::from_le_bytes(data[0..8].try_into()?);
        let start_time = i64::from_le_bytes(data[8..16].try_into()?);
        let end_time = i64::from_le_bytes(data[16..24].try_into()?);
        let total_pot = u64::from_le_bytes(data[24..32].try_into()?);
        
        // Initialize empty blocks
        let blocks: [BlockData; 25] = std::array::from_fn(|i| BlockData {
            index: i as u8,
            total_deployed: 0,
            deployers: vec![],
        });
        
        // TODO: Parse actual block data from account
        
        Ok(RoundState {
            round_id,
            start_time,
            end_time,
            total_pot,
            blocks,
        })
    }
    
    fn parse_user_account(&self, data: &[u8]) -> Result<OreAccountBalance> {
        // Parse user account data structure
        // This depends on ore-api's actual account layout
        
        if data.len() < 24 {
            return Ok(OreAccountBalance::default());
        }
        
        let unclaimed_sol = u64::from_le_bytes(data[0..8].try_into()?);
        let unclaimed_ore = u64::from_le_bytes(data[8..16].try_into()?);
        let refined_ore = u64::from_le_bytes(data[16..24].try_into()?);
        
        Ok(OreAccountBalance {
            unclaimed_sol,
            unclaimed_ore,
            refined_ore,
        })
    }
    
    /// Get the RPC client for direct access
    pub fn rpc(&self) -> &AsyncRpcClient {
        &self.rpc
    }
    
    /// Get latest blockhash
    pub async fn get_latest_blockhash(&self) -> Result<solana_sdk::hash::Hash> {
        let blockhash = self.rpc.get_latest_blockhash().await
            .context("Failed to get latest blockhash")?;
        Ok(blockhash)
    }
    
    /// Send and confirm transaction
    pub async fn send_transaction(&self, tx: &Transaction) -> Result<Signature> {
        let sig = self.rpc.send_and_confirm_transaction(tx).await
            .context("Failed to send transaction")?;
        Ok(sig)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_program_id() {
        let pubkey = Pubkey::from_str(ORE_PROGRAM_ID).unwrap();
        assert_eq!(pubkey.to_string(), ORE_PROGRAM_ID);
    }
}
