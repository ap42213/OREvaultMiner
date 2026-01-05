//! Mining Strategy Engine
//! 
//! Implements the timing strategy and EV calculation for ORE v3 mining.
//! Timing: Wait until T-2.0s, evaluate all 25 blocks, GO/NO-GO decision at T-1.6s,
//! submit via Jito at T-1.0s.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Result, Context};
use tokio::sync::{broadcast, RwLock};
use tokio::time::{Duration, sleep};
use tracing::{debug, info, warn, error};
use uuid::Uuid;

use crate::ai::{AiStrategy, GridState};
use crate::ore::{OreClient, BlockData, RoundState};
use crate::jito::JitoClient;
use crate::wallet::WalletManager;
use crate::Strategy;

/// Round decision result
#[derive(Debug, Clone)]
pub enum RoundDecision {
    Deploy {
        block_index: u8,
        expected_ev: f64,
        deploy_amount: u64,
        tip_amount: u64,
    },
    Skip {
        reason: String,
        best_ev: f64,
    },
}

/// EV calculation result for a block
#[derive(Debug, Clone)]
pub struct BlockEv {
    pub index: u8,
    pub total_deployed: u64,
    pub potential_reward: u64,
    pub win_probability: f64,
    pub ev: f64,
    pub tip_cost: u64,
}

/// Session configuration
#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub session_id: Uuid,
    pub wallet: String,
    pub strategy: Strategy,
    pub deploy_amount: u64,
    pub max_tip: u64,
    pub num_blocks: u8,
}

/// Active session state
#[derive(Debug)]
struct ActiveSession {
    config: SessionConfig,
    rounds_played: u64,
    rounds_skipped: u64,
    total_deployed: u64,
    total_tips: u64,
    total_won: u64,
    cancel_tx: broadcast::Sender<()>,
}

/// Strategy engine for automated mining
pub struct StrategyEngine {
    ore_client: OreClient,
    jito_client: JitoClient,
    ai_strategy: Option<AiStrategy>,
    wallet_manager: Option<Arc<WalletManager>>,
    active_sessions: HashMap<String, ActiveSession>,
    event_tx: broadcast::Sender<StrategyEvent>,
}

/// Events emitted by the strategy engine
#[derive(Debug, Clone)]
pub enum StrategyEvent {
    RoundUpdate {
        wallet: String,
        round_id: u64,
        time_left: f64,
        blocks: Vec<BlockEv>,
    },
    /// AI analysis result
    AiAnalysis {
        wallet: String,
        selected_block: u8,
        confidence: f64,
        reasoning: String,
        skip: bool,
    },
    DecisionMade {
        wallet: String,
        decision: RoundDecision,
    },
    TxSubmitted {
        wallet: String,
        signature: String,
        block_index: u8,
        amount: u64,
    },
    TxConfirmed {
        wallet: String,
        signature: String,
        status: String,
        reward: Option<u64>,
    },
}

impl StrategyEngine {
    /// Create a new strategy engine
    pub fn new(ore_client: OreClient, jito_client: JitoClient) -> Self {
        let (event_tx, _) = broadcast::channel(1024);
        
        Self {
            ore_client,
            jito_client,
            ai_strategy: None,
            wallet_manager: None,
            active_sessions: HashMap::new(),
            event_tx,
        }
    }
    
    /// Set wallet manager for server-side signing (automine)
    pub fn set_wallet_manager(&mut self, wm: Arc<WalletManager>) {
        self.wallet_manager = Some(wm);
    }
    
    /// Set the AI strategy for intelligent block selection
    pub fn set_ai_strategy(&mut self, ai: AiStrategy) {
        self.ai_strategy = Some(ai);
    }
    
    /// Subscribe to strategy events
    pub fn subscribe(&self) -> broadcast::Receiver<StrategyEvent> {
        self.event_tx.subscribe()
    }
    
