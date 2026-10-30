// N4.18 (A103, 2026-10-27): server-side wallet-balance proxy.
//
// Why this route exists: client-side, every keyless path is refused for
// `getTokenAccountsByOwner` — publicnode answers -32602 ("Indexed requests
// require a personal token") and mainnet-beta 403s any request carrying the
// app Origin — while `getBalance` (SOL) passes through the N4.14 rotation.
// A server-side route carries no Origin header, so the refusal never fires:
// chat-side headerless probes 2026-10-27 called BOTH methods against
// mainnet-beta and got 200 with real parsed data.
//
// Contract: GET /api/balances?owner=<base58 pubkey>
// Response (at least one asset answered):
//   {ok:true, owner, source, balances:{sol,usdc,usdt}, errors:{sol,usdc,usdt}, cached}
//   balances hold numbers or null; errors hold null or a message string.
//   A null balance is NEVER a zero — it is an unanswered read, and the
//   client renders it with the N4.11/N4.12 honest copy, never as $0.
//   If ALL three assets fail, the route answers ok:false (UPSTREAM_FAILED)
//   so the client falls back to the N4.14 endpoint rotation instead of
//   presenting a wholly-empty result as authoritative.
// Response cache: fully-successful responses only, 30 s per owner. A
// partial or failed response is never cached, so the N4.13 retry ladder
// re-probes on its own cadence instead of re-reading a stale failure.
// Read-only public chain data: no keys, no signing, no custody. The owner
// is echoed back only because it is public address data, never a secret.
//
// N5.28 (2026-10-27, A104): this route also serves the stablecoin supply
// card (GET /api/balances?supply=1, no owner): getTokenSupply for the USDC
// and USDT mints, same server-side mainnet-beta path, same honesty rules —
// a null supply is an unanswered read, never a zero; both failing answers
// ok:false so the card renders its honest error state. The balances path
// below is unchanged by this extension; the two paths share only the
// RPC helper, the cache discipline (fully-successful only, 30 s), and
// the client-safe error strings.
//
// N3.35 (2026-10-29, A131, data-source roadmap D1): this route also
// serves EVM token facts (GET /api/balances?mode=evm-token&chain=<disc
// id>&address=0x...): a server-side read of the per-chain keyless
// Blockscout instance (/api/v2/tokens/<address>), merged here, not a
// new api/*.rs file, because Vercel Hobby allows 12 serverless
// functions and this project already uses all 12. The instance host
// is chosen server-side from the disc chain id; only chains whose
// instance was verified live are mapped (ethereum, base, zksync —
// the zksync instance at zksync.blockscout.com was confirmed live
// 2026-10-30 by a read of its Blockscout homepage, titled for
// zkSync Era; any other chain answers UNSUPPORTED_CHAIN, and the
// strip keeps today's honest dashes, never a guessed figure). Explicit User-Agent: the
// instances' CDN refuses default client signatures (the 1010 class
// in the Blockscout connector doc), so this branch builds its own
// client with TurnpoolBlockscoutClient/1.0 instead of reusing
// common::client(). DISPLAY FACTS ONLY: name/symbol, total supply
// (whole tokens), holder count, reputation flag — exactly what the
// payload carries; a missing field is null, never an estimate. A
// token the instance does not know answers TOKEN_NOT_FOUND, never a
// guessed row.
// N3.36 (2026-10-30, A132, data-source roadmap D4): this route also
// serves the keyless Coinbase Exchange cross-check (GET
// /api/balances?mode=coinbase-price&symbol=ETH|BTC|SOL): a server-side
// read of the public /products/<SYM>-USD/ticker endpoint, merged here,
// not a new api/*.rs file, for the same Hobby 12-function reason as
// N3.35. No key, no account, no signing. Only the three majors are
// mapped; anything else answers INVALID_SYMBOL and the client fetches
// nothing for it. The ticker `price` field is the last-trade price in
// USD; a missing, non-numeric, non-finite or non-positive price is
// UPSTREAM_FAILED, never a zero or a stale figure. Successful reads
// share the 30 s cache discipline; failures are never cached.
// N5.40 (2026-10-29, A126): this route also serves the mint/freeze
// authority display (GET /api/balances?mode=mint-authority&mint=<base58>):
// getAccountInfo for the mint, parsed server-side from the SPL Mint
// layout (mintAuthorityOption u32 + 32-byte authority at bytes 0..36,
// freezeAuthorityOption + authority at bytes 46..82). Merged here, not
// a new api/*.rs file, because Vercel Hobby allows 12 serverless
// functions and this project already uses all 12 (see the N4.21 merge
// note in api/solscan/holders.rs). DISPLAY FACTS ONLY: the response
// carries the two authority addresses (or null = no authority) and no
// verdict; an unknown/unreadable mint answers ok:false, never a
// guessed authority. The owner guard (Token / Token-2022 program only)
// keeps a non-mint account from being parsed as a mint.
#[path = "../src/raydium_common.rs"]
mod common;

use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use vercel_runtime::{run, service_fn, Error, Request};

