import type { Metadata } from 'next';
import './globals.css';
import { Providers } from './providers';

export const metadata: Metadata = {
  title: 'OreVault - Automated ORE v3 Mining',
  description: 'Automated mining system for ORE v3 on Solana mainnet with Jito bundles and EV-based strategy.',
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body className="antialiased min-h-screen bg-background text-white">
        <Providers>
          {children}
        </Providers>
      </body>
    </html>
  );
}
