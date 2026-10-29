// N3.31 (A122, 2026-10-29): server-side holder-distribution proxy.
//
// N4.21 MERGE NOTE (2026-10-29, Hobby function-count fix): the decoded
// wallet-activity route (A123) lives in THIS file, not in a second bin:
// Vercel's Hobby plan allows 12 serverless functions per deployment and
// this project already uses all 12, so a 13th function (the standalone
// api/solscan/activity.rs, commit 4826b27b) failed the deploy at
// patchBuild with exceeded_serverless_functions_per_deployment while
// the frontend build itself succeeded. vercel.json rewrites the public
// /api/solscan/activity URL to /api/solscan/holders?mode=activity and
// the handler branches on mode=activity before the holder flow. The
// client is unchanged. Any future server route must merge into an
// existing function the same way (or wait for a Pro plan) — never add
// a 13th api/*.rs file on Hobby.
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
const ROW_CAP: usize = 20; // N4.21: merged activity list cap
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
    // N4.21: the activity route rewrites here (see header note).
    if query_param(&req, "mode").as_deref() == Some("activity") {
        return activity_handler(req).await;
    }
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

/// Numeric unix time (seconds or milliseconds) -> epoch millis.
/// Non-numeric strings (ISO timestamps) return None; the raw string
/// is carried through for the client to format instead.
fn parse_time_ms(raw: &str) -> Option<i64> {
    let s = raw.trim();
    let v: i64 = s.parse().ok()?;
    if v >= 1_000_000_000_000 {
        Some(v)
    } else if v >= 1_000_000_000 {
        Some(v * 1000)
    } else {
        None
    }
}

fn str_field(item: &Value, key: &str) -> Option<String> {
    item.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}

fn num_field(item: &Value, key: &str) -> Option<f64> {
    item.get(key).and_then(|v| v.as_f64()).filter(|f| f.is_finite())
}

/// One normalized activity row. `time_ms` None means "sort last, show
/// time_raw"; `amount` display strings are already decimal-shifted
/// where the documented amount/decimals pair existed.
#[derive(Clone, Debug)]
struct ActRow {
    kind: &'static str,
    tx: String,
    time_ms: Option<i64>,
    time_raw: Option<String>,
    action: String,
    from: Option<String>,
    to: Option<String>,
    token1: Option<Value>,
    token2: Option<Value>,
    change: Option<Value>,
    change_type: Option<String>,
}

impl ActRow {
    fn to_json(&self) -> Value {
        json!({
            "kind": self.kind,
            "tx": self.tx,
            "time_ms": self.time_ms,
            "time_raw": self.time_raw,
            "action": self.action,
            "from": self.from,
            "to": self.to,
            "token1": self.token1,
            "token2": self.token2,
            "change": self.change,
            "change_type": self.change_type,
        })
    }
}

fn token_json(
    address: Option<String>,
    symbol: Option<String>,
    amount_str: Option<&str>,
    decimals: Option<i64>,
    amount_num: Option<f64>,
) -> Option<Value> {
    let address = address?;
    let amount = match (amount_str, decimals, amount_num) {
        (Some(s), Some(d), _) => format_amount(s, d).ok().unwrap_or_else(|| s.to_string()),
        (Some(s), None, _) => s.to_string(),
        // A JSON number amount: when it is integer-valued and
        // decimals are known, it is raw units — shift it like the
        // string form. A fractional number has no documented unit
        // here, so pass it through verbatim rather than shifting.
        (None, Some(d), Some(n)) if n.fract() == 0.0 => {
            let int_s = format!("{}", n as i64);
            format_amount(&int_s, d).unwrap_or(int_s)
        }
        (None, _, Some(n)) => {
            let mut t = format!("{}", n);
            if t.ends_with(".0") {
                t.truncate(t.len() - 2);
            }
            t
        }
        (None, _, None) => return None,
    };
    Some(json!({"address": address, "symbol": symbol, "amount": amount}))
}

