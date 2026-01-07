import { create } from 'zustand';

interface WsMessage {
  type: string;
  payload: any;
}

interface BlockData {
  index: number;
  total_deployed: number;
  ev: number;
}

interface RoundState {
  roundId: number | null;
  timeLeft: number;
  blocks: BlockData[];
}

interface DecisionState {
  action: string | null;
  block: number | null;
  ev: number;
  reason: string | null;
}

interface AiAnalysis {
  model: string;
  latencyMs: number;
  recommendation: string;
  confidence: number;
  reasoning: string;
  timestamp: number;
}

interface Transaction {
  signature: string;
  type: string;
  block: number;
  amount: number;
  status: 'pending' | 'confirmed' | 'failed';
  timestamp: number;
}

interface Bet {
  signature: string;
  block: number;
  amount: number;
  status: 'pending' | 'won' | 'lost';
  reward?: number;
  roundId?: number;
  timestamp: number;
}

interface OreVaultState {
  // Mining wallet (from backend)
  miningWallet: string | null;
  miningWalletLoading: boolean;
  
  // Connection state
  wsConnected: boolean;
  
  // Round state
  round: RoundState;
  
  // Decision state
  decision: DecisionState;
  
  // AI Analysis
  aiAnalysis: AiAnalysis | null;
  
  // Recent transactions
  transactions: Transaction[];
  
  // Live bets tracking
  bets: Bet[];
  
  // Balances
  unclaimedSol: number;
  unclaimedOre: number;
  refinedOre: number;
  
  // Session state
  isRunning: boolean;
  
  // Actions
  setMiningWallet: (wallet: string | null) => void;
  setMiningWalletLoading: (loading: boolean) => void;
  setWsConnected: (connected: boolean) => void;
  setIsRunning: (running: boolean) => void;
  updateRound: (round: RoundState) => void;
  updateDecision: (decision: DecisionState) => void;
  updateAiAnalysis: (analysis: AiAnalysis) => void;
  addTransaction: (tx: Transaction) => void;
  updateTransactionStatus: (sig: string, status: 'confirmed' | 'failed') => void;
  updateBalances: (sol: number, ore: number, refined: number) => void;
  addBet: (bet: Bet) => void;
  updateBetResult: (sig: string, status: 'won' | 'lost', reward?: number) => void;
  clearBets: () => void;
  handleWsMessage: (message: WsMessage) => void;
}

export const useOreVaultStore = create<OreVaultState>((set, get) => ({
  // Initial state
  miningWallet: null,
  miningWalletLoading: true,
  wsConnected: false,
  isRunning: false,
  
  round: {
    roundId: null,
    timeLeft: 0,
    blocks: [],
  },
  
  decision: {
    action: null,
    block: null,
    ev: 0,
    reason: null,
  },
  
  aiAnalysis: null,
  transactions: [],
  bets: [],
  
  unclaimedSol: 0,
  unclaimedOre: 0,
  refinedOre: 0,
  
  // Actions
  setMiningWallet: (wallet) => set({ miningWallet: wallet }),
  setMiningWalletLoading: (loading) => set({ miningWalletLoading: loading }),
  setWsConnected: (connected) => set({ wsConnected: connected }),
  setIsRunning: (running) => set({ isRunning: running }),
  
  updateRound: (round) => set({ round }),
  updateDecision: (decision) => set({ decision }),
  updateAiAnalysis: (analysis) => set({ aiAnalysis: analysis }),
  
  addTransaction: (tx) => set((state) => ({
    transactions: [tx, ...state.transactions].slice(0, 50), // Keep last 50
  })),
  
  updateTransactionStatus: (sig, status) => set((state) => ({
    transactions: state.transactions.map(tx =>
      tx.signature === sig ? { ...tx, status } : tx
    ),
  })),
  
  updateBalances: (sol, ore, refined) => set({
    unclaimedSol: sol,
    unclaimedOre: ore,
    refinedOre: refined,
  }),
  
  addBet: (bet) => set((state) => ({
    bets: [bet, ...state.bets].slice(0, 100), // Keep last 100 bets
  })),
  
  updateBetResult: (sig, status, reward) => set((state) => ({
    bets: state.bets.map(bet =>
      bet.signature === sig ? { ...bet, status, reward } : bet
    ),
  })),
  
  clearBets: () => set({ bets: [] }),
  
  handleWsMessage: (message) => {
    const { type, payload } = message;
    
    switch (type) {
      case 'round:update':
        set({
          round: {
            roundId: payload.round_id,
            timeLeft: payload.time_left,
            blocks: payload.blocks,
          },
        });
        break;
        
      case 'decision:made':
        set({
          decision: {
            action: payload.action,
            block: payload.block,
            ev: payload.ev,
            reason: payload.reason,
          },
        });
        break;
        
      case 'ai:analysis':
        set({
          aiAnalysis: {
            model: payload.model,
            latencyMs: payload.latency_ms,
            recommendation: payload.recommendation,
            confidence: payload.confidence,
            reasoning: payload.reasoning,
            timestamp: Date.now(),
          },
        });
        break;
        
      case 'tx:submitted':
        get().addTransaction({
          signature: payload.signature,
          type: payload.tx_type || 'deploy',
          block: payload.block,
          amount: payload.amount,
          status: 'pending',
          timestamp: Date.now(),
        });
        // Also track as a bet for LiveBets component
        get().addBet({
          signature: payload.signature,
          block: payload.block,
          amount: payload.amount,
          status: 'pending',
          roundId: get().round.roundId ?? undefined,
          timestamp: Date.now(),
        });
        break;
        
      case 'tx:confirmed':
        get().updateTransactionStatus(payload.signature, 'confirmed');
        // Update bet result - reward > 0 means we won
        const hasReward = payload.reward && payload.reward > 0;
        get().updateBetResult(
          payload.signature,
          hasReward ? 'won' : 'lost',
          payload.reward
        );
        break;
        
      case 'tx:failed':
        get().updateTransactionStatus(payload.signature, 'failed');
        get().updateBetResult(payload.signature, 'lost');
        break;
        
      case 'balance:update':
        set({
          unclaimedSol: payload.unclaimed_sol,
          unclaimedOre: payload.unclaimed_ore,
          refinedOre: payload.refined_ore,
        });
        break;
        
      case 'session:started':
        set({ isRunning: true });
        break;
        
      case 'session:stopped':
        set({ isRunning: false });
        break;
        
      default:
        console.log('Unknown WS message:', message);
    }
  },
}));
