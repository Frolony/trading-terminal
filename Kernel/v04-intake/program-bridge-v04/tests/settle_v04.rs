use coordination_kernel::*;
use kernel_launch_v04::*;
use program_bridge_v04::settle::*;
use program_bridge_v04::*;

fn tid(n: u8) -> TransitionId {
    let mut id = [0u8; 32];
    id[0] = n;
    id
}

fn committed_transition(n: u8) -> (RecordedTransition, LaunchKernel) {
    let record = RecordedTransition::ordinary(tid(n), RES_TREASURY_TRANCHE, amt(100.0));
    let (outcome, kernel) = replay_transition_full(&record).expect("replay runs");
    assert!(matches!(outcome, ReplayOutcome::Committed { .. }), "replay must commit: {outcome:?}");
    (record, kernel)
}

#[test]
fn s1_committed_replay_executes_settles_and_records_exactly() {
    let (record, mut kernel) = committed_transition(31);
    let evidence = record.manifest.declared_next_hash;
    assert_eq!(kernel.life_of(&record.id), Some(Life::Committed));
    begin_execution(&mut kernel, &record.id).expect("execution must start");
    assert_eq!(kernel.life_of(&record.id), Some(Life::Executing));
    let settled = settle_with_evidence(&mut kernel, &record.id, evidence).expect("settle runs");
    assert_eq!(settled.first, GateDecision::Attested(false), "one attestation is below threshold");
    assert_eq!(settled.second, GateDecision::Attested(true), "second attestation settles");
    assert!(settled.exact, "realized == declared, so the outcome is exact");
    assert_eq!(kernel.life_of(&record.id), Some(Life::EvaluatedOutcome));
    assert_eq!(kernel.realized_hash(&record.id), Some(evidence));
    // The settle tail moves no stock: the tranche debit happened at commit.
    assert_eq!(kernel.capacity_of(RES_TREASURY_TRANCHE), Some(amt(900.0)));
    assert!(kernel.events().iter().any(|e| matches!(
        e,
        Event::Settled { id, exact } if *id == record.id && *exact
    )));
}

#[test]
fn s2_gate_refusals_consume_no_vote_and_correct_attestation_still_settles() {
    let (record, mut kernel) = committed_transition(32);
    let evidence = record.manifest.declared_next_hash;
    begin_execution(&mut kernel, &record.id).expect("execution must start");
    // No evidence recorded: refused, no vote consumed.
    let empty_ledger = EvidenceLedger::new();
    let gate = SettlerGate::new(&empty_ledger);
    assert_eq!(
        gate.attest_settlement(&mut kernel, &SETTLER_A, &record.id, evidence, ATTEST_A_AT)
            .expect("gate runs"),
        GateDecision::NoEvidence
    );
    assert_eq!(kernel.life_of(&record.id), Some(Life::Executing));
    // Evidence recorded but a different hash: refused, no vote consumed.
    let mut ledger = EvidenceLedger::new();
    ledger.record(record.id, evidence);
    let gate = SettlerGate::new(&ledger);
    let mut wrong = evidence;
    wrong[0] ^= 0xFF;
    assert_eq!(
        gate.attest_settlement(&mut kernel, &SETTLER_A, &record.id, wrong, ATTEST_A_AT)
            .expect("gate runs"),
        GateDecision::HashMismatch
    );
    // The refused settler can still attest correctly afterwards.
    let settled = settle_with_evidence(&mut kernel, &record.id, evidence).expect("settle runs");
    assert_eq!(settled.second, GateDecision::Attested(true));
    assert_eq!(kernel.life_of(&record.id), Some(Life::EvaluatedOutcome));
}

#[test]
fn s3_non_settler_refused_and_duplicate_vote_refused() {
    let (record, mut kernel) = committed_transition(33);
    let evidence = record.manifest.declared_next_hash;
    begin_execution(&mut kernel, &record.id).expect("execution must start");
    let mut ledger = EvidenceLedger::new();
    ledger.record(record.id, evidence);
    let gate = SettlerGate::new(&ledger);
    // A council seat without the Settler role cannot attest.
    assert_eq!(
        gate.attest_settlement(&mut kernel, &PROPOSER, &record.id, evidence, ATTEST_A_AT),
        Err(KernelError::Unauthorized)
    );
    assert_eq!(
        gate.attest_settlement(&mut kernel, &SETTLER_A, &record.id, evidence, ATTEST_A_AT)
            .expect("gate runs"),
        GateDecision::Attested(false)
    );
    // The same settler cannot vote twice, even for the same hash.
    assert_eq!(
        gate.attest_settlement(&mut kernel, &SETTLER_A, &record.id, evidence, ATTEST_A_AT),
        Err(KernelError::Duplicate)
    );
    assert_eq!(
        gate.attest_settlement(&mut kernel, &SETTLER_B, &record.id, evidence, ATTEST_B_AT)
            .expect("gate runs"),
        GateDecision::Attested(true)
    );
    assert_eq!(kernel.life_of(&record.id), Some(Life::Settled));
}

