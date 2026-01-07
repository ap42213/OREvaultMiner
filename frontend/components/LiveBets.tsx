'use client';

import { useEffect, useRef } from 'react';
import { useOreVaultStore } from '@/lib/store';

/**
 * LiveBets Component
 * 
 * Real-time feed showing bet outcomes as they happen.
 * Shows: pending bets, wins (green), losses (red)
 * Uses existing tx:submitted and tx:confirmed WebSocket events.
 */
export function LiveBets() {
  const { bets, miningWallet, isRunning } = useOreVaultStore();
  const containerRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to top when new bets come in
  useEffect(() => {
    if (containerRef.current && bets.length > 0) {
      containerRef.current.scrollTop = 0;
    }
  }, [bets.length]);

  if (!miningWallet) return null;

  const formatTime = (timestamp: number) => {
    const date = new Date(timestamp);
    return date.toLocaleTimeString('en-US', { 
      hour: '2-digit', 
      minute: '2-digit', 
      second: '2-digit',
      hour12: false 
    });
  };

  const getStatusIcon = (status: 'pending' | 'won' | 'lost') => {
    switch (status) {
      case 'pending':
        return (
          <div className="w-4 h-4 border-2 border-yellow-400 border-t-transparent rounded-full animate-spin" />
        );
      case 'won':
        return (
          <div className="w-4 h-4 bg-green-500 rounded-full flex items-center justify-center animate-pulse-once">
            <svg className="w-2.5 h-2.5 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={3} d="M5 13l4 4L19 7" />
            </svg>
          </div>
        );
      case 'lost':
        return (
          <div className="w-4 h-4 bg-red-500 rounded-full flex items-center justify-center">
            <svg className="w-2.5 h-2.5 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={3} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </div>
        );
    }
  };

  const getStatusColor = (status: 'pending' | 'won' | 'lost') => {
    switch (status) {
      case 'pending': return 'bg-yellow-500/10 border-yellow-500/30';
      case 'won': return 'bg-green-500/10 border-green-500/30';
      case 'lost': return 'bg-red-500/10 border-red-500/30';
    }
  };

  const getPnL = (bet: typeof bets[0]) => {
    if (bet.status === 'pending') return null;
    if (bet.status === 'won' && bet.reward) {
      const profit = bet.reward - bet.amount;
      return { value: profit, display: `+${profit.toFixed(4)}` };
    }
    return { value: -bet.amount, display: `-${bet.amount.toFixed(4)}` };
  };

  // Calculate session stats from bets
  const sessionStats = {
    total: bets.filter(b => b.status !== 'pending').length,
    wins: bets.filter(b => b.status === 'won').length,
    losses: bets.filter(b => b.status === 'lost').length,
    pending: bets.filter(b => b.status === 'pending').length,
    netPnL: bets.reduce((acc, bet) => {
      if (bet.status === 'won' && bet.reward) {
        return acc + (bet.reward - bet.amount);
      } else if (bet.status === 'lost') {
        return acc - bet.amount;
      }
      return acc;
    }, 0),
  };

  return (
    <div className="bg-surface rounded-lg border border-border p-6">
      <div className="flex justify-between items-center mb-4">
        <h2 className="text-lg font-semibold flex items-center gap-2">
          Live Bets
          {isRunning && (
            <span className="relative flex h-2 w-2">
              <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-green-400 opacity-75"></span>
              <span className="relative inline-flex rounded-full h-2 w-2 bg-green-500"></span>
            </span>
          )}
        </h2>
        {sessionStats.total > 0 && (
          <div className="text-xs text-muted">
            {sessionStats.wins}W / {sessionStats.losses}L
            {sessionStats.pending > 0 && ` (${sessionStats.pending} pending)`}
          </div>
        )}
      </div>

      {/* Quick Stats Bar */}
      {sessionStats.total > 0 && (
        <div className="grid grid-cols-3 gap-2 mb-4 p-3 bg-surface-light rounded-lg">
          <div className="text-center">
            <p className="text-lg font-mono text-green-400">{sessionStats.wins}</p>
            <p className="text-[10px] uppercase text-muted">Wins</p>
          </div>
          <div className="text-center border-x border-border">
            <p className="text-lg font-mono text-red-400">{sessionStats.losses}</p>
            <p className="text-[10px] uppercase text-muted">Losses</p>
          </div>
          <div className="text-center">
            <p className={`text-lg font-mono ${sessionStats.netPnL >= 0 ? 'text-green-400' : 'text-red-400'}`}>
              {sessionStats.netPnL >= 0 ? '+' : ''}{sessionStats.netPnL.toFixed(4)}
            </p>
            <p className="text-[10px] uppercase text-muted">Net SOL</p>
          </div>
        </div>
      )}

      {/* Bets Feed */}
      <div 
        ref={containerRef}
        className="space-y-2 max-h-[300px] overflow-y-auto pr-1"
      >
        {bets.length === 0 ? (
          <div className="text-center py-8 text-muted">
            <p className="text-sm">No bets yet</p>
            <p className="text-xs mt-1">
              {isRunning ? 'Waiting for next round...' : 'Start mining to see bets'}
            </p>
          </div>
        ) : (
          bets.map((bet, index) => {
            const pnl = getPnL(bet);
            const isNew = index === 0 && bet.status !== 'pending';
            
            return (
              <div
                key={bet.signature}
                className={`
                  p-3 rounded-lg border transition-all duration-300
                  ${getStatusColor(bet.status)}
                  ${isNew ? 'animate-flash' : ''}
                `}
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-3">
                    {getStatusIcon(bet.status)}
                    <div>
                      <div className="flex items-center gap-2">
                        <span className="font-mono text-sm">Block {bet.block + 1}</span>
                        <span className="text-xs text-muted">•</span>
                        <span className="text-xs text-muted font-mono">
                          {bet.amount.toFixed(4)} SOL
                        </span>
                      </div>
                      <div className="text-[10px] text-muted mt-0.5">
                        {formatTime(bet.timestamp)}
                        {bet.roundId && ` • Round #${bet.roundId}`}
                      </div>
                    </div>
                  </div>
                  
                  <div className="text-right">
                    {bet.status === 'pending' ? (
                      <span className="text-xs text-yellow-400">Pending...</span>
                    ) : pnl ? (
                      <div>
                        <span className={`font-mono text-sm font-medium ${
                          pnl.value >= 0 ? 'text-green-400' : 'text-red-400'
                        }`}>
                          {pnl.display}
                        </span>
                        {bet.status === 'won' && bet.reward && (
                          <p className="text-[10px] text-muted">
                            Won {bet.reward.toFixed(4)} SOL
                          </p>
                        )}
                      </div>
                    ) : null}
                  </div>
                </div>

                {/* Transaction link */}
                <div className="mt-2 pt-2 border-t border-border/50">
                  <a
                    href={`https://solscan.io/tx/${bet.signature}`}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-[10px] font-mono text-muted hover:text-primary transition-colors truncate block"
                  >
                    {bet.signature.slice(0, 20)}...{bet.signature.slice(-8)}
                  </a>
                </div>
              </div>
            );
          })
        )}
      </div>

      {/* Win Rate */}
      {sessionStats.total >= 3 && (
        <div className="mt-4 pt-4 border-t border-border">
          <div className="flex justify-between items-center text-sm">
            <span className="text-muted">Win Rate</span>
            <span className="font-mono">
              {((sessionStats.wins / sessionStats.total) * 100).toFixed(1)}%
            </span>
          </div>
          <div className="w-full bg-surface-light rounded-full h-2 mt-2 overflow-hidden">
            <div
              className="h-full bg-gradient-to-r from-green-500 to-green-400 transition-all duration-500"
              style={{ width: `${(sessionStats.wins / sessionStats.total) * 100}%` }}
            />
          </div>
        </div>
      )}
    </div>
  );
}
