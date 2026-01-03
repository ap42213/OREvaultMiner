//! OreVault Backend - Automated ORE v3 Mining Engine
//! 
//! This is the main entry point for the OreVault mining system.
//! Handles WebSocket connections, REST API, and coordinates mining strategy.

mod ai;
mod balances;
mod claims;
mod db;
mod jito;
mod ore;
mod strategy;
mod wallet;
mod ws;

use std::sync::Arc;
use std::net::SocketAddr;

use anyhow::Result;
use axum::{
    routing::{get, post},
    Router,
    Json,
    extract::{State, Query, WebSocketUpgrade},
    response::IntoResponse,
};
use tower_http::cors::{CorsLayer, Any};
use tower_http::trace::TraceLayer;
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;

use crate::ai::AiStrategy;
use crate::db::Database;
use crate::ws::WebSocketManager;
use crate::strategy::StrategyEngine;
use crate::balances::BalanceManager;
use crate::claims::ClaimsProcessor;
use crate::ore::OreClient;
use crate::jito::JitoClient;
use crate::wallet::WalletManager;

/// Application state shared across all handlers
pub struct AppState {
    pub db: Database,
    pub ws_manager: WebSocketManager,
    pub strategy_engine: Arc<RwLock<StrategyEngine>>,
    pub balance_manager: BalanceManager,
    pub claims_processor: ClaimsProcessor,
    pub ore_client: OreClient,
    pub jito_client: JitoClient,
    pub ai_strategy: AiStrategy,
    pub wallet_manager: Arc<WalletManager>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenvy::dotenv().ok();
    
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "orevault=debug,tower_http=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();
    
    info!("🚀 OreVault Backend Starting...");
    info!("Network: Solana Mainnet-Beta");
    
    // Configuration from environment
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    let rpc_url = std::env::var("RPC_URL")
        .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
    let jito_block_engine = std::env::var("JITO_BLOCK_ENGINE")
        .unwrap_or_else(|_| "ny.mainnet.block-engine.jito.wtf".to_string());
    let server_port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "3001".to_string())
        .parse()
        .expect("PORT must be a valid number");
    
    // Initialize database connection pool
    info!("Connecting to PostgreSQL...");
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await?;
    
    // Run migrations
    info!("Running database migrations...");
    sqlx::migrate!("../migrations")
        .run(&pool)
        .await?;
    
    // Initialize components
    let db = Database::new(pool);
    let ws_manager = WebSocketManager::new();
    let ore_client = OreClient::new(&rpc_url)?;
    let jito_client = JitoClient::new(&jito_block_engine).await?;
    let balance_manager = BalanceManager::new(ore_client.clone());
    let claims_processor = ClaimsProcessor::new(ore_client.clone());
    
    // Initialize AI strategy with OpenRouter API key (optional)
    let openrouter_api_key = std::env::var("OPENROUTER_API_KEY").unwrap_or_default();
    let ai_strategy = AiStrategy::new(openrouter_api_key.clone());
    if ai_strategy.is_configured() {
        info!("AI Strategy enabled with Gemini 2.0 Flash (~750ms decisions)");
    } else {
        info!("AI Strategy running in fallback mode (no API key)");
    }
    
    // Initialize wallet manager with database persistence (Supabase)
    let wallet_manager = Arc::new(WalletManager::with_database(db.clone()));
    
    // Load existing wallets from database
    match wallet_manager.load_from_database().await {
        Ok(count) => info!("Loaded {} wallets from Supabase", count),
        Err(e) => warn!("Failed to load wallets from database: {}", e),
    }
    
    // Create strategy engine and wire in AI + wallet manager
    let mut strategy_engine_inner = StrategyEngine::new(ore_client.clone(), jito_client.clone());
    if !openrouter_api_key.is_empty() {
        strategy_engine_inner.set_ai_strategy(ai_strategy.clone());
    }
    strategy_engine_inner.set_wallet_manager(wallet_manager.clone());
    let strategy_engine = Arc::new(RwLock::new(strategy_engine_inner));
    
    // Create shared application state
    let state = Arc::new(AppState {
        db,
        ws_manager,
        strategy_engine,
        balance_manager,
        claims_processor,
        ore_client,
        jito_client,
        ai_strategy,
        wallet_manager,
    });
    
    // Build router with all API routes
    let app = Router::new()
        // Mining endpoints
        .route("/api/session/start", post(start_session))
        .route("/api/session/stop", post(stop_session))
        .route("/api/stats", get(get_stats))
        .route("/api/transactions", get(get_transactions))
        // Grid & Round endpoints
        .route("/api/grid", get(get_grid))
        .route("/api/round", get(get_round))
        .route("/api/ai/suggest", post(get_ai_suggestion))
        // Balance & Claims endpoints
        .route("/api/balances", get(get_balances))
        .route("/api/balances/sync", post(sync_balances))
        .route("/api/claim/sol", post(claim_sol))
        .route("/api/claim/ore", post(claim_ore))
        .route("/api/claims/history", get(get_claims_history))
        // Wallet management (automine)
        .route("/api/wallet/generate", post(generate_wallet))
        .route("/api/wallet/import", post(import_wallet))
        .route("/api/wallet/list", get(list_wallets))
        .route("/api/wallet/export", post(export_wallet))
        // WebSocket endpoint
        .route("/ws", get(ws_handler))
        // Health check
        .route("/health", get(health_check))
        .layer(CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any))
        .layer(TraceLayer::new_for_http())
        .with_state(state);
    
    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], server_port));
    info!("OreVault API listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}

// =============================================================================
// API Handlers
// =============================================================================

/// Health check endpoint
async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "version": "1.0.0",
        "network": "mainnet-beta"
    }))
}

