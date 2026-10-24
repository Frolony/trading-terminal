//! Classification with confidence — and the unclassified guarantee.
//!
//! Each classifier is a pure function of `(workflow, dictionary-hits)` that
//! returns an optional `(Label, confidence, evidence-note)` triple. The
//! public entry point is [`classify`], which takes the highest-confidence
//! candidate and **seals** it through [`Verdict::seal`]: any candidate below
//! [`CONFIDENCE_THRESHOLD`] is demoted to [`Classification::Unclassified`]
//! with the reason stated. There is no public constructor for a labeled
//! verdict — a label the machine cannot justify is printed as a guess, never
//! as a fact. That is the whole claims discipline of S13, enforced in the
//! type, not in the UI copy.
//!
//! ## The threshold
//!
//! 0.70. A dictionary family-name-only match scores exactly 0.70
//! ([`crate::dictionary`]), so a lone name match can never label on its
//! own — a second independent signal (venue count, loop closure, treasury
//! program context) is required to clear the bar. Strong multi-signal
//! candidates score 0.75–0.95.

use crate::dictionary::{Dictionary, HopKind, Venue, VenueHit};
use crate::reconstruct::{NetEffect, Workflow};

/// Below this confidence, a candidate is demoted to Unclassified.
pub const CONFIDENCE_THRESHOLD: f64 = 0.70;

/// The workflow labels of the S13 spec (§6 of the master plan). The
/// market-maker-inventory and incentive-farming classes landed in W6.1.3, and
/// the cross-mint round-trip reading in W6.1.3 as well.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Label {
    /// Multi-venue swap route: value passed through ≥2 venues; the
    /// intermediaries kept nothing (pass-through plumbing).
    AggregatorSplit,
    /// Liquidity added to / removed from a pool, no swap legs.
    LiquidityAddRemove,
    /// Circular flow returning value to the origin wallet (fees aside) —
    /// the manufactured-movement shape. The workflow is already collapsed
    /// to one by reconstruction; this labels it.
    WashLoop,
    /// Value moved across a bridge boundary (mint change at a bridge venue).
    BridgeHop,
    /// Funds left the user's wallet for a venue/program and did not return
    /// within the window: a real position was opened.
    PositionOpen,
    /// Funds returned from a venue/program to the user's wallet: a position
    /// was closed.
    PositionClose,
    /// A flow through our own treasury program (W4.1): enroll, tranche,
    /// borrow, default settlement, exit. The audit story for S12
    /// settlements, labeled as what it is.
    TreasuryTransition,
    /// A completed swap route: the origin wallet's funds came home in a
    /// *different* mint — the output leg of a route the reconstruction layer
    /// deliberately did NOT collapse as circular. Multi-venue routes keep the
    /// AggregatorSplit plumbing reading; this is the short / single-venue
    /// completed-swap shape.
    CrossMintRoundTrip,
    /// Active liquidity-inventory management: liquidity added AND removed in
    /// the same workflow (cycling), or a position managed with fee harvest.
    /// The more specific reading of the LiquidityAddRemove shape.
    MarketMaker,
    /// A liquidity position with repeated fee/reward collection (harvest).
    /// The weakest class by design: it requires the position AND the
    /// harvests — harvest evidence alone is plumbing until proven otherwise.
    IncentiveFarming,
}

/// A classification: either a justified label or an explicit guess.
#[derive(Debug, Clone)]
pub enum Classification {
    Labeled {
        label: Label,
        /// Always ≥ CONFIDENCE_THRESHOLD — enforced by [`Verdict::seal`].
        confidence: f64,
        /// Why the classifier believes it, in one sentence.
        rationale: String,
    },
    /// The machine's honest answer when it cannot justify a label.
    Unclassified {
        /// What was tried and why it fell short (e.g. "best candidate
        /// AggregatorSplit at 0.62, below the 0.70 threshold").
        reason: String,
    },
}

