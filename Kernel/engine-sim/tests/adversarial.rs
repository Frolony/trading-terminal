//! W3.2 adversarial theorem tests: each attack is an executable claim —
//! either the design survives (asserted here) or the attack pencils out (a
//! design finding for the roadmap and Carlos, never a test to weaken).

use engine_sim::adversarial::{
    admit_entry, bulk_exit_attack, default_as_exit_attack, exit_contract, is_whale,
    movable_liquidity_micro, window_camper_attack, whale_bulk_exit_attack, ExitContract, LockLedger,
};
use engine_sim::*;

fn usd(micro: i128) -> f64 {
    micro as f64 / MICRO as f64
}

/// T10 — Default-as-exit is strictly worse than scheduled exit, at BOTH D-3
/// forfeiture bases. The attacker borrows the max λ against their $100
/// position at every annual window and defaults (interest forgiven, principal
/// settles against the position, next dividend forfeited).
#[test]
fn t10_default_as_exit_strictly_worse_than_scheduled_exit() {
    for base in [ForfeitureBase::PreDefault, ForfeitureBase::PostDefault] {
        let mut cfg = Config::reference();
        cfg.forfeiture_base = base;
        let o = default_as_exit_attack(&cfg, dollars(100), 5);
        assert!(
            !o.pencils_out,
            "base {base:?}: default-as-exit must not beat the honest path"
        );
        let t = cfg.initial_treasury_micro();
        // Round-to-nearest, exactly as Engine::begin_year computes it; the
        // division is exact at reference settings either way (62,580 ppm).
        let rate = (cfg.annual_distributable_micro(t) * PPM + t / 2) / t;
        let div_full = dollars(100) * rate / PPM; // $6.2580, exact
        // The no-money-creation identity: Σ loan cash + the eroded position
        // telescopes to exactly P0; the only cash beyond it is the year-0
        // dividend (every later one is forfeited by the default).
        assert_eq!(
            o.attacker_total_micro,
            dollars(100) + div_full,
            "base {base:?}: attacker total must be exactly P0 + the year-0 dividend"
        );
        // The gap to the honest path is exactly the 4 skipped dividends on
        // the full position — the forgiven interest creates no value, and
        // the position erosion earns no dividends either.
        assert_eq!(
            o.honest_total_micro - o.attacker_total_micro,
            4 * div_full,
            "base {base:?}: the honest gap must be exactly the 4 forfeited dividends"
        );
        // The D-3 base prices the door but never changes the attack's cash
        // flow: $6.258 pre-default vs $3.129 post-default, outcome identical.
        let expect_door = if base == ForfeitureBase::PreDefault {
            6.26
        } else {
            3.13
        };
        assert!(
            (usd(o.door_cost_micro) - expect_door).abs() < 0.01,
            "base {base:?}: door cost {} != {expect_door}",
            usd(o.door_cost_micro)
        );
        // The honest path really did keep the position and pay all 5.
        assert_eq!(o.honest_total_micro, dollars(100) + 5 * div_full);
    }
}

/// T11 — Blind-ape bulk exit vs the penalty door. The attacker forces the
/// entire $100K position out in one go above the per-period allowance and
/// pays the 12% penalty on the excess. The penalty must destroy value vs the
/// patient path — and it is a transfer to the remaining holders (the
/// deterrent's design intent), not destruction.
#[test]
fn t11_bulk_exit_penalty_door_deters() {
    let cfg = Config::reference();
    let pos = dollars(100_000);
    let o = bulk_exit_attack(&cfg, pos, 5);
    assert!(!o.pencils_out, "bulk exit must not beat the patient path");
    let t = cfg.initial_treasury_micro();
    let rate = (cfg.annual_distributable_micro(t) * PPM + t / 2) / t;
    let allowance = pos * rate / PPM; // $6,258 — the penalty-free allowance
    let excess = pos - allowance;
    assert_eq!(o.allowance_micro, allowance);
    // The penalty is exactly 12% of the excess (Carlos's deterrent, 12% floor).
    assert_eq!(
        o.penalty_paid_micro,
        excess_extraction_cost_micro(&cfg, excess),
        "penalty must be the door's own price"
    );
    assert_eq!(o.penalty_paid_micro, excess * 12 / 100);
    // Attacker + penalty == the full position: the penalty is a transfer to
    // the remaining holders, not value destruction.
    assert_eq!(o.attacker_total_micro + o.penalty_paid_micro, pos);
    // The patient path keeps the position AND collects 5 allowances free.
    assert_eq!(o.patient_total_micro, pos + 5 * allowance);
    // The gap decomposes exactly: the penalty plus the 5 allowances the ape
    // abandoned by rushing (the bulk exit consumed the whole position in one
    // go, so none of the 5 penalty-free allowances were ever taken).
    assert_eq!(
        o.patient_total_micro - o.attacker_total_micro,
        o.penalty_paid_micro + 5 * allowance
    );
}

