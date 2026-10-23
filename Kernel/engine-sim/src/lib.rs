//! # engine-sim — the treasury mathematics as an executable specification (W3.1)
//!
//! This crate ports `TREASURY_MATHEMATICAL_LOGIC.md` into a deterministic
//! simulator. Every parameter is one of Carlos's stated design assumptions
//! (2026-10-22); they are assumptions under test, not forecasts or promises.
//!
//! ## Modelling choices (all deliberate, all documented)
//!
//! * **Ratio-exact aggregate arithmetic.** Holders are modelled as 12 monthly
//!   cohorts of aggregate positions, in fixed-point micro-dollars and
//!   parts-per-million rates — the logic document's "pure percentage"
//!   formulation. The projection model's worked examples use integer holders
//!   (83,333 per cohort) and differ from the ratio-exact figures by < 0.001%
//!   (e.g. lending income $528,000 ratio-exact vs $527,979 printed; the
//!   printed per-cohort $43,998 × 12 = $527,976 — the $3 gap is a rounding
//!   slip in the document, flagged in the roadmap, not silently absorbed).
//! * **Monthly cadence, Carlos's timing rule:** dividends are paid at the
//!   *beginning* of a cohort's month, book-kept monthly against the annual
//!   obligation; loan interest and portfolio accrual land on the same rhythm.
//! * **Defaults settle against the locked position** (Carlos's ruling):
//!   principal is recovered by reducing the defaulter's position; interest is
//!   forgiven; the defaulter skips their next dividend airdrop.
//! * **Open decisions stay visible:** the distribute/retain split (D-1) is
//!   modelled at its all-distributed upper bound, and the dividend-forfeiture
//!   base (D-3) is a parameter ([`ForfeitureBase`]) whose two values the door
//!   calibration test prices side by side.
//! * **Governance is not decoration:** with a [`Governance`] adapter attached,
//!   every dividend tranche and lending batch must first commit as a Kernel
//!   transition under the `kernel-policy` launch profile. A flow whose commit
//!   predicate does not clear does not move money in the simulator either.

pub mod governance;

use governance::Governance;

/// Parts-per-million denominator for all rates.
pub const PPM: i128 = 1_000_000;
/// Micro-dollars per dollar (matches the kernel's `Amount` scale).
pub const MICRO: i128 = 1_000_000;

fn mul_ppm(x: i128, ppm: i128) -> i128 { x * ppm / PPM }
/// Micro-dollars for a whole-dollar amount.
pub fn dollars(x: i128) -> i128 { x * MICRO }

/// Which position the default forfeiture is computed on — roadmap item D-3,
/// deliberately left open; the simulator prices both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForfeitureBase {
    /// Dividend booked on the position as it stood before default ($100 → $6.26).
    PreDefault,
    /// Dividend computed on the post-settlement position ($50 → $3.13).
    PostDefault,
}

/// Scenario parameters. All rates are parts-per-million.
#[derive(Debug, Clone)]
pub struct Config {
    pub holders: u64,
    pub position_micro: i128,
    /// Portfolio growth assumption (the 5% blend): 50_000 ppm.
    pub growth_ppm: i128,
    /// Combined transaction + IDL fee (60 + 13 bps), if both ride one flow (D-5 open).
    pub fee_ppm: i128,
    pub turnover_ppm: i128,
    /// Cash sleeve weight (25% after Carlos's 2026-10-22 revision).
    pub cash_ppm: i128,
    /// Share of each cohort that borrows at its window.
    pub uptake_ppm: i128,
    /// Max loan per position (50%).
    pub lambda_ppm: i128,
    /// Loan rate per term (12% — Carlos's final ruling).
    pub loan_rate_ppm: i128,
    /// Assumed delinquency (~12%).
    pub delinquency_ppm: i128,
    /// Penalty on extraction above the per-period allowance (12% floor).
    pub penalty_ppm: i128,
    pub term_months: u32,
    pub forfeiture_base: ForfeitureBase,
    /// Zero-income stress: pay dividends at this fixed annual rate (ppm of T)
    /// while growth, fees and lending income are switched off by the caller.
    pub dividend_rate_override_ppm: Option<i128>,
}

