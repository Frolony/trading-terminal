// Reconstructed 2026-10-21 from [sources]; not byte-identical to the deployed original.
// Sources: api/raydium/quote.rs.partial (genuine head: POST body fields input_mint/output_mint/amount/
//   slippage_bps; forwards to Raydium with inputMint/outputMint/amount/slippageBps/txVersion) and
//   api/raydium/build.rs.partial (genuine head: POST body wallet_pubkey/input_mint/output_mint/quote; it
//   validates the raw quote at /data/inputMint, /data/outputMint, success==true, id present and
//   /data/outputAmount — so the quote endpoint MUST hand the raw Raydium quote JSON back to the client
//   for the build call); api/raydium/pairs.rs ({ok:true, tokens, pairs} envelope proves the {ok,
//   data?, error?} envelope convention); master plan §2.5 (quote UI facts: est. output, min after
//   slippage, route-leg count, price impact, 25 s refresh).
// UNEVIDENCED (flagged): endpoint paths '/api/raydium/quote' and '/api/raydium/build' (strongly implied
//   by the Vercel Rust bin->route layout api/raydium/<bin>.rs, never observed); the exact success-payload
//   field names (quote.rs tail is missing, so the parser tolerantly accepts {quote:<raydium JSON>} or the
//   bare Raydium JSON and snake_case fallbacks expected_out_raw/min_out_raw/output_decimals/output_symbol/
//   price_impact_pct); QuoteResult field naming (minimumOut chosen over otherAmountThreshold); the
//   fallback output-decimals default of 6; user-facing error-message text.

export interface QuoteResult {
  inputMint: string;
  outputMint: string;
  /** Amount being sold, in input-token base units. */
  inputAmount: string;
  /** Estimated amount received, in output-token base units. */
  outputAmount: string;
  /** Minimum received after slippage, in output-token base units. */
  minimumOut: string;
  /** Raydium price impact percent, when provided. */
  priceImpactPct: number | null;
  /** Number of route legs (routePlan entries). */
  routeLegs: number;
  /** One entry per routePlan leg, when the payload names both mints; [] when it does not. */
  route: RouteLeg[];
  outputSymbol: string;
  outputDecimals: number;
  /** Epoch ms when the quote was fetched (drives the quote-age/refresh UI). */
  fetchedAt: number;
  /** Raw Raydium quote JSON, passed back to /api/raydium/build for signing. */
  raw: unknown;
}

/** One Raydium routePlan leg, in payload terms only. poolId is the leg's pool
 *  key (ammKey) when the payload carries one, else null — never invented.
 *  feeLabel is the leg's own fee as labelled at parse time (N3.140): a feeBps
 *  value reads '<v> bps' (the field name carries the unit); any other fee
 *  field is quoted verbatim with its provenance, never silently re-unit-ed;
 *  null when the payload reports no fee for the leg. */
export interface RouteLeg {
  inputMint: string;
  outputMint: string;
  poolId: string | null;
  feeLabel: string | null;
}

/** Parse a raw Raydium routePlan into RouteLegs. Returns [] unless EVERY leg
 *  names both mints: a partial route would misname an intermediate, so an
 *  incomplete plan is reported as no route, not as a guessed one. */
export function parseRouteLegs(plan: unknown): RouteLeg[] {
  if (!Array.isArray(plan) || plan.length === 0) return [];
  const legs: RouteLeg[] = [];
  for (const entry of plan) {
    const rec = asRecord(entry);
    const swap = asRecord(rec.swapInfo);
    const source = swap.inputMint ? swap : rec;
    const inputMint = asNonEmptyString(source.inputMint);
    const outputMint = asNonEmptyString(source.outputMint);
    if (!inputMint || !outputMint) return [];
    const poolId = asNonEmptyString(swap.ammKey) ?? asNonEmptyString(swap.poolId) ?? asNonEmptyString(rec.ammKey) ?? asNonEmptyString(rec.poolId);
    legs.push({ inputMint, outputMint, poolId, feeLabel: parseLegFeeLabel(swap, rec) });
  }
  return legs;
}

/** One routePlan entry's fee as a display value, or null when it carries none.
 *  A bare number or non-empty string only — objects and booleans are not fees. */
function legFeeValue(value: unknown): string | null {
  if (typeof value === 'number' && Number.isFinite(value)) return String(value);
  if (typeof value === 'string' && value.trim() !== '') return value.trim();
  return null;
}

/** N3.140 fee label for one leg. swapInfo-first, like poolId, so a leg's fee
 *  rides with its own pool. feeBps wins because its unit is in its name;
 *  feeRate and fee fall back verbatim with provenance because their unit is
 *  unverified upstream — re-unit-ing them would invent a figure. */
