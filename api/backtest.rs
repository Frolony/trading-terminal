// N7.2 backtest engine — server-side strategy backtesting over the N7.1 candle store.
//
// A Rust service in the terminal's API with Carlos's uploaded `jupiter-strategies`
// crate integrated as its scheme library (vendored under src/schemes/: the
// indicators + strategies modules only — the crate's jupiter.rs execution client
// and main.rs runner are NOT vendored: they would pull solana-sdk/solana-client
// and other heavy deps into every serverless function build. They remain at
// hidden_files/strategy-upload/jupiter-strategies/ as the N7.4 starting point).
//
// This file is NEW code (not part of the production reconstruction): it is the
// "executor" the crate's README leaves to the implementer — the crate only logs
// signals over synthetic bars; this engine runs the Strategy trait over real
// N7.1 bars with Playbook house-rule accounting.
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
// Response: {ok, scheme:{id,name,playbook_no,playbook_asset,playbook_edge_pp?,
//   tunable,params}, pair, pool_address, source,
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
use indicators::Bar;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use strategies::{AroonTimeStop, DriftGatedBollinger, Ema20_50, KamaCross, Strategy, SupertrendTimeStop};
use vercel_runtime::{run, service_fn, Error, Request};

const CACHE_TTL: Duration = Duration::from_secs(600);
const DISCLAIMER: &str = "Backtests run on historical daily bars under house-rule accounting (fill-at-close, long-only, one position, fee and deploy assumptions as stamped). Playbook edges are historical measurements on Kraken daily data under the same house rules — not promises. Every scheme is a fresh experiment on the selected data; edges measured on one asset do not transfer to another. Not financial advice.";

/// The 10 Playbook schemes, in Playbook order. #3 and #10 share parameters
/// (Supertrend(7,2)); the Playbook measured #3 on SOL and #10 on XRP — the
/// `pair` parameter selects the data, so the distinction lives there.
const SCHEME_IDS: &[&str] = &[
    "supertrend-10-3-25",
    "aroon-25-10",
    "supertrend-7-2",
    "bollinger-drift-20-2",
    "aroon-25-40",
    "aroon-25",
    "ema-20-50",
    "aroon-40",
    "kama-10-2-30",
    "supertrend-7-2-xrp",
];

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

struct SchemeDef {
    id: &'static str,
    playbook_no: u8,
    playbook_asset: &'static str,
    playbook_edge_pp: Option<f64>,
    tunable: &'static str,
}

struct BuiltScheme {
    strategy: Box<dyn Strategy>,
    name: String,
    def: SchemeDef,
    warmup_bars: usize,
    time_stop_at: Option<usize>,
    params: Value,
}

fn unknown_scheme(id: &str) -> Value {
    json!({
        "ok": false,
        "error": "UNKNOWN_SCHEME",
        "message": format!("Unknown scheme '{}'.", id),
        "schemes": SCHEME_IDS,
    })
}

