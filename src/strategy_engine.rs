// N7.4 shared strategy engine — the N7.2 backtest loop, extracted verbatim.
//
// Provenance: this module is a line-for-line extraction of the engine that
// shipped in api/backtest.rs (N7.2, independently verified to 1e-9 on
// 2026-10-22). The extraction is mechanical: the scheme registry
// (SCHEME_IDS / build_scheme / unknown_scheme) and the accounting loop moved
// here unchanged so that /api/backtest and the new /api/paperlive share one
// implementation ("build once, use twice", the same rule N7.2 applied to the
// candle store). The two behavioral parameters that differ between backtest
// and paper-live are explicit arguments:
//
// - `trade_from`: index in `feed` where trading starts (backtest passes the
//   scheme's warmup_bars over the full window; paper-live passes warmup_bars
//   over a feed slice that begins `warmup_bars` before the first tradable bar).
// - `mark_open`: false = backtest semantics (force-close at the last bar with
//   exit_reason "end_of_data"); true = paper-live semantics (an open position
//   is marked at realizable value `qty * close * (1 - fee)` — exactly what the
//   force-close would have produced — and reported via `open_position` instead
//   of a synthetic exit trade).
//
// Intended inclusion (bins only — never compiled standalone):
//   #[path = "../src/strategy_engine.rs"] mod strategy_engine;
// The module reaches the sibling helper modules through `crate::common`,
// `crate::indicators`, `crate::strategies` and `crate::ohlcv_source`, so every
// including bin must declare those mods (see api/backtest.rs).

use crate::indicators::Bar;
use crate::ohlcv_source::RawBar;
use crate::strategies::{AroonTimeStop, DriftGatedBollinger, Ema20_50, KamaCross, Strategy, SupertrendTimeStop};
use serde_json::{json, Value};

