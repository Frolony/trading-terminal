// N7.2 backtest engine — server-side strategy backtesting over the N7.1 candle store.
//
// A Rust service in the terminal's API with Carlos's uploaded `jupiter-strategies`
// crate integrated as its scheme library (vendored under src/schemes/: the
// indicators + strategies modules only — the crate's `jupiter.rs` execution client
// and main.rs runner are NOT vendored: they would pull solana-sdk/solana-client
// and other heavy deps into every serverless build; they remain at
// hidden_files/strategy-upload/ as the N7.4 starting point).
//
// This file is NEW code (not part of the production reconstruction): it is the
// "executor" the crate's README leaves to the implementer — the crate only logs
// signals over synthetic bars; this engine runs the Strategy trait over real
// N7.1 bars with Playbook house-rule accounting.
//
// N7.4 refactor: the scheme registry and the accounting loop now live in
// src/strategy_engine.rs, shared verbatim with /api/paperlive ("build once,
// use twice"). The response contract below is unchanged — the extraction is
// mechanical and the engine math is identical.
//
// House rules (Playbook defaults, stamped on every result):
// - long-only, one position at a time
// - signals evaluated after on_bar; fills at the signal bar's close
// - fee_bps per side (default 26 = 0.26%), deducted from deployed capital
// - deploy_pct of equity deployed per entry (default 95)
// - the forming (today's) bar is EXCLUDED — it is not a completed daily bar
// - warmup: indicators are fed from the first bar, but trading (and the equity
//   curve) starts after `warmup_bars` = the scheme's max indicator lookback,
//   stamped as `trading_from`
// - no entry on the final bar (no bar left to exit on); any open position is
//   force-closed at the last bar's close (exit_reason "end_of_data")
// - the buy-and-hold benchmark uses IDENTICAL deploy and fee assumptions
//
// Deterministic: the engine is a pure function of (scheme, params, bars, fee,
// deploy). The data range actually used is stamped on every result.
//
// Contract: GET /api/backtest?scheme=<id>&pair=sol-usd
//   Required: scheme=<one of the 10 ids below>.
//   Optional: pair=<slug> | pool=<address> (default pair=sol-usd),
//     timeframe=day (only "day" in v1), target_days=730 (1..=2000),
//     fee_bps=26 (0..=1000), deploy_pct=95 (1..=100),
//     from=<epoch secs>, to=<epoch secs> (slice applied before warmup),
//     scheme params: atr_period, mult, max_hold (supertrend family);
//                    period, max_hold (aroon time-stop variants);
//                    period (aroon pure variants).
//   Params that are not tunable for the chosen scheme are rejected with
//   INVALID_PARAM (strict contract so the N7.3 UI knows what's editable).
// Response: {ok, run_id ("bt-<scheme>-<12 hex fnv1a>", deterministic for the
//   experiment), run_at (ISO-8601 UTC of computation), run_epoch, scheme:{id,name,
//   playbook_no,playbook_asset,playbook_edge_pp?,tunable,params}, pair, pool_address, source,
//   data:{from,to,bars,trading_from,warmup_bars,forming_excluded,from_param?,
//   to_param?,target_days,complete,note?},
//   fee:{fee_bps_per_side,deploy_pct,fill,long_only,one_position,benchmark},
//   result:{total_return_pct,buy_hold_return_pct,edge_pp,max_drawdown_pct,
//   trades,wins,win_rate_pct,avg_bars_held,equity_curve:[[t,equity]...],
//   trade_list:[{entry_t,entry_price,exit_t,exit_price,bars_held,pnl_pct,
//   exit_reason}]}, cached, disclaimer}
#[path = "../src/raydium_common.rs"] mod common;
#[path = "../src/ohlcv_source.rs"] mod ohlcv_source;
#[path = "../src/schemes/indicators/mod.rs"] mod indicators;
#[path = "../src/schemes/strategies/mod.rs"] mod strategies;
#[path = "../src/strategy_engine.rs"] mod strategy_engine;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use strategy_engine::{build_scheme, run_engine, SchemeQuery, SCHEME_IDS};
use vercel_runtime::{run, service_fn, Error, Request};