impl Config {
    /// The reference scenario of the projection model: 1M holders × $100.
    pub fn reference() -> Config {
        Config {
            holders: 1_000_000,
            position_micro: dollars(100),
            growth_ppm: 50_000,
            fee_ppm: 7_300,
            turnover_ppm: PPM,
            cash_ppm: 250_000,
            uptake_ppm: 100_000,
            lambda_ppm: 500_000,
            loan_rate_ppm: 120_000,
            delinquency_ppm: 120_000,
            penalty_ppm: 120_000,
            term_months: 2,
            forfeiture_base: ForfeitureBase::PreDefault,
            dividend_rate_override_ppm: None,
        }
    }
    pub fn initial_treasury_micro(&self) -> i128 {
        self.holders as i128 * self.position_micro
    }
    /// Expected annual lending income at this config, on a treasury of `t`
    /// (performers' interest only — defaulters' interest is forgiven):
    /// originated/yr = T · uptake · λ; interest = originated · (1 − δ) · r.
    pub fn expected_lending_income_micro(&self, t: i128) -> i128 {
        let originated = mul_ppm(mul_ppm(t, self.uptake_ppm), self.lambda_ppm);
        mul_ppm(mul_ppm(originated, PPM - self.delinquency_ppm), self.loan_rate_ppm)
    }
    /// Expected annual growth + fee income on a treasury of `t`.
    pub fn expected_growth_fee_income_micro(&self, t: i128) -> i128 {
        let fees = mul_ppm(mul_ppm(t, self.turnover_ppm), self.fee_ppm);
        mul_ppm(t, self.growth_ppm) + fees
    }
    /// The annual distributable income the dividend book-keeps against.
    pub fn annual_distributable_micro(&self, t: i128) -> i128 {
        match self.dividend_rate_override_ppm {
            Some(rate) => mul_ppm(t, rate),
            None => self.expected_growth_fee_income_micro(t) + self.expected_lending_income_micro(t),
        }
    }
}

/// A group of identical holders inside a cohort (same position, same standing).
#[derive(Debug, Clone)]
struct Segment {
    /// Holder count, scaled by PPM so cohort fractions stay exact.
    holders_ppm: i128,
    position_micro: i128,
    skip_next_dividend: bool,
}

#[derive(Debug, Clone)]
struct LoanBatch {
    maturity_month: u32,
    cohort: usize,
    position_micro: i128,
    borrowers_ppm: i128,
    principal_micro: i128,
}

/// Cumulative outcome of a run.
#[derive(Debug, Default, Clone)]
pub struct Report {
    pub months_run: u32,
    pub growth_fee_income_micro: i128,
    pub lending_interest_micro: i128,
    pub dividends_paid_micro: i128,
    pub dividend_shortfall_micro: i128,
    pub full_tranches_paid: u32,
    pub max_loans_outstanding_micro: i128,
    pub final_cash_micro: i128,
    pub final_total_position_micro: i128,
    pub blocked_flows: u32,
    pub settled_transitions: u32,
}

impl Report {
    pub fn total_income_micro(&self) -> i128 {
        self.growth_fee_income_micro + self.lending_interest_micro
    }
}

/// The deterministic monthly engine.
pub struct Engine {
    cfg: Config,
    month: u32,
    cohorts: Vec<Vec<Segment>>,
    batches: Vec<LoanBatch>,
    loans_outstanding_micro: i128,
    cash_micro: i128,
    year_start_total_micro: i128,
    annual_distributable_micro: i128,
    annual_rate_ppm: i128,
    paid_this_year_micro: i128,
    report: Report,
}

impl Engine {
    pub fn new(cfg: Config) -> Engine {
        let total = cfg.initial_treasury_micro();
        let total_scaled = cfg.holders as i128 * PPM;
        let per = total_scaled / 12;
        let mut cohorts = Vec::new();
        for i in 0..12 {
            let n = if i == 11 { total_scaled - per * 11 } else { per };
            cohorts.push(vec![Segment { holders_ppm: n, position_micro: cfg.position_micro, skip_next_dividend: false }]);
        }
        let mut e = Engine {
            cash_micro: mul_ppm(total, cfg.cash_ppm),
            cfg,
            month: 0,
            cohorts,
            batches: Vec::new(),
            loans_outstanding_micro: 0,
            year_start_total_micro: total,
            annual_distributable_micro: 0,
            annual_rate_ppm: 0,
            paid_this_year_micro: 0,
            report: Report::default(),
        };
        e.begin_year();
        e.report.final_cash_micro = e.cash_micro;
        e.report.final_total_position_micro = e.total_position_micro();
        e
    }