/// Start mining session request
#[derive(Debug, Deserialize)]
pub struct StartSessionRequest {
    pub wallet: String,
    pub strategy: Strategy,
    pub deploy_amount: f64,
    pub max_tip: f64,
    pub budget: f64,
    #[serde(default = "default_num_blocks")]
    pub num_blocks: u8,
}

fn default_num_blocks() -> u8 { 1 }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Strategy {
    BestEv,
    Conservative,
    Aggressive,
}

/// Start a mining session
async fn start_session(
    State(state): State<Arc<AppState>>,
    Json(req): Json<StartSessionRequest>,
) -> impl IntoResponse {
    // Basic input validation (safety): prevent accidental catastrophic SOL amounts.
    // These values come from user input (frontend) and are interpreted as SOL.
    if !req.deploy_amount.is_finite() || req.deploy_amount <= 0.0 || req.deploy_amount > 10.0 {
        return Json(serde_json::json!({
            "success": false,
            "error": "deploy_amount must be > 0 and <= 10 (SOL)"
        }));
    }
    if !req.max_tip.is_finite() || req.max_tip < 0.0 || req.max_tip > 1.0 {
        return Json(serde_json::json!({
            "success": false,
            "error": "max_tip must be >= 0 and <= 1 (SOL)"
        }));
    }
    if !req.budget.is_finite() || req.budget <= 0.0 {
        return Json(serde_json::json!({
            "success": false,
            "error": "budget must be > 0 (SOL)"
        }));
    }
    let num_blocks = req.num_blocks.clamp(1, 25);

    // Verify wallet signature for authentication
    // In production, verify the signature against a known message
    
    // Convert f64 to lamports (i64)
    let max_tip_lamports = (req.max_tip * 1_000_000_000.0) as i64;
    let deploy_lamports = (req.deploy_amount * 1_000_000_000.0) as i64;
    let budget_lamports = (req.budget * 1_000_000_000.0) as i64;
    
    match state.db.create_session(
        &req.wallet,
        req.strategy.clone(),
        max_tip_lamports,
        deploy_lamports,
        budget_lamports,
    ).await {
        Ok(session) => {
            // Start the strategy engine for this wallet
            let mut engine = state.strategy_engine.write().await;
            engine.start_session(session.id, req.wallet.clone(), req.strategy, req.deploy_amount, req.max_tip, num_blocks).await;
            
            info!("Started session {} for wallet {}", session.id, req.wallet);
            Json(serde_json::json!({
                "success": true,
                "session_id": session.id
            }))
        }
        Err(e) => {
            Json(serde_json::json!({
                "success": false,
                "error": e.to_string()
            }))
        }
    }
}

/// Stop mining session request
#[derive(Debug, Deserialize)]
pub struct StopSessionRequest {
    pub wallet: String,
}

/// Stop a mining session
async fn stop_session(
    State(state): State<Arc<AppState>>,
    Json(req): Json<StopSessionRequest>,
) -> impl IntoResponse {
    let mut engine = state.strategy_engine.write().await;
    engine.stop_session(&req.wallet).await;
    
    match state.db.end_session(&req.wallet).await {
        Ok(_) => {
            info!("Stopped session for wallet {}", req.wallet);
            Json(serde_json::json!({
                "success": true
            }))
        }
        Err(e) => {
            Json(serde_json::json!({
                "success": false,
                "error": e.to_string()
            }))
        }
    }
}

