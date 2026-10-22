// N4.6 (2026-10-22): persistent trade-history store for the /raydium/ panel.
// Same schema as src/index.html's N4.6 block (localStorage key
// 'gecko-terminal:v1:history', version 1) so the home page's History panel
// shows live swap events under the wallet namespace. Pure + dependency-free
// (web3-free: amounts arrive as base-unit strings and are converted here
// with integer-safe arithmetic) so the guards stay Node-testable.
//
// Honesty contract (pack §3.1): live events are written ONLY after on-chain
// confirmation — SwapPanel calls recordSwapFillEvent at phase==='confirmed' —
// or with status 'failed' when confirmation itself fails (on-chain error or
// timeout). A signed-but-unconfirmed signature never lands as 'filled'.

import { TOKENS } from './tokens';

/** Must match src/index.html's HIST_KEY / HIST_VERSION / HIST_CAP. */
export const TRADE_HISTORY_KEY = 'gecko-terminal:v1:history';
const TRADE_HISTORY_VERSION = 1;
const TRADE_HISTORY_CAP = 500;

export type TradeEventMode = 'paper' | 'live';
export type TradeEventKind =
  | 'market-fill'
  | 'swap-fill'
  | 'limit-create'
  | 'limit-cancel'
  | 'limit-fill'
  | 'paper-limit-fill';
export type TradeEventStatus = 'open' | 'filled' | 'cancelled' | 'expired' | 'failed';

export interface TradeEventPair {
  baseMint: string;
  quoteMint: string;
  baseSymbol: string;
  quoteSymbol: string;
}

export interface TradeEventFees {
  networkLamports: number;
  protocolQuote: number;
  label: string;
}

export interface TradeEvent {
  id: string;
  ts: number;
  mode: TradeEventMode;
  kind: TradeEventKind;
  wallet: string | null;
  pair: TradeEventPair;
  side: 'buy' | 'sell';
  sizeBase: number;
  execPrice: number;
  notionalQuote: number;
  fees: TradeEventFees;
  quoted?: { minOut: number; slippageBps: number; minOutUnit: string };
  status: TradeEventStatus;
  txSig: string | null;
  error?: string;
  note?: string;
}

const KINDS: TradeEventKind[] = [
  'market-fill',
  'swap-fill',
  'limit-create',
  'limit-cancel',
  'limit-fill',
  'paper-limit-fill',
];
const STATUSES: TradeEventStatus[] = ['open', 'filled', 'cancelled', 'expired', 'failed'];

interface HistoryState {
  version: number;
  paper: TradeEvent[];
  live: Record<string, TradeEvent[]>;
}

function newId(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function')
    return 'h_' + crypto.randomUUID().replace(/-/g, '');
  return 'h_' + Date.now().toString(36) + '_' + Math.floor(Math.random() * 1e9).toString(36);
}

function storage(): Storage | null {
  try {
    return typeof localStorage !== 'undefined' ? localStorage : null;
  } catch {
    return null;
  }
}

function quarantine(raw: unknown): void {
  const store = storage();
  if (!store) return;
  try {
    const body =
      typeof raw === 'string'
        ? raw.slice(0, 100000)
        : JSON.stringify(raw).slice(0, 100000);
    store.setItem(TRADE_HISTORY_KEY + ':corrupt:' + Date.now(), body);
  } catch {
    /* storage unavailable — nothing to do */
  }
}

function isValidEvent(e: unknown): e is TradeEvent {
  if (!e || typeof e !== 'object') return false;
  const r = e as Record<string, unknown>;
  if (typeof r.id !== 'string' || !r.id) return false;
  if (!Number.isFinite(Number(r.ts))) return false;
  if (r.mode !== 'paper' && r.mode !== 'live') return false;
  if (KINDS.indexOf(r.kind as TradeEventKind) < 0) return false;
  if (STATUSES.indexOf(r.status as TradeEventStatus) < 0) return false;
  if (r.side !== 'buy' && r.side !== 'sell') return false;
  if (
    !Number.isFinite(Number(r.sizeBase)) ||
    !Number.isFinite(Number(r.execPrice)) ||
    !Number.isFinite(Number(r.notionalQuote))
  )
    return false;
  if (r.mode === 'live' && typeof r.wallet !== 'string') return false;
  return true;
}

