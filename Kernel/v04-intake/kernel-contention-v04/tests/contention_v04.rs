//! K-1 contention probes (migration step 3 preparation).
//!
//! These tests drive the delivered v0.4 crate through the step-2 launch
//! profile's public API only. They do NOT rule K-1 — they pin down what the
//! delivered build already does, so the author's ruling answers a precise
//! question instead of a vague one. Timings mirror the launch tests exactly
//! (register base, evaluate/eligible +10, signals +20/+21, select +120,
//! planning +130, plans +140, reconcile +240, validate/declare/commit +245),
//! with each drive on its own later base so time stays monotone and the
//! genesis observation (expiry 5000) stays fresh.
//!
//! What the probes establish (all read off the delivered source first, then
//! proven here):
//!   * compatibility is computed ONCE at reconcile: a capacity-blocked
//!     transition keeps `compatible = false`, so re-validating it alone
//!     returns `StillBlocked` forever;
//!   * re-planning cannot unblock it either: re-plan does not clear plans,
//!     an existing planner's resubmission is `Duplicate`, and new plans
//!     only grow the aggregate — so a capacity-blocked transition has NO
//!     in-place unblock at all; its exits are proposer withdrawal, expiry,
//!     or the 3-replan loop ending at `ReplanCap` still blocked;
//!   * a Blocked transition holds the `selected_for` slot for its parent
//!     key, so no same-parent transition — including a governance top-up —
//!     can select until the blocked claim is withdrawn (only the proposer
//!     may withdraw);
//!   * once a top-up commits, the old transition is stranded: its mask
//!     carries LINEAGE, and only a fresh transition against the new head
//!     (adapter-level carry, new id) can carry the original claim. Carry-over
//!     is therefore an adapter duty, not a kernel transition state — which
//!     is exactly what K-1 must rule on.

use coordination_kernel::*;
use kernel_launch_v04::*;
use std::collections::{BTreeMap, BTreeSet};

fn tid(n: u8) -> TransitionId {
    [n; 32]
}

/// Manifest against the kernel's CURRENT declared head (the adapter's view).
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

