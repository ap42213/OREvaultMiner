//! OreVault Backend - Automated ORE v3 Mining Engine
//! 
//! This is the main entry point for the OreVault mining system.
//! Handles WebSocket connections, REST API, and coordinates mining strategy.

mod balances;
mod claims;
mod db;
mod jito;
mod ore;
mod strategy;
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
use axum_extra::TypedHeader;
use headers::authorization::Bearer;
use headers::Authorization;
use tower_http::cors::{CorsLayer, Any};
use tower_http::trace::TraceLayer;
use tracing::{info, Level};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;

use crate::db::Database;
use crate::ws::WebSocketManager;
use crate::strategy::StrategyEngine;
use crate::balances::BalanceManager;
use crate::claims::ClaimsProcessor;
use crate::ore::OreClient;
use crate::jito::JitoClient;

/// Application state shared across all handlers
pub struct AppState {
    pub db: Database,
    pub ws_manager: WebSocketManager,
    pub strategy_engine: Arc<RwLock<StrategyEngine>>,
    pub balance_manager: BalanceManager,
    pub claims_processor: ClaimsProcessor,
    pub ore_client: OreClient,
    pub jito_client: JitoClient,
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
    let strategy_engine = Arc::new(RwLock::new(
        StrategyEngine::new(ore_client.clone(), jito_client.clone())
    ));
    
    // Create shared application state
    let state = Arc::new(AppState {
        db,
        ws_manager,
        strategy_engine,
        balance_manager,
        claims_processor,
        ore_client,
        jito_client,
    });
    
    // Build router with all API routes
    let app = Router::new()
        // Mining endpoints
        .route("/api/session/start", post(start_session))
        .route("/api/session/stop", post(stop_session))
        .route("/api/stats", get(get_stats))
        .route("/api/transactions", get(get_transactions))
        // Balance & Claims endpoints
        .route("/api/balances", get(get_balances))
        .route("/api/balances/sync", post(sync_balances))
        .route("/api/claim/sol", post(claim_sol))
        .route("/api/claim/ore", post(claim_ore))
        .route("/api/claims/history", get(get_claims_history))
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
    pub signature: String, // Wallet signature for auth
}

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
    // Verify wallet signature for authentication
    // In production, verify the signature against a known message
    
    match state.db.create_session(
        &req.wallet,
        req.strategy.clone(),
        req.max_tip,
        req.deploy_amount,
        req.budget,
    ).await {
        Ok(session) => {
            // Start the strategy engine for this wallet
            let mut engine = state.strategy_engine.write().await;
            engine.start_session(session.id, req.wallet.clone(), req.strategy, req.deploy_amount, req.max_tip).await;
            
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
    pub signature: String,
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
    match state.db.get_session_stats(&query.wallet).await {
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
