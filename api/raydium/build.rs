// Reconstructed 2026-10-21 from [1500-byte head byte-faithful to the deployed production source; the Raydium public Trade API contract and the same-author raydium-swap-starter backend/src/api.rs — STRUCTURE ONLY, not the deployed source]; not byte-identical to the deployed original.
// FLAGGED UNEVIDENCED: The task brief guessed POST {RAYDIUM_TRADE_API}/transaction/swap with a "quote" payload key and wrapSol=input-only, but the same-author starter template (structure evidence only, NOT the deployed source) plus the public Raydium Trade API contract use POST {RAYDIUM_TRADE_API}/transaction/swap-base-in, key "swapResponse", "computeUnitPriceMicroLamports":"50000" (the starter's value), and wrapSol=input||output — so the tail follows the author's convention, flagged here. "UPSTREAM_ERROR"/"BACKEND_CONFIGURATION" codes are unevidenced. Pass-through only — Raydium's JSON response is forwarded untouched; no transaction math was invented.
#[path = "../../src/raydium_common.rs"] mod common;
use serde::Deserialize;
use serde_json::{json,Value};
use vercel_runtime::{run,service_fn,Error,Request};
#[derive(Deserialize)] struct BuildRequest{wallet_pubkey:String,input_mint:String,output_mint:String,quote:Value}
#[tokio::main] async fn main()->Result<(),Error>{run(service_fn(handler)).await}
async fn handler(req:Request)->Result<Value,Error>{
 let r:BuildRequest=match common::read_json(req).await{Ok(v)=>v,Err(e)=>return Ok(e)};
 if !common::is_supported_pair(&r.input_mint,&r.output_mint){return Ok(common::api_error("PAIR_NOT_ALLOWED","This token pair is not enabled."));}
 if !common::valid_pubkey(&r.wallet_pubkey){return Ok(common::api_error("INVALID_WALLET","wallet_pubkey must decode to a 32-byte Solana public key."));}
 if r.quote.pointer("/data/inputMint").and_then(Value::as_str)!=Some(r.input_mint.as_str())||r.quote.pointer("/data/outputMint").and_then(Value::as_str)!=Some(r.output_mint.as_str()){return Ok(common::api_error("QUOTE_PAIR_MISMATCH","Quote mints must match requested input/output."));}
 if r.quote.get("success").and_then(Value::as_bool)!=Some(true)||r.quote.get("id").and_then(Value::as_str).is_none()||r.quote.pointer("/data/outputAmount").is_none(){return Ok(common::api_error("INVALID_QUOTE","Quote is invalid or incomplete; request a fresh quote."));}
 let input_is_sol=r.input_mint==common::WSOL_MINT;
 let output_is_sol=r.output_mint==common::WSOL_MINT;
 let payload=json!({"computeUnitPriceMicroLamports":"50000","swapResponse":&r.quote,"txVersion":"V0","wallet":&r.wallet_pubkey,"wrapSol":input_is_sol||output_is_sol,"unwrapSol":output_is_sol});
 let client=match common::client(){Ok(v)=>v,Err(_)=>return Ok(common::api_error("BACKEND_CONFIGURATION","Could not initialize HTTP client."))};
 let url=format!("{}/transaction/swap-base-in",common::RAYDIUM_TRADE_API);
 let resp=match client.post(&url).json(&payload).send().await{Ok(r)=>r,Err(_)=>return Ok(common::api_error("UPSTREAM_ERROR","Raydium transaction build request failed."))};
 match resp.json::<Value>().await{Ok(v)=>Ok(v),Err(_)=>Ok(common::api_error("UPSTREAM_ERROR","Could not parse the Raydium build response."))}
}
