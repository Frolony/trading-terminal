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
  outputSymbol: string;
  outputDecimals: number;
  /** Epoch ms when the quote was fetched (drives the quote-age/refresh UI). */
  fetchedAt: number;
  /** Raw Raydium quote JSON, passed back to /api/raydium/build for signing. */
  raw: unknown;
}

interface ApiEnvelope {
  ok: boolean;
  data?: unknown;
  error?: { code?: string; message?: string };
}

async function postJson(path: string, body: unknown): Promise<unknown> {
  let response: Response;
  try {
    response = await fetch(path, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });
  } catch {
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
}): Promise<QuoteResult> {
  const data = await postJson('/api/raydium/quote', {
    input_mint: req.inputMint,
    output_mint: req.outputMint,
    amount: req.amountRaw,
    slippage_bps: req.slippageBps,
  });
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
