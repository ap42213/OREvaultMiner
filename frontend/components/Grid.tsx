'use client';

import { useState, useEffect } from 'react';
import { useWallet } from '@solana/wallet-adapter-react';

interface BlockData {
  index: number;
  total_deployed: number;
  ev: number;
}

interface RoundData {
  round_id: number;
  time_left: number;
  blocks: BlockData[];
}

/**
 * Grid Component
 * 
 * Optional 5x5 grid visualization showing:
 * - All 25 blocks
 * - SOL deployed per block
 * - EV indicator (green = positive, red = negative)
 */
export function Grid() {
  const { publicKey, connected } = useWallet();
  const [roundData, setRoundData] = useState<RoundData | null>(null);
  const [selectedBlock, setSelectedBlock] = useState<number | null>(null);
  const [showGrid, setShowGrid] = useState(false);

  // WebSocket connection for real-time updates
  useEffect(() => {
    if (!connected || !publicKey) return;

    const wsUrl = `${process.env.NEXT_PUBLIC_WS_URL}?wallet=${publicKey.toBase58()}`;
    const ws = new WebSocket(wsUrl);

    ws.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data);
        if (data.type === 'round:update') {
          setRoundData(data.payload);
        }
      } catch (e) {
        console.error('WS parse error:', e);
      }
    };

    ws.onerror = (error) => {
      console.error('WebSocket error:', error);
    };

    return () => {
      ws.close();
    };
  }, [connected, publicKey]);

  if (!connected) return null;

  // Generate empty blocks if no data
  const blocks: BlockData[] = roundData?.blocks || Array.from({ length: 25 }, (_, i) => ({
    index: i,
    total_deployed: 0,
    ev: 0,
  }));

  // Find max for scaling
  const maxDeployed = Math.max(...blocks.map(b => b.total_deployed), 0.001);

  return (
    <div className="bg-surface rounded-lg border border-border p-6">
      <div className="flex justify-between items-center mb-4">
        <h2 className="text-lg font-semibold">5x5 Grid</h2>
        <button
          onClick={() => setShowGrid(!showGrid)}
          className="text-sm text-muted hover:text-white transition-colors"
        >
          {showGrid ? 'Hide' : 'Show'}
        </button>
      </div>

      {/* Round Timer */}
      {roundData && (
        <div className="mb-4">
          <div className="flex justify-between text-sm">
            <span className="text-muted">Round #{roundData.round_id}</span>
            <span className={`font-mono ${roundData.time_left <= 5 ? 'text-warning' : ''}`}>
              {roundData.time_left.toFixed(1)}s
            </span>
          </div>
          <div className="w-full bg-surface-light rounded-full h-1.5 mt-2">
            <div
              className={`h-1.5 rounded-full transition-all ${
                roundData.time_left <= 5 ? 'bg-warning' : 'bg-primary'
              }`}
              style={{ width: `${(roundData.time_left / 60) * 100}%` }}
            />
          </div>
        </div>
      )}

      {showGrid && (
        <>
          {/* Grid */}
          <div className="grid grid-cols-5 gap-1">
            {blocks.map((block) => {
              const intensity = block.total_deployed / maxDeployed;
              const isPositiveEv = block.ev > 0;
              const isSelected = selectedBlock === block.index;

              return (
                <button
                  key={block.index}
                  onClick={() => setSelectedBlock(isSelected ? null : block.index)}
                  className={`
                    aspect-square rounded transition-all
                    flex flex-col items-center justify-center text-xs
                    ${isSelected ? 'ring-2 ring-white' : ''}
                    ${isPositiveEv ? 'bg-primary/20 hover:bg-primary/30' : 'bg-danger/20 hover:bg-danger/30'}
                  `}
                  style={{
                    opacity: 0.3 + intensity * 0.7,
                  }}
                >
                  <span className="font-mono text-[10px]">{block.index}</span>
                  {block.total_deployed > 0 && (
                    <span className="font-mono text-[8px] text-muted">
                      {block.total_deployed.toFixed(2)}
                    </span>
                  )}
                </button>
              );
            })}
          </div>

          {/* Selected Block Details */}
          {selectedBlock !== null && (
            <div className="mt-4 p-3 bg-surface-light rounded-lg">
              <h4 className="font-medium mb-2">Block {selectedBlock}</h4>
              <div className="grid grid-cols-2 gap-2 text-sm">
                <div>
                  <span className="text-muted">Deployed:</span>
                  <span className="font-mono ml-2">
                    {blocks[selectedBlock]?.total_deployed.toFixed(4)} SOL
                  </span>
                </div>
                <div>
                  <span className="text-muted">EV:</span>
                  <span className={`font-mono ml-2 ${
                    blocks[selectedBlock]?.ev >= 0 ? 'text-primary' : 'text-danger'
                  }`}>
                    {blocks[selectedBlock]?.ev >= 0 ? '+' : ''}
                    {blocks[selectedBlock]?.ev.toFixed(4)} SOL
                  </span>
                </div>
              </div>
            </div>
          )}

          {/* Legend */}
          <div className="mt-4 flex justify-center gap-6 text-xs text-muted">
            <div className="flex items-center gap-2">
              <div className="w-3 h-3 bg-primary/50 rounded" />
              <span>Positive EV</span>
            </div>
            <div className="flex items-center gap-2">
              <div className="w-3 h-3 bg-danger/50 rounded" />
              <span>Negative EV</span>
            </div>
          </div>
        </>
      )}

      {!showGrid && (
        <p className="text-sm text-muted">
          Click Show to view real-time block data
        </p>
      )}
    </div>
  );
}
