//! W3.2 adversarial theorem tests: each attack is an executable claim —
//! either the design survives (asserted here) or the attack pencils out (a
//! design finding for the roadmap and Carlos, never a test to weaken).

use engine_sim::adversarial::{bulk_exit_attack, default_as_exit_attack};
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
