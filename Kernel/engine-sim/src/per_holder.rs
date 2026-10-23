//! Per-holder (integer) execution mode for the treasury engine (W3.1 polish).
//!
//! The aggregate engine ([`crate::Engine`]) models cohorts as exact ratios —
//! holder counts scaled by parts-per-million, the logic document's "pure
//! percentage" formulation. This module runs the *same mechanics* one integer
//! holder at a time, so the projection model's integer-cohort worked examples
//! can be reproduced exactly (83,333–83,334 holders per cohort) and the W3.2
//! adversarial suite can reason about real indivisible holders — an attacker's
//! $50 position is one position, not 50 million parts-per-million of one.
//!
//! ## Integer-mode rounding rules (all documented, all conservative)
//!
//! Every indivisible-unit quantity is floored toward the treasury: a
//! fractional micro-dollar of dividend never leaves, a fractional borrower
//! never borrows, a fractional defaulter never defaults. The floor losses are
//! bounded (≤ 1 micro-dollar per holder per floored step) and the dividend
//! obligation is book-kept on the floored per-holder dividend, so the books
//! stay closed: `booked == cash_paid + reinvested`, exactly.
//!
//! Cohort split: `n = holders / 12`, the first `holders % 12` cohorts take one
//! extra holder each — the 1M-holder reference becomes four cohorts of 83,334
//! and eight of 83,333, against the projection model's rounded 83,333 figure.
//!
//! A second, compounding flooring effect is inherent to indivisible holders:
//! each window splits every segment into reinvestors/extractors by floor, so
//! up to one holder per segment per window falls on the extract side instead
//! of the reinvest side. Segments double each year of reinvestment, so the
//! shortfall vs the continuous-50% geometric line grows ~doubling (measured:
//! −$25 / −$77 / −$188 / −$408 / −$635 over 5y at reference scale, 5.4 ppm —
//! the integer-truthful answer, not a math error). Test T9(d) documents both
//! figures rather than rounding the rule away to match the line.
//!
//! The monthly cadence, Carlos's beginning-of-month dividend timing, the
//! extract-or-reinvest window rule, position-settled defaults with forgiven
//! interest, and the Kernel gating discipline are identical to the aggregate
//! engine; any drift between the two modes beyond the documented flooring
//! band is a bug in one of them (see test T9).

use super::{Config, Report, PPM};
use crate::governance::Governance;

/// An integer group of identical holders inside a cohort.
#[derive(Debug, Clone)]
struct HolderSegment {
    holders: u64,
    position_micro: i128,
    skip_next_dividend: bool,
}

#[derive(Debug, Clone)]
struct HolderLoanBatch {
    maturity_month: u32,
    cohort: usize,
    position_micro: i128,
    borrowers: u64,
    principal_micro: i128,
}

/// The deterministic monthly engine in per-holder (integer) mode.
pub struct HolderEngine {
    cfg: Config,
    month: u32,
    cohorts: Vec<Vec<HolderSegment>>,
    batches: Vec<HolderLoanBatch>,
    loans_outstanding_micro: i128,
    cash_micro: i128,
    year_start_total_micro: i128,
    annual_distributable_micro: i128,
    annual_rate_ppm: i128,
    paid_this_year_micro: i128,
    report: Report,
}

