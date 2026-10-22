// Reconstructed 2026-10-21 from [1500-byte head byte-faithful to the deployed production source (Vercel connector file listing); name/signature contracts for read_json, api_error, is_supported_pair, client, valid_pubkey taken from the api/raydium/quote.rs and build.rs heads; 64KB collect-then-parse body pattern and the {"ok":false,"error"} envelope style from complete api/validate.rs; 10s client timeout rationale from hidden_files/prep/N1.6-feed-resilience.md; pair-list shape, bidirectional is_supported_pair logic, and the bs58-decode-to-32-bytes pubkey check from raydium-swap-starter backend/src/tokens.rs + api.rs — STRUCTURE ONLY, that template is NOT the deployed source]; not byte-identical to the deployed original.
// FLAGGED UNEVIDENCED: Pair id strings ("SOL-USDC" etc.) follow the same author's starter convention — the production originals are unevidenced. read_json error codes "invalid_json"/"request_too_large" mirror validate.rs style, but the original common.rs wording is unevidenced. api_error envelope gains a "message" field (genuine validate.rs envelope has ok+error only). Bidirectional pair acceptance and the 10s timeout are the task's conservative contracts, also unevidenced.
use http_body_util::BodyExt;
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::time::Duration;
use vercel_runtime::Request;

pub const RAYDIUM_TRADE_API: &str = "https://transaction-v1.raydium.io";
pub const WSOL_MINT: &str = "So11111111111111111111111111111111111111112";
pub const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
pub const USDT_MINT: &str = "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB";
pub const RAY_MINT: &str = "4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R";
pub const JUP_MINT: &str = "JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN";
pub const BONK_MINT: &str = "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263";

#[derive(Clone, Copy, serde::Serialize)]
pub struct Token { pub symbol: &'static str, pub name: &'static str, pub mint: &'static str, pub decimals: u8 }
#[derive(Clone, Copy, serde::Serialize)]
pub struct Pair { pub id: &'static str, pub base_symbol: &'static str, pub base_mint: &'static str, pub quote_symbol: &'static str, pub quote_mint: &'static str }

pub const TOKENS: &[Token] = &[
 Token{symbol:"SOL",name:"Wrapped SOL",mint:WSOL_MINT,decimals:9},
 Token{symbol:"USDC",name:"USD Coin",mint:USDC_MINT,decimals:6},
 Token{symbol:"USDT",name:"Tether USD",mint:USDT_MINT,decimals:6},
 Token{symbol:"RAY",name:"Raydium",mint:RAY_MINT,decimals:6},
 Token{symbol:"JUP",name:"Jupiter",mint:JUP_MINT,decimals:6},
 Token{symbol:"BONK",name:"Bonk",mint:BONK_MINT,decimals:5}
];
pub const PAIRS: &[Pair] = &[ Pair{id:"SOL-USDC",base_symbol:"SOL",base_mint:WSOL_MINT,quote_symbol:"USDC",quote_mint:USDC_MINT},
 Pair{id:"SOL-USDT",base_symbol:"SOL",base_mint:WSOL_MINT,quote_symbol:"USDT",quote_mint:USDT_MINT},
 Pair{id:"USDC-USDT",base_symbol:"USDC",base_mint:USDC_MINT,quote_symbol:"USDT",quote_mint:USDT_MINT},
 Pair{id:"RAY-USDC",base_symbol:"RAY",base_mint:RAY_MINT,quote_symbol:"USDC",quote_mint:USDC_MINT},
 Pair{id:"JUP-USDC",base_symbol:"JUP",base_mint:JUP_MINT,quote_symbol:"USDC",quote_mint:USDC_MINT},
 Pair{id:"BONK-USDC",base_symbol:"BONK",base_mint:BONK_MINT,quote_symbol:"USDC",quote_mint:USDC_MINT}
];

pub fn api_error(code:&str,msg:&str)->Value{json!({"ok":false,"error":code,"message":msg})}

pub async fn read_json<T:DeserializeOwned>(req:Request)->Result<T,Value>{
 let collected=match req.into_body().collect().await{Ok(v)=>v,Err(_)=>return Err(api_error("invalid_json","Could not read the request body."))};
 let bytes=collected.to_bytes();
 if bytes.len()>65536{return Err(api_error("request_too_large","Request body exceeds the 64KB limit."));}
 match serde_json::from_slice(&bytes){Ok(v)=>Ok(v),Err(_)=>Err(api_error("invalid_json","Request body is not valid JSON."))}
}

pub fn is_supported_pair(input:&str,output:&str)->bool{PAIRS.iter().any(|p|(p.base_mint==input&&p.quote_mint==output)||(p.base_mint==output&&p.quote_mint==input))}

pub fn client()->Result<Client,reqwest::Error>{Client::builder().timeout(Duration::from_secs(10)).build()}

pub fn valid_pubkey(s:&str)->bool{match bs58::decode(s).into_vec(){Ok(v)=>v.len()==32,Err(_)=>false}}
