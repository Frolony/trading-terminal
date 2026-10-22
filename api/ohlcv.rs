// N7.1 historical candle store — server-side coverage API.
// Shared by the chart (N3.1, daily ranges) and the backtest engine (N7.2).
//
// Sources:
// - `?pair=sol-usd` → Kraken public OHLC (SOLUSD, interval=1440): 720 daily
//   bars, no key, reachable from server IPs. This is the PRIMARY working
//   source — it is also the exact data the Strategy Lab Playbook's own
//   backtests ran on (Kraken daily 2024-10-27 → 2026-10-17), so engine results
//   stay comparable to the Playbook's numbers under the same house rules.
// - `?pair=sol-usdc|ray-usdc|jup-usdc` or `?pool=<address>` → GeckoTerminal
//   pool OHLCV (DEX-native daily bars, before_timestamp pagination). BLOCKED
//   server-side as of 2026-10-22: api.geckoterminal.com keyless 401s from
//   Vercel egress IPs (works from browsers and this sandbox). Upgrade path:
//   set the GECKO_DEMO_API_KEY env var (free CoinGecko demo key) and the
//   endpoint switches to the keyed api.coingecko.com/api/v3/onchain base.
//
// Contract: GET /api/ohlcv?pair=sol-usd
//           GET /api/ohlcv?pair=sol-usdc | ?pool=<geckoterminal pool address>
//           Optional: timeframe=day (only "day" in v1),
//           aggregate=1 (1..=30, gecko path only), target_days=730 (1..=2000).
// Response: {ok, pair, pool_address, source, timeframe, aggregate,
//            coverage:{from,to,bars,target_days,complete,note?},
//            bars:[...], cached}
// Bars: [{t,o,h,l,c,v,forming}] with t = UTC-day-start epoch seconds.
// "forming" marks today's still-forming day bar; the engine must treat it
// accordingly. coverage.complete=false is honest about shortfalls; `note`
// explains source ceilings (Kraken's 720-bar max; GeckoTerminal's ~365-day
// free-tier depth on the best SOL/USDC pool, verified 2026-10-22).
#[path = "../src/raydium_common.rs"] mod common;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::env;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use vercel_runtime::{run, service_fn, Error, Request};

const GECKO_BASE: &str = "https://api.geckoterminal.com/api/v2/networks/solana/pools";
const COINGECKO_ONCHAIN_BASE: &str = "https://api.coingecko.com/api/v3/onchain/networks/solana/pools";
const KRAKEN_OHLC: &str = "https://api.kraken.com/0/public/OHLC";
const SOURCE_GECKO: &str = "geckoterminal";
const SOURCE_KRAKEN: &str = "kraken";
const CACHE_TTL: Duration = Duration::from_secs(600);
const MAX_PAGES: usize = 5;
const UPSTREAM_LIMIT: u32 = 1000;
const DEFAULT_TARGET_DAYS: u32 = 730;
const BROWSER_UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

type Bar = (u64, f64, f64, f64, f64, f64); // t, o, h, l, c, v

struct PoolDef { pair: &'static str, address: &'static str }
// Highest-volume pools whose GeckoTerminal daily OHLCV history is deepest.
// SOL-USDC's pool carries 365 daily bars (2025-10-24 → present, verified
// 2026-10-22); alternates carry ~183. (Server-side ingestion blocked until a
// GECKO_DEMO_API_KEY is configured — see header.)
const POOLS: &[PoolDef] = &[
    PoolDef{pair:"SOL-USDC", address:"Czfq3xZZDmsdGdUyrNLtRhGc47cXcZtLG4crryfu44zE"},
    PoolDef{pair:"RAY-USDC", address:"6UmmUiYoBjSrhakAobJw8BvkmJtDVxaeBtbt7rxWo1mg"},
    PoolDef{pair:"JUP-USDC", address:"HfgjZDmexhFVD28Vkb1NbQwWeXP3uDcVTLPjSGHmRHhL"},
];

