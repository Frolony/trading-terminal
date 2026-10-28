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
// N5.28 will extend this route (getTokenSupply for the stablecoin card);
// this file's balances path is unchanged by that extension.
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

#[tokio::main]
async fn main() -> Result<(), Error> {
    run(service_fn(handler)).await
}

async fn handler(req: Request) -> Result<Value, Error> {
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
    fn spl_unparseable_account_is_skipped_not_fatal() {
        let v = json!({"value": [
            {"account": {"data": {"parsed": {"info": {"tokenAmount": {"uiAmount": 2.0}}}}}},
            {"account": {"data": "base64-blob"}},
        ]});
        assert_eq!(parse_spl(&v).unwrap(), 2.0);
    }
}