fn build_scheme(id: &str, q: &Query) -> Result<BuiltScheme, Value> {
    let no = |name: &str| common::api_error("INVALID_PARAM", &format!("'{}' is not tunable for this scheme.", name));
    match id {
        "supertrend-10-3-25" | "supertrend-7-2" | "supertrend-7-2-xrp" => {
            if q.period.is_some() { return Err(no("period")); }
            let (d_atr, d_mult, d_hold): (usize, f64, usize) = match id {
                "supertrend-10-3-25" => (10, 3.0, 25),
                // #3/#10 are the pure Supertrend(7,2): no Playbook time-stop, so the
                // default hold is effectively infinite over daily data (tunable down).
                _ => (7, 2.0, 10_000),
            };
            let atr = q.atr_period.unwrap_or(d_atr).clamp(2, 50);
            let mult = q.mult.unwrap_or(d_mult).clamp(0.5, 10.0);
            let hold = q.max_hold.unwrap_or(d_hold).clamp(1, 10_000);
            let (pno, asset, edge, sid): (u8, &'static str, Option<f64>, &'static str) = match id {
                "supertrend-10-3-25" => (1, "XRP", Some(87.8), "supertrend-10-3-25"),
                "supertrend-7-2" => (3, "SOL", None, "supertrend-7-2"),
                _ => (10, "XRP", None, "supertrend-7-2-xrp"),
            };
            let strategy = SupertrendTimeStop::new(atr, mult, hold);
            let name = strategy.name().to_string();
            Ok(BuiltScheme {
                strategy: Box::new(strategy), name,
                def: SchemeDef { id: sid, playbook_no: pno, playbook_asset: asset, playbook_edge_pp: edge, tunable: "atr_period, mult, max_hold" },
                warmup_bars: atr,
                time_stop_at: Some(hold),
                params: json!({"atr_period": atr, "mult": mult, "max_hold": hold}),
            })
        }
        "aroon-25-10" | "aroon-25-40" | "aroon-25" | "aroon-40" => {
            if q.atr_period.is_some() { return Err(no("atr_period")); }
            if q.mult.is_some() { return Err(no("mult")); }
            let d_period: usize = match id {
                "aroon-40" => 40,
                _ => 25,
            };
            let period = q.period.unwrap_or(d_period).clamp(2, 200);
            let hold: Option<usize> = match id {
                "aroon-25-10" => Some(q.max_hold.unwrap_or(10).clamp(1, 10_000)),
                "aroon-25-40" => Some(q.max_hold.unwrap_or(40).clamp(1, 10_000)),
                _ => {
                    if q.max_hold.is_some() {
                        return Err(common::api_error("INVALID_PARAM", "'max_hold' is not tunable on the pure trend-state variant — use aroon-25-10 or aroon-25-40 for a time-stop overlay."));
                    }
                    None
                }
            };
            let (pno, asset, sid): (u8, &'static str, &'static str) = match id {
                "aroon-25-10" => (2, "SOL", "aroon-25-10"),
                "aroon-25-40" => (5, "ETH", "aroon-25-40"),
                "aroon-25" => (6, "ETH", "aroon-25"),
                _ => (8, "ETH", "aroon-40"),
            };
            let edge = if id == "aroon-25-10" { Some(83.1) } else { None };
            let strategy = AroonTimeStop::new(period, hold);
            let name = strategy.name().to_string();
            Ok(BuiltScheme {
                strategy: Box::new(strategy), name,
                def: SchemeDef { id: sid, playbook_no: pno, playbook_asset: asset, playbook_edge_pp: edge, tunable: "period, max_hold (time-stop variants only)" },
                warmup_bars: period,
                time_stop_at: hold,
                params: json!({"period": period, "max_hold": hold}),
            })
        }
        "bollinger-drift-20-2" => {
            if q.atr_period.is_some() { return Err(no("atr_period")); }
            if q.mult.is_some() { return Err(no("mult")); }
            if q.max_hold.is_some() { return Err(no("max_hold")); }
            if q.period.is_some() { return Err(no("period")); }
            let strategy = DriftGatedBollinger::new();
            let name = strategy.name().to_string();
            Ok(BuiltScheme {
                strategy: Box::new(strategy), name,
                def: SchemeDef { id: "bollinger-drift-20-2", playbook_no: 4, playbook_asset: "SOL", playbook_edge_pp: None, tunable: "none — fixed Playbook parameters (20, 2, ±25% SMA200 drift gate)" },
                // The drift gate reads a 200-day SMA: the dominant lookback.
                warmup_bars: 200,
                time_stop_at: None,
                params: json!({}),
            })
        }
        "ema-20-50" => {
            if q.atr_period.is_some() { return Err(no("atr_period")); }
            if q.mult.is_some() { return Err(no("mult")); }
            if q.max_hold.is_some() { return Err(no("max_hold")); }
            if q.period.is_some() { return Err(no("period")); }
            let strategy = Ema20_50::new();
            let name = strategy.name().to_string();
            Ok(BuiltScheme {
                strategy: Box::new(strategy), name,
                def: SchemeDef { id: "ema-20-50", playbook_no: 7, playbook_asset: "ETH", playbook_edge_pp: None, tunable: "none — fixed Playbook parameters (20/50)" },
                warmup_bars: 50,
                time_stop_at: None,
                params: json!({}),
            })
        }
        "kama-10-2-30" => {
            if q.atr_period.is_some() { return Err(no("atr_period")); }
            if q.mult.is_some() { return Err(no("mult")); }
            if q.max_hold.is_some() { return Err(no("max_hold")); }
            if q.period.is_some() { return Err(no("period")); }
            let strategy = KamaCross::new();
            let name = strategy.name().to_string();
            Ok(BuiltScheme {
                strategy: Box::new(strategy), name,
                def: SchemeDef { id: "kama-10-2-30", playbook_no: 9, playbook_asset: "SOL", playbook_edge_pp: None, tunable: "none — fixed Playbook parameters (10, 2, 30)" },
                warmup_bars: 30,
                time_stop_at: None,
                params: json!({}),
            })
        }
        _ => Err(unknown_scheme(id)),
    }
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

    let built = match build_scheme(&raw_id, &q) {
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

    // ---- Engine: pure function of (scheme, params, bars, fee, deploy). ----
    let fee = fee_bps / 10000.0;
    let deploy = deploy_pct / 100.0;
    let mut strategy = built.strategy;
    let mut equity = 1.0f64;
    let mut cash = 1.0f64;
    let mut qty = 0.0f64;
    let mut entry_notional = 0.0f64;
    let mut entry_price = 0.0f64;
    let mut entry_t = 0u64;
    let mut bars_held = 0usize;
    let mut in_position = false;
    let mut trades: Vec<Value> = Vec::new();
    let mut trade_count = 0usize;
    let mut wins = 0usize;
    let mut held_sum = 0usize;
    let mut curve: Vec<Value> = Vec::with_capacity(n);
    let mut peak = 1.0f64;
    let mut max_dd = 0.0f64;

    for (i, bar) in bars_all.iter().enumerate() {
        let (t, _o, h, l, c, _v) = *bar;
        strategy.on_bar(&Bar::new(h, l, c));
        if i < built.warmup_bars { continue; }
        if in_position {
            bars_held += 1;
            if strategy.should_exit(bars_held) {
                let proceeds = qty * c * (1.0 - fee);
                let pnl_pct = (proceeds - entry_notional) / entry_notional * 100.0;
                equity = cash + proceeds;
                // When the time-stop and the signal flip fire on the same bar,
                // classification reports the time-stop.
                let reason = match built.time_stop_at {
                    Some(m) if bars_held >= m => "time_stop",
                    _ => "signal",
                };
                trade_count += 1;
                held_sum += bars_held;
                if pnl_pct > 0.0 { wins += 1; }
                trades.push(json!({
                    "entry_t": entry_t, "entry_price": entry_price,
                    "exit_t": t, "exit_price": c,
                    "bars_held": bars_held, "pnl_pct": pnl_pct,
                    "exit_reason": reason,
                }));
                in_position = false;
                qty = 0.0;
            } else {
                equity = cash + qty * c;
            }
        } else if i + 1 < n && strategy.should_enter() {
            // No entry on the final bar — there would be no bar left to exit on.
            let notional = equity * deploy;
            let cost = notional * fee;
            qty = (notional - cost) / c;
            entry_notional = notional;
            entry_price = c;
            entry_t = t;
            cash = equity - notional;
            bars_held = 0;
            in_position = true;
            equity = cash + qty * c;
        }
        curve.push(json!([t, equity]));
        if equity > peak { peak = equity; }
        let dd = (peak - equity) / peak;
        if dd > max_dd { max_dd = dd; }
    }
    if in_position {
        // Force-close at the last bar's close; restate the curve's last point.
        let (t, _o, _h, _l, c, _v) = bars_all[n - 1];
        let proceeds = qty * c * (1.0 - fee);
        let pnl_pct = (proceeds - entry_notional) / entry_notional * 100.0;
        equity = cash + proceeds;
        trade_count += 1;
        held_sum += bars_held;
        if pnl_pct > 0.0 { wins += 1; }
        trades.push(json!({
            "entry_t": entry_t, "entry_price": entry_price,
            "exit_t": t, "exit_price": c,
            "bars_held": bars_held, "pnl_pct": pnl_pct,
            "exit_reason": "end_of_data",
        }));
        if let Some(pt) = curve.last_mut() { *pt = json!([t, equity]); }
    }

    // Buy-and-hold benchmark over the same trading window, identical deploy/fee.
    let bh_entry = bars_all[built.warmup_bars].4;
    let bh_last = bars_all[n - 1].4;
    let bh_qty = deploy * (1.0 - fee) / bh_entry;
    let bh_equity = (1.0 - deploy) + bh_qty * bh_last * (1.0 - fee);
    let bh_ret = (bh_equity - 1.0) * 100.0;
    let total_ret = (equity - 1.0) * 100.0;

    let win_rate = if trade_count > 0 { wins as f64 / trade_count as f64 * 100.0 } else { 0.0 };
    let avg_held = if trade_count > 0 { held_sum as f64 / trade_count as f64 } else { 0.0 };

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
    data_map.insert("trading_from".to_string(), json!(bars_all[built.warmup_bars].0));
    data_map.insert("warmup_bars".to_string(), json!(built.warmup_bars));
    data_map.insert("forming_excluded".to_string(), json!(forming_t));
    if let Some(f) = q.from { data_map.insert("from_param".to_string(), json!(f)); }
    if let Some(t) = q.to { data_map.insert("to_param".to_string(), json!(t)); }
    data_map.insert("target_days".to_string(), json!(target_days));
    data_map.insert("complete".to_string(), json!(complete));
    if let Some(nt) = note { data_map.insert("note".to_string(), json!(nt)); }

    let resp = json!({
        "ok": true,
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
            "total_return_pct": total_ret,
            "buy_hold_return_pct": bh_ret,
            "edge_pp": total_ret - bh_ret,
            "max_drawdown_pct": max_dd * 100.0,
            "trades": trade_count,
            "wins": wins,
            "win_rate_pct": win_rate,
            "avg_bars_held": avg_held,
            "equity_curve": curve,
            "trade_list": trades,
        },
        "cached": false,
        "disclaimer": DISCLAIMER,
    });
    if let Ok(mut guard) = cache().lock() {
        guard.insert(cache_key, (Instant::now(), resp.clone()));
    }
    Ok(resp)
}
