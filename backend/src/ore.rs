//! ORE v3 Program Client
//! 
//! Handles all interactions with the ORE v3 program on Solana mainnet.
//! Using ore-api crate for correct PDAs and account structures.

use std::sync::Arc;

use anyhow::{Result, Context};
use ore_api::state::{board_pda, round_pda, miner_pda, treasury_pda};
use solana_client::nonblocking::rpc_client::RpcClient as AsyncRpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::Signature,
    transaction::Transaction,
};
use tracing::{debug, info, warn};

/// ORE v3 Program ID on Mainnet
pub const ORE_PROGRAM_ID: Pubkey = ore_api::ID;

/// Block data from ORE grid (5x5 = 25 squares)
#[derive(Debug, Clone)]
pub struct BlockData {
    pub index: u8,
    pub total_deployed: u64,
    pub miner_count: u64,
}

/// Round state from on-chain
#[derive(Debug, Clone)]
pub struct RoundState {
    pub round_id: u64,
    pub start_slot: u64,
    pub end_slot: u64,
    pub expires_at: u64,
    pub total_deployed: u64,
    pub total_vaulted: u64,
    pub total_winnings: u64,
    pub total_miners: u64,
    pub motherlode: u64,
    pub top_miner: Pubkey,
    pub blocks: [BlockData; 25],
    pub slot_hash: [u8; 32],
}

/// Board state (current round info)
#[derive(Debug, Clone)]
pub struct BoardState {
    pub round_id: u64,
    pub start_slot: u64,
    pub end_slot: u64,
    pub epoch_id: u64,
}

/// User's Miner account data
#[derive(Debug, Clone, Default)]
pub struct MinerData {
    pub authority: Pubkey,
    pub deployed: [u64; 25],
    pub cumulative: [u64; 25],
    pub checkpoint_fee: u64,
    pub checkpoint_id: u64,
    pub rewards_sol: u64,
    pub rewards_ore: u64,
    pub refined_ore: u64,
    pub round_id: u64,
    pub lifetime_rewards_sol: u64,
    pub lifetime_rewards_ore: u64,
    pub lifetime_deployed: u64,
}

/// ORE v3 client for interacting with the program
#[derive(Clone)]
pub struct OreClient {
    rpc: Arc<AsyncRpcClient>,
}

impl OreClient {
    /// Create a new ORE client
    pub fn new(rpc_url: &str) -> Result<Self> {
        let rpc = Arc::new(AsyncRpcClient::new_with_commitment(
            rpc_url.to_string(),
            CommitmentConfig::confirmed(),
        ));
        
        info!("ORE Client initialized for program: {}", ORE_PROGRAM_ID);
        
        Ok(Self { rpc })
    }
    
    /// Get the ORE program ID
    pub fn program_id(&self) -> Pubkey {
        ORE_PROGRAM_ID
    }
    
    /// Get current board state (tells us current round_id)
    pub async fn get_board_state(&self) -> Result<BoardState> {
        let (board_address, _) = board_pda();
        
        let account = self.rpc.get_account(&board_address).await
            .context("Failed to fetch board account")?;
        
        // Parse board account - ore-api uses first 8 bytes as discriminator
        let data = &account.data;
        if data.len() < 8 + 32 {
            anyhow::bail!("Board account data too short: {} bytes", data.len());
        }
        
        // Skip 8-byte discriminator
        let board_data = &data[8..];
        
        let round_id = u64::from_le_bytes(board_data[0..8].try_into()?);
        let start_slot = u64::from_le_bytes(board_data[8..16].try_into()?);
        let end_slot = u64::from_le_bytes(board_data[16..24].try_into()?);
        let epoch_id = u64::from_le_bytes(board_data[24..32].try_into()?);
        
        debug!("Board state: round_id={}, start_slot={}, end_slot={}", round_id, start_slot, end_slot);
        
        Ok(BoardState {
            round_id,
            start_slot,
            end_slot,
            epoch_id,
        })
    }
    
