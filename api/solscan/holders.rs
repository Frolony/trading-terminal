// N3.31 (A122, 2026-10-29): server-side holder-distribution proxy.
//
// Why this route exists: the ranked holder list behind a token has no
// source in the app's keyless feed set (DexScreener/GeckoTerminal rows
// carry no per-holder data). Solscan's Pro API v2.0 `token/holders` is
// the ranked list Solscan's own token page renders, and Carlos set the
// `SOLSCAN_API_KEY` project env var (production, sensitive; value never
// read by any loop run). The key is read here, server-side only, and
// sent as the `token:` header Solscan documents — it never reaches the
// page, and the page never calls pro-api.solscan.io directly.
//
// Contract: GET /api/solscan/holders?mint=<base58 mint>
// Response (success):
//   {ok:true, mint, source:"solscan", total, holders:[{rank, owner,
//   account, amount, percentage, value_usd}], top10_share, cached}
//   `amount` is whole tokens as a string, formatted server-side by
//   decimal-shifting `amount_str` (never a float: base-unit strings
//   exceed f64 integer precision for large supplies). `percentage` is
//   Solscan's own share figure, or null when Solscan omits it.
//   `top10_share` sums the first 10 items' percentages when at least
//   10 items are present and all 10 carry a percentage; with fewer
//   than 10 items it sums all of them (the client labels it
//   accordingly); it is null when any needed percentage is missing.
//   `total` is Solscan's holder count, or null when unreported.
// Response (failure): {ok:false, error, message} with error in
//   INVALID_MINT | KEY_MISSING | KEY_INVALID | RATE_LIMITED |
//   UPSTREAM_FAILED. The client renders the honest unavailable state
//   for every failure class — never a stale or partial list presented
//   as complete, and never a cached failure (only fully-successful
//   responses are cached, 300 s per mint; holder lists move slowly).
// Read-only public chain data: no signing, no custody, no execution
// surface. The mint is echoed back only because it is public data.

#[path = "../../src/raydium_common.rs"]
mod common;

use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use vercel_runtime::{run, service_fn, Error, Request};

const SOLSCAN_BASE: &str = "https://pro-api.solscan.io/v2.0";
const CACHE_TTL: Duration = Duration::from_secs(300);

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

