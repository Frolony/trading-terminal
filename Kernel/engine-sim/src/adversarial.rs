//! W3.2 adversarial suite: the attacks the design must survive, as executable
//! claims. An attack "pencils out" iff the attacker's total wealth ends above
//! the honest scheduled-exit baseline — such a result is a DESIGN FINDING for
//! the roadmap and Carlos (the line stops until he rules), never a test to
//! weaken or a number to adjust.
//!
//! Attack classes (roadmap W3.2):
//! * **Default-as-exit** ([`default_as_exit_attack`]): the attacker borrows
//!   the maximum λ against their position at every window and defaults at
//!   maturity. Carlos's default ruling applies: interest forgiven, principal
//!   settles against the locked position, the next dividend airdrop is
//!   forfeited. Theorem: the default creates no money — loan cash plus the
//!   eroded position telescopes to exactly the original position, and the
//!   attacker's only delta vs the honest path is the forfeited dividend
//!   stream. Strictly worse at every D-3 forfeiture base (the base prices the
//!   door; it never changes the cash flow).
//! * **Blind-ape bulk exit** ([`bulk_exit_attack`]): the attacker forces the
//!   exit door in one go, extracting the whole position above the per-period
//!   allowance, and pays the 12% penalty on the excess (Carlos's deterrent,
//!   not a revenue line). Theorem: the penalty destroys exactly 12% of the
//!   extracted excess vs the patient rotation path, which pays zero penalty;
//!   the penalty is a transfer to the remaining holders, not destruction.
//! * **Cohort-timing games** — NOT yet modeled: the engine's aggregate and
//!   per-holder modes have no entry path, so a window camper cannot be
//!   expressed without inventing entry semantics; entry rules are a Carlos
//!   policy decision (D-2/D-9 adjacent). The class stays open.
//! * **Pre-launch whale entry refused by policy** — NOT yet modeled: the
//!   refusal threshold is an administrator policy parameter (Carlos's ruling
//!   territory — "no whales before the project is live"), and `kernel-policy`
//!   has no entry gate to refuse at. Inventing the threshold here would be
//!   builder improvisation, which the build discipline forbids. The class
//!   stays open until Carlos sets the parameter.
//!
//! Predicate fidelity (Carlos's standing emphasis, 2026-10-23): these attacks
//! model the ATTACKER's wealth only. No treasury flow in the scenarios
//! bypasses governance — every value-moving engine flow still routes through
//! a mask-clear Kernel commit via the governance adapter; the attacks are
//! measured against the honest paths the predicate admits, never against a
//! weakened kernel.
//!
//! Parameters are Carlos's stated assumptions (2026-10-22), labeled as such:
//! they are the attack surface under test, never forecasts or promises.

use crate::{default_door_cost_micro, excess_extraction_cost_micro, Config, PPM};

fn mul_ppm(x: i128, ppm: i128) -> i128 {
    x * ppm / PPM
}

/// The annual distribution rate (ppm of T) implied by a config, computed the
/// same way `Engine::begin_year` computes it (rounded to nearest ppm).
fn annual_rate_ppm(cfg: &Config) -> i128 {
    let t = cfg.initial_treasury_micro();
    if t <= 0 {
        return 0;
    }
    (cfg.annual_distributable_micro(t) * PPM + t / 2) / t
}

/// Outcome of the default-as-exit attack over a run of annual windows.
#[derive(Debug, Clone)]
pub struct DefaultExitOutcome {
    /// Attacker's end wealth: extracted cash + remaining (eroded) position.
    pub attacker_total_micro: i128,
    /// Honest scheduled-exit baseline: position intact + every dividend.
    pub honest_total_micro: i128,
    /// The dividend stream the attacker forfeited by defaulting.
    pub dividends_forfeited_micro: i128,
    /// The door's calibrated price at this D-3 base (for the record).
    pub door_cost_micro: i128,
    /// True iff the attack ends wealthier than the honest path (a finding).
    pub pencils_out: bool,
}

/// The attacker borrows the max λ·position at every annual window and
/// defaults at maturity: interest forgiven, principal settles against the
/// locked position, the next dividend airdrop forfeited — exactly the rules
/// `Engine::run_month` applies on its default path, at holder granularity.
/// The honest baseline extracts the scheduled dividend every year and never
/// borrows.
pub fn default_as_exit_attack(cfg: &Config, position_micro: i128, years: u32) -> DefaultExitOutcome {
    let rate = annual_rate_ppm(cfg);
    let lambda = cfg.lambda_ppm;
    let door_cost = default_door_cost_micro(cfg, position_micro, rate);
    let mut pos = position_micro;
    let mut cash = 0i128;
    let mut forfeited = 0i128;
    let mut skip = false;
    for _ in 0..years {
        if skip {
            // The default forfeits the next eligible dividend airdrop.
            forfeited += mul_ppm(pos, rate);
        } else {
            cash += mul_ppm(pos, rate);
        }
        // Borrow the max, then default at maturity: the loan cash arrives,
        // the position absorbs the principal.
        let loan = mul_ppm(pos, lambda);
        cash += loan;
        pos -= loan;
        skip = true;
    }
    let attacker_total = cash + pos;
    let honest_total = position_micro + years as i128 * mul_ppm(position_micro, rate);
    DefaultExitOutcome {
        attacker_total_micro: attacker_total,
        honest_total_micro: honest_total,
        dividends_forfeited_micro: forfeited,
        door_cost_micro: door_cost,
        pencils_out: attacker_total > honest_total,
    }
}

/// Outcome of the blind-ape bulk exit: one forced exit above the allowance.
#[derive(Debug, Clone)]
pub struct BulkExitOutcome {
    /// Attacker's end wealth: the position minus the penalty paid.
    pub attacker_total_micro: i128,
    /// Patient baseline: position intact + one allowance per year, no penalty.
    pub patient_total_micro: i128,
    /// The penalty destroyed on the excess (transferable to the treasury).
    pub penalty_paid_micro: i128,
    /// The per-period penalty-free allowance used in the scenario.
    pub allowance_micro: i128,
    /// True iff the attack ends wealthier than the patient path (a finding).
    pub pencils_out: bool,
}

/// The attacker forces the exit door in one go — extracting the whole
/// position above the per-period allowance — and pays the 12% penalty on the
/// excess. The patient baseline extracts the penalty-free allowance each
/// year and keeps the position.
pub fn bulk_exit_attack(cfg: &Config, position_micro: i128, years: u32) -> BulkExitOutcome {
    let rate = annual_rate_ppm(cfg);
    // The per-period penalty-free allowance: the dividend the position would
    // have earned this period (logic doc §8 two-door calibration).
    let allowance = mul_ppm(position_micro, rate);
    let excess = (position_micro - allowance).max(0);
    let penalty = excess_extraction_cost_micro(cfg, excess);
    let attacker_total = position_micro - penalty;
    let patient_total = position_micro + years as i128 * allowance;
    BulkExitOutcome {
        attacker_total_micro: attacker_total,
        patient_total_micro: patient_total,
        penalty_paid_micro: penalty,
        allowance_micro: allowance,
        pencils_out: attacker_total > patient_total,
    }
}
