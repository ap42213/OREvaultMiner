'use client';

import { useOreVaultStore } from '@/lib/store';
import useSWR from 'swr';

interface SessionStats {
  rounds_played: number;
  rounds_skipped: number;
  rounds_won: number;
  rounds_lost: number;
  total_deployed: number;
  total_tips: number;
  total_won: number;
  net_pnl: number;
  win_rate: number;
}

const fetcher = async (url: string) => {
  const res = await fetch(url);
  if (!res.ok) throw new Error('Failed to fetch');
  const data = await res.json();
  if (!data.success) throw new Error(data.error);
  return data.stats;
};

/**
 * Stats Component
 * 
 * Displays mining statistics
 */
export function Stats() {
  const { miningWallet, miningWalletLoading } = useOreVaultStore();

  const { data: stats, error } = useSWR<SessionStats>(
    miningWallet
      ? `${process.env.NEXT_PUBLIC_API_URL}/api/stats?wallet=${miningWallet}`
      : null,
    fetcher,
    { refreshInterval: 5000 }
  );

  if (miningWalletLoading) {
    return (
      <div className="bg-surface rounded-lg border border-border p-6">
        <div className="animate-pulse">
          <div className="h-6 bg-surface-light rounded w-1/3 mb-4" />
          <div className="h-32 bg-surface-light rounded" />
        </div>
      </div>
    );
  }

  if (!miningWallet) return null;

  return (
    <div className="bg-surface rounded-lg border border-border p-6">
      <h2 className="text-lg font-semibold mb-4">Statistics</h2>

      {error ? (
        <p className="text-danger text-sm">Failed to load stats</p>
      ) : !stats ? (
        <p className="text-muted text-sm">No session data yet</p>
      ) : (
        <div className="space-y-6">
          {/* Rounds */}
          <div>
            <h3 className="text-xs uppercase tracking-wide text-muted mb-3">Rounds</h3>
            <div className="grid grid-cols-2 gap-4">
              <div>
                <p className="text-2xl font-mono">{stats.rounds_played}</p>
                <p className="text-xs text-muted">Played</p>
              </div>
              <div>
                <p className="text-2xl font-mono text-muted">{stats.rounds_skipped}</p>
                <p className="text-xs text-muted">Skipped</p>
              </div>
            </div>
          </div>

          {/* Win/Loss */}
          <div>
            <h3 className="text-xs uppercase tracking-wide text-muted mb-3">Results</h3>
            <div className="grid grid-cols-3 gap-4">
              <div>
                <p className="text-2xl font-mono text-primary">{stats.rounds_won}</p>
                <p className="text-xs text-muted">Wins</p>
              </div>
              <div>
                <p className="text-2xl font-mono text-danger">{stats.rounds_lost}</p>
                <p className="text-xs text-muted">Losses</p>
              </div>
              <div>
                <p className="text-2xl font-mono">{stats.win_rate.toFixed(1)}%</p>
                <p className="text-xs text-muted">Win Rate</p>
              </div>
            </div>
          </div>

          {/* Amounts */}
          <div>
            <h3 className="text-xs uppercase tracking-wide text-muted mb-3">SOL Amounts</h3>
            <div className="space-y-2">
              <div className="flex justify-between">
                <span className="text-muted">Deployed</span>
                <span className="font-mono">{stats.total_deployed.toFixed(4)}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-muted">Tips</span>
                <span className="font-mono text-muted">{stats.total_tips.toFixed(4)}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-muted">Won</span>
                <span className="font-mono text-primary">{stats.total_won.toFixed(4)}</span>
              </div>
            </div>
          </div>

          {/* Net P&L */}
          <div className="pt-4 border-t border-border">
            <div className="flex justify-between items-center">
              <span className="font-medium">Net P&L</span>
              <span className={`text-2xl font-mono font-medium ${
                stats.net_pnl >= 0 ? 'text-primary' : 'text-danger'
              }`}>
                {stats.net_pnl >= 0 ? '+' : ''}{stats.net_pnl.toFixed(4)} SOL
              </span>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
