//! Blob-collapse judgment: the S13 blob-rendering rule, as code.
//!
//! Carlos's direction (2026-10-24 ~02:26, filed as the §6 S13 addendum in
//! the master plan): the tracer's visualization unit is the workflow, not
//! the wallet. Plumbing hops collapse into ONE blob displaying net effect
//! only; raw hops demote to the audit appendix. A new blob spawns only at
//! a genuine entrance or exit. And a collapse is itself a classification
//! under S13's confidence discipline: sub-threshold merges render as
//! separate unclassified blobs, never a guessed merge.
//!
//! [`collapse_decision`] is the collapse half of that rule, as a pure
//! function of one stitched workflow. A workflow collapses into one blob
//! only when EVERY gate passes:
//!
//! 1. **Not circular.** A loop is manufactured movement, never plumbing to
//!    hide — a circular workflow never collapses, whatever else holds.
//! 2. **At least 2 hops.** A single leg is not a collapse.
//! 3. **Single mint** across all hops (same asset; an asset change is an
//!    exit boundary, not plumbing).
//! 4. **No recognized venue hop.** Any dictionary hit — swap, liquidity,
//!    order, treasury, even `wrapSol`/`unwrapSol` (ProtocolOps) — means the
//!    workflow does venue work with its own meaning; silent pass-through
//!    has no venue signal at all. A wrap/unwrap leg is plumbing with a
//!    name, not anonymous plumbing, so it blocks collapse deliberately.
//! 5. **Value conserved within tolerance.** Skim = origin outflow − final
//!    inflow; it must satisfy `0 ≤ skim ≤ skim_tolerance`, and every
//!    intermediate wallet's net must be `≥ 0` and `≤ skim_tolerance`
//!    (nobody extracted more than the tolerance; intermediates keeping
//!    the skim is the relay shape). Fees are excluded: hop amounts are
//!    value, `fee_lamports` is paid separately.
//! 6. **Single-use intermediates.** Each intermediate wallet appears in
//!    exactly 2 hops (one in, one out); the origin appears once (the head
//!    `from`), the final wallet once (the tail `to`). A reused
//!    intermediate is a persistent actor, not a burner hop.
//! 7. **Single-operation cadence.** The workflow's time span must fit the
//!    caller-supplied `max_span_ms` window.
//! 8. **Unbroken linkage.** Each hop's `to` must equal the next hop's
//!    `from`. Reconstruction guarantees this for stitched workflows; the
//!    check stays so a hand-built workflow cannot smuggle a break in.
//!
//! Fail-closed: a failed gate never weakens the others — [`BlobDecision::Separate`]
//! carries ALL failed reasons, so the renderer can show why the merge was
//! refused instead of silently drawing two blobs. This module judges
//! collapse only; it labels nothing (see [`crate::classify`]) and it
//! decides nothing about entrances or exits — those are boundary
//! judgments over the workflow's neighborhood (see [`crate::boundary`]).

use crate::dictionary::Dictionary;
use crate::reconstruct::{NetEffect, Workflow};

/// Caller-supplied tolerances for the collapse gates.
///
/// Both are deployment judgments, not constants of the rule: the skim
/// tolerance is the largest value loss a merge may silently absorb
/// (fees/rent/dust only), and the span is what "single-operation cadence"
/// means in milliseconds for the window under study.
#[derive(Debug, Clone, Copy)]
pub struct BlobParams {
    /// Largest acceptable skim (origin outflow − final inflow) and largest
    /// acceptable single-intermediate net, in the mint's base units.
    pub skim_tolerance: i128,
    /// Largest acceptable workflow time span, in milliseconds.
    pub max_span_ms: u64,
}

/// The collapse verdict for one workflow.
#[derive(Debug, Clone)]
pub enum BlobDecision {
    /// The workflow renders as ONE blob: net effect only, raw hops in the
    /// audit appendix. `skim` is the measured value kept en route.
    Collapse {
        skim: i128,
        hops: usize,
        wallets: usize,
    },
    /// The workflow does NOT collapse: every failed gate, stated.
    Separate {
        reasons: Vec<String>,
    },
}

fn occurrences(wf: &Workflow, wallet: &str) -> usize {
    wf.hops
        .iter()
        .filter(|h| h.from == wallet || h.to == wallet)
        .count()
}