#[test]
fn s4_settle_before_commit_refused_at_both_doors() {
    // Door 1: a Blocked transition cannot start execution.
    let blocked = RecordedTransition::ordinary(tid(34), RES_TREASURY_TRANCHE, amt(2000.0));
    let (outcome, mut kernel) = replay_transition_full(&blocked).expect("replay runs");
    assert!(matches!(outcome, ReplayOutcome::Blocked { .. }));
    assert_eq!(
        begin_execution(&mut kernel, &blocked.id),
        Err(KernelError::BadLifecycle(Life::Blocked))
    );
    // Door 2: a Committed (not yet Executing) transition cannot be attested.
    let (record, mut kernel) = committed_transition(35);
    let evidence = record.manifest.declared_next_hash;
    assert_eq!(
        kernel.attest_settlement(&SETTLER_A, &record.id, evidence, ATTEST_A_AT),
        Err(KernelError::BadLifecycle(Life::Committed))
    );
    assert_eq!(kernel.realized_hash(&record.id), None);
}

#[test]
fn s5_disagreeing_settlers_never_settle() {
    let (record, mut kernel) = committed_transition(36);
    begin_execution(&mut kernel, &record.id).expect("execution must start");
    let mut hash_x = [0x11; 32];
    hash_x[1..9].copy_from_slice(&record.id[0..8]);
    let mut hash_y = [0x22; 32];
    hash_y[1..9].copy_from_slice(&record.id[0..8]);
    // Raw kernel attestations, bypassing the gate on purpose: the gate
    // would never forward two different hashes for one evidence record,
    // so the per-hash vote counting is a kernel property, proven here.
    assert_eq!(
        kernel.attest_settlement(&SETTLER_A, &record.id, hash_x, ATTEST_A_AT),
        Ok(false)
    );
    assert_eq!(
        kernel.attest_settlement(&SETTLER_B, &record.id, hash_y, ATTEST_B_AT),
        Ok(false)
    );
    // Neither hash reached the threshold of 2; nothing settled.
    assert_eq!(kernel.life_of(&record.id), Some(Life::Executing));
    assert_eq!(kernel.realized_hash(&record.id), None);
    // And neither settler can change their vote after the fact.
    assert_eq!(
        kernel.attest_settlement(&SETTLER_A, &record.id, hash_y, ATTEST_B_AT),
        Err(KernelError::Duplicate)
    );
    assert_eq!(kernel.life_of(&record.id), Some(Life::Executing));
}

#[test]
fn s6_governance_replay_settles_through_the_same_tail() {
    let action = GovAction::SetCapacity { resource: RES_EXIT_WINDOW, capacity: amt(400.0) };
    let record = RecordedGovernance::new(tid(37), action);
    let (outcome, mut kernel) = replay_governance_full(&record).expect("replay runs");
    assert!(matches!(outcome, ReplayOutcome::Committed { .. }), "governance must commit: {outcome:?}");
    begin_execution(&mut kernel, &record.id).expect("execution must start");
    let settled = settle_with_evidence(&mut kernel, &record.id, [0xE1; 32]).expect("settle runs");
    assert_eq!(settled.first, GateDecision::Attested(false));
    assert_eq!(settled.second, GateDecision::Attested(true));
    assert!(settled.exact, "governance declared hash is the 0xE1 marker");
    assert_eq!(kernel.life_of(&record.id), Some(Life::EvaluatedOutcome));
    // The rule change survives the settle tail.
    assert_eq!(kernel.capacity_of(RES_EXIT_WINDOW), Some(amt(400.0)));
}

#[test]
fn s7_settled_transition_cannot_execute_or_attest_again() {
    let (record, mut kernel) = committed_transition(38);
    let evidence = record.manifest.declared_next_hash;
    begin_execution(&mut kernel, &record.id).expect("execution must start");
    settle_with_evidence(&mut kernel, &record.id, evidence).expect("settle runs");
    assert_eq!(kernel.life_of(&record.id), Some(Life::EvaluatedOutcome));
    // Forward time, so the refusals below are lifecycle refusals, not
    // clock refusals (the helper's EXECUTE_AT predates settlement).
    assert_eq!(
        kernel.start_execution(&EXECUTOR, &record.id, POST_SETTLE_AT),
        Err(KernelError::BadLifecycle(Life::EvaluatedOutcome))
    );
    assert_eq!(
        kernel.attest_settlement(&SETTLER_A, &record.id, evidence, POST_SETTLE_AT),
        Err(KernelError::BadLifecycle(Life::EvaluatedOutcome))
    );
}