    /// Start a mining session for a wallet
    pub async fn start_session(
        &mut self,
        session_id: Uuid,
        wallet: String,
        strategy: Strategy,
        deploy_amount: f64,
        max_tip: f64,
        num_blocks: u8,
    ) {
        // Convert SOL to lamports
        let deploy_amount_lamports = (deploy_amount * 1_000_000_000.0) as u64;
        let max_tip_lamports = (max_tip * 1_000_000_000.0) as u64;
        
        let config = SessionConfig {
            session_id,
            wallet: wallet.clone(),
            strategy,
            deploy_amount: deploy_amount_lamports,
            max_tip: max_tip_lamports,
            num_blocks: num_blocks.clamp(1, 25),
        };
        
        let (cancel_tx, _) = broadcast::channel(1);
        
        let session = ActiveSession {
            config: config.clone(),
            rounds_played: 0,
            rounds_skipped: 0,
            total_deployed: 0,
            total_tips: 0,
            total_won: 0,
            cancel_tx: cancel_tx.clone(),
        };
        
        self.active_sessions.insert(wallet.clone(), session);
        
        info!("Started mining session {} for wallet {}", session_id, wallet);
        
        // Spawn the mining loop
        let ore_client = self.ore_client.clone();
        let jito_client = self.jito_client.clone();
        let event_tx = self.event_tx.clone();
        let ai_strategy = self.ai_strategy.clone();
        let wallet_manager = self.wallet_manager.clone();
        let cancel_rx = cancel_tx.subscribe();
        
        tokio::spawn(async move {
            Self::mining_loop(
                config,
                ore_client,
                jito_client,
                ai_strategy,
                wallet_manager,
                event_tx,
                cancel_rx,
            ).await;
        });
    }
    
    /// Stop a mining session
    pub async fn stop_session(&mut self, wallet: &str) {
        if let Some(session) = self.active_sessions.remove(wallet) {
            let _ = session.cancel_tx.send(());
            info!(
                "Stopped session for wallet {} - Played: {}, Skipped: {}, Won: {} lamports",
                wallet, session.rounds_played, session.rounds_skipped, session.total_won
            );
        }
    }
    
