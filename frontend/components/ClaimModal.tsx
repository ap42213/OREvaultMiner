'use client';

import { useState } from 'react';
import { useWallet, useConnection } from '@solana/wallet-adapter-react';
import { Transaction } from '@solana/web3.js';

interface ClaimModalProps {
  isOpen: boolean;
  onClose: () => void;
  claimType: 'sol' | 'ore';
  grossAmount: number;
  netAmount: number;
  onSuccess: () => void;
}

/**
 * ClaimModal Component
 * 
 * Confirmation modal for claiming SOL or ORE with fee breakdown:
 * 
 * CLAIM SOL
 * Available:    0.850 SOL
 * Fee (10%):   -0.085 SOL
 * You receive:  0.765 SOL
 * [Cancel] [Confirm]
 */
export function ClaimModal({ 
  isOpen, 
  onClose, 
  claimType, 
  grossAmount, 
  netAmount,
  onSuccess 
}: ClaimModalProps) {
  const { publicKey, signTransaction } = useWallet();
  const { connection } = useConnection();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!isOpen) return null;

  const feeAmount = grossAmount - netAmount;
  const symbol = claimType.toUpperCase();

  const handleClaim = async () => {
    if (!publicKey || !signTransaction) {
      setError('Wallet not connected');
      return;
    }

    setLoading(true);
    setError(null);

    try {
      // 1. Get claim transaction from backend
      const response = await fetch(`${process.env.NEXT_PUBLIC_API_URL}/api/claim/${claimType}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ 
          wallet: publicKey.toBase58(),
          amount: null, // Claim all
        }),
      });

      const data = await response.json();
      if (!data.success) {
        throw new Error(data.error || 'Failed to build transaction');
      }

      // 2. Deserialize transaction
      const txBuffer = Buffer.from(data.transaction, 'base64');
      const transaction = Transaction.from(txBuffer);

      // 3. Get latest blockhash
      const { blockhash, lastValidBlockHeight } = await connection.getLatestBlockhash();
      transaction.recentBlockhash = blockhash;
      transaction.feePayer = publicKey;

      // 4. Sign with wallet
      const signedTx = await signTransaction(transaction);

      // 5. Send transaction
      const signature = await connection.sendRawTransaction(signedTx.serialize());

      // 6. Confirm transaction
      await connection.confirmTransaction({
        signature,
        blockhash,
        lastValidBlockHeight,
      });

      console.log(`Claim ${symbol} confirmed:`, signature);
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
          <div className="border-t border-border pt-3 flex justify-between text-lg font-semibold">
            <span>You receive</span>
            <span className="font-mono text-primary">
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
            className="flex-1 py-2 px-4 bg-surface-light hover:bg-border rounded-lg transition-colors disabled:opacity-50"
          >
            Cancel
          </button>
          <button
            onClick={handleClaim}
            disabled={loading}
            className="flex-1 py-2 px-4 bg-primary hover:bg-primary/80 text-black font-medium rounded-lg transition-colors disabled:opacity-50"
          >
            {loading ? 'Claiming...' : 'Confirm'}
          </button>
        </div>
      </div>
    </div>
  );
}
