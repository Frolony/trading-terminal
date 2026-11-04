//! Migration step 5, slice 3: ruling-2 council discard, as a bridge replay.
//!
//! K-1 ruling 2 (Jeffrey, 2026-10-25; implemented in the delivered v0.4.1
//! build as `discard_blocked`) says a blocked transition may be discarded by
//! a strict majority of ALL council seats, from the Blocked state only —
//! never Committable or later, never by the proposer's seat alone — and
//! discarding frees the parent selection slot for a sibling. The step-1
//! conformance port proves the rule on the kernel (G8); this slice proves
//! it on the bridge: a recorded over-claim replayed to Blocked, then
//! discarded by replayed votes, then replaced by its sibling — the shape
//! the program rewrite will execute on-chain, where an abandoned blocked
//! transition must not freeze its parent until expiry.
//!
//! The bridge contract, stated as tests:
//! * A blocked replay debits nothing and carries no declare/commit step.
//! * A non-seat cannot vote (`NotCouncil`); one seat is not a strict
//!   majority (`Ok(false)`, still Blocked); a seat cannot vote twice in
//!   one episode (`AlreadyVoted`).
//! * A strict majority discards (`Ok(true)`, life `Discarded`), and only
//!   then does the sibling select — the slot is held until discard.
//! * Discard refuses anything not Blocked: a Validated transition and an
//!   already-Discarded transition both answer `BadLifecycle`.
//! * Votes are per blocking episode: a sibling that blocks again on its
//!   own over-claim takes fresh votes from the same seats.
//!
//! Honest limits, carried from slices 1–2: this is a host replay over the
//! launch fixture's stocks (tranche cap 1000), not an on-chain
//! measurement — CU is still unmeasured until the cu-golden step on the
//! program rewrite, and the D-10 flag arithmetic is program logic, not
//! host-fixture logic. The penalty door is engine-priced (D-4: flat 13%);
//! no penalty figure is asserted here.

use crate::{BASE, PROPOSER, RecordedTransition};
use coordination_kernel::*;
use kernel_launch_v04::*;
use std::collections::BTreeSet;

/// Drive one record through the added steps to validate on a fresh sealed
/// launch kernel, returning the kernel and the step trace. The caller
/// asserts the resulting life: `Blocked` for an over-claim, `Validated`
/// for a fitting claim. No declare/commit step is taken here — a blocked
/// replay must never reach them, and a validated replay's commit is the
/// slice-1 replay's job, not the discard's.
pub fn drive_to_validate(
    record: &RecordedTransition,
) -> Result<(LaunchKernel, Vec<&'static str>), KernelError> {
    let mut trace: Vec<&'static str> = vec![];
    let mut k = sealed_launch_kernel();
    k.ingest_observation(&OBSERVER, record.observation.clone(), BASE)?;
    trace.push("ingest_observation");
    k.register_transition(
        &PROPOSER,
        record.id,
        record.manifest.clone(),
        BTreeSet::new(),
        BASE,
    )?;
    trace.push("register");
    k.evaluate_transition(&record.id, BASE + 10)?;
    trace.push("evaluate");
    k.mark_eligible(&record.id, BASE + 10)?;
    trace.push("eligible");
    for (i, (agent, raw, support)) in record.signals.iter().enumerate() {
        k.submit_round_signal(agent, &record.id, *raw, *support, BASE + 20 + i as u64)?;
        trace.push("signal");
    }
    k.select_candidate(&record.id, BASE + 120)?;
    trace.push("select");
    k.open_planning(&PROPOSER, &record.id, BASE + 130)?;
    trace.push("open_planning");
    k.submit_plan(
        &PROPOSER,
        &record.id,
        Plan {
            agent: PROPOSER,
            claims: record.plan_claims.clone(),
            expires_at: BASE + 5000,
        },
        BASE + 140,
    )?;
    trace.push("submit_plan");
    k.reconcile_plans(&record.id, BASE + 240)?;
    trace.push("reconcile");
    let _ = k.validate_transition(&record.id, BASE + 245);
    trace.push("validate");
    Ok((k, trace))
}

/// One council discard vote on a blocked transition, as the program will
/// execute it: `Ok(true)` once a strict majority of all seats has voted
/// in this blocking episode, `Ok(false)` before that, errors for a
/// non-seat, a double vote, or a non-Blocked transition.
pub fn discard_vote(
    k: &mut LaunchKernel,
    seat: &AgentId,
    id: &TransitionId,
    now: Timestamp,
) -> Result<bool, KernelError> {
    k.discard_blocked(seat, id, now)
}
