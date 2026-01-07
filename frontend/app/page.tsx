'use client';

import { MiningWallet } from '@/components/MiningWallet';
import { BalanceDisplay } from '@/components/BalanceDisplay';
import { ClaimPanel } from '@/components/ClaimPanel';
import { BetConfig } from '@/components/BetConfig';
import { AiDecisions } from '@/components/AiDecisions';
import { Stats } from '@/components/Stats';
import { Grid } from '@/components/Grid';
import { LiveBets } from '@/components/LiveBets';
import { WinLossTracker } from '@/components/WinLossTracker';
import { useOreVaultStore } from '@/lib/store';

export default function Home() {
  const { miningWalletLoading } = useOreVaultStore();

  return (
    <main className="min-h-screen p-8">
      {/* Header */}
      <header className="flex justify-between items-center mb-8">
        <div>
          <h1 className="text-2xl font-bold">OreVault</h1>
          <p className="text-muted text-sm">Automated ORE v3 Mining</p>
        </div>
        <MiningWallet />
      </header>

      {miningWalletLoading ? (
        <div className="flex flex-col items-center justify-center min-h-[60vh]">
          <div className="text-center">
            <div className="w-8 h-8 border-2 border-primary border-t-transparent rounded-full animate-spin mx-auto mb-4" />
            <p className="text-muted">Loading mining wallet...</p>
          </div>
        </div>
      ) : (
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
          {/* Left Column - Balances & Claims */}
          <div className="space-y-6">
            <BalanceDisplay />
            <ClaimPanel />
          </div>

          {/* Center Column - Config, Tracker & Grid */}
          <div className="space-y-6">
            <BetConfig />
            <WinLossTracker />
            <Grid />
          </div>

          {/* Right Column - Live Bets, AI & Stats */}
          <div className="space-y-6">
            <LiveBets />
            <AiDecisions />
            <Stats />
          </div>
        </div>
      )}

      {/* Footer */}
      <footer className="mt-12 pt-6 border-t border-border text-center text-muted text-sm">
        <p>Mainnet - Real SOL Transactions</p>
        <p className="mt-1">Program: oreV3EG1i9BEgiAJ8b177Z2S2rMarzak4NMv1kULvWv</p>
      </footer>
    </main>
  );
}
