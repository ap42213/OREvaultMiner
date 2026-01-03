//! Wallet Management Module
//!
//! Handles server-side keypair storage for automated signing.
//! Wallets are stored in Supabase for persistence across restarts.
//!
//! Security Notes:
//! - Private keys stored in database (should encrypt in production)
//! - Use burner wallets with limited funds
//! - Keep main wallet separate in Phantom

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Result, Context};
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer},
    transaction::Transaction,
};
use tokio::sync::RwLock;
use tracing::{info, warn, error};

use crate::db::Database;

/// Wallet manager for server-side signing
/// Caches keypairs in memory, persists to Supabase
pub struct WalletManager {
    /// In-memory cache (pubkey -> keypair)
    keypairs: Arc<RwLock<HashMap<String, Keypair>>>,
    /// Database connection for persistence
    db: Option<Database>,
}

impl WalletManager {
    /// Create a new wallet manager (memory only)
    pub fn new() -> Self {
        Self {
            keypairs: Arc::new(RwLock::new(HashMap::new())),
            db: None,
        }
    }
    
    /// Create wallet manager with database persistence
    pub fn with_database(db: Database) -> Self {
        Self {
            keypairs: Arc::new(RwLock::new(HashMap::new())),
            db: Some(db),
        }
    }
    
    /// Load all active wallets from database into memory
    pub async fn load_from_database(&self) -> Result<usize> {
        let db = self.db.as_ref().context("No database configured")?;
        
        let wallet_infos = db.list_wallets().await?;
        let mut loaded = 0;
        
        for info in wallet_infos {
            if let Ok(Some(record)) = db.get_wallet(&info.wallet_address).await {
                if self.import_from_base58_internal(&record.private_key_b58, false).await.is_ok() {
                    loaded += 1;
                }
            }
        }
        
        info!("Loaded {} wallets from database", loaded);
        Ok(loaded)
    }
    
    /// Generate a new burner wallet for mining
    pub async fn generate_burner(&self) -> Result<String> {
        let keypair = Keypair::new();
        let pubkey = keypair.pubkey().to_string();
        let private_key_b58 = bs58::encode(keypair.to_bytes()).into_string();
        
        // Store in memory
        {
            let mut keypairs = self.keypairs.write().await;
            keypairs.insert(pubkey.clone(), keypair);
        }
        
        // Persist to database
        if let Some(ref db) = self.db {
            db.save_wallet(&pubkey, &private_key_b58, None).await?;
        }
        
        info!("Generated new mining wallet: {}", pubkey);
        Ok(pubkey)
    }
    
    /// Import a keypair from base58 private key (with DB save)
    pub async fn import_from_base58(&self, private_key: &str) -> Result<String> {
        self.import_from_base58_internal(private_key, true).await
    }
    
    /// Internal import (optionally save to DB)
    async fn import_from_base58_internal(&self, private_key: &str, save_to_db: bool) -> Result<String> {
        let bytes = bs58::decode(private_key)
            .into_vec()
            .context("Invalid base58 private key")?;
        
        let keypair = Keypair::from_bytes(&bytes)
            .context("Invalid keypair bytes")?;
        
        let pubkey = keypair.pubkey().to_string();
        
        // Store in memory
        {
            let mut keypairs = self.keypairs.write().await;
            keypairs.insert(pubkey.clone(), keypair);
        }
        
        // Persist to database
        if save_to_db {
            if let Some(ref db) = self.db {
                db.save_wallet(&pubkey, private_key, None).await?;
            }
        }
        
        info!("Imported wallet: {}", pubkey);
        Ok(pubkey)
    }
    