/// The full verdict for one workflow: classification + evidence + net
/// effect, ready for the headline renderer.
#[derive(Debug, Clone)]
pub struct Verdict {
    pub workflow_id: u32,
    pub classification: Classification,
    /// The underlying event signatures — the audit appendix.
    pub evidence: Vec<String>,
    pub net: NetEffect,
}

impl Verdict {
    /// Seal a candidate into a verdict. **This is the unclassified
    /// guarantee**: a candidate below [`CONFIDENCE_THRESHOLD`] cannot become
    /// a label through this constructor — it is demoted to Unclassified with
    /// the shortfall stated. Keep this the only path to a labeled verdict.
    pub fn seal(
        workflow_id: u32,
        candidate: Option<(Label, f64, String)>,
        evidence: Vec<String>,
        net: NetEffect,
    ) -> Self {
        let classification = match candidate {
            Some((label, confidence, rationale))
                if confidence >= CONFIDENCE_THRESHOLD =>
            {
                Classification::Labeled { label, confidence, rationale }
            }
            Some((label, confidence, rationale)) => Classification::Unclassified {
                reason: format!(
                    "best candidate {:?} at {:.2}, below the {:.2} threshold ({})",
                    label, confidence, CONFIDENCE_THRESHOLD, rationale
                ),
            },
            None => Classification::Unclassified {
                reason: "no classifier produced a candidate".to_string(),
            },
        };
        Verdict { workflow_id, classification, evidence, net }
    }

    /// True when the verdict is a guess, not a justified label.
    pub fn is_guess(&self) -> bool {
        matches!(self.classification, Classification::Unclassified { .. })
    }

    /// The label, if this verdict carries one.
    pub fn label(&self) -> Option<Label> {
        match &self.classification {
            Classification::Labeled { label, .. } => Some(*label),
            Classification::Unclassified { .. } => None,
        }
    }

    /// The confidence of a labeled verdict. Panics on Unclassified — callers
    /// must check [`Verdict::label`] first; a guess has no confidence worth
    /// quoting.
    pub fn confidence(&self) -> f64 {
        match &self.classification {
            Classification::Labeled { confidence, .. } => *confidence,
            Classification::Unclassified { .. } => {
                panic!("Unclassified verdict has no confidence to quote")
            }
        }
    }
}

fn recognize_all(dict: &Dictionary, wf: &Workflow) -> Vec<Option<VenueHit>> {
    wf.hops
        .iter()
        .map(|h| dict.recognize(&h.program_id, &h.instruction))
        .collect()
}

fn distinct_venues(hits: &[Option<VenueHit>]) -> Vec<Venue> {
    let mut out: Vec<Venue> = Vec::new();
    for h in hits.iter().flatten() {
        if !out.contains(&h.venue) {
            out.push(h.venue);
        }
    }
    out
}

