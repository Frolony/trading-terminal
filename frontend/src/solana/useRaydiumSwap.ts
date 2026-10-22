// Reconstructed 2026-10-21 from [sources]; not byte-identical to the deployed original.
// Sources: the deployed SwapPanel.tsx.partial head (destructures {getQuote, executeQuote, connected} from
//   useRaydiumSwap and passes getQuote {inputMint, outputMint, amountRaw, slippageBps, outputSymbol});
//   build.rs.partial (the build call needs the raw Raydium quote JSON, supplied via QuoteResult.raw);
//   @solana/wallet-adapter-react 0.15.40 + @solana/web3.js 1.98.4 (useWallet/useConnection,
//   VersionedTransaction.deserialize, wallet.signTransaction, connection.sendRawTransaction).
// UNEVIDENCED (flagged): the exact /build success-payload shape (tail of build.rs missing), so the
//   base64 transaction is extracted tolerantly — explicit transaction/transactions/data/result variants
//   first (incl. the nested Raydium {data:[{transaction}]} shape), then a bounded deep search for a
//   base64-looking string; no on-chain confirmation wait inside executeQuote — the returned signature
//   is from sendRawTransaction only (the panel shows it in a pending state immediately, then calls
//   confirmSwap below); the hook additionally exposes the wallet publicKey (base58 or null) so the
//   panel can pass it to executeQuote per the (quote, walletPubkey) contract.
// N4.2 (2026-10-22): added confirmSwap — waits for 'confirmed' commitment after sendRawTransaction,
//   surfacing the on-chain error instead of swallowing it; the panel renders pending → confirmed/failed.

import { useCallback } from 'react';
import { useConnection, useWallet } from '@solana/wallet-adapter-react';
import { PublicKey, VersionedTransaction } from '@solana/web3.js';
import { buildSwap, getQuote } from './raydiumApi';
import type { QuoteResult } from './raydiumApi';
import { CONFIRM_COMMITMENT, CONFIRM_TIMEOUT_MS } from './swapFlow';

/** lamports → trimmed SOL string (up to 9 decimals, no float-rounding of base units elsewhere). */
function trimLamports(lamports: number): string {
  return (lamports / 1_000_000_000).toFixed(9).replace(/\.?0+$/, '');
}

/** Sum of uiAmount strings, trimmed (RPC already trims; the sum needs it). */
function trimTokenSum(total: number): string {
  return total.toFixed(9).replace(/\.?0+$/, '');
}

export interface QuoteRequest {
  inputMint: string;
  outputMint: string;
  amountRaw: string;
  slippageBps: number;
  outputSymbol: string;
}

