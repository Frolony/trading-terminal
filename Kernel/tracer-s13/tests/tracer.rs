//! W6.1 acceptance tests (T15–T21): the tracer's first verified increment.
//!
//! Every test feeds the tracer synthetic-but-shaped-like-reality event
//! streams and asserts the S13 contract: reconstruction stitches, the
//! dictionary recognizes, classification carries confidence AND evidence,
//! and the unclassified guarantee holds — no labeled verdict below 0.70.

use tracer_s13::classify::{classify, Label, Verdict, CONFIDENCE_THRESHOLD};
use tracer_s13::dictionary::{default_dictionary, HopKind, Venue};
use tracer_s13::ingest::{ingest, TransferEvent};
use tracer_s13::reconstruct::{reconstruct, NetEffect, Workflow};
use tracer_s13::{headline, reconstruct as rc};

#[allow(clippy::too_many_arguments)] // test-only event builder; arity is the point
fn ev(
    sig: &str,
    slot: u64,
    index: u32,
    from: &str,
    to: &str,
    mint: &str,
    amount: i128,
    program: &str,
    instruction: &str,
    fee: u64,
) -> TransferEvent {
    TransferEvent {
        signature: sig.to_string(),
        slot,
        index,
        timestamp_ms: 1_790_000_000_000 + slot * 400,
        from: from.to_string(),
        to: to.to_string(),
        mint: mint.to_string(),
        amount,
        program_id: program.to_string(),
        instruction: instruction.to_string(),
        fee_lamports: fee,
    }
}

fn run(events: Vec<TransferEvent>) -> Vec<tracer_s13::classify::Verdict> {
    let dict = default_dictionary();
    let ingested = ingest(events).expect("test events must ingest");
    reconstruct(&ingested)
        .iter()
        .map(|wf| classify(&dict, wf))
        .collect()
}

// T15 — aggregator split: 4 hops, 2 venues, intermediaries pass value
// through. Labels AggregatorSplit with confidence >= threshold, evidence
// attached, headline leads with the net effect.
#[test]
fn t15_aggregator_split_labeled() {
    let verdicts = run(vec![
        ev("sig0", 100, 0, "user1", "router1", "USDC", 1_000_000, "P_agg", "routeV2", 5000),
        ev("sig1", 100, 1, "router1", "book1", "USDC", 1_000_000, "P_book", "fillOrder", 5000),
        ev("sig2", 100, 2, "book1", "router1", "WSOL", 5_000_000, "P_book", "fillOrder", 5000),
        ev("sig3", 100, 3, "router1", "user1", "WSOL", 4_990_000, "P_agg", "proxySwap", 5000),
    ]);
    assert_eq!(verdicts.len(), 1, "the four legs are one workflow");
    let v = &verdicts[0];
    assert_eq!(v.label(), Some(Label::AggregatorSplit));
    assert!(v.confidence() >= CONFIDENCE_THRESHOLD);
    assert_eq!(v.evidence.len(), 4, "every hop is evidence");
    assert!(!v.is_guess());
    let h = headline(v);
    assert!(h.contains("Aggregator split"), "headline names the label: {h}");
    assert!(h.contains("4 hops"), "headline counts hops: {h}");
    assert!(h.contains("20000"), "headline totals fees (4x5000): {h}");
    // The user swapped; the router kept a 10_000 WSOL spread — the headline
    // must show real kept positions, not hide them.
    assert!(v.net.net_of("user1", "USDC") == -1_000_000);
    assert!(v.net.net_of("user1", "WSOL") == 4_990_000);
    assert!(v.net.net_of("router1", "WSOL") == 10_000);
}

// T16 — genuine position open: funds leave the user for a venue and do not
// return. Labels PositionOpen; the net outflow is the headline's lead.
#[test]
fn t16_position_open_labeled() {
    let verdicts = run(vec![
        ev("sig0", 200, 0, "user2", "pool1", "SOL", 2_000_000_000, "P_clmm", "increaseLiquidity", 5000),
    ]);
    assert_eq!(verdicts.len(), 1);
    let v = &verdicts[0];
    assert_eq!(v.label(), Some(Label::PositionOpen));
    assert!(v.confidence() >= CONFIDENCE_THRESHOLD);
    let h = headline(v);
    assert!(h.contains("Position opened"), "headline: {h}");
    assert!(v.net.net_of("user2", "SOL") == -2_000_000_000);
}

