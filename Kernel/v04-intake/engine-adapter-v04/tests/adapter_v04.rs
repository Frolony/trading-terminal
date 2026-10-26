//! Step-4 increment-1 tests: the engine adapter over the v0.4 lifecycle.
//!
//! Stocks are the step-2 launch-profile fixtures (tranche 1000, lending
//! 500, exit 250) — staging numbers that prove the mechanics, not pilot
//! parameters. Every test drives the delivered v0.4 crate through the
//! adapter's public API only; the delivered crate is never modified.

use coordination_kernel::*;
use engine_adapter_v04::*;
use kernel_launch_v04::*;

// ---------------------------------------------------------------- core
#[test]
fn a1_authorize_tranche_commits_settles_and_debits() {
    let mut a = EngineAdapter::new();
    let auth = a.authorize(RES_TREASURY_TRANCHE, 100.0).unwrap().expect("must commit");
    assert!(auth.is_real(), "real flow must be settleable");
    assert_eq!(a.kernel().life_of(&auth.id), Some(Life::EvaluatedOutcome));
    assert_eq!(
        a.kernel().realized_hash(&auth.id),
        Some(a.kernel().declared_state().hash),
        "settled hash must be the declared outcome (exact settlement)"
    );
    assert_eq!(a.kernel().capacity_of(RES_TREASURY_TRANCHE), Some(amt(900.0)));
    assert_eq!(a.kernel().declared_state().index, 1);
}

#[test]
fn a2_zero_amount_is_a_noop_that_never_touches_the_kernel() {
    let mut a = EngineAdapter::new();
    let auth = a.authorize(RES_TREASURY_TRANCHE, 0.0).unwrap().expect("zero is a no-op auth");
    assert!(!auth.is_real(), "zero-amount auth must not settle");
    assert_eq!(a.kernel().declared_state().index, 0, "no transition may exist");
    assert_eq!(a.kernel().capacity_of(RES_TREASURY_TRANCHE), Some(TRANCHE_CAP));
}

#[test]
fn a3_overclaim_blocks_without_moving_money_and_frees_its_slot() {
    let mut a = EngineAdapter::new();
    // 1500 > 1000 tranche stock: blocked, nothing debited.
    assert_eq!(a.authorize(RES_TREASURY_TRANCHE, 1500.0).unwrap(), None);
    assert_eq!(a.kernel().capacity_of(RES_TREASURY_TRANCHE), Some(TRANCHE_CAP));
    assert_eq!(a.pending().len(), 1, "blocked claim must be held for carry");
    let blocked_id = a.pending()[0].blocked_id;
    assert_eq!(a.kernel().life_of(&blocked_id), Some(Life::Discarded), "blocked flow must be withdrawn");
    // The slot is free: a fitting flow on the same parent commits at once.
    // (Without the withdrawal, select would fail ParentAlreadyHasASelectedTransition.)
    let auth = a.authorize(RES_TREASURY_TRANCHE, 100.0).unwrap().expect("slot must be free");
    assert_ne!(auth.id, blocked_id, "fitting flow is a new transition, not the blocked one");
    assert_eq!(a.kernel().capacity_of(RES_TREASURY_TRANCHE), Some(amt(900.0)));
}

// ---------------------------------------------------------------- K-1
#[test]
fn a4_blocked_claim_carries_after_governance_topup() {
    let mut a = EngineAdapter::new();
    // 300 > 250 exit stock: blocked, held, withdrawn.
    assert_eq!(a.authorize(RES_EXIT_WINDOW, 300.0).unwrap(), None);
    assert_eq!(a.pending().len(), 1);
    let blocked_id = a.pending()[0].blocked_id;
    // Governed top-up to 300 (unanimous council, full lifecycle — no admin call).
    a.set_capacity_via_governance(RES_EXIT_WINDOW, 300.0).unwrap();
    assert_eq!(a.kernel().capacity_of(RES_EXIT_WINDOW), Some(amt(300.0)));
    // Carry: withdraw + re-register against the NEW head, full consensus re-earned.
    let carried = a.carry_pending().unwrap();
    assert_eq!(carried.len(), 1, "one pending claim must commit on the new head");
    assert_ne!(carried[0].id, blocked_id, "carry is a NEW transition id (ruling 4: duplicate intended)");
    assert_eq!(a.kernel().life_of(&blocked_id), Some(Life::Discarded), "old transition never mutates");
    assert_eq!(a.kernel().life_of(&carried[0].id), Some(Life::EvaluatedOutcome));
    assert_eq!(a.kernel().capacity_of(RES_EXIT_WINDOW), Some(amt(0.0)), "full 300 debited, no partial");
    assert!(a.pending().is_empty(), "carried claim leaves pending");
}

#[test]
fn a5_carry_without_topup_reblocks_and_keeps_the_whole_claim() {
    let mut a = EngineAdapter::new();
    assert_eq!(a.authorize(RES_EXIT_WINDOW, 300.0).unwrap(), None);
    // No top-up: the re-registered claim blocks again — carry is not a bypass.
    let carried = a.carry_pending().unwrap();
    assert!(carried.is_empty(), "nothing may commit against an unchanged head");
    assert_eq!(a.pending().len(), 1, "claim survives the re-block, whole and unmutated");
    assert_eq!(a.pending()[0].resource, RES_EXIT_WINDOW);
    assert_eq!(a.kernel().capacity_of(RES_EXIT_WINDOW), Some(EXIT_CAP), "re-block debits nothing (ruling 5)");
    // And it can still carry later, once a top-up lands.
    a.set_capacity_via_governance(RES_EXIT_WINDOW, 300.0).unwrap();
    assert_eq!(a.carry_pending().unwrap().len(), 1);
    assert_eq!(a.kernel().capacity_of(RES_EXIT_WINDOW), Some(amt(0.0)));
}

#[test]
fn a6_smaller_claim_is_a_new_transition_never_a_replan() {
    let mut a = EngineAdapter::new();
    assert_eq!(a.authorize(RES_EXIT_WINDOW, 300.0).unwrap(), None);
    let blocked_id = a.pending()[0].blocked_id;
    // A smaller claim goes through as its own transition (ruling 4: plans frozen).
    let auth = a.authorize(RES_EXIT_WINDOW, 200.0).unwrap().expect("smaller claim must commit");
    assert_ne!(auth.id, blocked_id);
    assert_eq!(a.kernel().capacity_of(RES_EXIT_WINDOW), Some(amt(50.0)));
    // The pending 300 claim is untouched by the 200 flow — not scaled, not consumed.
    assert_eq!(a.pending().len(), 1);
    assert_eq!(a.pending()[0].blocked_id, blocked_id);
    assert_eq!(a.kernel().life_of(&blocked_id), Some(Life::Discarded));
}

#[test]
fn a7_two_flows_in_sequence_keep_independent_heads_and_stocks() {
    let mut a = EngineAdapter::new();
    let t = a.authorize(RES_TREASURY_TRANCHE, 100.0).unwrap().expect("tranche must commit");
    let l = a.authorize(RES_LENDING_BOOK, 200.0).unwrap().expect("lending must commit");
    assert_ne!(t.id, l.id);
    assert_eq!(a.kernel().capacity_of(RES_TREASURY_TRANCHE), Some(amt(900.0)));
    assert_eq!(a.kernel().capacity_of(RES_LENDING_BOOK), Some(amt(300.0)));
    assert_eq!(a.kernel().declared_state().index, 2, "each commit advances the head once");
    assert!(a.pending().is_empty());
}