function parseLegFeeLabel(swap: Record<string, unknown>, rec: Record<string, unknown>): string | null {
  const bps = legFeeValue(swap.feeBps) ?? legFeeValue(rec.feeBps);
  if (bps !== null) return bps + ' bps';
  const rate = legFeeValue(swap.feeRate) ?? legFeeValue(rec.feeRate);
  if (rate !== null) return rate + ' (rate as reported)';
  const fee = legFeeValue(swap.fee) ?? legFeeValue(rec.fee);
  if (fee !== null) return fee + ' (as reported)';
  return null;
}

/** Shorten a mint/pool id for display: first 4 + ellipsis + last 4. Short ids
 *  pass through unchanged. */
export function shortId(id: string): string {
  if (id.length <= 8) return id;
  return id.slice(0, 4) + '…' + id.slice(-4);
}

// N3.121 (2026-11-03, A263): plain-words route trace for a /raydium/ quote.
// Pure: takes parsed legs plus a mint->display resolver (the panel resolves
// known mints to symbols and falls back to shortId for unknown ones), so no
// symbol or pool is ever invented here. A route the payload does not name
// renders as 'not reported by this quote'.
export function quoteRouteText(legs: RouteLeg[], symbolFor: (mint: string) => string): string {
  if (!Array.isArray(legs) || legs.length === 0) return 'not reported by this quote';
  const chain: string[] = [symbolFor(legs[0].inputMint)];
  for (const leg of legs) {
    chain.push(symbolFor(leg.outputMint));
  }
  const path = chain.join(' → ');
  const pools: string[] = [];
  for (const leg of legs) {
    if (leg.poolId) pools.push(shortId(leg.poolId));
  }
  if (legs.length === 1) {
    if (pools.length === 1) return path + ', direct via Raydium pool ' + pools[0];
    return path + ', direct via Raydium';
  }
  if (pools.length === legs.length) return path + ' via Raydium pools ' + pools.join(', ');
  return path + ' via Raydium';
}

// N3.140 (2026-11-04, A282): hop diagram for a /raydium/ quote. This replaces
// the cryptic aggregate the 23:03 tour saw ('Route legs: 2 pools · per-leg:
// 1 · 1' — a count plus unit-less numbers with no hop attached) with one
// labelled line per hop. The header line carries the path, the pool count
// and the price impact together (S102 presentation leg: 'Route: SOL → USDT
// → USDC · 2 Raydium pools · 0% price impact'); each hop line below it, in
// smaller muted text, names its own pool and fee. A leg with no reported
// pool reads 'pool not reported' and a leg with no reported fee reads 'fee
// not reported' — a missing value is never dropped silently (dropping it is
// what let 'per-leg: 1' stand for a 2-leg route) and never guessed. A direct
// route collapses to a single hop by construction (one leg, one hop line).
// Returns null when the payload names no route, so the caller falls back to
// the existing 'not reported by this quote' line. Pure: symbols come only
// from the caller's resolver, pools and fees only from the parsed legs.
export interface RouteDiagramHop {
  pair: string;
  pool: string;
  fee: string;
}

export interface RouteDiagram {
  header: string;
  hops: RouteDiagramHop[];
}

export function quoteRouteDiagram(legs: RouteLeg[], symbolFor: (mint: string) => string, priceImpactPct: number | null): RouteDiagram | null {
  if (!Array.isArray(legs) || legs.length === 0) return null;
  const chain: string[] = [symbolFor(legs[0].inputMint)];
  for (const leg of legs) {
    chain.push(symbolFor(leg.outputMint));
  }
  const path = chain.join(' → ');
  const poolWord = legs.length === 1 ? '1 Raydium pool' : legs.length + ' Raydium pools';
  const impact = priceImpactPct === null ? 'price impact not reported' : priceImpactPct + '% price impact';
  const hops: RouteDiagramHop[] = legs.map((leg) => ({
    pair: symbolFor(leg.inputMint) + ' → ' + symbolFor(leg.outputMint),
    pool: leg.poolId ? 'pool ' + shortId(leg.poolId) : 'pool not reported',
    fee: leg.feeLabel ? 'fee ' + leg.feeLabel : 'fee not reported',
  }));
  return { header: path + ' · ' + poolWord + ' · ' + impact, hops };
}

/** N3.140: the diagram's fees as one per-hop line for the Pool fees row, so
 *  the quote panel and the review sheet can never disagree with the diagram
 *  above them — both read the same parsed legs. Null when there is no route,
 *  so the caller falls back to the honest 'included in quote' line. */