static CACHE: OnceLock<Mutex<HashMap<String, (Instant, Value)>>> = OnceLock::new();
fn cache() -> &'static Mutex<HashMap<String, (Instant, Value)>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Deserialize, Default)]
struct Query { pair: Option<String>, pool: Option<String>, timeframe: Option<String>, aggregate: Option<u32>, target_days: Option<u32> }

fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = |c: u8| (c as char).to_digit(16).unwrap_or(0) as u8;
                out.push((hex(bytes[i+1]) << 4 | hex(bytes[i+2])) as char);
                i += 3;
            }
            b'+' => { out.push(' '); i += 1; }
            b => { out.push(b as char); i += 1; }
        }
    }
    out
}

fn parse_query(req: &Request) -> Query {
    let mut q = Query::default();
    let qs = match req.uri().query() { Some(s) => s, None => return q };
    for kv in qs.split('&') {
        let mut it = kv.splitn(2, '=');
        let (k, v) = match (it.next(), it.next()) { (Some(k), Some(v)) => (k, url_decode(v)), _ => continue };
        match k {
            "pair" => q.pair = Some(v),
            "pool" => q.pool = Some(v),
            "timeframe" => q.timeframe = Some(v),
            "aggregate" => q.aggregate = v.parse::<u32>().ok(),
            "target_days" => q.target_days = v.parse::<u32>().ok(),
            _ => {}
        }
    }
    q
}

fn num(v: &Value) -> f64 {
    v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse::<f64>().ok())).unwrap_or(f64::NAN)
}
fn ts(v: &Value) -> u64 {
    v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok())).unwrap_or(0)
}
fn valid_bar(t: u64, o: f64, h: f64, l: f64, c: f64, v: f64) -> bool {
    t != 0 && o.is_finite() && h.is_finite() && l.is_finite() && c.is_finite() && v.is_finite()
        && v >= 0.0 && h >= l && h >= o.max(c) && l <= o.min(c)
}

async fn fetch_kraken(client: &reqwest::Client) -> Result<Vec<Bar>, Value> {
    let url = format!("{}/?pair=SOLUSD&interval=1440", KRAKEN_OHLC);
    let resp = match client.get(&url).header("User-Agent", BROWSER_UA).header("Accept", "application/json").send().await {
        Ok(r) => r,
        Err(_) => return Err(common::api_error("UPSTREAM_ERROR", "Kraken OHLC request failed.")),
    };
    if !resp.status().is_success() {
        return Err(common::api_error("UPSTREAM_ERROR", &format!("Kraken OHLC returned HTTP {}.", resp.status().as_u16())));
    }
    let body: Value = match resp.json().await {
        Ok(v) => v,
        Err(_) => return Err(common::api_error("UPSTREAM_ERROR", "Could not parse the Kraken OHLC response.")),
    };
    if let Some(errs) = body.get("error").and_then(|e| e.as_array()) {
        if !errs.is_empty() {
            return Err(common::api_error("UPSTREAM_ERROR", "Kraken OHLC returned an error payload."));
        }
    }
    let rows = match body.pointer("/result/SOLUSD").and_then(|v| v.as_array()) {
        Some(r) => r,
        None => return Err(common::api_error("UPSTREAM_ERROR", "Kraken OHLC response has no SOLUSD series.")),
    };
    let mut out: Vec<Bar> = Vec::with_capacity(rows.len());
    for r in rows {
        let a = match r.as_array() { Some(a) if a.len() >= 7 => a, _ => continue };
        let (t, o, h, l, c, v) = (ts(&a[0]), num(&a[1]), num(&a[2]), num(&a[3]), num(&a[4]), num(&a[6]));
        if valid_bar(t, o, h, l, c, v) { out.push((t, o, h, l, c, v)); }
    }
    out.sort_by_key(|b| b.0);
    out.dedup_by_key(|b| b.0);
    Ok(out)
}

