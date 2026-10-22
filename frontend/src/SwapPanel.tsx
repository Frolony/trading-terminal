// Reconstructed 2026-10-21 from [sources]; not byte-identical to the deployed original.
// PROVENANCE: the code between this header and the tail marker is the genuine 1500-byte head of the
//   deployed frontend/src/SwapPanel.tsx — byte-faithful, never reformatted. Everything after the tail
//   marker is reconstruction.
// NOTE: the two import lines below are reconstruction additions placed ABOVE the genuine head (they are
//   hoisted ESM imports and do not alter a single head byte). They exist because the head's own
//   `import {useMemo,useState} from 'react'` line cannot be edited, and the reconstructed tail needs
//   useEffect (quote-age timer + 25 s auto-refresh) and useWallet (wallet pubkey for executeQuote).
// Sources: the head itself (state names; validation copy 'Slippage must be 1–500 bps (0.01%–5%).';
//   initial 'Connect your wallet to begin.'; the truncated 'Quote ready. Ch…' string); live-page copy
//   recorded in the master plan §2.5 from the production /raydium/ page — toast 'Quote ready. Check
//   expected and minimum received before signing.' (this completes the truncated string EXACTLY),
//   eyebrow 'NON-CUSTODIAL SOLANA SWAPS', panel eyebrow 'TRADE ROUTE', 'Swap tokens', 'Solana Mainnet',
//   'You pay', 'You receive (estimate)', 'Minimum after slippage: — USDC', 'Slippage tolerance (basis
//   points)', '50 bps = 0.50%. Maximum: 500 bps = 5.', 'Connect Phantom, Backpack, or another Wallet
//   Standard-compatible wallet to sign.', the non-custodial paragraph 'The backend only requests quotes
//   and builds unsigned transactions. Your wallet signs locally; the backend never receives a seed phrase
//   or private key. Always verify amounts and wallet prompts.', back link '← Back to Gecko Terminal' →
//   '/', quote facts ('est.', 'min', N route leg(s), impact 0, '25 s refresh'), and 'Review & swap'
//   disabled until a wallet is connected; frontend/index.html (document title
//   'Raydium Swap | Gecko Terminal'); styles.css.partial (.shell/.topbar/.panel/.eyebrow/.field-label/
//   .amount-heading/.amount-row/.network class names).
// UNEVIDENCED (flagged): onQuote catch/finally copy; the onExecute flow and its copy (success message with
//   signature; failure copy); wiring the head's reset() into the pair/amount/slippage/reverse handlers;
//   'Change wallet' — the string is rendered by the wallet-adapter modal itself when a wallet is connected,
//   not by this file; exact JSX structure and the '⇄' reverse glyph; the quote-age countdown plus
//   auto-refresh at 25 s (behavior evidenced, exact presentation invented); every CSS class name used here
//   beyond the evidenced set.
import {useEffect} from 'react';
import {useWallet} from '@solana/wallet-adapter-react';
import {useMemo,useState} from 'react';import {WalletMultiButton} from '@solana/wallet-adapter-react-ui';import {formatBaseUnits,PAIRS,parseAmountToBaseUnits,TOKENS} from './solana/tokens';import {useRaydiumSwap} from './solana/useRaydiumSwap';import type {QuoteResult} from './solana/raydiumApi';
export function SwapPanel(){const [pairId,setPairId]=useState(PAIRS[0].id),[reverse,setReverse]=useState(false),[amount,setAmount]=useState('0.1'),[slippageBps,setSlippageBps]=useState('50'),[quote,setQuote]=useState<QuoteResult|null>(null),[busy,setBusy]=useState(false),[message,setMessage]=useState('Connect your wallet to begin.'),[error,setError]=useState<string|null>(null);const {getQuote,executeQuote,connected}=useRaydiumSwap();const pair=useMemo(()=>PAIRS.find(p=>p.id===pairId)??PAIRS[0],[pairId]);const input=TOKENS[reverse?pair.quoteSymbol:pair.baseSymbol],output=TOKENS[reverse?pair.baseSymbol:pair.quoteSymbol];
function reset(){setQuote(null);setError(null);setMessage('Request and review a fresh quote before signing.')}
async function onQuote(){setBusy(true);setError(null);setMessage('Fetching Raydium route…');setQuote(null);try{const raw=parseAmountToBaseUnits(amount,input.decimals),slip=Number(slippageBps);if(!Number.isInteger(slip)||slip<1||slip>500)throw Error('Slippage must be 1–500 bps (0.01%–5%).');const q=await getQuote({inputMint:input.mint,outputMint:output.mint,amountRaw:raw,slippageBps:slip,outputSymbol:output.symbol});setQuote(q);setMessage('Quote ready. Check expected and minimum received before signing.');}catch(err){setError(err instanceof Error?err.message:'Quote failed.');setMessage('Quote failed. Request a fresh quote and try again.');}finally{setBusy(false);}}
async function onExecute(){if(!quote){return;}if(!connected||!publicKey){setError('Connect a wallet before swapping.');return;}setBusy(true);setError(null);setMessage('Building the swap transaction…');try{const sig=await executeQuote(quote,publicKey.toBase58());setQuote(null);setMessage('Swap submitted. Signature: '+sig);}catch(err){setError(err instanceof Error?err.message:'Swap failed.');setMessage('Swap failed before signing. Request a fresh quote and try again.');}finally{setBusy(false);}}
function onPairChange(id:string){setPairId(id);reset();}
function onToggleReverse(){setReverse(r=>!r);reset();}
function onAmountChange(v:string){setAmount(v);reset();}
function onSlippageChange(v:string){setSlippageBps(v);reset();}
const [quoteAge,setQuoteAge]=useState(0);
const {publicKey}=useWallet();
useEffect(()=>{setQuoteAge(0);},[quote]);
useEffect(()=>{if(!quote){return;}const t=setInterval(()=>{setQuoteAge(a=>a+1);},1000);return()=>{clearInterval(t);};},[quote]);
useEffect(()=>{if(quote&&quoteAge>=25&&!busy){void onQuote();}},[quote,quoteAge,busy]);
return(<div className="shell">
<header className="topbar"><div><p className="eyebrow">NON-CUSTODIAL SOLANA SWAPS</p><h1>Raydium Swap</h1><p className="tagline">Gecko Terminal · Raydium Trade API</p></div><div className="wallet-col"><WalletMultiButton/><p className="wallet-hint">Connect Phantom, Backpack, or another Wallet Standard-compatible wallet to sign.</p></div></header>
<section className="panel"><div className="panel-heading"><div><p className="eyebrow">TRADE ROUTE</p><h2>Swap tokens</h2></div><span className="network">Solana Mainnet</span></div>
<label className="field-label" htmlFor="pair">Pair preset</label>
<select id="pair" value={pairId} onChange={e=>onPairChange(e.target.value)} disabled={busy}>{PAIRS.map(p=><option key={p.id} value={p.id}>{p.baseSymbol} / {p.quoteSymbol}</option>)}</select>
<div className="amount-heading"><label className="field-label" htmlFor="amount">You pay</label><span className="token-symbol">{input.symbol}</span></div>
<div className="amount-row"><input id="amount" value={amount} onChange={e=>onAmountChange(e.target.value)} inputMode="decimal" placeholder="0.0" disabled={busy}/><button type="button" className="reverse-btn" onClick={onToggleReverse} disabled={busy} title="Reverse direction" aria-label="Reverse swap direction">⇄</button></div>
<div className="amount-heading"><span className="field-label">You receive (estimate)</span><span className="token-symbol">{output.symbol}</span></div>
<div className="receive-box">{quote?formatBaseUnits(quote.outputAmount,quote.outputDecimals):'—'}</div>
<p className="slippage-hint">Minimum after slippage: {quote?formatBaseUnits(quote.minimumOut,quote.outputDecimals):'—'} {output.symbol}</p>
<label className="field-label" htmlFor="slippage">Slippage tolerance (basis points)</label>
<input id="slippage" value={slippageBps} onChange={e=>onSlippageChange(e.target.value)} inputMode="numeric" disabled={busy}/>
<p className="slippage-hint">50 bps = 0.50%. Maximum: 500 bps = 5.</p>
{quote?(<div className="quote-details"><div className="quote-row"><span>Estimated output</span><strong>{formatBaseUnits(quote.outputAmount,quote.outputDecimals)} {quote.outputSymbol}</strong></div><div className="quote-row"><span>Minimum after slippage</span><strong>{formatBaseUnits(quote.minimumOut,quote.outputDecimals)} {quote.outputSymbol}</strong></div><div className="quote-row"><span>Route legs</span><strong>{quote.routeLegs}</strong></div><div className="quote-row"><span>Price impact</span><strong>{quote.priceImpactPct===null?'—':quote.priceImpactPct+'%'}</strong></div><div className="quote-row"><span>Quote age</span><strong>{quoteAge}s · auto-refreshes every 25s</strong></div></div>):null}
{message?(<p className="message">{message}</p>):null}
{error?(<p className="error">{error}</p>):null}
<div className="wallet-row"><button type="button" className="btn btn-primary" onClick={()=>{void onQuote();}} disabled={busy}>{busy?'Working…':'Get quote'}</button><button type="button" className="btn" onClick={()=>{void onExecute();}} disabled={!connected||!quote||busy}>Review &amp; swap</button></div>
<p className="noncustodial">The backend only requests quotes and builds unsigned transactions. Your wallet signs locally; the backend never receives a seed phrase or private key. Always verify amounts and wallet prompts.</p>
<a className="back-link" href="/">← Back to Gecko Terminal</a>
</section></div>);}