    /// Main mining loop
    async fn mining_loop(
        config: SessionConfig,
        ore_client: OreClient,
        jito_client: JitoClient,
        ai_strategy: Option<AiStrategy>,
        wallet_manager: Option<Arc<WalletManager>>,
        event_tx: broadcast::Sender<StrategyEvent>,
        mut cancel_rx: broadcast::Receiver<()>,
    ) {
        info!("Mining loop started for wallet {}", config.wallet);
        
        // Check if we have signing capability
        let can_sign = if let Some(ref wm) = wallet_manager {
            wm.has_keypair(&config.wallet).await
        } else {
            false
        };
        
        if !can_sign {
            warn!("No keypair found for {} - transactions will require frontend signing", config.wallet);
        } else {
            info!("Automine enabled - server-side signing for {}", config.wallet);
        }
        
        loop {
            // Check for cancellation
            if cancel_rx.try_recv().is_ok() {
                info!("Mining loop cancelled for wallet {}", config.wallet);
                break;
            }
            
            // PHASE 1: No pre-caching needed - Gemini Flash gives ~750ms real-time decisions
            // We'll query AI at T-2s when we have the latest state
            
            // PHASE 2: Wait for final submission window (T-2.0s)
            match Self::wait_for_submission_window(&ore_client).await {
                Ok(round) => {
                    // Snapshot all blocks at T-2.0s
                    let blocks = match ore_client.get_all_blocks().await {
                        Ok(b) => b,
                        Err(e) => {
                            error!("Failed to get blocks: {}", e);
                            continue;
                        }
                    };
                    
                    // Calculate EV for all blocks at T-1.8s
                    let recommended_tip = jito_client.get_recommended_tip().await.unwrap_or(1_000_000);
                    let tip_cost = recommended_tip.min(config.max_tip);
                    
                    let block_evs = Self::calculate_all_ev(
                        &blocks,
                        round.total_deployed,
                        config.deploy_amount,
                        tip_cost,
                    );
                    
                    // Emit round update event - convert slots to approximate seconds (400ms per slot)
                    let slots_left = if round.end_slot > round.start_slot { 
                        ore_client.get_slots_remaining().await.unwrap_or(0) 
                    } else { 0 };
                    let time_left = slots_left as f64 * 0.4; // ~400ms per slot
                    
                    let _ = event_tx.send(StrategyEvent::RoundUpdate {
                        wallet: config.wallet.clone(),
                        round_id: round.round_id,
                        time_left,
                        blocks: block_evs.clone(),
                    });
                    
                    // PHASE 3: Pick lowest stake blocks (no AI - too slow)
                    // Use num_blocks from session config
                    let num_blocks: usize = config.num_blocks as usize;
                    
                    // Sort blocks by stake (lowest first)
                    let mut sorted_blocks: Vec<(usize, u64)> = blocks.iter()
                        .enumerate()
                        .map(|(i, b)| (i, b.total_deployed))
                        .collect();
                    sorted_blocks.sort_by_key(|(_, stake)| *stake);
                    
                    // Take the N lowest stake blocks
                    let selected_blocks: Vec<u8> = sorted_blocks.iter()
                        .take(num_blocks)
                        .map(|(i, _)| *i as u8)
                        .collect();
                    
                    let first_block = selected_blocks.first().copied().unwrap_or(0);
                    let min_stake = sorted_blocks.first().map(|(_, s)| *s).unwrap_or(0);
                    
                    info!("Selected {} block(s): {:?} (lowest stake: {} lamports)", 
                        selected_blocks.len(), selected_blocks, min_stake);
                    
                    // Emit AI analysis event for frontend
                    let _ = event_tx.send(StrategyEvent::AiAnalysis {
                        wallet: config.wallet.clone(),
                        selected_block: first_block,
                        confidence: 0.9,
                        reasoning: format!("Lowest {} stake block(s), min {} lamports", num_blocks, min_stake),
                        skip: false,
                    });
                    
                    let block_ev = block_evs.iter()
                        .find(|b| b.index == first_block)
                        .map(|b| b.ev)
                        .unwrap_or(0.0);
                    
                    // For multi-block, we'll use a custom squares array
                    let decision = RoundDecision::Deploy {
                        block_index: first_block, // Primary block for logging
                        expected_ev: block_ev,
                        deploy_amount: config.deploy_amount,
                        tip_amount: tip_cost,
                    };
                    
                    // Store selected blocks for submit_deploy
                    let selected_squares: [bool; 25] = {
                        let mut arr = [false; 25];
                        for &idx in &selected_blocks {
                            if (idx as usize) < 25 {
                                arr[idx as usize] = true;
                            }
                        }
                        arr
                    };
                    
                    // Emit decision event
                    let _ = event_tx.send(StrategyEvent::DecisionMade {
                        wallet: config.wallet.clone(),
                        decision: decision.clone(),
                    });
                    
                    // Submit immediately - we're already in tight window (3 seconds or less)
                    match decision {
                        RoundDecision::Deploy { block_index, deploy_amount, tip_amount, .. } => {
                            // No additional delay - window is already tight at 8 slots (~3s)
                            
                            // Build and submit bundle
                            match Self::submit_deploy(
                                &ore_client,
                                &jito_client,
                                &wallet_manager,
                                &config.wallet,
                                block_index,
                                deploy_amount,
                                tip_amount,
                                selected_squares,
                            ).await {
                                Ok(signature) => {
                                    let _ = event_tx.send(StrategyEvent::TxSubmitted {
                                        wallet: config.wallet.clone(),
                                        signature: signature.clone(),
                                        block_index,
                                        amount: deploy_amount,
                                    });
                                    
                                    let blocks_count = selected_squares.iter().filter(|&&b| b).count();
                                    info!(
                                        "Submitted deploy: wallet={}, blocks={} ({:?}), amount={} lamports, tx={}",
                                        config.wallet, blocks_count, selected_blocks, deploy_amount, signature
                                    );
                                }
                                Err(e) => {
                                    error!("Failed to submit deploy: {}", e);
                                }
                            }
                        }
                        RoundDecision::Skip { reason, best_ev } => {
                            debug!(
                                "Skipped round: wallet={}, reason={}, best_ev={}",
                                config.wallet, reason, best_ev
                            );
                        }
                    }
                    
                    // Wait for this round to end before looking for next
                    let current_round = round.round_id;
                    loop {
                        sleep(Duration::from_millis(500)).await;
                        // Only need the board's round_id here (cheaper than fetching the full round account)
                        if let Ok(board) = ore_client.get_board_state().await {
                            if board.round_id != current_round {
                                info!("Round {} ended, moving to round {}", current_round, board.round_id);
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Error waiting for submission window: {}", e);
                    sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }
    
    /// Wait until we're in the submission window (near end of round)
    /// OPTIMIZED: Uses parallel RPC calls with timeouts to avoid blocking
    async fn wait_for_submission_window(ore_client: &OreClient) -> Result<RoundState> {
        use tokio::time::{timeout, Instant};

        // Keep timeouts short so we can recover quickly from slow RPC.
        const RPC_TIMEOUT: Duration = Duration::from_millis(1000);
        // Target the *actual* end-of-round window. 10 slots ~= ~4s at ~400ms/slot.
        // This aligns much better with the README timing (T-2s snapshot, T-1s submit)
        // than the previous 30-slot (~12s) trigger.
        const SUBMISSION_WINDOW_SLOTS: u64 = 10;
        const BOARD_REFRESH_INTERVAL: Duration = Duration::from_millis(1000);

        let mut last_board_fetch = Instant::now() - BOARD_REFRESH_INTERVAL;
        let mut cached_board: Option<crate::ore::BoardState> = None;
        let mut consecutive_failures: u32 = 0;

        loop {
            // Refresh board state occasionally (end_slot changes only once per round).
            if cached_board.is_none() || last_board_fetch.elapsed() >= BOARD_REFRESH_INTERVAL {
                match timeout(RPC_TIMEOUT, ore_client.get_board_state()).await {
                    Ok(Ok(board)) => {
                        cached_board = Some(board);
                        last_board_fetch = Instant::now();
                        consecutive_failures = 0;
                    }
                    Ok(Err(e)) => {
                        debug!("Board fetch error: {}", e);
                        consecutive_failures = consecutive_failures.saturating_add(1);
                    }
                    Err(_) => {
                        debug!("Board fetch timeout (>1s)");
                        consecutive_failures = consecutive_failures.saturating_add(1);
                    }
                }
            }

            let Some(board) = cached_board.clone() else {
                // Back off slightly to avoid a tight failure loop when RPC is down.
                sleep(Duration::from_millis(100)).await;
                continue;
            };

            // If end_slot is MAX, round hasn't started yet.
            if board.end_slot == u64::MAX {
                sleep(Duration::from_millis(200)).await;
                continue;
            }

            let current_slot = match timeout(RPC_TIMEOUT, ore_client.rpc().get_slot()).await {
                Ok(Ok(s)) => {
                    consecutive_failures = 0;
                    s
                }
                Ok(Err(e)) => {
                    debug!("Slot fetch error: {}", e);
                    consecutive_failures = consecutive_failures.saturating_add(1);
                    sleep(Duration::from_millis(50)).await;
                    continue;
                }
                Err(_) => {
                    debug!("Slot fetch timeout (>1s)");
                    consecutive_failures = consecutive_failures.saturating_add(1);
                    sleep(Duration::from_millis(50)).await;
                    continue;
                }
            };

            let slots_remaining = if current_slot >= board.end_slot {
                0
            } else {
                board.end_slot - current_slot
            };

            // If round advanced, force a board refresh next loop.
            if slots_remaining == 0 {
                cached_board = None;
                sleep(Duration::from_millis(50)).await;
                continue;
            }

            if slots_remaining <= SUBMISSION_WINDOW_SLOTS {
                info!(
                    "Entering submission window: {} slots remaining (~{:.1}s), round_id={}",
                    slots_remaining,
                    slots_remaining as f64 * 0.4,
                    board.round_id
                );

                match timeout(Duration::from_millis(1500), ore_client.get_current_round_state()).await {
                    Ok(Ok(round)) => return Ok(round),
                    Ok(Err(e)) => {
                        warn!("Failed to fetch round state at window entry: {}", e);
                    }
                    Err(_) => {
                        warn!("Round state fetch timeout at window entry");
                    }
                }

                // If the round-state fetch failed, retry quickly but never spin.
                sleep(Duration::from_millis(30)).await;
                continue;
            }

            // Dynamic polling cadence: frequent near the end, gentler earlier.
            let sleep_ms = if slots_remaining > 120 {
                250
            } else if slots_remaining > 60 {
                120
            } else if slots_remaining > 25 {
                60
            } else {
                30
            };

            // Extra backoff when RPC is unhappy.
            let backoff_ms = match consecutive_failures {
                0..=2 => 0,
                3..=6 => 50,
                _ => 150,
            };
            sleep(Duration::from_millis(sleep_ms + backoff_ms)).await;
        }
    }
    
    /// Calculate EV for all 25 blocks
    fn calculate_all_ev(
        blocks: &[BlockData; 25],
        total_pot: u64,
        deploy_amount: u64,
        tip_cost: u64,
    ) -> Vec<BlockEv> {
        blocks.iter().map(|block| {
            Self::calculate_block_ev(block, total_pot, deploy_amount, tip_cost)
        }).collect()
    }
    
    /// Calculate EV for a single block
    /// EV = (potential_reward * win_probability) - tip_cost
    fn calculate_block_ev(
        block: &BlockData,
        total_pot: u64,
        deploy_amount: u64,
        tip_cost: u64,
    ) -> BlockEv {
        // Win probability is 1/25 for each block (RNG)
        let win_probability = 1.0 / 25.0;
        
        // If we deploy, our share of winning block
        let new_block_total = block.total_deployed + deploy_amount;
        let our_share = if new_block_total > 0 {
            deploy_amount as f64 / new_block_total as f64
        } else {
            1.0 // We'd be the only deployer
        };
        
        // Potential reward if our block wins
        // We get our share of the total pot
        let potential_reward = (total_pot as f64 * our_share) as u64;
        
        // Expected value calculation
        // EV = (potential_reward * 1/25) - (tip_cost + deploy_amount that could be lost)
        // Note: Deploy amount is at risk, but we keep it if we win
        // So we only consider the cost of the tip
        let expected_reward = potential_reward as f64 * win_probability;
        let ev = expected_reward - (tip_cost as f64);
        
        BlockEv {
            index: block.index,
            total_deployed: block.total_deployed,
            potential_reward,
            win_probability,
            ev,
            tip_cost,
        }
    }
    
    /// Make GO/NO-GO decision based on strategy - ALWAYS DEPLOY
    fn make_decision(
        block_evs: &[BlockEv],
        strategy: &Strategy,
        deploy_amount: u64,
        tip_cost: u64,
    ) -> RoundDecision {
        // Find best block based on strategy
        let best_block = match strategy {
            Strategy::BestEv => {
                // Pick the block with highest EV
                block_evs.iter().max_by(|a, b| a.ev.partial_cmp(&b.ev).unwrap())
            }
            Strategy::Conservative => {
                // Pick block with lowest competition
                block_evs.iter().min_by_key(|b| b.total_deployed)
            }
            Strategy::Aggressive => {
                // Pick block with highest pot share
                block_evs.iter().max_by(|a, b| 
                    a.potential_reward.cmp(&b.potential_reward))
            }
        };
        
        match best_block {
            Some(block) => {
                // ALWAYS deploy, regardless of EV
                RoundDecision::Deploy {
                    block_index: block.index,
                    expected_ev: block.ev,
                    deploy_amount,
                    tip_amount: tip_cost,
                }
            }
            None => {
                // Fallback to block 0 if somehow no blocks
                RoundDecision::Deploy {
                    block_index: 0,
                    expected_ev: 0.0,
                    deploy_amount,
                    tip_amount: tip_cost,
                }
            }
        }
    }
    
    /// Make AI-powered decision using OpenRouter/Intellect 3
    async fn make_ai_decision(
        ai: &AiStrategy,
        blocks: &[BlockData; 25],
        round: &RoundState,
        slots_remaining: u64,
        strategy: &Strategy,
        deploy_amount: u64,
        tip_cost: u64,
    ) -> RoundDecision {
        // Build grid state for AI
        let grid = GridState {
            deployed: blocks.iter().map(|b| b.total_deployed).collect(),
            miner_counts: blocks.iter().map(|b| b.miner_count).collect(),
            total_pot: round.total_deployed,
            round_id: round.round_id,
            slots_remaining,
            deploy_amount,
            tip_cost,
        };
        
        let strategy_hint = match strategy {
            Strategy::BestEv => "best_ev",
            Strategy::Conservative => "conservative",
            Strategy::Aggressive => "aggressive",
        };
        
        // Get AI selection (1 block for now)
        match ai.select_blocks(&grid, 1, strategy_hint).await {
            Ok(selection) if !selection.blocks.is_empty() => {
                let block_index = selection.blocks[0];
                
                // Calculate EV for the selected block
                let block_deployed = blocks[block_index as usize].total_deployed;
                let new_total = block_deployed + deploy_amount;
                let win_probability = if new_total > 0 {
                    deploy_amount as f64 / new_total as f64
                } else {
                    1.0
                };
                let other_squares_pot = round.total_deployed.saturating_sub(block_deployed);
                let expected_winnings = win_probability * other_squares_pot as f64;
                let ev = expected_winnings * 0.04 - tip_cost as f64;
                let ev_sol = ev / 1_000_000_000.0;
                
                // If confidence is low or EV is very negative, skip
                if selection.confidence < 0.3 || ev_sol < -0.1 {
                    info!("AI selected block {} but confidence low ({:.2}) or EV too negative ({:.6})", 
                        block_index, selection.confidence, ev_sol);
                    return RoundDecision::Skip {
                        reason: format!("AI confidence too low: {:.2}", selection.confidence),
                        best_ev: ev_sol,
                    };
                }
                
                info!("AI selected block {} with confidence {:.2}: {}", 
                    block_index, selection.confidence, selection.reasoning);
                
                RoundDecision::Deploy {
                    block_index,
                    expected_ev: ev_sol,
                    deploy_amount,
                    tip_amount: tip_cost,
                }
            }
            Ok(_) => {
                warn!("AI returned no block selections");
                RoundDecision::Skip {
                    reason: "AI found no viable blocks".to_string(),
                    best_ev: 0.0,
                }
            }
            Err(e) => {
                warn!("AI selection failed, using fallback: {}", e);
                // Fall back to best EV calculation
                let block_evs: Vec<_> = blocks.iter().map(|block| {
                    Self::calculate_block_ev(block, round.total_deployed, deploy_amount, tip_cost)
                }).collect();
                
                Self::make_decision(&block_evs, strategy, deploy_amount, tip_cost)
            }
        }
    }
    
    /// Submit deploy transaction via Jito
    /// If wallet_manager has the keypair, sign server-side (automine)
    /// Otherwise, return unsigned for frontend signing
    async fn submit_deploy(
        ore_client: &OreClient,
        jito_client: &JitoClient,
        wallet_manager: &Option<Arc<WalletManager>>,
        wallet: &str,
        block_index: u8,
        deploy_amount: u64,
        tip_amount: u64,
        squares: [bool; 25],
    ) -> Result<String> {
        let wallet_pubkey: solana_sdk::pubkey::Pubkey = wallet.parse()
            .context("Invalid wallet address")?;
        
        let blocks_selected: Vec<usize> = squares.iter().enumerate().filter(|(_, &b)| b).map(|(i, _)| i).collect();
        info!("Building deploy tx: wallet={}, blocks={:?}, amount={} lamports", 
            wallet, blocks_selected, deploy_amount);
        
        // Get current round ID from board
        let board = ore_client.get_board_state().await?;
        info!("Current round: {} (end_slot: {})", board.round_id, board.end_slot);

        // Check if miner PDA exists and needs checkpointing.
        // The ORE deploy instruction requires: miner.checkpoint_id == miner.round_id
        // If miner participated in a previous round, we must checkpoint that round first.
        // IMPORTANT: Checkpoint must be sent as a SEPARATE transaction before deploy
        // because Solana instructions in the same tx see original state, not modified state.
        let miner_data = ore_client.get_miner_data(&wallet_pubkey).await?;
        let needs_checkpoint = match &miner_data {
            Some(m) => {
                // Need checkpoint if:
                // 1. checkpoint_id != round_id (haven't checkpointed last participation), OR
                // 2. miner.round_id > 0 AND miner.round_id < board.round_id (participated in old round)
                let needs_cp = (m.checkpoint_id != m.round_id) || 
                               (m.round_id > 0 && m.round_id < board.round_id);
                info!(
                    "Miner state: round_id={}, checkpoint_id={}, board_round={}, needs_checkpoint={}",
                    m.round_id, m.checkpoint_id, board.round_id, needs_cp
                );
                needs_cp
            }
            None => {
                info!("Miner PDA does not exist yet - no checkpoint needed");
                false
            }
        };
        
        // If checkpoint is needed, send it as a SEPARATE transaction first
        if needs_checkpoint {
            let miner_round_id = miner_data.as_ref().unwrap().round_id;
            info!(
                "Sending checkpoint transaction FIRST for miner's round {} (current board round: {})",
                miner_round_id, board.round_id
            );
            
            let checkpoint_ix = ore_client.build_checkpoint_instruction(
                &wallet_pubkey,
                &wallet_pubkey,
                miner_round_id,
            )?;
            
            // Add compute budget instructions for priority (checkpoint needs to land fast)
            let cu_limit_ix = solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_limit(50_000);
            let cu_price_ix = solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_price(100_000); // 100k micro-lamports per CU
            
            let blockhash = ore_client.get_latest_blockhash().await?;
            
            // Build and sign checkpoint transaction (need wallet_manager for signing)
            if let Some(ref wm) = wallet_manager {
                let mut checkpoint_tx = solana_sdk::transaction::Transaction::new_with_payer(
                    &[cu_limit_ix, cu_price_ix, checkpoint_ix],
                    Some(&wallet_pubkey),
                );
                checkpoint_tx.message.recent_blockhash = blockhash;
                wm.sign_transaction(wallet, &mut checkpoint_tx).await
                    .context("Failed to sign checkpoint transaction")?;
                
                // Send checkpoint transaction via RPC with priority fee
                match ore_client.send_transaction(&checkpoint_tx).await {
                    Ok(sig) => {
                        info!("Checkpoint transaction sent with priority fee: {}", sig);
                        
                        // Wait for RPC confirmation (up to 5 seconds)
                        let confirmed = ore_client.confirm_transaction(&sig, 5).await.unwrap_or(false);
                        
                        if confirmed {
                            info!("Checkpoint transaction confirmed via RPC: {}", sig);
                        } else {
                            // Fallback: poll miner state to verify checkpoint applied
                            warn!("RPC confirm timed out, checking miner state...");
                            let mut checkpoint_confirmed = false;
                            for attempt in 0..5 {
                                tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;
                                if let Some(m) = ore_client.get_miner_data(&wallet_pubkey).await? {
                                    if m.checkpoint_id == m.round_id {
                                        info!(
                                            "Checkpoint verified via miner state after {}ms: checkpoint_id={} == round_id={}",
                                            (attempt + 1) * 400, m.checkpoint_id, m.round_id
                                        );
                                        checkpoint_confirmed = true;
                                        break;
                                    }
                                }
                            }
                            
                            if !checkpoint_confirmed {
                                warn!("Checkpoint may not have confirmed - proceeding anyway");
                            }
                        }
                    }
                    Err(e) => {
                        // Checkpoint might fail if already done or round expired - that's OK
                        warn!("Checkpoint transaction failed (may be OK): {}", e);
                    }
                }
            } else {
                warn!("No wallet manager - cannot sign checkpoint transaction server-side");
            }
        }
        
        // Build deploy instruction using ore-api SDK (squares already passed in)
        // IMPORTANT: ORE v3 requires the automation account PDA to exist before deploying.
        // The automation account is created by calling `automate` instruction first.
        // For ORE v3, if an automation account exists, deploy MUST use the automation path.
        // We need to ensure the automation account has sufficient balance before deploying.
        // Calculate needed balance: deploy_amount * num_squares (squares we're deploying to)
        let num_squares = squares.iter().filter(|&&s| s).count() as u64;
        let needed_balance = deploy_amount.saturating_mul(num_squares.max(1));
        
        info!("Automate config: deploy_amount={} lamports ({} SOL), num_squares={}, needed_balance={}", 
              deploy_amount, deploy_amount as f64 / 1_000_000_000.0, num_squares, needed_balance);
        
        // Check existing automation balance and only deposit the difference
        let current_balance = ore_client.get_automation_balance(&wallet_pubkey).await.unwrap_or(0);
        let deposit_needed = if current_balance >= needed_balance {
            0 // Already have enough
        } else {
            needed_balance - current_balance
        };
        
        // Only call automate if we need to deposit more funds
        if deposit_needed > 0 {
            info!("Automation setup: amount_per_square={} lamports ({} SOL), balance_needed={}, depositing={}", 
                  deploy_amount, deploy_amount as f64 / 1_000_000_000.0, needed_balance, deposit_needed);
        
            // ORE v3 AutomationStrategy enum: 0=Random, 1=Preferred, 2=Discretionary
            let automate_ix = ore_client.build_automate_instruction(
                &wallet_pubkey,  // signer
                deploy_amount,   // amount per square (MUST be in lamports)
                deposit_needed,  // deposit - only what we need to add (lamports)
                &wallet_pubkey,  // executor = self (discretionary mode)
                0,               // fee = 0 (no executor fee since we're our own executor)
                0,               // mask = 0 (we specify squares in deploy for discretionary)
                2,               // strategy = 2 (Discretionary - use executor's provided mask)
                false,           // reload = false
            )?;
            
            info!("Built automate instruction with amount={} lamports for {} squares", deploy_amount, num_squares);
            
            let cu_limit_ix = solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_limit(400_000);
            let cu_price_ix = solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_price(100_000);
            
            let blockhash = ore_client.get_latest_blockhash().await?;
            
            if let Some(ref wm) = wallet_manager {
                let mut automate_tx = solana_sdk::transaction::Transaction::new_with_payer(
                    &[cu_limit_ix, cu_price_ix, automate_ix],
                    Some(&wallet_pubkey),
                );
                automate_tx.message.recent_blockhash = blockhash;
                wm.sign_transaction(wallet, &mut automate_tx).await
                    .context("Failed to sign automate transaction")?;
                
                match ore_client.send_transaction(&automate_tx).await {
                    Ok(sig) => {
                        info!("Automate transaction sent: {}", sig);
                        
                        // Wait for confirmation
                        let confirmed = ore_client.confirm_transaction(&sig, 5).await.unwrap_or(false);
                        if confirmed {
                            info!("Automate transaction confirmed - automation account funded: {}", sig);
                        } else {
                            warn!("Automate confirmation timed out - proceeding anyway");
                        }
                    }
                    Err(e) => {
                        // May fail if already funded - that's OK
                        warn!("Automate transaction failed (may be OK): {}", e);
                    }
                }
            } else {
                warn!("No wallet manager - cannot fund automation account");
            }
        } else {
            info!("Automation balance sufficient: {} lamports (need {})", current_balance, needed_balance);
        }
        
        let deploy_ix = ore_client.build_deploy_instruction(
            &wallet_pubkey,
            &wallet_pubkey, // authority is same as signer for user deploys
            deploy_amount,
            board.round_id,
            squares,
        )?;
        
        info!("Deploy instruction built: program={}", deploy_ix.program_id);
        
        // Add compute budget for priority fee on deploy
        let cu_limit_ix = solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_limit(500_000);
        let cu_price_ix = solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_price(100_000); // 100k micro-lamports per CU
        
        // Get recent blockhash
        let blockhash = ore_client.get_latest_blockhash().await?;
        info!("Blockhash: {}", blockhash);
        
        // Build transaction with compute budget + deploy (no Jito tip)
        let mut tx = solana_sdk::transaction::Transaction::new_with_payer(
            &[cu_limit_ix, cu_price_ix, deploy_ix],
            Some(&wallet_pubkey),
        );
        tx.message.recent_blockhash = blockhash;

        info!("Transaction built with priority fee + deploy instruction");
        
        // Check if we can sign server-side (automine)
        if let Some(ref wm) = wallet_manager {
            if wm.has_keypair(wallet).await {
                // Server-side signing - automine mode!
                tx.message.recent_blockhash = blockhash;
                wm.sign_transaction(wallet, &mut tx).await
                    .context("Failed to sign transaction")?;
                
                info!("Signed transaction server-side for automine");
                
                // Send directly via RPC (Jito disabled - too unreliable)
                match ore_client.send_transaction(&tx).await {
                    Ok(sig) => {
                        info!("Transaction sent via RPC: {}", sig);
                        return Ok(sig.to_string());
                    }
                    Err(rpc_err) => {
                        error!("RPC send failed: {}", rpc_err);
                        return Err(anyhow::anyhow!("RPC send failed: {}", rpc_err));
                    }
                }
            }
        }
        
        // No keypair available - need frontend signing
        warn!("No keypair for {} - transaction requires frontend signing", wallet);
        Ok(format!("pending_signature_{}", uuid::Uuid::new_v4()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ev_calculation() {
        let block = BlockData {
            index: 0,
            total_deployed: 1_000_000_000, // 1 SOL
            miner_count: 5,
        };
        
        let total_pot = 10_000_000_000; // 10 SOL
        let deploy_amount = 100_000_000; // 0.1 SOL
        let tip_cost = 1_000_000; // 0.001 SOL
        
        let ev = StrategyEngine::calculate_block_ev(
            &block,
            total_pot,
            deploy_amount,
            tip_cost,
        );
        
        // New block total = 1.1 SOL
        // Our share = 0.1 / 1.1 = ~0.0909
        // Potential reward = 10 * 0.0909 = ~0.909 SOL
        // Expected value = 0.909 * (1/25) - 0.001 = ~0.0354 SOL
        
        assert!(ev.ev > 0.0, "EV should be positive for profitable block");
        assert_eq!(ev.index, 0);
    }
    
    #[test]
    fn test_decision_skip_negative_ev() {
        let block_evs = vec![BlockEv {
            index: 0,
            total_deployed: 100_000_000_000, // 100 SOL - very crowded
            potential_reward: 100_000, // Tiny share
            win_probability: 0.04,
            ev: -900_000.0, // Negative EV
            tip_cost: 1_000_000,
        }];
        
        let decision = StrategyEngine::make_decision(
            &block_evs,
            &Strategy::BestEv,
            100_000_000,
            1_000_000,
        );
        
        assert!(matches!(decision, RoundDecision::Skip { .. }));
    }
}