export function loadTradeHistory(): HistoryState {
  const store = storage();
  const empty: HistoryState = { version: TRADE_HISTORY_VERSION, paper: [], live: {} };
  if (!store) return empty;
  try {
    const raw = store.getItem(TRADE_HISTORY_KEY);
    if (raw === null || raw === '') return empty;
    const s = JSON.parse(raw) as {
      version?: unknown;
      paper?: unknown;
      live?: unknown;
    };
    if (
      !s ||
      s.version !== TRADE_HISTORY_VERSION ||
      !Array.isArray(s.paper) ||
      !s.live ||
      typeof s.live !== 'object'
    ) {
      quarantine(raw);
      return empty;
    }
    const paper = s.paper.filter(isValidEvent).slice(0, TRADE_HISTORY_CAP);
    const live: Record<string, TradeEvent[]> = {};
    for (const k of Object.keys(s.live as Record<string, unknown>).slice(0, 50)) {
      const arr = (s.live as Record<string, unknown>)[k];
      if (Array.isArray(arr)) live[k] = arr.filter(isValidEvent).slice(0, TRADE_HISTORY_CAP);
    }
    return { version: TRADE_HISTORY_VERSION, paper, live };
  } catch {
    quarantine(null);
    return empty;
  }
}

export type NewTradeEvent = Omit<TradeEvent, 'id' | 'ts' | 'pair' | 'fees'> & {
  id?: string;
  ts?: number;
  pair?: Partial<TradeEventPair>;
  fees?: Partial<TradeEventFees>;
};

/**
 * Append-only writeNow. Live events REQUIRE a wallet pubkey (never mix
 * namespaces). Returns the stored event, or null when the event is invalid
 * or storage failed — callers treat null as "not recorded".
 */
export function recordTradeEvent(evt: NewTradeEvent): TradeEvent | null {
  if (!evt || typeof evt !== 'object') return null;
  const mode: TradeEventMode = evt.mode === 'live' ? 'live' : 'paper';
  if (mode === 'live' && (typeof evt.wallet !== 'string' || !evt.wallet)) return null;
  const p = evt.pair || {};
  const f = evt.fees || {};
  const nl = Number(f.networkLamports);
  const pq = Number(f.protocolQuote);
  const e: TradeEvent = {
    id: typeof evt.id === 'string' && evt.id ? evt.id : newId(),
    ts: Number.isFinite(Number(evt.ts)) ? Number(evt.ts) : Date.now(),
    mode,
    kind: KINDS.indexOf(evt.kind) >= 0 ? evt.kind : 'market-fill',
    wallet: mode === 'live' ? (evt.wallet as string) : null,
    pair: {
      baseMint: String(p.baseMint || '').slice(0, 64),
      quoteMint: String(p.quoteMint || '').slice(0, 64),
      baseSymbol: String(p.baseSymbol || 'TOKEN').slice(0, 20),
      quoteSymbol: String(p.quoteSymbol || 'USDC').slice(0, 10),
    },
    side: evt.side === 'sell' ? 'sell' : 'buy',
    sizeBase: Number(evt.sizeBase) || 0,
    execPrice: Number(evt.execPrice) || 0,
    notionalQuote: Number(evt.notionalQuote) || 0,
    fees: {
      networkLamports: Number.isFinite(nl) && nl >= 0 ? nl : 0,
      protocolQuote: Number.isFinite(pq) && pq >= 0 ? pq : 0,
      label: String(f.label || '—').slice(0, 200),
    },
    status: STATUSES.indexOf(evt.status) >= 0 ? evt.status : 'filled',
    txSig: typeof evt.txSig === 'string' && evt.txSig ? evt.txSig : null,
  };
  if (
    evt.quoted &&
    Number.isFinite(Number(evt.quoted.minOut)) &&
    Number.isFinite(Number(evt.quoted.slippageBps))
  ) {
    e.quoted = {
      minOut: Number(evt.quoted.minOut),
      slippageBps: Number(evt.quoted.slippageBps),
      minOutUnit: String(evt.quoted.minOutUnit || e.pair.quoteSymbol).slice(0, 10),
    };
  }
  if (typeof evt.error === 'string' && evt.error) e.error = evt.error.slice(0, 500);
  if (typeof evt.note === 'string' && evt.note) e.note = evt.note.slice(0, 500);
  const s = loadTradeHistory();
  if (mode === 'paper') {
    s.paper.unshift(e);
    s.paper.length = Math.min(s.paper.length, TRADE_HISTORY_CAP);
  } else {
    const arr = s.live[e.wallet as string] || (s.live[e.wallet as string] = []);
    arr.unshift(e);
    arr.length = Math.min(arr.length, TRADE_HISTORY_CAP);
  }
  const store = storage();
  if (!store) return null;
  try {
    store.setItem(TRADE_HISTORY_KEY, JSON.stringify(s));
  } catch {
    return null;
  }
  return e;
}