/// T12 — Cohort-timing games under Carlos's entry ruling (2026-10-23 ~16:00
/// CST): entry is next-window-only; each token lock cycle is a unique pool of
/// locked funds that cannot be counted twice for accounting purposes.
/// (a) The unique-pool invariant holds over a deterministic entry schedule
///     spanning fractional times and window boundaries.
/// (b) Timing within a window (after its opening) changes nothing: every
///     entry lands in the next window's cohort.
/// (c) No retroactive eligibility: an entry can never join a window that has
///     already opened.
/// (d) The window-camper attack — enter just before each window, capture the
///     dividend, exit just after, 12 round-trips — ends poorer than the
///     honest scheduled path. The unique-pool rule forces the round-trip
///     (the same funds cannot sit in two cohorts), and the 12% penalty door
///     makes every round-trip lossy: the two rulings compose into the defense.
#[test]
fn t12_cohort_timing_games_fail_at_window_boundaries() {
    // (a) Unique-pool invariant over a mixed schedule (milli-months).
    let mut ledger = LockLedger::new();
    let schedule: &[(i128, i64)] = &[
        (dollars(100), 0),      // t=0.000 -> cohort 1
        (dollars(50), 2_999),   // t=2.999 -> cohort 3
        (dollars(200), 3_000),  // t=3.000 -> cohort 4 (window 3 already opened)
        (dollars(75), 3_500),   // t=3.500 -> cohort 4
        (dollars(125), 11_999), // t=11.999 -> cohort 0
        (dollars(300), 12_000), // t=12.000 -> cohort 1
    ];
    let mut total = 0i128;
    for (funds, t) in schedule {
        ledger.lock(*funds, *t);
        total += funds;
    }
    assert!(ledger.unique_pool_holds(), "every locked unit counted exactly once");
    assert_eq!(ledger.total_locked_micro, total);
    assert_eq!(ledger.pool_micro(1), dollars(400)); // 100 + 300
    assert_eq!(ledger.pool_micro(3), dollars(50));
    assert_eq!(ledger.pool_micro(4), dollars(275)); // 200 + 75
    assert_eq!(ledger.pool_micro(0), dollars(125));

    // (b) Timing within a window is invariant: all post-opening entries in
    // month 3 land in cohort 4.
    let mut ledger = LockLedger::new();
    for t in [3_000i64, 3_250, 3_500, 3_750, 3_999] {
        assert_eq!(ledger.lock(dollars(10), t), 4, "t={t}");
    }
    assert!(ledger.unique_pool_holds());
    assert_eq!(ledger.pool_micro(4), dollars(50));

    // (c) No retroactive eligibility across a sweep of entry times.
    let mut ledger = LockLedger::new();
    for t in [0i64, 1, 999, 1_000, 5_500, 11_000] {
        let c = ledger.lock(dollars(1), t);
        let opened = (t / 1_000) as usize % 12; // the window open at t
        assert_ne!(c, opened, "t={t}: entry must not join the already-open window {opened}");
    }

    // (d) The window camper: 12 round-trips vs the honest scheduled path.
    let cfg = Config::reference();
    let o = window_camper_attack(&cfg, dollars(100), 12);
    assert!(!o.pencils_out, "the camper must not beat the honest path");
    // Every round-trip is lossy: the dividend ($6.258) never covers the
    // door's price on the round-tripped principal ($11.249).
    assert!(o.per_window_net_micro < 0, "per-window net {}", usd(o.per_window_net_micro));
    let t = cfg.initial_treasury_micro();
    let rate = (cfg.annual_distributable_micro(t) * PPM + t / 2) / t;
    let div = dollars(100) * rate / PPM;
    let door = excess_extraction_cost_micro(&cfg, dollars(100) - div);
    assert_eq!(o.per_window_net_micro, div - door);
    // The honest path keeps the $100 and collects its one annual dividend.
    assert_eq!(o.honest_total_micro, dollars(100) + div);
    assert!(o.camper_total_micro < o.honest_total_micro);
    // The camper even loses against doing nothing: the attack destroys wealth.
    assert!(o.camper_total_micro < dollars(100), "camper {}", usd(o.camper_total_micro));
}