const RPC_URL: &str = "https://api.mainnet-beta.solana.com";
const CACHE_TTL: Duration = Duration::from_secs(30);

static CACHE: OnceLock<Mutex<HashMap<String, (Instant, Value)>>> = OnceLock::new();
fn cache() -> &'static Mutex<HashMap<String, (Instant, Value)>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = |c: u8| (c as char).to_digit(16).unwrap_or(0) as u8;
                out.push((hex(bytes[i + 1]) << 4 | hex(bytes[i + 2])) as char);
                i += 3;
            }
            b'+' => {
                out.push(' ');
                i += 1;
            }
            b => {
                out.push(b as char);
                i += 1;
            }
        }
    }
    out
}

fn query_supply(req: &Request) -> bool {
    let qs = match req.uri().query() {
        Some(q) => q,
        None => return false,
    };
    qs.split('&').any(|kv| {
        let mut it = kv.splitn(2, '=');
        match (it.next(), it.next()) {
            (Some("supply"), Some("1")) => true,
            (Some("mode"), Some("supply")) => true,
            _ => false,
        }
    })
}

fn query_owner(req: &Request) -> Option<String> {
    let qs = req.uri().query()?;
    for kv in qs.split('&') {
        let mut it = kv.splitn(2, '=');
        if let (Some(k), Some(v)) = (it.next(), it.next()) {
            if k == "owner" {
                return Some(url_decode(v));
            }
        }
    }
    None
}

fn query_param(req: &Request, wanted: &str) -> Option<String> {
    let qs = req.uri().query()?;
    for kv in qs.split('&') {
        let mut it = kv.splitn(2, '=');
        if let (Some(k), Some(v)) = (it.next(), it.next()) {
            if k == wanted {
                return Some(url_decode(v));
            }
        }
    }
    None
}

fn query_mint_authority(req: &Request) -> bool {
    query_param(req, "mode").as_deref() == Some("mint-authority")
}

/// SPL Mint account layout (82 bytes, both Token and Token-2022):
/// bytes 0..4 mintAuthorityOption (u32 LE: 0 = none, 1 = some),
/// 4..36 mint authority, 36..44 supply, 44 decimals, 45 initialized,
/// 46..50 freezeAuthorityOption, 50..82 freeze authority. Any other
/// option word, or a buffer shorter than 82 bytes, is malformed —
/// never parsed into a guessed authority.
fn parse_mint_authorities(data: &[u8]) -> Option<(Option<String>, Option<String>)> {
    if data.len() < 82 {
        return None;
    }
    let opt = |at: usize| -> Option<Option<String>> {
        let word = u32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]]);
        match word {
            0 => Some(None),
            1 => Some(Some(bs58::encode(&data[at + 4..at + 36]).into_string())),
            _ => None,
        }
    };
    let mint = opt(0)?;
    let freeze = opt(46)?;
    Some((mint, freeze))
}

const TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const TOKEN_2022_PROGRAM: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";

/// N5.40 mint-authority branch: fully-successful responses only are
/// cached (30 s, own cache key so an authority response can never be
/// served as a balance or supply response).
async fn mint_auth_handler(req: &Request) -> Result<Value, Error> {
    let mint = match query_param(req, "mint") {
        Some(m) => m,
        None => {
            return Ok(common::api_error(
                "INVALID_MINT",
                "Query parameter mint=<base58 mint> is required.",
            ))
        }
    };
    if !common::valid_pubkey(&mint) {
        return Ok(common::api_error(
            "INVALID_MINT",
            "mint is not a valid Solana public key.",
        ));
    }
    let key = format!("mint-auth:{}", mint);
    if let Ok(guard) = cache().lock() {
        if let Some((at, val)) = guard.get(&key) {
            if at.elapsed() < CACHE_TTL {
                let mut hit = val.clone();
                if let Some(obj) = hit.as_object_mut() {
                    obj.insert("cached".to_string(), json!(true));
                }
                return Ok(hit);
            }
        }
    }
    let client = match common::client() {
        Ok(c) => c,
        Err(_) => {
            return Ok(common::api_error(
                "UPSTREAM_FAILED",
                "Could not build the upstream RPC client.",
            ))
        }
    };
    let result = match rpc_call(
        &client,
        "getAccountInfo",
        json!([mint, {"encoding": "base64", "commitment": "confirmed"}]),
    )
    .await
    {
        Ok(v) => v,
        Err(e) => {
            return Ok(common::api_error("UPSTREAM_FAILED", &e));
        }
    };
    let value = match result.get("value") {
        Some(v) if !v.is_null() => v,
        _ => {
            return Ok(common::api_error(
                "ACCOUNT_NOT_FOUND",
                "No account exists at that mint address.",
            ))
        }
    };
    let owner = value.get("owner").and_then(|o| o.as_str()).unwrap_or("");
    if owner != TOKEN_PROGRAM && owner != TOKEN_2022_PROGRAM {
        return Ok(common::api_error(
            "NOT_A_MINT",
            "That account is not owned by a token program, so it is not a token mint.",
        ));
    }
    let b64 = match value
        .get("data")
        .and_then(|d| d.as_array())
        .and_then(|a| a.first())
        .and_then(|s| s.as_str())
    {
        Some(s) => s,
        None => {
            return Ok(common::api_error(
                "UPSTREAM_FAILED",
                "Upstream account data was not base64.",
            ))
        }
    };
    use base64::Engine;
    let bytes = match base64::engine::general_purpose::STANDARD.decode(b64) {
        Ok(b) => b,
        Err(_) => {
            return Ok(common::api_error(
                "UPSTREAM_FAILED",
                "Upstream account data was not decodable base64.",
            ))
        }
    };
    let (mint_authority, freeze_authority) = match parse_mint_authorities(&bytes) {
        Some(pair) => pair,
        None => {
            return Ok(common::api_error(
                "UPSTREAM_FAILED",
                "Mint account data did not match the expected layout.",
            ))
        }
    };
    let out = json!({
        "ok": true,
        "mint": mint,
        "source": RPC_URL,
        "mintAuthority": mint_authority,
        "freezeAuthority": freeze_authority,
        "cached": false,
    });
    if let Ok(mut guard) = cache().lock() {
        guard.insert(key, (Instant::now(), out.clone()));
    }
    Ok(out)
}