    /// Get round state for a specific round ID
    pub async fn get_round_state(&self, round_id: u64) -> Result<RoundState> {
        let (round_address, _) = round_pda(round_id);
        
        let account = self.rpc.get_account(&round_address).await
            .context(format!("Failed to fetch round {} account", round_id))?;
        
        let data = &account.data;
        if data.len() < 8 {
            anyhow::bail!("Round account data too short");
        }
        
        // Skip 8-byte discriminator
        let round_data = &data[8..];
        
        // Parse Round struct fields based on ore-api/src/state/round.rs
        let mut offset = 0;
        
        // id: u64
        let id = u64::from_le_bytes(round_data[offset..offset+8].try_into()?);
        offset += 8;
        
        // deployed: [u64; 25]
        let mut deployed = [0u64; 25];
        for i in 0..25 {
            deployed[i] = u64::from_le_bytes(round_data[offset..offset+8].try_into()?);
            offset += 8;
        }
        
        // slot_hash: [u8; 32]
        let mut slot_hash = [0u8; 32];
        slot_hash.copy_from_slice(&round_data[offset..offset+32]);
        offset += 32;
        
        // count: [u64; 25]
        let mut count = [0u64; 25];
        for i in 0..25 {
            count[i] = u64::from_le_bytes(round_data[offset..offset+8].try_into()?);
            offset += 8;
        }
        
        // expires_at: u64
        let expires_at = u64::from_le_bytes(round_data[offset..offset+8].try_into()?);
        offset += 8;
        
        // motherlode: u64
        let motherlode = u64::from_le_bytes(round_data[offset..offset+8].try_into()?);
        offset += 8;
        
        // rent_payer: Pubkey (32 bytes)
        offset += 32;
        
        // top_miner: Pubkey
        let top_miner = Pubkey::try_from(&round_data[offset..offset+32])?;
        offset += 32;
        
        // top_miner_reward: u64
        offset += 8;
        
        // total_deployed: u64
        let total_deployed = u64::from_le_bytes(round_data[offset..offset+8].try_into()?);
        offset += 8;
        
        // total_miners: u64
        let total_miners = u64::from_le_bytes(round_data[offset..offset+8].try_into()?);
        offset += 8;
        
        // total_vaulted: u64
        let total_vaulted = u64::from_le_bytes(round_data[offset..offset+8].try_into()?);
        offset += 8;
        
        // total_winnings: u64
        let total_winnings = u64::from_le_bytes(round_data[offset..offset+8].try_into()?);
        
        // Build blocks array
        let blocks: [BlockData; 25] = std::array::from_fn(|i| BlockData {
            index: i as u8,
            total_deployed: deployed[i],
            miner_count: count[i],
        });
        
        debug!("Round {} state: total_deployed={}, total_miners={}", id, total_deployed, total_miners);
        
        Ok(RoundState {
            round_id: id,
            start_slot: 0, // Get from board
            end_slot: 0,   // Get from board
            expires_at,
            total_deployed,
            total_vaulted,
            total_winnings,
            total_miners,
            motherlode,
            top_miner,
            blocks,
            slot_hash,
        })
    }
    
    /// Get current round state (fetches board first to get round_id)
    pub async fn get_current_round_state(&self) -> Result<RoundState> {
        let board = self.get_board_state().await?;
        let mut round = self.get_round_state(board.round_id).await?;
        round.start_slot = board.start_slot;
        round.end_slot = board.end_slot;
        Ok(round)
    }
    
    /// Get all 25 blocks for current round
    pub async fn get_all_blocks(&self) -> Result<[BlockData; 25]> {
        let round = self.get_current_round_state().await?;
        Ok(round.blocks)
    }
    
    /// Get specific block data
    pub async fn get_block(&self, index: u8) -> Result<BlockData> {
        if index >= 25 {
            anyhow::bail!("Block index must be 0-24, got {}", index);
        }
        
        let round = self.get_current_round_state().await?;
        Ok(round.blocks[index as usize].clone())
    }
    