fn parse_defi(item: &Value) -> Option<ActRow> {
    let tx = str_field(item, "trans_id")?;
    if tx.is_empty() {
        return None;
    }
    let time_raw = str_field(item, "time");
    let time_ms = time_raw.as_deref().and_then(parse_time_ms);
    let action = str_field(item, "activity_type").unwrap_or_else(|| "activity".to_string());
    let t1 = token_json(
        str_field(item, "token1"),
        str_field(item, "token1_symbol").or_else(|| str_field(item, "token1_name")),
        item.get("amount1").and_then(|a| a.as_str()),
        item.get("token1_decimals").and_then(|d| d.as_i64()),
        item.get("amount1").and_then(|a| a.as_f64()),
    );
    let t2 = token_json(
        str_field(item, "token2"),
        str_field(item, "token2_symbol").or_else(|| str_field(item, "token2_name")),
        item.get("amount2").and_then(|a| a.as_str()),
        item.get("token2_decimals").and_then(|d| d.as_i64()),
        item.get("amount2").and_then(|a| a.as_f64()),
    );
    // A DeFi row with neither token is still a real activity (e.g. a
    // stake action); keep it with the decoded action + tx link.
    Some(ActRow {
        kind: "defi",
        tx,
        time_ms,
        time_raw,
        action,
        from: str_field(item, "from_address"),
        to: str_field(item, "to_address"),
        token1: t1,
        token2: t2,
        change: None,
        change_type: None,
    })
}

fn parse_balance(item: &Value) -> Option<ActRow> {
    let tx = str_field(item, "trans_id")?;
    if tx.is_empty() {
        return None;
    }
    let time_raw = str_field(item, "time");
    let time_ms = time_raw.as_deref().and_then(parse_time_ms);
    let change = token_json(
        str_field(item, "token_address"),
        str_field(item, "token_symbol").or_else(|| str_field(item, "token_name")),
        item.get("amount").and_then(|a| a.as_str()),
        item.get("token_decimals").and_then(|d| d.as_i64()),
        num_field(item, "amount"),
    );
    let change_type = str_field(item, "change_type");
    Some(ActRow {
        kind: "balance",
        tx,
        time_ms,
        time_raw,
        action: "balance change".to_string(),
        from: None,
        to: None,
        token1: None,
        token2: None,
        change,
        change_type,
    })
}

