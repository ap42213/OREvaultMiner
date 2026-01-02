'use client';

import { useState } from 'react';
import { useWallet } from '@solana/wallet-adapter-react';

type Strategy = 'best_ev' | 'conservative' | 'aggressive';

interface BetConfigState {
  strategy: Strategy;
  deployAmount: string;
  maxTip: string;
  budget: string;
}

/**
 * BetConfig Component
 * 
 * Configuration for automated mining:
 * - Strategy selection (Best EV, Conservative, Aggressive)
 * - Deploy amount per round
 * - Maximum tip for Jito
 * - Total budget limit
 */
export function BetConfig() {
  const { publicKey, connected, signMessage } = useWallet();
  const [config, setConfig] = useState<BetConfigState>({
    strategy: 'best_ev',
    deployAmount: '0.1',
    maxTip: '0.001',
    budget: '1.0',
  });
  const [isRunning, setIsRunning] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!connected) return null;

  const handleStart = async () => {
    if (!publicKey || !signMessage) return;
    
    setLoading(true);
    setError(null);

    try {
      // Sign message for authentication
      const message = new TextEncoder().encode(
        `OreVault: Start mining session at ${Date.now()}`
      );
      const signature = await signMessage(message);
      const signatureBase64 = Buffer.from(signature).toString('base64');

      // Start session
      const response = await fetch(`${process.env.NEXT_PUBLIC_API_URL}/api/session/start`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          wallet: publicKey.toBase58(),
          strategy: config.strategy,
          deploy_amount: parseFloat(config.deployAmount),
          max_tip: parseFloat(config.maxTip),
          budget: parseFloat(config.budget),
          signature: signatureBase64,
        }),
      });

      const data = await response.json();
      if (!data.success) {
        throw new Error(data.error || 'Failed to start session');
      }

      setIsRunning(true);
    } catch (e: any) {
      setError(e.message || 'Failed to start');
    } finally {
      setLoading(false);
    }
  };

  const handleStop = async () => {
    if (!publicKey || !signMessage) return;

    setLoading(true);
    setError(null);

    try {
      const message = new TextEncoder().encode(
        `OreVault: Stop mining session at ${Date.now()}`
      );
      const signature = await signMessage(message);
      const signatureBase64 = Buffer.from(signature).toString('base64');

      const response = await fetch(`${process.env.NEXT_PUBLIC_API_URL}/api/session/stop`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          wallet: publicKey.toBase58(),
          signature: signatureBase64,
        }),
      });

      const data = await response.json();
      if (!data.success) {
        throw new Error(data.error || 'Failed to stop session');
      }

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

      {/* Deploy Amount */}
      <div className="mb-4">
        <label className="block text-sm text-muted mb-2">Deploy Amount (SOL)</label>
        <input
          type="number"
          step="0.01"
          min="0.01"
          value={config.deployAmount}
          onChange={(e) => setConfig(c => ({ ...c, deployAmount: e.target.value }))}
          disabled={isRunning}
          className="w-full bg-surface-light border border-border rounded-lg py-2 px-3 font-mono focus:outline-none focus:border-primary disabled:opacity-50"
        />
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
