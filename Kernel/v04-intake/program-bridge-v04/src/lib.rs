//! Migration step 5, slice 1: the program bridge over the v0.4 lifecycle.
//!
//! The v0.2 program bridge (`Kernel/treasury-program/.../kernel_bridge.rs`)
//! rebuilds a transition's kernel state by replaying its recorded inputs
//! through the kernel's public guarded entry points, in lifecycle order —
//! because the pristine crate offers no serialization or state-restore API,
//! a `Kernel` cannot be rehydrated from accounts. This crate is the same
//! shape over the v0.4 lifecycle, and it is what changes in step 5:
//!
//! * v0.2 replay: register -> signal -> open_planning -> plan ->
//!   reconcile -> mask -> commit. A direct admin `set_capacity` set stocks.
//! * v0.4 replay: register -> **evaluate -> eligible** -> signals ->
//!   **select** -> open_planning -> plan -> **reconcile -> validate ->
//!   declare** -> commit. The added steps are part of the replay, not
//!   skipped: a record that cannot pass them cannot commit, exactly as a
//!   live drive could not. Governance increments become governance
//!   transitions: a rule change replays through council approval +
//!   `register_governance_transition` + the same added steps +
//!   `commit_governance`, never through an admin shortcut (there is no
//!   direct `set_capacity` on the sealed profile at all).
//!
//! The record types below are the host analogue of the on-chain
//! `TransitionRecord` account: everything the program would have stored
//! when the instruction arrived, and nothing else. `replay_*` builds a
//! FRESH sealed launch kernel, replays the record into it, and returns the
//! outcome — no live kernel state is carried over, so the predicate judges
//! exactly the recorded inputs against the genesis head, never a
//! publisher's say-so. Three failure shapes, all proven by tests:
//!
//! * `Blocked` — validate refused structurally (e.g. an over-claim:
//!   reconcile already found the claim does not fit capacity).
//! * `Refused(mask)` — validate passed structurally but the full predicate
//!   at declare refused. Nothing moves.
//! * Registration refusal — a record built against a stale parent never
//!   enters the lifecycle at all: registration itself errors
//!   `ParentNeDeclared` (stronger than the declare-time LINEAGE refusal,
//!   which remains the backstop if a head moves mid-replay).
//!
//! What this slice does NOT do (later step-5 slices, not this one):
//! * It does not touch the Anchor program: the program rewrite rides the
//!   branching question, now ruled 2026-10-26 (v0.4 enters the repo only
//!   on the dedicated `kernel-v04` branch, never on main, until Carlos
//!   separately rules a merge or replacement). This crate proves the
//!   replay shape on the host, as the step-4 adapter proved the drive
//!   shape.
//!
//! Slice 2 lives in [`treasury`]: the §4.2/§4.3 treasury instructions as
//! typed constructors over these records, plus the §3 account inventory.
//! * It does not measure CU. F2's commit-clone concern is gone from the
//!   commit path as implemented in the delivered v0.4.1 build
//!   (`commit_check`/`commit_apply`: check first, apply without a backup
//!   copy), but on-chain CU is still unmeasured — the cu-golden step
//!   measures it on the program rewrite; this slice assumes nothing about
//!   it. Settlement after commit is unchanged: it stays with the step-2
//!   `SettlerGate` composition (evidence-bound, settle-after-realized).
//!
//! Timing schedule: the replay reuses the step-4 adapter's proven drive
//! offsets verbatim (evaluate/eligible +10, signals +20/+21, select +120,
//! open_planning +130, plan +140, reconcile +240, validate/declare/commit
//! +245, base 100). The schedule is part of the bridge contract: the
//! launch policy's consensus window (100) and planning window (100) are
//! what those offsets clear.

use coordination_kernel::*;
use kernel_launch_v04::*;
use std::collections::{BTreeMap, BTreeSet};

pub mod discard;
pub mod settle;
pub mod treasury;

/// Proposer of every replayed flow (launch-profile agent 1, a council seat).
pub const PROPOSER: AgentId = [1; 32];

/// Base time of every replay. The sealed launch kernel is sealed at t=100,
/// so a replay starting at 100 is the first drive on a fresh kernel —
/// exactly the step-4 adapter's first drive.
pub const BASE: Timestamp = 100;

/// One recorded ordinary transition: the manifest, its observation, its
/// consensus signals, and its plan claims. The declared-next hash is part
/// of the manifest; the parent is whatever head the record was built
/// against (genesis for a well-formed record).
#[derive(Debug, Clone)]
pub struct RecordedTransition {
    pub id: TransitionId,
    pub manifest: Manifest,
    pub observation: Observation,
    pub signals: Vec<(AgentId, Amount, Option<Amount>)>,
    pub plan_claims: BTreeMap<ResourceId, Amount>,
}

