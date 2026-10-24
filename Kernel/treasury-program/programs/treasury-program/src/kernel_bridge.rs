//! Bridge from the treasury program to the pristine Coordination Kernel.
//!
//! Design: `files/OWN_IDL_DESIGN.md` §1 (placement C) and §4.1 (launch
//! policy table). The kernel crate is compiled into the program — the full
//! 13-bit `commit_fail_mask` is computed on-chain, and there is NO digest
//! publisher. This module is the wiring: the launch policy, the launch
//! config, and a kernel constructor with the launch profile's hooks.
//!
//! # What the launch profile is, exactly
//!
//! * Weights: `FlatWeights` — reputation is recorded from the first
//!   settlement but has zero influence on quorum (design §4.1, roadmap
//!   W2.4). The kernel's default `FlatWeights` is exactly this.
//! * Attestation: the program uses the kernel's default `RoleAttestor`
//!   (Oracle role may observe/settle) as the in-kernel hook — NOT
//!   `kernel-policy`'s `EvidenceAttestor`. That attestor is `std`-only
//!   (`Rc`/`RefCell` shared log) and cannot be vendored into an SBF
//!   program. The evidence-bound settle rule the design requires
//!   (settle-after-realized, design §4.1) is therefore enforced at the
//!   program layer, not inside the kernel hook: the `settle` instruction
//!   (governance increment) checks the presented realized hash against the
//!   on-chain `EvidenceRecord` account — written by `record_evidence`
//!   only after the realized outcome exists — before it ever calls the
//!   kernel's `settle`. No record, or a mismatched hash, refuses; the
//!   transition stays `Committed` for a correct settlement to retry.
//!   This is the same fail-closed shape as the `EvidenceAttestor`, with
//!   the log kept in a program account instead of an `Rc`.
//!
//! # The state discipline, stated before the governance increment
//!
//! The pristine crate offers no serialization or state-restore API: a
//! `Kernel` cannot be rehydrated from accounts. The governance increment
//! therefore does NOT pretend to load/store a kernel. Each instruction
//! records its inputs on the `TransitionRecord` account (design §3), and
//! the `commit` instruction rebuilds the transition's kernel state by
//! replaying those recorded inputs through the kernel's public guarded
//! entry points, in lifecycle order, then calls `commit_fail_mask` /
//! `commit` on the replayed kernel. Replay is deterministic and uses only
//! the public API, so the on-chain predicate judges exactly the recorded
//! inputs — never a publisher's say-so. The tests in this module prove the
//! replay shape on the host: a well-formed lifecycle clears the mask and
//! commits; an over-claim fails COMPAT and commits nothing.
//!
//! # Genesis parameters carried, not invented
//!
//! `PLANNING_WINDOW` carries the engine-sim governance adapter's proven
//! value unchanged (engine-sim `governance.rs`). Its seconds
//! interpretation at genesis is a named parameter (design §9), not a
//! ruling — the body of the W4.3 task says to carry the proven value, and
//! this constant is that carry, labeled as such.

use coordination_kernel::{
    Amount, Config, FlatWeights, Kernel, KernelError, Policy, RoleAttestor, SystemState,
};

/// Kernel time units for the planning window, carried from the engine-sim
/// governance adapter (see module docs). Design §9 parameter, not a ruling.
pub const PLANNING_WINDOW: u64 = 10;

/// Launch policy per transition type (design §4.1 table, all types at
/// launch): quorum 1.0, confidence 0.5, minimum observation confidence 0.5,
/// objectives not required, 3 rounds, 4 plans.
pub fn launch_policy() -> Policy {
    Policy {
        quorum_required: Amount::ONE,
        confidence_threshold: Amount::from_raw(500_000),
        min_obs_confidence: Amount::from_raw(500_000),
        require_objectives: false,
        planning_window: PLANNING_WINDOW,
        max_rounds: 3,
        max_plans: 4,
    }
}

/// Launch kernel config (design §4.1): reputation bounded 0–100,
/// per-settlement attribution capped at ±20. Matches `kernel-policy`'s
/// `launch_kernel` config exactly (reputation 0–100, cap 20 units).
pub fn launch_config() -> Config {
    Config {
        rep_min: Amount::ZERO,
        rep_max: Amount::units(100).expect("100 units fits"),
        max_attribution: Amount::units(20).expect("20 units fits"),
    }
}

