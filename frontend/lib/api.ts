// API client for OreVault backend

const API_URL = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:3001';

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

// Session API
export async function startSession(params: {
  wallet: string;
  strategy: string;
  deploy_amount: number;
  max_tip: number;
  budget: number;
  signature: string;
}) {
  return fetchApi('/api/session/start', {
    method: 'POST',
    body: JSON.stringify(params),
  });
}

export async function stopSession(params: {
  wallet: string;
  signature: string;
}) {
  return fetchApi('/api/session/stop', {
    method: 'POST',
    body: JSON.stringify(params),
  });
}

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

// Balance API
export async function getBalances(wallet: string) {
  return fetchApi(`/api/balances?wallet=${wallet}`);
}

export async function syncBalances(wallet: string) {
  return fetchApi('/api/balances/sync', {
    method: 'POST',
    body: JSON.stringify({ wallet }),
  });
}

// Claims API
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
