// N4.2 (2026-10-22): pure swap-flow helpers for the /raydium/ review-sign-confirm path.
// Dependency-free on purpose so the guards are unit-testable in Node: the wallet adapter
// and web3.js never touch these functions. All amounts stay in base-unit strings until
// formatBaseUnits is called at render time — never float-round an amount that gets signed.

import type { QuoteResult } from './raydiumApi';

/** Quote freshness window, ms. Must match the panel's auto-refresh interval (currently 25 s):
 * a quote at or past this age is NEVER signed — the UI re-quotes and asks for confirmation again. */
export const QUOTE_FRESH_MS = 25_000;

/** Confirmation wait: 'confirmed' commitment per the N4.2 pack (stated on the review sheet). */
export const CONFIRM_COMMITMENT = 'confirmed' as const;

/** How long to wait for on-chain confirmation before reporting a timeout (the tx may still confirm). */
export const CONFIRM_TIMEOUT_MS = 60_000;

/** A quote is signable only while it is younger than QUOTE_FRESH_MS. */
export function isQuoteFresh(quote: QuoteResult, nowMs: number): boolean {
  return nowMs - quote.fetchedAt < QUOTE_FRESH_MS;
}

/** Wallet adapters report user rejections with varying messages ("User rejected the request",
 * "Transaction rejected", "User cancelled", ...). Match loosely; never classify an unknown
 * failure as a rejection (a real failure must surface as an error, not a soft state). */
const REJECTION_RE = /reject|cancel(led)?|declined|denied|disapproved/i;

export function isRejectionError(err: unknown): boolean {
  if (!err) return false;
  const message =
    typeof err === 'string' ? err : err instanceof Error ? err.message : String(err);
  return REJECTION_RE.test(message);
}

/** Block-explorer link for a signature. Solscan is a UI choice (no precedent in the codebase);
 * N4.6's persistent history will reuse this. */
export function explorerUrl(signature: string): string {
  return `https://solscan.io/tx/${signature}`;
}

/** Copy contracts for the review-sign-confirm flow (single source so tests can assert them). */
export const COPY = {
  reviewTitle: 'Review your swap',
  reviewNote:
    'These numbers are exactly what the quote returned. Confirming sends the transaction to your wallet for signing — nothing is signed yet.',
  staleQuote:
    'That quote expired before you confirmed. A fresh quote was fetched — review the new numbers and confirm again.',
  waitingApproval: 'Waiting for your wallet approval…',
  pendingConfirm: 'Swap sent — awaiting on-chain confirmation',
  confirmed: 'Swap confirmed',
  rejected:
    'Signature rejected in your wallet — no transaction was sent. Review the numbers and confirm again if you wish.',
  confirmTimedOut:
    'Confirmation timed out — the transaction may still confirm. Check the explorer link before retrying.',
} as const;