/// Merge DeFi rows (winners) with balance rows (only txs not already
/// covered), newest-first by numeric time, capped at ROW_CAP.
fn merge_rows(mut defi: Vec<ActRow>, balance: Vec<ActRow>) -> Vec<ActRow> {
    let mut seen: std::collections::HashSet<String> =
        defi.iter().map(|r| r.tx.clone()).collect();
    for b in balance {
        if seen.insert(b.tx.clone()) {
            defi.push(b);
        }
    }
    // Stable sort: numeric times newest first; unparseable times keep
    // upstream order at the tail.
    defi.sort_by(|a, b| match (a.time_ms, b.time_ms) {
        (Some(x), Some(y)) => y.cmp(&x),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
    defi.truncate(ROW_CAP);
    defi
}

/// Extract the item array from a Solscan success body: `data` may be
/// an array itself or an object carrying `items`/`data`.
fn items_of(body: &Value) -> Option<&Vec<Value>> {
    match body.get("data") {
        Some(Value::Array(a)) => Some(a),
        Some(d) => d
            .get("items")
            .and_then(|i| i.as_array())
            .or_else(|| d.get("data").and_then(|i| i.as_array())),
        None => None,
    }
}

async fn fetch_endpoint(
    client: &reqwest::Client,
    url: &str,
    key: &str,
) -> Result<Vec<Value>, (&'static str, &'static str)> {
    let resp = client
        .get(url)
        .header("token", key)
        .send()
        .await
        .map_err(|_| ("UPSTREAM_FAILED", "The Solscan request failed."))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(match status.as_u16() {
            401 => (
                "KEY_INVALID",
                "Solscan rejected the server API key.",
            ),
            429 => (
                "RATE_LIMITED",
                "Solscan rate-limited the request.",
            ),
            _ => (
                "UPSTREAM_FAILED",
                "Solscan did not return activity data.",
            ),
        });
    }
    let body: Value = resp
        .json()
        .await
        .map_err(|_| ("UPSTREAM_FAILED", "Solscan returned a response that was not JSON."))?;
    if body.get("success").and_then(|s| s.as_bool()) != Some(true) {
        return Err((
            "UPSTREAM_FAILED",
            "Solscan did not report success for the activity request.",
        ));
    }
    match items_of(&body) {
        Some(items) => Ok(items.clone()),
        None => Err((
            "UPSTREAM_FAILED",
            "Solscan returned no activity list.",
        )),
    }
}

async fn activity_handler(req: Request) -> Result<Value, Error> {
    let address = match query_param(&req, "address").or_else(|| query_param(&req, "wallet")) {
        Some(a) => a,
        None => {
            return Ok(common::api_error(
                "INVALID_ADDRESS",
                "Query parameter address=<wallet address> is required.",
            ))
        }
    };
    if !common::valid_pubkey(&address) {
        return Ok(common::api_error(
            "INVALID_ADDRESS",
            "address is not a valid Solana public key.",
        ));
    }

    let cache_key = format!("activity:{}", address);
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
                "The Solscan API key is not configured on the server; activity data is unavailable.",
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
    let defi_url = format!(
        "{}/account/defi/activities?address={}&page=1&page_size=20",
        SOLSCAN_BASE, address
    );
    let bal_url = format!(
        "{}/account/balance_change?address={}&page=1&page_size=20",
        SOLSCAN_BASE, address
    );
    let defi_res = fetch_endpoint(&client, &defi_url, &key).await;
    // Key/rate errors apply to both endpoints: do not spend a second
    // call (and a second rate-limit hit) re-proving them.
    if let Err((code, _)) = &defi_res {
        if *code == "KEY_INVALID" || *code == "RATE_LIMITED" {
            return Ok(common::api_error(code, match *code {
                "KEY_INVALID" => "Solscan rejected the server API key; activity data is unavailable.",
                _ => "Solscan rate-limited the request; activity data is unavailable for now.",
            }));
        }
    }
    let bal_res = fetch_endpoint(&client, &bal_url, &key).await;
    if let Err((code, _)) = &bal_res {
        if *code == "KEY_INVALID" || *code == "RATE_LIMITED" {
            return Ok(common::api_error(code, match *code {
                "KEY_INVALID" => "Solscan rejected the server API key; activity data is unavailable.",
                _ => "Solscan rate-limited the request; activity data is unavailable for now.",
            }));
        }
    }
    let partial = defi_res.is_err() || bal_res.is_err();
    if defi_res.is_err() && bal_res.is_err() {
        let (code, msg) = defi_res.err().unwrap_or(("UPSTREAM_FAILED", "Solscan did not return activity data."));
        return Ok(common::api_error(code, msg));
    }
    let mut defi_rows: Vec<ActRow> = Vec::new();
    if let Ok(items) = &defi_res {
        for it in items {
            if let Some(r) = parse_defi(it) {
                defi_rows.push(r);
            }
        }
    }
    let mut bal_rows: Vec<ActRow> = Vec::new();
    if let Ok(items) = &bal_res {
        for it in items {
            if let Some(r) = parse_balance(it) {
                bal_rows.push(r);
            }
        }
    }
    let rows = merge_rows(defi_rows, bal_rows);
    let out = json!({
        "ok": true,
        "address": address,
        "source": "solscan",
        "activities": rows.iter().map(|r| r.to_json()).collect::<Vec<Value>>(),
        "count": rows.len(),
        "partial": partial,
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

#[cfg(test)]
mod n421_tests {
    use super::*;

    #[test]
    fn parse_time_ms_seconds_and_millis() {
        assert_eq!(parse_time_ms("1720153259"), Some(1720153259000));
        assert_eq!(parse_time_ms("1720153259000"), Some(1720153259000));
        assert_eq!(parse_time_ms("2024-07-04T12:00:00Z"), None);
        assert_eq!(parse_time_ms("42"), None);
    }

    #[test]
    fn format_amount_docs_shape() {
        assert_eq!(format_amount("2032700001514268", 6).unwrap(), "2,032,700,001.514268");
        assert_eq!(format_amount("1000000", 6).unwrap(), "1");
        assert!(format_amount("12.5", 6).is_err());
    }

    #[test]
    fn parse_defi_swap_fixture() {
        let v = json!({
            "block_id": 123456789,
            "trans_id": "5abcDEF123",
            "time": "1720153259",
            "activity_type": "swap",
            "from_address": "wallet111",
            "to_address": "wallet222",
            "token1": "So11111111111111111111111111111111111111112",
            "token1_decimals": 9,
            "amount1": "1500000000",
            "token2": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            "token2_decimals": 6,
            "amount2": "25000000"
        });
        let r = parse_defi(&v).unwrap();
        assert_eq!(r.action, "swap");
        assert_eq!(r.time_ms, Some(1720153259000));
        assert_eq!(r.token1.as_ref().unwrap().get("amount").unwrap(), "1.5");
        assert_eq!(r.token2.as_ref().unwrap().get("amount").unwrap(), "25");
    }

    #[test]
    fn parse_defi_missing_tx_is_skipped_not_fabricated() {
        let v = json!({"activity_type": "swap", "time": "1720153259"});
        assert!(parse_defi(&v).is_none());
    }

    #[test]
    fn parse_balance_fixture() {
        let v = json!({
            "trans_id": "9xyzABC789",
            "time": "1720153300",
            "token_address": "So11111111111111111111111111111111111111112",
            "token_decimals": 9,
            "amount": 500000000,
            "change_type": "inc"
        });
        let r = parse_balance(&v).unwrap();
        assert_eq!(r.kind, "balance");
        assert_eq!(r.change_type.as_deref(), Some("inc"));
        // Integer-valued number amount + decimals = raw units, shifted.
        assert_eq!(r.change.as_ref().unwrap().get("amount").unwrap(), "0.5");
    }

    #[test]
    fn merge_dedupes_same_tx_sorts_newest_caps() {
        let mk = |tx: &str, ms: Option<i64>| ActRow {
            kind: "defi",
            tx: tx.to_string(),
            time_ms: ms,
            time_raw: None,
            action: "swap".to_string(),
            from: None,
            to: None,
            token1: None,
            token2: None,
            change: None,
            change_type: None,
        };
        let mut defi = vec![mk("tx1", Some(2000)), mk("tx2", Some(1000))];
        let bal = vec![
            ActRow { kind: "balance", tx: "tx1".to_string(), time_ms: Some(2000), time_raw: None, action: "balance change".to_string(), from: None, to: None, token1: None, token2: None, change: None, change_type: Some("inc".to_string()) },
            ActRow { kind: "balance", tx: "tx3".to_string(), time_ms: Some(3000), time_raw: None, action: "balance change".to_string(), from: None, to: None, token1: None, token2: None, change: None, change_type: Some("dec".to_string()) },
        ];
        let _ = &mut defi;
        let rows = merge_rows(defi, bal);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].tx, "tx3"); // newest, balance-only
        assert_eq!(rows[1].tx, "tx1"); // defi winner, not the balance dup
        assert_eq!(rows[1].kind, "defi");
        assert_eq!(rows[2].tx, "tx2");
    }

    #[test]
    fn items_of_array_and_items_forms() {
        let a = json!({"success": true, "data": [{"trans_id": "x"}]});
        assert_eq!(items_of(&a).unwrap().len(), 1);
        let b = json!({"success": true, "data": {"items": [{"trans_id": "y"}], "total": 1}});
        assert_eq!(items_of(&b).unwrap().len(), 1);
        let c = json!({"success": true, "data": {"total": 0}});
        assert!(items_of(&c).is_none());
    }
}