// T17 — wash loop: a circular flow returning value to the origin is
// collapsed into ONE workflow and labeled WashLoop (manufactured movement),
// not rendered as three impressive hops.
#[test]
fn t17_wash_loop_collapsed_and_labeled() {
    let dict = default_dictionary();
    let ingested = ingest(vec![
        ev("sig0", 300, 0, "A", "B", "USDC", 1_000_000, "P1", "swap", 5000),
        ev("sig1", 300, 1, "B", "C", "USDC", 999_000, "P2", "twoHopSwap", 5000),
        ev("sig2", 300, 2, "C", "A", "USDC", 998_000, "P3", "proxySwap", 5000),
    ])
    .unwrap();
    let wfs = reconstruct(&ingested);
    assert_eq!(wfs.len(), 1, "the loop collapses to a single workflow");
    assert!(wfs[0].is_circular, "the loop is flagged circular");
    let v = classify(&dict, &wfs[0]);
    let v = &v;
    assert_eq!(v.label(), Some(Label::WashLoop));
    assert!(v.confidence() >= 0.85, "a closed 3-hop loop is strong evidence");
    assert_eq!(v.evidence.len(), 3);
    let h = headline(v);
    assert!(h.contains("Wash loop"), "headline: {h}");
    // Value came home: A's net is the fee bleed only.
    assert_eq!(v.net.net_of("A", "USDC"), -2_000);
}

// T18 — the unclassified guarantee, two ways:
// (a) a lone unrecognized transfer is a payment until proven otherwise —
//     Unclassified, never a guessed PositionOpen;
// (b) Verdict::seal demotes a 0.69 candidate and keeps a 0.70 one.
#[test]
fn t18_unclassified_guarantee() {
    let verdicts = run(vec![
        ev("sig0", 400, 0, "user3", "friend9", "USDC", 50_000, "P_unknown", "mysteryIx", 5000),
    ]);
    assert_eq!(verdicts.len(), 1);
    let v = &verdicts[0];
    assert!(v.is_guess(), "an unrecognized lone transfer is a guess, not a label");
    assert_eq!(v.label(), None);
    let h = headline(v);
    assert!(h.contains("Unclassified"), "headline says so: {h}");
    assert!(h.contains("a guess, not a verdict"), "headline carries the posture: {h}");

    let wf = Workflow { id: 0, hops: vec![], is_circular: false };
    let net = NetEffect::of(&wf);
    let below = Verdict::seal(
        0,
        Some((Label::WashLoop, 0.69, "test candidate".to_string())),
        vec![],
        net.clone(),
    );
    assert!(below.is_guess(), "0.69 must not label");
    let at = Verdict::seal(
        0,
        Some((Label::WashLoop, 0.70, "test candidate".to_string())),
        vec![],
        net,
    );
    assert_eq!(at.label(), Some(Label::WashLoop), "0.70 clears the bar");
}

// T19 — net-effect accounting: fees sum, spans and wallet counts are exact,
// per-wallet nets balance (every send is someone's receive).
#[test]
fn t19_net_effect_accounting() {
    let dict = default_dictionary();
    let ingested = ingest(vec![
        ev("sig0", 500, 0, "u", "v", "M", 100, "P", "swap", 5000),
        ev("sig1", 502, 0, "v", "w", "M", 100, "P", "swap", 7000),
    ])
    .unwrap();
    let wfs = rc(&ingested);
    assert_eq!(wfs.len(), 1);
    let net = NetEffect::of(&wfs[0]);
    assert_eq!(net.total_fees_lamports, 12_000);
    assert_eq!(net.slot_span, (500, 502));
    assert_eq!(net.wallet_count, 3);
    assert_eq!(net.net_of("u", "M"), -100);
    assert_eq!(net.net_of("v", "M"), 0, "v passed everything through");
    assert_eq!(net.net_of("w", "M"), 100);
    assert!(net.passthrough_wallets.contains(&"v".to_string()));
    // Conservation: nets sum to zero per mint.
    let total: i128 = net.net_per_wallet_mint.iter().map(|(_, _, n)| *n).sum();
    assert_eq!(total, 0);
    let _ = dict;
}

