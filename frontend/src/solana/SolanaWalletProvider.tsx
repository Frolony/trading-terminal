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
//   observed in the deployed source); autoConnect enabled (inferred, not observed).

import { useMemo, type ReactNode } from 'react';
import { ConnectionProvider, WalletProvider } from '@solana/wallet-adapter-react';
import { WalletModalProvider } from '@solana/wallet-adapter-react-ui';

const MAINNET_ENDPOINT = 'https://api.mainnet-beta.solana.com';

export function SolanaWalletProvider({ children }: { children: ReactNode }) {
  // Wallet Standard only: the page copy names Phantom/Backpack generically instead of listing adapters.
  const wallets = useMemo(() => [], []);
  return (
    <ConnectionProvider endpoint={MAINNET_ENDPOINT}>
      <WalletProvider wallets={wallets} autoConnect>
        <WalletModalProvider>{children}</WalletModalProvider>
      </WalletProvider>
    </ConnectionProvider>
  );
}