/// Classify one workflow. Candidates are evaluated in priority order and
/// the first match wins — the shapes are specific enough that ordering is
/// more principled than a confidence horse race:
///
/// 1. [`Label::MarketMaker`] (closed unwind) — a closed loop whose legs are
///    all liquidity-management legs is inventory unwound home, not
///    manufactured movement. Evaluated before WashLoop so the more specific
///    reading wins; a loop with any swap leg, or any unrecognized leg,
///    falls through to WashLoop — the unwind arm never weakens wash
///    detection, it only carves out the shape wash cannot honestly claim.
/// 2. [`Label::WashLoop`] — a closed loop overrides every other reading of
///    the same hops; a loop that looks like routing is still a loop.
/// 3. [`Label::TreasuryTransition`] — our own program's flows are labeled
///    as what they are before venue heuristics get a say.
/// 4. [`Label::PositionClose`] — a workflow of removal legs only, all
///    returning funds to one wallet that sends nothing onward: a position
///    closed (or reduced — the stateless limit, see
///    [`position_close_candidate`]). Evaluated before PositionOpen because
///    the naive one-sided-net reading mislabels a close leg as an open.
/// 5. [`Label::PositionOpen`] (net reading) — a user wallet with a
///    one-sided outflow net answers the tracer's core question ("did anyone
///    take a real position?") directly. The inflow side of that reading is
///    claimed by the removal-leg arm above, so a close leg never lands here.
/// 6. [`Label::AggregatorSplit`] — multi-venue pass-through routing.
/// 7. [`Label::CrossMintRoundTrip`] — a completed swap whose output leg came
///    home in a different mint. Evaluated after AggregatorSplit so
///    multi-venue routes keep the plumbing reading; single-venue or short
///    completed swaps get the round-trip reading. Both can match; the order
///    is the overlap rule, stated here.
/// 8. [`Label::MarketMaker`] — inventory cycling or managed-with-harvest.
/// 9. [`Label::IncentiveFarming`] — position plus repeated harvest.
/// 10. [`Label::LiquidityAddRemove`] — pool liquidity moves with no swap legs.
///
/// The reported confidence still reflects the evidence strength; the order
/// only decides ties and overlaps. The sealed verdict enforces the
/// unclassified guarantee either way.
pub fn classify(dict: &Dictionary, wf: &Workflow) -> Verdict {
    let hits = recognize_all(dict, wf);
    let net = NetEffect::of(wf);
    let evidence: Vec<String> = wf.hops.iter().map(|h| h.signature.clone()).collect();

    let venues = distinct_venues(&hits);
    let has_venue = |v: Venue| venues.contains(&v);
    let hop_kinds: Vec<HopKind> = hits.iter().flatten().map(|h| h.hop).collect();
    // Index-aligned with wf.hops (None for unrecognized hops) — the
    // flattened hop_kinds above is for counting only.
    let hop_kinds_aligned: Vec<Option<HopKind>> =
        hits.iter().map(|h| h.map(|v| v.hop)).collect();
    let recognized = hits.iter().any(|h| h.is_some());

    let candidate: Option<(Label, f64, String)> = mm_unwind_candidate(&hop_kinds_aligned, wf)
        .or_else(|| wash_candidate(wf, &net))
        .or_else(|| treasury_candidate(wf, &hits, &has_venue))
        .or_else(|| position_close_candidate(&hop_kinds_aligned, wf, &venues))
        .or_else(|| position_candidate(wf, &net, &venues, recognized))
        .or_else(|| aggregator_candidate(wf, &net, &venues))
        .or_else(|| cross_mint_round_trip_candidate(wf, &net, &venues, &hop_kinds, recognized))
        .or_else(|| market_maker_candidate(&hop_kinds, wf, recognized))
        .or_else(|| incentive_farming_candidate(&hop_kinds_aligned, wf, recognized))
        .or_else(|| liquidity_candidate(&hop_kinds, wf));

    Verdict::seal(wf.id, candidate, evidence, net)
}

