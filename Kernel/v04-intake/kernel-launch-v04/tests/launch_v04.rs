//! Migration step 2 tests: the treasury launch profile as configuration over
//! the delivered v0.4 kernel crate. The crate under test is never modified;
//! these tests drive it through its public API only.
//!
//! Timings mirror the v0.4 conformance fixture exactly (proven there): base
//! 100, evaluate/eligible 110, signals 120/121, select 220, planning 230,
//! plans 240, reconcile 340, validate/declare/commit 345, execution 400,
//! attestations 410+, outcome 420+. The genesis observation (timestamp 100,
//! expiry 5000, confidence 0.9) stays fresh throughout.
//!
//! One test documents an author-side gap as a finding, not a failure:
//!   l7 — the kernel settles a non-evidence hash when attested directly;
//!        evidence binding today lives only in the `SettlerGate` (l6).
//! F1 (closed in v0.4.1): l8 pins the switch-ON behavior via an explicit
//! configuration — influence follows reputation — l10 proves the explicit
//! switch-OFF behavior, and l11 proves the profile default itself is
//! pin-OFF (reputation recorded, influence stays at base).

use coordination_kernel::*;
use kernel_launch_v04::*;
use std::collections::{BTreeMap, BTreeSet};

fn tid(n: u8) -> TransitionId {
    [n; 32]
}

