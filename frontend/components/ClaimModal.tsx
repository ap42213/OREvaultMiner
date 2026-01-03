'use client';

import { useState } from 'react';
import { claimSol, claimOre } from '@/lib/api';

interface ClaimModalProps {
  isOpen: boolean;
  onClose: () => void;
  claimType: 'sol' | 'ore';
  wallet: string;
  grossAmount: number;
  netAmount: number;
  onSuccess: () => void;
}

/**
 * ClaimModal Component
 * 
 * Confirmation modal for claiming SOL or ORE with fee breakdown.
 * Backend handles signing with the mining wallet.
 */
export function ClaimModal({ 
  isOpen, 
  onClose, 
  claimType, 
  wallet,
  grossAmount, 
  netAmount,
  onSuccess 
}: ClaimModalProps) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!isOpen) return null;

  const feeAmount = grossAmount - netAmount;
  const symbol = claimType.toUpperCase();

  const handleClaim = async () => {
    setLoading(true);
    setError(null);

    try {
      // Backend handles signing with the mining wallet
      if (claimType === 'sol') {
        await claimSol(wallet);
      } else {
        await claimOre(wallet);
      }

      console.log(`Claim ${symbol} submitted`);
      onSuccess();

    } catch (e: any) {
      console.error('Claim failed:', e);
      setError(e.message || 'Transaction failed');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      <div 
        className="absolute inset-0 bg-black/60"
        onClick={onClose}
      />

      {/* Modal */}
      <div className="relative bg-surface border border-border rounded-lg p-6 w-full max-w-sm mx-4">
        <h2 className="text-xl font-semibold mb-6">Claim {symbol}</h2>

        {/* Fee Breakdown */}
        <div className="space-y-3 mb-6">
          <div className="flex justify-between">
            <span className="text-muted">Available</span>
            <span className="font-mono">
              {grossAmount.toFixed(claimType === 'sol' ? 4 : 2)} {symbol}
            </span>
          </div>
          <div className="flex justify-between text-danger">
            <span>Fee (10%)</span>
            <span className="font-mono">
              -{feeAmount.toFixed(claimType === 'sol' ? 4 : 2)} {symbol}
            </span>
          </div>
          <div className="border-t border-border pt-3 flex justify-between">
            <span className="font-medium">You receive</span>
            <span className="font-mono text-primary font-medium">
              {netAmount.toFixed(claimType === 'sol' ? 4 : 2)} {symbol}
            </span>
          </div>
        </div>

        {/* Error */}
        {error && (
          <p className="text-danger text-sm mb-4">{error}</p>
        )}

        {/* Actions */}
        <div className="flex gap-3">
          <button
            onClick={onClose}
            disabled={loading}
            className="flex-1 py-3 px-4 rounded-lg bg-surface-light hover:bg-border text-white font-medium transition-colors disabled:opacity-50"
          >
            Cancel
          </button>
          <button
            onClick={handleClaim}
            disabled={loading || netAmount <= 0}
            className="flex-1 py-3 px-4 rounded-lg bg-primary hover:bg-primary/80 text-black font-medium transition-colors disabled:opacity-50"
          >
            {loading ? 'Claiming...' : 'Confirm'}
          </button>
        </div>

        {/* Note */}
        <p className="text-xs text-muted mt-4 text-center">
          Claim will be signed by the mining wallet
        </p>
      </div>
    </div>
  );
}