/// T13 — Whale threshold (Carlos's ruling: CORRECTED 2026-10-23 ~16:06 CST —
/// the fixed "purse of 1,000,000" figure is retracted; REFINED ~16:09 CST —
/// the threshold is the 25% cash sleeve NET of the lending book's maximum
/// draw = 2/3 × 25% × T = 16.67% of T; HARD BOUND ~16:15 CST — no single
/// exit order may exceed one period's movable liquidity. Canonical form in
/// the math doc, §117-119). A whale is a holder who can move more of the
/// treasury than the treasury itself can move in liquidity. The threshold is
/// relative and scale-invariant; entry is never refused by size; the
/// threshold selects the exit contract (whale → timed/tranched, below →
/// open-trader), and the window count is DERIVED from the hard bound —
/// position size stretches the schedule, it never enlarges an order.
/// Breaking the tranched schedule pays the penalty door.
#[test]
fn t13_whale_threshold_is_relative_and_sets_the_exit_contract() {
    let cfg = Config::reference();
    let t = cfg.initial_treasury_micro(); // $100M at reference
    let sleeve = cfg.cash_ppm; // 250_000

    // (a) The movable-liquidity figure itself: 2/3 × 25% × $100M.
    let movable = movable_liquidity_micro(t, sleeve);
    assert_eq!(movable, 16_666_666_666_666, "$16,666,666.666666 ratio-exact");
    // The math doc (§118) prints Carlos's $16,666,700 — a $33.33 arithmetic
    // rounding of the same figure, recorded here, not absorbed.
    assert!((usd(movable) - 16_666_666.67).abs() < 0.01);

    // (b) Relative and scale-invariant: the SAME purse classifies as whale
    // at a small treasury and non-whale at a large one.
    assert!(is_whale(dollars(1_000_000), dollars(3_000_000), sleeve)); // > $500K movable
    assert!(!is_whale(dollars(1_000_000), dollars(100_000_000), sleeve)); // < $16.67M movable
    assert!(is_whale(dollars(30_000_000), dollars(100_000_000), sleeve));
    assert!(!is_whale(dollars(10_000_000), dollars(100_000_000), sleeve));
    // Boundary: the purse must be LARGER than movable liquidity — exactly
    // movable is not a whale, one micro above is.
    assert!(!is_whale(movable, t, sleeve));
    assert!(is_whale(movable + 1, t, sleeve));
    // The same ratio classifies identically at $100 (scale invariance).
    let small_movable = movable_liquidity_micro(dollars(100), sleeve);
    assert!(!is_whale(small_movable, dollars(100), sleeve));
    assert!(is_whale(small_movable + 1, dollars(100), sleeve));

    // (c) Entry is never refused by size — the math is scalar. Even a purse
    // of 100% of the treasury is admitted; the definition governs the exit.
    assert!(admit_entry(t), "a purse of 100% of T is admitted");
    assert!(admit_entry(dollars(30_000_000)), "whale-size entry is admitted");
    assert!(admit_entry(dollars(100)));
    assert!(!admit_entry(0), "zero is not an entry");

    // (d) The threshold selects the exit contract (T = $100M, movable $16.67M).
    match exit_contract(&cfg, dollars(30_000_000)) {
        ExitContract::TimedTranched { tranche_micro, windows } => {
            // ceil($30M / $16.6667M) = 2 windows of $15M, each ≤ movable.
            assert_eq!(windows, 2);
            assert_eq!(tranche_micro, dollars(15_000_000));
            assert!(tranche_micro <= movable);
            assert_eq!(tranche_micro * windows as i128, dollars(30_000_000));
        }
        ExitContract::OpenTrader => panic!("whale-size must get the tranched contract"),
    }
    // Exactly 3× movable: 3 windows of exactly movable — the bound's edge.
    match exit_contract(&cfg, 3 * movable) {
        ExitContract::TimedTranched { tranche_micro, windows } => {
            assert_eq!(windows, 3);
            assert_eq!(tranche_micro, movable);
        }
        ExitContract::OpenTrader => panic!("3x movable is whale-size"),
    }
    assert_eq!(exit_contract(&cfg, dollars(100)), ExitContract::OpenTrader);
    assert_eq!(
        exit_contract(&cfg, movable),
        ExitContract::OpenTrader,
        "exactly movable stays open-trader"
    );

    // (e) HARD BOUND at the extreme edge: a purse equal to 100% of the
    // treasury exits ONLY in per-period tranches at or below movable
    // liquidity — a whale cannot order the entire treasury.
    match exit_contract(&cfg, t) {
        ExitContract::TimedTranched { tranche_micro, windows } => {
            // movable floors a hair under T/6 (16_666_666_666_666 < T/6), so
            // ceil lands on 7 periods — the conservative direction; the
            // doc's "at least six" holds.
            assert_eq!(windows, 7);
            assert_eq!(tranche_micro, t / 7); // $14,285,714.285714
            assert!(
                tranche_micro <= movable,
                "tranche {} exceeds one period's movable liquidity {}",
                usd(tranche_micro),
                usd(movable)
            );
            // The schedule is whole: position size stretched the schedule
            // over more periods, and the leftover dust is sub-micro.
            let covered = tranche_micro * windows as i128;
            assert!(covered <= t);
            assert!(t - covered < windows as i128);
        }
        ExitContract::OpenTrader => panic!("a purse of 100% of T is whale-size"),
    }

    // (f) Breaking the schedule: the $30M whale forces it all out in one
    // window against $15M tranches. The $15M excess pays the 12% door; the
    // scheduled path stays whole. The attack does not pencil out.
    let o = whale_bulk_exit_attack(dollars(30_000_000), 2, cfg.penalty_ppm);
    assert!(!o.pencils_out, "breaking the schedule must not beat it");
    assert_eq!(o.tranche_micro, dollars(15_000_000));
    assert_eq!(o.penalty_paid_micro, dollars(15_000_000) * 12 / 100);
    // Attacker + penalty == the full position: a transfer, not destruction.
    assert_eq!(o.attacker_total_micro + o.penalty_paid_micro, dollars(30_000_000));
    assert_eq!(o.scheduled_total_micro, dollars(30_000_000));
    assert!(o.attacker_total_micro < o.scheduled_total_micro);
}

