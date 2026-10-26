//! Migration step 4, increment 1: the engine-sim governance adapter over the
//! v0.4 kernel lifecycle.
//!
//! This crate never edits the delivered v0.4 crate or the step-2 launch
//! profile: it consumes both by path, exactly as the v0.2 stack does, and it
//! never touches `Kernel/` in the repo. All v0.4 work stays in
//! `hidden_files/v04-intake/` until Carlos rules the notes' branching question
//! (branch vs replace outright once step 1 passed).
//!
//! What it implements (interface spec: `hidden_files/STEP4_ADAPTER_MAP.md`):
//!
//! * The v0.2 adapter contract, unchanged: `authorize()` returns
//!   `Some(Authorization)` iff the flow committed and money may move,
//!   `None` iff the predicate refused and no money moves, and a zero-amount
//!   flow is a no-op that never reaches the kernel. Settlement runs only
//!   after commit, against the recorded evidence hash, through the step-2
//!   `SettlerGate` (settle-after-realized; two distinct Settlers).
//! * The v0.4 lifecycle in full, not a shortcut: register -> evaluate ->
//!   eligible -> signals -> select -> planning -> plan -> reconcile ->
//!   validate -> declare -> commit -> start_execution -> gated attestation
//!   -> record_outcome. A per-flow observation is ingested at the drive's
//!   base time, so the schedule does not depend on the genesis observation's
//!   expiry; timestamps advance one monotonic base per drive.
//! * The K-1 rulings (Jeffrey, 2026-10-25 ~23:40 CST, via Carlos) as code:
//!   (1) carrying a blocked claim is ADAPTER-LEVEL — `carry_pending()`
//!   withdraws the blocked transition and re-registers the same claim
//!   against the current head under a NEW id, and the claim re-earns
//!   consensus through the full drive (no kernel carry mechanism, and
//!   ruling 2's council-discard build, when it lands, is a second exit,
//!   not this path); (3) carried claims compete under the EXISTING
//!   selection rule — the carry drive is byte-for-byte the same drive as
//!   any new flow, and re-registration resets eligibility time (the carry
//!   selects only after its own consensus window closes); (4) duplicate
//!   on re-plan is INTENDED — plans are frozen once submitted, so a
//!   smaller claim goes through as a new transition and never mutates the
//!   blocked one; (5) NO pro-rata / partial debit — a claim commits in
//!   full or not at all, and a blocked flow debits nothing.
//! * The blocked-slot hygiene the v0.4 lifecycle forces and v0.2 did not:
//!   a Blocked transition holds its parent's `selected_for` slot, so a
//!   blocked flow is WITHDRAWN (by its proposer) as soon as it is recorded
//!   as pending — otherwise every later same-parent flow could never
//!   select. Withdrawal frees the slot; the claim itself survives in
//!   `pending()` for an explicit later carry, it is not silently dropped.
//!
//! What it does NOT yet do (next step-4 increments, not this one):
//! * The engine itself (`engine-sim` lib/per_holder/adversarial) is not
//!   ported here, so T1–T9 are not yet re-run against this adapter; that
//!   is the next increment once this core is green.
//! * Per-flow stock refresh is a governance act in v0.4 (there is no direct
//!   admin `set_capacity`); `set_capacity_via_governance()` drives one
//!   such act (unanimous council, full lifecycle) for tests and for the
//!   engine's later stock management. It is a driven governance transition,
//!   not a shortcut around governance.
//! * F1 is CLOSED in the delivered v0.4.1 build (`Config::reputation_weighs`
//!   exists; the step-2 profile itself is pin-OFF). F2's commit-clone cost
//!   is gone from the commit path as implemented in v0.4.1
//!   (`commit_check`/`commit_apply`: check first, apply without a backup
//!   copy); step 5's cu-golden step still has to measure on-chain CU, which
//!   this adapter does not assume.

use coordination_kernel::*;
use kernel_launch_v04::*;
use std::collections::{BTreeMap, BTreeSet};

/// Proposer of every engine flow (launch-profile agent 1, a council seat).
pub const PROPOSER: AgentId = [1; 32]; // == ag(1), the launch-profile agent 1

/// A committed authorization awaiting post-execution settlement — the same
/// shape as the v0.2 adapter's, so the engine's contract does not change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Authorization {
    pub id: TransitionId,
    pub resource: ResourceId,
    pub amount: Amount,
    /// Outcome time; 0 marks the zero-amount no-op, which is never settled.
    pub settle_at: Timestamp,
}

impl Authorization {
    /// A zero-amount flow never reaches the kernel, so it must not be
    /// settled or counted — only real commits settle.
    pub fn is_real(&self) -> bool {
        self.settle_at != 0
    }
}

/// A blocked claim held for an explicit later carry: the original
/// transition id (already withdrawn, hence `Discarded`), its resource, and
/// its full, unmutated amount. Nothing here is a partial claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingClaim {
    pub blocked_id: TransitionId,
    pub resource: ResourceId,
    pub amount: Amount,
}

/// Outcome of one drive through the lifecycle.
enum DriveOutcome {
    Committed(Authorization),
    Blocked(PendingClaim),
}