/// MarketMaker (closed unwind): the W6.2 stitcher-overlap resolution. A
/// circular workflow is, by construction (see [`crate::reconstruct`]), a
/// same-mint return home — but that shape has two readings, and the hop
/// kinds decide which one is honest. When every hop is a recognized
/// liquidity-management leg (add, remove, or a fee harvest home) with at
/// least one add AND at least one remove, the loop is a position that was
/// built and then fully unwound: inventory came home because the MM closed
/// it, not because value was manufactured. A loop with any swap leg, any
/// order/escrow/treasury leg, or any unrecognized leg is NOT provably an
/// unwind and falls through to [`wash_candidate`] — the arm is deliberately
/// narrow so it can never launder a wash loop. Confidence mirrors the
/// wash short-loop discount: 0.80 for a ≥3-leg unwind, 0.75 for the minimal
/// 2-leg add-and-remove.
fn mm_unwind_candidate(
    hop_kinds_aligned: &[Option<HopKind>],
    wf: &Workflow,
) -> Option<(Label, f64, String)> {
    if !wf.is_circular || wf.hops.is_empty() {
        return None;
    }
    // Every leg must be recognized and a liquidity-management leg; one
    // unrecognized or swap leg voids the unwind reading.
    if hop_kinds_aligned.iter().any(|k| k.is_none()) {
        return None;
    }
    if hop_kinds_aligned.iter().any(|k| {
        !matches!(
            k,
            Some(HopKind::LiquidityAdd) | Some(HopKind::LiquidityRemove) | Some(HopKind::FeeCollect)
        )
    }) {
        return None;
    }
    let adds = hop_kinds_aligned
        .iter()
        .filter(|k| matches!(k, Some(HopKind::LiquidityAdd)))
        .count();
    let removes = hop_kinds_aligned
        .iter()
        .filter(|k| matches!(k, Some(HopKind::LiquidityRemove)))
        .count();
    if adds == 0 || removes == 0 {
        return None;
    }
    let origin = wf.origin()?;
    let conf = if adds + removes >= 3 { 0.80 } else { 0.75 };
    Some((
        Label::MarketMaker,
        conf,
        format!(
            "inventory unwound home: {} liquidity adds and {} removes closed the position at {}; no swap legs",
            adds, removes, origin
        ),
    ))
}

/// WashLoop: the circular, same-mint value-returning shape. By construction
/// (see [`crate::reconstruct`]) a circular workflow returned the origin's
/// own mint to the origin wallet — value came home, only fees burned.
///
/// Carve-out (resolved W6.2): a circular loop whose legs are all
/// liquidity-management legs is claimed by [`mm_unwind_candidate`] first —
/// an MM unwind is not manufactured movement. Wash keeps every other
/// circular loop: swap legs, unrecognized legs, and mixed shapes.
fn wash_candidate(wf: &Workflow, net: &NetEffect) -> Option<(Label, f64, String)> {
    if !wf.is_circular {
        return None;
    }
    let origin = wf.origin()?;
    let hops = wf.hops.len();
    if hops >= 3 {
        Some((
            Label::WashLoop,
            0.90,
            format!(
                "circular flow of {} hops returning the origin's mint to {}; intermediaries kept nothing; fees burned {} lamports",
                hops, origin, net.total_fees_lamports
            ),
        ))
    } else {
        Some((
            Label::WashLoop,
            0.72,
            format!(
                "circular flow of only {} hops returning to {}; short loops are weak evidence",
                hops, origin
            ),
        ))
    }
}

/// TreasuryTransition: any hop through our own program. The program's events
/// carry authorization context by design (W4.1 §6); the tracer labels the
/// flow as a treasury transition and leaves the authorization audit to the
/// receipt chain.
fn treasury_candidate(
    wf: &Workflow,
    hits: &[Option<VenueHit>],
    has_venue: &dyn Fn(Venue) -> bool,
) -> Option<(Label, f64, String)> {
    if !has_venue(Venue::TreasuryProgram) {
        return None;
    }
    let mut uniq: Vec<&str> = Vec::new();
    for (h, hit) in wf.hops.iter().zip(hits.iter()) {
        if hit.map(|x| x.venue == Venue::TreasuryProgram).unwrap_or(false)
            && !uniq.contains(&h.instruction.as_str())
        {
            uniq.push(h.instruction.as_str());
        }
    }
    uniq.sort();
    Some((
        Label::TreasuryTransition,
        0.90,
        format!("treasury-program flow through instructions: {}", uniq.join(", ")),
    ))
}

