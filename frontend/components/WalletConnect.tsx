'use client';

import { useWallet } from '@solana/wallet-adapter-react';
import { WalletMultiButton } from '@solana/wallet-adapter-react-ui';

/**
 * WalletConnect Component
 * 
 * Multi-wallet connection supporting:
 * - Phantom (Required)
 * - Backpack (Required)
 * - Solflare (Required)
 * - Hush (Required)
 */
export function WalletConnect() {
  const { publicKey, connected, wallet } = useWallet();

  return (
    <div className="flex items-center gap-4">
      {connected && publicKey && (
        <div className="text-sm text-muted hidden sm:block">
          <span className="font-mono">
            {publicKey.toBase58().slice(0, 4)}...{publicKey.toBase58().slice(-4)}
          </span>
          {wallet?.adapter.name && (
            <span className="ml-2 text-xs opacity-60">
              via {wallet.adapter.name}
            </span>
          )}
        </div>
      )}
      <WalletMultiButton 
        style={{
          backgroundColor: '#1f1f1f',
          border: '1px solid #2a2a2a',
          borderRadius: '8px',
          height: '40px',
          fontSize: '14px',
        }}
      />
    </div>
  );
}