pub struct EngineAdapter {
    kernel: LaunchKernel,
    ledger: EvidenceLedger,
    seq: u64,
    next: u64,
    pending: Vec<PendingClaim>,
}

impl EngineAdapter {
    /// The sealed step-2 launch profile, ready for engine flows.
    pub fn new() -> Self {
        Self {
            kernel: sealed_launch_kernel(),
            ledger: EvidenceLedger::new(),
            seq: 0,
            next: 1,
            pending: Vec::new(),
        }
    }

    /// Read access to the underlying kernel (life, capacity, declared head).
    pub fn kernel(&self) -> &LaunchKernel {
        &self.kernel
    }

    /// Blocked claims awaiting an explicit carry, in blocking order.
    pub fn pending(&self) -> &[PendingClaim] {
        &self.pending
    }

    /// Drive one engine flow (dividend tranche or lending batch — the
    /// caller picks the resource) through the full v0.4 lifecycle.
    ///
    /// `Some` iff the flow committed and settled (money may move);
    /// `None` iff the predicate refused (blocked: the claim is recorded in
    /// `pending()`, the blocked transition is withdrawn to free its parent
    /// slot, and no money moves). Zero amounts never reach the kernel.
    pub fn authorize(
        &mut self,
        resource: ResourceId,
        amount_units: f64,
    ) -> Result<Option<Authorization>, KernelError> {
        if amount_units <= 0.0 {
            return Ok(Some(Authorization {
                id: [0; 32],
                resource,
                amount: amt(0.0),
                settle_at: 0,
            }));
        }
        match self.drive_once(resource, amt(amount_units))? {
            DriveOutcome::Committed(auth) => Ok(Some(auth)),
            DriveOutcome::Blocked(claim) => {
                self.pending.push(claim);
                Ok(None)
            }
        }
    }

    /// K-1 ruling 1, the adapter-level carry: every pending claim is
    /// withdrawn (already done at block time) and RE-REGISTERED against the
    /// current head under a new id, re-earning consensus through the full
    /// drive. A claim that blocks again is re-recorded in `pending()` —
    /// still whole, still unmutated — and debits nothing. Returned
    /// authorizations are the claims that committed on this carry; an empty
    /// return with pending claims remaining is a re-block, not a loss.
    pub fn carry_pending(&mut self) -> Result<Vec<Authorization>, KernelError> {
        let claims = std::mem::take(&mut self.pending);
        let mut committed = Vec::new();
        for claim in claims {
            match self.drive_once(claim.resource, claim.amount)? {
                DriveOutcome::Committed(auth) => committed.push(auth),
                DriveOutcome::Blocked(again) => self.pending.push(again),
            }
        }
        Ok(committed)
    }

    /// Drive a governed capacity change (the v0.4 form of the v0.2
    /// adapter's per-flow `set_capacity`): unanimous council approval, then
    /// the full governance lifecycle to `commit_governance`. There is no
    /// admin shortcut — this is the governed path, driven, not bypassed.
    pub fn set_capacity_via_governance(
        &mut self,
        resource: ResourceId,
        capacity_units: f64,
    ) -> Result<(), KernelError> {
        let base = self.next_base();
        let id = self.next_id();
        let action = GovAction::SetCapacity { resource, capacity: amt(capacity_units) };
        for (i, seat) in [ag(1), ag(2), ag(3)].iter().enumerate() {
            self.kernel.approve_governance(seat, id, action.clone(), base + i as u64)?;
        }
        let mut obs_id = [0xC1; 32];
        obs_id[1..9].copy_from_slice(&(self.next - 1).to_be_bytes());
        // Ingested at base+3 (after the approvals at base..base+2): the
        // kernel clock is monotonic, so an ingest at base would regress.
        self.kernel.ingest_observation(
            &OBSERVER,
            Observation { id: obs_id, timestamp: base + 3, expiry: base + 9003, confidence: amt(0.9) },
            base + 3,
        )?;
        let mut m = self.manifest();
        m.transition_type = action.category();
        m.declared_next_hash = [0xE1; 32];
        m.observation_id = obs_id;
        m.expires_at = base + 10_000;
        self.kernel.register_governance_transition(&PROPOSER, id, m, action, base + 3)?;
        self.kernel.evaluate_transition(&id, base + 10)?;
        self.kernel.mark_eligible(&id, base + 10)?;
        self.kernel.submit_round_signal(&ag(1), &id, amt(1.0), Some(amt(0.9)), base + 20)?;
        self.kernel.submit_round_signal(&ag(2), &id, amt(1.0), Some(amt(0.8)), base + 21)?;
        self.kernel.select_candidate(&id, base + 120)?;
        self.kernel.open_planning(&PROPOSER, &id, base + 130)?;
        self.kernel.submit_plan(
            &PROPOSER,
            &id,
            Plan { agent: PROPOSER, claims: BTreeMap::new(), expires_at: base + 5000 },
            base + 140,
        )?;
        self.kernel.reconcile_plans(&id, base + 240)?;
        let _ = self.kernel.validate_transition(&id, base + 245);
        self.kernel.declare_commit(&id, base + 245)?;
        self.kernel.commit_governance(&id, base + 245)?;
        Ok(())
    }

