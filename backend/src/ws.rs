//! WebSocket Module
//! 
//! Handles real-time communication between frontend and backend.
//! Events: round:update, decision:made, tx:confirmed, balance:update, claim:confirmed

use std::sync::Arc;
use std::collections::HashMap;

use axum::extract::ws::{WebSocket, Message};
use futures_util::{StreamExt, SinkExt};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::AppState;
use crate::strategy::StrategyEvent;

/// WebSocket event types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum WsEvent {
    /// Round update with block data
    #[serde(rename = "round:update")]
    RoundUpdate {
        round_id: u64,
        time_left: f64,
        blocks: Vec<BlockInfo>,
    },
    
    /// AI analysis result
    #[serde(rename = "ai:analysis")]
    AiAnalysis {
        selected_block: u8,
        confidence: f64,
        reasoning: String,
        skip: bool,
    },
    
    /// Decision made (deploy or skip)
    #[serde(rename = "decision:made")]
    DecisionMade {
        action: String,
        block: Option<u8>,
        ev: f64,
        reason: Option<String>,
    },
    
    /// Transaction submitted
    #[serde(rename = "tx:submitted")]
    TxSubmitted {
        signature: String,
        block: u8,
        amount: f64,
    },
    
    /// Transaction confirmed
    #[serde(rename = "tx:confirmed")]
    TxConfirmed {
        signature: String,
        status: String,
        reward: Option<f64>,
    },
    
    /// Balance update
    #[serde(rename = "balance:update")]
    BalanceUpdate {
        unclaimed_sol: f64,
        unclaimed_ore: f64,
        refined_ore: f64,
    },
    
    /// Claim confirmed
    #[serde(rename = "claim:confirmed")]
    ClaimConfirmed {
        claim_type: String,
        net_amount: f64,
        tx_signature: String,
    },
    
    /// Error message
    #[serde(rename = "error")]
    Error {
        message: String,
    },
    
    /// Authentication result
    #[serde(rename = "auth:result")]
    AuthResult {
        success: bool,
        message: String,
    },
}

/// Block info for WebSocket updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockInfo {
    pub index: u8,
    pub total_deployed: f64,
    pub ev: f64,
}

/// Client message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum ClientMessage {
    /// Authenticate with wallet signature
    #[serde(rename = "auth")]
    Auth {
        wallet: String,
        signature: String,
        message: String,
    },
    
    /// Subscribe to updates for a wallet
    #[serde(rename = "subscribe")]
    Subscribe {
        wallet: String,
    },
    
    /// Ping to keep connection alive
    #[serde(rename = "ping")]
    Ping,
    
    /// Request balance sync
    #[serde(rename = "sync:balances")]
    SyncBalances,
}

/// Connected client state
#[derive(Debug)]
struct ConnectedClient {
    id: Uuid,
    wallet: Option<String>,
    authenticated: bool,
}

/// WebSocket connection manager
pub struct WebSocketManager {
    clients: RwLock<HashMap<Uuid, ConnectedClient>>,
}

impl WebSocketManager {
    /// Create a new WebSocket manager
    pub fn new() -> Self {
        Self {
            clients: RwLock::new(HashMap::new()),
        }
    }
    
    /// Register a new client
    pub fn register_client(&self, id: Uuid) {
        let mut clients = self.clients.write();
        clients.insert(id, ConnectedClient {
            id,
            wallet: None,
            authenticated: false,
        });
        debug!("WebSocket client registered: {}", id);
    }
    
    /// Remove a client
    pub fn remove_client(&self, id: &Uuid) {
        let mut clients = self.clients.write();
        clients.remove(id);
        debug!("WebSocket client removed: {}", id);
    }
    
    /// Authenticate a client
    pub fn authenticate_client(&self, id: &Uuid, wallet: String) {
        let mut clients = self.clients.write();
        if let Some(client) = clients.get_mut(id) {
            client.wallet = Some(wallet.clone());
            client.authenticated = true;
            info!("WebSocket client {} authenticated as {}", id, wallet);
        }
    }
    
    /// Check if client is authenticated
    pub fn is_authenticated(&self, id: &Uuid) -> bool {
        let clients = self.clients.read();
        clients.get(id).map(|c| c.authenticated).unwrap_or(false)
    }
    
    /// Get wallet for client
    pub fn get_client_wallet(&self, id: &Uuid) -> Option<String> {
        let clients = self.clients.read();
        clients.get(id).and_then(|c| c.wallet.clone())
    }
    
    /// Get all clients for a wallet
    pub fn get_wallet_clients(&self, wallet: &str) -> Vec<Uuid> {
        let clients = self.clients.read();
        clients.iter()
            .filter(|(_, c)| c.wallet.as_deref() == Some(wallet))
            .map(|(id, _)| *id)
            .collect()
    }
}

