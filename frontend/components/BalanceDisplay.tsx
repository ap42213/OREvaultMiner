'use client';

import { useState } from 'react';
import { useOreVaultStore } from '@/lib/store';
import useSWR from 'swr';

interface Balances {
  wallet: {
    sol: number;
    ore: number;
  };
  unclaimed: {
    sol: number;
    ore: number;
    refined_ore: number;
  };
  claimable: {
    sol: number;
    ore: number;
  };
  last_synced: string;
}

const fetcher = async (url: string) => {
  const res = await fetch(url);
  if (!res.ok) throw new Error('Failed to fetch');
  const data = await res.json();
  if (!data.success) throw new Error(data.error);
  return data;
};

/**
 * BalanceDisplay Component
 * 
 * Shows wallet AND unclaimed ORE account balances
 */
export function BalanceDisplay() {
  const { miningWallet, miningWalletLoading } = useOreVaultStore();
  const [syncing, setSyncing] = useState(false);

  const { data, error, mutate } = useSWR<Balances>(
    miningWallet 
      ? `${process.env.NEXT_PUBLIC_API_URL}/api/balances?wallet=${miningWallet}`
      : null,
    fetcher,
    { refreshInterval: 10000 }
  );

  const handleSync = async () => {
    if (!miningWallet) return;
    setSyncing(true);
    try {
      await fetch(`${process.env.NEXT_PUBLIC_API_URL}/api/balances/sync`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ wallet: miningWallet }),
      });
      mutate();
    } catch (e) {
      console.error('Sync failed:', e);
    } finally {
      setSyncing(false);
    }
  };

  if (miningWalletLoading) {
    return (
      <div className="bg-surface rounded-lg border border-border p-6">
        <div className="animate-pulse">
          <div className="h-6 bg-surface-light rounded w-1/3 mb-4" />
          <div className="h-20 bg-surface-light rounded" />
        </div>
      </div>
    );
  }

  if (!miningWallet) return null;

  const balances = data as Balances | undefined;

  return (
    <div className="bg-surface rounded-lg border border-border p-6">
      <div className="flex justify-between items-center mb-4">
        <h2 className="text-lg font-semibold">Balances</h2>
        <button
          onClick={handleSync}
          disabled={syncing}
          className="text-sm text-muted hover:text-white transition-colors disabled:opacity-50"
        >
          {syncing ? 'Syncing...' : 'Sync'}
        </button>
      </div>

      {error ? (
        <p className="text-danger text-sm">Failed to load balances</p>
      ) : !balances ? (
        <p className="text-muted text-sm">Loading...</p>
      ) : (
        <div className="grid grid-cols-2 gap-6">
          {/* Wallet Balances */}
          <div>
            <h3 className="text-xs uppercase tracking-wide text-muted mb-3">Wallet</h3>
            <div className="space-y-2">
              <div className="flex justify-between">
                <span className="text-muted">SOL</span>
                <span className="font-mono">{balances.wallet.sol.toFixed(4)}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-muted">ORE</span>
                <span className="font-mono">{balances.wallet.ore.toFixed(2)}</span>
              </div>
            </div>
          </div>

          {/* Unclaimed Balances (In ORE Account) */}
          <div>
            <h3 className="text-xs uppercase tracking-wide text-muted mb-3">In Account</h3>
            <div className="space-y-2">
              <div className="flex justify-between">
                <span className="text-muted">SOL</span>
                <span className="font-mono text-primary">
                  {balances.unclaimed.sol.toFixed(4)}
                </span>
              </div>
              <div className="flex justify-between">
                <span className="text-muted">ORE</span>
                <span className="font-mono text-primary">
                  {balances.unclaimed.ore.toFixed(2)}
                </span>
              </div>
              <div className="flex justify-between">
                <span className="text-muted">Refined</span>
                <span className="font-mono text-yellow-500">
                  {balances.unclaimed.refined_ore.toFixed(2)}
                </span>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Claimable Summary */}
      {balances && (balances.unclaimed.sol > 0 || balances.unclaimed.ore > 0) && (
        <div className="mt-4 pt-4 border-t border-border">
          <p className="text-xs text-muted">
            Claimable (after 10% fee): {balances.claimable.sol.toFixed(4)} SOL / {balances.claimable.ore.toFixed(2)} ORE
          </p>
        </div>
      )}

      {/* Last Synced */}
      {balances?.last_synced && (
        <p className="text-xs text-muted mt-2">
          Last synced: {new Date(balances.last_synced).toLocaleTimeString()}
        </p>
      )}
    </div>
  );
}
