// Shared server-side OHLCV ingestion for the terminal's Rust API.
//
// Extracted 2026-10-22 (N7.2) from api/ohlcv.rs — the fetch/parse/validate
// logic is byte-for-byte the N7.1 implementation; api/ohlcv.rs now calls into
// this module and api/backtest.rs (the N7.2 backtest engine) shares it, so the
// chart (N3.1) and the backtest engine (N7.2) always read the same bars from
// the same sources ("build once, use twice").
//
// Intended inclusion (bins only — never compiled standalone):
//   #[path = "../src/ohlcv_source.rs"] mod ohlcv_source;
// The module reaches the sibling `common` helper module through `crate::common`,
// so every including bin must declare `mod common` (via ../src/raydium_common.rs).
//
// Sources (unchanged from N7.1):
// - Source::Kraken  → Kraken public OHLC (SOLUSD, interval=1440): ~720 daily
//   bars, no key, reachable from server IPs. This is also the exact data the
//   Strategy Lab Playbook's own backtests ran on (Kraken daily 2024-10-27 →
//   2026-10-17), so engine results stay comparable to the Playbook's numbers
//   under the same house rules.
// - Source::Gecko   → GeckoTerminal pool OHLCV (DEX-native daily bars,
//   before_timestamp pagination). BLOCKED server-side as of 2026-10-22:
//   api.geckoterminal.com keyless 401s from Vercel egress IPs. Upgrade path:
//   set the GECKO_DEMO_API_KEY env var (free CoinGecko demo key) and the
//   endpoint switches to the keyed api.coingecko.com/api/v3/onchain base.

use serde_json::Value;
use std::env;

const GECKO_BASE: &str = "https://api.geckoterminal.com/api/v2/networks/solana/pools";
const COINGECKO_ONCHAIN_BASE: &str = "https://api.coingecko.com/api/v3/onchain/networks/solana/pools";
const KRAKEN_OHLC: &str = "https://api.kraken.com/0/public/OHLC";
const SOURCE_GECKO: &str = "geckoterminal";
const SOURCE_KRAKEN: &str = "kraken";
const UPSTREAM_LIMIT: u32 = 1000;
const MAX_PAGES: usize = 5;
const BROWSER_UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

/// Default daily-bar backfill target shared by /api/ohlcv and /api/backtest.
pub const DEFAULT_TARGET_DAYS: u32 = 730;

/// One validated daily bar: (t, o, h, l, c, v), t = UTC-day-start epoch seconds.
pub type RawBar = (u64, f64, f64, f64, f64, f64);

pub struct PoolDef {
    pub pair: &'static str,
    pub address: &'static str,
}
// Highest-volume pools whose GeckoTerminal daily OHLCV history is deepest.
// SOL-USDC's pool carries 365 daily bars (2025-10-24 → present, verified
// 2026-10-22); alternates carry ~183. (Server-side ingestion blocked until a
// GECKO_DEMO_API_KEY is configured — see header.)
pub const POOLS: &[PoolDef] = &[
    PoolDef{pair:"SOL-USDC", address:"Czfq3xZZDmsdGdUyrNLtRhGc47cXcZtLG4crryfu44zE"},
    PoolDef{pair:"RAY-USDC", address:"6UmmUiYoBjSrhakAobJw8BvkmJtDVxaeBtbt7rxWo1mg"},
    PoolDef{pair:"JUP-USDC", address:"HfgjZDmexhFVD28Vkb1NbQwWeXP3uDcVTLPjSGHmRHhL"},
];

/// Resolved upstream source for a request.
pub enum Source {
    Kraken,
    Gecko { pair_label: String, pool_addr: String },
}

/// Resolve `?pair=<slug>` / `?pool=<address>` to a Source, with the same
/// normalization and errors the N7.1 handler returned.
pub fn resolve_source(pair: Option<&str>, pool: Option<&str>) -> Result<Source, Value> {
    match pair.map(|p| p.trim().to_ascii_uppercase().replace('_', "-")) {
        Some(want) if want == "SOL-USD" => Ok(Source::Kraken),
        Some(want) => match POOLS.iter().find(|d| d.pair == want) {
            Some(d) => Ok(Source::Gecko { pair_label: d.pair.to_string(), pool_addr: d.address.to_string() }),
            None => Err(crate::common::api_error("UNKNOWN_PAIR", "No OHLCV source is configured for this pair slug. Try ?pair=sol-usd.")),
        },
        None => match pool.map(|a| a.trim().to_string()) {
            Some(a) if crate::common::valid_pubkey(&a) => Ok(Source::Gecko { pair_label: "custom".to_string(), pool_addr: a }),
            _ => Err(crate::common::api_error("MISSING_PARAM", "Provide ?pair=<slug> (sol-usd, sol-usdc, ray-usdc, jup-usdc) or ?pool=<address>.")),
        },
    }
}

/// (pair_label, pool_address, source_name) for response envelopes.
pub fn source_meta(source: &Source) -> (String, String, &'static str) {
    match source {
        Source::Kraken => ("SOL-USD".to_string(), "kraken:SOLUSD".to_string(), SOURCE_KRAKEN),
        Source::Gecko { pair_label, pool_addr } => (pair_label.clone(), pool_addr.clone(), SOURCE_GECKO),
    }
}

