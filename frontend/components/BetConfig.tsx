'use client';

import { useState } from 'react';
import { useOreVaultStore } from '@/lib/store';
import { startSession, stopSession } from '@/lib/api';

type Strategy = 'best_ev' | 'conservative' | 'aggressive';

interface BetConfigState {
  strategy: Strategy;
  deployAmount: string;
  maxTip: string;
  budget: string;
  numBlocks: number;
}

/**
 * BetConfig Component
 * 
 * Configuration for automated mining:
 * - Strategy selection (Best EV, Conservative, Aggressive)
 * - Deploy amount per round
 * - Maximum tip for Jito
 * - Total budget limit
 * 
 * No wallet signing needed - backend handles everything
 */
export function BetConfig() {
  const { miningWallet, miningWalletLoading, isRunning, setIsRunning } = useOreVaultStore();
  const [config, setConfig] = useState<BetConfigState>({
    strategy: 'conservative',  // Default to lowest stake block
    deployAmount: '0.1',
    maxTip: '0.001',
    budget: '1.0',
    numBlocks: 1,
  });
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (miningWalletLoading) {
    return (
      <div className="bg-surface rounded-lg border border-border p-6">
        <div className="animate-pulse">
          <div className="h-6 bg-surface-light rounded w-1/3 mb-4" />
          <div className="h-10 bg-surface-light rounded mb-4" />
          <div className="h-10 bg-surface-light rounded mb-4" />
          <div className="h-10 bg-surface-light rounded" />
        </div>
      </div>
    );
  }

  if (!miningWallet) {
    return (
      <div className="bg-surface rounded-lg border border-border p-6">
        <h2 className="text-lg font-semibold mb-4">Mining Configuration</h2>
        <p className="text-muted">No mining wallet configured.</p>
      </div>
    );
  }

  const handleStart = async () => {
    setLoading(true);
    setError(null);

    try {
      await startSession({
        wallet: miningWallet,
        strategy: config.strategy,
        deploy_amount: parseFloat(config.deployAmount),
        max_tip: parseFloat(config.maxTip),
        budget: parseFloat(config.budget),
        num_blocks: config.numBlocks,
      });

      setIsRunning(true);
    } catch (e: any) {
      setError(e.message || 'Failed to start');
    } finally {
      setLoading(false);
    }
  };

  const handleStop = async () => {
    setLoading(true);
    setError(null);

    try {
      await stopSession({ wallet: miningWallet });
      setIsRunning(false);
    } catch (e: any) {
      setError(e.message || 'Failed to stop');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="bg-surface rounded-lg border border-border p-6">
      <h2 className="text-lg font-semibold mb-4">Mining Configuration</h2>

      {/* Strategy Selection */}
      <div className="mb-4">
        <label className="block text-sm text-muted mb-2">Strategy</label>
        <div className="grid grid-cols-3 gap-2">
          {[
            { value: 'best_ev', label: 'Best EV' },
            { value: 'conservative', label: 'Conservative' },
            { value: 'aggressive', label: 'Aggressive' },
          ].map(({ value, label }) => (
            <button
              key={value}
              onClick={() => setConfig(c => ({ ...c, strategy: value as Strategy }))}
              disabled={isRunning}
              className={`py-2 px-3 rounded text-sm transition-colors ${
                config.strategy === value
                  ? 'bg-primary text-black font-medium'
                  : 'bg-surface-light hover:bg-border text-white'
              } disabled:opacity-50`}
            >
              {label}
            </button>
          ))}
        </div>
      </div>

      {/* Number of Blocks */}
      <div className="mb-4">
        <label className="block text-sm text-muted mb-2">Blocks to Bet (1-25)</label>
        <input
          type="number"
          min="1"
          max="25"
          step="1"
          value={config.numBlocks}
          onChange={(e) => {
            const val = Math.min(25, Math.max(1, parseInt(e.target.value) || 1));
            setConfig(c => ({ ...c, numBlocks: val }));
          }}
          disabled={isRunning}
          className="w-full bg-surface-light border border-border rounded-lg py-2 px-3 font-mono focus:outline-none focus:border-primary disabled:opacity-50"
        />
        <p className="text-xs text-muted mt-1">More blocks = higher chance to win, but stake split across them</p>
      </div>

      {/* Deploy Amount */}
      <div className="mb-4">
        <label className="block text-sm text-muted mb-2">Deploy Amount per Block (SOL)</label>
        <input
          type="number"
          step="0.0001"
          min="0.0001"
          value={config.deployAmount}
          onChange={(e) => setConfig(c => ({ ...c, deployAmount: e.target.value }))}
          disabled={isRunning}
          className="w-full bg-surface-light border border-border rounded-lg py-2 px-3 font-mono focus:outline-none focus:border-primary disabled:opacity-50"
        />
        <p className="text-xs text-muted mt-1">Amount deployed per block each round (min 0.0001 SOL = 100,000 lamports)</p>
      </div>

      {/* Max Tip */}
      <div className="mb-4">
        <label className="block text-sm text-muted mb-2">Max Jito Tip (SOL)</label>
        <input
          type="number"
          step="0.0001"
          min="0.0001"
          value={config.maxTip}
          onChange={(e) => setConfig(c => ({ ...c, maxTip: e.target.value }))}
          disabled={isRunning}
          className="w-full bg-surface-light border border-border rounded-lg py-2 px-3 font-mono focus:outline-none focus:border-primary disabled:opacity-50"
        />
      </div>

      {/* Budget */}
      <div className="mb-6">
        <label className="block text-sm text-muted mb-2">Total Budget (SOL)</label>
        <input
          type="number"
          step="0.1"
          min="0.1"
          value={config.budget}
          onChange={(e) => setConfig(c => ({ ...c, budget: e.target.value }))}
          disabled={isRunning}
          className="w-full bg-surface-light border border-border rounded-lg py-2 px-3 font-mono focus:outline-none focus:border-primary disabled:opacity-50"
        />
      </div>

      {/* Error */}
      {error && (
        <p className="text-danger text-sm mb-4">{error}</p>
      )}

      {/* Start/Stop Button */}
      <button
        onClick={isRunning ? handleStop : handleStart}
        disabled={loading}
        className={`w-full py-3 rounded-lg font-medium transition-colors ${
          isRunning
            ? 'bg-danger hover:bg-danger/80 text-white'
            : 'bg-primary hover:bg-primary/80 text-black'
        } disabled:opacity-50`}
      >
        {loading ? 'Processing...' : isRunning ? 'Stop Mining' : 'Start Mining'}
      </button>

      {/* Status Indicator */}
      {isRunning && (
        <div className="mt-4 flex items-center gap-2">
          <div className="w-2 h-2 bg-primary rounded-full animate-pulse" />
          <span className="text-sm text-primary">Mining active</span>
        </div>
      )}

      {/* Warning */}
      <p className="text-xs text-warning mt-4">
        Real SOL transactions on Solana mainnet. Start with small amounts.
      </p>
    </div>
  );
}