const CACHE_TTL: Duration = Duration::from_secs(600);
const DISCLAIMER: &str = "Backtests run on historical daily bars under house-rule accounting (fill-at-close, long-only, one position, fee and deploy assumptions as stamped). Playbook edges are historical measurements on Kraken daily data under the same house rules — not promises. Every scheme is a fresh experiment on the selected data; edges measured on one asset do not transfer to another. Not financial advice.";

static CACHE: OnceLock<Mutex<HashMap<String, (Instant, Value)>>> = OnceLock::new();
fn cache() -> &'static Mutex<HashMap<String, (Instant, Value)>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Deserialize, Default)]
struct Query {
    scheme: Option<String>,
    pair: Option<String>, pool: Option<String>,
    timeframe: Option<String>, target_days: Option<u32>,
    fee_bps: Option<f64>, deploy_pct: Option<f64>,
    from: Option<u64>, to: Option<u64>,
    atr_period: Option<usize>, mult: Option<f64>, max_hold: Option<usize>,
    period: Option<usize>,
}

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
            "scheme" => q.scheme = Some(v),
            "pair" => q.pair = Some(v),
            "pool" => q.pool = Some(v),
            "timeframe" => q.timeframe = Some(v),
            "target_days" => q.target_days = v.parse::<u32>().ok(),
            "fee_bps" => q.fee_bps = v.parse::<f64>().ok(),
            "deploy_pct" => q.deploy_pct = v.parse::<f64>().ok(),
            "from" => q.from = v.parse::<u64>().ok(),
            "to" => q.to = v.parse::<u64>().ok(),
            "atr_period" => q.atr_period = v.parse::<usize>().ok(),
            "mult" => q.mult = v.parse::<f64>().ok(),
            "max_hold" => q.max_hold = v.parse::<usize>().ok(),
            "period" => q.period = v.parse::<usize>().ok(),
            _ => {}
        }
    }
    q
}

// ---- N7.5 (A39): immutable run identifier. ----
// The run ID is a pure function of everything that determines the result
// (scheme, source, params, range, fee, deploy, target days, and the actual
// data window the engine ran on), so an identical experiment on identical
// data re-audits to the identical ID. run_at stamps when the result was
// computed (UTC). Cached hits keep the original run's stamp — the ID is
// still what a fresh run would produce.
fn fnv1a64(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// Epoch seconds → ISO-8601 UTC ("2026-10-24T04:20:00Z"). Hand-rolled so the
// backtest binary needs no date crate (Howard Hinnant's days→civil date).
fn epoch_to_iso(ts: u64) -> String {
    let days = (ts / 86400) as i64;
    let secs = ts % 86400;
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, m, d, secs / 3600, (secs % 3600) / 60, secs % 60)
}

#[tokio::main]
async fn main() -> Result<(), Error> { run(service_fn(handler)).await }