/// One JSON-RPC call. Err strings are already client-safe: they name the
/// failure class and status/code, never any credential (none exists here).
async fn rpc_call(
    client: &reqwest::Client,
    method: &str,
    params: Value,
) -> Result<Value, String> {
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params});
    let resp = match client.post(RPC_URL).json(&body).send().await {
        Ok(r) => r,
        Err(_) => return Err("upstream transport error".to_string()),
    };
    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        return Err(match status {
            403 => "upstream refused (HTTP 403)".to_string(),
            429 => "upstream busy (HTTP 429)".to_string(),
            500..=599 => format!("upstream busy (HTTP {})", status),
            _ => format!("upstream error (HTTP {})", status),
        });
    }
    let j: Value = match resp.json().await {
        Ok(v) => v,
        Err(_) => return Err("upstream response was not JSON".to_string()),
    };
    if let Some(err) = j.get("error") {
        let code = err.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
        let msg = err
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("RPC error");
        let msg: String = msg.chars().take(120).collect();
        return Err(format!("RPC error {}: {}", code, msg));
    }
    match j.get("result") {
        Some(v) => Ok(v.clone()),
        None => Err("upstream response missing result".to_string()),
    }
}

/// getBalance result.value is lamports (u64). A non-numeric value is a
/// failed read, never 0 — the caller keeps the previous value instead.
fn parse_sol(result: &Value) -> Result<f64, String> {
    match result.get("value").and_then(|v| v.as_u64()) {
        Some(lamports) => Ok(lamports as f64 / 1e9),
        None => Err("malformed upstream response (getBalance)".to_string()),
    }
}

/// getTokenAccountsByOwner/jsonParsed result.value must be an array; each
/// account contributes its parsed tokenAmount.uiAmount when finite. A
/// genuine empty array sums to 0 (a real zero: the owner holds none).
/// A missing/non-array value is a failed read, never 0.
fn parse_spl(result: &Value) -> Result<f64, String> {
    let arr = match result.get("value").and_then(|v| v.as_array()) {
        Some(a) => a,
        None => {
            return Err(
                "malformed upstream response (getTokenAccountsByOwner)".to_string(),
            )
        }
    };
    let mut total = 0.0f64;
    for acct in arr {
        if let Some(ui) = acct
            .get("account")
            .and_then(|a| a.get("data"))
            .and_then(|d| d.get("parsed"))
            .and_then(|p| p.get("info"))
            .and_then(|i| i.get("tokenAmount"))
            .and_then(|t| t.get("uiAmount"))
            .and_then(|u| u.as_f64())
        {
            if ui.is_finite() {
                total += ui;
            }
        }
    }
    Ok(total)
}

/// N5.28 supply branch: same cache discipline as balances (fully-
/// successful responses only, 30 s), under its own cache key so a
/// supply response can never be served as a balance response.
async fn supply_handler(_req: &Request) -> Result<Value, Error> {
    if let Ok(guard) = cache().lock() {
        if let Some((at, val)) = guard.get("supply") {
            if at.elapsed() < CACHE_TTL {
                let mut hit = val.clone();
                if let Some(obj) = hit.as_object_mut() {
                    obj.insert("cached".to_string(), json!(true));
                }
                return Ok(hit);
            }
        }
    }
    let client = match common::client() {
        Ok(c) => c,
        Err(_) => {
            return Ok(common::api_error(
                "UPSTREAM_FAILED",
                "Could not build the upstream RPC client.",
            ))
        }
    };
    let (usdc_r, usdt_r) = tokio::join!(
        async {
            match rpc_call(&client, "getTokenSupply", json!([common::USDC_MINT])).await {
                Ok(v) => parse_supply(&v),
                Err(e) => Err(e),
            }
        },
        async {
            match rpc_call(&client, "getTokenSupply", json!([common::USDT_MINT])).await {
                Ok(v) => parse_supply(&v),
                Err(e) => Err(e),
            }
        },
    );
    let supplies = json!({
        "usdc": usdc_r.as_ref().ok().copied(),
        "usdt": usdt_r.as_ref().ok().copied(),
    });
    let errors = json!({
        "usdc": usdc_r.as_ref().err().cloned(),
        "usdt": usdt_r.as_ref().err().cloned(),
    });
    if usdc_r.is_err() && usdt_r.is_err() {
        return Ok(json!({
            "ok": false,
            "error": "UPSTREAM_FAILED",
            "message": "Both token-supply reads failed upstream; the card renders its honest error state.",
            "errors": errors,
        }));
    }
    let out = json!({
        "ok": true,
        "source": RPC_URL,
        "supplies": supplies,
        "errors": errors,
        "mints": {"usdc": common::USDC_MINT, "usdt": common::USDT_MINT},
        "cached": false,
    });
    if usdc_r.is_ok() && usdt_r.is_ok() {
        if let Ok(mut guard) = cache().lock() {
            guard.insert("supply".to_string(), (Instant::now(), out.clone()));
        }
    }
    Ok(out)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    run(service_fn(handler)).await
}