/// PositionClose (removal-leg arm): the W6.2 dead-arm resolution. The
/// stitcher defines a workflow's origin as its first hop's `from`, so a
/// close leg's workflow starts AT the pool — and the net reading below
/// would then mislabel the pool's outflow as a PositionOpen. The
/// instruction semantics rescue the reading: a workflow whose every leg
/// is a recognized liquidity-removal instruction (`decreaseLiquidity`,
/// `closePosition`, …), all landing at one wallet that sends nothing
/// onward in this window, is a position being taken OUT, not put in —
/// whatever wallet the stitcher calls the origin. Guards, each load-bearing:
/// non-circular (a loop is wash or an unwind, decided above); at most one
/// venue (one position, not a route); every leg recognized and a removal
/// (any swap, add, harvest, or unrecognized leg voids the arm — a
/// removal routed onward is routing, not a terminal close, and a
/// removal-plus-add is inventory management, decided below); one receiver
/// that never sends. Strictly removal-only: under the greedy stitcher a
/// close's harvest legs do not chain into this workflow (they arrive from
/// the pool, not from the receiver), so no harvest allowance is pretended
/// here — a close-with-harvest transaction splits into a close workflow
/// and a harvest workflow, and the harvest-alone reading is a separate
/// known quirk, not folded into this arm.
/// Stated limit (stateless): this arm cannot distinguish a FULL close from
/// a PARTIAL removal — both are removal legs to one wallet. Full-close
/// certainty needs cross-window position state (an open ledger carried
/// across windows), which this stateless classifier does not keep. The
/// label therefore means "removal legs returned funds to this wallet", at
/// 0.75, not "this wallet's position is provably zero". The gate still
/// holds on real labeled mainnet routes; this limit is the next W6.2 item
/// after them, not a reason to leave the arm dead.
fn position_close_candidate(
    hop_kinds_aligned: &[Option<HopKind>],
    wf: &Workflow,
    venues: &[Venue],
) -> Option<(Label, f64, String)> {
    if wf.is_circular || wf.hops.is_empty() || venues.len() > 1 {
        return None;
    }
    if hop_kinds_aligned.iter().any(|k| k.is_none()) {
        return None;
    }
    if hop_kinds_aligned
        .iter()
        .any(|k| !matches!(k, Some(HopKind::LiquidityRemove)))
    {
        return None;
    }
    let receiver = wf.hops.first()?.to.clone();
    if wf.hops.iter().any(|h| h.to != receiver) {
        return None;
    }
    if wf.hops.iter().any(|h| h.from == receiver) {
        return None;
    }
    let removes = hop_kinds_aligned.len();
    Some((
        Label::PositionClose,
        0.75,
        format!(
            "position removal legs returned funds to {receiver}: {removes} remove leg(s), no onward leg from the receiver in this window"
        ),
    ))
}

/// PositionOpen / PositionClose: a non-circular, at-most-one-venue flow
/// where the origin wallet has a one-sided net (outflow only = open, inflow
/// only = close) with no return leg. Requires at least one *recognized*
/// hop: a lone unrecognized transfer is a payment until proven otherwise.
/// The inflow side here is unreachable under the current stitcher, stated
/// plainly: a workflow's origin always sends its first hop, so the origin
/// can never have an inflow-only net — the branch is kept for symmetry
/// and documentation, and the removal-leg arm above is the live
/// PositionClose path, not this branch.
fn position_candidate(
    wf: &Workflow,
    net: &NetEffect,
    venues: &[Venue],
    recognized: bool,
) -> Option<(Label, f64, String)> {
    if !recognized || wf.is_circular || venues.len() > 1 || wf.hops.is_empty() {
        return None;
    }
    let origin = wf.origin()?;
    let mut opened: Vec<(String, i128)> = Vec::new();
    let mut closed: Vec<(String, i128)> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for (w, m, n) in &net.net_per_wallet_mint {
        if w != origin || seen.contains(m) {
            continue;
        }
        seen.push(m.clone());
        if *n < 0 {
            opened.push((m.clone(), -*n));
        } else if *n > 0 {
            closed.push((m.clone(), *n));
        }
    }
    if !opened.is_empty() && closed.is_empty() {
        let (mint, amt) = &opened[0];
        Some((
            Label::PositionOpen,
            0.75,
            format!("origin net outflow of {} {} to a venue with no return leg", amt, mint),
        ))
    } else if !closed.is_empty() && opened.is_empty() {
        let (mint, amt) = &closed[0];
        Some((
            Label::PositionClose,
            0.75,
            format!("origin net inflow of {} {} from a venue with no outbound leg", amt, mint),
        ))
    } else {
        None
    }
}