async fn fetch_gecko(client: &reqwest::Client, pool_addr: &str, agg: u32, target_days: u32) -> Result<Vec<Bar>, Value> {
    // Keyed upgrade path: a free CoinGecko demo key unlocks the onchain base,
    // which does not 401 server IPs the way api.geckoterminal.com keyless does.
    let demo_key = env::var("GECKO_DEMO_API_KEY").ok().filter(|k| !k.trim().is_empty());
    let base = if demo_key.is_some() { COINGECKO_ONCHAIN_BASE } else { GECKO_BASE };
    let mut raw: Vec<Bar> = Vec::new();
    let mut before: Option<u64> = None;
    for _ in 0..MAX_PAGES {
        let mut url = format!("{}/{}/ohlcv/day?aggregate={}&limit={}", base, pool_addr, agg, UPSTREAM_LIMIT);
        if let Some(b) = before { url = format!("{}&before_timestamp={}", url, b); }
        let mut req = client.get(&url).header("User-Agent", BROWSER_UA).header("Accept", "application/json");
        if let Some(k) = demo_key.as_deref() { req = req.header("x-cg-demo-api-key", k); }
        let resp = match req.send().await {
            Ok(r) => r,
            Err(_) => return Err(common::api_error("UPSTREAM_ERROR", "GeckoTerminal OHLCV request failed.")),
        };
        if resp.status().as_u16() == 401 {
            return Err(common::api_error("UPSTREAM_BLOCKED", "GeckoTerminal rejected the server-side request (HTTP 401). Its keyless API serves browsers but blocks datacenter IPs; set the GECKO_DEMO_API_KEY env var (free CoinGecko demo key) to enable server-side DEX ingestion."));
        }
        if !resp.status().is_success() {
            return Err(common::api_error("UPSTREAM_ERROR", &format!("GeckoTerminal OHLCV returned HTTP {}.", resp.status().as_u16())));
        }
        let body: Value = match resp.json().await {
            Ok(v) => v,
            Err(_) => return Err(common::api_error("UPSTREAM_ERROR", "Could not parse the GeckoTerminal OHLCV response.")),
        };
        let rows = match body.pointer("/data/attributes/ohlcv_list").and_then(|v| v.as_array()) {
            Some(r) => r,
            None => return Err(common::api_error("UPSTREAM_ERROR", "GeckoTerminal OHLCV response has no ohlcv_list.")),
        };
        if rows.is_empty() { break; }
        let mut oldest: Option<u64> = None;
        for r in rows {
            let a = match r.as_array() { Some(a) if a.len() >= 6 => a, _ => continue };
            let (t, o, h, l, c, v) = (ts(&a[0]), num(&a[1]), num(&a[2]), num(&a[3]), num(&a[4]), num(&a[5]));
            if valid_bar(t, o, h, l, c, v) {
                raw.push((t, o, h, l, c, v));
                oldest = Some(match oldest { Some(prev) => prev.min(t), None => t });
            }
        }
        if raw.len() as u32 >= target_days { break; }
        before = oldest;
    }
    raw.sort_by_key(|b| b.0);
    raw.dedup_by_key(|b| b.0);
    Ok(raw)
}

#[tokio::main]
async fn main() -> Result<(), Error> { run(service_fn(handler)).await }