// T20 — reconstruction: disjoint flows stay separate workflows; a hop that
// continues nothing opens a new one.
#[test]
fn t20_reconstruction_keeps_flows_separate() {
    let ingested = ingest(vec![
        ev("sig0", 600, 0, "a1", "b1", "M", 10, "P", "swap", 5000),
        ev("sig1", 600, 1, "x9", "y9", "M", 20, "P", "swap", 5000),
        ev("sig2", 600, 2, "b1", "c1", "M", 10, "P", "swap", 5000),
    ])
    .unwrap();
    let wfs = reconstruct(&ingested);
    assert_eq!(wfs.len(), 2, "two disjoint flows, two workflows");
    let chain = wfs.iter().find(|w| w.hops.len() == 2).expect("the a1 chain");
    assert_eq!(chain.origin(), Some("a1"));
}

// T21 — dictionary tiers: pinned program id scores 0.95, family-name-only
// scores exactly the threshold (never enough alone), unknown is None.
#[test]
fn t21_dictionary_tiers() {
    use tracer_s13::dictionary::{Dictionary, VenueEntry};
    let dict = Dictionary::new(vec![VenueEntry {
        program_id: Some("PinnedProgram111111111111111111111111111"),
        family: "routeV2",
        venue: Venue::AggregatorRoute,
        hop: HopKind::SwapLeg,
    }]);
    let pinned = dict.recognize("PinnedProgram111111111111111111111111111", "routeV2").unwrap();
    assert!((pinned.confidence - 0.95).abs() < 1e-12);
    assert!(pinned.pinned);
    let name_only = dict.recognize("SomeOtherProgram", "routeV2").unwrap();
    assert!((name_only.confidence - CONFIDENCE_THRESHOLD).abs() < 1e-12);
    assert!(!name_only.pinned);
    assert!(dict.recognize("P", "nope").is_none());

    // The shipped S11 table recognizes the v1/v3 families by name.
    let d = default_dictionary();
    let hit = d.recognize("anything", "processRotationalExtractionSlot").unwrap();
    assert_eq!(hit.venue, Venue::ControlledExit);
    let hit = d.recognize("anything", "pay_tranche").unwrap();
    assert_eq!(hit.venue, Venue::TreasuryProgram);
}
// ---------------------------------------------------------------------------
// W6.1.2 — attestation-timing detection (T22–T27, Carlos's 2026-10-22 addendum).
//
// Helpers for the (event, attestation) pairs the detectors judge.
// ---------------------------------------------------------------------------

use tracer_s13::attestation::{
    attest, attestation_headline, AttestationLabel, AttestationRecord, AttestationVerdict,
    CausalTime, RealizedEvent,
};

fn realized(id: &str, hash: &str, slot: u64, index: u32) -> RealizedEvent {
    RealizedEvent {
        id: id.to_string(),
        hash: hash.to_string(),
        realized_at: CausalTime::at(slot, index),
        signature: format!("event-sig-{id}"),
    }
}

fn attestation(
    attester: &str,
    event_id: &str,
    certified_hash: &str,
    certified_at: (u64, u32),
    signed_at: (u64, u32),
) -> AttestationRecord {
    AttestationRecord {
        attester: attester.to_string(),
        event_id: event_id.to_string(),
        certified_hash: certified_hash.to_string(),
        certified_at: CausalTime::at(certified_at.0, certified_at.1),
        signed_at: CausalTime::at(signed_at.0, signed_at.1),
        signature: format!("att-sig-{attester}-{event_id}"),
    }
}

// T22 — pre-certification: signed before the certified event existed. The
// verification-theater pattern — structural impossibility, not a judgment
// call. Primary label is PreCertification; the premature-claim finding is
// preserved in the appendix, not hidden.
#[test]
fn t22_precertification_detected() {
    let e = realized("ev-1", "h1", 1000, 0);
    let rec = attestation("attesterA", "ev-1", "h1", (900, 5), (900, 5));
    let v = attest(Some(&e), &rec);
    assert_eq!(v.label(), Some(AttestationLabel::PreCertification));
    assert!(v.confidence() >= CONFIDENCE_THRESHOLD);
    assert!(!v.is_guess());
    assert_eq!(v.evidence.len(), 2, "event + attestation signatures");
    // The backdated claim is a second, real finding — recorded, not lost.
    assert!(
        v.findings.iter().any(|f| f.contains("predates the realized outcome")),
        "premature-claim finding in the appendix: {:?}",
        v.findings
    );
    let h = attestation_headline(&v);
    assert!(h.contains("Pre-certification"), "headline names the anomaly: {h}");
    assert!(h.contains("attesterA"), "headline names the attester: {h}");
}