impl HolderEngine {
    pub fn new(cfg: Config) -> HolderEngine {
        let total = cfg.initial_treasury_micro();
        let base = cfg.holders / 12;
        let extra = cfg.holders % 12;
        let mut cohorts = Vec::with_capacity(12);
        for i in 0..12u64 {
            let n = base + if i < extra { 1 } else { 0 };
            cohorts.push(vec![HolderSegment {
                holders: n,
                position_micro: cfg.position_micro,
                skip_next_dividend: false,
            }]);
        }
        let mut e = HolderEngine {
            cash_micro: total * cfg.cash_ppm / PPM,
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

    /// Total holders across all cohorts (exact by construction).
    pub fn total_holders(&self) -> u64 {
        self.cohorts.iter().flat_map(|c| c.iter()).map(|s| s.holders).sum()
    }

    /// Holder count of one cohort (for the integer-split check).
    pub fn cohort_holders(&self, cohort: usize) -> u64 {
        self.cohorts[cohort].iter().map(|s| s.holders).sum()
    }

    fn total_position_micro(&self) -> i128 {
        self.cohorts.iter().flat_map(|c| c.iter())
            .map(|s| s.holders as i128 * s.position_micro).sum()
    }

    fn begin_year(&mut self) {
        self.year_start_total_micro = self.total_position_micro();
        self.annual_distributable_micro =
            self.cfg.annual_distributable_micro(self.year_start_total_micro);
        self.annual_rate_ppm = if self.year_start_total_micro > 0 {
            // Rounded to the nearest ppm (same rule as the aggregate engine:
            // floor truncation compounds a systematic downward bias).
            (self.annual_distributable_micro * PPM + self.year_start_total_micro / 2)
                / self.year_start_total_micro
        } else { 0 };
        self.paid_this_year_micro = 0;
    }

    pub fn cash_micro(&self) -> i128 { self.cash_micro }
    pub fn annual_rate_ppm(&self) -> i128 { self.annual_rate_ppm }

    /// Run one month — the same four phases as the aggregate engine, in
    /// integer arithmetic. Governance gating is identical: the cash-paid part
    /// of the tranche and each lending batch must commit as a Kernel
    /// transition; a refused commit blocks that flow for the month.
    pub fn run_month(&mut self, gov: Option<&mut Governance>) -> Result<(), String> {
        let mut gov = gov;
        if self.month > 0 && self.month.is_multiple_of(12) { self.begin_year(); }
        let cohort_idx = (self.month % 12) as usize;

        // ---- 1. Dividend tranche at the cohort's month start.
        let mut booked: i128 = 0;
        let mut cash_payout: i128 = 0;
        let mut growers: Vec<HolderSegment> = Vec::new();
        for s in &mut self.cohorts[cohort_idx] {
            if !s.skip_next_dividend {
                // Floored per holder: a fractional micro-dollar of dividend
                // cannot be paid to an indivisible holder.
                let div_per_holder = s.position_micro * self.annual_rate_ppm / PPM;
                booked += s.holders as i128 * div_per_holder;
                let rei = s.holders as i128 * self.cfg.reinvest_ppm / PPM;
                let rei = u64::try_from(rei).map_err(|_| "reinvestor overflow".to_string())?;
                if rei > 0 {
                    growers.push(HolderSegment {
                        holders: rei,
                        position_micro: s.position_micro + div_per_holder,
                        skip_next_dividend: false,
                    });
                }
                s.holders -= rei;
                cash_payout += s.holders as i128 * div_per_holder;
            }
            s.skip_next_dividend = false;
        }
        self.cohorts[cohort_idx].retain(|s| s.holders > 0);
        self.cohorts[cohort_idx].extend(growers);
        if booked > 0 {
            let mut authorized = true;
            let mut auth = None;
            if let Some(g) = gov.as_deref_mut() {
                let remaining = self.annual_distributable_micro - self.paid_this_year_micro;
                match g.authorize(kernel_policy::RES_TREASURY_TRANCHE, cash_payout, remaining, self.month, 1)? {
                    Some(a) => auth = Some(a),
                    None => authorized = false,
                }
            }
            if authorized {
                let paid = cash_payout.min(self.cash_micro);
                self.cash_micro -= paid;
                self.report.dividends_paid_micro += paid;
                self.report.dividend_shortfall_micro += cash_payout - paid;
                self.paid_this_year_micro += booked;
                if paid == cash_payout { self.report.full_tranches_paid += 1; }
                if let (Some(g), Some(a)) = (gov.as_deref_mut(), auth) {
                    if a.is_real() {
                        g.settle(&a)?;
                        self.report.settled_transitions += 1;
                    }
                }
            } else {
                self.report.blocked_flows += 1;
            }
        }

        // ---- 2. Maturing loans settle.
        let maturing: Vec<HolderLoanBatch> = self.batches.iter()
            .filter(|b| b.maturity_month == self.month).cloned().collect();
        self.batches.retain(|b| b.maturity_month != self.month);
        for b in maturing {
            let loan_per = b.position_micro * self.cfg.lambda_ppm / PPM;
            let defaulters = b.borrowers as i128 * self.cfg.delinquency_ppm / PPM;
            let defaulters = u64::try_from(defaulters).map_err(|_| "defaulter overflow".to_string())?;
            let performers = b.borrowers - defaulters;
            let performers_principal = performers as i128 * loan_per;
            let interest = performers_principal * self.cfg.loan_rate_ppm / PPM;
            self.cash_micro += performers_principal + interest;
            self.report.lending_interest_micro += interest;
            self.loans_outstanding_micro -= b.principal_micro;
            // Default: principal settles against the locked position; interest
            // forgiven; the defaulter skips their next airdrop. The batch
            // matures before the cohort's next window, so the segment
            // positions it was originated from are still intact.
            if defaulters > 0 {
                let cohort = &mut self.cohorts[b.cohort];
                let seg = cohort.iter_mut()
                    .find(|s| s.position_micro == b.position_micro && !s.skip_next_dividend)
                    .expect("loan batch's segment must still exist before the cohort's next window");
                debug_assert!(seg.holders >= defaulters, "defaulters cannot exceed borrowers");
                seg.holders -= defaulters;
                let reduced = b.position_micro - loan_per;
                if let Some(seg) = cohort.iter_mut()
                    .find(|s| s.position_micro == reduced && s.skip_next_dividend) {
                    seg.holders += defaulters;
                } else {
                    cohort.push(HolderSegment { holders: defaulters, position_micro: reduced, skip_next_dividend: true });
                }
            }
        }

        // ---- 3. This cohort borrows at its window.
        let mut principal: i128 = 0;
        let mut batch_borrowers: Vec<(i128, u64)> = Vec::new();
        for s in &self.cohorts[cohort_idx] {
            let borrowers = u64::try_from(s.holders as i128 * self.cfg.uptake_ppm / PPM)
                .map_err(|_| "borrower overflow".to_string())?;
            if borrowers > 0 {
                let loan_per = s.position_micro * self.cfg.lambda_ppm / PPM;
                principal += borrowers as i128 * loan_per;
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
                    let loan_per = position * self.cfg.lambda_ppm / PPM;
                    self.batches.push(HolderLoanBatch {
                        maturity_month: self.month + self.cfg.term_months,
                        cohort: cohort_idx,
                        position_micro: position,
                        borrowers,
                        principal_micro: borrowers as i128 * loan_per,
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
