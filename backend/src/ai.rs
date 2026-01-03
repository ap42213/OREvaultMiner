//! AI Strategy Module
//! 
//! Uses OpenRouter API with Gemini 2.0 Flash for real-time block selection.
//! Achieves ~750ms latency for sub-second decisions in final round seconds.

use anyhow::{Result, Context};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::ore::BlockData;

/// OpenRouter API endpoint
const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

/// Gemini 2.0 Flash for ultra-fast mining decisions (~750ms)
/// Fastest model that can make real-time decisions in final seconds
const AI_MODEL: &str = "google/gemini-2.0-flash-001";

/// AI-based strategy selector
#[derive(Clone)]
pub struct AiStrategy {
    client: Client,
    api_key: String,
}

/// Block selection from AI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSelection {
    /// Selected block indices (0-24)
    pub blocks: Vec<u8>,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// Skip this round (all blocks equal)
    pub skip: bool,
    /// AI's reasoning
    pub reasoning: String,
}

/// Grid state for AI analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridState {
    /// Amount deployed per block (25 blocks)
    pub deployed: Vec<u64>,
    /// Number of miners per block
    pub miner_counts: Vec<u64>,
    /// Total pot
    pub total_pot: u64,
    /// Round ID
    pub round_id: u64,
    /// Slots remaining in round
    pub slots_remaining: u64,
    /// Our deploy amount
    pub deploy_amount: u64,
    /// Tip cost
    pub tip_cost: u64,
}

