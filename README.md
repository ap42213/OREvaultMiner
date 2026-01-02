# OreVault

Automated mining system for ORE v3 on Solana mainnet with Jito bundle submission and EV-based strategy.

## ⚠️ WARNING: PRODUCTION SYSTEM

This system handles **REAL SOL** on Solana mainnet. All transactions are real cryptocurrency transactions with real financial implications.

**Start with small amounts for testing.**

## Architecture

```
BROWSER (Phantom/Backpack/Solflare/Hush)
  Signs mainnet transactions | Balance display | Claim UI
       |
       | WebSocket
       v
VPS (NYC) - Strategy engine | Jito submission | Claims processor
       |
       | RPC
       v
SOLANA MAINNET + JITO (NY)
```

## Features

- **Automated Mining**: EV-based strategy with last-second Jito bundle submission
- **Multi-Wallet Support**: Phantom, Backpack, Solflare, Hush
- **Balance Tracking**: Wallet + unclaimed ORE account balances
- **Claims System**: Claim SOL/ORE with 10% fee preview
- **Real-time Updates**: WebSocket for live round and decision updates
- **Strategy Options**: Best EV, Conservative, Aggressive

## ORE v3 Mechanics

- 60 second rounds
- Deploy SOL on 5x5 grid (25 blocks)
- RNG selects winning block
- Losing SOL redistributes to winners
- Winnings go to ORE account (not wallet directly)
- 10% fee on all claims

## Quick Start

### Prerequisites

- Rust 1.75+
- Node.js 20+
- PostgreSQL 15+
- Helius RPC API key (or other mainnet RPC)

### Backend Setup

```bash
cd backend

# Copy environment file
cp .env.example .env

# Edit .env with your configuration
# - Set RPC_URL to your Helius mainnet endpoint
# - Set DATABASE_URL to your PostgreSQL connection

# Build
cargo build --release

# Run migrations
sqlx migrate run

# Start server
cargo run --release
```

### Frontend Setup

```bash
cd frontend

# Install dependencies
npm install

# Copy environment file
cp .env.example .env.local

# Edit .env.local with your configuration
# - Set NEXT_PUBLIC_API_URL to your backend URL

# Development
npm run dev

# Production build
npm run build
npm start
```

## Deployment

### Backend (Vultr NYC VPS)

```bash
# Run deployment script on VPS
sudo ./deploy/deploy.sh
```

### Frontend (Vercel)

1. Connect GitHub repository to Vercel
2. Set environment variables
3. Deploy

## API Reference

### REST Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/session/start` | Start autominer |
| POST | `/api/session/stop` | Stop autominer |
| GET | `/api/stats` | Session stats |
| GET | `/api/transactions` | Bet history |
| GET | `/api/balances` | All balances (wallet + unclaimed) |
| POST | `/api/balances/sync` | Sync from on-chain ORE account |
| POST | `/api/claim/sol` | Claim SOL (returns tx to sign) |
| POST | `/api/claim/ore` | Claim ORE (returns tx to sign) |
| GET | `/api/claims/history` | Past claim transactions |

### WebSocket Events

| Event | Description |
|-------|-------------|
| `round:update` | Round info with block data |
| `decision:made` | Deploy or skip decision |
| `tx:confirmed` | Transaction confirmation |
| `balance:update` | Balance changes |
| `claim:confirmed` | Claim completion |

## Configuration

### Environment Variables

| Variable | Description |
|----------|-------------|
| `RPC_URL` | Helius mainnet RPC URL |
| `JITO_BLOCK_ENGINE` | Jito block engine (ny.mainnet.block-engine.jito.wtf) |
| `DATABASE_URL` | PostgreSQL connection string |
| `ORE_PROGRAM_ID` | oreV3EG1i9BEgiAJ8b177Z2S2rMarzak4NMv1kULvWv |

## Timing Strategy

```
Round: 60s
Other bots: T=55-58s
OreVault: T=58-60s

T-2.0s: Snapshot 25 blocks
T-1.8s: Calculate EV per block
T-1.6s: GO/NO-GO decision
T-1.0s: Submit Jito bundle
T-0.0s: Round closes
```

## EV Calculation

```
EV = (potential_reward * 1/25) - tip_cost

potential_reward = total_pot * (your_deploy / block_total)
```

Skip round if best EV < 0.

## Cost Estimates

| Component | Monthly Cost |
|-----------|-------------|
| Frontend (Vercel) | $0 |
| Database (Supabase) | $0 |
| VPS (Vultr NYC) | $12-24 |
| RPC (Helius) | $50+ |

## License

MIT

## Disclaimer

This software is provided as-is. Use at your own risk. The authors are not responsible for any financial losses incurred through the use of this software.