/// getTokenSupply result.value.uiAmount is the supply in whole tokens.
/// Fallback: amount (base units, string) / 10^decimals. A missing value,
/// a null uiAmount with no usable amount/decimals, or a non-finite
/// result is a failed read, never 0 — a token with genuinely zero supply
/// answers uiAmount 0, which parses cleanly.
fn parse_supply(result: &Value) -> Result<f64, String> {
    let v = match result.get("value") {
        Some(v) => v,
        None => return Err("malformed upstream response (getTokenSupply)".to_string()),
    };
    if let Some(ui) = v.get("uiAmount").and_then(|u| u.as_f64()) {
        if ui.is_finite() {
            return Ok(ui);
        }
    }
    let amount: f64 = match v.get("amount").and_then(|a| a.as_str()).and_then(|s| s.parse().ok()) {
        Some(a) => a,
        None => return Err("malformed upstream response (getTokenSupply)".to_string()),
    };
    let decimals: i32 = match v.get("decimals").and_then(|d| d.as_i64()) {
        Some(d) if (0..=18).contains(&d) => d as i32,
        _ => return Err("malformed upstream response (getTokenSupply)".to_string()),
    };
    let out = amount / 10f64.powi(decimals);
    if out.is_finite() {
        Ok(out)
    } else {
        Err("malformed upstream response (getTokenSupply)".to_string())
    }
}

// N3.35 EVM token-facts branch (data-source roadmap D1).
const BS_USER_AGENT: &str = "TurnpoolBlockscoutClient/1.0";

fn query_evm_token(req: &Request) -> bool {
    query_param(req, "mode").as_deref() == Some("evm-token")
}

/// Disc chain id -> keyless Blockscout instance base URL. Only
/// instances verified live this run are mapped; anything else is
/// UNSUPPORTED_CHAIN (honest dashes on the strip, no guessed host).
fn bs_instance(chain: &str) -> Option<&'static str> {
    match chain {
        "ethereum" => Some("https://eth.blockscout.com"),
        "base" => Some("https://base.blockscout.com"),
        "zksync" => Some("https://zksync.blockscout.com"),
        _ => None,
    }
}

