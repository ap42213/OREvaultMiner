'use client';

import { useOreVaultStore } from '@/lib/store';

/**
 * AiDecisions Component
 * 
 * Real-time display of AI analysis:
 * - Model used and latency
 * - Recommendation (deploy/skip)
 * - Confidence level
 * - Reasoning
 * - Recent transactions
 */
export function AiDecisions() {
  const { aiAnalysis, decision, transactions, isRunning } = useOreVaultStore();

  const displayBlock = (idx: number) => idx + 1;

  return (
    <div className="bg-surface rounded-lg border border-border p-6">
      <h2 className="text-lg font-semibold mb-4">AI Decisions</h2>

      {!isRunning ? (
        <p className="text-muted text-sm">Start mining to see AI decisions</p>
      ) : !aiAnalysis ? (
        <div className="flex items-center gap-2 text-muted">
          <div className="w-2 h-2 bg-primary rounded-full animate-pulse" />
          <span className="text-sm">Waiting for next round...</span>
        </div>
      ) : (
        <>
          {/* AI Analysis Card */}
          <div className="bg-surface-light rounded-lg p-4 mb-4">
            {/* Header */}
            <div className="flex justify-between items-start mb-3">
              <div>
                <span className={`text-lg font-bold ${
                  aiAnalysis.recommendation === 'DEPLOY' ? 'text-primary' : 'text-warning'
                }`}>
                  {aiAnalysis.recommendation}
                </span>
                {decision.block !== null && aiAnalysis.recommendation === 'DEPLOY' && (
                  <span className="ml-2 text-muted">→ Block {displayBlock(decision.block)}</span>
                )}
              </div>
              <div className="text-right">
                <p className="text-xs text-muted">{aiAnalysis.model}</p>
                <p className={`text-xs font-mono ${
                  aiAnalysis.latencyMs < 500 ? 'text-primary' : 
                  aiAnalysis.latencyMs < 1000 ? 'text-warning' : 'text-danger'
                }`}>
                  {aiAnalysis.latencyMs}ms
                </p>
              </div>
            </div>

            {/* Confidence Bar */}
            <div className="mb-3">
              <div className="flex justify-between text-xs mb-1">
                <span className="text-muted">Confidence</span>
                <span className="font-mono">{(aiAnalysis.confidence * 100).toFixed(0)}%</span>
              </div>
              <div className="w-full bg-surface rounded-full h-2">
                <div
                  className={`h-2 rounded-full transition-all ${
                    aiAnalysis.confidence > 0.7 ? 'bg-primary' :
                    aiAnalysis.confidence > 0.4 ? 'bg-warning' : 'bg-danger'
                  }`}
                  style={{ width: `${aiAnalysis.confidence * 100}%` }}
                />
              </div>
            </div>

            {/* Reasoning */}
            <p className="text-sm text-muted">{aiAnalysis.reasoning}</p>
          </div>

          {/* Current Decision */}
          {decision.action && (
            <div className="flex items-center gap-2 text-sm mb-4">
              <span className="text-muted">Current:</span>
              <span className={`font-medium ${
                decision.action === 'deploy' ? 'text-primary' : 'text-warning'
              }`}>
                {decision.action.toUpperCase()}
              </span>
              {decision.block !== null && (
                <span className="font-mono">Block {displayBlock(decision.block)}</span>
              )}
              {decision.ev > 0 && (
                <span className="text-xs text-muted">(EV: {decision.ev.toFixed(4)})</span>
              )}
            </div>
          )}
        </>
      )}

      {/* Recent Transactions */}
      {transactions.length > 0 && (
        <div className="mt-4 pt-4 border-t border-border">
          <h3 className="text-sm font-medium mb-2">Recent Transactions</h3>
          <div className="space-y-2 max-h-48 overflow-y-auto">
            {transactions.slice(0, 10).map((tx) => (
              <div
                key={tx.signature}
                className="flex items-center justify-between text-xs"
              >
                <div className="flex items-center gap-2">
                  <div className={`w-2 h-2 rounded-full ${
                    tx.status === 'confirmed' ? 'bg-primary' :
                    tx.status === 'failed' ? 'bg-danger' : 'bg-warning animate-pulse'
                  }`} />
                  <a
                    href={`https://solscan.io/tx/${tx.signature}`}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="font-mono hover:text-primary"
                  >
                    {tx.signature.slice(0, 8)}...
                  </a>
                </div>
                <div className="flex items-center gap-2">
                  <span className="text-muted">Block {displayBlock(tx.block)}</span>
                  <span className="font-mono">{tx.amount.toFixed(4)} SOL</span>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
