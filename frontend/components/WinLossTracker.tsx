'use client';

import { useOreVaultStore } from '@/lib/store';

/**
 * WinLossTracker Component
 * 
 * Dedicated win/loss tracking display showing:
 * - Total wins and losses
 * - Win rate percentage
 * - Current streak
 * - Session P&L
 */
export function WinLossTracker() {
  const { bets, miningWallet, isRunning } = useOreVaultStore();

  if (!miningWallet) return null;

  // Calculate stats from bets
  const completedBets = bets.filter(b => b.status !== 'pending');
  const wins = bets.filter(b => b.status === 'won').length;
  const losses = bets.filter(b => b.status === 'lost').length;
  const pending = bets.filter(b => b.status === 'pending').length;
  const total = wins + losses;
  const winRate = total > 0 ? (wins / total) * 100 : 0;

  // Calculate P&L
  const totalWon = bets
    .filter(b => b.status === 'won' && b.reward)
    .reduce((acc, b) => acc + (b.reward || 0), 0);
  const totalStaked = completedBets.reduce((acc, b) => acc + b.amount, 0);
  const netPnL = totalWon - totalStaked;

  // Calculate current streak
  const getStreak = () => {
    let streak = 0;
    let streakType: 'win' | 'loss' | null = null;
    
    for (const bet of completedBets) {
      if (streakType === null) {
        streakType = bet.status === 'won' ? 'win' : 'loss';
        streak = 1;
      } else if (
        (streakType === 'win' && bet.status === 'won') ||
        (streakType === 'loss' && bet.status === 'lost')
      ) {
        streak++;
      } else {
        break;
      }
    }
    
    return { streak, type: streakType };
  };

  const currentStreak = getStreak();

  return (
    <div className="bg-surface rounded-lg border border-border p-6">
      <div className="flex justify-between items-center mb-4">
        <h2 className="text-lg font-semibold">Session Tracker</h2>
        {isRunning && (
          <span className="relative flex h-2 w-2">
            <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-green-400 opacity-75"></span>
            <span className="relative inline-flex rounded-full h-2 w-2 bg-green-500"></span>
          </span>
        )}
      </div>

      {/* Main Stats Grid */}
      <div className="grid grid-cols-2 gap-4 mb-4">
        {/* Wins */}
        <div className="bg-green-500/10 border border-green-500/30 rounded-lg p-4 text-center">
          <p className="text-3xl font-mono font-bold text-green-400">{wins}</p>
          <p className="text-xs uppercase tracking-wide text-green-400/70 mt-1">Wins</p>
        </div>
        
        {/* Losses */}
        <div className="bg-red-500/10 border border-red-500/30 rounded-lg p-4 text-center">
          <p className="text-3xl font-mono font-bold text-red-400">{losses}</p>
          <p className="text-xs uppercase tracking-wide text-red-400/70 mt-1">Losses</p>
        </div>
      </div>

      {/* Win Rate Bar */}
      <div className="mb-4">
        <div className="flex justify-between items-center mb-2">
          <span className="text-sm text-muted">Win Rate</span>
          <span className="text-sm font-mono font-medium">
            {winRate.toFixed(1)}%
          </span>
        </div>
        <div className="w-full h-3 bg-surface-light rounded-full overflow-hidden">
          {total > 0 ? (
            <div className="h-full flex">
              <div
                className="bg-gradient-to-r from-green-600 to-green-400 transition-all duration-500"
                style={{ width: `${winRate}%` }}
              />
              <div
                className="bg-gradient-to-r from-red-600 to-red-400 transition-all duration-500"
                style={{ width: `${100 - winRate}%` }}
              />
            </div>
          ) : (
            <div className="h-full bg-surface-light" />
          )}
        </div>
        <div className="flex justify-between text-[10px] text-muted mt-1">
          <span>{wins}W</span>
          <span>{losses}L</span>
        </div>
      </div>

      {/* Streak & Pending */}
      <div className="grid grid-cols-2 gap-4 mb-4">
        {/* Current Streak */}
        <div className="bg-surface-light rounded-lg p-3 text-center">
          {currentStreak.streak > 0 ? (
            <>
              <p className={`text-2xl font-mono font-bold ${
                currentStreak.type === 'win' ? 'text-green-400' : 'text-red-400'
              }`}>
                {currentStreak.streak}
                <span className="text-sm ml-1">
                  {currentStreak.type === 'win' ? '🔥' : '❄️'}
                </span>
              </p>
              <p className="text-[10px] uppercase text-muted mt-1">
                {currentStreak.type === 'win' ? 'Win' : 'Loss'} Streak
              </p>
            </>
          ) : (
            <>
              <p className="text-2xl font-mono text-muted">-</p>
              <p className="text-[10px] uppercase text-muted mt-1">No Streak</p>
            </>
          )}
        </div>

        {/* Pending */}
        <div className="bg-surface-light rounded-lg p-3 text-center">
          <p className="text-2xl font-mono font-bold text-yellow-400">
            {pending}
            {pending > 0 && (
              <span className="inline-block ml-1 w-2 h-2 bg-yellow-400 rounded-full animate-pulse" />
            )}
          </p>
          <p className="text-[10px] uppercase text-muted mt-1">Pending</p>
        </div>
      </div>

      {/* Net P&L */}
      <div className="border-t border-border pt-4">
        <div className="flex justify-between items-center">
          <div>
            <p className="text-sm text-muted">Net P&L</p>
            <p className="text-[10px] text-muted">
              Won {totalWon.toFixed(4)} / Staked {totalStaked.toFixed(4)}
            </p>
          </div>
          <p className={`text-2xl font-mono font-bold ${
            netPnL >= 0 ? 'text-green-400' : 'text-red-400'
          }`}>
            {netPnL >= 0 ? '+' : ''}{netPnL.toFixed(4)}
            <span className="text-sm ml-1">SOL</span>
          </p>
        </div>
      </div>

      {/* No data state */}
      {total === 0 && pending === 0 && (
        <div className="text-center py-4 text-muted">
          <p className="text-sm">No bets yet this session</p>
          <p className="text-xs mt-1">
            {isRunning ? 'Waiting for first round...' : 'Start mining to begin'}
          </p>
        </div>
      )}
    </div>
  );
}