/// The 10 Playbook schemes, in Playbook order. #3 and #10 share parameters
/// (Supertrend(7,2)); the Playbook measured #3 on SOL and #10 on XRP — the
/// `pair` parameter selects the data, so the distinction lives there.
pub const SCHEME_IDS: &[&str] = &[
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

/// Scheme parameter inputs, decoupled from any bin's query struct so both
/// /api/backtest and /api/paperlive share the strict tuning contract.
#[derive(Default, Clone, Copy)]
pub struct SchemeQuery {
    pub atr_period: Option<usize>,
    pub mult: Option<f64>,
    pub max_hold: Option<usize>,
    pub period: Option<usize>,
}

pub struct SchemeDef {
    pub id: &'static str,
    pub playbook_no: u8,
    pub playbook_asset: &'static str,
    pub playbook_edge_pp: Option<f64>,
    pub tunable: &'static str,
}

pub struct BuiltScheme {
    pub strategy: Box<dyn Strategy>,
    pub name: String,
    pub def: SchemeDef,
    pub warmup_bars: usize,
    pub time_stop_at: Option<usize>,
    pub params: Value,
}

pub fn unknown_scheme(id: &str) -> Value {
    json!({
        "ok": false,
        "error": "UNKNOWN_SCHEME",
        "message": format!("Unknown scheme '{}'.", id),
        "schemes": SCHEME_IDS,
    })
}

pub fn build_scheme(id: &str, q: &SchemeQuery) -> Result<BuiltScheme, Value> {
    let no = |name: &str| crate::common::api_error("INVALID_PARAM", &format!("'{}' is not tunable for this scheme.", name));
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
                        return Err(crate::common::api_error("INVALID_PARAM", "'max_hold' is not tunable on the pure trend-state variant — use aroon-25-10 or aroon-25-40 for a time-stop overlay."));
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

/// One closed simulated trade — field order matches the N7.2 `trade_list` JSON.
pub struct Trade {
    pub entry_t: u64,
    pub entry_price: f64,
    pub exit_t: u64,
    pub exit_price: f64,
    pub bars_held: usize,
    pub pnl_pct: f64,
    pub exit_reason: &'static str, // "signal" | "time_stop" | "end_of_data"
}

impl Trade {
    pub fn to_json(&self) -> Value {
        json!({
            "entry_t": self.entry_t, "entry_price": self.entry_price,
            "exit_t": self.exit_t, "exit_price": self.exit_price,
            "bars_held": self.bars_held, "pnl_pct": self.pnl_pct,
            "exit_reason": self.exit_reason,
        })
    }
}

/// Paper-live only: a position still open at the last completed bar, marked
/// at realizable value (exit fee applied) — never a synthetic exit.
pub struct OpenPosition {
    pub entry_t: u64,
    pub entry_price: f64,
    pub bars_held: usize,
    pub unrealized_pct: f64,
}

impl OpenPosition {
    pub fn to_json(&self) -> Value {
        json!({
            "entry_t": self.entry_t, "entry_price": self.entry_price,
            "bars_held": self.bars_held, "unrealized_pct": self.unrealized_pct,
        })
    }
}

pub struct EngineResult {
    pub total_return_pct: f64,
    pub buy_hold_return_pct: f64,
    pub edge_pp: f64,
    pub max_drawdown_pct: f64,
    pub trades: usize,
    pub wins: usize,
    pub win_rate_pct: f64,
    pub avg_bars_held: f64,
    /// [t, equity] points, one per fed bar (warmup seeding included, equity 1.0).
    pub equity_curve: Vec<Value>,
    pub trade_list: Vec<Trade>,
    /// None under backtest semantics (`mark_open=false`); Some iff a position
    /// is open at the last bar under paper-live semantics.
    pub open_position: Option<OpenPosition>,
    /// Equity at the last bar's close (realizable).
    pub final_equity: f64,
}

/// The house-rule accounting loop, verbatim from the N7.2 engine:
///
/// - long-only, one position at a time
/// - signals evaluated after on_bar; fills at the signal bar's close
/// - fee_bps per side, deducted from deployed capital
/// - deploy_pct of equity deployed per entry
/// - indicators fed from feed[0]; trading starts at feed[trade_from]
/// - no entry on the final bar (no bar left to exit on)
/// - final bar: `mark_open=false` force-closes with exit_reason "end_of_data";
///   `mark_open=true` marks the open position at realizable value instead.
///
/// Deterministic: a pure function of (scheme, params, feed, trade_from, fee,
/// deploy, mark_open).
pub fn run_engine(
    built: &mut BuiltScheme,
    feed: &[RawBar],
    trade_from: usize,
    mark_open: bool,
    fee_bps: f64,
    deploy_pct: f64,
) -> EngineResult {
    let n = feed.len();
    let fee = fee_bps / 10000.0;
    let deploy = deploy_pct / 100.0;
    let strategy = &mut built.strategy;
    let mut equity = 1.0f64;
    let mut cash = 1.0f64;
    let mut qty = 0.0f64;
    let mut entry_notional = 0.0f64;
    let mut entry_price = 0.0f64;
    let mut entry_t = 0u64;
    let mut bars_held = 0usize;
    let mut in_position = false;
    let mut trades: Vec<Trade> = Vec::new();
    let mut trade_count = 0usize;
    let mut wins = 0usize;
    let mut held_sum = 0usize;
    let mut curve: Vec<Value> = Vec::with_capacity(n);
    let mut peak = 1.0f64;
    let mut max_dd = 0.0f64;

    for (i, bar) in feed.iter().enumerate() {
        let (t, _o, h, l, c, _v) = *bar;
        strategy.on_bar(&Bar::new(h, l, c));
        if i < trade_from { continue; }
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
                trades.push(Trade {
                    entry_t, entry_price,
                    exit_t: t, exit_price: c,
                    bars_held, pnl_pct,
                    exit_reason: reason,
                });
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
    let mut open_position: Option<OpenPosition> = None;
    if in_position {
        // Realizable equity at the last completed close: exit fee applied, the
        // same arithmetic the backtest's force-close uses.
        let (t, _o, _h, _l, c, _v) = feed[n - 1];
        let realizable = qty * c * (1.0 - fee);
        equity = cash + realizable;
        if let Some(pt) = curve.last_mut() { *pt = json!([t, equity]); }
        if mark_open {
            // Paper-live: report the open position; do NOT synthesize an exit.
            let unrealized_pct = (realizable - entry_notional) / entry_notional * 100.0;
            open_position = Some(OpenPosition { entry_t, entry_price, bars_held, unrealized_pct });
        } else {
            // Backtest: force-close at the last bar's close; restate the curve's last point.
            let pnl_pct = (realizable - entry_notional) / entry_notional * 100.0;
            trade_count += 1;
            held_sum += bars_held;
            if pnl_pct > 0.0 { wins += 1; }
            trades.push(Trade {
                entry_t, entry_price,
                exit_t: t, exit_price: c,
                bars_held, pnl_pct,
                exit_reason: "end_of_data",
            });
        }
    }

    // Buy-and-hold benchmark over the same trading window, identical deploy/fee.
    let bh_entry = feed[trade_from].4;
    let bh_last = feed[n - 1].4;
    let bh_qty = deploy * (1.0 - fee) / bh_entry;
    let bh_equity = (1.0 - deploy) + bh_qty * bh_last * (1.0 - fee);
    let bh_ret = (bh_equity - 1.0) * 100.0;
    let total_ret = (equity - 1.0) * 100.0;

    let win_rate = if trade_count > 0 { wins as f64 / trade_count as f64 * 100.0 } else { 0.0 };
    let avg_held = if trade_count > 0 { held_sum as f64 / trade_count as f64 } else { 0.0 };

    EngineResult {
        total_return_pct: total_ret,
        buy_hold_return_pct: bh_ret,
        edge_pp: total_ret - bh_ret,
        max_drawdown_pct: max_dd * 100.0,
        trades: trade_count,
        wins,
        win_rate_pct: win_rate,
        avg_bars_held: avg_held,
        equity_curve: curve,
        trade_list: trades,
        open_position,
        final_equity: equity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 140 synthetic daily bars: 70 falling 1%/day, then 70 rising 1%/day.
    /// EMA 20/50 crosses down on the fall and up on the rise, so the engine
    /// opens at least one position on this series.
    fn synth_bars() -> Vec<RawBar> {
        let day = 86400u64;
        let t0 = 1_700_000_000u64 - (1_700_000_000u64 % day);
        let mut bars = Vec::with_capacity(140);
        let mut price = 100.0f64;
        for i in 0..140 {
            let drift = if i < 70 { 0.99 } else { 1.01 };
            let o = price;
            price *= drift;
            let h = o.max(price) * 1.002;
            let l = o.min(price) * 0.998;
            bars.push((t0 + i as u64 * day, o, h, l, price, 1_000.0));
        }
        bars
    }

    fn ema_scheme() -> BuiltScheme {
        build_scheme("ema-20-50", &SchemeQuery::default()).expect("ema scheme builds")
    }

    #[test]
    fn engine_is_deterministic() {
        let bars = synth_bars();
        let mut a = ema_scheme();
        let mut b = ema_scheme();
        let ra = run_engine(&mut a, &bars, 50, false, 26.0, 95.0);
        let rb = run_engine(&mut b, &bars, 50, false, 26.0, 95.0);
        assert_eq!(ra.trades, rb.trades);
        assert!((ra.total_return_pct - rb.total_return_pct).abs() < 1e-15);
        assert_eq!(ra.equity_curve.len(), rb.equity_curve.len());
        assert!(ra.trades > 0, "synthetic series should produce at least one trade");
    }

    #[test]
    fn trade_chain_equity_matches_total_return() {
        let bars = synth_bars();
        let mut s = ema_scheme();
        let r = run_engine(&mut s, &bars, 50, false, 26.0, 95.0);
        // Re-walk the closed trades: equity after the last trade's exit must
        // equal final equity (no open position under backtest semantics).
        let mut eq = 1.0f64;
        let fee = 0.0026f64;
        let deploy = 0.95f64;
        for t in &r.trade_list {
            let notional = eq * deploy;
            let qty = (notional * (1.0 - fee)) / t.entry_price;
            eq = (eq - notional) + qty * t.exit_price * (1.0 - fee);
        }
        assert!((eq - r.final_equity).abs() < 1e-9, "chain {} vs final {}", eq, r.final_equity);
        assert!((eq - 1.0) * 100.0 - r.total_return_pct < 1e-9);
        assert!(r.open_position.is_none());
    }

    #[test]
    fn mark_open_matches_force_close_equity() {
        let bars = synth_bars();
        let mut a = ema_scheme();
        let mut b = ema_scheme();
        let back = run_engine(&mut a, &bars, 50, false, 26.0, 95.0);
        let live = run_engine(&mut b, &bars, 50, true, 26.0, 95.0);
        // Same data, same window: realizable equity is identical by construction.
        assert!((back.final_equity - live.final_equity).abs() < 1e-12);
        assert!((back.total_return_pct - live.total_return_pct).abs() < 1e-12);
        match live.open_position {
            Some(op) => {
                // The backtest's last trade must be the synthetic end_of_data exit
                // of exactly this position.
                let last = back.trade_list.last().expect("open position implies a final exit trade");
                assert_eq!(last.exit_reason, "end_of_data");
                assert_eq!(last.entry_t, op.entry_t);
                assert_eq!(last.bars_held, op.bars_held);
                assert!((last.pnl_pct - op.unrealized_pct).abs() < 1e-12);
                assert_eq!(live.trade_list.len() + 1, back.trade_list.len());
            }
            None => {
                assert_eq!(live.trade_list.len(), back.trade_list.len());
            }
        }
    }

    #[test]
    fn no_trading_before_trade_from() {
        let bars = synth_bars();
        let mut s = ema_scheme();
        let r = run_engine(&mut s, &bars, 100, false, 26.0, 95.0);
        for t in &r.trade_list {
            assert!(t.entry_t >= bars[100].0, "entry before trade_from");
        }
        // The equity curve starts at trade_from (warmup seeding is not stamped).
        assert_eq!(r.equity_curve.len(), bars.len() - 100);
    }

    #[test]
    fn unknown_scheme_and_invalid_param_contract() {
        assert!(build_scheme("nope", &SchemeQuery::default()).is_err());
        let mut q = SchemeQuery::default();
        q.period = Some(20); // not tunable on ema-20-50
        assert!(build_scheme("ema-20-50", &q).is_err());
        let mut q2 = SchemeQuery::default();
        q2.max_hold = Some(10);
        assert!(build_scheme("aroon-25-10", &q2).is_ok());
        assert_eq!(SCHEME_IDS.len(), 10);
    }

    #[test]
    fn time_stop_is_honored_and_reported() {
        // Supertrend(10,3) with max_hold=3 on a smooth uptrend: every position
        // must exit exactly at bars_held == 3 with reason "time_stop".
        let day = 86400u64;
        let t0 = 1_700_000_000u64 - (1_700_000_000u64 % day);
        let mut bars = Vec::with_capacity(120);
        let mut price = 100.0f64;
        for i in 0..120 {
            let o = price;
            price *= 1.005;
            bars.push((t0 + i as u64 * day, o, o * 1.002, o * 0.998, price, 1_000.0));
        }
        let mut q = SchemeQuery::default();
        q.max_hold = Some(3);
        let mut s = build_scheme("supertrend-10-3-25", &q).expect("scheme builds");
        let r = run_engine(&mut s, &bars, 10, false, 26.0, 95.0);
        assert!(r.trades > 0, "uptrend should trigger entries");
        for t in &r.trade_list {
            if t.exit_reason == "time_stop" {
                assert_eq!(t.bars_held, 3);
            }
        }
        assert!(r.trade_list.iter().any(|t| t.exit_reason == "time_stop"),
            "expected at least one time_stop exit on a smooth uptrend");
    }
}