/// A kernel with the launch profile: default role attestation (see module
/// docs for why the evidence check lives at the program layer) and flat
/// weights. The caller is the founding Admin.
pub fn launch_kernel(
    admin: [u8; 32],
    genesis: SystemState,
) -> Result<Kernel<RoleAttestor, FlatWeights>, KernelError> {
    Kernel::new(admin, genesis, launch_config())
}

#[cfg(test)]
mod tests {
    use super::*;
    use coordination_kernel::{
        AgentProfile, ConstraintStatus, FailMask, Manifest, Observation, Plan, Role, TState,
    };
    // Test code builds for the host only (never SBF), so std collections apply.
    use std::collections::{BTreeMap, BTreeSet};

    // Test-only ids, mirroring the engine-sim governance adapter.
    const ADMIN: [u8; 32] = [0xA0; 32];
    const ORACLE: [u8; 32] = [0xA1; 32];
    const EVALUATOR: [u8; 32] = [0xA2; 32];
    const PROPOSER: [u8; 32] = [1; 32];
    const CONSTRAINT: [u8; 32] = [0xC1; 32];
    const TT_TRANCHE: [u8; 32] = [0xD1; 32];
    const RESOURCE_TRANCHE: u16 = 0;

    // Fixed-point literals at SCALE 1_000_000 (kernel num.rs).
    const HALF: Amount = Amount::from_raw(500_000);
    const NINE_TENTHS: Amount = Amount::from_raw(900_000);

    fn setup_kernel() -> Kernel<RoleAttestor, FlatWeights> {
        let genesis = SystemState { index: 0, hash: [0; 32], timestamp: 1_000 };
        let mut kernel = launch_kernel(ADMIN, genesis).expect("genesis kernel");
        kernel.grant_role(&ADMIN, ORACLE, Role::Oracle).expect("oracle role");
        kernel.grant_role(&ADMIN, EVALUATOR, Role::Evaluator).expect("evaluator role");
        kernel
            .set_mandatory_constraints(&ADMIN, TT_TRANCHE, BTreeSet::from([CONSTRAINT]))
            .expect("mandatory constraint");
        kernel
            .register_agent(
                &ADMIN,
                AgentProfile {
                    id: PROPOSER,
                    active: true,
                    groups: BTreeSet::from([1]),
                    influence: Amount::ONE,
                    reputation: Amount::units(50).expect("50 units fits"),
                },
            )
            .expect("proposer agent");
        kernel
    }

    /// Drive one transition through the full lifecycle to Committable.
    /// Returns the transition id. `claim_raw` is the plan's claim on the
    /// tranche stock; `stock_raw` is the stock set before the lifecycle.
    fn drive_to_committable(
        kernel: &mut Kernel<RoleAttestor, FlatWeights>,
        claim_raw: u64,
        stock_raw: u64,
    ) -> [u8; 32] {
        let base: u64 = 1_100;
        kernel
            .set_capacity(&ADMIN, RESOURCE_TRANCHE, Amount::from_raw(stock_raw))
            .expect("stock set");
        let mut id = [0u8; 32];
        id[0] = 1;
        let mut declared = [0u8; 32];
        declared[0] = 1;
        let mut obs_id = [0xB0; 32];
        obs_id[1] = 7;
        let parent = kernel.declared_state().clone();
        kernel
            .post_observation(
                &ORACLE,
                Observation { id: obs_id, timestamp: base, expiry: base + 9_000, confidence: NINE_TENTHS },
                base,
            )
            .expect("observation posted");
        kernel
            .register_transition(
                &PROPOSER,
                Manifest {
                    id,
                    transition_type: TT_TRANCHE,
                    parent_index: parent.index,
                    parent_hash: parent.hash,
                    declared_next_hash: declared,
                    observation_id: obs_id,
                    expires_at: base + 10_000,
                    relevant_groups: BTreeSet::from([1]),
                },
                launch_policy(),
                BTreeSet::new(),
                base,
            )
            .expect("transition registered");
        kernel
            .report_constraint(&EVALUATOR, &id, CONSTRAINT, ConstraintStatus::Pass, base + 1)
            .expect("constraint reported");
        kernel
            .submit_signal(&PROPOSER, &id, Amount::ONE, NINE_TENTHS, base + 2)
            .expect("signal submitted");
        kernel.open_planning(&PROPOSER, &id, base + 3).expect("planning opened");
        kernel
            .submit_plan(
                &PROPOSER,
                Plan {
                    agent: PROPOSER,
                    claims: BTreeMap::from([(RESOURCE_TRANCHE, Amount::from_raw(claim_raw))]),
                    expires_at: base + 5_000,
                },
                &id,
                base + 4,
            )
            .expect("plan submitted");
        // Planning deadline is base + 3 + PLANNING_WINDOW (10) = base + 13.
        kernel.reconcile(&PROPOSER, &id, base + 50).expect("reconciled");
        id
    }

