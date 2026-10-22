// Reconstructed 2026-10-21 from [sources]; not byte-identical to the deployed original.
// Sources: src/raydium_common.rs.partial (genuine 1500-byte head of the deployed backend: token
//   symbols/names/mints/decimals, the Pair struct shape {id, base_symbol, base_mint, quote_symbol,
//   quote_mint}, and the fact that /api/raydium/pairs returns six pairs); the deployed
//   SwapPanel.tsx.partial head (imports {formatBaseUnits, PAIRS, parseAmountToBaseUnits, TOKENS} and uses
//   TOKENS[reverse?pair.quoteSymbol:pair.baseSymbol], input.decimals, PAIRS[0].id, pair.baseSymbol,
//   pair.quoteSymbol, input.mint, output.mint, output.symbol).
// UNEVIDENCED (flagged): pair id strings ('sol-usdc' etc.) — the Rust head is truncated before the PAIRS
//   array literal, and the starter template's 'SOL-USDC'-style ids were verified NOT to be the deployed
//   source; the exact Error message text of parseAmountToBaseUnits ('Invalid amount'); truncation (rather
//   than rejection) of fractions longer than the token decimals.

export interface TokenInfo {
  symbol: string;
  name: string;
  mint: string;
  decimals: number;
}

export interface PairPreset {
  id: string;
  baseSymbol: string;
  baseMint: string;
  quoteSymbol: string;
  quoteMint: string;
}

export const TOKENS: Record<string, TokenInfo> = {
  SOL: {
    symbol: 'SOL',
    name: 'Wrapped SOL',
    mint: 'So11111111111111111111111111111111111111112',
    decimals: 9,
  },
  USDC: {
    symbol: 'USDC',
    name: 'USD Coin',
    mint: 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v',
    decimals: 6,
  },
  USDT: {
    symbol: 'USDT',
    name: 'Tether USD',
    mint: 'Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB',
    decimals: 6,
  },
  RAY: {
    symbol: 'RAY',
    name: 'Raydium',
    mint: '4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R',
    decimals: 6,
  },
  JUP: {
    symbol: 'JUP',
    name: 'Jupiter',
    mint: 'JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN',
    decimals: 6,
  },
  BONK: {
    symbol: 'BONK',
    name: 'Bonk',
    mint: 'DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263',
    decimals: 5,
  },
};

export const PAIRS: PairPreset[] = [
  { id: 'sol-usdc', baseSymbol: 'SOL', baseMint: TOKENS.SOL.mint, quoteSymbol: 'USDC', quoteMint: TOKENS.USDC.mint },
  { id: 'sol-usdt', baseSymbol: 'SOL', baseMint: TOKENS.SOL.mint, quoteSymbol: 'USDT', quoteMint: TOKENS.USDT.mint },
  { id: 'usdc-usdt', baseSymbol: 'USDC', baseMint: TOKENS.USDC.mint, quoteSymbol: 'USDT', quoteMint: TOKENS.USDT.mint },
  { id: 'ray-usdc', baseSymbol: 'RAY', baseMint: TOKENS.RAY.mint, quoteSymbol: 'USDC', quoteMint: TOKENS.USDC.mint },
  { id: 'jup-usdc', baseSymbol: 'JUP', baseMint: TOKENS.JUP.mint, quoteSymbol: 'USDC', quoteMint: TOKENS.USDC.mint },
  { id: 'bonk-usdc', baseSymbol: 'BONK', baseMint: TOKENS.BONK.mint, quoteSymbol: 'USDC', quoteMint: TOKENS.USDC.mint },
];

/** Parse a decimal string like '0.1' into a base-unit integer string without floating point. */
export function parseAmountToBaseUnits(amountStr: string, decimals: number): string {
  const text = amountStr.trim();
  const parts = text.split('.');
  const wholeOk = parts.length >= 1 && parts.length <= 2 && /^\d+$/.test(parts[0]);
  const fracOk = parts.length === 1 || /^\d*$/.test(parts[1]);
  if (!text || !wholeOk || !fracOk) throw new Error('Invalid amount');
  const whole = parts[0].replace(/^0+/, '') || '0';
  const frac = (parts[1] ?? '').slice(0, decimals).padEnd(decimals, '0');
  const digits = (whole + frac).replace(/^0+/, '');
  if (!digits) throw new Error('Invalid amount');
  return digits;
}

/** Format a base-unit integer string back into a decimal string, trimming trailing zeros. */
export function formatBaseUnits(raw: string, decimals: number, symbol?: string): string {
  const digits = raw.replace(/^0+/, '') || '0';
  const padded = digits.padStart(decimals + 1, '0');
  const whole = padded.slice(0, padded.length - decimals) || '0';
  const frac = decimals > 0 ? padded.slice(padded.length - decimals).replace(/0+$/, '') : '';
  const text = frac ? `${whole}.${frac}` : whole;
  return symbol ? `${text} ${symbol}` : text;
}
