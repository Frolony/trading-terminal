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

import { useEffect, useMemo, useRef, type ReactNode } from 'react';
import { ConnectionProvider, WalletProvider, useWallet } from '@solana/wallet-adapter-react';
import { WalletModalProvider } from '@solana/wallet-adapter-react-ui';
import { PhantomWalletAdapter, SolflareWalletAdapter } from '@solana/wallet-adapter-wallets';

const MAINNET_ENDPOINT = 'https://api.mainnet-beta.solana.com';

// N2.8 (2026-10-26, A78): this page shares the `/` page's wallet-session key
// (gecko-terminal:wallet-session:v1). The shape mirrors the `/` page's
// writer exactly (version 1, active, roster, sync) — sync is preserved, never
// invented here, because sync auth lives on the `/` page. A connect on this
// page persists the session (so `/` can restore it); a disconnect here
// forgets it (so `/` does not restore it); and autoConnect on this page is
// gated on the key, so an explicit disconnect on either surface sticks on
// both. The autoConnect itself is the adapters' trusted-only path (the
// react provider calls adapter.autoConnect()), so it can never pop up.
// One honest limit: the react provider can only autoConnect a wallet it has
// selected, so a wallet first connected on `/` restores here only after one
// manual select on this page (SessionSync performs that select from the
// shared key); after that, restores are silent.
const WALLET_SESSION_KEY = 'gecko-terminal:wallet-session:v1';
type SessionWallet = { pubkey: string; name: string; kind: string };
type SessionDoc = { version: number; active: SessionWallet; roster: SessionWallet[]; sync: { pubkey: string; token: string; expiresAtMs: number } | null };
function readSession(): SessionDoc | null {
  try {
    const raw = window.localStorage.getItem(WALLET_SESSION_KEY);
    if (!raw) return null;
    const j = JSON.parse(raw) as Partial<SessionDoc>;
    if (!j || j.version !== 1 || !j.active || typeof j.active.pubkey !== 'string' || !j.active.pubkey) return null;
    if (!Array.isArray(j.roster)) return null;
    const roster = (j.roster as SessionWallet[]).filter((e) => e && typeof e.pubkey === 'string' && e.pubkey);
    if (!roster.some((e) => e.pubkey === j.active!.pubkey)) roster.unshift({ pubkey: j.active.pubkey, name: j.active.name || 'Wallet', kind: j.active.kind || 'adapter' });
    return { version: 1, active: j.active, roster, sync: j.sync ?? null };
  } catch {
    return null;
  }
}
function writeSession(doc: SessionDoc): void {
  try {
    window.localStorage.setItem(WALLET_SESSION_KEY, JSON.stringify(doc));
  } catch {
    /* storage unavailable: session simply does not persist */
  }
}
function forgetSession(pubkey: string): void {
  try {
    const s = readSession();
    if (!s) return;
    const roster = s.roster.filter((e) => e.pubkey !== pubkey);
    if (!roster.length) {
      window.localStorage.removeItem(WALLET_SESSION_KEY);
      return;
    }
    const active = s.active.pubkey !== pubkey ? s.active : roster[0];
    const sync = s.sync && s.sync.pubkey !== pubkey ? s.sync : null;
    writeSession({ version: 1, active, roster, sync });
  } catch {
    /* ignore */
  }
}
function SessionSync(): null {
  const { connected, publicKey, wallet, select } = useWallet();
  const prevRef = useRef<string | null>(null);
  const selectedRef = useRef(false);
  useEffect(() => {
    if (selectedRef.current || connected || wallet) return;
    selectedRef.current = true;
    const s = readSession();
    if (!s || !s.active || !s.active.name) return;
    try {
      (select as (name: string) => void)(s.active.name);
    } catch {
      /* unknown wallet name: no autoConnect, user selects manually */
    }
  }, [connected, wallet, select]);
  useEffect(() => {
    if (connected && publicKey) {
      const pk = publicKey.toBase58();
      const prev = readSession();
      const roster: SessionWallet[] = prev
        ? prev.roster.filter((e) => e.pubkey !== pk)
        : [];
      roster.unshift({ pubkey: pk, name: wallet?.adapter.name ?? 'Wallet', kind: 'adapter' });
      const sync = prev && prev.sync && prev.sync.pubkey === pk ? prev.sync : null;
      writeSession({ version: 1, active: { pubkey: pk, name: wallet?.adapter.name ?? 'Wallet', kind: 'adapter' }, roster, sync });
      prevRef.current = pk;
    } else if (!connected && prevRef.current) {
      forgetSession(prevRef.current);
      prevRef.current = null;
    }
  }, [connected, publicKey, wallet]);
  return null;
}

// N4.9 (2026-10-24, A44) as amended by N2.8: autoConnect is ON only when a
// persisted session exists (see above) — a wallet connects silently on load
// only if the user connected it before and never explicitly disconnected it.
export function SolanaWalletProvider({ children }: { children: ReactNode }) {
  const wallets = useMemo(() => [new PhantomWalletAdapter(), new SolflareWalletAdapter()], []);
  const restoreAutoConnect = useMemo(() => {
    const s = readSession();
    return !!(s && s.active && s.active.pubkey);
  }, []);
  return (
    <ConnectionProvider endpoint={MAINNET_ENDPOINT}>
      <WalletProvider wallets={wallets} autoConnect={restoreAutoConnect}>
        <WalletModalProvider>
          <SessionSync />
          {children}
        </WalletModalProvider>
      </WalletProvider>
    </ConnectionProvider>
  );
}
