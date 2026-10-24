//! Route reconstruction: stitching events into end-to-end workflows.
//!
//! [`reconstruct`] walks the ingested events in causal order and greedily
//! appends each event to the most recent open workflow whose last hop's `to`
//! equals the event's `from` (a hop continuation — the receiving wallet is
//! the sending wallet of the next leg, whether the mint changed across the
//! leg, as in a swap route, or not). A workflow whose next hop returns the
//! *same mint* to its origin wallet is a loop: it is collapsed into that
//! single workflow and flagged [`Workflow::is_circular`] — the
//! manufactured-movement shape the classifier then judges. A *different*
//! mint arriving home is a completed route's output leg, not a loop.
//!
//! Deliberate limit: this is a greedy single pass, not a DAG assembler. A
//! fan-out (one wallet paying many) opens one workflow per branch; when two
//! open chains could both accept an event, the most recent wins. The
//! limitation is documented here and in the crate root, not hidden.

use crate::ingest::TransferEvent;

/// One reconstructed end-to-end workflow: a maximal chain of hop
/// continuations.
#[derive(Debug, Clone)]
pub struct Workflow {
    pub id: u32,
    /// The stitched hops, in causal order.
    pub hops: Vec<TransferEvent>,
    /// True when the chain returned the origin's own mint to the origin
    /// wallet — the manufactured-movement shape, collapsed into one
    /// workflow.
    pub is_circular: bool,
}

impl Workflow {
    /// The wallet that started the workflow.
    pub fn origin(&self) -> Option<&str> {
        self.hops.first().map(|h| h.from.as_str())
    }

    /// All wallets that appear in the workflow, in first-seen order.
    pub fn wallets(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for h in &self.hops {
            for w in [&h.from, &h.to] {
                if !out.iter().any(|x| x == w) {
                    out.push(w.clone());
                }
            }
        }
        out
    }
}

/// The net-effect summary: what the workflow *did*, before any label is
/// applied. This is what the headline leads with — plumbing (zero net
/// position change) is demoted below real position-taking here, in the data,
/// not just in the UI copy.
#[derive(Debug, Clone)]
pub struct NetEffect {
    /// (wallet, mint, net base units): +amount received, −amount sent.
    pub net_per_wallet_mint: Vec<(String, String, i128)>,
    pub total_fees_lamports: u64,
    pub slot_span: (u64, u64),
    pub time_span_ms: (u64, u64),
    pub wallet_count: usize,
    /// Wallets whose net across ALL mints is exactly zero — pure pass-through
    /// plumbing (venues, routers). Sorted for determinism.
    pub passthrough_wallets: Vec<String>,
}

impl NetEffect {
    pub fn of(wf: &Workflow) -> Self {
        let mut nets: Vec<(String, String, i128)> = Vec::new();
        let mut add = |wallet: &str, mint: &str, delta: i128| {
            if let Some(entry) = nets.iter_mut().find(|(w, m, _)| w == wallet && m == mint) {
                entry.2 += delta;
            } else {
                nets.push((wallet.to_string(), mint.to_string(), delta));
            }
        };
        let mut total_fees: u64 = 0;
        let mut min_slot = u64::MAX;
        let mut max_slot = 0u64;
        let mut min_t = u64::MAX;
        let mut max_t = 0u64;
        for h in &wf.hops {
            add(&h.from, &h.mint, -h.amount);
            add(&h.to, &h.mint, h.amount);
            total_fees = total_fees.saturating_add(h.fee_lamports);
            min_slot = min_slot.min(h.slot);
            max_slot = max_slot.max(h.slot);
            min_t = min_t.min(h.timestamp_ms);
            max_t = max_t.max(h.timestamp_ms);
        }
        nets.sort();
        // A wallet is pass-through when its all-mint total nets to zero.
        let mut totals: Vec<(String, i128)> = Vec::new();
        for (w, _, n) in &nets {
            if let Some(e) = totals.iter_mut().find(|(x, _)| x == w) {
                e.1 += *n;
            } else {
                totals.push((w.clone(), *n));
            }
        }
        let mut passthrough: Vec<String> =
            totals.into_iter().filter(|(_, n)| *n == 0).map(|(w, _)| w).collect();
        passthrough.sort();
        let wallets = wf.wallets();
        NetEffect {
            net_per_wallet_mint: nets,
            total_fees_lamports: total_fees,
            slot_span: (min_slot, max_slot),
            time_span_ms: (min_t, max_t),
            wallet_count: wallets.len(),
            passthrough_wallets: passthrough,
        }
    }

    /// Net of one wallet in one mint, 0 if absent.
    pub fn net_of(&self, wallet: &str, mint: &str) -> i128 {
        self.net_per_wallet_mint
            .iter()
            .find(|(w, m, _)| w == wallet && m == mint)
            .map(|(_, _, n)| *n)
            .unwrap_or(0)
    }
}

/// Stitch ingested events into workflows. Input must already be in causal
/// order (see [`crate::ingest::ingest`]).
pub fn reconstruct(events: &[TransferEvent]) -> Vec<Workflow> {
    let mut open: Vec<Workflow> = Vec::new();
    let mut next_id: u32 = 0;
    for e in events {
        // Find the most recent open workflow this event continues.
        let mut target: Option<usize> = None;
        for (i, wf) in open.iter().enumerate().rev() {
            if wf.is_circular {
                continue; // a closed loop accepts no more hops
            }
            if let Some(last) = wf.hops.last() {
                if last.to == e.from {
                    target = Some(i);
                    break;
                }
            }
        }
        match target {
            Some(i) => {
                open[i].hops.push(e.clone());
            }
            None => {
                let wf = Workflow { id: next_id, hops: vec![e.clone()], is_circular: false };
                next_id += 1;
                open.push(wf);
            }
        }
        // A same-mint return to the origin closes the loop: value came home,
        // the manufactured-movement shape. A *different* mint coming home is
        // a completed route (the swap's output leg), not a loop — cross-mint
        // round-trips are a W6.2 extension, not guessed here.
        if let Some(i) = target {
            if !open[i].is_circular {
                if let (Some(first), Some(origin)) =
                    (open[i].hops.first(), open[i].origin().map(str::to_string))
                {
                    if let Some(last) = open[i].hops.last() {
                        if last.to == origin && last.mint == first.mint {
                            open[i].is_circular = true;
                        }
                    }
                }
            }
        }
    }
    open
}
