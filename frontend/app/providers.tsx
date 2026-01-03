'use client';

import { useEffect, useRef } from 'react';
import { useOreVaultStore } from '@/lib/store';
import { getMiningWallets, generateWallet, getSessionStatus } from '@/lib/api';

/**
 * App Providers
 * 
 * - Loads mining wallet from backend on mount
 * - Checks for active mining session
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
    setIsRunning,
    handleWsMessage 
  } = useOreVaultStore();

  // Load mining wallet on mount
  useEffect(() => {
    async function loadWallet() {
      try {
        const { wallets } = await getMiningWallets();
        
        if (wallets && wallets.length > 0) {
          // Use the first active wallet
          const walletAddress = wallets[0].wallet_address;
          setMiningWallet(walletAddress);
          
          // Check if there's an active session
          try {
            const status = await getSessionStatus(walletAddress) as { active: boolean };
            if (status.active) {
              setIsRunning(true);
            }
          } catch (e) {
            // Session status check failed, assume not running
            console.log('Session status check failed:', e);
          }
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
  }, [setMiningWallet, setMiningWalletLoading, setIsRunning]);

  // WebSocket connection when wallet is loaded
  useEffect(() => {
    if (!miningWallet) return;

    let ws: WebSocket | null = null;
    let reconnectTimeout: NodeJS.Timeout | null = null;
    let shouldReconnect = true;

    const connect = () => {
      // Build WebSocket URL from current page location
      const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
      const wsUrl = process.env.NEXT_PUBLIC_WS_URL || `${protocol}//${window.location.host}/ws`;
      ws = new WebSocket(`${wsUrl}?wallet=${miningWallet}`);
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
        // Auto-reconnect after 2 seconds
        if (shouldReconnect) {
          reconnectTimeout = setTimeout(connect, 2000);
        }
      };

      ws.onerror = (error) => {
        console.error('WebSocket error:', error);
      };
    };

    connect();

    return () => {
      shouldReconnect = false;
      if (reconnectTimeout) clearTimeout(reconnectTimeout);
      if (ws) ws.close();
    };
  }, [miningWallet, setWsConnected, handleWsMessage]);

  return <>{children}</>;
}
