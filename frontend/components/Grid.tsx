'use client';

import { useState } from 'react';
import { useOreVaultStore } from '@/lib/store';

/**
 * Grid Component
 * 
 * 5x5 grid visualization showing:
 * - All 25 blocks
 * - SOL deployed per block
 * - EV indicator (green = positive, red = negative)
 * - AI selected block highlighted
 */
export function Grid() {
  const { round, decision, miningWallet, isRunning } = useOreVaultStore();
  const [selectedBlock, setSelectedBlock] = useState<number | null>(null);
  const [showGrid, setShowGrid] = useState(true);

  if (!miningWallet) return null;

  // Generate empty blocks if no data
  const blocks = round.blocks.length > 0 
    ? round.blocks 
    : Array.from({ length: 25 }, (_, i) => ({
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
      {round.roundId && (
        <div className="mb-4">
          <div className="flex justify-between text-sm">
            <span className="text-muted">Round #{round.roundId}</span>
            <span className={`font-mono ${round.timeLeft <= 5 ? 'text-warning' : ''}`}>
              {round.timeLeft.toFixed(1)}s
            </span>
          </div>
          <div className="w-full bg-surface-light rounded-full h-1.5 mt-2">
            <div
              className={`h-1.5 rounded-full transition-all ${
                round.timeLeft <= 5 ? 'bg-warning' : 'bg-primary'
              }`}
              style={{ width: `${(round.timeLeft / 60) * 100}%` }}
            />
          </div>
        </div>
      )}

      {!isRunning && !round.roundId && (
        <p className="text-muted text-sm mb-4">Start mining to see round data</p>
      )}

      {showGrid && (
        <>
          {/* Grid */}
          <div className="grid grid-cols-5 gap-1">
            {blocks.map((block) => {
              const intensity = block.total_deployed / maxDeployed;
              const isPositiveEv = block.ev > 0;
              const isSelected = selectedBlock === block.index;
              const isAiChoice = decision.block === block.index;

              return (
                <button
                  key={block.index}
                  onClick={() => setSelectedBlock(isSelected ? null : block.index)}
                  className={`
                    aspect-square rounded transition-all relative
                    flex flex-col items-center justify-center text-xs
                    ${isSelected ? 'ring-2 ring-white' : ''}
                    ${isAiChoice ? 'ring-2 ring-primary' : ''}
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
                  {isAiChoice && (
                    <div className="absolute -top-1 -right-1 w-2 h-2 bg-primary rounded-full" />
                  )}
                </button>
              );
            })}
          </div>

          {/* Legend */}
          <div className="mt-4 flex items-center gap-4 text-xs text-muted">
            <div className="flex items-center gap-1">
              <div className="w-3 h-3 bg-primary/30 rounded" />
              <span>+EV</span>
            </div>
            <div className="flex items-center gap-1">
              <div className="w-3 h-3 bg-danger/30 rounded" />
              <span>-EV</span>
            </div>
            <div className="flex items-center gap-1">
              <div className="w-2 h-2 bg-primary rounded-full" />
              <span>AI Pick</span>
            </div>
          </div>

          {/* Selected Block Details */}
          {selectedBlock !== null && blocks[selectedBlock] && (
            <div className="mt-4 p-3 bg-surface-light rounded-lg">
              <h4 className="font-medium mb-2">Block {selectedBlock}</h4>
              <div className="grid grid-cols-2 gap-2 text-sm">
                <div>
                  <span className="text-muted">Deployed:</span>
                  <span className="font-mono ml-2">
                    {blocks[selectedBlock].total_deployed.toFixed(4)} SOL
                  </span>
                </div>
                <div>
                  <span className="text-muted">EV:</span>
                  <span className={`font-mono ml-2 ${
                    blocks[selectedBlock].ev > 0 ? 'text-primary' : 'text-danger'
                  }`}>
                    {blocks[selectedBlock].ev > 0 ? '+' : ''}{blocks[selectedBlock].ev.toFixed(4)}
                  </span>
                </div>
              </div>
            </div>
          )}
        </>
      )}
    </div>
  );
}
