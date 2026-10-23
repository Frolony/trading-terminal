// N7.4 paper-live deploy path — server-side track record for a deployed config.
//
// A Rust service in the terminal's API. This file is NEW code (not part of the
// production reconstruction). It replays the shared strategy engine
// (src/strategy_engine.rs — the N7.2 loop, byte-identical math) over live
// daily bars from a deployment date forward, with SIMULATED fills. No wallet,
// no signing, no chain interaction: paper-live is a simulation record, and
// every surface that renders it must carry the PAPER-LIVE chip and the
// "simulated fills, no real funds" line.
//
// The endpoint is stateless and deterministic: a pure function of
// (scheme, params, pair, fee_bps, deploy_pct, deployed_at, bars). Deployment
// records live client-side (localStorage, identity-namespaced like the N7.3
// saved configs); the browser re-evaluates on visit/refresh. A scheduled
// server-side runner is a documented future upgrade (needs a durable store —
// Carlos's approval), not v1.
//
// Deployment semantics:
// - deployed_at (epoch secs, REQUIRED) = wall-clock moment of deployment.
// - Bars are daily with t = UTC-day-start. A deployment at time T first trades
//   the first bar with t >= floor(T/86400)*86400 + 86400 (the next UTC day's
//   bar, filled at that bar's close). The forming day's bar is never traded —
//   no lookahead, the same conservative posture as N7.2's forming exclusion.
// - deployed_at in the future -> DEPLOYED_IN_FUTURE.
// - No completed bar after the deploy day yet -> status "pending" (200): the
//   deployment is tracked but unevaluable; the first evaluable bar closes
//   <first_tradable_bar_t>.
// - The scheme needs warmup_bars of completed history before its first tradable
//   bar: too-early deployments -> DEPLOYED_BEFORE_WARMUP; deployments older
//   than the fetched window -> DEPLOYED_BEFORE_DATA.
// - Paper-live deltas vs the backtest (the ONLY deltas — the loop is shared):
//   indicators seeded from the warmup_bars preceding the first tradable bar,
//   and an open position at the last completed bar is MARKED at realizable
//   value (qty * close * (1 - fee)) instead of force-closed. Realizable equity
//   is therefore exactly what the backtest's force-close would have produced;
//   the closed-trade list matches the equivalent backtest's list minus its
//   final "end_of_data" trade.
//
// Contract: GET /api/paperlive?scheme=<id>&pair=sol-usd&deployed_at=<epoch>
//   Required: scheme=<one of the 10 ids>, deployed_at=<unix epoch seconds>.
//   Optional: pair=<slug> | pool=<address> (default pair=sol-usd),
//     timeframe=day (only "day" in v1), target_days=730 (1..=2000),
//     fee_bps=26 (0..=1000), deploy_pct=95 (1..=100),
//     scheme params per the N7.2 strict contract (INVALID_PARAM on non-tunable).
// Response (status "tracking"): {ok, status, scheme:{...same as /api/backtest},
//   pair, pool_address, source,
//   deployment:{deployed_at,deployed_day_t,first_tradable_bar_t,bars_live,seeded_bars},
//   data:{from,to,bars,trading_from,warmup_bars,forming_excluded,complete,note?},
//   fee:{fee_bps_per_side,deploy_pct,fill,long_only,one_position,benchmark},
//   result:{...same as /api/backtest's result, plus
//     open_position:{entry_t,entry_price,bars_held,unrealized_pct}|null,
//     current_equity},
//   backtest_equivalent:{from,to,url,note}, cached, disclaimer}
// Response (status "pending"): {ok, status:"pending", scheme:{...},
//   deployment:{deployed_at,deployed_day_t,first_tradable_bar_t,
//   note:"No completed daily bar after the deployment yet."}}
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
const DISCLAIMER: &str = "Backtests run on historical daily bars under house-rule accounting (fill-at-close, long-only, one position, fee and deploy assumptions as stamped). Playbook edges are historical measurements on Kraken daily data under the same house rules — not promises. Every scheme is a fresh experiment on the selected data; edges measured on one asset do not transfer to another. Paper-live replays the same engine over bars that arrived after deployment with simulated fills at bar closes. It is a simulation record on this device, not trading. Not financial advice.";

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
    deployed_at: Option<String>,
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
            "deployed_at" => q.deployed_at = Some(v),
            "atr_period" => q.atr_period = v.parse::<usize>().ok(),
            "mult" => q.mult = v.parse::<f64>().ok(),
            "max_hold" => q.max_hold = v.parse::<usize>().ok(),
            "period" => q.period = v.parse::<usize>().ok(),
            _ => {}
        }
    }
    q
}

