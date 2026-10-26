use coordination_kernel::*;
use kernel_launch_v04::*;
use program_bridge_v04::discard::*;
use program_bridge_v04::*;
use std::collections::BTreeSet;

fn tid(n: u8) -> TransitionId {
    let mut id = [0u8; 32];
    id[0] = n;
    id
}

/// The bridge analogue of the delivered build's `blocked_world`: a blocked
/// transition and its sibling, both registered against the genesis head
/// and driven through the added steps, with only the blocked one selected
/// and driven to Blocked on a 1500 claim against the 1000 tranche stock.
/// The sibling sits Eligible, holding no slot — the blocked one does.
fn blocked_world() -> (LaunchKernel, TransitionId, TransitionId) {
    let blocked = RecordedTransition::ordinary(tid(41), RES_TREASURY_TRANCHE, amt(1500.0));
    let sibling = RecordedTransition::ordinary(tid(42), RES_TREASURY_TRANCHE, amt(100.0));
    let mut k = sealed_launch_kernel();
    // The kernel clock is strictly monotonic per call (`tick` rejects a
    // repeated timestamp), so the two ingests/registers are staggered by
    // one tick each; the added-steps schedule below is shared from +10.
    k.ingest_observation(&OBSERVER, blocked.observation.clone(), BASE)
        .expect("ingest runs");
    k.ingest_observation(&OBSERVER, sibling.observation.clone(), BASE + 1)
        .expect("ingest runs");
    k.register_transition(&PROPOSER, blocked.id, blocked.manifest.clone(), BTreeSet::new(), BASE + 2)
        .expect("register runs");
    k.register_transition(&PROPOSER, sibling.id, sibling.manifest.clone(), BTreeSet::new(), BASE + 3)
        .expect("register runs");
    // Every kernel call ticks the strictly-monotonic clock, so the two
    // transitions' added steps are staggered tick-by-tick, not shared.
    k.evaluate_transition(&blocked.id, BASE + 10).expect("evaluate runs");
    k.evaluate_transition(&sibling.id, BASE + 11).expect("evaluate runs");
    k.mark_eligible(&blocked.id, BASE + 12).expect("eligible runs");
    k.mark_eligible(&sibling.id, BASE + 13).expect("eligible runs");
    for (i, (agent, raw, support)) in blocked.signals.iter().enumerate() {
        k.submit_round_signal(agent, &blocked.id, *raw, *support, BASE + 20 + i as u64)
            .expect("signal runs");
    }
    for (i, (agent, raw, support)) in sibling.signals.iter().enumerate() {
        k.submit_round_signal(agent, &sibling.id, *raw, *support, BASE + 22 + i as u64)
            .expect("signal runs");
    }
    k.select_candidate(&blocked.id, BASE + 120).expect("select runs");
    k.open_planning(&PROPOSER, &blocked.id, BASE + 130)
        .expect("open_planning runs");
    k.submit_plan(
        &PROPOSER,
        &blocked.id,
        Plan {
            agent: PROPOSER,
            claims: blocked.plan_claims.clone(),
            expires_at: BASE + 5000,
        },
        BASE + 140,
    )
    .expect("submit_plan runs");
    k.reconcile_plans(&blocked.id, BASE + 240)
        .expect("reconcile runs");
    let _ = k.validate_transition(&blocked.id, BASE + 245);
    assert_eq!(k.life_of(&blocked.id), Some(Life::Blocked));
    (k, blocked.id, sibling.id)
}

#[test]
fn d1_over_claim_blocks_with_added_steps_and_no_debit() {
    let record = RecordedTransition::ordinary(tid(43), RES_TREASURY_TRANCHE, amt(1500.0));
    let (k, trace) = drive_to_validate(&record).expect("drive runs");
    assert_eq!(k.life_of(&record.id), Some(Life::Blocked));
    assert!(trace_contains_in_order(&trace, &["evaluate", "eligible", "select", "reconcile", "validate"]));
    assert!(!trace.contains(&"declare"), "a blocked replay must never declare");
    assert!(!trace.contains(&"commit"), "a blocked replay must never commit");
    assert_eq!(k.capacity_of(RES_TREASURY_TRANCHE), Some(amt(1000.0)));
    assert_eq!(k.capacity_of(RES_LENDING_BOOK), Some(amt(500.0)));
    assert_eq!(k.capacity_of(RES_EXIT_WINDOW), Some(amt(250.0)));
}

#[test]
fn d2_non_council_vote_is_refused_and_blocked_stands() {
    let (mut k, blocked, _sibling) = blocked_world();
    assert_eq!(
        discard_vote(&mut k, &ag(4), &blocked, BASE + 300),
        Err(KernelError::NotCouncil),
        "only council seats may vote to discard"
    );
    assert_eq!(k.life_of(&blocked), Some(Life::Blocked));
    assert_eq!(k.capacity_of(RES_TREASURY_TRANCHE), Some(amt(1000.0)));
}