/// Query parameters for stats
#[derive(Debug, Deserialize)]
pub struct StatsQuery {
    pub wallet: String,
}

/// Get session statistics
async fn get_stats(
    State(state): State<Arc<AppState>>,
    Query(query): Query<StatsQuery>,
) -> impl IntoResponse {
    // First get the active session for the wallet
    match state.db.get_active_session(&query.wallet).await {
        Ok(Some(session)) => {
            match state.db.get_session_stats(session.id).await {
                Ok(stats) => Json(serde_json::json!({
                    "success": true,
                    "stats": stats
                })),
                Err(e) => Json(serde_json::json!({
                    "success": false,
                    "error": e.to_string()
                }))
            }
        }
        Ok(None) => Json(serde_json::json!({
            "success": false,
            "error": "No active session found"
        })),
        Err(e) => Json(serde_json::json!({
            "success": false,
            "error": e.to_string()
        }))
    }
}

/// Query parameters for transactions
#[derive(Debug, Deserialize)]
pub struct TransactionsQuery {
    pub wallet: String,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Get transaction history
async fn get_transactions(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TransactionsQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(50);
    let offset = query.offset.unwrap_or(0);
    
    match state.db.get_transactions(&query.wallet, limit, offset).await {
        Ok(transactions) => Json(serde_json::json!({
            "success": true,
            "transactions": transactions
        })),
        Err(e) => Json(serde_json::json!({
            "success": false,
            "error": e.to_string()
        }))
    }
}

/// Get all balances (wallet + unclaimed)
async fn get_balances(
    State(state): State<Arc<AppState>>,
    Query(query): Query<StatsQuery>,
) -> impl IntoResponse {
    match state.balance_manager.get_all_balances(&query.wallet).await {
        Ok(balances) => Json(serde_json::json!({
            "success": true,
            "wallet": balances.wallet,
            "unclaimed": balances.unclaimed,
            "claimable": balances.claimable,
            "last_synced": balances.last_synced
        })),
        Err(e) => Json(serde_json::json!({
            "success": false,
            "error": e.to_string()
        }))
    }
}

/// Sync request with wallet signature
#[derive(Debug, Deserialize)]
pub struct SyncRequest {
    pub wallet: String,
}

/// Sync balances from on-chain ORE account
async fn sync_balances(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SyncRequest>,
) -> impl IntoResponse {
    match state.balance_manager.sync_from_chain(&req.wallet, &state.db).await {
        Ok(balances) => Json(serde_json::json!({
            "success": true,
            "balances": balances
        })),
        Err(e) => Json(serde_json::json!({
            "success": false,
            "error": e.to_string()
        }))
    }
}

/// Claim request
#[derive(Debug, Deserialize)]
pub struct ClaimRequest {
    pub wallet: String,
    pub amount: Option<f64>, // If None, claim all
}

/// Claim SOL from ORE account (returns transaction for wallet to sign)
async fn claim_sol(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ClaimRequest>,
) -> impl IntoResponse {
    match state.claims_processor.build_claim_sol_tx(&req.wallet, req.amount).await {
        Ok(tx_data) => Json(serde_json::json!({
            "success": true,
            "transaction": tx_data.serialized_tx,
            "gross_amount": tx_data.gross_amount,
            "fee_amount": tx_data.fee_amount,
            "net_amount": tx_data.net_amount
        })),
        Err(e) => Json(serde_json::json!({
            "success": false,
            "error": e.to_string()
        }))
    }
}

/// Claim ORE from account (returns transaction for wallet to sign)
async fn claim_ore(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ClaimRequest>,
) -> impl IntoResponse {
    match state.claims_processor.build_claim_ore_tx(&req.wallet, req.amount).await {
        Ok(tx_data) => Json(serde_json::json!({
            "success": true,
            "transaction": tx_data.serialized_tx,
            "gross_amount": tx_data.gross_amount,
            "fee_amount": tx_data.fee_amount,
            "net_amount": tx_data.net_amount
        })),
        Err(e) => Json(serde_json::json!({
            "success": false,
            "error": e.to_string()
        }))
    }
}

/// Get claims history
async fn get_claims_history(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TransactionsQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(50);
    let offset = query.offset.unwrap_or(0);
    
    match state.db.get_claims(&query.wallet, limit, offset).await {
        Ok(claims) => Json(serde_json::json!({
            "success": true,
            "claims": claims
        })),
        Err(e) => Json(serde_json::json!({
            "success": false,
            "error": e.to_string()
        }))
    }
}

/// Get current ORE grid state (5x5 grid with deployed amounts)
async fn get_grid(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.ore_client.get_current_round_state().await {
        Ok(round) => {
            // Build grid data with EV calculations
            let squares: Vec<serde_json::Value> = round.blocks.iter().map(|block| {
                let deployed_sol = block.total_deployed as f64 / 1_000_000_000.0;
                serde_json::json!({
                    "index": block.index,
                    "deployed": deployed_sol,
                    "miner_count": block.miner_count,
                })
            }).collect();
            
            // Calculate slots remaining and convert to approx seconds
            let slots_remaining = if round.end_slot > round.start_slot && round.end_slot != u64::MAX {
                state.ore_client.get_slots_remaining().await.unwrap_or(0)
            } else {
                0
            };
            let time_remaining = slots_remaining as f64 * 0.4; // ~400ms per slot
            
            Json(serde_json::json!({
                "success": true,
                "round_id": round.round_id,
                "start_slot": round.start_slot,
                "end_slot": round.end_slot,
                "slots_remaining": slots_remaining,
                "time_remaining": time_remaining,
                "total_deployed": round.total_deployed as f64 / 1_000_000_000.0,
                "total_miners": round.total_miners,
                "motherlode": round.motherlode as f64 / 100_000_000_000.0,
                "squares": squares
            }))
        }
        Err(e) => Json(serde_json::json!({
            "success": false,
            "error": e.to_string()
        }))
    }
}

/// Get current round info (lighter endpoint)
async fn get_round(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.ore_client.get_board_state().await {
        Ok(board) => {
            let current_slot = state.ore_client.get_slot().await.unwrap_or(0);
            let slots_remaining = if current_slot < board.end_slot && board.end_slot != u64::MAX {
                board.end_slot - current_slot
            } else {
                0
            };
            
            Json(serde_json::json!({
                "success": true,
                "round_id": board.round_id,
                "start_slot": board.start_slot,
                "end_slot": board.end_slot,
                "current_slot": current_slot,
                "slots_remaining": slots_remaining,
                "time_remaining": slots_remaining as f64 * 0.4
            }))
        }
        Err(e) => Json(serde_json::json!({
            "success": false,
            "error": e.to_string()
        }))
    }
}

/// AI suggestion request
#[derive(Debug, Deserialize)]
pub struct AiSuggestionRequest {
    pub deploy_amount: f64, // SOL per square
    pub tip_amount: f64,    // Jito tip
    pub num_squares: u8,    // How many squares to select
}

/// Get AI-powered square suggestions using OpenRouter
async fn get_ai_suggestion(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AiSuggestionRequest>,
) -> impl IntoResponse {
    // Get current grid state
    let round = match state.ore_client.get_current_round_state().await {
        Ok(r) => r,
        Err(e) => return Json(serde_json::json!({
            "success": false,
            "error": format!("Failed to get round state: {}", e)
        }))
    };
    
    // Calculate EV for each square
    let deploy_lamports = (req.deploy_amount * 1_000_000_000.0) as u64;
    let tip_lamports = (req.tip_amount * 1_000_000_000.0) as u64;
    let total_pot = round.total_deployed;
    
    let mut square_evs: Vec<(u8, f64)> = round.blocks.iter().map(|block| {
        let block_deployed = block.total_deployed;
        let other_squares_pot = total_pot.saturating_sub(block_deployed);
        
        // Win probability = my_stake / (block_total + my_stake)
        let my_new_total = block_deployed + deploy_lamports;
        let win_probability = if my_new_total > 0 {
            deploy_lamports as f64 / my_new_total as f64
        } else {
            1.0 // Empty square, 100% win if we're first
        };
        
        // Expected winnings = probability * pot from other squares
        let expected_winnings = win_probability * other_squares_pot as f64;
        
        // Cost = deploy amount + tip
        let cost = (deploy_lamports + tip_lamports) as f64;
        
        // EV = expected winnings - cost (in lamports)
        let ev = expected_winnings - cost;
        let ev_sol = ev / 1_000_000_000.0;
        
        (block.index, ev_sol)
    }).collect();
    
    // Sort by EV descending
    square_evs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    
    // Select top N squares
    let num_to_select = (req.num_squares as usize).min(25);
    let selected: Vec<serde_json::Value> = square_evs.iter().take(num_to_select).map(|(idx, ev)| {
        let block = &round.blocks[*idx as usize];
        serde_json::json!({
            "square": idx,
            "ev": ev,
            "deployed": block.total_deployed as f64 / 1_000_000_000.0,
            "miner_count": block.miner_count,
            "recommendation": if *ev > 0.0 { "strong_buy" } else if *ev > -req.deploy_amount * 0.1 { "consider" } else { "avoid" }
        })
    }).collect();
    
    // Calculate aggregate stats
    let positive_ev_count = square_evs.iter().filter(|(_, ev)| *ev > 0.0).count();
    let best_ev = square_evs.first().map(|(_, ev)| *ev).unwrap_or(0.0);
    let should_play = best_ev > 0.0 || positive_ev_count >= 3;
    
    Json(serde_json::json!({
        "success": true,
        "round_id": round.round_id,
        "analysis": {
            "total_pot": round.total_deployed as f64 / 1_000_000_000.0,
            "total_miners": round.total_miners,
            "positive_ev_squares": positive_ev_count,
            "best_ev": best_ev,
            "should_play": should_play
        },
        "suggested_squares": selected,
        "strategy": if positive_ev_count >= 10 {
            "Many positive EV squares - spread bets across multiple squares"
        } else if positive_ev_count >= 3 {
            "Some positive EV squares - focus on top 3-5 squares"
        } else if positive_ev_count >= 1 {
            "Limited positive EV - consider single square bet"
        } else {
            "No positive EV squares - consider skipping this round"
        }
    }))
}

/// WebSocket upgrade handler
async fn ws_handler(
    State(state): State<Arc<AppState>>,
    ws: WebSocketUpgrade,
    Query(query): Query<WsAuthQuery>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| ws::handle_socket(socket, state, query.wallet))
}

