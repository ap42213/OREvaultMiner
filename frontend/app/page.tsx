'use client';

import { WalletConnect } from '@/components/WalletConnect';
import { BalanceDisplay } from '@/components/BalanceDisplay';
import { ClaimPanel } from '@/components/ClaimPanel';
import { BetConfig } from '@/components/BetConfig';
import { Stats } from '@/components/Stats';
import { Grid } from '@/components/Grid';
import { useWallet } from '@solana/wallet-adapter-react';

export default function Home() {
  const { connected } = useWallet();

  return (
    <main className="min-h-screen p-8">
      {/* Header */}
      <header className="flex justify-between items-center mb-8">
        <div>
          <h1 className="text-2xl font-bold">OreVault</h1>
          <p className="text-muted text-sm">Automated ORE v3 Mining</p>
        </div>
        <WalletConnect />
      </header>

      {!connected ? (
        <div className="flex flex-col items-center justify-center min-h-[60vh]">
          <div className="text-center">
            <h2 className="text-xl mb-4">Connect Your Wallet</h2>
            <p className="text-muted mb-6">
              Connect with Phantom, Backpack, Solflare, or Hush to start mining
            </p>
            <WalletConnect />
          </div>
        </div>
      ) : (
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
          {/* Left Column - Balances & Claims */}
          <div className="space-y-6">
            <BalanceDisplay />
            <ClaimPanel />
          </div>

          {/* Center Column - Config & Grid */}
          <div className="space-y-6">
            <BetConfig />
            <Grid />
          </div>

          {/* Right Column - Stats */}
          <div>
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