/// Cache key for a source + request shape (response caches live in the bins).
pub fn cache_key(source: &Source, agg: u32, target_days: u32) -> String {
    match source {
        Source::Kraken => format!("kraken|day|{}", target_days),
        Source::Gecko { pool_addr, .. } => format!("{}|day|{}|{}", pool_addr, agg, target_days),
    }
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

async fn fetch_kraken(client: &reqwest::Client) -> Result<Vec<RawBar>, Value> {
    let url = format!("{}/?pair=SOLUSD&interval=1440", KRAKEN_OHLC);
    let resp = match client.get(&url).header("User-Agent", BROWSER_UA).header("Accept", "application/json").send().await {
        Ok(r) => r,
        Err(_) => return Err(crate::common::api_error("UPSTREAM_ERROR", "Kraken OHLC request failed.")),
    };
    if !resp.status().is_success() {
        return Err(crate::common::api_error("UPSTREAM_ERROR", &format!("Kraken OHLC returned HTTP {}.", resp.status().as_u16())));
    }
    let body: Value = match resp.json().await {
        Ok(v) => v,
        Err(_) => return Err(crate::common::api_error("UPSTREAM_ERROR", "Could not parse the Kraken OHLC response.")),
    };
    if let Some(errs) = body.get("error").and_then(|e| e.as_array()) {
        if !errs.is_empty() {
            return Err(crate::common::api_error("UPSTREAM_ERROR", "Kraken OHLC returned an error payload."));
        }
    }
    let rows = match body.pointer("/result/SOLUSD").and_then(|v| v.as_array()) {
        Some(r) => r,
        None => return Err(crate::common::api_error("UPSTREAM_ERROR", "Kraken OHLC response has no SOLUSD series.")),
    };
    let mut out: Vec<RawBar> = Vec::with_capacity(rows.len());
    for r in rows {
        let a = match r.as_array() { Some(a) if a.len() >= 7 => a, _ => continue };
        let (t, o, h, l, c, v) = (ts(&a[0]), num(&a[1]), num(&a[2]), num(&a[3]), num(&a[4]), num(&a[6]));
        if valid_bar(t, o, h, l, c, v) { out.push((t, o, h, l, c, v)); }
    }
    out.sort_by_key(|b| b.0);
    out.dedup_by_key(|b| b.0);
    Ok(out)
}

async fn fetch_gecko(client: &reqwest::Client, pool_addr: &str, agg: u32, target_days: u32) -> Result<Vec<RawBar>, Value> {
    // Keyed upgrade path: a free CoinGecko demo key unlocks the onchain base,
    // which does not 401 server IPs the way api.geckoterminal.com keyless does.
    let demo_key = env::var("GECKO_DEMO_API_KEY").ok().filter(|k| !k.trim().is_empty());
    let base = if demo_key.is_some() { COINGECKO_ONCHAIN_BASE } else { GECKO_BASE };
    let mut raw: Vec<RawBar> = Vec::new();
    let mut before: Option<u64> = None;
    for _ in 0..MAX_PAGES {
        let mut url = format!("{}/{}/ohlcv/day?aggregate={}&limit={}", base, pool_addr, agg, UPSTREAM_LIMIT);
        if let Some(b) = before { url = format!("{}&before_timestamp={}", url, b); }
        let mut req = client.get(&url).header("User-Agent", BROWSER_UA).header("Accept", "application/json");
        if let Some(k) = demo_key.as_deref() { req = req.header("x-cg-demo-api-key", k); }
        let resp = match req.send().await {
            Ok(r) => r,
            Err(_) => return Err(crate::common::api_error("UPSTREAM_ERROR", "GeckoTerminal OHLCV request failed.")),
        };
        if resp.status().as_u16() == 401 {
            return Err(crate::common::api_error("UPSTREAM_BLOCKED", "GeckoTerminal rejected the server-side request (HTTP 401). Its keyless API serves browsers but blocks datacenter IPs; set the GECKO_DEMO_API_KEY env var (free CoinGecko demo key) to enable server-side DEX ingestion."));
        }
        if !resp.status().is_success() {
            return Err(crate::common::api_error("UPSTREAM_ERROR", &format!("GeckoTerminal OHLCV returned HTTP {}.", resp.status().as_u16())));
        }
        let body: Value = match resp.json().await {
            Ok(v) => v,
            Err(_) => return Err(crate::common::api_error("UPSTREAM_ERROR", "Could not parse the GeckoTerminal OHLCV response.")),
        };
        let rows = match body.pointer("/data/attributes/ohlcv_list").and_then(|v| v.as_array()) {
            Some(r) => r,
            None => return Err(crate::common::api_error("UPSTREAM_ERROR", "GeckoTerminal OHLCV response has no ohlcv_list.")),
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

/// Fetch + validate daily bars for a resolved source.
/// Returns (ascending, deduped bars, Option<honest shortfall note>).
/// Errors use the same api_error envelopes the N7.1 handler returned.
pub async fn fetch_bars(client: &reqwest::Client, source: &Source, agg: u32, target_days: u32) -> Result<(Vec<RawBar>, Option<String>), Value> {
    let (bars, note) = match source {
        Source::Kraken => {
            let bars = fetch_kraken(client).await?;
            let note = if (bars.len() as u32) < target_days {
                Some("Kraken serves at most 720 daily bars plus the forming day; this is the source ceiling.".to_string())
            } else { None };
            (bars, note)
        }
        Source::Gecko { pool_addr, .. } => {
            let bars = fetch_gecko(client, pool_addr, agg, target_days).await?;
            let note = if (bars.len() as u32) < target_days {
                Some(format!("GeckoTerminal free tier carries limited daily history for this pool ({} bars); coverage grows ~1 bar/day.", bars.len()))
            } else { None };
            (bars, note)
        }
    };
    if bars.len() < 2 {
        return Err(crate::common::api_error("INSUFFICIENT_DATA", "The upstream source returned too few valid daily bars."));
    }
    Ok((bars, note))
}