async fn handler(req: Request) -> Result<Value, Error> {
    let q = parse_query(&req);
    let tf = q.timeframe.as_deref().unwrap_or("day");
    if tf != "day" {
        return Ok(common::api_error("INVALID_TIMEFRAME", "Only timeframe=day is supported in v1."));
    }
    let agg = q.aggregate.unwrap_or(1).clamp(1, 30);
    let target_days = q.target_days.unwrap_or(DEFAULT_TARGET_DAYS).clamp(1, 2000);

    // Resolve the source: SOL-USD → Kraken; pool slugs/addresses → GeckoTerminal.
    enum Source { Kraken, Gecko { pair_label: String, pool_addr: String } }
    let source = match q.pair.as_deref().map(|p| p.trim().to_ascii_uppercase().replace('_', "-")) {
        Some(want) if want == "SOL-USD" => Source::Kraken,
        Some(want) => match POOLS.iter().find(|d| d.pair == want) {
            Some(d) => Source::Gecko { pair_label: d.pair.to_string(), pool_addr: d.address.to_string() },
            None => return Ok(common::api_error("UNKNOWN_PAIR", "No OHLCV source is configured for this pair slug. Try ?pair=sol-usd.")),
        },
        None => match q.pool.as_deref().map(|a| a.trim().to_string()) {
            Some(a) if common::valid_pubkey(&a) => Source::Gecko { pair_label: "custom".to_string(), pool_addr: a },
            _ => return Ok(common::api_error("MISSING_PARAM", "Provide ?pair=<slug> (sol-usd, sol-usdc, ray-usdc, jup-usdc) or ?pool=<address>.")),
        },
    };

    let (pair_label, pool_addr, src_name, cache_key) = match &source {
        Source::Kraken => ("SOL-USD".to_string(), "kraken:SOLUSD".to_string(), SOURCE_KRAKEN, format!("kraken|day|{}", target_days)),
        Source::Gecko { pair_label, pool_addr } => (pair_label.clone(), pool_addr.clone(), SOURCE_GECKO, format!("{}|day|{}|{}", pool_addr, agg, target_days)),
    };
    if let Ok(guard) = cache().lock() {
        if let Some((at, val)) = guard.get(&cache_key) {
            if at.elapsed() < CACHE_TTL {
                let mut hit = val.clone();
                if let Some(obj) = hit.as_object_mut() { obj.insert("cached".to_string(), json!(true)); }
                return Ok(hit);
            }
        }
    }

    let client = match common::client() {
        Ok(c) => c,
        Err(_) => return Ok(common::api_error("BACKEND_CONFIGURATION", "Could not initialize HTTP client.")),
    };

    let (bars_raw, note) = match &source {
        Source::Kraken => {
            let bars = match fetch_kraken(&client).await { Ok(b) => b, Err(e) => return Ok(e) };
            let note = if (bars.len() as u32) < target_days {
                Some("Kraken serves at most 720 daily bars plus the forming day; this is the source ceiling.".to_string())
            } else { None };
            (bars, note)
        }
        Source::Gecko { pool_addr, .. } => {
            let bars = match fetch_gecko(&client, pool_addr, agg, target_days).await { Ok(b) => b, Err(e) => return Ok(e) };
            let note = if (bars.len() as u32) < target_days {
                Some(format!("GeckoTerminal free tier carries limited daily history for this pool ({} bars); coverage grows ~1 bar/day.", bars.len()))
            } else { None };
            (bars, note)
        }
    };
    if bars_raw.len() < 2 {
        return Ok(common::api_error("INSUFFICIENT_DATA", "The upstream source returned too few valid daily bars."));
    }

    let now_secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let today_start = now_secs - (now_secs % 86400);
    let bars: Vec<Value> = bars_raw.iter().map(|(t, o, h, l, c, v)| {
        json!({"t": t, "o": o, "h": h, "l": l, "c": c, "v": v, "forming": *t == today_start})
    }).collect();

    let from = bars_raw.first().map(|b| b.0).unwrap_or(0);
    let to = bars_raw.last().map(|b| b.0).unwrap_or(0);
    let complete = bars_raw.len() as u32 >= target_days;
    let mut cov_map = serde_json::Map::with_capacity(6);
    cov_map.insert("from".to_string(), json!(from));
    cov_map.insert("to".to_string(), json!(to));
    cov_map.insert("bars".to_string(), json!(bars_raw.len()));
    cov_map.insert("target_days".to_string(), json!(target_days));
    cov_map.insert("complete".to_string(), json!(complete));
    if let Some(n) = note { cov_map.insert("note".to_string(), json!(n)); }
    let coverage = Value::Object(cov_map);

    let resp = json!({
        "ok": true,
        "pair": pair_label,
        "pool_address": pool_addr,
        "source": src_name,
        "timeframe": tf,
        "aggregate": agg,
        "coverage": coverage,
        "bars": bars,
        "cached": false,
    });
    if let Ok(mut guard) = cache().lock() {
        guard.insert(cache_key, (Instant::now(), resp.clone()));
    }
    Ok(resp)
}