impl RecordedTransition {
    /// A well-formed record against the genesis head: one claim on one
    /// resource, two council signals (the step-4 drive's signal pair).
    pub fn ordinary(id: TransitionId, resource: ResourceId, amount: Amount) -> Self {
        let mut declared = [0u8; 32];
        declared[0] = 0xD1;
        declared[1..9].copy_from_slice(&id[0..8]);
        let mut obs_id = [0xC0; 32];
        obs_id[1..9].copy_from_slice(&id[0..8]);
        Self {
            id,
            manifest: Manifest {
                transition_type: TT,
                parent_index: 0,
                parent_hash: [0; 32],
                declared_next_hash: declared,
                observation_id: obs_id,
                expires_at: BASE + 10_000,
                relevant_groups: BTreeSet::from([1]),
                action: None,
            },
            observation: Observation {
                id: obs_id,
                timestamp: BASE,
                expiry: BASE + 9000,
                confidence: amt(0.9),
            },
            signals: vec![
                (ag(1), amt(1.0), Some(amt(0.9))),
                (ag(2), amt(1.0), Some(amt(0.8))),
            ],
            plan_claims: BTreeMap::from([(resource, amount)]),
        }
    }
}

/// One recorded governance transition: the action, the council approvals
/// that precede registration, its observation, and its manifest. The plan
/// is always the empty-claims plan (a rule change claims no stock).
#[derive(Debug, Clone)]
pub struct RecordedGovernance {
    pub id: TransitionId,
    pub action: GovAction,
    pub approvals: Vec<AgentId>,
    pub observation: Observation,
    pub manifest: Manifest,
}

impl RecordedGovernance {
    /// A well-formed governance record: unanimous 3-seat approval of the
    /// action, manifest typed by the action's category, declared hash
    /// 0xE1 (the step-4 governance drive's marker).
    pub fn new(id: TransitionId, action: GovAction) -> Self {
        let mut obs_id = [0xC1; 32];
        obs_id[1..9].copy_from_slice(&id[0..8]);
        Self {
            id,
            action: action.clone(),
            approvals: vec![ag(1), ag(2), ag(3)],
            observation: Observation {
                id: obs_id,
                timestamp: BASE + 3,
                expiry: BASE + 9003,
                confidence: amt(0.9),
            },
            manifest: Manifest {
                transition_type: action.category(),
                parent_index: 0,
                parent_hash: [0; 32],
                declared_next_hash: [0xE1; 32],
                observation_id: obs_id,
                expires_at: BASE + 10_000,
                relevant_groups: BTreeSet::from([1]),
                action: None,
            },
        }
    }
}