/// AggregatorSplit: ≥2 venues, ≥3 hops, not circular, intermediaries are
/// pass-through.
fn aggregator_candidate(
    wf: &Workflow,
    net: &NetEffect,
    venues: &[Venue],
) -> Option<(Label, f64, String)> {
    if wf.is_circular || venues.len() < 2 || wf.hops.len() < 3 {
        return None;
    }
    let conf = 0.80 + 0.05 * (venues.len().min(3) as f64 - 1.0)
        + 0.05 * (net.passthrough_wallets.len().min(2) as f64) / 2.0;
    Some((
        Label::AggregatorSplit,
        conf.min(0.92),
        format!(
            "{} hops across {} venues; {} pass-through wallets kept nothing",
            wf.hops.len(),
            venues.len(),
            net.passthrough_wallets.len()
        ),
    ))
}

/// CrossMintRoundTrip: the reconstruction layer's "completed route" shape —
/// the last hop returned to the origin wallet in a DIFFERENT mint than the
/// first hop's (reconstruction deliberately does not flag it circular; a
/// same-mint homecoming is the wash shape instead). The origin's net must
/// show the two-sided cross-mint position (outflow of mint A, inflow of mint
/// B) — a gift or a barter with strangers is not this, and an unrecognized
/// hop pair is a payment until proven otherwise, per the dictionary rule.
///
/// Two guards keep the reading honest: (a) the workflow must contain no
/// liquidity-position legs — a swap route does not open positions, and a
/// position-with-harvest flow that comes home cross-mint is the
/// [`Label::IncentiveFarming`] shape, not a completed swap; (b) the
/// multi-venue variant also matches [`Label::AggregatorSplit`], which fires
/// first by priority — this candidate is the completed-swap reading for
/// short or single-venue routes, where there is no pass-through plumbing to
/// describe instead.
fn cross_mint_round_trip_candidate(
    wf: &Workflow,
    net: &NetEffect,
    venues: &[Venue],
    hop_kinds: &[HopKind],
    recognized: bool,
) -> Option<(Label, f64, String)> {
    if !recognized || wf.is_circular || wf.hops.len() < 2 {
        return None;
    }
    if hop_kinds.iter().any(|k| {
        *k == HopKind::LiquidityAdd || *k == HopKind::LiquidityRemove
    }) {
        return None; // position legs — the harvest/close shape, not a swap
    }
    let origin = wf.origin()?;
    let first = wf.hops.first()?;
    let last = wf.hops.last()?;
    if last.to != origin || last.mint == first.mint {
        return None;
    }
    // The origin's net must be two-sided across the two mints: value went
    // out in one asset and came home in another.
    let out_a = net.net_of(origin, &first.mint) < 0;
    let in_b = net.net_of(origin, &last.mint) > 0;
    if !out_a || !in_b {
        return None;
    }
    let conf = if wf.hops.len() >= 3 || venues.len() >= 2 { 0.80 } else { 0.72 };
    Some((
        Label::CrossMintRoundTrip,
        conf,
        format!(
            "completed swap route: {} sent and {} came home across {} hops",
            first.mint, last.mint, wf.hops.len()
        ),
    ))
}