export function quoteRouteFeesText(legs: RouteLeg[], symbolFor: (mint: string) => string): string | null {
  if (!Array.isArray(legs) || legs.length === 0) return null;
  return legs.map((leg) => symbolFor(leg.inputMint) + ' → ' + symbolFor(leg.outputMint) + ': ' + (leg.feeLabel ?? 'fee not reported')).join(' · ');
}

interface ApiEnvelope {
  ok: boolean;
  data?: unknown;
  error?: { code?: string; message?: string };
}

async function postJson(path: string, body: unknown, signal?: AbortSignal): Promise<unknown> {
  let response: Response;
  try {
    response = await fetch(path, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
      signal,
    });
  } catch (err) {
    // N3.134: an aborted fetch is a user Cancel, not a backend failure —
    // rethrow it unchanged so the caller can swallow it silently instead
    // of surfacing a bogus "could not reach the backend" error.
    if (err instanceof DOMException && err.name === 'AbortError') throw err;
    throw new Error('Could not reach the swap backend. Check your connection and try again.');
  }
  let envelope: ApiEnvelope;
  try {
    envelope = (await response.json()) as ApiEnvelope;
  } catch {
    throw new Error(`Backend returned an unreadable response (HTTP ${response.status}).`);
  }
  if (!response.ok || !envelope || envelope.ok !== true || envelope.data === undefined) {
    const message = envelope && envelope.error ? envelope.error.message : undefined;
    throw new Error(message || `Backend request failed (HTTP ${response.status}).`);
  }
  return envelope.data;
}

function asRecord(value: unknown): Record<string, unknown> {
  return value !== null && typeof value === 'object' ? (value as Record<string, unknown>) : {};
}

function asNonEmptyString(value: unknown): string | null {
  return typeof value === 'string' && value.length > 0 ? value : null;
}

function asNumber(value: unknown): number | null {
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  if (typeof value === 'string' && value.trim() !== '' && !Number.isNaN(Number(value))) return Number(value);
  return null;
}

export async function getQuote(req: {
  inputMint: string;
  outputMint: string;
  amountRaw: string;
  slippageBps: number;
  outputSymbol: string;
}, signal?: AbortSignal): Promise<QuoteResult> {
  const data = await postJson('/api/raydium/quote', {
    input_mint: req.inputMint,
    output_mint: req.outputMint,
    amount: req.amountRaw,
    slippage_bps: req.slippageBps,
  }, signal);
  const payload = asRecord(data);
  // The backend must return the raw Raydium quote JSON (build validates it). Accept it wrapped
  // as {quote: ...} or as the bare Raydium response object.
  const rawQuote = asRecord(payload.quote && typeof payload.quote === 'object' ? payload.quote : data);
  const qdata = asRecord(rawQuote.data);
  const outputAmount = asNonEmptyString(qdata.outputAmount) ?? asNonEmptyString(payload.expected_out_raw);
  const minimumOut = asNonEmptyString(qdata.otherAmountThreshold) ?? asNonEmptyString(payload.min_out_raw);
  if (!outputAmount || !minimumOut) {
    throw new Error('Backend returned an incomplete quote. Request a fresh quote.');
  }
  const routePlan = Array.isArray(qdata.routePlan) ? qdata.routePlan : [];
  const decimals = asNumber(payload.output_decimals);
  const symbol = asNonEmptyString(payload.output_symbol);
  return {
    inputMint: asNonEmptyString(qdata.inputMint) ?? req.inputMint,
    outputMint: asNonEmptyString(qdata.outputMint) ?? req.outputMint,
    inputAmount: asNonEmptyString(qdata.inputAmount) ?? req.amountRaw,
    outputAmount,
    minimumOut,
    priceImpactPct: asNumber(qdata.priceImpactPct) ?? asNumber(payload.price_impact_pct),
    routeLegs: routePlan.length,
    route: parseRouteLegs(qdata.routePlan),
    outputSymbol: symbol ?? req.outputSymbol,
    outputDecimals: decimals === null ? 6 : Math.trunc(decimals),
    fetchedAt: Date.now(),
    raw: rawQuote,
  };
}

/** Ask the backend to build an unsigned swap transaction for a raw quote. Returns the upstream JSON. */
export async function buildSwap(req: {
  walletPubkey: string;
  inputMint: string;
  outputMint: string;
  quoteRaw: unknown;
}): Promise<unknown> {
  return postJson('/api/raydium/build', {
    wallet_pubkey: req.walletPubkey,
    input_mint: req.inputMint,
    output_mint: req.outputMint,
    quote: req.quoteRaw,
  });
}
