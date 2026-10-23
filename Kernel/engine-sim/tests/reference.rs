//! W3.1 theorem tests: every number the projection model and the mathematical
//! logic document assert, reproduced by the simulator — or the test fails.

use engine_sim::governance::Governance;
use engine_sim::*;

fn usd(micro: i128) -> f64 { micro as f64 / MICRO as f64 }

/// T1 — Income identity (logic doc §2, scale table): at reference settings
/// the engine's year-1 income is the ratio-exact $6,258,000 on $100M:
/// growth $5,000,000 + fees $730,000 + lending $528,000 (performers only).
/// The projection model's printed $6,257,979 differs by $21 (0.00034%):
/// its integer-cohort lending figure ($43,998 × 12 = $527,976) vs the
/// ratio-exact $528,000 — reconciled here, within a $25 tolerance band,
/// and flagged in the roadmap rather than silently absorbed.
#[test]
fn t1_income_identity_reproduced() {
    let mut e = Engine::new(Config::reference());
    let r = e.run_months(12, None).unwrap();
    assert!((usd(r.growth_fee_income_micro) - 5_730_000.0).abs() < 0.01);
    // Lending interest realized inside year 1 lags origination by the 60-day
    // term: cohorts borrowing in months 11–12 settle in year 2. Realized
    // in-year interest is therefore 10/12 of the annual figure; the full
    // $528,000 is the steady-state annual rate (asserted via the identity).
    assert!((usd(r.lending_interest_micro) - 440_000.0).abs() < 0.01);
    let cfg = Config::reference();
    let t = cfg.initial_treasury_micro();
    assert_eq!(usd(cfg.expected_lending_income_micro(t)), 528_000.0);
    assert_eq!(usd(cfg.annual_distributable_micro(t)), 6_258_000.0);
    assert!((usd(cfg.annual_distributable_micro(t)) - 6_257_979.0).abs() <= 25.0);
    // Dividend bound: $6.26 per $100 holder, all-distributed (D-1 bound).
    let per_holder = usd(cfg.annual_distributable_micro(t)) / 1_000_000.0;
    assert!((per_holder - 6.26).abs() < 0.005, "dividend bound {per_holder}");
    // Every cohort was paid in full during the year.
    assert_eq!(r.full_tranches_paid, 12);
    assert_eq!(r.dividend_shortfall_micro, 0);
}

/// T2 — Lending cap theorem (logic doc §4): at ANY uptake, with 60-day terms,
/// outstanding never exceeds uptake × 8.333% of T — at 100% uptake exactly
/// 1/3 of the 25% cash sleeve. (Ratio-exact $8,333,333; the projection
/// model's integer-cohort figure is $8,333,300.)
#[test]
fn t2_lending_cap_holds_at_any_uptake() {
    for uptake in [10_000, 50_000, 100_000, 500_000, 1_000_000] {
        let mut cfg = Config::reference();
        cfg.uptake_ppm = uptake;
        let t = cfg.initial_treasury_micro();
        let mut e = Engine::new(cfg);
        let r = e.run_months(6, None).unwrap();
        let cap_principal = t * uptake / PPM / 12; // monthly origination × 2 cohorts (λ = ½ cancels the ×2)
        assert!(r.max_loans_outstanding_micro <= cap_principal + MICRO,
            "uptake {uptake}: outstanding {} exceeds cap {}", usd(r.max_loans_outstanding_micro), usd(cap_principal));
        assert!(r.max_loans_outstanding_micro * 3 <= t / 4 + 3 * MICRO,
            "uptake {uptake}: outstanding exceeds 1/3 of the cash sleeve");
    }
    // Exact at 100% uptake: 8.333% of T.
    let mut cfg = Config::reference();
    cfg.uptake_ppm = PPM;
    let mut e = Engine::new(cfg);
    let r = e.run_months(4, None).unwrap();
    assert!((usd(r.max_loans_outstanding_micro) - 8_333_333.33).abs() < 1.0);
}

/// T3 — Guarantee theorem (logic doc §5): with ZERO income, the cash sleeve
/// alone funds ~48 monthly tranches at the with-lending dividend rate
/// (6.258%): 47 full tranches, the 48th short. (The projection model's
/// 52-month figure is the same computation at the earlier 5.73% rate:
/// 25,000,000 / (5,730,000/12) = 52.4 — both are asserted so the two
/// documents' numbers are each reproduced at their own stated rate.)
#[test]
fn t3_guarantee_runway_at_both_documented_rates() {
    for (rate_ppm, expect_full) in [(62_580, 47), (57_300, 52)] {
        let mut cfg = Config::reference();
        cfg.growth_ppm = 0;
        cfg.fee_ppm = 0;
        cfg.uptake_ppm = 0;
        cfg.dividend_rate_override_ppm = Some(rate_ppm);
        let mut e = Engine::new(cfg);
        let r = e.run_months(60, None).unwrap();
        assert_eq!(r.full_tranches_paid, expect_full,
            "rate {rate_ppm}: full tranches {} != {expect_full}", r.full_tranches_paid);
        assert!(r.dividend_shortfall_micro > 0, "the sleeve must eventually run dry in the stress");
    }
}