type Claims<'a> = &'a [(u8, &'a [(ResourceId, f64)])];

/// Register -> evaluate -> eligible -> signals -> select. Panics unless the
/// transition reaches Selected.
fn drive_to_selected(k: &mut LaunchKernel, id: TransitionId, next: u8, base: u64) {
    let m = manifest(k, next);
    k.register_transition(&ag(1), id, m, BTreeSet::new(), base).unwrap();
    k.evaluate_transition(&id, base + 10).unwrap();
    k.mark_eligible(&id, base + 10).unwrap();
    k.submit_round_signal(&ag(1), &id, amt(1.0), Some(amt(0.9)), base + 20).unwrap();
    k.submit_round_signal(&ag(2), &id, amt(1.0), Some(amt(0.8)), base + 21).unwrap();
    k.select_candidate(&id, base + 120).unwrap();
    assert_eq!(k.life_of(&id), Some(Life::Selected), "drive must select");
}

/// Selected -> planning -> plans -> reconcile. The caller asserts the
/// resulting life (Validated or Blocked).
fn drive_plans(k: &mut LaunchKernel, id: TransitionId, base: u64, claims: Claims) {
    k.open_planning(&ag(1), &id, base + 130).unwrap();
    for (n, c) in claims {
        let map: BTreeMap<ResourceId, Amount> =
            c.iter().map(|(r, v)| (*r, amt(*v))).collect();
        k.submit_plan(
            &ag(*n),
            &id,
            Plan { agent: ag(*n), claims: map, expires_at: 9000 },
            base + 140,
        )
        .unwrap();
    }
    k.reconcile_plans(&id, base + 240).unwrap();
}

/// Full drive to a validation outcome; returns the life after validation.
fn drive_to_validation(
    k: &mut LaunchKernel,
    id: TransitionId,
    next: u8,
    base: u64,
    claims: Claims,
) -> Life {
    drive_to_selected(k, id, next, base);
    drive_plans(k, id, base, claims);
    let _ = k.validate_transition(&id, base + 245);
    k.life_of(&id).expect("transition must exist")
}

// ---------------------------------------------------------------- K1a: no in-place unblock
#[test]
fn k1a_blocked_cannot_unblock_in_place() {
    let mut k = sealed_launch_kernel();
    // 300 > 250 exit stock: blocked, COMPAT, nothing debited.
    let life = drive_to_validation(
        &mut k,
        tid(1),
        7,
        100,
        &[(1, &[(RES_EXIT_WINDOW, 300.0)])],
    );
    assert_eq!(life, Life::Blocked, "over-claim must block");
    assert!(k.commit_fail_mask(&tid(1), 345).unwrap().has(FailMask::COMPAT));
    assert_eq!(k.capacity_of(RES_EXIT_WINDOW), Some(EXIT_CAP));
    // Re-validating alone never unblocks: compatibility is a stored flag
    // from reconcile, not a live re-check.
    assert_eq!(
        k.validate_transition(&tid(1), 346),
        Err(KernelError::StillBlocked),
        "re-validate without a re-plan must stay blocked"
    );
    // Re-planning does NOT clear plans: the original planner cannot revise
    // its claim (Duplicate), so the aggregate can only grow on re-plan.
    k.open_planning(&ag(1), &tid(1), 500).unwrap();
    assert_eq!(
        k.submit_plan(
            &ag(1),
            &tid(1),
            Plan {
                agent: ag(1),
                claims: BTreeMap::from([(RES_EXIT_WINDOW, amt(200.0))]),
                expires_at: 9000,
            },
            510,
        ),
        Err(KernelError::Duplicate),
        "re-plan cannot revise an existing planner's claim"
    );
    // A new planner can only add to the aggregate — still blocked.
    k.submit_plan(
        &ag(3),
        &tid(1),
        Plan {
            agent: ag(3),
            claims: BTreeMap::from([(RES_EXIT_WINDOW, amt(10.0))]),
            expires_at: 9000,
        },
        510,
    )
    .unwrap();
    k.reconcile_plans(&tid(1), 610).unwrap();
    let _ = k.validate_transition(&tid(1), 615);
    assert_eq!(k.life_of(&tid(1)), Some(Life::Blocked), "aggregate only grew");
    assert_eq!(k.capacity_of(RES_EXIT_WINDOW), Some(EXIT_CAP));
    // The only exit is withdrawal; the claim then carries at the adapter
    // level as a FRESH transition with a fitting claim.
    k.withdraw(&ag(1), &tid(1), 620).unwrap();
    let life = drive_to_validation(
        &mut k,
        tid(2),
        8,
        1000,
        &[(1, &[(RES_EXIT_WINDOW, 200.0)])],
    );
    assert_eq!(life, Life::Validated);
    k.declare_commit(&tid(2), 1245).unwrap();
    k.commit(&tid(2), 1245).unwrap();
    assert_eq!(k.capacity_of(RES_EXIT_WINDOW), Some(amt(50.0)));
    assert_eq!(k.declared_state().index, 1);
}

// ---------------------------------------------------------------- K1b: slot
#[test]
fn k1b_blocked_holds_parent_slot_until_withdrawn() {
    let mut k = sealed_launch_kernel();
    let life = drive_to_validation(
        &mut k,
        tid(1),
        7,
        100,
        &[(1, &[(RES_EXIT_WINDOW, 300.0)])],
    );
    assert_eq!(life, Life::Blocked);
    // A second transition on the same parent cannot select while T1 is Blocked.
    let m = manifest(&k, 8);
    k.register_transition(&ag(1), tid(2), m, BTreeSet::new(), 1000).unwrap();
    k.evaluate_transition(&tid(2), 1010).unwrap();
    k.mark_eligible(&tid(2), 1010).unwrap();
    k.submit_round_signal(&ag(1), &tid(2), amt(1.0), Some(amt(0.9)), 1020).unwrap();
    k.submit_round_signal(&ag(2), &tid(2), amt(1.0), Some(amt(0.8)), 1021).unwrap();
    assert_eq!(
        k.select_candidate(&tid(2), 1120),
        Err(KernelError::ParentAlreadyHasASelectedTransition),
        "blocked T1 must hold the parent slot"
    );
    // Only the proposer can free it — and only by withdrawing (Discarded).
    assert_eq!(
        k.withdraw(&ag(2), &tid(1), 1125),
        Err(KernelError::Unauthorized),
        "a non-proposer must not free the slot"
    );
    k.withdraw(&ag(1), &tid(1), 1125).unwrap();
    assert_eq!(k.life_of(&tid(1)), Some(Life::Discarded));
    k.select_candidate(&tid(2), 1130).unwrap();
    assert_eq!(k.life_of(&tid(2)), Some(Life::Selected), "slot frees on withdrawal");
}

// ---------------------------------------------------------------- K1c: top-up
#[test]
fn k1c_topup_waits_for_the_blocked_claim_then_adapter_carry() {
    let mut k = sealed_launch_kernel();
    let life = drive_to_validation(
        &mut k,
        tid(1),
        7,
        100,
        &[(1, &[(RES_EXIT_WINDOW, 300.0)])],
    );
    assert_eq!(life, Life::Blocked);
    // The governance top-up cannot even select while T1 holds the slot.
    let action = GovAction::SetCapacity { resource: RES_EXIT_WINDOW, capacity: amt(300.0) };
    for (i, seat) in [ag(1), ag(2), ag(3)].iter().enumerate() {
        k.approve_governance(seat, tid(9), action.clone(), 1000 + i as u64).unwrap();
    }
    let mut m = manifest(&k, 7);
    m.transition_type = action.category();
    k.register_governance_transition(&ag(1), tid(9), m, action.clone(), 1003).unwrap();
    k.evaluate_transition(&tid(9), 1010).unwrap();
    k.mark_eligible(&tid(9), 1010).unwrap();
    k.submit_round_signal(&ag(1), &tid(9), amt(1.0), Some(amt(0.9)), 1020).unwrap();
    k.submit_round_signal(&ag(2), &tid(9), amt(1.0), Some(amt(0.8)), 1021).unwrap();
    assert_eq!(
        k.select_candidate(&tid(9), 1120),
        Err(KernelError::ParentAlreadyHasASelectedTransition),
        "top-up must wait for the blocked claim"
    );
    // Withdraw T1, then the top-up selects and commits: head moves to 1.
    k.withdraw(&ag(1), &tid(1), 1125).unwrap();
    k.select_candidate(&tid(9), 1130).unwrap();
    k.open_planning(&ag(1), &tid(9), 1130).unwrap();
    k.submit_plan(
        &ag(1),
        &tid(9),
        Plan { agent: ag(1), claims: BTreeMap::new(), expires_at: 9000 },
        1140,
    )
    .unwrap();
    k.reconcile_plans(&tid(9), 1240).unwrap();
    k.validate_transition(&tid(9), 1245).unwrap();
    k.declare_commit(&tid(9), 1245).unwrap();
    k.commit_governance(&tid(9), 1245).unwrap();
    assert_eq!(k.capacity_of(RES_EXIT_WINDOW), Some(amt(300.0)));
    assert_eq!(k.declared_state().index, 1);
    // The old transition is stranded: its mask now carries LINEAGE. It
    // cannot be marked conflicted after withdrawal — Conflicted is only
    // for transitions still pre-commit, so withdrawal (not conflict
    // marking) is the blocked claim's exit once a top-up has moved the head.
    assert!(k.commit_fail_mask(&tid(1), 1250).unwrap().has(FailMask::LINEAGE));
    assert_eq!(
        k.mark_conflicted(&tid(1), 1250),
        Err(KernelError::BadLifecycle(Life::Discarded)),
        "a withdrawn transition cannot also be marked conflicted"
    );
    // Adapter-level carry: a FRESH transition against the new head carries
    // the original 300 claim and commits it in full.
    let life = drive_to_validation(
        &mut k,
        tid(3),
        8,
        2000,
        &[(1, &[(RES_EXIT_WINDOW, 300.0)])],
    );
    assert_eq!(life, Life::Validated, "carried claim must validate on the new head");
    k.declare_commit(&tid(3), 2245).unwrap();
    k.commit(&tid(3), 2245).unwrap();
    assert_eq!(k.capacity_of(RES_EXIT_WINDOW), Some(amt(0.0)));
    assert_eq!(k.declared_state().index, 2);
}

// ---------------------------------------------------------------- K1d: cap + no partial
#[test]
fn k1d_replan_cap_and_no_partial_debit() {
    let mut k = sealed_launch_kernel();
    // Aggregate over-claim across two plans: the whole transition blocks
    // and debits nothing — never a partial debit.
    let life = drive_to_validation(
        &mut k,
        tid(1),
        7,
        100,
        &[(1, &[(RES_EXIT_WINDOW, 200.0)]), (2, &[(RES_EXIT_WINDOW, 100.0)])],
    );
    assert_eq!(life, Life::Blocked, "aggregate 300 > 250 must block");
    assert_eq!(k.capacity_of(RES_EXIT_WINDOW), Some(EXIT_CAP));
    // Three re-plans, each adding a NEW planner's claim (existing plans
    // cannot be revised — see k1a): the fourth re-plan is capped.
    let mut base = 500u64;
    for n in [3u8, 4, 5] {
        k.open_planning(&ag(1), &tid(1), base).unwrap();
        k.submit_plan(
            &ag(n),
            &tid(1),
            Plan {
                agent: ag(n),
                claims: BTreeMap::from([(RES_EXIT_WINDOW, amt(10.0))]),
                expires_at: 9000,
            },
            base + 10,
        )
        .unwrap();
        k.reconcile_plans(&tid(1), base + 110).unwrap();
        let _ = k.validate_transition(&tid(1), base + 115);
        assert_eq!(k.life_of(&tid(1)), Some(Life::Blocked));
        base += 300;
    }
    assert_eq!(
        k.open_planning(&ag(1), &tid(1), base),
        Err(KernelError::ReplanCap),
        "the 4th re-plan must be refused"
    );
    assert_eq!(k.capacity_of(RES_EXIT_WINDOW), Some(EXIT_CAP), "still nothing debited");
}