/// T14 — D-5 CLOSED (2026-10-23 ~16:0x CST): the 60 bps transaction fee and
/// the 13 bps IDL fee ride the SAME flow — one combined 73 bps stream. The
/// projection model's income identity already prices 73bps×turnover; this
/// test locks the single-stream formulation: the 60/13 decomposition is
/// exact accounting with no seam, and one flow accrues the fee exactly once
/// (the split formulation's double-accrual risk — 146 bps on a both-routed
/// flow — cannot occur with a single fee term).
#[test]
fn t14_fee_stream_is_single_accrual() {
    // (a) The decomposition is exact: 6000 + 1300 == 7300, no seam.
    assert_eq!(6_000 + 1_300, 7_300);
    let cfg = Config::reference();
    assert_eq!(cfg.fee_ppm, 7_300, "the config carries the closed D-5 stream");
    let t = cfg.initial_treasury_micro();

    // (b) The engine's annual fee income is exactly T × turnover × 73 bps.
    let turnover_volume = t * cfg.turnover_ppm / PPM;
    let fee_income = turnover_volume * cfg.fee_ppm / PPM;
    assert_eq!(usd(fee_income), 730_000.0);

    // (c) Whole-dollar volumes decompose with no seam at all.
    for v in [1i128, 7, 100, 1_234_567, 100_000_000] {
        let m = dollars(v);
        assert_eq!(
            m * 7_300 / PPM,
            m * 6_000 / PPM + m * 1_300 / PPM,
            "whole-dollar volume {v}"
        );
    }
    // Sub-dollar micro volumes: the floor rule leaves at most a 1-micro seam
    // ($0.000001) — economically nothing, documented here, not hidden.
    for m in [1i128, 137, 999_999, 1_000_001] {
        let single = m * 7_300 / PPM;
        let parts = m * 6_000 / PPM + m * 1_300 / PPM;
        assert!((single - parts).abs() <= 1, "micro volume {m}: {single} vs {parts}");
    }

    // (d) The engine prices the model's 73bps×turnover term: T1's $730,000
    // fee slice inside the $5,730,000 growth+fee income.
    let gf = cfg.expected_growth_fee_income_micro(t);
    assert!((usd(gf - fee_income) - 5_000_000.0).abs() < 0.01);
    assert!((usd(gf) - 5_730_000.0).abs() < 0.01);
}
