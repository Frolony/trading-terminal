//! PORT NOTE (migration step 4, increment 3): the v0.2 engine-sim
//! adversarial suite, ported verbatim except for this note and the D-3
//! line below. Arithmetic, attack classes, and T10–T14 values are
//! unchanged; any value change against the v0.2 run would be a finding,
//! not a re-baseline. The attacks are pure-wealth models: they measure
//! the attacker against the honest paths the predicate admits, exactly as
//! in v0.2 — no attack flow bypasses governance, and the flows they are
//! measured against are the gated flows T7/T9 prove through the v0.4
//! adapter. D-3 is CLOSED (pre-default base, 2026-10-23); T10 still prices
//! both bases so the post-default figure stays as the rejected
//! counterfactual, as T4 does. NOT a D-10 import: the program-layer whale
//! FLAG revision (2026-10-24, flag >= T/30, weekly 20%-of-position blocks
//! capped at FLAG) is a separate program slice and does NOT redefine
//! T13's W3.2 whale threshold (> movable liquidity, timed/tranched exit)
//! — the two must not be conflated.
//!
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
//!   allowance, and pays the flat 13% penalty on the excess (Carlos's deterrent,
//!   not a revenue line). Theorem: the penalty destroys exactly 13% of the
//!   extracted excess vs the patient rotation path, which pays zero penalty;
//!   the penalty is a transfer to the remaining holders, not destruction.
//! * **Cohort-timing games** ([`LockLedger`], [`window_camper_attack`]):
//!   LANDED on Carlos's entry ruling (2026-10-23 ~16:00 CST): entry is
//!   next-window-only — each token lock cycle is a unique pool of locked
//!   funds that cannot be counted twice for accounting purposes, so there is
//!   no mid-window entry. An entry at time `t` joins the first window opening
//!   strictly after `t`; the ledger enforces the unique-pool invariant, and
//!   the window-camper attack (enter just before each window, capture the
//!   dividend, exit just after) is measured against the honest scheduled
//!   path. The unique-pool rule forces the camper to round-trip through the
//!   exit door, and the 13% penalty door makes every round-trip lossy — the
//!   two rulings compose into the defense.
//! * **Whale exit contract** ([`movable_liquidity_micro`], [`is_whale`],
//!   [`admit_entry`], [`exit_contract`], [`whale_bulk_exit_attack`]): LANDED
//!   on Carlos's whale ruling as CORRECTED 2026-10-23 ~16:06 CST (the fixed
//!   "purse of 1,000,000" figure from ~16:00 is retracted), then REFINED
//!   ~16:09 CST and bound ~16:15 CST — now in the math doc as §117-119. A
//!   whale is a holder who can move more of the treasury than the treasury
//!   itself can move in liquidity — a purse larger than the treasury's
//!   MOVABLE liquidity, which is the 25% cash sleeve NET of the lending
//!   book's maximum draw: exactly 1/3 of the sleeve at 60-day terms (logic
//!   doc §4: max outstanding = uptake × 8.333% of T), so movable liquidity =
//!   2/3 × 25% × T = 16.67% of T ($16,666,666.67 ratio-exact at the $100M
//!   reference; the doc prints Carlos's $16,666,700 — a $33.33 arithmetic
//!   rounding, recorded, not absorbed). The threshold is relative: it scales
//!   with the treasury, preserving the scale invariance T5 asserts. Entry is
//!   NOT refused by size (the math is scalar, so position size is
//!   percentage-neutral); the threshold sets the EXIT contract instead:
//!   whale-size exits on a timed/tranched contract, below-whale positions are
//!   open-trader under the standard penalty door. The aligned/patient
//!   carve-out stands as governance stance, not a simulator parameter.
//!   HARD BOUND: no single exit order — a whale's tranche included — may
//!   exceed one period's movable liquidity (the period exit capacity);
//!   position size stretches the exit schedule over more periods, it never
//!   enlarges a single order. A purse equal to the entire treasury exits only
//!   in per-period tranches at or below movable liquidity — the contract
//!   derives the window count from the bound; it is not a free parameter.
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
/// position above the per-period allowance — and pays the flat 13% penalty on the
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

// ---------------------------------------------------------------------------
// Cohort-timing games (Carlos's entry ruling, 2026-10-23 ~16:00 CST)
// ---------------------------------------------------------------------------