impl AiStrategy {
    /// Create a new AI strategy instance
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
        }
    }
    
    /// Check if AI is configured (has API key)
    pub fn is_configured(&self) -> bool {
        !self.api_key.is_empty()
    }
    
    /// Get AI block selection based on current grid state
    pub async fn select_blocks(
        &self,
        grid: &GridState,
        num_blocks: usize,
        strategy_hint: &str, // "aggressive", "conservative", "best_ev"
    ) -> Result<AiSelection> {
        if !self.is_configured() {
            // Fallback to basic EV calculation if no API key
            return self.fallback_selection(grid, num_blocks);
        }
        
        let prompt = self.build_prompt(grid, num_blocks, strategy_hint);
        
        let response = self.call_openrouter(&prompt).await?;
        
        // Parse AI response
        self.parse_response(&response, num_blocks)
    }
    
    /// Build concise prompt for fast AI response (~750ms target)
    /// Strategy: ALWAYS pick the lowest stake block, never skip
    fn build_prompt(&self, grid: &GridState, _num_blocks: usize, _strategy: &str) -> String {
        // Build sorted list: (index, stake_sol)
        let mut blocks_sorted: Vec<(usize, f64)> = grid.deployed.iter()
            .enumerate()
            .map(|(i, &d)| (i, d as f64 / 1_000_000_000.0))
            .collect();
        blocks_sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        
        // Format block list: "idx:stake"
        let blocks_str: String = blocks_sorted.iter()
            .take(10) // Show lowest 10
            .map(|(i, s)| format!("{}:{:.3}", i, s))
            .collect::<Vec<_>>()
            .join(",");
        
        let lowest_block = blocks_sorted.first().map(|(i, _)| *i).unwrap_or(0);
        
        // Simple prompt - always pick lowest stake
        format!(
            r#"ORE mining: Pick the LOWEST stake block.
Blocks[idx:SOL] sorted by stake: [{}]

ALWAYS pick a block. Never skip. Lowest is block {}.
Reply JSON only: {{"blocks":[{}],"confidence":0.95,"skip":false,"reasoning":"lowest stake"}}"#,
            blocks_str,
            lowest_block,
            lowest_block
        )
    }
    
    /// Call OpenRouter API
    async fn call_openrouter(&self, prompt: &str) -> Result<String> {
        let request_body = serde_json::json!({
            "model": AI_MODEL,
            "messages": [
                {
                    "role": "system",
                    "content": "You are an expert cryptocurrency mining strategist. Always respond with valid JSON only, no other text."
                },
                {
                    "role": "user", 
                    "content": prompt
                }
            ],
            "max_tokens": 300,
            "temperature": 0.3
        });
        
        let response = self.client
            .post(OPENROUTER_API_URL)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://orevault.app")
            .header("X-Title", "OreVault Miner")
            .json(&request_body)
            .send()
            .await
            .context("Failed to call OpenRouter API")?;
        
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("OpenRouter API error {}: {}", status, body);
        }
        
        let json: serde_json::Value = response.json().await
            .context("Failed to parse OpenRouter response")?;
        
        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .context("No content in OpenRouter response")?;
        
        debug!("AI response: {}", content);
        
        Ok(content.to_string())
    }
    
    /// Parse AI response into selection
    fn parse_response(&self, response: &str, num_blocks: usize) -> Result<AiSelection> {
        // Try to extract JSON from response
        let json_str = if response.contains("{") {
            let start = response.find('{').unwrap();
            let end = response.rfind('}').unwrap() + 1;
            &response[start..end]
        } else {
            response
        };
        
        let parsed: serde_json::Value = serde_json::from_str(json_str)
            .context("Failed to parse AI JSON response")?;
        
        let blocks: Vec<u8> = parsed["blocks"]
            .as_array()
            .context("No blocks array in response")?
            .iter()
            .filter_map(|v| v.as_u64().map(|n| n as u8))
            .filter(|&b| b < 25)
            .take(num_blocks)
            .collect();
        
        let confidence = parsed["confidence"].as_f64().unwrap_or(0.5);
        let skip = parsed["skip"].as_bool().unwrap_or(false);
        let reasoning = parsed["reasoning"].as_str().unwrap_or("").to_string();
        
        // If AI says skip, return with skip=true
        if skip {
            info!("AI recommends SKIP: {}", reasoning);
            return Ok(AiSelection {
                blocks: vec![],
                confidence,
                skip: true,
                reasoning,
            });
        }
        
        if blocks.is_empty() {
            anyhow::bail!("AI returned no valid blocks");
        }
        
        info!("AI selected blocks {:?} with confidence {:.2}", blocks, confidence);
        
        Ok(AiSelection {
            blocks,
            confidence,
            skip: false,
            reasoning,
        })
    }
    
    /// Fallback selection using basic EV calculation (no AI)
    fn fallback_selection(&self, grid: &GridState, num_blocks: usize) -> Result<AiSelection> {
        // Calculate average stake
        let total_stake: u64 = grid.deployed.iter().sum();
        let avg_stake = total_stake as f64 / 25.0;
        
        // Find blocks below average stake (lowest = best)
        let mut blocks_below_avg: Vec<(u8, u64)> = grid.deployed.iter()
            .enumerate()
            .filter(|(_, &stake)| (stake as f64) < avg_stake)
            .map(|(i, &stake)| (i as u8, stake))
            .collect();
        
        // Sort by stake ascending (lowest first)
        blocks_below_avg.sort_by_key(|(_, stake)| *stake);
        
        // Check if all blocks have equal stake (skip condition)
        let min_stake = grid.deployed.iter().min().unwrap_or(&0);
        let max_stake = grid.deployed.iter().max().unwrap_or(&0);
        if min_stake == max_stake {
            return Ok(AiSelection {
                blocks: vec![],
                confidence: 1.0,
                skip: true,
                reasoning: "All blocks have equal stake - skipping".to_string(),
            });
        }
        
        // If no blocks below average, skip
        if blocks_below_avg.is_empty() {
            return Ok(AiSelection {
                blocks: vec![],
                confidence: 0.8,
                skip: true,
                reasoning: "No blocks below average stake".to_string(),
            });
        }
        
        // Take lowest stake blocks
        let blocks: Vec<u8> = blocks_below_avg
            .iter()
            .take(num_blocks)
            .map(|(idx, _)| *idx)
            .collect();
        
        let lowest_stake = blocks_below_avg[0].1 as f64 / 1_000_000_000.0;
        
        Ok(AiSelection {
            blocks,
            confidence: 0.8,
            skip: false,
            reasoning: format!("Lowest stake block at {:.4} SOL (avg: {:.4})", lowest_stake, avg_stake / 1_000_000_000.0),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_fallback_selection() {
        let ai = AiStrategy::new(String::new()); // No API key = fallback mode
        
        let grid = GridState {
            deployed: vec![
                1_000_000_000, 500_000_000, 0, 0, 0,  // Row 1
                2_000_000_000, 0, 100_000_000, 0, 0,  // Row 2
                0, 0, 0, 0, 0,                         // Row 3
                0, 0, 0, 0, 0,                         // Row 4
                0, 0, 0, 0, 0,                         // Row 5
            ],
            miner_counts: vec![5, 3, 0, 0, 0, 8, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            total_pot: 10_000_000_000, // 10 SOL
            round_id: 1000,
            slots_remaining: 50,
            deploy_amount: 100_000_000, // 0.1 SOL
            tip_cost: 1_000_000, // 0.001 SOL
        };
        
        let result = ai.fallback_selection(&grid, 3).unwrap();
        
        // Should prefer empty blocks (index 2, 3, 4, etc.)
        assert!(!result.blocks.is_empty());
        println!("Selected blocks: {:?}", result.blocks);
        println!("Reasoning: {}", result.reasoning);
    }
}
