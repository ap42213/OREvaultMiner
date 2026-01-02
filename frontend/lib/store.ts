import { create } from 'zustand';

interface WsMessage {
  type: string;
  payload: any;
}

interface RoundState {
  roundId: number | null;
  timeLeft: number;
  blocks: Array<{
    index: number;
    total_deployed: number;
    ev: number;
  }>;
}

interface DecisionState {
  action: string | null;
  block: number | null;
  ev: number;
  reason: string | null;
}

interface OreVaultState {
  // Connection state
  connected: boolean;
  wallet: string | null;
  
  // Round state
  round: RoundState;
  
  // Decision state
  decision: DecisionState;
  
  // Balances
  unclaimedSol: number;
  unclaimedOre: number;
  refinedOre: number;
  
  // Actions
  setConnected: (connected: boolean) => void;
  setWallet: (wallet: string | null) => void;
  updateRound: (round: RoundState) => void;
  updateDecision: (decision: DecisionState) => void;
  updateBalances: (sol: number, ore: number, refined: number) => void;
  handleWsMessage: (message: WsMessage) => void;
}

export const useOreVaultStore = create<OreVaultState>((set) => ({
  // Initial state
  connected: false,
  wallet: null,
  
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
  
  unclaimedSol: 0,
  unclaimedOre: 0,
  refinedOre: 0,
  
  // Actions
  setConnected: (connected) => set({ connected }),
  setWallet: (wallet) => set({ wallet }),
  
  updateRound: (round) => set({ round }),
  
  updateDecision: (decision) => set({ decision }),
  
  updateBalances: (sol, ore, refined) => set({
    unclaimedSol: sol,
    unclaimedOre: ore,
    refinedOre: refined,
  }),
  
  handleWsMessage: (message) => {
    switch (message.type) {
      case 'round:update':
        set({
          round: {
            roundId: message.payload.round_id,
            timeLeft: message.payload.time_left,
            blocks: message.payload.blocks,
          },
        });
        break;
        
      case 'decision:made':
        set({
          decision: {
            action: message.payload.action,
            block: message.payload.block,
            ev: message.payload.ev,
            reason: message.payload.reason,
          },
        });
        break;
        
      case 'balance:update':
        set({
          unclaimedSol: message.payload.unclaimed_sol,
          unclaimedOre: message.payload.unclaimed_ore,
          refinedOre: message.payload.refined_ore,
        });
        break;
        
      default:
        console.log('Unknown WS message:', message);
    }
  },
}));