/**
 * Cluster-aware explorer links (pack §3.3). Only real signatures get links —
 * paper rows never render one (the home History panel enforces this).
 */
export function tradeExplorerLinks(
  signature: string,
  cluster: 'mainnet' | 'devnet' = 'mainnet',
): { solscan: string; explorer: string } {
  const sig = String(signature || '');
  const dev = cluster === 'devnet';
  return {
    solscan: 'https://solscan.io/tx/' + sig + (dev ? '?cluster=devnet' : ''),
    explorer:
      'https://explorer.solana.com/tx/' + sig + '?cluster=' + (dev ? 'devnet' : 'mainnet-beta'),
  };
}

const STABLE_MINTS = new Set(
  [TOKENS.USDC?.mint, TOKENS.USDT?.mint].filter((m): m is string => typeof m === 'string'),
);

export interface SwapFillInput {
  wallet: string;
  inputMint: string;
  inputSymbol: string;
  inputDecimals: number;
  /** Amount being sold, in input-token base units. */
  inputAmountBase: string;
  outputMint: string;
  outputSymbol: string;
  outputDecimals: number;
  /** Estimated amount received, in output-token base units. */
  outputAmountBase: string;
  /** Minimum received after slippage, in output-token base units. */
  minimumOutBase: string;
  slippageBps: number | null;
  /** getFeeForMessage lamports; null only if the prepare gate was bypassed (it blocks). */
  networkFeeLamports: number | null;
  signature: string;
  /** 'filled' only after on-chain confirmation at 'confirmed' commitment. */
  status: 'filled' | 'failed';
  error?: string;
}

function baseUnitsToUnits(base: string, decimals: number): number {
  const n = Number(base);
  if (!Number.isFinite(n) || n < 0) return 0;
  return n / 10 ** decimals;
}

/**
 * Build a swap-fill TradeEvent from a signed Raydium swap. Convention:
 * swapping INTO a stablecoin (USDC/USDT) is a 'sell' of the input asset;
 * swapping into anything else is a 'buy' of the output asset. execPrice is
 * quote-per-base under that convention, so price × sizeBase ≈ notionalQuote.
 */
export function buildSwapFillEvent(inp: SwapFillInput): NewTradeEvent {
  const inputUnits = baseUnitsToUnits(inp.inputAmountBase, inp.inputDecimals);
  const outputUnits = baseUnitsToUnits(inp.outputAmountBase, inp.outputDecimals);
  const minOutUnits = baseUnitsToUnits(inp.minimumOutBase, inp.outputDecimals);
  const sellForStable = STABLE_MINTS.has(inp.outputMint);
  const side: 'buy' | 'sell' = sellForStable ? 'sell' : 'buy';
  const sizeBase = side === 'sell' ? inputUnits : outputUnits;
  const execPrice =
    sizeBase > 0 ? (side === 'sell' ? outputUnits / inputUnits : inputUnits / outputUnits) : 0;
  const notionalQuote = side === 'sell' ? outputUnits : inputUnits;
  const baseSymbol = side === 'sell' ? inp.inputSymbol : inp.outputSymbol;
  const quoteSymbol = side === 'sell' ? inp.outputSymbol : inp.inputSymbol;
  const evt: NewTradeEvent = {
    mode: 'live',
    kind: 'swap-fill',
    wallet: inp.wallet,
    pair: {
      baseMint: side === 'sell' ? inp.inputMint : inp.outputMint,
      quoteMint: side === 'sell' ? inp.outputMint : inp.inputMint,
      baseSymbol,
      quoteSymbol,
    },
    side,
    sizeBase,
    execPrice,
    notionalQuote,
    fees: {
      networkLamports: inp.networkFeeLamports ?? 0,
      protocolQuote: 0,
      label: 'Pool fees included in quote (not separately reported)',
    },
    status: inp.status,
    txSig: inp.signature,
    note: 'Live swap — ' + (inp.status === 'filled' ? 'confirmed on-chain (confirmed commitment).' : 'see error.'),
  };
  if (Number.isFinite(minOutUnits) && inp.slippageBps !== null && Number.isFinite(inp.slippageBps)) {
    evt.quoted = { minOut: minOutUnits, slippageBps: inp.slippageBps, minOutUnit: inp.outputSymbol };
  }
  if (inp.error) evt.error = inp.error;
  return evt;
}