    /// Get user's Miner account data
    pub async fn get_miner_data(&self, wallet: &Pubkey) -> Result<Option<MinerData>> {
        let (miner_address, _) = miner_pda(*wallet);
        
        match self.rpc.get_account(&miner_address).await {
            Ok(account) => {
                let data = &account.data;
                if data.len() < 8 {
                    return Ok(None);
                }
                
                // Skip 8-byte discriminator
                let miner_data = &data[8..];
                let mut offset = 0;
                
                // authority: Pubkey
                let authority = Pubkey::try_from(&miner_data[offset..offset+32])?;
                offset += 32;
                
                // deployed: [u64; 25]
                let mut deployed = [0u64; 25];
                for i in 0..25 {
                    deployed[i] = u64::from_le_bytes(miner_data[offset..offset+8].try_into()?);
                    offset += 8;
                }
                
                // cumulative: [u64; 25]
                let mut cumulative = [0u64; 25];
                for i in 0..25 {
                    cumulative[i] = u64::from_le_bytes(miner_data[offset..offset+8].try_into()?);
                    offset += 8;
                }
                
                // checkpoint_fee: u64
                let checkpoint_fee = u64::from_le_bytes(miner_data[offset..offset+8].try_into()?);
                offset += 8;
                
                // checkpoint_id: u64
                let checkpoint_id = u64::from_le_bytes(miner_data[offset..offset+8].try_into()?);
                offset += 8;
                
                // last_claim_ore_at: i64
                offset += 8;
                
                // last_claim_sol_at: i64
                offset += 8;
                
                // rewards_factor: Numeric (16 bytes)
                offset += 16;
                
                // rewards_sol: u64
                let rewards_sol = u64::from_le_bytes(miner_data[offset..offset+8].try_into()?);
                offset += 8;
                
                // rewards_ore: u64
                let rewards_ore = u64::from_le_bytes(miner_data[offset..offset+8].try_into()?);
                offset += 8;
                
                // refined_ore: u64
                let refined_ore = u64::from_le_bytes(miner_data[offset..offset+8].try_into()?);
                offset += 8;
                
                // round_id: u64
                let round_id = u64::from_le_bytes(miner_data[offset..offset+8].try_into()?);
                offset += 8;
                
                // lifetime_rewards_sol: u64
                let lifetime_rewards_sol = u64::from_le_bytes(miner_data[offset..offset+8].try_into()?);
                offset += 8;
                
                // lifetime_rewards_ore: u64
                let lifetime_rewards_ore = u64::from_le_bytes(miner_data[offset..offset+8].try_into()?);
                offset += 8;
                
                // lifetime_deployed: u64
                let lifetime_deployed = u64::from_le_bytes(miner_data[offset..offset+8].try_into()?);
                
                Ok(Some(MinerData {
                    authority,
                    deployed,
                    cumulative,
                    checkpoint_fee,
                    checkpoint_id,
                    rewards_sol,
                    rewards_ore,
                    refined_ore,
                    round_id,
                    lifetime_rewards_sol,
                    lifetime_rewards_ore,
                    lifetime_deployed,
                }))
            }
            Err(_) => Ok(None), // Account doesn't exist
        }
    }
    