#[derive(Debug, Deserialize)]
pub struct WsAuthQuery {
    pub wallet: String,
    pub signature: Option<String>,
}

// =============================================================================
// Wallet Management Handlers (for automine)
// =============================================================================

/// Generate a new burner wallet for mining
async fn generate_wallet(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.wallet_manager.generate_burner().await {
        Ok(pubkey) => {
            // Get the private key for backup
            match state.wallet_manager.export_base58(&pubkey).await {
                Ok(private_key) => Json(serde_json::json!({
                    "success": true,
                    "wallet_address": pubkey,
                    "private_key": private_key,
                    "warning": "SAVE THIS PRIVATE KEY! Import into Backpack/Hush to access funds."
                })),
                Err(e) => Json(serde_json::json!({
                    "success": true,
                    "wallet_address": pubkey,
                    "error": format!("Generated but failed to export: {}", e)
                }))
            }
        }
        Err(e) => Json(serde_json::json!({
            "success": false,
            "error": format!("Failed to generate wallet: {}", e)
        }))
    }
}

/// Import an existing wallet for mining
#[derive(Debug, Deserialize)]
pub struct ImportWalletRequest {
    pub private_key: String, // base58 encoded
}

async fn import_wallet(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ImportWalletRequest>,
) -> impl IntoResponse {
    match state.wallet_manager.import_from_base58(&req.private_key).await {
        Ok(pubkey) => {
            // Check balance
            let balance = state.balance_manager.get_sol_balance(&pubkey).await.unwrap_or(0.0);
            Json(serde_json::json!({
                "success": true,
                "wallet_address": pubkey,
                "balance_sol": balance,
                "ready": balance >= 0.01 // Minimum for mining
            }))
        }
        Err(e) => Json(serde_json::json!({
            "success": false,
            "error": format!("Failed to import wallet: {}", e)
        }))
    }
}

/// List all managed wallets
async fn list_wallets(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let wallets = state.wallet_manager.list_wallets().await;
    
    // Get balances for each
    let mut wallet_info = Vec::new();
    for wallet in wallets {
        let balance = state.balance_manager.get_sol_balance(&wallet).await.unwrap_or(0.0);
        wallet_info.push(serde_json::json!({
            "wallet_address": wallet,
            "name": "Mining Wallet",
            "balance_sol": balance,
            "ready": balance >= 0.01
        }));
    }
    
    Json(serde_json::json!({
        "success": true,
        "wallets": wallet_info
    }))
}

/// Export a wallet's private key
#[derive(Debug, Deserialize)]
pub struct ExportWalletRequest {
    pub wallet_address: String,
}

async fn export_wallet(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ExportWalletRequest>,
) -> impl IntoResponse {
    match state.wallet_manager.export_base58(&req.wallet_address).await {
        Ok(private_key) => Json(serde_json::json!({
            "success": true,
            "wallet_address": req.wallet_address,
            "private_key": private_key
        })),
        Err(e) => Json(serde_json::json!({
            "success": false,
            "error": format!("Wallet not found or export failed: {}", e)
        }))
    }
}
