//! W6.2 (sixth pass) — blob-collapse judgment tests.
//!
//! The S13 blob-rendering rule (Carlos, 2026-10-24 ~02:26) as executable
//! checks: a workflow collapses into ONE blob only when every gate in
//! `src/blob.rs` passes, and a refused merge names every failed gate.
//! Fixtures here are synthetic workflow shapes, in the same pattern as the
//! rest of the tracer suite; the real-mainnet-route gate item is separate
//! (see `labeled_validation.rs` header) and is NOT claimed by these tests.
//! Amounts are base units of one mint; "transfer" is deliberately not a
//! dictionary family, so the legs carry no venue signal.

use tracer_s13::blob::{blob_headline, collapse_decision, BlobDecision, BlobParams};
use tracer_s13::dictionary::default_dictionary;
use tracer_s13::ingest::{ingest, TransferEvent};
use tracer_s13::reconstruct::{reconstruct, Workflow};

#[allow(clippy::too_many_arguments)] // test-only event builder; arity is the point
fn ev(
    sig: &str,
    slot: u64,
    from: &str,
    to: &str,
    mint: &str,
    amount: i128,
    program: &str,
    instruction: &str,
) -> TransferEvent {
    TransferEvent {
        signature: sig.to_string(),
        slot,
        index: 0,
        timestamp_ms: 1_790_000_000_000 + slot * 400,
        from: from.to_string(),
        to: to.to_string(),
        mint: mint.to_string(),
        amount,
        program_id: program.to_string(),
        instruction: instruction.to_string(),
        fee_lamports: 5000,
    }
}

fn params() -> BlobParams {
    BlobParams { skim_tolerance: 10_000, max_span_ms: 60_000 }
}

/// A linear 3-hop relay: A→B→C→D, 5,000 kept per hop, 10,000 total skim.
fn relay_events() -> Vec<TransferEvent> {
    vec![
        ev("r0", 100, "A", "B", "WSOL", 1_000_000, "sys", "transfer"),
        ev("r1", 101, "B", "C", "WSOL", 995_000, "sys", "transfer"),
        ev("r2", 102, "C", "D", "WSOL", 990_000, "sys", "transfer"),
    ]
}

fn decide(events: Vec<TransferEvent>) -> BlobDecision {
    let dict = default_dictionary();
    let ingested = ingest(events).expect("fixture events valid");
    let wfs = reconstruct(&ingested);
    assert_eq!(wfs.len(), 1, "fixture must stitch into exactly one workflow");
    collapse_decision(&dict, &wfs[0], &params())
}

fn reasons(d: &BlobDecision) -> Vec<String> {
    match d {
        BlobDecision::Separate { reasons } => reasons.clone(),
        BlobDecision::Collapse { .. } => panic!("expected Separate, got Collapse"),
    }
}

#[test]
fn blob_relay_collapses_with_exact_skim() {
    let d = decide(relay_events());
    match &d {
        BlobDecision::Collapse { skim, hops, wallets } => {
            assert_eq!(*skim, 10_000);
            assert_eq!(*hops, 3);
            assert_eq!(*wallets, 4);
        }
        BlobDecision::Separate { reasons } => panic!("relay should collapse: {:?}", reasons),
    }
    let ingested = ingest(relay_events()).unwrap();
    let wfs = reconstruct(&ingested);
    let headline = blob_headline(&d, &wfs[0]);
    assert!(headline.contains("One blob"), "headline: {headline}");
    assert!(headline.contains("10000"), "headline carries the skim: {headline}");
}

#[test]
fn blob_value_leak_beyond_tolerance_stays_separate() {
    let mut events = relay_events();
    events[2].amount = 900_000; // final inflow 90,000 short: skim 100,000 > 10,000
    let d = decide(events);
    let r = reasons(&d);
    assert!(r.iter().any(|x| x.contains("skim")), "reasons: {r:?}");
}