/// Decimal-shift a base-unit string into whole tokens, as a display
/// string with comma-grouped integer part. `amount_str` must be all
/// digits (Solscan's format); anything else is a malformed item, never
/// a zero. Trailing fractional zeros are trimmed; a zero fraction is
/// dropped entirely. Examples: ("2032700001514268", 6) ->
/// "2,032,700,001.514268"; ("1000000", 6) -> "1"; ("5", 9) ->
/// "0.000000005".
fn format_amount(amount_str: &str, decimals: i64) -> Result<String, String> {
    if amount_str.is_empty() || !amount_str.bytes().all(|b| b.is_ascii_digit()) {
        return Err("amount_str is not a digit string".to_string());
    }
    if !(0..=18).contains(&decimals) {
        return Err("decimals out of range".to_string());
    }
    let digits = amount_str.trim_start_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    let d = decimals as usize;
    let (int_part, frac_part) = if d == 0 {
        (digits.to_string(), String::new())
    } else if digits.len() > d {
        let split = digits.len() - d;
        (
            digits[..split].to_string(),
            digits[split..].trim_end_matches('0').to_string(),
        )
    } else {
        (
            "0".to_string(),
            format!("{}{}", "0".repeat(d - digits.len()), digits)
                .trim_end_matches('0')
                .to_string(),
        )
    };
    let mut grouped = String::new();
    for (i, ch) in int_part.chars().enumerate() {
        if i > 0 && (int_part.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    if frac_part.is_empty() {
        Ok(grouped)
    } else {
        Ok(format!("{}.{}", grouped, frac_part))
    }
}

struct HolderRow {
    rank: i64,
    owner: String,
    account: String,
    amount: String,
    percentage: Option<f64>,
    value_usd: Option<f64>,
}

fn parse_holder(item: &Value, fallback_rank: i64) -> Result<HolderRow, String> {
    let rank = item
        .get("rank")
        .and_then(|r| r.as_i64())
        .unwrap_or(fallback_rank);
    let owner = item
        .get("owner")
        .and_then(|o| o.as_str())
        .ok_or_else(|| "holder item missing owner".to_string())?;
    let account = item
        .get("address")
        .and_then(|a| a.as_str())
        .ok_or_else(|| "holder item missing address".to_string())?;
    let amount_str = match item.get("amount_str").and_then(|a| a.as_str()) {
        Some(s) => s.to_string(),
        None => match item.get("amount").and_then(|a| a.as_u64()) {
            Some(n) => n.to_string(),
            None => return Err("holder item missing amount".to_string()),
        },
    };
    let decimals = item
        .get("decimals")
        .and_then(|d| d.as_i64())
        .ok_or_else(|| "holder item missing decimals".to_string())?;
    let amount = format_amount(&amount_str, decimals)?;
    let percentage = item
        .get("percentage")
        .and_then(|p| p.as_f64())
        .filter(|p| p.is_finite() && *p >= 0.0 && *p <= 100.0);
    let value_usd = item
        .get("value")
        .and_then(|v| v.as_f64())
        .filter(|v| v.is_finite() && *v >= 0.0);
    Ok(HolderRow {
        rank,
        owner: owner.to_string(),
        account: account.to_string(),
        amount,
        percentage,
        value_usd,
    })
}

/// Top-10 combined share: the first 10 items' percentages when at
/// least 10 items exist, else all items' percentages. Null when any
/// needed percentage is missing — a partial sum would understate the
/// concentration and must never render as the top-10 figure.
fn top_share(items: &[HolderRow]) -> Option<f64> {
    let n = items.len().min(10);
    if n == 0 {
        return None;
    }
    let mut sum = 0.0f64;
    for h in items.iter().take(n) {
        match h.percentage {
            Some(p) => sum += p,
            None => return None,
        }
    }
    if sum.is_finite() {
        Some(sum)
    } else {
        None
    }
}

async fn handler(req: Request) -> Result<Value, Error> {
    let mint = match query_param(&req, "mint").or_else(|| query_param(&req, "address")) {
        Some(m) => m,
        None => {
            return Ok(common::api_error(
                "INVALID_MINT",
                "Query parameter mint=<token mint> is required.",
            ))
        }
    };
    if !common::valid_pubkey(&mint) {
        return Ok(common::api_error(
            "INVALID_MINT",
            "mint is not a valid Solana public key.",
        ));
    }

    let cache_key = format!("holders:{}", mint);
    if let Ok(guard) = cache().lock() {
        if let Some((at, val)) = guard.get(&cache_key) {
            if at.elapsed() < CACHE_TTL {
                let mut hit = val.clone();
                if let Some(obj) = hit.as_object_mut() {
                    obj.insert("cached".to_string(), json!(true));
                }
                return Ok(hit);
            }
        }
    }

    let key = match std::env::var("SOLSCAN_API_KEY") {
        Ok(k) if !k.trim().is_empty() => k.trim().to_string(),
        _ => {
            return Ok(common::api_error(
                "KEY_MISSING",
                "The Solscan API key is not configured on the server; holder data is unavailable.",
            ))
        }
    };

    let client = match common::client() {
        Ok(c) => c,
        Err(_) => {
            return Ok(common::api_error(
                "UPSTREAM_FAILED",
                "Could not build the upstream client.",
            ))
        }
    };
    let url = format!(
        "{}/token/holders?address={}&page=1&page_size=20",
        SOLSCAN_BASE, mint
    );
    let resp = match client.get(&url).header("token", key).send().await {
        Ok(r) => r,
        Err(_) => {
            return Ok(common::api_error(
                "UPSTREAM_FAILED",
                "The Solscan request failed; holder data is unavailable.",
            ))
        }
    };
    let status = resp.status();
    if !status.is_success() {
        let (code, msg) = match status.as_u16() {
            401 => (
                "KEY_INVALID",
                "Solscan rejected the server API key; holder data is unavailable.",
            ),
            429 => (
                "RATE_LIMITED",
                "Solscan rate-limited the request; holder data is unavailable for now.",
            ),
            _ => (
                "UPSTREAM_FAILED",
                "Solscan did not return holder data; the section is unavailable.",
            ),
        };
        return Ok(common::api_error(code, msg));
    }
    let body: Value = match resp.json().await {
        Ok(v) => v,
        Err(_) => {
            return Ok(common::api_error(
                "UPSTREAM_FAILED",
                "Solscan returned a response that was not JSON.",
            ))
        }
    };
    if body.get("success").and_then(|s| s.as_bool()) != Some(true) {
        return Ok(common::api_error(
            "UPSTREAM_FAILED",
            "Solscan did not report success for the holder request.",
        ));
    }
    let data = match body.get("data") {
        Some(d) => d,
        None => {
            return Ok(common::api_error(
                "UPSTREAM_FAILED",
                "Solscan returned no holder data.",
            ))
        }
    };
    let total = data.get("total").and_then(|t| t.as_u64());
    let raw_items = match data.get("items").and_then(|i| i.as_array()) {
        Some(a) => a,
        None => {
            return Ok(common::api_error(
                "UPSTREAM_FAILED",
                "Solscan returned no holder list.",
            ))
        }
    };
    let mut holders: Vec<HolderRow> = Vec::new();
    for (i, item) in raw_items.iter().enumerate() {
        match parse_holder(item, i as i64 + 1) {
            Ok(h) => holders.push(h),
            Err(_) => continue, // skip one malformed row; never a zero row
        }
    }
    if !raw_items.is_empty() && holders.is_empty() {
        return Ok(common::api_error(
            "UPSTREAM_FAILED",
            "Solscan holder rows could not be parsed.",
        ));
    }
    let share = top_share(&holders);
    let out = json!({
        "ok": true,
        "mint": mint,
        "source": "solscan",
        "total": total,
        "holders": holders.iter().map(|h| json!({
            "rank": h.rank,
            "owner": h.owner,
            "account": h.account,
            "amount": h.amount,
            "percentage": h.percentage,
            "value_usd": h.value_usd,
        })).collect::<Vec<Value>>(),
        "top10_share": share,
        "cached": false,
    });
    if let Ok(mut guard) = cache().lock() {
        guard.insert(cache_key, (Instant::now(), out.clone()));
    }
    Ok(out)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    run(service_fn(handler)).await
}

#[cfg(test)]
mod n331_tests {
    use super::*;

    #[test]
    fn format_amount_docs_example() {
        // Solscan docs' own rank-1 example: 2032700001514268, 6 decimals.
        assert_eq!(
            format_amount("2032700001514268", 6).unwrap(),
            "2,032,700,001.514268"
        );
    }

    #[test]
    fn format_amount_whole_and_dust() {
        assert_eq!(format_amount("1000000", 6).unwrap(), "1");
        assert_eq!(format_amount("5", 9).unwrap(), "0.000000005");
        assert_eq!(format_amount("0", 6).unwrap(), "0");
        assert_eq!(format_amount("123456789", 0).unwrap(), "123,456,789");
    }

    #[test]
    fn format_amount_rejects_non_digits_and_bad_decimals() {
        assert!(format_amount("12.5", 6).is_err());
        assert!(format_amount("", 6).is_err());
        assert!(format_amount("100", 19).is_err());
        assert!(format_amount("100", -1).is_err());
    }

    #[test]
    fn parse_holder_docs_fixture() {
        let v = json!({
            "address": "26ddLrqXDext6caX1gRxARePN4kzajyGiAUz9JmzmTGQ",
            "amount": 2032700001514268u64,
            "amount_str": "2032700001514268",
            "decimals": 6,
            "owner": "61aq585V8cR2sZBeawJFt2NPqmN7zDi1sws4KLs5xHXV",
            "rank": 1,
            "value": 1032525962.971027,
            "percentage": 29.041823417809326
        });
        let h = parse_holder(&v, 1).unwrap();
        assert_eq!(h.rank, 1);
        assert_eq!(h.amount, "2,032,700,001.514268");
        assert!((h.percentage.unwrap() - 29.041823417809326).abs() < 1e-9);
    }

    #[test]
    fn parse_holder_missing_owner_is_failure_not_zero() {
        let v = json!({"address": "x", "amount_str": "100", "decimals": 6});
        assert!(parse_holder(&v, 1).is_err());
    }

    #[test]
    fn top_share_first_ten_and_partial_null() {
        let mk = |p: Option<f64>| HolderRow {
            rank: 1,
            owner: "o".to_string(),
            account: "a".to_string(),
            amount: "1".to_string(),
            percentage: p,
            value_usd: None,
        };
        let full: Vec<HolderRow> = (0..12).map(|_| mk(Some(5.0))).collect();
        assert_eq!(top_share(&full).unwrap(), 50.0);
        let mut partial: Vec<HolderRow> = (0..12).map(|_| mk(Some(5.0))).collect();
        partial[3].percentage = None;
        assert_eq!(top_share(&partial), None);
        let few: Vec<HolderRow> = (0..3).map(|_| mk(Some(10.0))).collect();
        assert_eq!(top_share(&few).unwrap(), 30.0);
        let empty: Vec<HolderRow> = vec![];
        assert_eq!(top_share(&empty), None);
    }
}