/// Entry ledger for the next-window-only rule. Time is in integer
/// milli-months (1_000 = one month) so window boundaries are exact — no float
/// seam at the boundary where the timing game lives.
///
/// Rule: an entry at time `t` joins the first window opening strictly after
/// `t`, i.e. cohort `(floor(t) + 1) mod 12`. An entry can therefore never join
/// a window that has already opened (no retroactive eligibility), and no two
/// windows can count the same funds (the unique-pool invariant).
#[derive(Debug, Clone, Default)]
pub struct LockLedger {
    /// Locked funds per cohort window (micro-dollars).
    pools: [i128; 12],
    /// Every unit ever locked, for the invariant check.
    pub total_locked_micro: i128,
}

impl LockLedger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Lock `funds_micro` at `now_millimonths`; returns the assigned cohort.
    pub fn lock(&mut self, funds_micro: i128, now_millimonths: i64) -> usize {
        assert!(funds_micro > 0, "only positive locks are entries");
        assert!(now_millimonths >= 0, "entry time is measured from genesis");
        let cohort = ((now_millimonths / 1_000 + 1) % 12) as usize;
        self.pools[cohort] += funds_micro;
        self.total_locked_micro += funds_micro;
        cohort
    }

    pub fn pool_micro(&self, cohort: usize) -> i128 {
        self.pools[cohort % 12]
    }

    /// Carlos's invariant: each token lock cycle is a unique pool of locked
    /// funds — no unit counted twice, none lost.
    pub fn unique_pool_holds(&self) -> bool {
        self.pools.iter().sum::<i128>() == self.total_locked_micro
    }
}

/// Outcome of the window-camper attack: enter just before each window opens,
/// capture that window's dividend, exit just after — repeated across windows.
#[derive(Debug, Clone)]
pub struct CamperOutcome {
    /// Camper's end wealth after `windows` round-trips.
    pub camper_total_micro: i128,
    /// Honest scheduled path: lock once, keep the position, one annual dividend.
    pub honest_total_micro: i128,
    /// Per-window net at the opening position: dividend minus the door's
    /// price on the round-tripped principal. Negative every window means the
    /// round-trip destroys wealth and no repetition can pencil out.
    pub per_window_net_micro: i128,
    /// True iff the attack ends wealthier than the honest path (a finding).
    pub pencils_out: bool,
}

pub fn window_camper_attack(cfg: &Config, position_micro: i128, windows: u32) -> CamperOutcome {
    let rate = annual_rate_ppm(cfg);
    let div = mul_ppm(position_micro, rate);
    let penalty = excess_extraction_cost_micro(cfg, position_micro - div);
    // The camper must exit and re-enter between windows: the unique-pool rule
    // forbids the same funds sitting in two cohorts at once. Each exit forces
    // the whole principal through the penalty door above the allowance.
    let mut pos = position_micro;
    for _ in 0..windows {
        let d = mul_ppm(pos, rate);
        let p = excess_extraction_cost_micro(cfg, pos - d);
        pos = pos + d - p;
    }
    let honest_total = position_micro + mul_ppm(position_micro, rate);
    CamperOutcome {
        camper_total_micro: pos,
        honest_total_micro: honest_total,
        per_window_net_micro: div - penalty,
        pencils_out: pos > honest_total,
    }
}

// ---------------------------------------------------------------------------
// Whale exit contract (Carlos's ruling: corrected 2026-10-23 ~16:06 CST — the
// fixed "purse of 1,000,000" figure is retracted; refined ~16:09 CST — the
// threshold is the cash sleeve NET of the lending book's maximum draw; hard
// bound added ~16:15 CST. Canonical form in the math doc, §117-119.)
// ---------------------------------------------------------------------------

/// One period's movable liquidity: the treasury's cash sleeve NET of the
/// lending book's maximum draw on it. By the logic doc's §4, the book can
/// consume at most exactly 1/3 of the 25% sleeve at 60-day terms
/// (max outstanding = uptake × 8.333% of T; at 100% uptake = 8.333% of T =
/// 1/3 of the sleeve), so L = 2/3 × cash_ppm × T — 16.67% of T at reference.
/// Integer math — no float seam at the boundary.
///
/// The `cash_sleeve_ppm` parameter keeps the sleeve weight configurable; the
/// 2/3 net factor is the §4 lending cap, not a tuning knob.
pub fn movable_liquidity_micro(treasury_micro: i128, cash_sleeve_ppm: i128) -> i128 {
    treasury_micro * cash_sleeve_ppm * 2 / (PPM * 3)
}