/// Handle a WebSocket connection
pub async fn handle_socket(
    socket: WebSocket,
    state: Arc<AppState>,
    wallet: String,
) {
    let client_id = Uuid::new_v4();
    state.ws_manager.register_client(client_id);
    
    // Auto-authenticate if wallet provided in query
    if !wallet.is_empty() {
        state.ws_manager.authenticate_client(&client_id, wallet.clone());
    }
    
    let (mut sender, mut receiver) = socket.split();
    
    // Channel for forwarding events to sender
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(100);
    
    // Subscribe to strategy events
    let mut event_rx = state.strategy_engine.read().await.subscribe();
    
    // Spawn task to forward strategy events via channel
    let wallet_clone = wallet.clone();
    let tx_clone = tx.clone();
    let event_task = tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            // Only forward events for this client's wallet
            let target_wallet = match &event {
                StrategyEvent::RoundUpdate { wallet, .. } => wallet,
                StrategyEvent::AiAnalysis { wallet, .. } => wallet,
                StrategyEvent::DecisionMade { wallet, .. } => wallet,
                StrategyEvent::TxSubmitted { wallet, .. } => wallet,
                StrategyEvent::TxConfirmed { wallet, .. } => wallet,
            };
            
            if target_wallet == &wallet_clone {
                let ws_event = convert_strategy_event(event);
                if let Ok(msg) = serde_json::to_string(&ws_event) {
                    let _ = tx_clone.send(msg).await;
                }
            }
        }
    });
    
    // Spawn task to send messages from channel to WebSocket
    let sender_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });
    
    // Handle incoming messages (need a new sender for responses)
    // Since sender is moved, we'll use the channel for responses too
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                // For client messages, we can send responses via the channel
                if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                    match client_msg {
                        ClientMessage::Auth { wallet, .. } => {
                            state.ws_manager.authenticate_client(&client_id, wallet.clone());
                            let response = WsEvent::AuthResult {
                                success: true,
                                message: "Authenticated successfully".to_string(),
                            };
                            if let Ok(msg) = serde_json::to_string(&response) {
                                let _ = tx.send(msg).await;
                            }
                        }
                        ClientMessage::Subscribe { wallet } => {
                            info!("Client {} subscribed to wallet {}", client_id, wallet);
                        }
                        ClientMessage::Ping => {
                            // Pong handled below
                        }
                        ClientMessage::SyncBalances => {
                            if let Some(w) = state.ws_manager.get_client_wallet(&client_id) {
                                if let Ok(balances) = state.balance_manager.get_all_balances(&w).await {
                                    let response = WsEvent::BalanceUpdate {
                                        unclaimed_sol: balances.unclaimed.sol,
                                        unclaimed_ore: balances.unclaimed.ore,
                                        refined_ore: balances.unclaimed.refined_ore,
                                    };
                                    if let Ok(msg) = serde_json::to_string(&response) {
                                        let _ = tx.send(msg).await;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Ok(Message::Ping(_)) => {
                // Pong is auto-handled by axum
            }
            Ok(Message::Close(_)) => {
                break;
            }
            Err(e) => {
                warn!("WebSocket error: {}", e);
                break;
            }
            _ => {}
        }
    }
    
    // Cleanup
    event_task.abort();
    sender_task.abort();
    state.ws_manager.remove_client(&client_id);
    info!("WebSocket client {} disconnected", client_id);
}

/// Convert strategy event to WebSocket event
fn convert_strategy_event(event: StrategyEvent) -> WsEvent {
    match event {
        StrategyEvent::RoundUpdate { round_id, time_left, blocks, .. } => {
            WsEvent::RoundUpdate {
                round_id,
                time_left,
                blocks: blocks.into_iter().map(|b| BlockInfo {
                    index: b.index,
                    total_deployed: b.total_deployed as f64 / 1_000_000_000.0,
                    ev: b.ev / 1_000_000_000.0,
                }).collect(),
            }
        }
        StrategyEvent::AiAnalysis { selected_block, confidence, reasoning, skip, .. } => {
            WsEvent::AiAnalysis {
                selected_block,
                confidence,
                reasoning,
                skip,
            }
        }
        StrategyEvent::DecisionMade { decision, .. } => {
            match decision {
                crate::strategy::RoundDecision::Deploy { block_index, expected_ev, .. } => {
                    WsEvent::DecisionMade {
                        action: "deploy".to_string(),
                        block: Some(block_index),
                        ev: expected_ev / 1_000_000_000.0,
                        reason: None,
                    }
                }
                crate::strategy::RoundDecision::Skip { reason, best_ev } => {
                    WsEvent::DecisionMade {
                        action: "skip".to_string(),
                        block: None,
                        ev: best_ev / 1_000_000_000.0,
                        reason: Some(reason),
                    }
                }
            }
        }
        StrategyEvent::TxSubmitted { signature, block_index, amount, .. } => {
            WsEvent::TxSubmitted {
                signature,
                block: block_index,
                amount: amount as f64 / 1_000_000_000.0,
            }
        }
        StrategyEvent::TxConfirmed { signature, status, reward, .. } => {
            WsEvent::TxConfirmed {
                signature,
                status,
                reward: reward.map(|r| r as f64 / 1_000_000_000.0),
            }
        }
    }
}

/// Broadcast event to all clients for a wallet
pub async fn broadcast_to_wallet(
    ws_manager: &WebSocketManager,
    wallet: &str,
    event: WsEvent,
) {
    let clients = ws_manager.get_wallet_clients(wallet);
    let msg = serde_json::to_string(&event).unwrap();
    
    // In a full implementation, we'd maintain sender handles
    // and broadcast to all connected clients
    debug!("Broadcasting to {} clients for wallet {}", clients.len(), wallet);
}
