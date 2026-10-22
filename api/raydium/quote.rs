// Reconstructed 2026-10-21 from [1500-byte head byte-faithful to the deployed production source; the Raydium public Trade API contract and the same-author raydium-swap-starter backend/src/api.rs — STRUCTURE ONLY, not the deployed source]; not byte-identical to the deployed original.
// FLAGGED UNEVIDENCED: GET {RAYDIUM_TRADE_API}/compute/swap-base-in, txVersion "V0", and the "UPSTREAM_ERROR" code are the task's conservative choices (the endpoint path and V0 are also the starter's/author's convention). A16 (2026-10-22): success responses are wrapped as {ok:true,data:<upstream JSON>} — the bare upstream JSON broke the frontend's envelope contract (browser acceptance 2026-10-22: "Backend request failed (HTTP 200)" on a 200 with valid quote data). Upstream payloads are validated (success flag + /data/outputAmount) before wrapping; no swap math was invented. The deployed original's exact response handling beyond the head is unevidenced.
#[path = "../../src/raydium_common.rs"] mod common;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use vercel_runtime::{run,service_fn,Error,Request};
#[derive(Deserialize)] struct QuoteRequest{input_mint:String,output_mint:String,amount:String,slippage_bps:Option<u16>}
#[derive(Serialize)] struct QuoteQuery<'a>{#[serde(rename="inputMint")] input_mint:&'a str,#[serde(rename="outputMint")] output_mint:&'a str,amount:&'a str,#[serde(rename="slippageBps")] slippage_bps:u16,#[serde(rename="txVersion")] tx_version:&'static str}
#[tokio::main] async fn main()->Result<(),Error>{run(service_fn(handler)).await}
async fn handler(req:Request)->Result<Value,Error>{
 let r:QuoteRequest=match common::read_json(req).await{Ok(v)=>v,Err(e)=>return Ok(e)};
 if !common::is_supported_pair(&r.input_mint,&r.output_mint){return Ok(common::api_error("PAIR_NOT_ALLOWED","This token pair is not enabled."));}
 let amount=match r.amount.trim().parse::<u64>(){Ok(v) if v>0=>v,_=>return Ok(common::api_error("INVALID_AMOUNT","Amount must be a positive integer string in token base units."))};
 let slip=r.slippage_bps.unwrap_or(50); if slip==0||slip>500{return Ok(common::api_error("INVALID_SLIPPAGE","Slippage must be between 1 and 500 basis points."));}
 let client=match common::client(){Ok(v)=>v,Err(_)=>return Ok(common::api_error("BACKEND_CONFIGURATION","Could not initialize HTTP client."))};
 let amount=amount.to_string();
 let q=QuoteQuery{input_mint:&r.input_mint,output_mint:&r.output_mint,amount:&amount,slippage_bps:slip,tx_version:"V0"};
 let url=format!("{}/compute/swap-base-in",common::RAYDIUM_TRADE_API);
 let resp=match client.get(&url).query(&q).send().await{Ok(r)=>r,Err(_)=>return Ok(common::api_error("UPSTREAM_ERROR","Raydium quote request failed."))};
 if !resp.status().is_success(){return Ok(common::api_error("UPSTREAM_ERROR","Raydium quote request failed."));}
 // N4.2/A16 (2026-10-22): wrap the upstream JSON in the house envelope — the frontend's postJson
 // requires {ok:true,data}. Validate the upstream payload first (success flag + output amount),
 // mirroring the quote-validation contract build.rs enforces.
 match resp.json::<Value>().await{
  Ok(v)=>{
   let ok_upstream=v.get("success").and_then(Value::as_bool)==Some(true);
   let has_amount=v.pointer("/data/outputAmount").and_then(Value::as_str).map(|s|!s.is_empty())==Some(true);
   if !ok_upstream||!has_amount{return Ok(common::api_error("UPSTREAM_ERROR","Raydium returned an invalid quote."));}
   Ok(json!({"ok":true,"data":v}))
  },
  Err(_)=>Ok(common::api_error("UPSTREAM_ERROR","Could not parse the Raydium quote response."))
 }
}