    // ------------------------------------------------------------ internals
    /// One fresh monotonic base per drive (100, 1100, 2100, ...), so every
    /// drive's windows close in order and the kernel clock never regresses.
    fn next_base(&mut self) -> Timestamp {
        let base = 100 + self.seq * 1000;
        self.seq += 1;
        base
    }

    fn next_id(&mut self) -> TransitionId {
        let n = self.next;
        self.next += 1;
        let mut id = [0u8; 32];
        id[0..8].copy_from_slice(&n.to_be_bytes());
        id
    }

    /// Manifest against the kernel's CURRENT declared head — the carry's
    /// "new head" is whatever head the kernel has at drive time, never the
    /// head the blocked claim was registered against.
    fn manifest(&self) -> Manifest {
        let d = self.kernel.declared_state().clone();
        Manifest {
            transition_type: TT,
            parent_index: d.index,
            parent_hash: d.hash,
            declared_next_hash: [0; 32],
            observation_id: OBS,
            expires_at: 0,
            relevant_groups: BTreeSet::from([1]),
            action: None,
        }
    }

    /// One full lifecycle drive for a single claim (single planner, single
    /// resource, full amount). On Blocked, the transition is withdrawn
    /// here — freeing its parent slot — and the whole claim is returned
    /// for the caller to record as pending. Nothing is partially driven:
    /// a blocked drive never reaches declare/commit, and a validated drive
    /// always settles through the evidence gate before returning.
    fn drive_once(
        &mut self,
        resource: ResourceId,
        amount: Amount,
    ) -> Result<DriveOutcome, KernelError> {
        let base = self.next_base();
        let id = self.next_id();
        let mut declared = [0u8; 32];
        declared[0] = 0xD1;
        declared[1..9].copy_from_slice(&(self.next - 1).to_be_bytes());

        // Fresh observation for this drive (expiry well past every gate).
        let mut obs_id = [0xC0; 32];
        obs_id[1..9].copy_from_slice(&(self.next - 1).to_be_bytes());
        self.kernel.ingest_observation(
            &OBSERVER,
            Observation { id: obs_id, timestamp: base, expiry: base + 9000, confidence: amt(0.9) },
            base,
        )?;
        let mut m = self.manifest();
        m.declared_next_hash = declared;
        m.observation_id = obs_id;
        m.expires_at = base + 10_000;
        self.kernel.register_transition(&PROPOSER, id, m, BTreeSet::new(), base)?;
        self.kernel.evaluate_transition(&id, base + 10)?;
        self.kernel.mark_eligible(&id, base + 10)?;
        self.kernel.submit_round_signal(&ag(1), &id, amt(1.0), Some(amt(0.9)), base + 20)?;
        self.kernel.submit_round_signal(&ag(2), &id, amt(1.0), Some(amt(0.8)), base + 21)?;
        self.kernel.select_candidate(&id, base + 120)?;
        self.kernel.open_planning(&PROPOSER, &id, base + 130)?;
        self.kernel.submit_plan(
            &PROPOSER,
            &id,
            Plan {
                agent: PROPOSER,
                claims: BTreeMap::from([(resource, amount)]),
                expires_at: base + 5000,
            },
            base + 140,
        )?;
        self.kernel.reconcile_plans(&id, base + 240)?;
        let _ = self.kernel.validate_transition(&id, base + 245);
        match self.kernel.life_of(&id) {
            Some(Life::Blocked) => {
                // Free the parent slot NOW; the claim survives in pending().
                self.kernel.withdraw(&PROPOSER, &id, base + 260)?;
                Ok(DriveOutcome::Blocked(PendingClaim { blocked_id: id, resource, amount }))
            }
            Some(Life::Validated) => {
                self.kernel.declare_commit(&id, base + 245)?;
                self.kernel.commit(&id, base + 245)?;
                // Settle-after-realized: record the evidence hash first,
                // then route both Settlers through the gate. A wrong hash
                // or missing evidence would refuse without consuming a vote
                // (launch l6); here the evidence IS the declared outcome.
                self.kernel.start_execution(&EXECUTOR, &id, base + 300)?;
                self.ledger.record(id, declared);
                let gate = SettlerGate::new(&self.ledger);
                let first = gate.attest_settlement(&mut self.kernel, &SETTLER_A, &id, declared, base + 310)?;
                debug_assert_eq!(first, GateDecision::Attested(false));
                let second = gate.attest_settlement(&mut self.kernel, &SETTLER_B, &id, declared, base + 311)?;
                debug_assert_eq!(second, GateDecision::Attested(true));
                self.kernel.record_outcome(&RECORDER, &id, &[], base + 320)?;
                Ok(DriveOutcome::Committed(Authorization {
                    id,
                    resource,
                    amount,
                    settle_at: base + 320,
                }))
            }
            other => Err(KernelError::BadLifecycle(other.unwrap_or(Life::Proposed))),
        }
    }
}

impl Default for EngineAdapter {
    fn default() -> Self {
        Self::new()
    }
}
