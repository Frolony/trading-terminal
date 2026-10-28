// N7.1 historical candle store — server-side coverage API.
// Shared by the chart (N3.1, daily ranges) and the backtest engine (N7.2).
//
// The upstream fetch/parse/validate layer lives in src/ohlcv_source.rs
// (extracted verbatim from this file 2026-10-22, N7.2); this handler keeps the
// response envelope, the 10-minute response cache, and the query contract.
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
//   set the GECKO_DEMO_API_KEY env var (free CoinGecko demo key) and, per
//   N7.16, the endpoint attempts BOTH keyed bases in order (CoinGecko
//   onchain, then api.geckoterminal.com/api/v2), caching per process the
//   base that answers; a total failure names each attempted base + status.
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
#[path = "../src/ohlcv_source.rs"] mod ohlcv_source;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use vercel_runtime::{run, service_fn, Error, Request};

const CACHE_TTL: Duration = Duration::from_secs(600);

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
    let tf = q.timeframe.as_deref().unwrap_or("day");
    if tf != "day" {
        return Ok(common::api_error("INVALID_TIMEFRAME", "Only timeframe=day is supported in v1."));
    }
    let agg = q.aggregate.unwrap_or(1).clamp(1, 30);
    let target_days = q.target_days.unwrap_or(ohlcv_source::DEFAULT_TARGET_DAYS).clamp(1, 2000);

    // Resolve the source: SOL-USD → Kraken; pool slugs/addresses → GeckoTerminal.
    let source = match ohlcv_source::resolve_source(q.pair.as_deref(), q.pool.as_deref()) {
        Ok(s) => s,
        Err(e) => return Ok(e),
    };

    let (pair_label, pool_addr, src_name) = ohlcv_source::source_meta(&source);
    let cache_key = ohlcv_source::cache_key(&source, agg, target_days);
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

    let (bars_raw, note) = match ohlcv_source::fetch_bars(&client, &source, agg, target_days).await {
        Ok(x) => x,
        Err(e) => return Ok(e),
    };

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