/// MarketMaker: active inventory management on a liquidity venue — adds AND
/// removes in the same workflow (cycling), or a position with exactly one
/// fee collection alongside ≥2 liquidity moves. The second arm is
/// deliberately narrow: two or more harvests make it
/// [`Label::IncentiveFarming`], not market making. Fires before the plain
/// liquidity reading so the more specific interpretation wins; a pure
/// one-sided add is still a [`Label::PositionOpen`] (evaluated earlier), and
/// add-only legs without any removal or harvest fall through to
/// [`Label::LiquidityAddRemove`).
fn market_maker_candidate(
    hop_kinds: &[HopKind],
    wf: &Workflow,
    recognized: bool,
) -> Option<(Label, f64, String)> {
    if !recognized || wf.is_circular {
        return None;
    }
    let adds = hop_kinds.iter().filter(|k| **k == HopKind::LiquidityAdd).count();
    let removes = hop_kinds.iter().filter(|k| **k == HopKind::LiquidityRemove).count();
    let collects = hop_kinds.iter().filter(|k| **k == HopKind::FeeCollect).count();
    if adds >= 1 && removes >= 1 && adds + removes >= 3 {
        Some((
            Label::MarketMaker,
            0.80,
            format!(
                "inventory cycling: {} liquidity adds and {} removes in one workflow",
                adds, removes
            ),
        ))
    } else if collects == 1 && adds + removes >= 2 {
        Some((
            Label::MarketMaker,
            0.75,
            format!(
                "managed position: {} liquidity moves with one fee collection",
                adds + removes
            ),
        ))
    } else {
        None
    }
}

/// IncentiveFarming: a liquidity position whose harvest is the point —
/// repeated fee/reward collections on the same position. Deliberately the
/// weakest class: it demands the position AND at least two harvests, with at
/// least one harvest landing back at the origin wallet (fees harvested to a
/// stranger's wallet are the harvester's shape, not the position owner's).
/// Harvest-only flows stay unclassified — fee plumbing until proven
/// otherwise.
fn incentive_farming_candidate(
    hop_kinds_aligned: &[Option<HopKind>],
    wf: &Workflow,
    recognized: bool,
) -> Option<(Label, f64, String)> {
    if !recognized || wf.is_circular {
        return None;
    }
    let origin = wf.origin()?;
    let liquidity = hop_kinds_aligned
        .iter()
        .filter(|k| matches!(k, Some(HopKind::LiquidityAdd) | Some(HopKind::LiquidityRemove)))
        .count();
    if liquidity == 0 {
        return None;
    }
    // Harvests are fee-collection hops landing back at the origin — the
    // position owner's shape, not a stranger's.
    let harvests_home = wf
        .hops
        .iter()
        .zip(hop_kinds_aligned.iter())
        .filter(|(h, k)| h.to == origin && matches!(k, Some(HopKind::FeeCollect)))
        .count();
    if harvests_home >= 2 {
        Some((
            Label::IncentiveFarming,
            0.75,
            format!(
                "position with {} repeated fee harvests back to the origin",
                harvests_home
            ),
        ))
    } else {
        None
    }
}

/// LiquidityAddRemove: liquidity hop kinds present, no swap legs, not
/// circular.
fn liquidity_candidate(hop_kinds: &[HopKind], wf: &Workflow) -> Option<(Label, f64, String)> {
    if wf.is_circular {
        return None;
    }
    let adds = hop_kinds.iter().filter(|k| **k == HopKind::LiquidityAdd).count();
    let removes = hop_kinds.iter().filter(|k| **k == HopKind::LiquidityRemove).count();
    let swaps = hop_kinds.iter().filter(|k| **k == HopKind::SwapLeg).count();
    if adds + removes == 0 || swaps > 0 {
        return None;
    }
    let action = if adds >= removes { "added to" } else { "removed from" };
    Some((
        Label::LiquidityAddRemove,
        0.80,
        format!(
            "liquidity {} a pool ({} add / {} remove hops), no swap legs",
            action, adds, removes
        ),
    ))
}