fn valid_evm_address(a: &str) -> bool {
    a.len() == 42
        && a.starts_with("0x")
        && a[2..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Blockscout /tokens/<address> payload -> display facts. Every field
/// is optional: a missing or malformed field is None, never a guess.
/// Supply is total_supply (base units, string) / 10^decimals; a
/// decimals value outside 0..=18 voids only the supply, not the row.
fn parse_bs_token(j: &Value) -> (Option<String>, Option<String>, Option<f64>, Option<u64>, Option<String>) {
    let str_field = |k: &str| j.get(k).and_then(|v| v.as_str()).map(|x| x.to_string());
    let name = str_field("name");
    let symbol = str_field("symbol");
    let reputation = str_field("reputation");
    let holders: Option<u64> = match j.get("holders_count") {
        Some(v) if v.is_string() => v.as_str().and_then(|x| x.parse().ok()),
        Some(v) => v.as_u64(),
        None => None,
    };
    let decimals: Option<i64> = match j.get("decimals") {
        Some(v) if v.is_string() => v.as_str().and_then(|x| x.parse().ok()),
        Some(v) => v.as_i64(),
        None => None,
    };
    let supply: Option<f64> = match (j.get("total_supply").and_then(|v| v.as_str()).and_then(|x| x.parse::<f64>().ok()), decimals) {
        (Some(raw), Some(d)) if (0..=18).contains(&d) => {
            let out = raw / 10f64.powi(d as i32);
            if out.is_finite() { Some(out) } else { None }
        }
        _ => None,
    };
    (name, symbol, supply, holders, reputation)
}

async fn evm_token_handler(req: &Request) -> Result<Value, Error> {
    let chain = match query_param(req, "chain") {
        Some(c) => c,
        None => {
            return Ok(common::api_error(
                "INVALID_CHAIN",
                "Query parameter chain=<disc chain id> is required.",
            ))
        }
    };
    let address = match query_param(req, "address") {
        Some(a) => a,
        None => {
            return Ok(common::api_error(
                "INVALID_ADDRESS",
                "Query parameter address=<0x contract> is required.",
            ))
        }
    };
    if !valid_evm_address(&address) {
        return Ok(common::api_error(
            "INVALID_ADDRESS",
            "address must be a 0x-prefixed 40-hex-character contract address.",
        ));
    }
    let base = match bs_instance(&chain) {
        Some(b) => b,
        None => {
            return Ok(common::api_error(
                "UNSUPPORTED_CHAIN",
                "No verified keyless Blockscout instance is mapped for this chain; the strip keeps its honest dashes.",
            ))
        }
    };
    let key = format!("evm-token:{}:{}", chain, address.to_lowercase());
    if let Ok(guard) = cache().lock() {
        if let Some((at, val)) = guard.get(&key) {
            if at.elapsed() < CACHE_TTL {
                let mut hit = val.clone();
                if let Some(obj) = hit.as_object_mut() {
                    obj.insert("cached".to_string(), json!(true));
                }
                return Ok(hit);
            }
        }
    }
    let client = match reqwest::Client::builder()
        .user_agent(BS_USER_AGENT)
        .timeout(Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(_) => {
            return Ok(common::api_error(
                "UPSTREAM_FAILED",
                "Could not build the upstream Blockscout client.",
            ))
        }
    };
    let url = format!("{}/api/v2/tokens/{}", base, address);
    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(_) => {
            return Ok(common::api_error(
                "UPSTREAM_FAILED",
                "Blockscout instance transport error.",
            ))
        }
    };
    let status = resp.status().as_u16();
    if status == 404 {
        return Ok(common::api_error(
            "TOKEN_NOT_FOUND",
            "Blockscout does not know this token on this chain.",
        ));
    }
    if !resp.status().is_success() {
        return Ok(common::api_error(
            "UPSTREAM_FAILED",
            &format!("Blockscout instance error (HTTP {})", status),
        ));
    }
    let j: Value = match resp.json().await {
        Ok(v) => v,
        Err(_) => {
            return Ok(common::api_error(
                "UPSTREAM_FAILED",
                "Blockscout instance response was not JSON.",
            ))
        }
    };
    if let Some(echo) = j.get("address_hash").and_then(|v| v.as_str()) {
        if echo.to_lowercase() != address.to_lowercase() {
            return Ok(common::api_error(
                "UPSTREAM_FAILED",
                "Blockscout instance answered for a different address.",
            ));
        }
    }
    let (name, symbol, supply, holders, reputation) = parse_bs_token(&j);
    let out = json!({
        "ok": true,
        "chain": chain,
        "address": address,
        "source": base,
        "name": name,
        "symbol": symbol,
        "totalSupply": supply,
        "holdersCount": holders,
        "reputation": reputation,
        "cached": false,
    });
    if let Ok(mut guard) = cache().lock() {
        guard.insert(key, (Instant::now(), out.clone()));
    }
    Ok(out)
}

// N3.36 Coinbase cross-check branch (data-source roadmap D4).
fn query_coinbase_price(req: &Request) -> bool {
    query_param(req, "mode").as_deref() == Some("coinbase-price")
}

/// Row symbol -> Coinbase product symbol. Only the three majors map;
/// wrapped forms collapse to their underlying major. Anything else is
/// None, so the client never fetches a cross-check for it.
fn cb_product(sym: &str) -> Option<&'static str> {
    match sym {
        "ETH" | "WETH" => Some("ETH"),
        "BTC" | "WBTC" => Some("BTC"),
        "SOL" | "WSOL" => Some("SOL"),
        _ => None,
    }
}

/// Coinbase Exchange ticker payload -> USD price. The `price` field is
/// a string; anything unparseable, non-finite or non-positive is a
/// failed read, never 0.
fn parse_cb_ticker(j: &Value) -> Option<f64> {
    let p: f64 = j.get("price").and_then(|v| v.as_str()).and_then(|s| s.parse().ok())?;
    if p.is_finite() && p > 0.0 {
        Some(p)
    } else {
        None
    }
}