#[test]
fn blob_circular_loop_never_collapses() {
    // A→B→A, same mint: reconstruction flags the loop; collapse must refuse.
    let events = vec![
        ev("c0", 200, "A", "B", "WSOL", 1_000_000, "sys", "transfer"),
        ev("c1", 201, "B", "A", "WSOL", 999_000, "sys", "transfer"),
    ];
    let d = decide(events);
    let r = reasons(&d);
    assert!(r.iter().any(|x| x.contains("circular")), "reasons: {r:?}");
}

#[test]
fn blob_recognized_venue_leg_blocks_collapse() {
    let mut events = relay_events();
    events[1].instruction = "swap".to_string();
    events[1].program_id = "P1".to_string();
    let d = decide(events);
    let r = reasons(&d);
    assert!(r.iter().any(|x| x.contains("venue signal")), "reasons: {r:?}");
}

#[test]
fn blob_reused_intermediate_stays_separate() {
    // B appears four times (in, out, in, out): a persistent actor, not a burner hop.
    let events = vec![
        ev("u0", 300, "A", "B", "WSOL", 1_000_000, "sys", "transfer"),
        ev("u1", 301, "B", "C", "WSOL", 995_000, "sys", "transfer"),
        ev("u2", 302, "C", "B", "WSOL", 990_000, "sys", "transfer"),
        ev("u3", 303, "B", "D", "WSOL", 985_000, "sys", "transfer"),
    ];
    let d = decide(events);
    let r = reasons(&d);
    assert!(r.iter().any(|x| x.contains("single-use")), "reasons: {r:?}");
}

#[test]
fn blob_single_leg_is_not_a_collapse() {
    let events = vec![ev("s0", 400, "A", "B", "WSOL", 1_000_000, "sys", "transfer")];
    let d = decide(events);
    let r = reasons(&d);
    assert!(r.iter().any(|x| x.contains("fewer than 2 hops")), "reasons: {r:?}");
}

#[test]
fn blob_mixed_mints_stay_separate() {
    let mut events = relay_events();
    events[2].mint = "USDC".to_string();
    let d = decide(events);
    let r = reasons(&d);
    assert!(r.iter().any(|x| x.contains("more than one mint")), "reasons: {r:?}");
}

#[test]
fn blob_broken_linkage_stays_separate() {
    // Hand-built workflow: second hop does not continue the first.
    let dict = default_dictionary();
    let wf = Workflow {
        id: 0,
        hops: vec![
            ev("k0", 500, "A", "B", "WSOL", 1_000_000, "sys", "transfer"),
            ev("k1", 501, "C", "D", "WSOL", 995_000, "sys", "transfer"),
        ],
        is_circular: false,
    };
    let d = collapse_decision(&dict, &wf, &params());
    let r = reasons(&d);
    assert!(r.iter().any(|x| x.contains("linkage broken")), "reasons: {r:?}");
}

#[test]
fn blob_slow_cadence_stays_separate() {
    // Slots 100 apart = ~40,000,000 ms span at the fixture clock, over the 60,000 cap.
    let events = vec![
        ev("t0", 600, "A", "B", "WSOL", 1_000_000, "sys", "transfer"),
        ev("t1", 600 + 100_000, "B", "C", "WSOL", 995_000, "sys", "transfer"),
        ev("t2", 600 + 200_000, "C", "D", "WSOL", 990_000, "sys", "transfer"),
    ];
    let d = decide(events);
    let r = reasons(&d);
    assert!(r.iter().any(|x| x.contains("cadence")), "reasons: {r:?}");
}

#[test]
fn blob_intermediate_injecting_value_stays_separate() {
    // B forwards MORE than it received: value created en route, not relayed.
    let events = vec![
        ev("v0", 700, "A", "B", "WSOL", 1_000_000, "sys", "transfer"),
        ev("v1", 701, "B", "C", "WSOL", 1_010_000, "sys", "transfer"),
    ];
    let d = decide(events);
    let r = reasons(&d);
    assert!(r.iter().any(|x| x.contains("negative") | x.contains("value created")), "reasons: {r:?}");
}
