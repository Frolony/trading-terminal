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
import {COPY as SWAP_COPY,explorerUrl,isQuoteFresh,isRejectionError} from './solana/swapFlow';
import {useMemo,useState} from 'react';import {WalletMultiButton} from '@solana/wallet-adapter-react-ui';import {formatBaseUnits,PAIRS,parseAmountToBaseUnits,TOKENS} from './solana/tokens';import {useRaydiumSwap} from './solana/useRaydiumSwap';import type {QuoteResult} from './solana/raydiumApi';
export function SwapPanel(){const [pairId,setPairId]=useState(PAIRS[0].id),[reverse,setReverse]=useState(false),[amount,setAmount]=useState('0.1'),[slippageBps,setSlippageBps]=useState('50'),[quote,setQuote]=useState<QuoteResult|null>(null),[busy,setBusy]=useState(false),[message,setMessage]=useState('Connect your wallet to begin.'),[error,setError]=useState<string|null>(null);const {getQuote,executeQuote,connected}=useRaydiumSwap();const pair=useMemo(()=>PAIRS.find(p=>p.id===pairId)??PAIRS[0],[pairId]);const input=TOKENS[reverse?pair.quoteSymbol:pair.baseSymbol],output=TOKENS[reverse?pair.baseSymbol:pair.quoteSymbol];
function reset(){setQuote(null);setError(null);setMessage('Request and review a fresh quote before signing.')}
async function onQuote(){setBusy(true);setError(null);setMessage('Fetching Raydium route…');setQuote(null);try{const raw=parseAmountToBaseUnits(amount,input.decimals),slip=Number(slippageBps);if(!Number.isInteger(slip)||slip<1||slip>500)throw Error('Slippage must be 1–500 bps (0.01%–5%).');const q=await getQuote({inputMint:input.mint,outputMint:output.mint,amountRaw:raw,slippageBps:slip,outputSymbol:output.symbol});setQuote(q);setMessage('Quote ready. Check expected and minimum received before signing.');return q;}catch(err){setError(err instanceof Error?err.message:'Quote failed.');setMessage('Quote failed. Request a fresh quote and try again.');return null;}finally{setBusy(false);}}
// N4.2 (2026-10-22) — review-sign-confirm flow. "Review & swap" opens a review sheet showing the
// quote's OWN numbers (byte-match requirement); signing is reachable ONLY from the sheet's
// "Confirm swap" button. A quote at/past the 25 s freshness window is never signed: the panel
// re-quotes and asks for confirmation again. After signing, the signature renders in a pending
// state immediately, then the panel waits for 'confirmed' commitment before the explorer link +
// post-swap balances.
// NOTE: line 31's useRaydiumSwap() destructuring is genuine-head bytes and cannot gain the new
// helpers, so they are read from a second (context-only) hook call below.
const swapHelpers=useRaydiumSwap();
const [reviewQuote,setReviewQuote]=useState<QuoteResult|null>(null);
const [phase,setPhase]=useState<'idle'|'signing'|'pending'|'confirmed'|'failed'>('idle');
const [activeSig,setActiveSig]=useState<string|null>(null);
const [postBalances,setPostBalances]=useState<{sol:string|null;token:string|null}|null>(null);
function clearSwapState(){setReviewQuote(null);setPhase('idle');setActiveSig(null);setPostBalances(null);}
function onPairChange(id:string){setPairId(id);clearSwapState();reset();}
function onToggleReverse(){setReverse(r=>!r);clearSwapState();reset();}
function onAmountChange(v:string){setAmount(v);clearSwapState();reset();}
function onSlippageChange(v:string){setSlippageBps(v);clearSwapState();reset();}
/** "Review & swap": never opens the sheet on a stale quote — re-quotes first and asks the user to
 *  review the NEW numbers (pack §3.5: never sign a stale quote silently). */
async function onReview(){
  if(!quote){return;}
  if(!connected||!publicKey){setError('Connect a wallet before swapping.');return;}
  setError(null);
  if(!isQuoteFresh(quote,Date.now())){
    setMessage(SWAP_COPY.staleQuote);
    const fresh=await onQuote();
    if(fresh){setMessage('Fresh quote fetched — review the new numbers, then click “Review & swap” again.');}
    return;
  }
  setReviewQuote(quote);setPhase('idle');setActiveSig(null);setPostBalances(null);
  setMessage('Review the numbers below — nothing is signed until you confirm in your wallet.');
}
function onCancelReview(){
  setReviewQuote(null);setPhase('idle');setActiveSig(null);setPostBalances(null);setError(null);
  setMessage('Review cancelled. The quote is still live — review again or request a fresh one.');
}
/** The ONLY path to signing. Re-asserts freshness at confirm time (covers the race where the quote
 *  ages out while the sheet is open, or the auto-refresh replaced `quote` underneath). */
async function onConfirmSwap(){
  const rq=reviewQuote;
  if(!rq||!publicKey){return;}
  setError(null);
  const now=Date.now();
  // The sheet's numbers are the only numbers ever signed. If the sheet's quote has aged out,
  // re-quote and ask for confirmation again — never silently switch to a different quote.
  if(!isQuoteFresh(rq,now)){
    setReviewQuote(null);setPhase('idle');setActiveSig(null);
    setMessage(SWAP_COPY.staleQuote);
    const fresh=await onQuote();
    if(fresh){setMessage('Fresh quote fetched — review the new numbers, then click “Review & swap” again.');}
    return;
  }
  const toSign:QuoteResult=rq;
  setPhase('signing');setBusy(true);setMessage(SWAP_COPY.waitingApproval);
  let sig:string;
  try{
    sig=await executeQuote(toSign,publicKey.toBase58());
  }catch(err){
    setBusy(false);
    if(isRejectionError(err)){
      // Back to the review sheet with a clear rejected state — nothing was sent.
      setPhase('idle');setError(SWAP_COPY.rejected);
      setMessage('Back at the review sheet — no transaction was sent.');
      return;
    }
    setPhase('failed');setActiveSig(null);
    setError('Swap failed before signing: '+(err instanceof Error?err.message:'unknown error'));
    setMessage('Swap failed before signing. Request a fresh quote and try again.');
    return;
  }
  // The signature exists: show it in a pending state immediately, THEN wait for confirmation.
  setActiveSig(sig);setPhase('pending');setMessage(SWAP_COPY.pendingConfirm);
  try{
    const onChainErr=await swapHelpers.confirmSwap(sig);
    if(onChainErr===null){
      setPhase('confirmed');setQuote(null);
      setMessage(SWAP_COPY.confirmed+' — details and post-swap balances below.');
      try{setPostBalances(await swapHelpers.fetchPostSwapBalances(toSign.outputMint));}
      catch{setPostBalances({sol:null,token:null});}
    }else{
      setPhase('failed');
      setError('Swap failed on-chain: '+onChainErr);
      setMessage('The transaction confirmed with an on-chain error — details on the explorer link.');
    }
  }catch(err){
    // Confirmation timed out; the transaction may still confirm. Keep the signature explorable
    // and do NOT auto-retry (double-send risk per the pack's non-negotiables).
    setPhase('failed');
    setError(SWAP_COPY.confirmTimedOut);
    setMessage('Confirmation timed out — check the explorer link before retrying.');
  }finally{
    setBusy(false);
  }
}
const [quoteAge,setQuoteAge]=useState(0);
const {publicKey}=useWallet();
useEffect(()=>{setQuoteAge(0);},[quote]);
useEffect(()=>{if(!quote){return;}const t=setInterval(()=>{setQuoteAge(a=>a+1);},1000);return()=>{clearInterval(t);};},[quote]);
useEffect(()=>{if(quote&&quoteAge>=25&&!busy&&phase==='idle'&&!reviewQuote){void onQuote();}},[quote,quoteAge,busy,phase,reviewQuote]);
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
{reviewQuote&&phase!=='confirmed'?(
<div className="review-sheet" role="dialog" aria-label={SWAP_COPY.reviewTitle}>
<h3>{SWAP_COPY.reviewTitle}</h3>
<p className="review-note">{SWAP_COPY.reviewNote}</p>
<div className="quote-details">
<div className="quote-row"><span>You pay</span><strong>{formatBaseUnits(reviewQuote.inputAmount,input.decimals)} {input.symbol}</strong></div>
<div className="quote-row"><span>You receive (estimate)</span><strong>{formatBaseUnits(reviewQuote.outputAmount,reviewQuote.outputDecimals)} {reviewQuote.outputSymbol}</strong></div>
<div className="quote-row"><span>Minimum after slippage</span><strong>{formatBaseUnits(reviewQuote.minimumOut,reviewQuote.outputDecimals)} {reviewQuote.outputSymbol}</strong></div>
<div className="quote-row"><span>Price impact</span><strong>{reviewQuote.priceImpactPct===null?'—':reviewQuote.priceImpactPct+'%'}</strong></div>
<div className="quote-row"><span>Route legs</span><strong>{reviewQuote.routeLegs}</strong></div>
<div className="quote-row"><span>Slippage tolerance</span><strong>{slippageBps} bps</strong></div>
<div className="quote-row"><span>Confirmation commitment</span><strong>confirmed</strong></div>
</div>
{phase==='signing'?(<p className="message">{SWAP_COPY.waitingApproval}</p>):null}
{phase==='pending'&&activeSig?(
<div className="pending-box">
<p className="message">{SWAP_COPY.pendingConfirm}</p>
<p className="sig-line">Signature: <a href={explorerUrl(activeSig)} target="_blank" rel="noreferrer">{activeSig}</a></p>
</div>):null}
<div className="wallet-row">
<button type="button" className="btn btn-primary" onClick={()=>{void onConfirmSwap();}} disabled={busy||phase==='signing'||phase==='pending'}>{phase==='signing'?'Waiting for wallet…':phase==='pending'?'Confirming on-chain…':'Confirm swap'}</button>
<button type="button" className="btn" onClick={onCancelReview} disabled={phase==='signing'||phase==='pending'}>Back</button>
</div>
</div>):null}
{phase==='confirmed'&&activeSig?(
<div className="confirmed-box" role="status">
<p className="success">✅ {SWAP_COPY.confirmed}</p>
<p className="sig-line">View on explorer: <a href={explorerUrl(activeSig)} target="_blank" rel="noreferrer">{activeSig}</a></p>
{postBalances?(
<div className="quote-details">
<div className="quote-row"><span>SOL balance</span><strong>{postBalances.sol===null?'unavailable':postBalances.sol+' SOL'}</strong></div>
<div className="quote-row"><span>{reviewQuote?reviewQuote.outputSymbol:'Output token'} balance</span><strong>{postBalances.token===null?'unavailable':postBalances.token}</strong></div>
</div>):null}
<div className="wallet-row"><button type="button" className="btn btn-primary" onClick={()=>{clearSwapState();void onQuote();}} disabled={busy}>New quote</button></div>
</div>):null}
{phase==='failed'?(
<div className="failed-box" role="alert">
{activeSig?(<p className="sig-line">Transaction: <a href={explorerUrl(activeSig)} target="_blank" rel="noreferrer">{activeSig}</a></p>):null}
<div className="wallet-row">
<button type="button" className="btn btn-primary" onClick={()=>{clearSwapState();void onQuote();}} disabled={busy}>Request fresh quote</button>
{reviewQuote?(<button type="button" className="btn" onClick={()=>{setPhase('idle');setError(null);setMessage('Back at the review sheet — no signature was created in this attempt.');}}>Back to review</button>):null}
</div>
</div>):null}
{message?(<p className="message">{message}</p>):null}
{error?(<p className="error">{error}</p>):null}
<div className="wallet-row"><button type="button" className="btn btn-primary" onClick={()=>{clearSwapState();void onQuote();}} disabled={busy}>{busy?'Working…':'Get quote'}</button><button type="button" className="btn" onClick={()=>{void onReview();}} disabled={!connected||!quote||busy||phase==='signing'||phase==='pending'}>Review &amp; swap</button></div>
<p className="noncustodial">The backend only requests quotes and builds unsigned transactions. Your wallet signs locally; the backend never receives a seed phrase or private key. Always verify amounts and wallet prompts.</p>
<a className="back-link" href="/">← Back to Gecko Terminal</a>
</section></div>);}