/// T4 — Door calibration (logic doc §4): the two early-exit doors must price
/// alike or traffic arbitrages to the cheaper one. Excess extraction of $50
/// costs 12% = $6.00. Borrow-and-default costs the forfeited next dividend:
/// $6.26 on the pre-default base (doors aligned, within $0.30) but only
/// $3.13 on the post-default base (default becomes the discount door).
/// D-3 — which base — remains Carlos's open decision; this test prices both
/// so the decision's stakes stay executable, not rhetorical.
#[test]
fn t4_door_calibration_prices_both_forfeiture_bases() {
    let mut cfg = Config::reference();
    let t = cfg.initial_treasury_micro();
    let rate = cfg.annual_distributable_micro(t) * PPM / t;
    let penalty = usd(excess_extraction_cost_micro(&cfg, dollars(50)));
    assert_eq!(penalty, 6.00);
    cfg.forfeiture_base = ForfeitureBase::PreDefault;
    let pre = usd(default_door_cost_micro(&cfg, dollars(100), rate));
    assert!((pre - 6.26).abs() < 0.01);
    assert!((pre - penalty).abs() < 0.30, "pre-default base keeps the doors aligned");
    cfg.forfeiture_base = ForfeitureBase::PostDefault;
    let post = usd(default_door_cost_micro(&cfg, dollars(100), rate));
    assert!((post - 3.13).abs() < 0.01);
    assert!(post < penalty, "post-default base makes default the discount door — the D-3 stakes");
}

/// T5 — Scale invariance (logic doc §7): identical percentages at 100,
/// 10,000 and 1,000,000 holders. Only the dollar translations change.
#[test]
fn t5_scale_invariance() {
    let mut rows = Vec::new();
    for holders in [100u64, 10_000, 1_000_000] {
        let mut cfg = Config::reference();
        cfg.holders = holders;
        let t = cfg.initial_treasury_micro();
        let income_pct = cfg.annual_distributable_micro(t) as f64 / t as f64;
        let mut e = Engine::new(cfg);
        let r = e.run_months(12, None).unwrap();
        let paid_pct = r.dividends_paid_micro as f64 / t as f64;
        rows.push((income_pct, paid_pct, r.full_tranches_paid));
    }
    for w in rows.windows(2) {
        assert!((w[0].0 - w[1].0).abs() < 1e-6, "income % differs across scale");
        assert!((w[0].1 - w[1].1).abs() < 1e-6, "paid % differs across scale");
        assert_eq!(w[0].2, w[1].2);
    }
    assert!((rows[0].0 - 0.06258).abs() < 1e-6);
}

/// T6 — Position settlement (Carlos's ruling): after defaults mature, the
/// defaulters' positions have absorbed the principal — total holder position
/// shrinks by exactly the defaulted principal, the treasury's cash does not
/// leak it, and LGD ≈ 0 by construction.
#[test]
fn t6_default_settles_against_position_not_cash() {
    let mut cfg = Config::reference();
    cfg.holders = 12_000; // 1,000 per cohort, clean numbers
    let mut e = Engine::new(cfg);
    let before = e.run_months(0, None).unwrap().final_total_position_micro;
    let r = e.run_months(3, None).unwrap();
    // Cohort 0 borrowed at month 0: 100 borrowers × $50 = $5,000 principal;
    // 12% default = 12 holders × $50 = $600 settled against positions.
    assert_eq!(before - r.final_total_position_micro, dollars(600));
    assert!(r.lending_interest_micro > 0, "performers paid interest");
}

/// T7 — Governance gating: with the Kernel attached, a full year of tranches
/// and lending batches commits and settles as Kernel transitions; with a
/// policy stock cap below the tranche size, the Kernel's COMPAT predicate
/// blocks the flow and NO dividend moves.
#[test]
fn t7_kernel_gates_the_flows() {
    // Ungated baseline for comparison.
    let mut cfg = Config::reference();
    cfg.holders = 12_000;
    let mut plain = Engine::new(cfg.clone());
    let plain_r = plain.run_months(12, None).unwrap();

    let mut gov = Governance::new(None).unwrap();
    let mut gated = Engine::new(cfg.clone());
    let gated_r = gated.run_months(12, Some(&mut gov)).unwrap();
    assert_eq!(gated_r.dividends_paid_micro, plain_r.dividends_paid_micro);
    assert_eq!(gated_r.blocked_flows, 0);
    assert_eq!(gated_r.settled_transitions, 24); // 12 tranches + 12 lending batches
    assert_eq!(gov.kernel().history().len(), 24);

    // Cap the stock below one tranche: the predicate refuses, money stays.
    let mut gov = Governance::new(Some(dollars(10))).unwrap();
    let mut blocked = Engine::new(cfg);
    let blocked_r = blocked.run_months(1, Some(&mut gov)).unwrap();
    assert_eq!(blocked_r.dividends_paid_micro, 0);
    assert!(blocked_r.blocked_flows >= 1);
}
