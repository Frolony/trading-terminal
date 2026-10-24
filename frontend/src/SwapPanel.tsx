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
//   or private key. Always verify amounts and wallet prompts.', back link '← Back to Turnpool' →
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
import type {VersionedTransaction} from '@solana/web3.js';
import {COPY as SWAP_COPY,SAFETY_COPY,DEFAULT_SLIPPAGE_BPS,explorerUrl,impactBand,impactNotice,isQuoteFresh,isRejectionError,lamportsToSol,MAX_SLIPPAGE_BPS,MIN_SLIPPAGE_BPS,parseSlippageInput,poolFeeNote,PREWALLET_FEES_COPY,readSharedSlippage,SLIPPAGE_PRESETS_BPS,solPriceUsdFromQuote,writeSharedSlippage} from './solana/swapFlow';
import type {PrepareState,ImpactBand} from './solana/swapFlow';
// N4.6 (2026-10-22): live swap-fill events land in the shared trade-history store
// (same schema as the home page's History panel) — written ONLY after on-chain
// confirmation, or with status 'failed' when confirmation itself fails. A
// signed-but-unconfirmed signature never lands as 'filled'.
import {buildSwapFillEvent,recordTradeEvent} from './solana/tradeHistory';
// N4.4 AMENDMENT (2026-10-22): the head's slippage validation below was widened from
//   1–500 bps to 1–1000 bps (0.01%–10%) with the pack's rejection copy, via parseSlippageInput.
//   The slippage UI gained presets (0.1/0.5/1.0%) + custom input. The byte-faithful claim above
//   covers the reconstruction era; post-seed evolution is documented here per amendment.
import {useMemo,useState} from 'react';import {WalletMultiButton} from '@solana/wallet-adapter-react-ui';import {formatBaseUnits,PAIRS,parseAmountToBaseUnits,TOKENS} from './solana/tokens';import {useRaydiumSwap} from './solana/useRaydiumSwap';import type {QuoteResult} from './solana/raydiumApi';
// N4.4 AMENDMENT (2026-10-22): `executeQuote` (build+sign in one step) is no longer used —
//   onConfirmSwap signs the PREPARED bytes via swapHelpers.signAndSendPrepared, so the
//   destructure drops it (tsc TS6133). Keeping the one-step signer wired would reintroduce
//   a sign-different-bytes path, i.e. a simulate-then-sign hole. executeQuote remains
//   exported by useRaydiumSwap for API completeness but is unwired here.
export function SwapPanel(){const [pairId,setPairId]=useState(PAIRS[0].id),[reverse,setReverse]=useState(false),[amount,setAmount]=useState('0.1'),[slippageBps,setSlippageBps]=useState(()=>String(readSharedSlippage())),[quote,setQuote]=useState<QuoteResult|null>(null),[busy,setBusy]=useState(false),[message,setMessage]=useState('Connect your wallet to begin.'),[error,setError]=useState<string|null>(null);const {getQuote,connected}=useRaydiumSwap();const pair=useMemo(()=>PAIRS.find(p=>p.id===pairId)??PAIRS[0],[pairId]);const input=TOKENS[reverse?pair.quoteSymbol:pair.baseSymbol],output=TOKENS[reverse?pair.baseSymbol:pair.quoteSymbol];
function reset(){setQuote(null);setError(null);setMessage('Request and review a fresh quote before signing.')}
async function onQuote(){setBusy(true);setError(null);setMessage('Fetching Raydium route…');setQuote(null);try{const slipParsed=parseSlippageInput(slippageBps);if(!slipParsed.ok)throw Error(slipParsed.error);const raw=parseAmountToBaseUnits(amount,input.decimals),slip=slipParsed.bps;const q=await getQuote({inputMint:input.mint,outputMint:output.mint,amountRaw:raw,slippageBps:slip,outputSymbol:output.symbol});setQuote(q);setMessage('Quote ready. Check expected and minimum received before signing.');return q;}catch(err){setError(err instanceof Error?err.message:'Quote failed.');setMessage('Quote failed. Request a fresh quote and try again.');return null;}finally{setBusy(false);}}
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
function clearSwapState(){setReviewQuote(null);setPhase('idle');setActiveSig(null);setPostBalances(null);clearPrepare();}
function onPairChange(id:string){setPairId(id);clearSwapState();reset();}
function onToggleReverse(){setReverse(r=>!r);clearSwapState();reset();}
function onAmountChange(v:string){setAmount(v);clearSwapState();reset();}
function onSlippageChange(v:string){
  const parsed=parseSlippageInput(v);
  if(!parsed.ok){setError(parsed.error);return;}
  setError(null);setSlippageBps(String(parsed.bps));writeSharedSlippage(parsed.bps);clearSwapState();reset();
  setMessage('Slippage set to '+(parsed.bps/100).toFixed(2)+'% — request a fresh quote before confirming. (Shared with the home ticket.)');
}
/** N4.4: slippage hint for the custom input (pack §3.1). */
const slippageHint=`Presets or custom, in basis points (${DEFAULT_SLIPPAGE_BPS} bps = ${(DEFAULT_SLIPPAGE_BPS/100).toFixed(2)}%). Allowed: ${MIN_SLIPPAGE_BPS}–${MAX_SLIPPAGE_BPS} bps (0.01%–10%).`;
// N4.4 (2026-10-22) — pre-trade safety prepare: build the unsigned transaction at sheet-open,
// simulate it, and estimate the network fee. The wallet later signs THESE bytes
// (simulate-then-sign the same bytes). Confirm arms ONLY when prepareState==='ready'.
const [prepareState,setPrepareState]=useState<PrepareState>('idle');
const [preparedTx,setPreparedTx]=useState<VersionedTransaction|null>(null);
const [simStatus,setSimStatus]=useState<'ok'|'failed'|'unavailable'|null>(null);
const [simMessage,setSimMessage]=useState<string|null>(null);
const [feeLamports,setFeeLamports]=useState<number|null>(null);
const [impactAck,setImpactAck]=useState(false);
function clearPrepare(){setPrepareState('idle');setPreparedTx(null);setSimStatus(null);setSimMessage(null);setFeeLamports(null);setImpactAck(false);}
async function prepareSwap(q:QuoteResult,walletPubkey:string){
  clearPrepare();setPrepareState('preparing');
  let tx:VersionedTransaction;
  try{
    tx=await swapHelpers.buildPreparedTx(q,walletPubkey);
  }catch(err){
    setPrepareState('blocked');
    setSimStatus('unavailable');
    setSimMessage('Could not build the transaction: '+(err instanceof Error?err.message:'unknown error'));
    return;
  }
  setPreparedTx(tx);
  let simOk=false;
  try{
    const sim=await swapHelpers.simulatePreparedTx(tx);
    if(sim.ok){setSimStatus('ok');setSimMessage(null);simOk=true;}
    else{setSimStatus('failed');setSimMessage(SAFETY_COPY.simFailedPrefix+(sim.message||'no reason returned')+SAFETY_COPY.simFailedSuffix);}
  }catch(err){
    setSimStatus('unavailable');
    setSimMessage(SAFETY_COPY.simUnavailable+' ('+(err instanceof Error?err.message:'unknown error')+')');
  }
  const fee=await swapHelpers.estimateFeeLamports(tx);
  setFeeLamports(fee);
  setPrepareState(simOk&&fee!==null?'ready':'blocked');
}
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
  void prepareSwap(quote,publicKey.toBase58());
}
function onCancelReview(){
  setReviewQuote(null);setPhase('idle');setActiveSig(null);setPostBalances(null);setError(null);
  setMessage('Review cancelled. The quote is still live — review again or request a fresh one.');
}
/** The ONLY path to signing. Re-asserts freshness at confirm time (covers the race where the quote
 *  ages out while the sheet is open, or the auto-refresh replaced `quote` underneath).
 *  N4.4: signs the PREPARED transaction (the bytes that were simulated + fee-estimated);
 *  the confirm is armed only when prepareState==='ready'. A Live confirm without a passed
 *  simulation is a P0 defect — the button is disabled, and this function double-guards. */
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
  const band=impactBand(rq.priceImpactPct);
  if(band==='blocked'){setError(impactNotice(band,rq.priceImpactPct));return;}
  if(band==='high'&&!impactAck){setError('Acknowledge the very-high price impact before confirming.');return;}
  if(prepareState!=='ready'||!preparedTx){
    setError('The transaction check did not complete — confirm is disabled. '+(prepareState==='blocked'?'See the reason above.':'Wait for the check to finish, then try again.'));
    return;
  }
  const toSign:QuoteResult=rq;
  setPhase('signing');setBusy(true);setMessage(SWAP_COPY.waitingApproval);
  let sig:string;
  try{
    sig=await swapHelpers.signAndSendPrepared(preparedTx);
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
  // N4.6: the history event for this swap. Written only after the on-chain
  // verdict below (filled on clean confirmation, failed otherwise) — never
  // for the pending state alone.
  const slipNum=Number(slippageBps);
  const histInput={wallet:publicKey.toBase58(),inputMint:toSign.inputMint,inputSymbol:input.symbol,inputDecimals:input.decimals,inputAmountBase:toSign.inputAmount,outputMint:toSign.outputMint,outputSymbol:output.symbol,outputDecimals:toSign.outputDecimals,outputAmountBase:toSign.outputAmount,minimumOutBase:toSign.minimumOut,slippageBps:Number.isFinite(slipNum)?slipNum:null,networkFeeLamports:feeLamports,signature:sig};
  try{
    const onChainErr=await swapHelpers.confirmSwap(sig);
    if(onChainErr===null){
      recordTradeEvent(buildSwapFillEvent(Object.assign({},histInput,{status:'filled' as const})));
      setPhase('confirmed');setQuote(null);
      setMessage(SWAP_COPY.confirmed+' — details and post-swap balances below.');
      try{setPostBalances(await swapHelpers.fetchPostSwapBalances(toSign.outputMint));}
      catch{setPostBalances({sol:null,token:null});}
    }else{
      recordTradeEvent(buildSwapFillEvent(Object.assign({},histInput,{status:'failed' as const,error:'Swap failed on-chain: '+onChainErr})));
      setPhase('failed');
      setError('Swap failed on-chain: '+onChainErr);
      setMessage('The transaction confirmed with an on-chain error — details on the explorer link.');
    }
  }catch(err){
    // Confirmation timed out; the transaction may still confirm. Keep the signature explorable
    // and do NOT auto-retry (double-send risk per the pack's non-negotiables). The history row
    // records the unknown outcome honestly as failed-with-timeout so the user has a trail.
    recordTradeEvent(buildSwapFillEvent(Object.assign({},histInput,{status:'failed' as const,error:SWAP_COPY.confirmTimedOut})));
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
// N4.4: derived review-sheet model (pack §3.6) — plain function calls, recomputed every render.
const rvBand:ImpactBand=reviewQuote?impactBand(reviewQuote.priceImpactPct):'unknown';
const rvNotice=reviewQuote?impactNotice(rvBand,reviewQuote.priceImpactPct):null;
const rvPoolFee=reviewQuote?poolFeeNote(reviewQuote.raw):null;
const rvSolPrice=reviewQuote?solPriceUsdFromQuote(reviewQuote):null;
const rvFeeSol=feeLamports!==null?lamportsToSol(feeLamports):null;
const rvFeeUsd=rvFeeSol!==null&&rvSolPrice!==null?Number(rvFeeSol)*rvSolPrice:null;
const rvImpactColor=rvBand==='low'?'#4ade80':rvBand==='elevated'?'#f5b544':rvBand==='high'||rvBand==='blocked'?'#ff7b7b':'inherit';
const rvBlockers:string[]=[];
if(reviewQuote){
  if(rvBand==='blocked'&&rvNotice)rvBlockers.push(rvNotice);
  if(prepareState==='preparing')rvBlockers.push(SAFETY_COPY.preparing);
  if(prepareState==='blocked'){
    if(simStatus==='failed')rvBlockers.push(simMessage??'Simulation failed — see above.');
    else if(simStatus==='unavailable')rvBlockers.push(simMessage??SAFETY_COPY.simUnavailable);
    else if(!preparedTx)rvBlockers.push('Transaction build failed — request a fresh quote.');
    if(simStatus==='ok'&&feeLamports===null)rvBlockers.push(SAFETY_COPY.feeUnavailable);
  }
  if(rvBand==='high'&&!impactAck)rvBlockers.push('Acknowledge the very-high price impact below to continue.');
}
const rvConfirmArmed=reviewQuote!==null&&prepareState==='ready'&&rvBlockers.length===0;
return(<div className="shell">
<header className="topbar"><div><p className="eyebrow">NON-CUSTODIAL SOLANA SWAPS</p><h1>Raydium Swap</h1><p className="tagline">Turnpool · Raydium Trade API</p></div><div className="wallet-col"><WalletMultiButton/><p className="wallet-hint">Connect Phantom, Backpack, or another Wallet Standard-compatible wallet to sign.</p></div></header>
<section className="panel"><div className="panel-heading"><div><p className="eyebrow">TRADE ROUTE</p><h2>Swap tokens</h2></div><span className="network">Solana Mainnet</span></div>
<label className="field-label" htmlFor="pair">Pair preset</label>
<select id="pair" value={pairId} onChange={e=>onPairChange(e.target.value)} disabled={busy}>{PAIRS.map(p=><option key={p.id} value={p.id}>{p.baseSymbol} / {p.quoteSymbol}</option>)}</select>
<div className="amount-heading"><label className="field-label" htmlFor="amount">You pay</label><span className="token-symbol">{input.symbol}</span></div>
<div className="amount-row"><input id="amount" value={amount} onChange={e=>onAmountChange(e.target.value)} inputMode="decimal" placeholder="0.0" disabled={busy}/><button type="button" className="reverse-btn" onClick={onToggleReverse} disabled={busy} title="Reverse direction" aria-label="Reverse swap direction">⇄</button></div>
<div className="amount-heading"><span className="field-label">You receive (estimate)</span><span className="token-symbol">{output.symbol}</span></div>
<div className="receive-box">{quote?formatBaseUnits(quote.outputAmount,quote.outputDecimals):'—'}</div>
<p className="slippage-hint">Minimum after slippage: {quote?formatBaseUnits(quote.minimumOut,quote.outputDecimals):'—'} {output.symbol}</p>
<label className="field-label" htmlFor="slippage">Slippage tolerance</label>
<div className="slippage-row" style={{display:'flex',gap:'8px',alignItems:'center',flexWrap:'wrap'}}>
{SLIPPAGE_PRESETS_BPS.map(bps=>{const active=slippageBps===String(bps);return(<button key={bps} type="button" onClick={()=>onSlippageChange(String(bps))} disabled={busy} aria-pressed={active} style={{padding:'6px 12px',borderRadius:'999px',border:active?'1px solid #b8f36b':'1px solid #2a3342',background:active?'#13251f':'#11151d',color:active?'#b8f36b':'#aab5c6',fontSize:'12px',fontWeight:700,cursor:busy?'default':'pointer'}}>{(bps/100).toFixed(1)}%</button>);})}
<input id="slippage" value={slippageBps} onChange={e=>onSlippageChange(e.target.value)} inputMode="numeric" disabled={busy} aria-label="Custom slippage in basis points" style={{width:'90px'}}/>
</div>
<p className="slippage-hint">{slippageHint}</p>
{quote?(<div className="quote-details"><div className="quote-row"><span>Estimated output</span><strong>{formatBaseUnits(quote.outputAmount,quote.outputDecimals)} {quote.outputSymbol}</strong></div><div className="quote-row"><span>Minimum after slippage</span><strong>{formatBaseUnits(quote.minimumOut,quote.outputDecimals)} {quote.outputSymbol}</strong></div><div className="quote-row"><span>Route legs</span><strong>{quote.routeLegs}</strong></div><div className="quote-row"><span>Price impact</span><strong>{quote.priceImpactPct===null?'—':quote.priceImpactPct+'%'}</strong></div><div className="quote-row"><span>Quote age</span><strong>{quoteAge}s · auto-refreshes every 25s</strong></div></div>):null}
{/* N4.8 (2026-10-23) — pre-wallet fee transparency. The network fee cannot be computed before a
    transaction exists, so the panel states honestly WHEN the estimate appears (at review, before
    signing) instead of inventing a number. The pool-fee note reuses the N4.4 tolerant reader
    (poolFeeNote) — the same function the wallet-gated review sheet uses on its own quote — so the
    two can never contradict each other. */}
{quote?(<div><p className="eyebrow">{PREWALLET_FEES_COPY.sectionHeading}</p><div className="quote-details"><div className="quote-row"><span>Network fee</span><strong>{PREWALLET_FEES_COPY.networkFee}</strong></div><div className="quote-row"><span>Pool fees</span><strong>{poolFeeNote(quote.raw)??SAFETY_COPY.poolFeesHonest}</strong></div></div></div>):null}
{reviewQuote&&phase!=='confirmed'?(
<div className="review-sheet" role="dialog" aria-label={SWAP_COPY.reviewTitle}>
<div style={{display:'flex',alignItems:'center',gap:'10px',flexWrap:'wrap'}}><h3 style={{margin:0}}>{SWAP_COPY.reviewTitle}</h3><span style={{fontSize:'11px',fontWeight:700,color:'#ff8f8f',border:'1px solid #7a2e2e',borderRadius:'999px',padding:'3px 10px',background:'#2a1414'}}>{SAFETY_COPY.liveChip}</span></div>
<p className="review-note">{SWAP_COPY.reviewNote}</p>
<div className="quote-details">
<div className="quote-row"><span>You pay</span><strong>{formatBaseUnits(reviewQuote.inputAmount,input.decimals)} {input.symbol}</strong></div>
<div className="quote-row"><span>You receive (estimate)</span><strong>{formatBaseUnits(reviewQuote.outputAmount,reviewQuote.outputDecimals)} {reviewQuote.outputSymbol}</strong></div>
<div className="quote-row"><span>Minimum received (after slippage)</span><strong>{formatBaseUnits(reviewQuote.minimumOut,reviewQuote.outputDecimals)} {reviewQuote.outputSymbol}</strong></div>
<div className="quote-row"><span>Price impact</span><strong style={{color:rvImpactColor}}>{reviewQuote.priceImpactPct===null?'—':reviewQuote.priceImpactPct.toFixed(2)+'%'}</strong></div>
{rvNotice?(<div className="quote-row"><span>Impact warning</span><strong style={{color:rvBand==='blocked'?'#ff7b7b':'#f5b544'}}>{rvNotice}</strong></div>):null}
<div className="quote-row"><span>Network fee</span><strong>{rvFeeSol!==null?rvFeeSol+' SOL'+(rvFeeUsd!==null?' (≈ $'+rvFeeUsd.toFixed(4)+')':' (USD estimate unavailable)'):prepareState==='preparing'?'Estimating…':'unavailable — confirm disabled'}</strong></div>
<div className="quote-row"><span>Pool fees</span><strong>{rvPoolFee??SAFETY_COPY.poolFeesHonest}</strong></div>
<div className="quote-row"><span>Total cost</span><strong>{rvFeeSol!==null?formatBaseUnits(reviewQuote.inputAmount,input.decimals)+' '+input.symbol+' + '+rvFeeSol+' SOL network fee':'—'}</strong></div>
<div className="quote-row"><span>Route</span><strong>Raydium · {reviewQuote.routeLegs} leg{reviewQuote.routeLegs===1?'':'s'}</strong></div>
<div className="quote-row"><span>Slippage tolerance</span><strong>{slippageBps} bps ({(Number(slippageBps)/100).toFixed(2)}%)</strong></div>
<div className="quote-row"><span>Quote age</span><strong>{quoteAge}s · auto-refreshes every 25s</strong></div>
<div className="quote-row"><span>Pre-flight simulation</span><strong style={{color:simStatus==='ok'?'#4ade80':simStatus?'#ff7b7b':'inherit'}}>{prepareState==='preparing'?'Running…':simStatus==='ok'?'Passed — expected to succeed on-chain':simStatus==='failed'?'Failed — see below':simStatus==='unavailable'?'Unavailable — confirm disabled':'—'}</strong></div>
<div className="quote-row"><span>Confirmation commitment</span><strong>confirmed</strong></div>
</div>
{simStatus==='failed'&&simMessage?(<p className="error">{simMessage}</p>):null}
{simStatus==='unavailable'&&simMessage?(<p className="error">{simMessage}</p>):null}
{prepareState==='preparing'?(<p className="message">{SAFETY_COPY.preparing}</p>):null}
{rvBand==='high'?(
<label style={{display:'flex',gap:'8px',alignItems:'flex-start',margin:'10px 0',fontSize:'13px',color:'#ff7b7b'}}>
<input type="checkbox" checked={impactAck} onChange={e=>setImpactAck(e.target.checked)} style={{marginTop:'3px'}}/>
<span>{SAFETY_COPY.ackLabel}</span>
</label>):null}
{rvBlockers.length?(<div role="alert">{rvBlockers.map(b=>(<p key={b} className="error" style={{margin:'6px 0'}}>{b}</p>))}</div>):null}
{phase==='signing'?(<p className="message">{SWAP_COPY.waitingApproval}</p>):null}
{phase==='pending'&&activeSig?(
<div className="pending-box">
<p className="message">{SWAP_COPY.pendingConfirm}</p>
<p className="sig-line">Signature: <a href={explorerUrl(activeSig)} target="_blank" rel="noreferrer">{activeSig}</a></p>
</div>):null}
<div className="wallet-row">
<button type="button" className="btn btn-primary" onClick={()=>{void onConfirmSwap();}} disabled={busy||phase==='signing'||phase==='pending'||!rvConfirmArmed}>{phase==='signing'?'Waiting for wallet…':phase==='pending'?'Confirming on-chain…':SAFETY_COPY.confirmLabel}</button>
<button type="button" className="btn" onClick={onCancelReview} disabled={phase==='signing'||phase==='pending'}>Back</button>
{prepareState==='blocked'?(<button type="button" className="btn" onClick={()=>{if(reviewQuote&&publicKey)void prepareSwap(reviewQuote,publicKey.toBase58());}} disabled={busy||phase==='signing'||phase==='pending'}>{SAFETY_COPY.retryPrepare}</button>):null}
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
<a className="back-link" href="/">← Back to Turnpool</a>
</section></div>);}