async fn handler(req: Request) -> Result<Value, Error> {
    let q = parse_query(&req);
    let tf = q.timeframe.as_deref().unwrap_or("day");
    if tf != "day" {
        return Ok(common::api_error("INVALID_TIMEFRAME", "Only timeframe=day is supported in v1."));
    }
    let raw_id = match q.scheme.as_deref() {
        Some(s) if !s.trim().is_empty() => s.trim().to_ascii_lowercase().replace('_', "-"),
        _ => return Ok(json!({
            "ok": false, "error": "MISSING_PARAM",
            "message": "Provide ?scheme=<id>.",
            "schemes": SCHEME_IDS,
        })),
    };
    let target_days = q.target_days.unwrap_or(ohlcv_source::DEFAULT_TARGET_DAYS).clamp(1, 2000);
    let fee_bps = q.fee_bps.unwrap_or(26.0).clamp(0.0, 1000.0);
    let deploy_pct = q.deploy_pct.unwrap_or(95.0).clamp(1.0, 100.0);

    let sq = SchemeQuery {
        atr_period: q.atr_period, mult: q.mult,
        max_hold: q.max_hold, period: q.period,
    };
    let mut built = match build_scheme(&raw_id, &sq) {
        Ok(b) => b,
        Err(e) => return Ok(e),
    };

    let source = match ohlcv_source::resolve_source(q.pair.as_deref(), q.pool.as_deref()) {
        Ok(s) => s,
        Err(e) => return Ok(e),
    };
    let (pair_label, pool_addr, src_name) = ohlcv_source::source_meta(&source);

    // The cache key covers everything that affects the output.
    let cache_key = format!("bt|{}|{}|{}|{:?}|{:?}|{}|{}|{}",
        built.def.id,
        ohlcv_source::cache_key(&source, 1, target_days),
        serde_json::to_string(&built.params).unwrap_or_default(),
        q.from, q.to, fee_bps, deploy_pct, target_days);
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
    let (bars_fetched, note) = match ohlcv_source::fetch_bars(&client, &source, 1, target_days).await {
        Ok(x) => x,
        Err(e) => return Ok(e),
    };
    let fetched = bars_fetched.len();
    let complete = fetched as u32 >= target_days;

    // The forming (today's) bar is not a completed daily bar — exclude it from
    // both signal evaluation and equity stamping.
    let now_secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let today_start = now_secs - (now_secs % 86400);
    let mut bars_all = bars_fetched;
    let forming_t: Option<u64> = match bars_all.last() {
        Some(b) if b.0 == today_start => bars_all.pop().map(|b| b.0),
        _ => None,
    };

    // Optional date-range slice, applied before warmup.
    if let Some(from) = q.from { bars_all.retain(|b| b.0 >= from); }
    if let Some(to) = q.to { bars_all.retain(|b| b.0 <= to); }
    let n = bars_all.len();
    if n < built.warmup_bars + 2 {
        return Ok(common::api_error("INSUFFICIENT_DATA", &format!(
            "{} bars in range; scheme '{}' needs warmup_bars={} plus trading room.",
            n, built.def.id, built.warmup_bars)));
    }

    // ---- Engine: the shared house-rule loop (src/strategy_engine.rs). ----
    // Backtest semantics: indicators fed from bar 0, trading starts after the
    // scheme's warmup, open positions force-closed at the last bar
    // (mark_open=false — exit_reason "end_of_data").
    let warmup = built.warmup_bars;
    let res = run_engine(&mut built, &bars_all, warmup, false, fee_bps, deploy_pct);

    // N7.5 (A39) — immutable run identifier, computed AFTER the data window is
    // known so the ID binds to the exact bars the engine ran on. (Placed before
    // scheme_map because built.params is moved into the response below.)
    let run_identity = format!("bt|{}|{}|{}|{:?}|{:?}|{}|{}|{}|{}|{}|{}",
        built.def.id,
        ohlcv_source::cache_key(&source, 1, target_days),
        serde_json::to_string(&built.params).unwrap_or_default(),
        q.from, q.to, fee_bps, deploy_pct, target_days,
        bars_all.first().map(|b| b.0).unwrap_or(0),
        bars_all.last().map(|b| b.0).unwrap_or(0),
        n);
    let run_id = format!("bt-{}-{:012x}", built.def.id, fnv1a64(&run_identity) & 0xFFFFFFFFFFFF);
    let run_epoch = now_secs;
    let run_at = epoch_to_iso(run_epoch);

    let mut scheme_map = serde_json::Map::with_capacity(8);
    scheme_map.insert("id".to_string(), json!(built.def.id));
    scheme_map.insert("name".to_string(), json!(built.name));
    scheme_map.insert("playbook_no".to_string(), json!(built.def.playbook_no));
    scheme_map.insert("playbook_asset".to_string(), json!(built.def.playbook_asset));
    if let Some(e) = built.def.playbook_edge_pp {
        scheme_map.insert("playbook_edge_pp".to_string(), json!(e));
    }
    scheme_map.insert("tunable".to_string(), json!(built.def.tunable));
    scheme_map.insert("params".to_string(), built.params);

    let mut data_map = serde_json::Map::with_capacity(11);
    data_map.insert("from".to_string(), json!(bars_all.first().map(|b| b.0).unwrap_or(0)));
    data_map.insert("to".to_string(), json!(bars_all.last().map(|b| b.0).unwrap_or(0)));
    data_map.insert("bars".to_string(), json!(n));
    data_map.insert("trading_from".to_string(), json!(bars_all[warmup].0));
    data_map.insert("warmup_bars".to_string(), json!(warmup));
    data_map.insert("forming_excluded".to_string(), json!(forming_t));
    if let Some(f) = q.from { data_map.insert("from_param".to_string(), json!(f)); }
    if let Some(t) = q.to { data_map.insert("to_param".to_string(), json!(t)); }
    data_map.insert("target_days".to_string(), json!(target_days));
    data_map.insert("complete".to_string(), json!(complete));
    if let Some(nt) = note { data_map.insert("note".to_string(), json!(nt)); }

    let resp = json!({
        "ok": true,
        "run_id": run_id,
        "run_at": run_at,
        "run_epoch": run_epoch,
        "scheme": Value::Object(scheme_map),
        "pair": pair_label,
        "pool_address": pool_addr,
        "source": src_name,
        "data": Value::Object(data_map),
        "fee": {
            "fee_bps_per_side": fee_bps,
            "deploy_pct": deploy_pct,
            "fill": "close",
            "long_only": true,
            "one_position": true,
            "benchmark": "buy-and-hold uses identical deploy and fee assumptions over the same trading window",
        },
        "result": {
            "total_return_pct": res.total_return_pct,
            "buy_hold_return_pct": res.buy_hold_return_pct,
            "edge_pp": res.edge_pp,
            "max_drawdown_pct": res.max_drawdown_pct,
            "trades": res.trades,
            "wins": res.wins,
            "win_rate_pct": res.win_rate_pct,
            "avg_bars_held": res.avg_bars_held,
            "equity_curve": res.equity_curve,
            "trade_list": res.trade_list.iter().map(|t| t.to_json()).collect::<Vec<Value>>(),
        },
        "cached": false,
        "disclaimer": DISCLAIMER,
    });
    if let Ok(mut guard) = cache().lock() {
        guard.insert(cache_key, (Instant::now(), resp.clone()));
    }
    Ok(resp)
}

