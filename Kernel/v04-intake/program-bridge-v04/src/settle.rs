//! Migration step 5, slice 4: settlement replay over the slice-1 bridge.
//!
//! Commit is not the end of the lifecycle — it is the gate to execution
//! and settlement, and the program rewrite must replay those steps too:
//! a committed transition executes, settlers attest the realized outcome,
//! and the outcome is recorded. This slice replays that tail on a
//! slice-1-committed kernel, through the step-2 `SettlerGate` composition
//! (evidence-bound settlement: settlers may attest only the recorded
//! evidence hash — Carlos's standing rule; the delivered v0.4 `Env` has
//! no evidence method, so the gate is a composition around the kernel,
//! not a kernel property, and this slice proves the composition, not a
//! claim that the kernel enforces it).
//!
//! The settle tail, in order, after a slice-1 commit at BASE+245:
//! * `begin_execution` — the launch Executor moves Committed → Executing
//!   (execution deadline is commit + 1000, so the settle times below are
//!   well inside it; the kernel clock is monotonic, never regressing).
//! * Two gated attestations — threshold 2, both settlers attesting the
//!   same recorded evidence hash. The first returns `Attested(false)`
//!   (below threshold), the second `Attested(true)` and moves the
//!   transition to Settled with `realized` set.
//! * `record_outcome` — the launch OutcomeRecorder closes Settled →
//!   EvaluatedOutcome. The attribution list is empty here: the bridge
//!   proves the settle tail's lifecycle and evidence binding, not
//!   reputation arithmetic (the delivered kernel's outcome math —
//!   order-independent summation, ±cap, positive-credit halving on
//!   inexact outcomes — is the author's tested surface, and the
//!   launch profile pins reputation influence OFF in any case).
//!
//! What this slice proves, and what it does not:
//! * A committed replay settles exactly, and only, on the recorded
//!   evidence hash; gate refusals (no evidence, wrong hash) consume no
//!   vote, so a refused settler can still attest correctly afterwards.
//! * Settle before commit is refused at both doors: execution cannot
//!   start on a Blocked transition, and attestation cannot run on a
//!   merely Committed one.
//! * Two settlers attesting different hashes never settle: the kernel
//!   counts votes per realized hash, so disagreement leaves the
//!   transition Executing with no realized hash — it cannot be forced
//!   through by a second vote from either side (a repeat vote is
//!   `Duplicate`, not a changed vote).
//! * The same tail runs for a governance replay: a rule change commits,
//!   executes, and settles like any transition.
//! * NOT proven here: on-chain CU for the settle tail (cu-golden rides
//!   the program rewrite), and attribution deltas (empty list above).

use crate::BASE;
use coordination_kernel::*;
use kernel_launch_v04::*;

/// Execution starts after commit (commit at BASE+245).
pub const EXECUTE_AT: Timestamp = BASE + 300;
/// First settler attestation.
pub const ATTEST_A_AT: Timestamp = BASE + 310;
/// Second settler attestation (reaches the threshold of 2).
pub const ATTEST_B_AT: Timestamp = BASE + 320;
/// Outcome recording after settlement.
pub const OUTCOME_AT: Timestamp = BASE + 330;
/// A forward time after settlement, for post-settle refusal tests: it
/// clears the monotonic clock so a re-execution / re-attestation attempt
/// is refused by the lifecycle check itself, not by time regression.
pub const POST_SETTLE_AT: Timestamp = BASE + 400;

/// Outcome of a gated settlement: both settlers' gate decisions and the
/// outcome recorder's exact flag (true iff realized == declared).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettleOutcome {
    pub first: GateDecision,
    pub second: GateDecision,
    pub exact: bool,
}

/// Move a Committed transition to Executing. The caller is the launch
/// Executor; any other caller, or a non-Committed transition, errors.
pub fn begin_execution(
    k: &mut LaunchKernel,
    id: &TransitionId,
) -> Result<(), KernelError> {
    k.start_execution(&EXECUTOR, id, EXECUTE_AT)
}

/// Settle an Executing transition through the evidence gate and record
/// the (empty-attribution) outcome. `evidence` must be the realized
/// outcome hash recorded by the observing process after the outcome
/// existed — for a bridge replay, the record's declared hash when the
/// execution matched its declaration exactly.
pub fn settle_with_evidence(
    k: &mut LaunchKernel,
    id: &TransitionId,
    evidence: Hash32,
) -> Result<SettleOutcome, KernelError> {
    let mut ledger = EvidenceLedger::new();
    ledger.record(*id, evidence);
    let gate = SettlerGate::new(&ledger);
    let first = gate.attest_settlement(k, &SETTLER_A, id, evidence, ATTEST_A_AT)?;
    let second = gate.attest_settlement(k, &SETTLER_B, id, evidence, ATTEST_B_AT)?;
    let exact = k.record_outcome(&RECORDER, id, &[], OUTCOME_AT)?;
    Ok(SettleOutcome { first, second, exact })
}