    /// Get user's unclaimed balances from Miner account
    pub async fn get_unclaimed_balances(&self, wallet: &Pubkey) -> Result<(u64, u64)> {
        match self.get_miner_data(wallet).await? {
            Some(miner) => Ok((miner.rewards_sol, miner.rewards_ore)),
            None => Ok((0, 0)),
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
        let ore_mint = ore_api::consts::MINT_ADDRESS;
        let ata = spl_associated_token_account::get_associated_token_address(
            wallet,
            &ore_mint,
        );
        
        match self.rpc.get_token_account_balance(&ata).await {
            Ok(balance) => {
                let amount = balance.amount.parse::<u64>().unwrap_or(0);
                Ok(amount)
            }
            Err(_) => Ok(0),
        }
    }
    
    /// Build Deploy instruction using ore-api SDK
    pub fn build_deploy_instruction(
        &self,
        signer: &Pubkey,
        authority: &Pubkey,
        amount: u64,
        round_id: u64,
        squares: [bool; 25],
    ) -> Result<solana_sdk::instruction::Instruction> {
        Ok(ore_api::sdk::deploy(*signer, *authority, amount, round_id, squares))
    }
    
    /// Build ClaimSol instruction using ore-api SDK
    pub fn build_claim_sol_instruction(
        &self,
        signer: &Pubkey,
    ) -> Result<solana_sdk::instruction::Instruction> {
        Ok(ore_api::sdk::claim_sol(*signer))
    }
    
    /// Build ClaimOre instruction using ore-api SDK
    pub fn build_claim_ore_instruction(
        &self,
        signer: &Pubkey,
    ) -> Result<solana_sdk::instruction::Instruction> {
        Ok(ore_api::sdk::claim_ore(*signer))
    }
    
    /// Build Checkpoint instruction using ore-api SDK
    pub fn build_checkpoint_instruction(
        &self,
        signer: &Pubkey,
        authority: &Pubkey,
        round_id: u64,
    ) -> Result<solana_sdk::instruction::Instruction> {
        Ok(ore_api::sdk::checkpoint(*signer, *authority, round_id))
    }
    
    /// Get time remaining in current round based on slots
    pub async fn get_slots_remaining(&self) -> Result<u64> {
        let board = self.get_board_state().await?;
        let current_slot = self.rpc.get_slot().await?;
        
        if current_slot >= board.end_slot || board.end_slot == u64::MAX {
            Ok(0)
        } else {
            Ok(board.end_slot - current_slot)
        }
    }
    
    /// Check if we're in the submission window (near end of round)
    pub async fn in_submission_window(&self) -> Result<bool> {
        let slots_remaining = self.get_slots_remaining().await?;
        // ORE rounds are ~150 slots, submit in last ~10 slots
        Ok(slots_remaining <= 10 && slots_remaining > 0)
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
    
    /// Send and confirm transaction with simulation first
    pub async fn send_transaction(&self, tx: &Transaction) -> Result<Signature> {
        use solana_client::rpc_config::RpcSimulateTransactionConfig;
        
        // Simulate first to get detailed error
        let sim_config = RpcSimulateTransactionConfig {
            sig_verify: true,
            replace_recent_blockhash: false,
            commitment: Some(solana_sdk::commitment_config::CommitmentConfig::confirmed()),
            ..Default::default()
        };
        
        let sim_result = self.rpc.simulate_transaction_with_config(tx, sim_config).await
            .context("Failed to simulate transaction")?;
        
        if let Some(err) = sim_result.value.err {
            let logs = sim_result.value.logs.unwrap_or_default().join("\n");
            tracing::error!("Simulation failed: {:?}\nLogs:\n{}", err, logs);
            return Err(anyhow::anyhow!("Simulation failed: {:?}\nLogs: {}", err, logs));
        }
        
        tracing::info!("Simulation passed, sending transaction...");
        
        // Now send for real
        let sig = self.rpc.send_and_confirm_transaction(tx).await
            .context("Failed to send transaction")?;
        Ok(sig)
    }
    
    /// Get current slot
    pub async fn get_slot(&self) -> Result<u64> {
        let slot = self.rpc.get_slot().await
            .context("Failed to get current slot")?;
        Ok(slot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    
    #[test]
    fn test_program_id() {
        assert_eq!(ORE_PROGRAM_ID.to_string(), "oreV3EG1i9BEgiAJ8b177Z2S2rMarzak4NMv1kULvWv");
    }
    
    #[test]
    fn test_pdas() {
        let (board, _) = board_pda();
        println!("Board PDA: {}", board);
        
        let (round, _) = round_pda(1);
        println!("Round 1 PDA: {}", round);
        
        let wallet = Pubkey::from_str("11111111111111111111111111111111").unwrap();
        let (miner, _) = miner_pda(wallet);
        println!("Miner PDA: {}", miner);
    }
}