function decodeBase64(value: string): Uint8Array {
  const binary = atob(value);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

function looksLikeBase64Tx(value: unknown): value is string {
  return (
    typeof value === 'string' && value.length >= 64 && /^[A-Za-z0-9+/=_-]+$/.test(value)
  );
}

/** Tolerantly pull the base64 unsigned transaction out of the /build response. */
function extractBase64Transaction(payload: unknown): string {
  if (looksLikeBase64Tx(payload)) return payload;
  const rec = payload !== null && typeof payload === 'object' ? (payload as Record<string, unknown>) : null;
  if (rec) {
    const direct =
      (Array.isArray(rec.transactions) ? rec.transactions.find(looksLikeBase64Tx) : undefined) ??
      (looksLikeBase64Tx(rec.transaction) ? rec.transaction : undefined) ??
      (looksLikeBase64Tx(rec.data) ? rec.data : undefined) ??
      (looksLikeBase64Tx(rec.result) ? rec.result : undefined);
    if (direct) return direct;
    // Raw Raydium /transaction/swap shape: {data:[{transaction: "<base64>", ...}]}
    const nested = rec.data;
    if (Array.isArray(nested)) {
      for (const leg of nested) {
        const legRec = leg !== null && typeof leg === 'object' ? (leg as Record<string, unknown>) : null;
        if (legRec && looksLikeBase64Tx(legRec.transaction)) return legRec.transaction;
      }
    } else if (nested !== null && typeof nested === 'object') {
      const nestedRec = nested as Record<string, unknown>;
      if (looksLikeBase64Tx(nestedRec.transaction)) return nestedRec.transaction;
    }
    // Last resort: bounded deep search for a base64-looking string.
    const seen = new Set<unknown>();
    const walk = (node: unknown): string | null => {
      if (node === null || node === undefined || seen.has(node)) return null;
      if (typeof node === 'object') seen.add(node);
      if (looksLikeBase64Tx(node)) return node;
      if (Array.isArray(node)) {
        for (const item of node) {
          const hit = walk(item);
          if (hit) return hit;
        }
        return null;
      }
      if (typeof node === 'object') {
        for (const value of Object.values(node as Record<string, unknown>)) {
          const hit = walk(value);
          if (hit) return hit;
        }
      }
      return null;
    };
    const found = walk(rec);
    if (found) return found;
  }
  throw new Error('Backend did not return a transaction. Request a fresh quote.');
}

export function useRaydiumSwap() {
  const { connection } = useConnection();
  const { publicKey, connected, signTransaction } = useWallet();

  const quote = useCallback((req: QuoteRequest): Promise<QuoteResult> => getQuote(req), []);

  const executeQuote = useCallback(
    async (quoteResult: QuoteResult, walletPubkey: string): Promise<string> => {
      if (!publicKey || !signTransaction) throw new Error('Connect a wallet before swapping.');
      const built = await buildSwap({
        walletPubkey,
        inputMint: quoteResult.inputMint,
        outputMint: quoteResult.outputMint,
        quoteRaw: quoteResult.raw,
      });
      const base64Tx = extractBase64Transaction(built);
      const transaction = VersionedTransaction.deserialize(decodeBase64(base64Tx));
      const signed = await signTransaction(transaction);
      return connection.sendRawTransaction(signed.serialize());
    },
    [connection, publicKey, signTransaction],
  );

  return {
    getQuote: quote,
    executeQuote,
    connected,
    publicKey: publicKey ? publicKey.toBase58() : null,
    /**
     * Wait for the sent transaction to reach 'confirmed' commitment.
     * Resolves with the on-chain error (stringified) or null when clean.
     * Rejects on timeout — the transaction may still confirm, so the caller
     * must show the explorer link rather than a plain error.
     */
    confirmSwap: useCallback(
      async (signature: string): Promise<string | null> => {
        const wait = connection.confirmTransaction(signature, CONFIRM_COMMITMENT);
        const timeout = new Promise<never>((_, reject) =>
          setTimeout(() => reject(new Error('Confirmation timed out')), CONFIRM_TIMEOUT_MS),
        );
        const result = await Promise.race([wait, timeout]);
        const err = result.value.err;
        if (err === null) return null;
        return typeof err === 'string' ? err : JSON.stringify(err);
      },
      [connection],
    ),
    /**
     * Post-swap balance refresh. Returns display strings or null per token — a null renders as
     * "unavailable", never as $0/0. SOL via getBalance; the output token via
     * getTokenAccountsByOwner/jsonParsed summed uiAmountString.
     */
    fetchPostSwapBalances: useCallback(
      async (outputMint: string): Promise<{ sol: string | null; token: string | null }> => {
        const out: { sol: string | null; token: string | null } = { sol: null, token: null };
        if (!publicKey) return out;
        try {
          out.sol = trimLamports(await connection.getBalance(publicKey));
        } catch {
          /* stays null → panel renders "unavailable" */
        }
        try {
          const accounts = await connection.getParsedTokenAccountsByOwner(
            publicKey,
            { mint: new PublicKey(outputMint) },
          );
          let total = 0;
          let seen = false;
          for (const entry of accounts.value) {
            const raw = entry.account.data.parsed?.info?.tokenAmount?.uiAmountString;
            const n = typeof raw === 'string' ? Number(raw) : NaN;
            if (Number.isFinite(n)) {
              total += n;
              seen = true;
            }
          }
          out.token = seen ? trimTokenSum(total) : null;
        } catch {
          /* stays null → panel renders "unavailable" */
        }
        return out;
      },
      [connection, publicKey],
    ),
  };
}