    #[test]
    fn launch_policy_matches_design_table() {
        let pol = launch_policy();
        assert_eq!(pol.quorum_required, Amount::ONE);
        assert_eq!(pol.confidence_threshold, HALF);
        assert_eq!(pol.min_obs_confidence, HALF);
        assert!(!pol.require_objectives);
        assert_eq!(pol.planning_window, PLANNING_WINDOW);
        assert_eq!(pol.max_rounds, 3);
        assert_eq!(pol.max_plans, 4);
    }

    #[test]
    fn launch_config_matches_kernel_policy_profile() {
        let cfg = launch_config();
        assert_eq!(cfg.rep_min, Amount::ZERO);
        assert_eq!(cfg.rep_max, Amount::units(100).expect("100 units fits"));
        assert_eq!(cfg.max_attribution, Amount::units(20).expect("20 units fits"));
    }

    #[test]
    fn fail_mask_bit_values_match_design() {
        // Design §1.1, bit by bit, in mask order.
        assert_eq!(FailMask::CONSTRAINTS, 1);
        assert_eq!(FailMask::OBJECTIVES, 2);
        assert_eq!(FailMask::QUORUM, 4);
        assert_eq!(FailMask::CONFIDENCE, 8);
        assert_eq!(FailMask::COMPAT, 16);
        assert_eq!(FailMask::EXCEPTIONS, 32);
        assert_eq!(FailMask::FRESH, 64);
        assert_eq!(FailMask::PLANNING_OPEN, 128);
        assert_eq!(FailMask::EXPIRED, 256);
        assert_eq!(FailMask::LINEAGE, 512);
        assert_eq!(FailMask::PLANS, 1024);
        assert_eq!(FailMask::CAPABILITY, 2048);
        assert_eq!(FailMask::DIALECT, 4096);
    }

    #[test]
    fn well_formed_transition_clears_mask_and_commits() {
        let mut kernel = setup_kernel();
        let id = drive_to_committable(&mut kernel, 1_000_000, 1_000_000);
        let mask = kernel.commit_fail_mask(&id, 1_150).expect("mask computed");
        assert!(mask.is_clear(), "mask should be clear, got {:?}", mask);
        let committed = kernel.commit(&id, 1_150).expect("commit lands");
        assert_eq!(committed.index, 1);
        assert_eq!(kernel.state_of(&id), Some(TState::Committed));
        // The stock was debited exactly by the claim.
        assert_eq!(kernel.capacity_of(RESOURCE_TRANCHE), Some(Amount::ZERO));
    }

    #[test]
    fn over_claim_fails_compat_and_commits_nothing() {
        let mut kernel = setup_kernel();
        let id = drive_to_committable(&mut kernel, 2_000_000, 1_000_000);
        let mask = kernel.commit_fail_mask(&id, 1_150).expect("mask computed");
        assert!(mask.has(FailMask::COMPAT), "COMPAT must be set, got {:?}", mask);
        let err = kernel.commit(&id, 1_150).expect_err("commit must refuse");
        assert!(matches!(err, KernelError::CommitFailed(m) if m.has(FailMask::COMPAT)));
        // Nothing moved: still Committable, stock untouched.
        assert_eq!(kernel.state_of(&id), Some(TState::Committable));
        assert_eq!(kernel.capacity_of(RESOURCE_TRANCHE), Some(Amount::from_raw(1_000_000)));
    }

}