async fn coinbase_price_handler(req: &Request) -> Result<Value, Error> {
    let symbol = match query_param(req, "symbol") {
        Some(s) => s,
        None => {
            return Ok(common::api_error(
                "INVALID_SYMBOL",
                "Query parameter symbol=ETH|BTC|SOL is required.",
            ))
        }
    };
    let product = match cb_product(&symbol) {
        Some(p) => p,
        None => {
            return Ok(common::api_error(
                "INVALID_SYMBOL",
                "Only ETH, BTC and SOL have a Coinbase cross-check; no figure is fetched for other symbols.",
            ))
        }
    };
    let key = format!("coinbase-price:{}", product);
    if let Ok(guard) = cache().lock() {
        if let Some((at, val)) = guard.get(&key) {
            if at.elapsed() < CACHE_TTL {
                let mut hit = val.clone();
                if let Some(obj) = hit.as_object_mut() {
                    obj.insert("cached".to_string(), json!(true));
                }
                return Ok(hit);
            }
        }
    }
    let client = match common::client() {
        Ok(c) => c,
        Err(_) => {
            return Ok(common::api_error(
                "UPSTREAM_FAILED",
                "Could not build the upstream Coinbase client.",
            ))
        }
    };
    let url = format!(
        "https://api.exchange.coinbase.com/products/{}-USD/ticker",
        product
    );
    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(_) => {
            return Ok(common::api_error(
                "UPSTREAM_FAILED",
                "Coinbase Exchange transport error.",
            ))
        }
    };
    if !resp.status().is_success() {
        return Ok(common::api_error(
            "UPSTREAM_FAILED",
            &format!("Coinbase Exchange error (HTTP {})", resp.status().as_u16()),
        ));
    }
    let j: Value = match resp.json().await {
        Ok(v) => v,
        Err(_) => {
            return Ok(common::api_error(
                "UPSTREAM_FAILED",
                "Coinbase Exchange response was not JSON.",
            ))
        }
    };
    let price = match parse_cb_ticker(&j) {
        Some(p) => p,
        None => {
            return Ok(common::api_error(
                "UPSTREAM_FAILED",
                "Coinbase Exchange ticker carried no usable price.",
            ))
        }
    };
    let out = json!({
        "ok": true,
        "symbol": product,
        "product": format!("{}-USD", product),
        "price": price,
        "source": "Coinbase Exchange",
        "cached": false,
    });
    if let Ok(mut guard) = cache().lock() {
        guard.insert(key, (Instant::now(), out.clone()));
    }
    Ok(out)
}