/// Outcome of a replay, with the step trace that produced it.
#[derive(Debug)]
pub enum ReplayOutcome {
    /// Declared and committed: head index/hash and the stock levels after
    /// the debit (ordinary) or the action (governance) are in the trace's
    /// kernel, reported here as the committed head.
    Committed { head: SystemState, trace: Vec<&'static str> },
    /// Validate refused structurally. No commit was attempted; nothing moved.
    Blocked { life: Life, trace: Vec<&'static str> },
    /// Validate passed but the full predicate at declare refused. The mask
    /// is the predicate's own verdict; nothing moved.
    Refused { mask: FailMask, trace: Vec<&'static str> },
}

fn drive_signals(
    k: &mut LaunchKernel,
    id: &TransitionId,
    signals: &[(AgentId, Amount, Option<Amount>)],
    trace: &mut Vec<&'static str>,
) -> Result<(), KernelError> {
    for (i, (agent, raw, support)) in signals.iter().enumerate() {
        k.submit_round_signal(agent, id, *raw, *support, BASE + 20 + i as u64)?;
        trace.push("signal");
    }
    Ok(())
}

fn drive_added_steps(
    k: &mut LaunchKernel,
    id: &TransitionId,
    trace: &mut Vec<&'static str>,
) -> Result<(), KernelError> {
    k.evaluate_transition(id, BASE + 10)?;
    trace.push("evaluate");
    k.mark_eligible(id, BASE + 10)?;
    trace.push("eligible");
    Ok(())
}

fn drive_plan_and_validate(
    k: &mut LaunchKernel,
    id: &TransitionId,
    claims: BTreeMap<ResourceId, Amount>,
    trace: &mut Vec<&'static str>,
) -> Result<Life, KernelError> {
    k.select_candidate(id, BASE + 120)?;
    trace.push("select");
    k.open_planning(&PROPOSER, id, BASE + 130)?;
    trace.push("open_planning");
    k.submit_plan(
        &PROPOSER,
        id,
        Plan { agent: PROPOSER, claims, expires_at: BASE + 5000 },
        BASE + 140,
    )?;
    trace.push("submit_plan");
    k.reconcile_plans(id, BASE + 240)?;
    trace.push("reconcile");
    let _ = k.validate_transition(id, BASE + 245);
    trace.push("validate");
    Ok(k.life_of(id).unwrap_or(Life::Proposed))
}

/// Replay an ordinary transition from its record alone, keeping the
/// rebuilt kernel so callers can assert post-replay stocks and head.
pub fn replay_transition_full(
    record: &RecordedTransition,
) -> Result<(ReplayOutcome, LaunchKernel), KernelError> {
    let mut trace: Vec<&'static str> = vec![];
    let mut k = sealed_launch_kernel();
    k.ingest_observation(&OBSERVER, record.observation.clone(), BASE)?;
    trace.push("ingest_observation");
    k.register_transition(&PROPOSER, record.id, record.manifest.clone(), BTreeSet::new(), BASE)?;
    trace.push("register");
    drive_added_steps(&mut k, &record.id, &mut trace)?;
    drive_signals(&mut k, &record.id, &record.signals, &mut trace)?;
    let life = drive_plan_and_validate(&mut k, &record.id, record.plan_claims.clone(), &mut trace)?;
    match life {
        Life::Validated => {
            let mask = k.commit_fail_mask(&record.id, BASE + 245)?;
            if mask.is_clear() {
                trace.push("declare");
                k.declare_commit(&record.id, BASE + 245)?;
                trace.push("commit");
                let head = k.commit(&record.id, BASE + 245)?;
                Ok((ReplayOutcome::Committed { head, trace }, k))
            } else {
                trace.push("declare_refused");
                let _ = k.declare_commit(&record.id, BASE + 245);
                Ok((ReplayOutcome::Refused { mask, trace }, k))
            }
        }
        other => Ok((ReplayOutcome::Blocked { life: other, trace }, k)),
    }
}

/// Replay an ordinary transition from its record alone.
pub fn replay_transition(record: &RecordedTransition) -> Result<ReplayOutcome, KernelError> {
    Ok(replay_transition_full(record)?.0)
}

/// Replay a governance transition from its record alone, keeping the
/// rebuilt kernel so callers can assert the applied action's effect.
/// The approvals precede registration (the step-4 governance drive's
/// order); the rest of the lifecycle is the same added-steps sequence,
/// ending in `commit_governance` instead of `commit`.
pub fn replay_governance_full(
    record: &RecordedGovernance,
) -> Result<(ReplayOutcome, LaunchKernel), KernelError> {
    let mut trace: Vec<&'static str> = vec![];
    let mut k = sealed_launch_kernel();
    for (i, seat) in record.approvals.iter().enumerate() {
        k.approve_governance(seat, record.id, record.action.clone(), BASE + i as u64)?;
        trace.push("approve_governance");
    }
    k.ingest_observation(&OBSERVER, record.observation.clone(), BASE + 3)?;
    trace.push("ingest_observation");
    k.register_governance_transition(
        &PROPOSER,
        record.id,
        record.manifest.clone(),
        record.action.clone(),
        BASE + 3,
    )?;
    trace.push("register_governance");
    drive_added_steps(&mut k, &record.id, &mut trace)?;
    drive_signals(
        &mut k,
        &record.id,
        &[(ag(1), amt(1.0), Some(amt(0.9))), (ag(2), amt(1.0), Some(amt(0.8)))],
        &mut trace,
    )?;
    let life = drive_plan_and_validate(&mut k, &record.id, BTreeMap::new(), &mut trace)?;
    match life {
        Life::Validated => {
            let mask = k.commit_fail_mask(&record.id, BASE + 245)?;
            if mask.is_clear() {
                trace.push("declare");
                k.declare_commit(&record.id, BASE + 245)?;
                trace.push("commit_governance");
                let head = k.commit_governance(&record.id, BASE + 245)?;
                Ok((ReplayOutcome::Committed { head, trace }, k))
            } else {
                trace.push("declare_refused");
                let _ = k.declare_commit(&record.id, BASE + 245);
                Ok((ReplayOutcome::Refused { mask, trace }, k))
            }
        }
        other => Ok((ReplayOutcome::Blocked { life: other, trace }, k)),
    }
}

/// Replay a governance transition from its record alone.
pub fn replay_governance(record: &RecordedGovernance) -> Result<ReplayOutcome, KernelError> {
    Ok(replay_governance_full(record)?.0)
}

/// The added steps, in lifecycle order. A replay trace must contain them
/// as a subsequence — this is what distinguishes the v0.4 bridge from the
/// v0.2 bridge's shorter replay.
pub const ADDED_STEPS: [&str; 6] =
    ["evaluate", "eligible", "select", "reconcile", "validate", "declare"];

/// True iff `steps` appear in `trace` in order (other steps may interleave).
pub fn trace_contains_in_order(trace: &[&str], steps: &[&str]) -> bool {
    let mut idx = 0;
    for step in trace {
        if idx < steps.len() && *step == steps[idx] {
            idx += 1;
        }
    }
    idx == steps.len()
}