// T23 — post-dated: the signature actually issued after its own claimed
// certification time — "post dated the thing they were verifying."
#[test]
fn t23_postdated_detected() {
    let e = realized("ev-2", "h2", 1000, 0);
    // Claims it certified at (1100, 0), but the signature envelope is
    // (1200, 3) — after the event, but later than claimed.
    let rec = attestation("attesterB", "ev-2", "h2", (1100, 0), (1200, 3));
    let v = attest(Some(&e), &rec);
    assert_eq!(v.label(), Some(AttestationLabel::PostDated));
    assert!(v.confidence() >= CONFIDENCE_THRESHOLD);
    let h = attestation_headline(&v);
    assert!(h.contains("Post-dated attestation"), "headline: {h}");
}

// T24 — hash mismatch: timing is fine, but the certified hash is not the
// realized outcome's hash — settle-after-realized violated on the evidence.
#[test]
fn t24_hash_mismatch_detected() {
    let e = realized("ev-3", "h3", 1000, 0);
    let rec = attestation("attesterC", "ev-3", "hX", (1100, 2), (1100, 2));
    let v = attest(Some(&e), &rec);
    assert_eq!(v.label(), Some(AttestationLabel::HashMismatch));
    assert!(v.confidence() >= CONFIDENCE_THRESHOLD);
    let h = attestation_headline(&v);
    assert!(h.contains("Hash mismatch"), "headline: {h}");
    assert!(h.contains("hX"), "headline shows the wrong certified hash: {h}");
}

// T25 — valid attestation: signed after the realized outcome, claimed time
// inside the causal envelope, hash matches. Labeled ValidAttestation, not a
// guess — the class exonerates as well as accuses.
#[test]
fn t25_valid_attestation_labeled() {
    let e = realized("ev-4", "h4", 1000, 0);
    let rec = attestation("attesterD", "ev-4", "h4", (1100, 2), (1100, 2));
    let v = attest(Some(&e), &rec);
    assert_eq!(v.label(), Some(AttestationLabel::ValidAttestation));
    assert!(v.confidence() >= CONFIDENCE_THRESHOLD);
    assert!(!v.is_guess());
    assert!(v.findings.is_empty(), "no anomalies, no findings: {:?}", v.findings);
    let h = attestation_headline(&v);
    assert!(h.contains("Valid attestation"), "headline: {h}");
}

// T26 — unknown event: no realized event on record. Unclassified with the
// reason stated — the machine does NOT turn "cannot verify" into a fraud
// verdict.
#[test]
fn t26_unknown_event_is_unclassified_not_fraud() {
    let rec = attestation("attesterE", "ev-never-seen", "h9", (1100, 2), (1100, 2));
    let v = attest(None, &rec);
    assert_eq!(v.label(), None, "no label without a realized event");
    assert!(v.is_guess());
    assert_eq!(v.evidence.len(), 1, "only the attestation signature");
    let h = attestation_headline(&v);
    assert!(h.contains("Unclassified attestation"), "headline: {h}");
    assert!(h.contains("no realized event on record"), "reason stated: {h}");
    assert!(h.contains("not a fraud verdict"), "the no-fraud discipline is in the copy: {h}");
}

// T27 — the seal: a candidate below the threshold cannot become a label
// (0.69 → Unclassified with the shortfall stated; 0.70 keeps the label).
// Mirrors T18's seal test for workflows.
#[test]
fn t27_attestation_seal_demotes_subthreshold() {
    let low = AttestationVerdict::seal(
        "ev-6".to_string(),
        "attesterF".to_string(),
        Some((AttestationLabel::PostDated, 0.69, "thin envelope evidence".to_string())),
        vec![],
        vec!["sig".to_string()],
    );
    assert_eq!(low.label(), None);
    assert!(low.is_guess());
    let h = attestation_headline(&low);
    assert!(h.contains("0.69"), "shortfall stated: {h}");
    assert!(h.contains("below the 0.70 threshold"), "threshold named: {h}");

    let at_edge = AttestationVerdict::seal(
        "ev-6".to_string(),
        "attesterF".to_string(),
        Some((AttestationLabel::PostDated, 0.70, "edge".to_string())),
        vec![],
        vec!["sig".to_string()],
    );
    assert_eq!(at_edge.label(), Some(AttestationLabel::PostDated));
}
