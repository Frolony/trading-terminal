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

// ---------------------------------------------------------------------------
// N4.4 (2026-10-22) — pre-trade safety: slippage control, price-impact ladder,
// fee lines, simulation gating. Pure helpers only (Node-testable); the panel
// and hook own all RPC/adapter calls.
// ---------------------------------------------------------------------------

/** Canonical slippage presets, basis points (pack §3.1). */
export const SLIPPAGE_PRESETS_BPS = [10, 50, 100] as const;

/** Default slippage: 0.5% (matches the pre-N4.4 50 bps default). */
export const DEFAULT_SLIPPAGE_BPS = 50;

/** Custom slippage bounds: 0.01% – 10% (pack §3.1.3). */
export const MIN_SLIPPAGE_BPS = 1;
export const MAX_SLIPPAGE_BPS = 1000;

export type SlippageParse = { ok: true; bps: number } | { ok: false; error: string };

/**
 * Validate a user-entered slippage value (basis points). Accepts integers
 * only; rejects out-of-range values with the specific reason (pack §3.5 —
 * no silent or generic failures).
 */
export function parseSlippageInput(value: string): SlippageParse {
  const trimmed = value.trim();
  if (!/^\d+$/.test(trimmed)) {
    return { ok: false, error: 'Slippage must be a whole number of basis points (e.g. 50 for 0.50%).' };
  }
  const bps = Number(trimmed);
  if (bps < MIN_SLIPPAGE_BPS) {
    return { ok: false, error: `Slippage must be at least ${MIN_SLIPPAGE_BPS} bp (0.01%).` };
  }
  if (bps > MAX_SLIPPAGE_BPS) {
    return {
      ok: false,
      error:
        'Slippage above 10% is disallowed; a quote this loose usually means a bad route. Pick a deeper pair or split the trade.',
    };
  }
  return { ok: true, bps };
}

/** Price-impact warning bands (pack §3.2.2 — UI policy, not physics). */
export type ImpactBand = 'unknown' | 'low' | 'elevated' | 'high' | 'blocked';

/**
 * Classify a quote's price-impact percent into the warning ladder.
 * - null/NaN → 'unknown' (not reported by the quote — render neutrally, never as 0%)
 * - < 1%    → 'low'      (green/neutral, no warning)
 * - 1–5%    → 'elevated' (amber: consider a smaller size)
 * - 5–15%   → 'high'     (red: extra acknowledgement required before confirm)
 * - > 15%   → 'blocked'  (confirm disabled; blocked in Paper AND Live per pack §6)
 */
export function impactBand(impactPct: number | null): ImpactBand {
  if (impactPct === null || !Number.isFinite(impactPct) || impactPct < 0) return 'unknown';
  if (impactPct < 1) return 'low';
  if (impactPct < 5) return 'elevated';
  if (impactPct <= 15) return 'high';
  return 'blocked';
}

/** Human copy per impact band (pack §3.2.2 ladder wording). */
export function impactNotice(band: ImpactBand, impactPct: number | null): string | null {
  const shown = impactPct === null ? '—' : `${impactPct.toFixed(2)}%`;
  switch (band) {
    case 'low':
      return null;
    case 'elevated':
      return `High price impact (${shown}). Consider a smaller size.`;
    case 'high':
      return `Very high price impact (${shown}). This trade is likely a bad deal — acknowledge below to continue.`;
    case 'blocked':
      return `Impact above 15% is blocked to protect you (${shown}). Split the trade or pick a deeper pair.`;
    case 'unknown':
    default:
      return 'The quote did not report price impact.';
  }
}

const WSOL_MINT = 'So11111111111111111111111111111111111111112';

/** Derive an approximate SOL/USD price from a SOL↔USDC quote (for the fee USD line).
 *  Returns null when the quote does not involve a SOL↔USDC leg — the caller must
 *  then render the honest "USD estimate unavailable" instead of inventing one. */
export function solPriceUsdFromQuote(quote: QuoteResult): number | null {
  const inAmt = Number(quote.inputAmount);
  const outAmt = Number(quote.outputAmount);
  if (!Number.isFinite(inAmt) || !Number.isFinite(outAmt) || inAmt <= 0 || outAmt <= 0) return null;
  const inIsSol = quote.inputMint === WSOL_MINT;
  const outIsSol = quote.outputMint === WSOL_MINT;
  const inIsUsdc = quote.inputMint === 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v';
  const outIsUsdc = quote.outputMint === 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v';
  // Raydium quotes are in base units; adjust by each side's decimals.
  const inUnits = inAmt / 10 ** (inIsSol ? 9 : 6);
  const outUnits = outAmt / 10 ** (outIsSol ? 9 : quote.outputDecimals);
  if (inUnits <= 0 || outUnits <= 0) return null;
  if (inIsSol && outIsUsdc) return outUnits / inUnits; // SOL → USDC: USDC per SOL
  if (inIsUsdc && outIsSol) return inUnits / outUnits; // USDC → SOL: invert
  return null;
}

