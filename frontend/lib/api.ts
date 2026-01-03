// API client for OreVault backend

// Use relative URLs so API calls go to the same host the page was loaded from
const API_URL = process.env.NEXT_PUBLIC_API_URL || '';

interface ApiResponse<T> {
  success: boolean;
  error?: string;
  data?: T;
}

async function fetchApi<T>(
  endpoint: string,
  options: RequestInit = {}
): Promise<T> {
  const response = await fetch(`${API_URL}${endpoint}`, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      ...options.headers,
    },
  });

  const data = await response.json();
  
  if (!data.success && data.error) {
    throw new Error(data.error);
  }
  
  return data;
}

// =========================================================================
// Wallet Management API
// =========================================================================

export interface WalletInfo {
  wallet_address: string;
  name: string | null;
  created_at: string;
  last_used_at: string | null;
}

export async function getMiningWallets(): Promise<{ wallets: WalletInfo[] }> {
  return fetchApi('/api/wallet/list');
}

export async function generateWallet(): Promise<{ wallet_address: string }> {
  return fetchApi('/api/wallet/generate', { method: 'POST' });
}

export async function importWallet(privateKey: string): Promise<{ wallet_address: string }> {
  return fetchApi('/api/wallet/import', {
    method: 'POST',
    body: JSON.stringify({ private_key: privateKey }),
  });
}

export async function exportWallet(walletAddress: string): Promise<{ private_key: string }> {
  return fetchApi('/api/wallet/export', {
    method: 'POST',
    body: JSON.stringify({ wallet_address: walletAddress }),
  });
}

// =========================================================================
// Session API (no wallet signing needed)
// =========================================================================

export async function startSession(params: {
  wallet: string;
  strategy: string;
  deploy_amount: number;
  max_tip: number;
  budget: number;
}) {
  return fetchApi('/api/session/start', {
    method: 'POST',
    body: JSON.stringify(params),
  });
}

export async function stopSession(params: { wallet: string }) {
  return fetchApi('/api/session/stop', {
    method: 'POST',
    body: JSON.stringify(params),
  });
}

export async function getSessionStatus(wallet: string) {
  return fetchApi(`/api/session/status?wallet=${wallet}`);
}

// =========================================================================
// Stats API
// =========================================================================

export async function getStats(wallet: string) {
  return fetchApi(`/api/stats?wallet=${wallet}`);
}

export async function getTransactions(
  wallet: string,
  limit = 50,
  offset = 0
) {
  return fetchApi(
    `/api/transactions?wallet=${wallet}&limit=${limit}&offset=${offset}`
  );
}

// =========================================================================
// Balance API
// =========================================================================

export async function getBalances(wallet: string) {
  return fetchApi(`/api/balances?wallet=${wallet}`);
}

export async function syncBalances(wallet: string) {
  return fetchApi('/api/balances/sync', {
    method: 'POST',
    body: JSON.stringify({ wallet }),
  });
}

// =========================================================================
// Claims API
// =========================================================================

export async function claimSol(wallet: string, amount?: number) {
  return fetchApi('/api/claim/sol', {
    method: 'POST',
    body: JSON.stringify({ wallet, amount }),
  });
}

export async function claimOre(wallet: string, amount?: number) {
  return fetchApi('/api/claim/ore', {
    method: 'POST',
    body: JSON.stringify({ wallet, amount }),
  });
}

export async function getClaimsHistory(
  wallet: string,
  limit = 50,
  offset = 0
) {
  return fetchApi(
    `/api/claims/history?wallet=${wallet}&limit=${limit}&offset=${offset}`
  );
}
