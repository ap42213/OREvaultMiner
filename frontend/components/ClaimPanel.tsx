'use client';

import { useState } from 'react';
import { useOreVaultStore } from '@/lib/store';
import { ClaimModal } from './ClaimModal';
import useSWR from 'swr';

interface ClaimableBalances {
  unclaimed: {
    sol: number;
    ore: number;
    refined_ore: number;
  };
  claimable: {
    sol: number;
    ore: number;
  };
}

const fetcher = async (url: string) => {
  const res = await fetch(url);
  if (!res.ok) throw new Error('Failed to fetch');
  const data = await res.json();
  if (!data.success) throw new Error(data.error);
  return data;
};

/**
 * ClaimPanel Component
 * 
 * Allows claiming SOL or ORE from the mining account.
 * Shows 10% fee preview before claiming.
 */
export function ClaimPanel() {
  const { miningWallet, miningWalletLoading } = useOreVaultStore();
  const [claimType, setClaimType] = useState<'sol' | 'ore' | null>(null);
  const [isModalOpen, setIsModalOpen] = useState(false);

  const { data, mutate } = useSWR<ClaimableBalances>(
    miningWallet
      ? `${process.env.NEXT_PUBLIC_API_URL}/api/balances?wallet=${miningWallet}`
      : null,
    fetcher,
    { refreshInterval: 10000 }
  );

  if (miningWalletLoading || !miningWallet || !data) return null;

  const balances = data as ClaimableBalances;
  const hasClaimableSol = balances.unclaimed.sol > 0;
  const hasClaimableOre = balances.unclaimed.ore > 0;

  const openClaimModal = (type: 'sol' | 'ore') => {
    setClaimType(type);
    setIsModalOpen(true);
  };

  return (
    <>
      <div className="bg-surface rounded-lg border border-border p-6">
        <h2 className="text-lg font-semibold mb-4">Claim Rewards</h2>
        
        <p className="text-sm text-muted mb-4">
          Claim your mining rewards from the ORE account.
        </p>

        <div className="space-y-3">
          {/* Claim SOL Button */}
          <button
            onClick={() => openClaimModal('sol')}
            disabled={!hasClaimableSol}
            className={`w-full py-3 px-4 rounded-lg font-medium transition-colors flex justify-between items-center
              ${hasClaimableSol 
                ? 'bg-surface-light hover:bg-border text-white' 
                : 'bg-surface-light text-muted cursor-not-allowed'}`}
          >
            <span>Claim SOL</span>
            <span className="font-mono">
              {balances.unclaimed.sol.toFixed(4)} SOL
            </span>
          </button>

          {/* Claim ORE Button */}
          <button
            onClick={() => openClaimModal('ore')}
            disabled={!hasClaimableOre}
            className={`w-full py-3 px-4 rounded-lg font-medium transition-colors flex justify-between items-center
              ${hasClaimableOre 
                ? 'bg-surface-light hover:bg-border text-white' 
                : 'bg-surface-light text-muted cursor-not-allowed'}`}
          >
            <span>Claim ORE</span>
            <span className="font-mono">
              {balances.unclaimed.ore.toFixed(2)} ORE
            </span>
          </button>
        </div>

        {/* Fee Notice */}
        <p className="text-xs text-muted mt-4">
          10% claim fee applied by ORE protocol
        </p>
      </div>

      {/* Claim Modal */}
      {claimType && miningWallet && (
        <ClaimModal
          isOpen={isModalOpen}
          onClose={() => {
            setIsModalOpen(false);
            setClaimType(null);
          }}
          claimType={claimType}
          wallet={miningWallet}
          grossAmount={claimType === 'sol' ? balances.unclaimed.sol : balances.unclaimed.ore}
          netAmount={claimType === 'sol' ? balances.claimable.sol : balances.claimable.ore}
          onSuccess={() => {
            mutate();
            setIsModalOpen(false);
            setClaimType(null);
          }}
        />
      )}
    </>
  );
}
