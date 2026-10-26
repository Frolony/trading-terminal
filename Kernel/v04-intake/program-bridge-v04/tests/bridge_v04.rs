use coordination_kernel::*;
use kernel_launch_v04::*;
use program_bridge_v04::*;

fn tid(n: u8) -> TransitionId {
    let mut id = [0u8; 32];
    id[0] = n;
    id
}

fn trace_of(outcome: &ReplayOutcome) -> &[&str] {
    match outcome {
        ReplayOutcome::Committed { trace, .. }
        | ReplayOutcome::Blocked { trace, .. }
        | ReplayOutcome::Refused { trace, .. } => trace,
    }
}

#[test]
fn b1_well_formed_ordinary_replay_commits_and_debits_exactly() {
    let record = RecordedTransition::ordinary(tid(7), RES_TREASURY_TRANCHE, amt(100.0));
    let declared = record.manifest.declared_next_hash;
    let (outcome, kernel) = replay_transition_full(&record).expect("replay runs");
    match &outcome {
        ReplayOutcome::Committed { head, .. } => {
            assert_eq!(head.index, 1);
            assert_eq!(head.hash, declared);
        }
        other => panic!("expected Committed, got {other:?}"),
    }
    assert!(trace_contains_in_order(trace_of(&outcome), &ADDED_STEPS));
    assert_eq!(kernel.life_of(&record.id), Some(Life::Committed));
    // 1000 fixture stock minus the 100 claim, exactly — no partial debit.
    assert_eq!(kernel.capacity_of(RES_TREASURY_TRANCHE), Some(amt(900.0)));
}

#[test]
fn b2_over_claim_replay_blocks_and_moves_nothing() {
    let record = RecordedTransition::ordinary(tid(8), RES_TREASURY_TRANCHE, amt(2000.0));
    let (outcome, kernel) = replay_transition_full(&record).expect("replay runs");
    match &outcome {
        ReplayOutcome::Blocked { life, .. } => assert_eq!(*life, Life::Blocked),
        other => panic!("expected Blocked, got {other:?}"),
    }
    // Added steps up to validate ran; declare never ran; nothing moved.
    assert!(trace_contains_in_order(
        trace_of(&outcome),
        &["evaluate", "eligible", "select", "reconcile", "validate"]
    ));
    assert!(!trace_of(&outcome).contains(&"declare"));
    assert_eq!(kernel.capacity_of(RES_TREASURY_TRANCHE), Some(amt(1000.0)));
    assert_eq!(kernel.declared_state().index, 0);
}

#[test]
fn b3_stale_parent_replay_refused_at_registration_and_moves_nothing() {
    let mut record = RecordedTransition::ordinary(tid(9), RES_TREASURY_TRANCHE, amt(100.0));
    // The record was built against a head that is no longer current.
    record.manifest.parent_hash = [9; 32];
    // Stronger than the declare-time LINEAGE refusal this test was written
    // for: the v0.4 kernel refuses the record at registration itself
    // (ParentNeDeclared), so it never enters the lifecycle at all. The
    // replay returns that error and no kernel state is produced to move.
    match replay_transition_full(&record) {
        Err(KernelError::ParentNeDeclared) => {}
        Err(other) => panic!("expected ParentNeDeclared, got {other:?}"),
        Ok(_) => panic!("stale parent must refuse"),
    }
}

#[test]
fn b4_governance_replay_applies_the_rule_change() {
    let action = GovAction::SetCapacity { resource: RES_EXIT_WINDOW, capacity: amt(400.0) };
    let record = RecordedGovernance::new(tid(11), action);
    let (outcome, kernel) = replay_governance_full(&record).expect("replay runs");
    match &outcome {
        ReplayOutcome::Committed { head, .. } => {
            assert_eq!(head.index, 1);
            assert_eq!(head.hash, [0xE1; 32]);
        }
        other => panic!("expected Committed, got {other:?}"),
    }
    assert!(trace_contains_in_order(trace_of(&outcome), &ADDED_STEPS));
    // The governance transition changed the stock it named: 250 -> 400.
    assert_eq!(kernel.capacity_of(RES_EXIT_WINDOW), Some(amt(400.0)));
    // Other stocks untouched by the rule change.
    assert_eq!(kernel.capacity_of(RES_TREASURY_TRANCHE), Some(amt(1000.0)));
}

#[test]
fn b5_same_record_replayed_twice_gives_the_same_outcome() {
    let record = RecordedTransition::ordinary(tid(12), RES_TREASURY_TRANCHE, amt(100.0));
    let (first, k1) = replay_transition_full(&record).expect("first replay runs");
    let (second, k2) = replay_transition_full(&record).expect("second replay runs");
    assert_eq!(trace_of(&first), trace_of(&second));
    assert_eq!(k1.declared_state(), k2.declared_state());
    assert_eq!(
        k1.capacity_of(RES_TREASURY_TRANCHE),
        k2.capacity_of(RES_TREASURY_TRANCHE)
    );
    // Replay needs no prior live state: each run rebuilt from the record.
    assert!(matches!(first, ReplayOutcome::Committed { .. }));
    assert!(matches!(second, ReplayOutcome::Committed { .. }));
}

#[test]
fn b6_governance_replay_without_unanimity_does_not_apply() {
    // Approvals are part of the record: drop one seat and the governance
    // transition must not reach commit. (It fails at approval time in this
    // host replay; the program-side equivalent is a refused instruction.)
    let action = GovAction::SetCapacity { resource: RES_EXIT_WINDOW, capacity: amt(400.0) };
    let mut record = RecordedGovernance::new(tid(13), action);
    record.approvals.pop();
    let result = replay_governance_full(&record);
    assert!(result.is_err(), "non-unanimous governance replay must not commit");
}
