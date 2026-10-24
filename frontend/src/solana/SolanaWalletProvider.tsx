// Reconstructed 2026-10-21 from [sources]; not byte-identical to the deployed original.
// Sources: App.tsx (imports {SolanaWalletProvider} and wraps <SwapPanel/> with it); the historical
//   TS2786 deployment failure ('ConnectionProvider' cannot be used as a JSX component — the
//   historical 10:44-deploy-era error this file once caused);
// NOTE 2026-10-21 (Stage B): explicit {children: ReactNode} typing alone did NOT fix TS2786 — the real
//   cause was a nested @types/react@19.3.0 under @solana/wallet-adapter-react (via react-native),
//   whose FC/JSX types conflict with the project's React 18 types. Fixed in frontend/package.json
//   via "overrides" pinning @types/react/@types/react-dom to the project's v18 line (no type
//   weakening; skipLibCheck untouched).
//   live-page copy ('Solana Mainnet' badge; wallet-modal helper 'Connect Phantom, Backpack, or another
//   Wallet Standard-compatible wallet to sign.' — implies Wallet Standard-only detection, hence an empty
//   wallets array rather than explicit adapters).
// UNEVIDENCED (flagged): RPC endpoint 'https://api.mainnet-beta.solana.com' (default mainnet-beta, never
//   observed in the deployed source); autoConnect was inferred enabled, now
//   explicitly OFF per N4.9/A44 (2026-10-24).
// N4.2 (2026-10-22): the empty adapter array (Wallet Standard-only detection) left the modal with zero
//   wallet options in browsers without Wallet-Standard-registered wallets (the A10 QA failure).
//   Now registers the same explicit adapters the `/` page's N4.1 modal discovers via the adapter
//   library — Phantom + Solflare — so the modal shows install CTAs instead of dead rows when nothing
//   is installed, and connect rows when it is. Wallet Standard wallets are still discovered
//   automatically alongside these.

import { useMemo, type ReactNode } from 'react';
import { ConnectionProvider, WalletProvider } from '@solana/wallet-adapter-react';
import { WalletModalProvider } from '@solana/wallet-adapter-react-ui';
import { PhantomWalletAdapter, SolflareWalletAdapter } from '@solana/wallet-adapter-wallets';

const MAINNET_ENDPOINT = 'https://api.mainnet-beta.solana.com';

// N4.9 (2026-10-24, A44): autoConnect is OFF — a wallet connects only on an
// explicit user click, matching the `/` page. Previously `autoConnect` silently
// reconnected on page load.
export function SolanaWalletProvider({ children }: { children: ReactNode }) {
  const wallets = useMemo(() => [new PhantomWalletAdapter(), new SolflareWalletAdapter()], []);
  return (
    <ConnectionProvider endpoint={MAINNET_ENDPOINT}>
      <WalletProvider wallets={wallets} autoConnect={false}>
        <WalletModalProvider>{children}</WalletModalProvider>
      </WalletProvider>
    </ConnectionProvider>
  );
}