/// One plan's claims: (agent, [(resource, units)]).
type Claims<'a> = &'a [(u8, &'a [(ResourceId, f64)])];

fn manifest(k: &LaunchKernel, next: u8) -> Manifest {
    let d = k.declared_state().clone();
    Manifest {
        transition_type: TT,
        parent_index: d.index,
        parent_hash: d.hash,
        declared_next_hash: [next; 32],
        observation_id: OBS,
        expires_at: 10_100,
        relevant_groups: BTreeSet::from([1]),
        action: None,
    }
}

/// Drive `id` from registration through reconcile. Errors ignored: callers
/// assert the resulting life.
fn drive_to_reconciling(k: &mut LaunchKernel, id: TransitionId, next: u8, claims: Claims) {
    let m = manifest(k, next);
    let _ = k.register_transition(&ag(1), id, m, BTreeSet::new(), 100);
    let _ = k.evaluate_transition(&id, 110);
    let _ = k.mark_eligible(&id, 110);
    let _ = k.submit_round_signal(&ag(1), &id, amt(1.0), Some(amt(0.9)), 120);
    let _ = k.submit_round_signal(&ag(2), &id, amt(1.0), Some(amt(0.8)), 121);
    let _ = k.select_candidate(&id, 220);
    let _ = k.open_planning(&ag(1), &id, 230);
    for (n, c) in claims {
        let map: BTreeMap<ResourceId, Amount> =
            c.iter().map(|(r, v)| (*r, amt(*v))).collect();
        let _ = k.submit_plan(
            &ag(*n),
            &id,
            Plan { agent: ag(*n), claims: map, expires_at: 4000 },
            240,
        );
    }
    let _ = k.reconcile_plans(&id, 340);
}

/// Drive to Validated; panics unless validation succeeds.
fn drive_to_validated(k: &mut LaunchKernel, id: TransitionId, next: u8, claims: Claims) {
    drive_to_reconciling(k, id, next, claims);
    let _ = k.validate_transition(&id, 345);
    assert_eq!(k.life_of(&id), Some(Life::Validated), "drive must validate");
}

/// Drive to Committed; panics unless the commit succeeds.
fn drive_to_committed(k: &mut LaunchKernel, id: TransitionId, next: u8, claims: Claims) {
    drive_to_validated(k, id, next, claims);
    k.declare_commit(&id, 345).unwrap();
    k.commit(&id, 345).unwrap();
    assert_eq!(k.life_of(&id), Some(Life::Committed));
}

/// Drive to Executing (commit at 345, execution starts at 400).
fn drive_to_executing(k: &mut LaunchKernel, id: TransitionId, next: u8, claims: Claims) {
    drive_to_committed(k, id, next, claims);
    k.start_execution(&EXECUTOR, &id, 400).unwrap();
    assert_eq!(k.life_of(&id), Some(Life::Executing));
}

/// Drive a unanimously approved governance transition to Committable.
/// Timings mirror the conformance port's `gov_to_committable` (base 100).
fn gov_to_committable(k: &mut LaunchKernel, id: TransitionId, action: GovAction) {
    for (i, seat) in [ag(1), ag(2), ag(3)].iter().enumerate() {
        k.approve_governance(seat, id, action.clone(), 100 + i as u64).unwrap();
    }
    let mut m = manifest(k, 7);
    m.transition_type = action.category();
    k.register_governance_transition(&ag(1), id, m, action, 103).unwrap();
    let _ = k.evaluate_transition(&id, 110);
    let _ = k.mark_eligible(&id, 110);
    let _ = k.submit_round_signal(&ag(1), &id, amt(1.0), Some(amt(0.9)), 120);
    let _ = k.submit_round_signal(&ag(2), &id, amt(1.0), Some(amt(0.8)), 121);
    k.select_candidate(&id, 220).unwrap();
    k.open_planning(&ag(1), &id, 230).unwrap();
    k.submit_plan(
        &ag(1),
        &id,
        Plan { agent: ag(1), claims: BTreeMap::new(), expires_at: 4000 },
        240,
    )
    .unwrap();
    k.reconcile_plans(&id, 340).unwrap();
    let _ = k.validate_transition(&id, 345);
    k.declare_commit(&id, 345).unwrap();
    assert_eq!(k.life_of(&id), Some(Life::Committable));
}

// ---------------------------------------------------------------- L1: shape
#[test]
fn l1_sealed_launch_profile_shape() {
    let k = sealed_launch_kernel();
    assert!(k.is_sealed(), "profile must seal");
    assert_eq!(k.council(), &[ag(1), ag(2), ag(3)], "launch council seats 1-3");
    assert_eq!(k.capacity_of(RES_TREASURY_TRANCHE), Some(TRANCHE_CAP));
    assert_eq!(k.capacity_of(RES_LENDING_BOOK), Some(LENDING_CAP));
    assert_eq!(k.capacity_of(RES_EXIT_WINDOW), Some(EXIT_CAP));
    assert_eq!(k.declared_state().index, 0, "nothing committed at genesis");
    for n in 1..=6u8 {
        let p = k.agent(&ag(n)).expect("seeded agent must exist");
        assert!(p.active, "agent {n} must be active");
        assert_eq!(p.base_influence, amt(2.0), "agent {n} base must be 2.0");
        assert_eq!(p.reputation, amt(50.0), "agents start at the reputation midpoint");
    }
    assert!(k.has_role(&OBSERVER, Role::Observer));
    assert!(k.has_role(&EXECUTOR, Role::Executor));
    assert!(k.has_role(&SETTLER_A, Role::Settler));
    assert!(k.has_role(&SETTLER_B, Role::Settler));
    assert!(k.has_role(&RECORDER, Role::OutcomeRecorder));
    for t in [TT, T_GOV_ADJUSTABLE, T_GOV_CONSTITUTION, T_GOV_COUNCIL, T_GOV_REBOOT] {
        assert!(k.policy_of(&t).is_some(), "policy must exist for type {t:?}");
    }
    // Default cap: ONE.div_ceil(6) == 166667 raw units (1/6, rounded up).
    assert_eq!(k.max_influence_share(), Amount::from_raw(166_667));
}

// ---------------------------------------------------------------- L2: authority
#[test]
fn l2_genesis_authority_retired_at_seal() {
    let mut k = sealed_launch_kernel();
    assert_eq!(
        k.genesis_apply(&GEN, GovAction::SetCapacity { resource: 9, capacity: amt(1.0) }, 200),
        Err(KernelError::GenesisIsSealed),
        "sealed genesis must refuse the old authority"
    );
    let mut open = fresh_kernel();
    assert_eq!(
        open.genesis_apply(&ag(1), GovAction::SetCapacity { resource: 9, capacity: amt(1.0) }, 100),
        Err(KernelError::NotGenesis),
        "only the genesis authority may apply pre-seal"
    );
}

// ---------------------------------------------------------------- L3: seal gates
#[test]
fn l3_seal_refused_until_ready() {
    // No council at all.
    let mut k = fresh_kernel();
    apply_genesis_rules(&mut k, 100);
    assert_eq!(
        k.seal_genesis(&GEN, 100),
        Err(KernelError::GenesisNotReady("COUNCIL")),
        "seal without a council must name COUNCIL"
    );
    // Council installed, but the REBOOT governance policy missing.
    let mut k2 = fresh_kernel();
    k2.genesis_apply(&GEN, GovAction::SetPolicy { ttype: TT, policy: policy() }, 100).unwrap();
    for t in [T_GOV_ADJUSTABLE, T_GOV_CONSTITUTION, T_GOV_COUNCIL] {
        k2.genesis_apply(&GEN, GovAction::SetPolicy { ttype: t, policy: policy() }, 100).unwrap();
    }
    apply_genesis_agents_roles_stocks(&mut k2, 100);
    install_launch_council(&mut k2, 100);
    assert_eq!(
        k2.seal_genesis(&GEN, 100),
        Err(KernelError::GenesisNotReady("GOV_REBOOT")),
        "seal without a governance policy must name it"
    );
}

// ---------------------------------------------------------------- L4/L5: stocks
#[test]
fn l4_three_stocks_debited_independently_at_commit() {
    let mut k = sealed_launch_kernel();
    drive_to_committed(
        &mut k,
        tid(1),
        7,
        &[(1, &[(RES_TREASURY_TRANCHE, 100.0), (RES_LENDING_BOOK, 50.0)]),
          (2, &[(RES_EXIT_WINDOW, 25.0)])],
    );
    assert_eq!(k.capacity_of(RES_TREASURY_TRANCHE), Some(amt(900.0)));
    assert_eq!(k.capacity_of(RES_LENDING_BOOK), Some(amt(450.0)));
    assert_eq!(k.capacity_of(RES_EXIT_WINDOW), Some(amt(225.0)));
    assert_eq!(k.declared_state().index, 1);
}

#[test]
fn l5_overclaim_blocks_at_validation_and_touches_nothing() {
    let mut k = sealed_launch_kernel();
    drive_to_reconciling(&mut k, tid(1), 7, &[(1, &[(RES_EXIT_WINDOW, 300.0)])]);
    let _ = k.validate_transition(&tid(1), 345);
    assert_eq!(k.life_of(&tid(1)), Some(Life::Blocked), "over-claim must block validation");
    let mask = k.commit_fail_mask(&tid(1), 345).unwrap();
    assert!(mask.has(FailMask::COMPAT), "mask must name COMPAT");
    assert_eq!(k.capacity_of(RES_EXIT_WINDOW), Some(EXIT_CAP), "blocked plans debit nothing");
    assert!(
        matches!(k.commit(&tid(1), 345), Err(KernelError::BadLifecycle(_))),
        "blocked transition must not commit"
    );
}

// ---------------------------------------------------------------- L6/L7: evidence
#[test]
fn l6_evidence_gate_binds_settlers_to_the_recorded_hash() {
    let mut k = sealed_launch_kernel();
    drive_to_executing(&mut k, tid(1), 7, &[(1, &[(RES_TREASURY_TRANCHE, 100.0)])]);
    let mut ledger = EvidenceLedger::new();
    let gate = SettlerGate::new(&ledger);
    // No evidence yet: refused, and the refusal consumes no vote.
    assert_eq!(
        gate.attest_settlement(&mut k, &SETTLER_A, &tid(1), [7; 32], 410),
        Ok(GateDecision::NoEvidence)
    );
    assert_eq!(k.life_of(&tid(1)), Some(Life::Executing));
    // Evidence recorded (after the outcome exists); a wrong hash is refused.
    ledger.record(tid(1), [7; 32]);
    let gate = SettlerGate::new(&ledger);
    assert_eq!(
        gate.attest_settlement(&mut k, &SETTLER_A, &tid(1), [9; 32], 411),
        Ok(GateDecision::HashMismatch)
    );
    assert_eq!(k.life_of(&tid(1)), Some(Life::Executing));
    // The refused settler's correct attestation still counts (not Duplicate):
    // the first vote, then the second settler reaches the threshold.
    assert_eq!(
        gate.attest_settlement(&mut k, &SETTLER_A, &tid(1), [7; 32], 412),
        Ok(GateDecision::Attested(false))
    );
    assert_eq!(
        gate.attest_settlement(&mut k, &SETTLER_B, &tid(1), [7; 32], 413),
        Ok(GateDecision::Attested(true))
    );
    assert_eq!(k.life_of(&tid(1)), Some(Life::Settled));
    assert_eq!(k.realized_hash(&tid(1)), Some([7; 32]), "settled hash must be the evidence");
}

/// FINDING (author-side): without the gate, the delivered crate settles any
/// hash a threshold of Settlers agrees on — it does not check evidence.
/// This test pins that behavior so the gap stays visible until the author
/// adds the `Env` evidence method consulted by `attest_settlement`.
#[test]
fn l7_crate_settles_non_evidence_hash_without_gate_finding() {
    let mut k = sealed_launch_kernel();
    drive_to_executing(&mut k, tid(1), 7, &[(1, &[(RES_TREASURY_TRANCHE, 100.0)])]);
    assert_eq!(k.attest_settlement(&SETTLER_A, &tid(1), [9; 32], 410), Ok(false));
    assert_eq!(k.attest_settlement(&SETTLER_B, &tid(1), [9; 32], 411), Ok(true));
    assert_eq!(k.life_of(&tid(1)), Some(Life::Settled));
    assert_eq!(
        k.realized_hash(&tid(1)),
        Some([9; 32]),
        "crate settles the attested hash even though it is not the declared next hash"
    );
}

// ---------------------------------------------------------------- L8: switch ON
/// Switch-ON behavior, via an explicit configuration (no longer the profile
/// default since the pin-OFF flip): a settled exact outcome with +20
/// attribution moves reputation 50 -> 70 and influence 1.0 -> 1.4. L10
/// proves the explicit switch-OFF counterpart; L11 proves the profile
/// default is OFF.
#[test]
fn l8_reputation_influence_follows_reputation_f1_finding() {
    let mut cfg = config();
    cfg.reputation_weighs = true;
    let mut k = sealed_launch_kernel_with_config(cfg);
    drive_to_executing(&mut k, tid(1), 7, &[(1, &[(RES_TREASURY_TRANCHE, 100.0)])]);
    assert_eq!(k.influence_at(&ag(1), 100), Ok(amt(1.0)), "midpoint influence must be 1.0");
    assert_eq!(k.attest_settlement(&SETTLER_A, &tid(1), [7; 32], 410), Ok(false));
    assert_eq!(k.attest_settlement(&SETTLER_B, &tid(1), [7; 32], 411), Ok(true));
    let exact = k.record_outcome(&RECORDER, &tid(1), &[(ag(1), 20_000_000)], 420).unwrap();
    assert!(exact, "exact hash must settle exactly");
    assert_eq!(k.agent(&ag(1)).unwrap().reputation, amt(70.0));
    assert_eq!(
        k.influence_at(&ag(1), 420),
        Ok(amt(1.4)),
        "F1: influence follows reputation in the delivered build"
    );
}

// ---------------------------------------------------------------- L9: governance
#[test]
fn l9_post_seal_governance_changes_capacity() {
    let mut k = sealed_launch_kernel();
    let action = GovAction::SetCapacity { resource: RES_EXIT_WINDOW, capacity: amt(300.0) };
    gov_to_committable(&mut k, tid(1), action);
    k.commit_governance(&tid(1), 345).unwrap();
    assert_eq!(k.life_of(&tid(1)), Some(Life::Committed));
    assert_eq!(
        k.capacity_of(RES_EXIT_WINDOW),
        Some(amt(300.0)),
        "sealed profile changes only through a committed governance transition"
    );
    assert!(k.is_sealed(), "governance must not unseal the profile");
}

// ---------------------------------------------------------------- L10: F1 closed
/// Switch-OFF behavior, via an explicit configuration (v0.4.1
/// `Config::reputation_weighs = false`): the same +20 outcome records
/// reputation 50 -> 70, but influence stays at the administrator-set base
/// 2.0 throughout — reputation recorded, influence zero. L11 proves the
/// same behavior through the profile default itself, which is now OFF.
#[test]
fn l10_reputation_recorded_with_zero_influence_when_switch_off() {
    let mut cfg = config();
    cfg.reputation_weighs = false;
    let mut k = sealed_launch_kernel_with_config(cfg);
    drive_to_executing(&mut k, tid(1), 7, &[(1, &[(RES_TREASURY_TRANCHE, 100.0)])]);
    assert_eq!(k.influence_at(&ag(1), 100), Ok(amt(2.0)), "base influence must be 2.0");
    assert_eq!(k.attest_settlement(&SETTLER_A, &tid(1), [7; 32], 410), Ok(false));
    assert_eq!(k.attest_settlement(&SETTLER_B, &tid(1), [7; 32], 411), Ok(true));
    let exact = k.record_outcome(&RECORDER, &tid(1), &[(ag(1), 20_000_000)], 420).unwrap();
    assert!(exact, "exact hash must settle exactly");
    assert_eq!(k.agent(&ag(1)).unwrap().reputation, amt(70.0));
    assert_eq!(
        k.influence_at(&ag(1), 420),
        Ok(amt(2.0)),
        "F1 closed: with the switch off, influence must stay at base"
    );
}

// ---------------------------------------------------------------- L11: profile default is pin-OFF
/// The pin-OFF flip itself: the profile default (`config()`, no override)
/// now carries `reputation_weighs = false`. The same L8/L10 flow — a
/// settled exact outcome with +20 attribution — records reputation
/// 50 -> 70 while influence stays at the administrator-set base 2.0
/// before and after. This is the treasury launch profile's ruled form:
/// reputation recorded, influence zero. (L8 keeps the switch-ON form
/// proven via an explicit configuration; L10 keeps the explicit OFF
/// form proven; neither was weakened — this test is additive.)
#[test]
fn l11_profile_default_is_pin_off() {
    assert!(!config().reputation_weighs, "profile default must be pin-OFF");
    let mut k = sealed_launch_kernel();
    drive_to_executing(&mut k, tid(1), 7, &[(1, &[(RES_TREASURY_TRANCHE, 100.0)])]);
    assert_eq!(k.influence_at(&ag(1), 100), Ok(amt(2.0)), "base influence must be 2.0");
    assert_eq!(k.attest_settlement(&SETTLER_A, &tid(1), [7; 32], 410), Ok(false));
    assert_eq!(k.attest_settlement(&SETTLER_B, &tid(1), [7; 32], 411), Ok(true));
    let exact = k.record_outcome(&RECORDER, &tid(1), &[(ag(1), 20_000_000)], 420).unwrap();
    assert!(exact, "exact hash must settle exactly");
    assert_eq!(k.agent(&ag(1)).unwrap().reputation, amt(70.0));
    assert_eq!(
        k.influence_at(&ag(1), 420),
        Ok(amt(2.0)),
        "pin-OFF default: influence must stay at base after the outcome"
    );
}
