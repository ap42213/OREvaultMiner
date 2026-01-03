'use client';

import { useState } from 'react';
import { useOreVaultStore } from '@/lib/store';
import { exportWallet } from '@/lib/api';

/**
 * MiningWallet Component
 * 
 * Displays the current mining wallet address
 * - Shows connection status
 * - Copy address to clipboard
 * - Export private key for backup
 */
export function MiningWallet() {
  const { miningWallet, miningWalletLoading, wsConnected } = useOreVaultStore();
  const [copied, setCopied] = useState(false);
  const [showExport, setShowExport] = useState(false);
  const [privateKey, setPrivateKey] = useState<string | null>(null);
  const [exporting, setExporting] = useState(false);

  if (miningWalletLoading) {
    return (
      <div className="bg-surface rounded-lg border border-border p-4">
        <div className="animate-pulse flex items-center gap-3">
          <div className="w-3 h-3 bg-surface-light rounded-full" />
          <div className="h-4 bg-surface-light rounded w-32" />
        </div>
      </div>
    );
  }

  if (!miningWallet) {
    return (
      <div className="bg-surface rounded-lg border border-border p-4">
        <div className="flex items-center gap-3">
          <div className="w-3 h-3 bg-danger rounded-full" />
          <span className="text-muted">No mining wallet</span>
        </div>
      </div>
    );
  }

  const handleCopy = async () => {
    await navigator.clipboard.writeText(miningWallet);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleExport = async () => {
    setExporting(true);
    try {
      const { private_key } = await exportWallet(miningWallet);
      setPrivateKey(private_key);
      setShowExport(true);
    } catch (e) {
      console.error('Failed to export:', e);
    } finally {
      setExporting(false);
    }
  };

  const handleCopyKey = async () => {
    if (privateKey) {
      await navigator.clipboard.writeText(privateKey);
    }
  };

  return (
    <div className="bg-surface rounded-lg border border-border p-4">
      {/* Status & Address */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className={`w-3 h-3 rounded-full ${wsConnected ? 'bg-primary animate-pulse' : 'bg-warning'}`} />
          <div>
            <p className="text-xs text-muted">Mining Wallet</p>
            <button
              onClick={handleCopy}
              className="font-mono text-sm hover:text-primary transition-colors"
            >
              {miningWallet.slice(0, 6)}...{miningWallet.slice(-4)}
              {copied && <span className="ml-2 text-primary text-xs">Copied!</span>}
            </button>
          </div>
        </div>

        <div className="flex items-center gap-2">
          {/* Solscan Link */}
          <a
            href={`https://solscan.io/account/${miningWallet}`}
            target="_blank"
            rel="noopener noreferrer"
            className="text-xs text-muted hover:text-white transition-colors"
          >
            Solscan ↗
          </a>

          {/* Export Button */}
          <button
            onClick={handleExport}
            disabled={exporting}
            className="text-xs text-muted hover:text-warning transition-colors"
          >
            {exporting ? '...' : 'Export'}
          </button>
        </div>
      </div>

      {/* Connection Status */}
      <p className="text-xs text-muted mt-2">
        {wsConnected ? 'Connected to mining server' : 'Connecting...'}
      </p>

      {/* Export Modal */}
      {showExport && privateKey && (
        <div className="fixed inset-0 bg-black/80 flex items-center justify-center z-50">
          <div className="bg-surface border border-border rounded-lg p-6 max-w-md w-full mx-4">
            <h3 className="text-lg font-semibold mb-2">Private Key Backup</h3>
            <p className="text-sm text-warning mb-4">
              ⚠️ Never share this! Import into Backpack/Hush to access funds.
            </p>
            
            <div className="bg-surface-light p-3 rounded font-mono text-xs break-all mb-4">
              {privateKey}
            </div>

            <div className="flex gap-2">
              <button
                onClick={handleCopyKey}
                className="flex-1 py-2 bg-primary text-black rounded font-medium hover:bg-primary/80"
              >
                Copy Key
              </button>
              <button
                onClick={() => {
                  setShowExport(false);
                  setPrivateKey(null);
                }}
                className="flex-1 py-2 bg-surface-light text-white rounded hover:bg-border"
              >
                Close
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