#[test]
fn d3_single_seat_is_not_a_majority_and_double_vote_is_refused() {
    let (mut k, blocked, sibling) = blocked_world();
    assert_eq!(discard_vote(&mut k, &ag(1), &blocked, BASE + 300), Ok(false));
    assert_eq!(k.life_of(&blocked), Some(Life::Blocked));
    assert_eq!(
        discard_vote(&mut k, &ag(1), &blocked, BASE + 301),
        Err(KernelError::AlreadyVoted),
        "one vote per seat per blocking episode"
    );
    assert_eq!(
        k.select_candidate(&sibling, BASE + 310),
        Err(KernelError::ParentAlreadyHasASelectedTransition),
        "the slot is still held while the transition is merely Blocked"
    );
}

#[test]
fn d4_strict_majority_discards_and_frees_parent_slot() {
    let (mut k, blocked, sibling) = blocked_world();
    assert_eq!(discard_vote(&mut k, &ag(1), &blocked, BASE + 300), Ok(false));
    assert_eq!(discard_vote(&mut k, &ag(2), &blocked, BASE + 301), Ok(true));
    assert_eq!(k.life_of(&blocked), Some(Life::Discarded));
    k.select_candidate(&sibling, BASE + 310).expect("slot must free on discard");
    assert_eq!(k.life_of(&sibling), Some(Life::Selected));
    // Discarding moves no capacity: the blocked claim never debited.
    assert_eq!(k.capacity_of(RES_TREASURY_TRANCHE), Some(amt(1000.0)));
    assert_eq!(k.capacity_of(RES_LENDING_BOOK), Some(amt(500.0)));
    assert_eq!(k.capacity_of(RES_EXIT_WINDOW), Some(amt(250.0)));
}

#[test]
fn d5_discard_refuses_non_blocked_and_already_discarded() {
    // A fitting claim validates: Validated is not discardable.
    let record = RecordedTransition::ordinary(tid(44), RES_TREASURY_TRANCHE, amt(100.0));
    let (mut k, _trace) = drive_to_validate(&record).expect("drive runs");
    assert_eq!(k.life_of(&record.id), Some(Life::Validated));
    assert_eq!(
        discard_vote(&mut k, &ag(1), &record.id, BASE + 300),
        Err(KernelError::BadLifecycle(Life::Validated)),
        "only Blocked transitions may be discarded"
    );
    // A discarded transition cannot be discarded again.
    let (mut k, blocked, _sibling) = blocked_world();
    discard_vote(&mut k, &ag(1), &blocked, BASE + 300).expect("first vote runs");
    discard_vote(&mut k, &ag(2), &blocked, BASE + 301).expect("majority discards");
    assert_eq!(
        discard_vote(&mut k, &ag(3), &blocked, BASE + 302),
        Err(KernelError::BadLifecycle(Life::Discarded)),
        "discard is terminal for the episode's transition"
    );
}

#[test]
fn d6_second_blocking_episode_resets_votes() {
    let (mut k, blocked, sibling) = blocked_world();
    discard_vote(&mut k, &ag(1), &blocked, BASE + 300).expect("first vote runs");
    discard_vote(&mut k, &ag(2), &blocked, BASE + 301).expect("majority discards");
    k.select_candidate(&sibling, BASE + 310).expect("slot frees");
    // The sibling blocks on its own over-claim: a NEW episode, so the
    // seats' votes reset — seat 1 may vote again, and the majority
    // discards it exactly as before.
    k.open_planning(&PROPOSER, &sibling, BASE + 320)
        .expect("open_planning runs");
    k.submit_plan(
        &PROPOSER,
        &sibling,
        Plan {
            agent: PROPOSER,
            claims: [(RES_TREASURY_TRANCHE, amt(1500.0))].into_iter().collect(),
            expires_at: BASE + 5000,
        },
        BASE + 330,
    )
    .expect("submit_plan runs");
    k.reconcile_plans(&sibling, BASE + 430).expect("reconcile runs");
    let _ = k.validate_transition(&sibling, BASE + 435);
    assert_eq!(k.life_of(&sibling), Some(Life::Blocked));
    assert_eq!(discard_vote(&mut k, &ag(1), &sibling, BASE + 440), Ok(false));
    assert_eq!(discard_vote(&mut k, &ag(2), &sibling, BASE + 441), Ok(true));
    assert_eq!(k.life_of(&sibling), Some(Life::Discarded));
    assert_eq!(k.capacity_of(RES_TREASURY_TRANCHE), Some(amt(1000.0)));
}