/** Lamports → trimmed SOL string (matches the hook's trimLamports; pure duplicate for tests). */
export function lamportsToSol(lamports: number): string {
  return (lamports / 1_000_000_000).toFixed(9).replace(/\.?0+$/, '');
}

/** Simulation + prepare-state machine for the review sheet (pack §3.4). */
export type PrepareState = 'idle' | 'preparing' | 'ready' | 'blocked';

/** Extra N4.4 copy for the safety-gated review sheet. */
export const SAFETY_COPY = {
  liveChip: 'LIVE — real transaction',
  preparing: 'Preparing transaction — building, simulating, and estimating fees…',
  slippageChanged:
    'Slippage changed — the current quote was fetched under different slippage. Request a fresh quote before confirming.',
  simUnavailable:
    'Pre-flight simulation is not available in this build — confirm disabled. Do not sign without simulation.',
  simFailedPrefix: 'Simulation failed: ',
  simFailedSuffix: ' This transaction would likely fail on-chain. Fix the cause or request a fresh quote.',
  feeUnavailable:
    'Network fee estimate unavailable — confirm disabled. A sign without a known fee is not allowed.',
  poolFeesHonest: 'Pool fees included in quote (not separately reported)',
  impactUnknown: '—',
  confirmLabel: 'Sign & send (Live)',
  ackLabel: 'I understand the price impact is very high and this trade is likely a bad deal.',
  retryPrepare: 'Retry transaction check',
} as const;

/**
 * Tolerantly read a pool-fee note from the raw Raydium quote JSON.
 * Returns null when the payload exposes no fee field — the caller must then
 * render the honest "included in quote (not separately reported)" line
 * (pack §3.3.2) instead of inventing a number. Upstream shape unverified.
 */
export function poolFeeNote(raw: unknown): string | null {
  const rec = raw !== null && typeof raw === 'object' ? (raw as Record<string, unknown>) : null;
  if (!rec) return null;
  const data =
    rec.data !== null && typeof rec.data === 'object' ? (rec.data as Record<string, unknown>) : rec;
  const direct = data.fee ?? data.feeBps ?? data.totalFee;
  if (typeof direct === 'number' && Number.isFinite(direct)) return `${direct} (as reported by quote)`;
  if (typeof direct === 'string' && direct.trim() !== '') return `${direct.trim()} (as reported by quote)`;
  const plan = Array.isArray(data.routePlan) ? data.routePlan : [];
  const legFees: string[] = [];
  for (const leg of plan) {
    const l = leg !== null && typeof leg === 'object' ? (leg as Record<string, unknown>) : null;
    if (!l) continue;
    const f = l.feeBps ?? l.fee ?? l.feeRate;
    if (typeof f === 'number' && Number.isFinite(f)) legFees.push(String(f));
    else if (typeof f === 'string' && f.trim() !== '') legFees.push(f.trim());
  }
  if (legFees.length) return `per-leg: ${legFees.join(' · ')} (as reported by quote)`;
  return null;
}

/**
 * Shared slippage store — one store, two render sites (pack §3.1.1).
 * The / ticket (src/index.html) and the /raydium/ panel (SwapPanel.tsx) both
 * read and write this localStorage key, so a slippage change on either page
 * is honoured on the other. Values are basis points, validated through
 * parseSlippageInput — never persisted unvalidated.
 */
export const SHARED_SLIPPAGE_KEY = 'gecko-terminal:slippage-bps:v1';

/** Read the shared slippage (bps); falls back to the default on missing/corrupt/unavailable storage. */
export function readSharedSlippage(): number {
  try {
    if (typeof localStorage === 'undefined') return DEFAULT_SLIPPAGE_BPS;
    const raw = localStorage.getItem(SHARED_SLIPPAGE_KEY);
    if (raw === null || raw === '') return DEFAULT_SLIPPAGE_BPS;
    const parsed = parseSlippageInput(raw);
    return parsed.ok ? parsed.bps : DEFAULT_SLIPPAGE_BPS;
  } catch {
    return DEFAULT_SLIPPAGE_BPS;
  }
}

/**
 * Persist a validated slippage value. Returns false (nothing written) when
 * the value is invalid or storage is unavailable — callers keep their current
 * valid value in that case.
 */
export function writeSharedSlippage(bps: number): boolean {
  if (!Number.isInteger(bps) || bps < MIN_SLIPPAGE_BPS || bps > MAX_SLIPPAGE_BPS) return false;
  try {
    if (typeof localStorage === 'undefined') return false;
    localStorage.setItem(SHARED_SLIPPAGE_KEY, String(bps));
    return true;
  } catch {
    return false;
  }
}