/// A whale is a holder who can move more of the treasury than the treasury
/// itself can move in liquidity: position > one period's movable liquidity.
/// Relative by construction: the predicate is identical at every scale (T5's
/// invariance, extended). This supersedes the interim 25%-of-full-sleeve
/// formulation — the lending book's maximum draw is spoken for, so it is
/// not movable.
pub fn is_whale(position_micro: i128, treasury_micro: i128, cash_sleeve_ppm: i128) -> bool {
    position_micro > movable_liquidity_micro(treasury_micro, cash_sleeve_ppm)
}

/// Entry is never refused by size (Carlos's ruling: the math is scalar, so
/// position size is percentage-neutral). The whale threshold sets the EXIT
/// contract, not admission. Positive positions are admitted; zero/negative
/// are not entries at all.
pub fn admit_entry(position_micro: i128) -> bool {
    position_micro > 0
}

/// The exit contract the whale threshold selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitContract {
    /// Whale-size: the exit is metered across `windows` tranches, each at or
    /// below one period's movable liquidity. Each window's tranche exits
    /// penalty-free; anything above the scheduled tranche pays the penalty
    /// door on the excess. The contract is more generous per window than the
    /// standard door (a full tranche vs the annual dividend allowance) but
    /// strictly metered — the treasury's liquidity is never outrun. Breaking
    /// the schedule is the attack.
    TimedTranched { tranche_micro: i128, windows: u32 },
    /// Below whale-size: the standard penalty door (per-period allowance).
    OpenTrader,
}

/// Select the exit contract for a position against a treasury. The window
/// count is DERIVED from the hard bound (Carlos ~16:15 CST): no single exit
/// order — a whale's tranche included — may exceed one period's movable
/// liquidity, so `windows = ceil(position / movable_liquidity)`. Position
/// size stretches the schedule over more periods; it never enlarges a
/// single order. A purse equal to the entire treasury exits over six-plus
/// periods of ≤ L. The window count is not a modeling parameter: a caller
/// that could choose 1 window would choose away the bound.
pub fn exit_contract(cfg: &Config, position_micro: i128) -> ExitContract {
    let t = cfg.initial_treasury_micro();
    let movable = movable_liquidity_micro(t, cfg.cash_ppm);
    if !is_whale(position_micro, t, cfg.cash_ppm) {
        return ExitContract::OpenTrader;
    }
    debug_assert!(movable > 0, "a positive treasury has positive movable liquidity");
    // Ceil division: the smallest window count whose tranches fit the bound.
    let windows = (position_micro + movable - 1) / movable;
    ExitContract::TimedTranched {
        tranche_micro: position_micro / windows,
        windows: windows as u32,
    }
}

/// Outcome of the whale breaking the timed/tranched contract: forcing the
/// whole position out in one window instead of following the schedule.
#[derive(Debug, Clone)]
pub struct WhaleExitOutcome {
    /// Attacker's end wealth: the position minus the door's price on the excess.
    pub attacker_total_micro: i128,
    /// The scheduled path: the full position, no penalty.
    pub scheduled_total_micro: i128,
    /// The penalty paid for breaking the schedule (a transfer, not destruction).
    pub penalty_paid_micro: i128,
    /// The per-window scheduled tranche the contract offered.
    pub tranche_micro: i128,
    /// True iff breaking the schedule beats following it (a finding).
    pub pencils_out: bool,
}

pub fn whale_bulk_exit_attack(
    position_micro: i128,
    tranche_windows: u32,
    penalty_ppm: i128,
) -> WhaleExitOutcome {
    let tranche = position_micro / tranche_windows as i128;
    let excess = position_micro - tranche;
    let penalty = excess * penalty_ppm / PPM;
    WhaleExitOutcome {
        attacker_total_micro: position_micro - penalty,
        scheduled_total_micro: position_micro,
        penalty_paid_micro: penalty,
        tranche_micro: tranche,
        pencils_out: position_micro - penalty > position_micro,
    }
}
