// N7.1 historical candle store — server-side coverage API.
// Shared by the chart (N3.1, daily ranges) and the backtest engine (N7.2).
// Upstream: GeckoTerminal free OHLCV (no key). Ingests daily bars with
// before_timestamp pagination, validates rows (finite, candle shape),
// merges + dedupes into ascending order, and caches in-memory (10 min TTL).
//
// Contract: GET /api/ohlcv?pair=sol-usdc
//           GET /api/ohlcv?pool=<geckoterminal pool address>
//           Optional: timeframe=day (only "day" is supported in v1),
//           aggregate=1 (1..=30), target_days=730 (1..=2000).
// Response: {ok, pair, pool_address, source, timeframe, aggregate,
//            coverage:{from,to,bars,target_days,complete}, bars:[...], cached}
// Bars: [{t,o,h,l,c,v,forming}] with t = UTC-day-start epoch seconds.
// "forming" marks today's still-forming day bar; the engine must treat it
// accordingly. coverage.complete=false is honest: the free tier only carries
// ~365 daily bars for the best SOL/USDC pool (verified 2026-10-22); coverage
// grows ~1 bar/day and the envelope always reports the real range.
// Upstream requests carry a browser User-Agent: GeckoTerminal's edge 401s
// bare reqwest/datacenter requests (observed 2026-10-22 from Vercel iad1).
#[path = "../src/raydium_common.rs"] mod common;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use vercel_runtime::{run, service_fn, Error, Request};

const GECKO_BASE: &str = "https://api.geckoterminal.com/api/v2/networks/solana/pools";
const SOURCE: &str = "geckoterminal";
const CACHE_TTL: Duration = Duration::from_secs(600);
const MAX_PAGES: usize = 5;
const UPSTREAM_LIMIT: u32 = 1000;
const DEFAULT_TARGET_DAYS: u32 = 730;

struct PoolDef { pair: &'static str, address: &'static str }
// Primary pools per pair — chosen as the highest-volume pool whose GeckoTerminal
// daily OHLCV history is deepest. SOL-USDC's pool carries 365 daily bars
// (2025-10-24 → present, verified 2026-10-22); alternates carry ~183.
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

#[tokio::main]
async fn main() -> Result<(), Error> { run(service_fn(handler)).await }