fn scheme_json(built: &strategy_engine::BuiltScheme) -> Value {
    let mut m = serde_json::Map::with_capacity(8);
    m.insert("id".to_string(), json!(built.def.id));
    m.insert("name".to_string(), json!(built.name));
    m.insert("playbook_no".to_string(), json!(built.def.playbook_no));
    m.insert("playbook_asset".to_string(), json!(built.def.playbook_asset));
    if let Some(e) = built.def.playbook_edge_pp {
        m.insert("playbook_edge_pp".to_string(), json!(e));
    }
    m.insert("tunable".to_string(), json!(built.def.tunable));
    m.insert("params".to_string(), built.params.clone());
    Value::Object(m)
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
    let deployed_at: u64 = match q.deployed_at.as_deref() {
        Some(s) => match s.trim().parse::<u64>() {
            Ok(v) => v,
            _ => return Ok(common::api_error("INVALID_PARAM",
                "deployed_at must be unix epoch seconds, e.g. ?deployed_at=1760000000.")),
        },
        None => return Ok(common::api_error("MISSING_PARAM",
            "Provide ?deployed_at=<unix epoch seconds> (the wall-clock moment of deployment).")),
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
    let cache_key = format!("pl|{}|{}|{:?}|{}|{}|{}|{}",
        built.def.id,
        ohlcv_source::cache_key(&source, 1, target_days),
        serde_json::to_string(&built.params).unwrap_or_default(),
        deployed_at, fee_bps, deploy_pct, target_days);
    if let Ok(guard) = cache().lock() {
        if let Some((at, val)) = guard.get(&cache_key) {
            if at.elapsed() < CACHE_TTL {
                let mut hit = val.clone();
                if let Some(obj) = hit.as_object_mut() { obj.insert("cached".to_string(), json!(true)); }
                return Ok(hit);
            }
        }
    }

    let now_secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    if deployed_at > now_secs {
        return Ok(common::api_error("DEPLOYED_IN_FUTURE",
            "deployed_at is in the future — a deployment can only track bars that already exist."));
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
    let today_start = now_secs - (now_secs % 86400);
    let mut bars_all = bars_fetched;
    let forming_t: Option<u64> = match bars_all.last() {
        Some(b) if b.0 == today_start => bars_all.pop().map(|b| b.0),
        _ => None,
    };
    let n = bars_all.len();

    // Deployment semantics: the first tradable bar is the first completed bar
    // of the UTC day AFTER the deployment day. Deploying mid-day never trades
    // the forming day's bar (no lookahead).
    let deployed_day = deployed_at - (deployed_at % 86400);
    let first_tradable = deployed_day + 86400;
    let deploy_idx = match bars_all.iter().position(|b| b.0 >= first_tradable) {
        Some(i) => i,
        None => {
            // No completed bar after the deploy day yet: tracked, not evaluable.
            let resp = json!({
                "ok": true,
                "status": "pending",
                "scheme": scheme_json(&built),
                "pair": pair_label,
                "pool_address": pool_addr,
                "source": src_name,
                "deployment": {
                    "deployed_at": deployed_at,
                    "deployed_day_t": deployed_day,
                    "first_tradable_bar_t": first_tradable,
                    "note": "No completed daily bar after the deployment yet — the first evaluable bar closes at the end of its UTC day. No trades simulated.",
                },
                "cached": false,
                "disclaimer": DISCLAIMER,
            });
            if let Ok(mut guard) = cache().lock() {
                guard.insert(cache_key, (Instant::now(), resp.clone()));
            }
            return Ok(resp);
        }
    };

    let warmup = built.warmup_bars;
    if n < warmup + 2 {
        return Ok(common::api_error("INSUFFICIENT_DATA", &format!(
            "{} completed bars fetched; scheme '{}' needs warmup_bars={} plus trading room.",
            n, built.def.id, warmup)));
    }
    if deploy_idx < warmup {
        if deploy_idx == 0 {
            // The first bar already satisfies t >= first_tradable, so the
            // deployment day predates the fetched window entirely.
            return Ok(common::api_error("DEPLOYED_BEFORE_DATA", &format!(
                "The deployment predates the fetched window (oldest completed bar is epoch {}). Raise target_days (max 2000) or deploy later.",
                bars_all[0].0)));
        }
        // Latest deployable moment with this data: the deployment day must end
        // at or before the warmup-th bar's day-start.
        let latest = bars_all[warmup].0.saturating_sub(1);
        return Ok(common::api_error("DEPLOYED_BEFORE_WARMUP", &format!(
            "Scheme '{}' needs {} completed bars of seeding history before its first tradable bar; with this data the latest deployable moment is epoch {} (deploy during an earlier UTC day, or raise target_days).",
            built.def.id, warmup, latest)));
    }

    // ---- Engine: the shared house-rule loop (src/strategy_engine.rs). ----
    // Paper-live semantics: indicators seeded from the warmup_bars preceding
    // the first tradable bar; trading starts at the first tradable bar; an
    // open position at the last completed bar is marked at realizable value
    // (mark_open=true), never force-closed.
    let feed = &bars_all[deploy_idx - warmup..];
    let res = run_engine(&mut built, feed, warmup, true, fee_bps, deploy_pct);

    let mut data_map = serde_json::Map::with_capacity(10);
    data_map.insert("from".to_string(), json!(feed.first().map(|b| b.0).unwrap_or(0)));
    data_map.insert("to".to_string(), json!(feed.last().map(|b| b.0).unwrap_or(0)));
    data_map.insert("bars".to_string(), json!(feed.len()));
    data_map.insert("trading_from".to_string(), json!(feed[warmup].0));
    data_map.insert("warmup_bars".to_string(), json!(warmup));
    data_map.insert("forming_excluded".to_string(), json!(forming_t));
    data_map.insert("target_days".to_string(), json!(target_days));
    data_map.insert("complete".to_string(), json!(complete));
    if let Some(nt) = note { data_map.insert("note".to_string(), json!(nt)); }

    // The equivalent backtest: from the first SEEDED bar (so its warmup feeds
    // the same bars and its trading starts on the same bar), to the same last
    // completed bar. Closed trades and total return are identical; the only
    // difference is the final-bar handling (force-close vs mark-open).
    let eq_from = feed.first().map(|b| b.0).unwrap_or(0);
    let eq_to = feed.last().map(|b| b.0).unwrap_or(0);
    // sq only carries tunable params for this scheme (non-tunable values were
    // already rejected by build_scheme), so every Some(_) rides along verbatim.
    let mut eq_qs = format!(
        "/api/backtest?scheme={}&from={}&to={}&fee_bps={}&deploy_pct={}",
        built.def.id, eq_from, eq_to, fee_bps, deploy_pct);
    if let Some(v) = sq.atr_period { eq_qs.push_str(&format!("&atr_period={}", v)); }
    if let Some(v) = sq.mult { eq_qs.push_str(&format!("&mult={}", v)); }
    if let Some(v) = sq.max_hold { eq_qs.push_str(&format!("&max_hold={}", v)); }
    if let Some(v) = sq.period { eq_qs.push_str(&format!("&period={}", v)); }
    if let Some(pr) = q.pair.as_deref() { eq_qs.push_str(&format!("&pair={}", pr)); }
    if let Some(pl) = q.pool.as_deref() { eq_qs.push_str(&format!("&pool={}", pl)); }

    let resp = json!({
        "ok": true,
        "status": "tracking",
        "scheme": scheme_json(&built),
        "pair": pair_label,
        "pool_address": pool_addr,
        "source": src_name,
        "deployment": {
            "deployed_at": deployed_at,
            "deployed_day_t": deployed_day,
            "first_tradable_bar_t": first_tradable,
            "bars_live": n - deploy_idx,
            "seeded_bars": warmup,
        },
        "data": Value::Object(data_map),
        "fee": {
            "fee_bps_per_side": fee_bps,
            "deploy_pct": deploy_pct,
            "fill": "close",
            "long_only": true,
            "one_position": true,
            "benchmark": "buy-and-hold over the paper-live window, identical deploy and fee assumptions",
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
            "open_position": res.open_position.as_ref().map(|o| o.to_json()).unwrap_or(Value::Null),
            "current_equity": res.final_equity,
        },
        "backtest_equivalent": {
            "from": eq_from,
            "to": eq_to,
            "url": eq_qs,
            "note": "The same scheme, params, data and window run through /api/backtest: closed trades and total return are identical. The only difference is the final bar — the backtest force-closes (exit_reason end_of_data); paper-live marks the open position at realizable value instead.",
        },
        "cached": false,
        "disclaimer": DISCLAIMER,
    });
    if let Ok(mut guard) = cache().lock() {
        guard.insert(cache_key, (Instant::now(), resp.clone()));
    }
    Ok(resp)
}