#[cfg(test)]
mod n75_runid_tests {
    use super::*;

    #[test]
    fn epoch_to_iso_epoch_is_1970() {
        assert_eq!(epoch_to_iso(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn epoch_to_iso_known_date() {
        // 2026-10-24T00:00:00Z — verified against `date -u`.
        assert_eq!(epoch_to_iso(1792800000), "2026-10-24T00:00:00Z");
        assert_eq!(epoch_to_iso(1792800061), "2026-10-24T00:01:01Z");
    }

    #[test]
    fn epoch_to_iso_leap_day() {
        // 2024-02-29T12:00:00Z (leap year) — the date math must not drift.
        assert_eq!(epoch_to_iso(1709208000), "2024-02-29T12:00:00Z");
    }

    #[test]
    fn fnv1a64_deterministic_and_sensitive() {
        let a = fnv1a64("bt|supertrend-cross|sol-usd-1-730|{\"atr_period\":10}|None|None|26|95|730");
        let b = fnv1a64("bt|supertrend-cross|sol-usd-1-730|{\"atr_period\":10}|None|None|26|95|730");
        assert_eq!(a, b);
        assert_ne!(a, fnv1a64("bt|supertrend-cross|sol-usd-1-730|{\"atr_period\":11}|None|None|26|95|730"));
        assert_ne!(a, fnv1a64(""));
    }

    #[test]
    fn run_id_shape() {
        // Mirrors the format! in the handler: bt-<scheme>-<12 lowercase hex>.
        let id = format!("bt-{}-{:012x}", "supertrend-cross", fnv1a64("x") & 0xFFFFFFFFFFFF);
        assert!(id.starts_with("bt-supertrend-cross-"));
        let hex: String = id.rsplit('-').next().unwrap().to_string();
        assert_eq!(hex.len(), 12);
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }
}