async fn handler(req: Request) -> Result<Value, Error> {
    if query_supply(&req) {
        return supply_handler(&req).await;
    }
    if query_mint_authority(&req) {
        return mint_auth_handler(&req).await;
    }
    if query_evm_token(&req) {
        return evm_token_handler(&req).await;
    }
    if query_coinbase_price(&req) {
        return coinbase_price_handler(&req).await;
    }
    let owner = match query_owner(&req) {
        Some(o) => o,
        None => {
            return Ok(common::api_error(
                "INVALID_OWNER",
                "Query parameter owner=<pubkey> is required.",
            ))
        }
    };
    if !common::valid_pubkey(&owner) {
        return Ok(common::api_error(
            "INVALID_OWNER",
            "owner is not a valid Solana public key.",
        ));
    }

    if let Ok(guard) = cache().lock() {
        if let Some((at, val)) = guard.get(&owner) {
            if at.elapsed() < CACHE_TTL {
                let mut hit = val.clone();
                if let Some(obj) = hit.as_object_mut() {
                    obj.insert("cached".to_string(), json!(true));
                }
                return Ok(hit);
            }
        }
    }

    let client = match common::client() {
        Ok(c) => c,
        Err(_) => {
            return Ok(common::api_error(
                "UPSTREAM_FAILED",
                "Could not build the upstream RPC client.",
            ))
        }
    };

    let (sol_r, usdc_r, usdt_r) = tokio::join!(
        async {
            match rpc_call(&client, "getBalance", json!([owner])).await {
                Ok(v) => parse_sol(&v),
                Err(e) => Err(e),
            }
        },
        async {
            match rpc_call(
                &client,
                "getTokenAccountsByOwner",
                json!([owner, {"mint": common::USDC_MINT}, {"encoding": "jsonParsed", "commitment": "confirmed"}]),
            )
            .await
            {
                Ok(v) => parse_spl(&v),
                Err(e) => Err(e),
            }
        },
        async {
            match rpc_call(
                &client,
                "getTokenAccountsByOwner",
                json!([owner, {"mint": common::USDT_MINT}, {"encoding": "jsonParsed", "commitment": "confirmed"}]),
            )
            .await
            {
                Ok(v) => parse_spl(&v),
                Err(e) => Err(e),
            }
        },
    );

    let balances = json!({
        "sol": sol_r.as_ref().ok().copied(),
        "usdc": usdc_r.as_ref().ok().copied(),
        "usdt": usdt_r.as_ref().ok().copied(),
    });
    let errors = json!({
        "sol": sol_r.as_ref().err().cloned(),
        "usdc": usdc_r.as_ref().err().cloned(),
        "usdt": usdt_r.as_ref().err().cloned(),
    });
    let all_failed = sol_r.is_err() && usdc_r.is_err() && usdt_r.is_err();
    if all_failed {
        return Ok(json!({
            "ok": false,
            "error": "UPSTREAM_FAILED",
            "message": "All three balance reads failed upstream; the client falls back to its endpoint rotation.",
            "errors": errors,
        }));
    }

    let out = json!({
        "ok": true,
        "owner": owner,
        "source": RPC_URL,
        "balances": balances,
        "errors": errors,
        "cached": false,
    });
    // Cache fully-successful responses only; a partial response must not
    // pin a failed asset for the TTL while the retry ladder re-probes.
    if sol_r.is_ok() && usdc_r.is_ok() && usdt_r.is_ok() {
        if let Ok(mut guard) = cache().lock() {
            guard.insert(owner, (Instant::now(), out.clone()));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod n418_tests {
    use super::*;

    #[test]
    fn sol_lamports_to_sol() {
        let got = parse_sol(&json!({"value": 52775000})).unwrap();
        assert!((got - 0.052775).abs() < 1e-12);
        assert_eq!(parse_sol(&json!({"value": 0})).unwrap(), 0.0);
    }

    #[test]
    fn sol_non_numeric_is_failure_not_zero() {
        assert!(parse_sol(&json!({})).is_err());
        assert!(parse_sol(&json!({"value": "9"})).is_err());
    }

    #[test]
    fn spl_sums_parsed_ui_amounts() {
        let v = json!({"value": [
            {"account": {"data": {"parsed": {"info": {"tokenAmount": {"uiAmount": 9.8468}}}}}},
            {"account": {"data": {"parsed": {"info": {"tokenAmount": {"uiAmount": 0.5}}}}}},
        ]});
        let got = parse_spl(&v).unwrap();
        assert!((got - 10.3468).abs() < 1e-9);
    }

    #[test]
    fn spl_empty_array_is_genuine_zero() {
        assert_eq!(parse_spl(&json!({"value": []})).unwrap(), 0.0);
    }

    #[test]
    fn spl_missing_array_is_failure_not_zero() {
        assert!(parse_spl(&json!({})).is_err());
        assert!(parse_spl(&json!({"value": null})).is_err());
    }

    #[test]
    fn supply_ui_amount_is_used() {
        let v = json!({"value": {"amount": "123456789000000", "decimals": 6, "uiAmount": 123456789.0}});
        assert_eq!(parse_supply(&v).unwrap(), 123456789.0);
    }

    #[test]
    fn supply_amount_decimals_fallback_when_ui_amount_null() {
        let v = json!({"value": {"amount": "2500000000000", "decimals": 6, "uiAmount": null}});
        assert!((parse_supply(&v).unwrap() - 2500000.0).abs() < 1e-6); // 2.5e12 base units / 1e6 = 2.5e6 tokens
    }

    #[test]
    fn supply_genuine_zero_parses_as_zero_not_failure() {
        let v = json!({"value": {"amount": "0", "decimals": 6, "uiAmount": 0.0}});
        assert_eq!(parse_supply(&v).unwrap(), 0.0);
    }

    #[test]
    fn supply_missing_value_is_failure_not_zero() {
        assert!(parse_supply(&json!({})).is_err());
        assert!(parse_supply(&json!({"value": {"amount": null, "decimals": 6, "uiAmount": null}})).is_err());
    }

    #[test]
    fn spl_unparseable_account_is_skipped_not_fatal() {
        let v = json!({"value": [
            {"account": {"data": {"parsed": {"info": {"tokenAmount": {"uiAmount": 2.0}}}}}},
            {"account": {"data": "base64-blob"}},
        ]});
        assert_eq!(parse_spl(&v).unwrap(), 2.0);
    }
}

#[cfg(test)]
mod n540_tests {
    use super::*;

    fn mint_bytes(mint_opt: u32, mint_auth: &[u8; 32], freeze_opt: u32, freeze_auth: &[u8; 32]) -> Vec<u8> {
        let mut b = vec![0u8; 82];
        b[0..4].copy_from_slice(&mint_opt.to_le_bytes());
        b[4..36].copy_from_slice(mint_auth);
        b[36..44].copy_from_slice(&1_000_000u64.to_le_bytes());
        b[44] = 6;
        b[45] = 1;
        b[46..50].copy_from_slice(&freeze_opt.to_le_bytes());
        b[50..82].copy_from_slice(freeze_auth);
        b
    }

    #[test]
    fn closed_mint_reads_none_on_both_lines() {
        let data = mint_bytes(0, &[0u8; 32], 0, &[0u8; 32]);
        assert_eq!(parse_mint_authorities(&data).unwrap(), (None, None));
    }

    #[test]
    fn open_authorities_round_trip_through_base58() {
        let m = bs58::decode("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA")
            .into_vec()
            .unwrap();
        let f = bs58::decode("11111111111111111111111111111111")
            .into_vec()
            .unwrap();
        let mut ma = [0u8; 32];
        let mut fa = [0u8; 32];
        ma.copy_from_slice(&m);
        fa.copy_from_slice(&f);
        let data = mint_bytes(1, &ma, 1, &fa);
        let (mint, freeze) = parse_mint_authorities(&data).unwrap();
        assert_eq!(
            mint.as_deref(),
            Some("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA")
        );
        assert_eq!(freeze.as_deref(), Some("11111111111111111111111111111111"));
    }

    #[test]
    fn short_buffer_and_bad_option_word_are_malformed_not_guessed() {
        assert!(parse_mint_authorities(&[0u8; 81]).is_none());
        let mut data = mint_bytes(1, &[7u8; 32], 0, &[0u8; 32]);
        data[0..4].copy_from_slice(&2u32.to_le_bytes());
        assert!(parse_mint_authorities(&data).is_none());
        let mut data2 = mint_bytes(0, &[0u8; 32], 1, &[9u8; 32]);
        data2[46..50].copy_from_slice(&9u32.to_le_bytes());
        assert!(parse_mint_authorities(&data2).is_none());
    }
}

#[cfg(test)]
mod n335_tests {
    use super::*;

    #[test]
    fn chain_map_only_verified_instances() {
        assert_eq!(bs_instance("ethereum"), Some("https://eth.blockscout.com"));
        assert_eq!(bs_instance("base"), Some("https://base.blockscout.com"));
        assert_eq!(bs_instance("solana"), None);
        assert_eq!(bs_instance("bsc"), None);
        assert_eq!(bs_instance("zksync"), Some("https://zksync.blockscout.com"));
        assert_eq!(bs_instance(""), None);
    }

    #[test]
    fn evm_address_validation() {
        assert!(valid_evm_address("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"));
        assert!(valid_evm_address("0x4200000000000000000000000000000000000006"));
        assert!(!valid_evm_address("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"));
        assert!(!valid_evm_address("0xZZZ"));
        assert!(!valid_evm_address("So11111111111111111111111111111111111111112"));
        assert!(!valid_evm_address(""));
    }

    #[test]
    fn parse_eth_weth_payload() {
        // Shape probed live 2026-10-30 from eth.blockscout.com.
        let j = json!({"address_hash":"0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2","decimals":"18","holders_count":"3359750","name":"WETH","reputation":"ok","symbol":"WETH","total_supply":"2068149417326146965621779","type":"ERC-20"});
        let (name, symbol, supply, holders, rep) = parse_bs_token(&j);
        assert_eq!(name.as_deref(), Some("WETH"));
        assert_eq!(symbol.as_deref(), Some("WETH"));
        assert_eq!(holders, Some(3359750));
        assert_eq!(rep.as_deref(), Some("ok"));
        let s = supply.unwrap();
        assert!((s - 2068149.4173261456).abs() < 1.0, "supply={}", s);
    }

    #[test]
    fn parse_base_weth_payload() {
        // Shape probed live 2026-10-30 from base.blockscout.com.
        let j = json!({"address_hash":"0x4200000000000000000000000000000000000006","decimals":"18","holders_count":"5269729","name":"L2 Standard Bridged WETH (Base)","reputation":"ok","symbol":"WETH","total_supply":"284673381463291273179290","type":"ERC-20"});
        let (_, _, supply, holders, _) = parse_bs_token(&j);
        assert_eq!(holders, Some(5269729));
        let s = supply.unwrap();
        assert!((s - 284673.38146329127).abs() < 1.0, "supply={}", s);
    }

    #[test]
    fn parse_missing_fields_are_null_not_guessed() {
        let j = json!({"address_hash":"0x4200000000000000000000000000000000000006"});
        let (name, symbol, supply, holders, rep) = parse_bs_token(&j);
        assert!(name.is_none() && symbol.is_none() && supply.is_none() && holders.is_none() && rep.is_none());
        let j2 = json!({"decimals":"99","total_supply":"1000","holders_count":"not-a-number"});
        let (_, _, supply2, holders2, _) = parse_bs_token(&j2);
        assert!(supply2.is_none(), "out-of-range decimals must void supply only");
        assert!(holders2.is_none());
        let j3 = json!({"decimals":"6","total_supply":"2500000","holders_count":42});
        let (_, _, supply3, holders3, _) = parse_bs_token(&j3);
        assert_eq!(supply3, Some(2.5));
        assert_eq!(holders3, Some(42));
    }
}

#[cfg(test)]
mod n336_tests {
    use super::*;

    #[test]
    fn cb_product_maps_majors_and_wrapped_forms_only() {
        assert_eq!(cb_product("ETH"), Some("ETH"));
        assert_eq!(cb_product("WETH"), Some("ETH"));
        assert_eq!(cb_product("BTC"), Some("BTC"));
        assert_eq!(cb_product("WBTC"), Some("BTC"));
        assert_eq!(cb_product("SOL"), Some("SOL"));
        assert_eq!(cb_product("WSOL"), Some("SOL"));
        assert_eq!(cb_product("USDC"), None);
        assert_eq!(cb_product("BONK"), None);
        assert_eq!(cb_product("eth"), None);
        assert_eq!(cb_product(""), None);
    }

    #[test]
    fn parse_cb_ticker_shape_probed_live() {
        // Shape probed live 2026-10-30 from api.exchange.coinbase.com.
        let j = json!({"ask":"2224.87","bid":"2224.71","price":"2224.88","size":"0.00979917","time":"2026-10-30T08:33:48.354635565Z"});
        assert_eq!(parse_cb_ticker(&j), Some(2224.88));
        assert_eq!(parse_cb_ticker(&json!({"price": "not-a-number"})), None);
        assert_eq!(parse_cb_ticker(&json!({"price": "0"})), None);
        assert_eq!(parse_cb_ticker(&json!({"price": "-5"})), None);
        assert_eq!(parse_cb_ticker(&json!({})), None);
        assert_eq!(parse_cb_ticker(&json!({"price": 2224.88})), None);
    }
}
