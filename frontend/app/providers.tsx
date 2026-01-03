'use client';

import { useEffect, useRef } from 'react';
import { useOreVaultStore } from '@/lib/store';
import { getMiningWallets, generateWallet } from '@/lib/api';

/**
 * App Providers
 * 
 * - Loads mining wallet from backend on mount
 * - Establishes WebSocket connection for real-time updates
 * - No wallet-adapter needed (backend manages signing)
 */
export function Providers({ children }: { children: React.ReactNode }) {
  const wsRef = useRef<WebSocket | null>(null);
  const { 
    miningWallet,
    setMiningWallet, 
    setMiningWalletLoading,
    setWsConnected,
    handleWsMessage 
  } = useOreVaultStore();

  // Load mining wallet on mount
  useEffect(() => {
    async function loadWallet() {
      try {
        const { wallets } = await getMiningWallets();
        
        if (wallets && wallets.length > 0) {
          // Use the first active wallet
          setMiningWallet(wallets[0].wallet_address);
        } else {
          // No wallet exists, generate one
          console.log('No mining wallet found, generating...');
          const { wallet_address } = await generateWallet();
          setMiningWallet(wallet_address);
        }
      } catch (error) {
        console.error('Failed to load mining wallet:', error);
      } finally {
        setMiningWalletLoading(false);
      }
    }

    loadWallet();
  }, [setMiningWallet, setMiningWalletLoading]);

  // WebSocket connection when wallet is loaded
  useEffect(() => {
    if (!miningWallet) return;

    // Build WebSocket URL from current page location
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = process.env.NEXT_PUBLIC_WS_URL || `${protocol}//${window.location.host}/ws`;
    const ws = new WebSocket(`${wsUrl}?wallet=${miningWallet}`);
    wsRef.current = ws;

    ws.onopen = () => {
      console.log('WebSocket connected');
      setWsConnected(true);
    };

    ws.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data);
        handleWsMessage(data);
      } catch (e) {
        console.error('WS parse error:', e);
      }
    };

    ws.onclose = () => {
      console.log('WebSocket disconnected');
      setWsConnected(false);
    };

    ws.onerror = (error) => {
      console.error('WebSocket error:', error);
    };

    return () => {
      ws.close();
    };
  }, [miningWallet, setWsConnected, handleWsMessage]);

  return <>{children}</>;
}