    fn total_position_micro(&self) -> i128 {
        self.cohorts.iter().flat_map(|c| c.iter())
            .map(|s| s.holders_ppm * s.position_micro / PPM).sum()
    }

    fn begin_year(&mut self) {
        self.year_start_total_micro = self.total_position_micro();
        self.annual_distributable_micro = self.cfg.annual_distributable_micro(self.year_start_total_micro);
        self.annual_rate_ppm = if self.year_start_total_micro > 0 {
            self.annual_distributable_micro * PPM / self.year_start_total_micro
        } else { 0 };
        self.paid_this_year_micro = 0;
    }

    pub fn cash_micro(&self) -> i128 { self.cash_micro }
    pub fn loans_outstanding_micro(&self) -> i128 { self.loans_outstanding_micro }
    pub fn annual_rate_ppm(&self) -> i128 { self.annual_rate_ppm }

    /// Run one month. With `gov` attached, the dividend tranche and the
    /// lending batch must each commit in the Kernel before money moves;
    /// a refused commit blocks that flow for the month (recorded, not hidden).
    pub fn run_month(&mut self, gov: Option<&mut Governance>) -> Result<(), String> {
        let mut gov = gov;
        if self.month > 0 && self.month.is_multiple_of(12) { self.begin_year(); }
        let cohort_idx = (self.month % 12) as usize;

        // ---- 1. Dividend tranche, paid at the beginning of the cohort's month.
        let mut payout: i128 = 0;
        for s in &self.cohorts[cohort_idx] {
            if !s.skip_next_dividend {
                payout += s.holders_ppm * s.position_micro * self.annual_rate_ppm / (PPM * PPM);
            }
        }
        if payout == 0 {
            // A cohort whose members all owe a skip has served it by being passed over.
            for s in &mut self.cohorts[cohort_idx] { s.skip_next_dividend = false; }
        }
        if payout > 0 {
            let mut authorized = true;
            let mut auth = None;
            if let Some(g) = gov.as_deref_mut() {
                let remaining = self.annual_distributable_micro - self.paid_this_year_micro;
                match g.authorize(kernel_policy::RES_TREASURY_TRANCHE, payout, remaining, self.month, 1)? {
                    Some(a) => auth = Some(a),
                    None => authorized = false,
                }
            }
            if authorized {
                let paid = payout.min(self.cash_micro);
                self.cash_micro -= paid;
                self.report.dividends_paid_micro += paid;
                self.report.dividend_shortfall_micro += payout - paid;
                self.paid_this_year_micro += paid;
                if paid == payout { self.report.full_tranches_paid += 1; }
                for s in &mut self.cohorts[cohort_idx] { s.skip_next_dividend = false; }
                if let (Some(g), Some(a)) = (gov.as_deref_mut(), auth) {
                    g.settle(&a)?;
                    self.report.settled_transitions += 1;
                }
            } else {
                self.report.blocked_flows += 1;
            }
        }

        // ---- 2. Maturing loans settle (60-day paper lands on the monthly rhythm).
        let maturing: Vec<LoanBatch> = self.batches.iter()
            .filter(|b| b.maturity_month == self.month).cloned().collect();
        self.batches.retain(|b| b.maturity_month != self.month);
        for b in maturing {
            let loan_per = mul_ppm(b.position_micro, self.cfg.lambda_ppm);
            let defaulters_ppm = mul_ppm(b.borrowers_ppm, self.cfg.delinquency_ppm);
            let performers_ppm = b.borrowers_ppm - defaulters_ppm;
            let performers_principal = performers_ppm * loan_per / PPM;
            let interest = mul_ppm(performers_principal, self.cfg.loan_rate_ppm);
            self.cash_micro += performers_principal + interest;
            self.report.lending_interest_micro += interest;
            self.loans_outstanding_micro -= b.principal_micro;
            // Default: principal settles against the locked position; interest
            // forgiven; the defaulter's segment halves and skips its next airdrop.
            if defaulters_ppm > 0 {
                let cohort = &mut self.cohorts[b.cohort];
                if let Some(seg) = cohort.iter_mut().find(|s| s.position_micro == b.position_micro && !s.skip_next_dividend) {
                    seg.holders_ppm -= defaulters_ppm;
                }
                let reduced = b.position_micro - loan_per;
                if let Some(seg) = cohort.iter_mut().find(|s| s.position_micro == reduced && s.skip_next_dividend) {
                    seg.holders_ppm += defaulters_ppm;
                } else {
                    cohort.push(Segment { holders_ppm: defaulters_ppm, position_micro: reduced, skip_next_dividend: true });
                }
            }
        }

        // ---- 3. This cohort borrows at its window.
        let mut principal: i128 = 0;
        let mut batch_borrowers: Vec<(i128, i128)> = Vec::new(); // (position, borrowers_ppm)
        for s in &self.cohorts[cohort_idx] {
            let borrowers = mul_ppm(s.holders_ppm, self.cfg.uptake_ppm);
            if borrowers > 0 {
                let loan_per = mul_ppm(s.position_micro, self.cfg.lambda_ppm);
                principal += borrowers * loan_per / PPM;
                batch_borrowers.push((s.position_micro, borrowers));
            }
        }
        if principal > 0 {
            let mut authorized = true;
            let mut auth = None;
            if let Some(g) = gov.as_deref_mut() {
                match g.authorize(kernel_policy::RES_LENDING_BOOK, principal, self.cash_micro, self.month, 2)? {
                    Some(a) => auth = Some(a),
                    None => authorized = false,
                }
            }
            if authorized && principal <= self.cash_micro {
                self.cash_micro -= principal;
                self.loans_outstanding_micro += principal;
                for (position, borrowers) in batch_borrowers {
                    let loan_per = mul_ppm(position, self.cfg.lambda_ppm);
                    self.batches.push(LoanBatch {
                        maturity_month: self.month + self.cfg.term_months,
                        cohort: cohort_idx,
                        position_micro: position,
                        borrowers_ppm: borrowers,
                        principal_micro: borrowers * loan_per / PPM,
                    });
                }
                if let (Some(g), Some(a)) = (gov, auth) {
                    g.settle(&a)?;
                    self.report.settled_transitions += 1;
                }
            } else {
                self.report.blocked_flows += 1;
            }
        }
        self.report.max_loans_outstanding_micro =
            self.report.max_loans_outstanding_micro.max(self.loans_outstanding_micro);

        // ---- 4. Portfolio growth + fee accrual for the month.
        let accrual = self.cfg.expected_growth_fee_income_micro(self.year_start_total_micro) / 12;
        self.cash_micro += accrual;
        self.report.growth_fee_income_micro += accrual;

        self.month += 1;
        self.report.months_run = self.month;
        self.report.final_cash_micro = self.cash_micro;
        self.report.final_total_position_micro = self.total_position_micro();
        Ok(())
    }

    pub fn run_months(&mut self, n: u32, mut gov: Option<&mut Governance>) -> Result<Report, String> {
        for _ in 0..n { self.run_month(gov.as_deref_mut())?; }
        Ok(self.report.clone())
    }
}

/// Cost of extracting `amount` above the per-period allowance: the penalty
/// door. Carlos's ruling: extreme by design, 12% floor, a deterrent.
pub fn excess_extraction_cost_micro(cfg: &Config, amount_micro: i128) -> i128 {
    mul_ppm(amount_micro, cfg.penalty_ppm)
}

/// Cost of the borrow-and-default door: the dividend forfeited at the next
/// eligible slot. Which position it is computed on is D-3 — still open, and
/// the whole point of pricing both here.
pub fn default_door_cost_micro(cfg: &Config, position_micro: i128, annual_rate_ppm: i128) -> i128 {
    match cfg.forfeiture_base {
        ForfeitureBase::PreDefault => mul_ppm(position_micro, annual_rate_ppm),
        ForfeitureBase::PostDefault => {
            let loan = mul_ppm(position_micro, cfg.lambda_ppm);
            mul_ppm(position_micro - loan, annual_rate_ppm)
        }
    }
}