async fn handler(req: Request) -> Result<Value, Error> {
    let q = parse_query(&req);

    // Resolve the pool: pair slug (case-insensitive) or raw pool address.
    let (pair_label, pool_addr) = if let Some(p) = q.pair.as_deref() {
        let want = p.trim().to_ascii_uppercase().replace('_', "-");
        match POOLS.iter().find(|d| d.pair == want) {
            Some(d) => (d.pair.to_string(), d.address.to_string()),
            None => return Ok(common::api_error("UNKNOWN_PAIR", "No OHLCV pool is configured for this pair slug.")),
        }
    } else if let Some(a) = q.pool.as_deref() {
        let a = a.trim();
        if !common::valid_pubkey(a) {
            return Ok(common::api_error("INVALID_POOL", "pool must be a base58 Solana address."));
        }
        ("custom".to_string(), a.to_string())
    } else {
        return Ok(common::api_error("MISSING_PARAM", "Provide ?pair=<slug> or ?pool=<address>."));
    };

    let tf = q.timeframe.as_deref().unwrap_or("day");
    if tf != "day" {
        return Ok(common::api_error("INVALID_TIMEFRAME", "Only timeframe=day is supported in v1."));
    }
    let agg = q.aggregate.unwrap_or(1).clamp(1, 30);
    let target_days = q.target_days.unwrap_or(DEFAULT_TARGET_DAYS).clamp(1, 2000);

    let cache_key = format!("{}|{}|{}|{}", pool_addr, tf, agg, target_days);
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

    // Paginate upstream until we hit the target, an empty page, or the page cap.
    // GeckoTerminal's daily pages are ~183 rows even with limit=1000, and
    // before_timestamp is inclusive of the boundary row (dedupe below handles it).
    let mut raw: Vec<(u64, f64, f64, f64, f64, f64)> = Vec::new();
    let mut before: Option<u64> = None;
    for _ in 0..MAX_PAGES {
        let mut url = format!("{}/{}/ohlcv/day?aggregate={}&limit={}", GECKO_BASE, pool_addr, agg, UPSTREAM_LIMIT);
        if let Some(b) = before { url = format!("{}&before_timestamp={}", url, b); }
        let resp = match client.get(&url)
            .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")
            .header("Accept", "application/json")
            .send().await {
            Ok(r) => r,
            Err(_) => return Ok(common::api_error("UPSTREAM_ERROR", "GeckoTerminal OHLCV request failed.")),
        };
        if !resp.status().is_success() {
            return Ok(common::api_error("UPSTREAM_ERROR", &format!("GeckoTerminal OHLCV returned HTTP {}.", resp.status().as_u16())));
        }
        let body: Value = match resp.json().await {
            Ok(v) => v,
            Err(_) => return Ok(common::api_error("UPSTREAM_ERROR", "Could not parse the GeckoTerminal OHLCV response.")),
        };
        let rows = body.pointer("/data/attributes/ohlcv_list").and_then(|v| v.as_array());
        let rows = match rows { Some(r) => r, None => return Ok(common::api_error("UPSTREAM_ERROR", "GeckoTerminal OHLCV response has no ohlcv_list.")) };
        if rows.is_empty() { break; }
        let mut oldest: Option<u64> = None;
        for r in rows {
            let a = match r.as_array() { Some(a) if a.len() >= 6 => a, _ => continue };
            let t = a[0].as_u64().unwrap_or(0);
            let (o, h, l, c, v) = (a[1].as_f64().unwrap_or(f64::NAN), a[2].as_f64().unwrap_or(f64::NAN),
                                  a[3].as_f64().unwrap_or(f64::NAN), a[4].as_f64().unwrap_or(f64::NAN),
                                  a[5].as_f64().unwrap_or(f64::NAN));
            if t == 0 || !(o.is_finite() && h.is_finite() && l.is_finite() && c.is_finite() && v.is_finite()) { continue; }
            if !(h >= l && h >= o.max(c) && l <= o.min(c)) { continue; } // candle-shape guard
            if v < 0.0 { continue; }
            raw.push((t, o, h, l, c, v));
            oldest = Some(match oldest { Some(prev) => prev.min(t), None => t });
        }
        if raw.len() as u32 >= target_days { break; }
        before = oldest;
    }

    raw.sort_by_key(|r| r.0);
    raw.dedup_by_key(|r| r.0);
    if raw.len() < 2 {
        return Ok(common::api_error("INSUFFICIENT_DATA", "GeckoTerminal returned too few valid daily bars for this pool."));
    }

    let now_secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let today_start = now_secs - (now_secs % 86400);
    let bars: Vec<Value> = raw.iter().map(|(t, o, h, l, c, v)| {
        json!({"t": t, "o": o, "h": h, "l": l, "c": c, "v": v, "forming": *t == today_start})
    }).collect();

    let from = raw.first().map(|r| r.0).unwrap_or(0);
    let to = raw.last().map(|r| r.0).unwrap_or(0);
    let complete = raw.len() as u32 >= target_days;

    let resp = json!({
        "ok": true,
        "pair": pair_label,
        "pool_address": pool_addr,
        "source": SOURCE,
        "timeframe": tf,
        "aggregate": agg,
        "coverage": {"from": from, "to": to, "bars": raw.len(), "target_days": target_days, "complete": complete},
        "bars": bars,
        "cached": false,
    });
    if let Ok(mut guard) = cache().lock() {
        guard.insert(cache_key, (Instant::now(), resp.clone()));
    }
    Ok(resp)
}