    /// Import a keypair from JSON file (Solana CLI format)
    pub async fn import_from_file(&self, path: &Path) -> Result<String> {
        let contents = std::fs::read_to_string(path)
            .context("Failed to read keypair file")?;
        
        let bytes: Vec<u8> = serde_json::from_str(&contents)
            .context("Invalid keypair JSON format")?;
        
        let keypair = Keypair::from_bytes(&bytes)
            .context("Invalid keypair bytes")?;
        
        let pubkey = keypair.pubkey().to_string();
        let private_key_b58 = bs58::encode(keypair.to_bytes()).into_string();
        
        // Store in memory
        {
            let mut keypairs = self.keypairs.write().await;
            keypairs.insert(pubkey.clone(), keypair);
        }
        
        // Persist to database
        if let Some(ref db) = self.db {
            let name = path.file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string());
            db.save_wallet(&pubkey, &private_key_b58, name.as_deref()).await?;
        }
        
        info!("Imported wallet from file: {}", pubkey);
        Ok(pubkey)
    }
    
    /// Export keypair as base58 (for backup)
    pub async fn export_base58(&self, pubkey: &str) -> Result<String> {
        // Try memory first
        {
            let keypairs = self.keypairs.read().await;
            if let Some(keypair) = keypairs.get(pubkey) {
                return Ok(bs58::encode(keypair.to_bytes()).into_string());
            }
        }
        
        // Try database
        if let Some(ref db) = self.db {
            if let Some(record) = db.get_wallet(pubkey).await? {
                let _ = self.import_from_base58_internal(&record.private_key_b58, false).await;
                return Ok(record.private_key_b58);
            }
        }
        
        anyhow::bail!("Wallet not found: {}", pubkey)
    }
    
    /// Check if we have a keypair for this wallet
    pub async fn has_keypair(&self, pubkey: &str) -> bool {
        // Check memory
        {
            let keypairs = self.keypairs.read().await;
            if keypairs.contains_key(pubkey) {
                return true;
            }
        }
        
        // Check database and load if found
        if let Some(ref db) = self.db {
            if let Ok(Some(record)) = db.get_wallet(pubkey).await {
                if self.import_from_base58_internal(&record.private_key_b58, false).await.is_ok() {
                    return true;
                }
            }
        }
        
        false
    }
    
    /// Get the public key for a stored keypair
    pub async fn get_pubkey(&self, pubkey: &str) -> Result<Pubkey> {
        if !self.has_keypair(pubkey).await {
            anyhow::bail!("Wallet not found: {}", pubkey);
        }
        
        let keypairs = self.keypairs.read().await;
        let keypair = keypairs.get(pubkey).context("Wallet not found")?;
        Ok(keypair.pubkey())
    }
    
    /// Sign a transaction with stored keypair
    pub async fn sign_transaction(&self, pubkey: &str, tx: &mut Transaction) -> Result<()> {
        if !self.has_keypair(pubkey).await {
            anyhow::bail!("Wallet not found: {}", pubkey);
        }
        
        let keypairs = self.keypairs.read().await;
        let keypair = keypairs.get(pubkey).context("Wallet not found")?;
        
        tx.try_sign(&[keypair], tx.message.recent_blockhash)
            .context("Failed to sign transaction")?;
        
        if let Some(ref db) = self.db {
            let _ = db.touch_wallet(pubkey).await;
        }
        
        Ok(())
    }
    
    /// Sign and return signature
    pub async fn sign_message(&self, pubkey: &str, message: &[u8]) -> Result<Signature> {
        if !self.has_keypair(pubkey).await {
            anyhow::bail!("Wallet not found: {}", pubkey);
        }
        
        let keypairs = self.keypairs.read().await;
        let keypair = keypairs.get(pubkey).context("Wallet not found")?;
        
        Ok(keypair.sign_message(message))
    }
    
    /// List all managed wallets
    pub async fn list_wallets(&self) -> Vec<String> {
        let keypairs = self.keypairs.read().await;
        keypairs.keys().cloned().collect()
    }
    
    /// Remove a wallet from management
    pub async fn remove_wallet(&self, pubkey: &str) -> bool {
        let removed = {
            let mut keypairs = self.keypairs.write().await;
            keypairs.remove(pubkey).is_some()
        };
        
        if let Some(ref db) = self.db {
            let _ = db.deactivate_wallet(pubkey).await;
        }
        
        removed
    }
}

impl Default for WalletManager {
    fn default() -> Self {
        Self::new()
    }
}