/// Judge whether one workflow collapses into a single blob.
pub fn collapse_decision(dict: &Dictionary, wf: &Workflow, params: &BlobParams) -> BlobDecision {
    let mut reasons: Vec<String> = Vec::new();

    if wf.is_circular {
        reasons.push(
            "workflow is circular; a loop is manufactured movement, never plumbing to collapse"
                .to_string(),
        );
    }
    if wf.hops.len() < 2 {
        reasons.push(format!(
            "fewer than 2 hops ({}); a single leg is not a collapse",
            wf.hops.len()
        ));
    }
    if wf.hops.is_empty() {
        return BlobDecision::Separate { reasons };
    }

    let first_mint = wf.hops[0].mint.as_str();
    if wf.hops.iter().any(|h| h.mint != first_mint) {
        reasons.push("hops span more than one mint; an asset change is an exit boundary".to_string());
    }
    for (i, pair) in wf.hops.windows(2).enumerate() {
        if pair[0].to != pair[1].from {
            reasons.push(format!(
                "hop linkage broken at index {} ({} does not continue to {})",
                i, pair[0].to, pair[1].from
            ));
        }
    }
    for (i, h) in wf.hops.iter().enumerate() {
        if dict.recognize(&h.program_id, &h.instruction).is_some() {
            reasons.push(format!(
                "hop {} carries a recognized venue signal ({}); venue work is not silent plumbing",
                i, h.instruction
            ));
        }
    }

    let origin = wf.origin().unwrap_or_default().to_string();
    let final_wallet = wf.hops.last().map(|h| h.to.clone()).unwrap_or_default();
    if origin == final_wallet {
        reasons.push(
            "workflow returns to its origin without the circular flag; not collapsible as plumbing"
                .to_string(),
        );
    } else {
        let net = NetEffect::of(wf);
        let outflow = -net.net_of(&origin, first_mint);
        let inflow = net.net_of(&final_wallet, first_mint);
        let skim = outflow - inflow;
        if skim < 0 {
            reasons.push(format!(
                "value created: final inflow ({}) exceeds origin outflow ({})",
                inflow, outflow
            ));
        } else if skim > params.skim_tolerance {
            reasons.push(format!(
                "skim {} exceeds tolerance {}",
                skim, params.skim_tolerance
            ));
        }
        for w in wf.wallets() {
            if w == origin || w == final_wallet {
                continue;
            }
            let n = net.net_of(&w, first_mint);
            if n < 0 {
                reasons.push(format!(
                    "intermediate {} net negative ({}); it injected value, not relayed it",
                    w, n
                ));
            } else if n > params.skim_tolerance {
                reasons.push(format!(
                    "intermediate {} kept {} above tolerance {}",
                    w, n, params.skim_tolerance
                ));
            }
        }
    }

    let net = NetEffect::of(wf);
    let (t0, t1) = net.time_span_ms;
    if t1 > t0 && t1 - t0 > params.max_span_ms {
        reasons.push(format!(
            "time span {} ms exceeds single-operation cadence {} ms",
            t1 - t0,
            params.max_span_ms
        ));
    }

    for w in wf.wallets() {
        let expected = if w == origin || w == final_wallet { 1 } else { 2 };
        let count = occurrences(wf, &w);
        if count != expected {
            reasons.push(format!(
                "wallet {} appears {} times; single-use requires {}",
                w, count, expected
            ));
        }
    }

    if reasons.is_empty() {
        let net = NetEffect::of(wf);
        let skim = -net.net_of(&origin, first_mint) - net.net_of(&final_wallet, first_mint);
        BlobDecision::Collapse { skim, hops: wf.hops.len(), wallets: wf.wallets().len() }
    } else {
        BlobDecision::Separate { reasons }
    }
}

/// One plain-language line for the renderer: the blob, or why there is none.
pub fn blob_headline(decision: &BlobDecision, wf: &Workflow) -> String {
    match decision {
        BlobDecision::Collapse { skim, hops, wallets } => {
            let mint = wf.hops.first().map(|h| h.mint.as_str()).unwrap_or("?");
            format!(
                "One blob — {} hops, {} wallets, skim {} {} (net effect only; hops in appendix)",
                hops, wallets, skim, mint
            )
        }
        BlobDecision::Separate { reasons } => {
            format!("Separate blobs — merge refused: {}", reasons.join("; "))
        }
    }
}
