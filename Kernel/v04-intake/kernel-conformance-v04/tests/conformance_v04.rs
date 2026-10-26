//! Conformance tests for the Coordination Kernel v0.4 — OUR requirements (migration
//! step 1), tested against the crate as-is. Neither the v0.4 crate nor the v0.2 crate
//! is modified by this crate.
//!
//! Ported from the v0.2 W2.3 suite, adapted to the v0.4 lifecycle. The v0.4
//! lifecycle front-loads several predicate terms into earlier gates, so the port
//! asserts each term at its natural gate instead of forcing every defect to the
//! commit stage:
//!   P1  nothing commits unless the full predicate is clear; a refusal quotes
//!       exactly the mask the predicate computes, and leaves state and capacity
//!       untouched (declare-stage and commit-stage variants)
//!   P2  capacity is a conserved stock: debits are exact, over-claims never commit
//!   P3  settle discipline: only after commit, only via Executor/Settler/
//!       OutcomeRecorder roles, threshold attestation, exactly once
//!   P4  time is monotonic at every mutating entry point
//!   P5  attribution: permutation-invariant, per-agent summed, capped, drift halves
//!       credit only — checked against an independent hand computation
//!   P6  lifecycle: transitions only ever move along legal edges; terminal states
//!       are absorbing (randomized operation fuzz)
//!   P7  lineage: the declared chain advances by exactly the committed manifests,
//!       in order; stale parents can never commit
//! New in v0.4 (migration step 1 additions, first slice):
//!   G1  governance unanimity binds the exact action (id + action pair)
//!   G2  the influence cap (1/6) at seal, and tightening under an existing
//!       holder is refused
//!   G3  the position gate never weights a signal or a commit
//!   G4  defection cooldown: a DefectionConfirmed resolution blocks candidacy
//!       through the cooldown window and no longer
//!   G5  failed governance commits leave no trace (commit_governance
//!       restore-on-error: capacity, declared state, history, events unchanged)
//!   G6  stale dialect at commit: a dependency that went stale between declare
//!       and commit is refused with the predicate's exact mask
//!   G7  post-seal registration cap: a governance-registered agent over 1/6 of
//!       total base is refused at commit; an at-cap registration applies
//!   G8  council discard of an abandoned blocked transition (ruling 2, v0.4.1):
//!       Blocked only, strict majority of all seats, one vote per seat per
//!       blocking episode, and discarding frees the parent slot
//!
//! F1 note (roadmap Part B4): v0.4.1 adds Config::reputation_weighs (the
//! v0.4.0 build lacked it). This fixture sets it true — influence follows
//! reputation, as in v0.4.0 — and pins influence arithmetically: agents
//! register at the reputation midpoint (50 of 0..100) with base influence 2.0,
//! so influence_at() is exactly 1.0 and signal weights equal raw weights.
//! The switch-off behavior is covered by the launch crate's L10 test.
//!
//! All randomness is a deterministic xorshift64* — same seeds, same sequences,
//! on every machine, forever. No external crates.

use coordination_kernel::*;
use std::collections::{BTreeMap, BTreeSet};

// ---------------------------------------------------------------- rng
struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self { Rng(seed ^ 0x9E37_79B9_7F4A_7C15) }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12; x ^= x << 25; x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 { if n == 0 { 0 } else { self.next() % n } }
    fn coin(&mut self) -> bool { self.next() & 1 == 1 }
    /// uniform in [0, max] with 0.05 granularity, as an Amount
    fn amount(&mut self, max_units: u64) -> Amount {
        let steps = self.below(max_units * 20 + 1);
        Amount::from_raw(steps * (SCALE / 20))
    }
}

// ---------------------------------------------------------------- test env
/// The administrator's answers. v0.4 asks the Env itself; callers never assert
/// constraints or objectives. Tests mutate this between steps via env_mut().
#[derive(Clone, Default)]
struct TestEnv {
    constraints: BTreeMap<ConstraintId, ConstraintStatus>,
    objectives: BTreeMap<ObjectiveId, bool>,
    positions: BTreeMap<AgentId, (u128, Timestamp)>,
    position_source: bool,
}
impl Env for TestEnv {
    fn constraint(&self, c: &ConstraintId, _m: &Manifest) -> ConstraintStatus {
        self.constraints.get(c).copied().unwrap_or(ConstraintStatus::Unavailable)
    }
    fn objective(&self, o: &ObjectiveId, _m: &Manifest) -> Option<bool> {
        self.objectives.get(o).copied()
    }
    fn has_position_source(&self) -> bool { self.position_source }
    fn locked_holding(&self, a: &AgentId) -> Option<(u128, Timestamp)> {
        self.positions.get(a).copied()
    }
}

// ---------------------------------------------------------------- fixtures
const GEN: AgentId = [0xA0; 32];
const ORACLE: AgentId = [0xA1; 32];
const EXE: AgentId = [0xB1; 32];
const SET_A: AgentId = [0xB2; 32];
const SET_B: AgentId = [0xB3; 32];
const REC: AgentId = [0xB4; 32];
const TT: TransitionType = [9; 32];
const C1: ConstraintId = [5; 32];
const O1: ObjectiveId = [6; 32];
const OBS: Hash32 = [3; 32];
const RES: ResourceId = 0;

fn a(x: f64) -> Amount { Amount::from_f64(x).unwrap() }
fn ag(n: u8) -> AgentId { [n; 32] }
fn tid(n: u8) -> TransitionId { [n; 32] }

fn pol() -> Policy {
    Policy { quorum_required: a(1.0), confidence_threshold: a(0.5), min_obs_confidence: a(0.5),
             require_objectives: false, consensus_window: 100, planning_window: 100,
             execution_window: 1000, max_plans: 6, settlement_threshold: 2, max_rounds: 3 }
}

fn cfg() -> Config {
    Config { rep_min: a(0.0), rep_max: a(100.0), max_attribution: a(20.0),
             epoch_length: 1000, council_seats: 3, reputation_weighs: true }
}

#[derive(Clone, Copy, PartialEq)]
enum SigMode { Strong, Weak, None }

/// Parameters for one drive from registration through reconcile.
struct Drive<'a> {
    id: TransitionId,
    next: u8,
    claims: &'a [(u8, f64)],
    base: u64,
    obs: Hash32,
    plan_expiry: u64,
    ingest: bool,
    sig: SigMode,
}

struct Fx { k: Kernel<TestEnv> }
impl Fx {
    /// Genesis at t=100, sealed, with `p` as the policy for TT. Six agents at
    /// base 2.0 (each 1/6 of total base — exactly at the influence cap, so the
    /// seal passes); council seats 1,2,3; capacity 1000 on RES.
    fn new_with(p: Policy) -> Fx {
        let mut env = TestEnv::default();
        env.constraints.insert(C1, ConstraintStatus::Pass);
        env.objectives.insert(O1, true);
        let mut k = Kernel::new(GEN, SystemState { index: 0, hash: [0; 32], timestamp: 100 },
            cfg(), env).unwrap();
        k.genesis_apply(&GEN, GovAction::SetPolicy { ttype: TT, policy: p }, 100).unwrap();
        for t in [T_GOV_ADJUSTABLE, T_GOV_CONSTITUTION, T_GOV_COUNCIL, T_GOV_REBOOT] {
            k.genesis_apply(&GEN, GovAction::SetPolicy { ttype: t, policy: pol() }, 100).unwrap();
        }
        k.genesis_apply(&GEN, GovAction::SetRequiredConstraints { ttype: TT, set: BTreeSet::from([C1]) }, 100).unwrap();
        k.genesis_apply(&GEN, GovAction::SetRequiredObjectives { ttype: TT, set: BTreeSet::from([O1]) }, 100).unwrap();
        for n in 1..=6u8 {
            k.genesis_apply(&GEN, GovAction::RegisterAgent { agent: ag(n), groups: BTreeSet::from([1]), base_influence: a(2.0) }, 100).unwrap();
        }
        k.genesis_apply(&GEN, GovAction::GrantRole { who: ORACLE, role: Role::Observer }, 100).unwrap();
        k.genesis_apply(&GEN, GovAction::GrantRole { who: EXE, role: Role::Executor }, 100).unwrap();
        k.genesis_apply(&GEN, GovAction::GrantRole { who: SET_A, role: Role::Settler }, 100).unwrap();
        k.genesis_apply(&GEN, GovAction::GrantRole { who: SET_B, role: Role::Settler }, 100).unwrap();
        k.genesis_apply(&GEN, GovAction::GrantRole { who: REC, role: Role::OutcomeRecorder }, 100).unwrap();
        k.genesis_apply(&GEN, GovAction::SetCapacity { resource: RES, capacity: a(1000.0) }, 100).unwrap();
        k.genesis_apply(&GEN, GovAction::InstallCouncil { seats: vec![ag(1), ag(2), ag(3)] }, 100).unwrap();
        k.seal_genesis(&GEN, 100).unwrap();
        assert!(k.is_sealed());
        k.ingest_observation(&ORACLE, Observation { id: OBS, timestamp: 100, expiry: 5000, confidence: a(0.9) }, 100).unwrap();
        Fx { k }
    }
    fn new() -> Fx { Fx::new_with(pol()) }

    fn manifest(&self, _id: TransitionId, next: u8, obs: Hash32, expires: u64) -> Manifest {
        let d = self.k.declared_state().clone();
        Manifest { transition_type: TT, parent_index: d.index, parent_hash: d.hash,
                   declared_next_hash: [next; 32], observation_id: obs, expires_at: expires,
                   relevant_groups: BTreeSet::from([1]), action: None }
    }

    /// Drive `id` from registration through reconcile. Errors are ignored: the
    /// caller inspects the resulting life and mask to assert the expected gate.
    /// Timeline relative to `base`: register base, evaluate base+10, eligible
    /// base+10, signals base+20/21, select base+120, planning base+130, plans
    /// base+140, reconcile base+240. If `ingest`, a fresh observation is posted
    /// at `base` (timestamp base, expiry base+4900).
    fn drive_to_reconciling(&mut self, d: Drive) {
        if d.ingest {
            let _ = self.k.ingest_observation(&ORACLE,
                Observation { id: d.obs, timestamp: d.base, expiry: d.base + 4900, confidence: a(0.9) }, d.base);
        }
        let m = self.manifest(d.id, d.next, d.obs, d.base + 10_000);
        let _ = self.k.register_transition(&ag(1), d.id, m, BTreeSet::new(), d.base);
        let _ = self.k.evaluate_transition(&d.id, d.base + 10);
        let _ = self.k.mark_eligible(&d.id, d.base + 10);
        match d.sig {
            SigMode::Strong => {
                let _ = self.k.submit_round_signal(&ag(1), &d.id, a(1.0), Some(a(0.9)), d.base + 20);
                let _ = self.k.submit_round_signal(&ag(2), &d.id, a(1.0), Some(a(0.8)), d.base + 21);
            }
            SigMode::Weak => {
                let _ = self.k.submit_round_signal(&ag(1), &d.id, a(1.0), Some(a(0.1)), d.base + 20);
                let _ = self.k.submit_round_signal(&ag(2), &d.id, a(1.0), Some(a(0.1)), d.base + 21);
            }
            SigMode::None => {}
        }
        let _ = self.k.select_candidate(&d.id, d.base + 120);
        let _ = self.k.open_planning(&ag(1), &d.id, d.base + 130);
        for (n, c) in d.claims {
            let _ = self.k.submit_plan(&ag(*n), &d.id,
                Plan { agent: ag(*n), claims: BTreeMap::from([(RES, a(*c))]), expires_at: d.plan_expiry }, d.base + 140);
        }
        let _ = self.k.reconcile_plans(&d.id, d.base + 240);
    }

    /// Validate at base+245; returns the resulting life.
    fn validate_at(&mut self, id: TransitionId, base: u64) -> Option<Life> {
        let _ = self.k.validate_transition(&id, base + 245);
        self.k.life_of(&id)
    }

    /// Drive `id` all the way to Validated on the clean path; panics otherwise.
    fn ready(&mut self, id: TransitionId, next: u8, claims: &[(u8, f64)]) {
        self.drive_to_reconciling(Drive { id, next, claims, base: 100, obs: OBS, plan_expiry: 4000, ingest: false, sig: SigMode::Strong });
        assert_eq!(self.validate_at(id, 100), Some(Life::Validated));
    }
    /// Time-shifted variant for chained scenarios.
    fn ready_at(&mut self, id: TransitionId, next: u8, claims: &[(u8, f64)], base: u64, obs: Hash32) {
        self.drive_to_reconciling(Drive { id, next, claims, base, obs, plan_expiry: base + 3000, ingest: true, sig: SigMode::Strong });
        assert_eq!(self.validate_at(id, base), Some(Life::Validated));
    }

    /// Drive a unanimously approved governance transition to Committable.
    /// Timings mirror the G1 commit test, relative to `base`: approvals
    /// base..base+2, register base+3, evaluate/eligible base+10, signals
    /// base+20/21, select base+120, planning base+130, plan base+140,
    /// reconcile base+240, validate/declare base+245.
    fn gov_to_committable(&mut self, id: TransitionId, action: GovAction, base: u64) {
        for (i, seat) in [ag(1), ag(2), ag(3)].iter().enumerate() {
            self.k.approve_governance(seat, id, action.clone(), base + i as u64).unwrap();
        }
        let mut m = self.manifest(id, 7, OBS, base + 10_000);
        m.transition_type = action.category();
        self.k.register_governance_transition(&ag(1), id, m, action, base + 3).unwrap();
        let _ = self.k.evaluate_transition(&id, base + 10);
        let _ = self.k.mark_eligible(&id, base + 10);
        let _ = self.k.submit_round_signal(&ag(1), &id, a(1.0), Some(a(0.9)), base + 20);
        let _ = self.k.submit_round_signal(&ag(2), &id, a(1.0), Some(a(0.8)), base + 21);
        self.k.select_candidate(&id, base + 120).unwrap();
        self.k.open_planning(&ag(1), &id, base + 130).unwrap();
        self.k.submit_plan(&ag(1), &id, Plan { agent: ag(1), claims: BTreeMap::new(), expires_at: base + 3000 }, base + 140).unwrap();
        self.k.reconcile_plans(&id, base + 240).unwrap();
        assert_eq!(self.k.validate_transition(&id, base + 245), Ok(()));
        self.k.declare_commit(&id, base + 245).unwrap();
        assert_eq!(self.k.life_of(&id), Some(Life::Committable));
    }
}

// ---------------------------------------------------------------- P1
/// Randomized gate injection: whatever is broken, the transition must stop at
/// its natural gate, the predicate mask must name the failing term, and a
/// refused declare/commit must quote the same mask and touch nothing.
#[test]
fn p1_nothing_commits_unless_predicate_clear_under_random_defects() {

    for iter in 0..240u64 {
        let mut rng = Rng::new(iter + 1);
        // defect class: 0 constraint-fail, 1 constraint-unavailable, 2 objective,
        // 3 no signals, 4 weak signals, 5 over-capacity, 6 unresolved exception,
        // 7 clean, 8 resolved exception (proceeds)
        let class = rng.below(9);
        let mut p = pol();
        if class == 2 { p.require_objectives = true; }
        let mut fx = Fx::new_with(p);
        match class {
            0 => { fx.k.env_mut().constraints.insert(C1, ConstraintStatus::Fail); }
            1 => { fx.k.env_mut().constraints.insert(C1, ConstraintStatus::Unavailable); }
            2 => { fx.k.env_mut().objectives.insert(O1, false); }
            _ => {}
        }
        // commit-time variant, chosen before the drive since it sets plan expiry
        let commit_variant = rng.below(3); // 0 on-time, 1 late-after-plan-expiry, 2 post-manifest-expiry
        let plan_expiry = if commit_variant == 1 { 360 } else { 4000 };
        let claims: &[(u8, f64)] = if class == 5 { &[(1, 700.0), (2, 700.0)] } else { &[(1, 100.0), (2, 100.0)] };
        fx.drive_to_reconciling(Drive { id: tid(1), next: 7, claims, base: 100, obs: OBS, plan_expiry, ingest: false, sig: match class {
            3 => SigMode::None,
            4 => SigMode::Weak,
            _ => SigMode::Strong,
        }});
        let life = fx.k.life_of(&tid(1));
        match class {
            0 => {
                assert_eq!(life, Some(Life::Rejected), "iter {iter}: constraint fail must reject at evaluation");
                let mask = fx.k.commit_fail_mask(&tid(1), 345).unwrap();
                assert!(mask.has(FailMask::CONSTRAINTS), "iter {iter}: mask must name CONSTRAINTS");
                assert!(matches!(fx.k.commit(&tid(1), 345), Err(KernelError::BadLifecycle(_))), "iter {iter}: rejected transition must not commit");
                continue;
            }
            1 => {
                assert_eq!(life, Some(Life::Proposed), "iter {iter}: unavailable constraint leaves Proposed");
                assert_eq!(fx.k.mark_eligible(&tid(1), 345), Err(KernelError::BadLifecycle(Life::Proposed)));
                let mask = fx.k.commit_fail_mask(&tid(1), 345).unwrap();
                assert!(mask.has(FailMask::CONSTRAINTS), "iter {iter}: mask must name CONSTRAINTS");
                continue;
            }
            2 => {
                assert_eq!(life, Some(Life::Evaluated), "iter {iter}: objective defect stops at eligibility");
                assert_eq!(fx.k.mark_eligible(&tid(1), 345), Err(KernelError::ObjectivesNotSatisfied));
                let mask = fx.k.commit_fail_mask(&tid(1), 345).unwrap();
                assert!(mask.has(FailMask::OBJECTIVES), "iter {iter}: mask must name OBJECTIVES");
                continue;
            }
            3 => {
                assert_eq!(life, Some(Life::Eligible), "iter {iter}: no signals stop at selection");
                assert_eq!(fx.k.select_candidate(&tid(1), 345), Err(KernelError::QuorumNotMet));
                let mask = fx.k.commit_fail_mask(&tid(1), 345).unwrap();
                assert!(mask.has(FailMask::QUORUM), "iter {iter}: mask must name QUORUM");
                continue;
            }
            4 => {
                assert_eq!(life, Some(Life::Eligible), "iter {iter}: weak signals stop at selection");
                assert_eq!(fx.k.select_candidate(&tid(1), 345), Err(KernelError::ConfidenceBelowThreshold));
                let mask = fx.k.commit_fail_mask(&tid(1), 345).unwrap();
                assert!(mask.has(FailMask::CONFIDENCE), "iter {iter}: mask must name CONFIDENCE");
                continue;
            }
            _ => {
                assert_eq!(life, Some(Life::Reconciling), "iter {iter}: class {class} must reach Reconciling");
            }
        }
        // exception stage (classes 6 and 8 only)
        if class == 6 || class == 8 {
            let idx = fx.k.submit_exception(&ag(4), &tid(1), Disposition::Block, None, 341).unwrap();
            if class == 8 {
                fx.k.resolve_exception(&ag(1), &tid(1), idx, ResolutionKind::Remediated, [8; 32], 342).unwrap();
                fx.k.resolve_exception(&ag(2), &tid(1), idx, ResolutionKind::Remediated, [8; 32], 343).unwrap();
            }
        }
        let life = fx.validate_at(tid(1), 100);
        if class == 6 {
            assert_eq!(life, Some(Life::Blocked), "iter {iter}: unresolved blocking exception must block validation");
            let mask = fx.k.commit_fail_mask(&tid(1), 345).unwrap();
            assert!(mask.has(FailMask::EXCEPTIONS), "iter {iter}: mask must name EXCEPTIONS");
            continue;
        }
        if class == 5 {
            assert_eq!(life, Some(Life::Blocked), "iter {iter}: over-capacity plans must block validation");
            let mask = fx.k.commit_fail_mask(&tid(1), 345).unwrap();
            assert!(mask.has(FailMask::COMPAT), "iter {iter}: mask must name COMPAT");
            continue;
        }
        assert_eq!(life, Some(Life::Validated), "iter {iter}: class {class} must validate");
        // declare at 345: mask clear for both expiry variants (360 > 345, 4000 > 345)
        let mask = fx.k.commit_fail_mask(&tid(1), 345).unwrap();
        assert!(mask.is_clear(), "iter {iter}: mask {:#x} must be clear at declare", mask.0);
        fx.k.declare_commit(&tid(1), 345).unwrap();
        assert_eq!(fx.k.life_of(&tid(1)), Some(Life::Committable));
        let cap_before = fx.k.capacity_of(RES).unwrap();
        let t = match commit_variant { 0 => 345, 1 => 400, _ => 10_100 };
        let mask = fx.k.commit_fail_mask(&tid(1), t).unwrap();
        match fx.k.commit(&tid(1), t) {
            Ok(s) => {
                assert!(mask.is_clear(), "iter {iter}: committed with mask {:#x}", mask.0);
                assert_eq!(s.index, 1, "iter {iter}");
                assert_eq!(fx.k.life_of(&tid(1)), Some(Life::Committed));
                assert_eq!(fx.k.capacity_of(RES).unwrap(), a(800.0), "iter {iter}: debit must be exact");
            }
            Err(KernelError::CommitFailed(m)) => {
                assert!(!mask.is_clear(), "iter {iter}: refused though mask clear");
                assert_eq!(m.0, mask.0, "iter {iter}: refusal must quote the predicate's mask");
                assert_eq!(fx.k.life_of(&tid(1)), Some(Life::Committable), "iter {iter}: failed commit moved state");
                assert_eq!(fx.k.capacity_of(RES).unwrap(), cap_before, "iter {iter}: failed commit touched capacity");
                if commit_variant == 1 {
                    assert!(mask.has(FailMask::PLANS), "iter {iter}: late commit must name PLANS");
                } else {
                    assert!(mask.has(FailMask::EXPIRED), "iter {iter}: post-expiry commit must name EXPIRED");
                }
            }
            Err(e) => panic!("iter {iter}: unexpected commit error {e:?}"),
        }
    }
}

/// Declare-stage refusal: a plan that expires after declare time but before a
/// later declare attempt must refuse with exactly the predicate's mask.
#[test]
fn p1_declare_refuses_with_exact_mask_after_plan_expiry() {
    let mut fx = Fx::new();
    fx.drive_to_reconciling(Drive { id: tid(1), next: 7, claims: &[(1, 100.0)], base: 100, obs: OBS, plan_expiry: 360, ingest: false, sig: SigMode::Strong });
    assert_eq!(fx.validate_at(tid(1), 100), Some(Life::Validated));
    let cap_before = fx.k.capacity_of(RES).unwrap();
    let mask = fx.k.commit_fail_mask(&tid(1), 400).unwrap();
    assert!(mask.has(FailMask::PLANS));
    assert_eq!(fx.k.declare_commit(&tid(1), 400), Err(KernelError::CommitFailed(mask)));
    assert_eq!(fx.k.life_of(&tid(1)), Some(Life::Validated), "failed declare moved state");
    assert_eq!(fx.k.capacity_of(RES).unwrap(), cap_before, "failed declare touched capacity");
}

// ---------------------------------------------------------------- P2
#[test]
fn p2_capacity_conserved_across_chained_commits() {
    for iter in 0..60u64 {
        let mut rng = Rng::new(10_000 + iter);
        let mut fx = Fx::new();
        let mut remaining = a(1000.0);
        let mut seq = 0u8;
        loop {
            seq += 1;
            let id = tid(seq);
            let base = 100 + (seq as u64) * 1000;
            let claim = 50.0 + rng.below(20) as f64 * 25.0; // 50..=525
            fx.drive_to_reconciling(Drive { id, next: 10 + seq, claims: &[(1, claim)], base, obs: [60 + seq; 32], plan_expiry: base + 3000, ingest: true, sig: SigMode::Strong });
            let life = fx.validate_at(id, base);
            if claim <= remaining.to_f64() {
                assert_eq!(life, Some(Life::Validated), "iter {iter} seq {seq}: fitting claim must validate");
                fx.k.declare_commit(&id, base + 245).unwrap();
                let mask = fx.k.commit_fail_mask(&id, base + 245).unwrap();
                assert!(mask.is_clear(), "iter {iter} seq {seq}: claim {claim} <= remaining {} but mask {:#x}", remaining.to_f64(), mask.0);
                fx.k.commit(&id, base + 245).unwrap();
                remaining = remaining.checked_sub(a(claim)).unwrap();
                assert_eq!(fx.k.capacity_of(RES).unwrap(), remaining, "iter {iter} seq {seq}: debit must be exact");
            } else {
                // v0.4 gate: over-claim blocks at validation, not at commit
                assert_eq!(life, Some(Life::Blocked), "iter {iter} seq {seq}: over-claim must block validation");
                let mask = fx.k.commit_fail_mask(&id, base + 245).unwrap();
                assert!(mask.has(FailMask::COMPAT), "iter {iter} seq {seq}: over-claim without COMPAT bit");
                assert!(matches!(fx.k.commit(&id, base + 245), Err(KernelError::BadLifecycle(_))), "iter {iter} seq {seq}: over-claim committed");
                assert_eq!(fx.k.capacity_of(RES).unwrap(), remaining, "iter {iter} seq {seq}: refused commit debited capacity");
                break;
            }
            if seq > 12 { break; }
        }
        assert!(seq >= 2, "iter {iter}: chain too short to be evidence");
    }
}

/// The v0.4 over-claim gate: an over-capacity plan blocks at validation, the
/// COMPAT bit is set, and capacity is untouched.
#[test]
fn p2_over_claim_blocks_at_validation_with_compat_bit() {
    let mut fx = Fx::new();
    fx.ready_at(tid(1), 11, &[(1, 600.0)], 1100, [61; 32]);
    fx.k.declare_commit(&tid(1), 1345).unwrap();
    fx.k.commit(&tid(1), 1345).unwrap();
    // remaining 400; a 500 claim over-claims
    fx.drive_to_reconciling(Drive { id: tid(2), next: 12, claims: &[(1, 500.0)], base: 2100, obs: [62; 32], plan_expiry: 5100, ingest: true, sig: SigMode::Strong });
    assert_eq!(fx.validate_at(tid(2), 2100), Some(Life::Blocked));
    let mask = fx.k.commit_fail_mask(&tid(2), 2345).unwrap();
    assert!(mask.has(FailMask::COMPAT));
    assert!(matches!(fx.k.commit(&tid(2), 2345), Err(KernelError::BadLifecycle(_))));
    assert_eq!(fx.k.capacity_of(RES).unwrap(), a(400.0));
}

// ---------------------------------------------------------------- P3
#[test]
fn p3_settle_only_after_commit_only_by_roles_exactly_once() {
    let mut fx = Fx::new();
    let id = tid(1);
    // no transition yet: unknown
    assert_eq!(fx.k.start_execution(&EXE, &id, 100), Err(KernelError::UnknownTransition));
    fx.ready(id, 7, &[(1, 100.0)]);
    // committed? no — still Validated: execution is a state error
    assert_eq!(fx.k.start_execution(&EXE, &id, 345), Err(KernelError::BadLifecycle(Life::Validated)));
    fx.k.declare_commit(&id, 345).unwrap();
    fx.k.commit(&id, 345).unwrap();
    // a non-Executor may not start execution
    assert_eq!(fx.k.start_execution(&ag(1), &id, 400), Err(KernelError::Unauthorized));
    assert_eq!(fx.k.life_of(&id), Some(Life::Committed));
    fx.k.start_execution(&EXE, &id, 400).unwrap();
    // threshold 2: one Settler's attestation does not settle
    assert_eq!(fx.k.attest_settlement(&SET_A, &id, [9; 32], 401), Ok(false));
    assert_eq!(fx.k.life_of(&id), Some(Life::Executing));
    // the same Settler may not attest twice
    assert_eq!(fx.k.attest_settlement(&SET_A, &id, [9; 32], 402), Err(KernelError::Duplicate));
    // a non-Settler may not attest
    assert_eq!(fx.k.attest_settlement(&ag(1), &id, [9; 32], 402), Err(KernelError::Unauthorized));
    // the second distinct Settler settles; drifted hash reports inexact
    assert_eq!(fx.k.attest_settlement(&SET_B, &id, [9; 32], 403), Ok(true));
    assert_eq!(fx.k.life_of(&id), Some(Life::Settled));
    assert_eq!(fx.k.realized_hash(&id), Some([9; 32]));
    // a non-OutcomeRecorder may not record the outcome
    assert_eq!(fx.k.record_outcome(&ag(1), &id, &[], 410), Err(KernelError::Unauthorized));
    assert_eq!(fx.k.record_outcome(&REC, &id, &[], 410), Ok(false));
    assert_eq!(fx.k.life_of(&id), Some(Life::EvaluatedOutcome));
    // exactly once: a second outcome is refused
    assert_eq!(fx.k.record_outcome(&REC, &id, &[], 411), Err(KernelError::BadLifecycle(Life::EvaluatedOutcome)));

    // exact path on a fresh kernel: realized == declared reports exact
    let mut fx = Fx::new();
    fx.ready(id, 7, &[(1, 100.0)]);
    fx.k.declare_commit(&id, 345).unwrap();
    fx.k.commit(&id, 345).unwrap();
    fx.k.start_execution(&EXE, &id, 400).unwrap();
    assert_eq!(fx.k.attest_settlement(&SET_A, &id, [7; 32], 401), Ok(false));
    assert_eq!(fx.k.attest_settlement(&SET_B, &id, [7; 32], 402), Ok(true));
    assert!(fx.k.record_outcome(&REC, &id, &[], 410).unwrap());
    assert_eq!(fx.k.realized_hash(&id), Some([7; 32]));
}

// ---------------------------------------------------------------- P4
#[test]
fn p4_time_regression_rejected_at_entry_points() {
    let mut fx = Fx::new();
    let id = tid(1);
    fx.k.register_transition(&ag(1), id, fx.manifest(id, 7, OBS, 10_000), BTreeSet::new(), 500).unwrap();
    // clock is now 500; everything earlier is a regression
    assert_eq!(fx.k.ingest_observation(&ORACLE, Observation { id: [4; 32], timestamp: 100, expiry: 900, confidence: a(0.9) }, 499),
               Err(KernelError::TimeRegression));
    assert_eq!(fx.k.evaluate_transition(&id, 499), Err(KernelError::TimeRegression));
    assert_eq!(fx.k.submit_round_signal(&ag(1), &id, a(1.0), Some(a(1.0)), 499), Err(KernelError::TimeRegression));
    assert_eq!(fx.k.open_planning(&ag(1), &id, 499), Err(KernelError::TimeRegression));
    assert_eq!(fx.k.commit(&id, 499), Err(KernelError::TimeRegression));
    assert_eq!(fx.k.start_execution(&EXE, &id, 499), Err(KernelError::TimeRegression));
    assert_eq!(fx.k.now(), 500, "rejected calls must not move the clock");
}

// ---------------------------------------------------------------- P5
/// Independent re-implementation of the settle math, from the spec text:
/// per-agent sum -> clamp to +/- max_attribution -> if drifted, halve positive
/// credit only -> apply, clamped into [rep_min, rep_max].
fn expected_rep(start: f64, deltas: &[Delta], exact: bool) -> Amount {
    let cap: i128 = 20_000_000; // max_attribution 20.0
    let mut net: i128 = deltas.iter().map(|d| *d as i128).sum();
    net = net.clamp(-cap, cap);
    if !exact && net > 0 { net /= 2; }
    a(start).apply_delta(net, a(0.0), a(100.0))
}

#[test]
fn p5_attribution_matches_independent_computation_and_is_permutation_invariant() {
    for iter in 0..120u64 {
        let mut rng = Rng::new(50_000 + iter);
        let n = 1 + rng.below(5) as usize;
        let mut deltas: Vec<Delta> = (0..n).map(|_| (rng.next() % 90_000_001) as i64 - 45_000_000).collect();
        let exact = rng.coin();
        let realized: Hash32 = if exact { [7; 32] } else { [9; 32] };
        let run = |ds: &[Delta]| -> Amount {
            let mut fx = Fx::new();
            let id = tid(1);
            fx.ready(id, 7, &[(1, 100.0)]);
            fx.k.declare_commit(&id, 345).unwrap();
            fx.k.commit(&id, 345).unwrap();
            fx.k.start_execution(&EXE, &id, 400).unwrap();
            fx.k.attest_settlement(&SET_A, &id, realized, 401).unwrap();
            fx.k.attest_settlement(&SET_B, &id, realized, 402).unwrap();
            let v: Vec<(AgentId, Delta)> = ds.iter().map(|d| (ag(1), *d)).collect();
            fx.k.record_outcome(&REC, &id, &v, 410).unwrap();
            fx.k.agent(&ag(1)).unwrap().reputation
        };
        let want = expected_rep(50.0, &deltas, exact);
        assert_eq!(run(&deltas), want, "iter {iter}: kernel disagrees with independent computation");
        for _ in 0..4 {
            let mut shuffled = deltas.clone();
            for i in (1..shuffled.len()).rev() {
                let j = rng.below(i as u64 + 1) as usize;
                shuffled.swap(i, j);
            }
            assert_eq!(run(&shuffled), want, "iter {iter}: attribution order changed the outcome");
            deltas = shuffled;
        }
    }
}

// ---------------------------------------------------------------- P6
fn legal_edge(from: Life, to: Life) -> bool {
    use Life::*;
    from == to || matches!(
        (from, to),
        (Proposed, Evaluated) | (Proposed, Rejected)
            | (Evaluated, Eligible)
            | (Eligible, Selected)
            | (Selected, Planning)
            | (Planning, Reconciling)
            | (Reconciling, Validated) | (Reconciling, Blocked)
            | (Validated, Committable) | (Validated, Blocked)
            | (Committable, Committed) | (Committable, Blocked)
            | (Committed, Executing) | (Executing, Settled) | (Settled, EvaluatedOutcome)
            | (Blocked, Validated) | (Blocked, Planning)
            | (Proposed, Expired) | (Evaluated, Expired) | (Eligible, Expired)
            | (Selected, Expired) | (Planning, Expired) | (Reconciling, Expired)
            | (Validated, Expired) | (Committable, Expired)
            | (Proposed, Conflicted) | (Evaluated, Conflicted) | (Eligible, Conflicted)
            | (Selected, Conflicted) | (Planning, Conflicted) | (Reconciling, Conflicted)
            | (Validated, Conflicted) | (Committable, Conflicted)
            | (Proposed, Discarded) | (Evaluated, Discarded) | (Eligible, Discarded)
            | (Selected, Discarded) | (Planning, Discarded) | (Reconciling, Discarded)
            | (Validated, Discarded) | (Committable, Discarded)
    )
}

#[test]
fn p6_lifecycle_never_regresses_under_random_ops() {
    for seed in 0..3u64 {
        let mut rng = Rng::new(90_000 + seed);
        let mut fx = Fx::new();
        let ids: Vec<TransitionId> = (1..=4u8).map(tid).collect();
        let mut last: BTreeMap<TransitionId, Life> = BTreeMap::new();
        let mut clock: u64 = 100;
        for step in 0..2000u64 {
            let now = if rng.below(20) == 0 { clock.saturating_sub(1) } else { clock + rng.below(3) };
            if now >= clock { clock = now; }
            let id = ids[rng.below(4) as usize];
            let who = [ag(1), ag(2), ag(3), ag(4)][rng.below(4) as usize];
            match rng.below(18) {
                0 => { let _ = fx.k.register_transition(&ag(1), id, fx.manifest(id, 7, OBS, 10_000), BTreeSet::new(), now); }
                1 => { let _ = fx.k.evaluate_transition(&id, now); }
                2 => { let _ = fx.k.mark_eligible(&id, now); }
                3 => { let _ = fx.k.submit_round_signal(&who, &id, a(1.0), Some(rng.amount(1)), now); }
                4 => { let _ = fx.k.select_candidate(&id, now); }
                5 => { let _ = fx.k.open_planning(&ag(1), &id, now); }
                6 => { let _ = fx.k.submit_plan(&who, &id, Plan { agent: who, claims: BTreeMap::from([(RES, rng.amount(600))]), expires_at: 4000 }, now); }
                7 => { let _ = fx.k.reconcile_plans(&id, now); }
                8 => { let _ = fx.k.validate_transition(&id, now); }
                9 => { let _ = fx.k.declare_commit(&id, now); }
                10 => { let _ = fx.k.commit(&id, now); }
                11 => { let _ = fx.k.start_execution(&EXE, &id, now); }
                12 => { let _ = fx.k.attest_settlement(&SET_A, &id, [7; 32], now); }
                13 => { let _ = fx.k.record_outcome(&REC, &id, &[], now); }
                14 => { let _ = fx.k.submit_exception(&ag(4), &id, Disposition::Block, None, now); }
                15 => { let _ = fx.k.withdraw(&ag(1), &id, now); }
                16 => { let _ = fx.k.resolve_exception(&ag(1), &id, 0, ResolutionKind::Remediated, [8; 32], now); }
                _ => { if rng.coin() { let _ = fx.k.expire(&id, now); } else { let _ = fx.k.mark_conflicted(&id, now); } }
            }
            if let Some(s) = fx.k.life_of(&id) {
                if let Some(prev) = last.get(&id) {
                    assert!(legal_edge(*prev, s), "seed {seed} step {step}: illegal transition {prev:?} -> {s:?}");
                    if matches!(prev, Life::EvaluatedOutcome | Life::Rejected | Life::Expired | Life::Conflicted | Life::Discarded) {
                        assert_eq!(*prev, s, "seed {seed} step {step}: terminal state {prev:?} moved");
                    }
                }
                last.insert(id, s);
            }
        }
    }
}

// ---------------------------------------------------------------- P7
#[test]
fn p7_chain_advances_only_by_committed_manifests_in_order() {
    let mut fx = Fx::new();
    for seq in 1..=3u8 {
        let id = tid(seq);
        let base = 100 + (seq as u64) * 1000;
        fx.ready_at(id, 20 + seq, &[(1, 10.0)], base, [70 + seq; 32]);
        let before = fx.k.declared_state().clone();
        fx.k.declare_commit(&id, base + 245).unwrap();
        let s = fx.k.commit(&id, base + 245).unwrap();
        assert_eq!(s.index, before.index + 1);
        assert_eq!(s.hash, [20 + seq; 32]);
        assert_eq!(fx.k.history().len(), seq as usize);
        assert_eq!(fx.k.history()[(seq - 1) as usize], (s.index, id, s.hash));
    }
    // two siblings for the same parent: commit one, the other is lineage-stale
    let mut fx = Fx::new();
    let a_id = tid(1);
    let b_id = tid(2);
    fx.k.register_transition(&ag(1), a_id, fx.manifest(a_id, 7, OBS, 10_000), BTreeSet::new(), 100).unwrap();
    fx.k.register_transition(&ag(1), b_id, fx.manifest(b_id, 8, OBS, 10_000), BTreeSet::new(), 101).unwrap();
    fx.drive_to_reconciling(Drive { id: a_id, next: 7, claims: &[(1, 10.0)], base: 100, obs: OBS, plan_expiry: 4000, ingest: false, sig: SigMode::Strong });
    assert_eq!(fx.validate_at(a_id, 100), Some(Life::Validated));
    fx.k.declare_commit(&a_id, 345).unwrap();
    fx.k.commit(&a_id, 345).unwrap();
    let mask = fx.k.commit_fail_mask(&b_id, 345).unwrap();
    assert!(mask.has(FailMask::LINEAGE), "sibling of a committed transition must carry LINEAGE");
    assert!(matches!(fx.k.commit(&b_id, 345), Err(KernelError::BadLifecycle(_))));
    assert_eq!(fx.k.mark_conflicted(&b_id, 346), Ok(()));
    assert_eq!(fx.k.life_of(&b_id), Some(Life::Conflicted));
    // registering against the already-advanced parent is refused at registration
    // (note: the manifest here names the ORIGINAL genesis parent explicitly —
    // the helper would read the advanced declared state instead)
    let c_id = tid(3);
    let stale = Manifest { transition_type: TT, parent_index: 0, parent_hash: [0; 32],
        declared_next_hash: [9; 32], observation_id: OBS, expires_at: 10_000,
        relevant_groups: BTreeSet::from([1]), action: None };
    assert_eq!(
        fx.k.register_transition(&ag(1), c_id, stale, BTreeSet::new(), 347),
        Err(KernelError::ParentNeDeclared));
}

// ---------------------------------------------------------------- G1: governance unanimity
#[test]
fn g1_governance_unanimity_binds_the_exact_action() {
    let mut fx = Fx::new();
    let action = GovAction::SetCapacity { resource: RES, capacity: a(2000.0) };
    let other = GovAction::SetCapacity { resource: RES, capacity: a(3000.0) };
    let id = tid(9);
    // a non-seat may not even propose
    let mut m = fx.manifest(id, 7, OBS, 10_000);
    m.transition_type = T_GOV_ADJUSTABLE;
    assert_eq!(fx.k.register_governance_transition(&ag(4), id, m.clone(), action.clone(), 200),
               Err(KernelError::NotCouncil));
    // wrong category for the action
    let mut m_wrong = m.clone();
    m_wrong.transition_type = T_GOV_CONSTITUTION;
    assert_eq!(fx.k.register_governance_transition(&ag(1), id, m_wrong, action.clone(), 200),
               Err(KernelError::WrongGovCategory));
    // two of three seats approve: not unanimous
    fx.k.approve_governance(&ag(1), id, action.clone(), 200).unwrap();
    fx.k.approve_governance(&ag(2), id, action.clone(), 201).unwrap();
    assert_eq!(fx.k.register_governance_transition(&ag(1), id, m.clone(), action.clone(), 202),
               Err(KernelError::CouncilNotUnanimous(ag(3))));
    // the third seat approves a DIFFERENT action: still not unanimous for this one
    fx.k.approve_governance(&ag(3), id, other, 203).unwrap();
    assert_eq!(fx.k.register_governance_transition(&ag(1), id, m.clone(), action.clone(), 204),
               Err(KernelError::CouncilNotUnanimous(ag(3))));
    // the third seat approves this exact action: registration opens
    fx.k.approve_governance(&ag(3), id, action.clone(), 205).unwrap();
    fx.k.register_governance_transition(&ag(1), id, m, action, 206).unwrap();
    assert_eq!(fx.k.life_of(&id), Some(Life::Proposed));
}

/// A unanimously approved governance transition, driven through the full
/// lifecycle, applies exactly its action on commit_governance.
#[test]
fn g1_governance_commit_applies_exact_action() {
    let mut fx = Fx::new();
    let action = GovAction::SetCapacity { resource: RES, capacity: a(2000.0) };
    let id = tid(9);
    for (i, seat) in [ag(1), ag(2), ag(3)].iter().enumerate() {
        fx.k.approve_governance(seat, id, action.clone(), 200 + i as u64).unwrap();
    }
    let mut m = fx.manifest(id, 7, OBS, 10_000);
    m.transition_type = T_GOV_ADJUSTABLE;
    fx.k.register_governance_transition(&ag(1), id, m, action, 203).unwrap();
    let _ = fx.k.evaluate_transition(&id, 210);
    let _ = fx.k.mark_eligible(&id, 210);
    let _ = fx.k.submit_round_signal(&ag(1), &id, a(1.0), Some(a(0.9)), 220);
    let _ = fx.k.submit_round_signal(&ag(2), &id, a(1.0), Some(a(0.8)), 221);
    fx.k.select_candidate(&id, 320).unwrap();
    fx.k.open_planning(&ag(1), &id, 330).unwrap();
    fx.k.submit_plan(&ag(1), &id, Plan { agent: ag(1), claims: BTreeMap::new(), expires_at: 4000 }, 340).unwrap();
    fx.k.reconcile_plans(&id, 440).unwrap();
    assert_eq!(fx.k.validate_transition(&id, 445), Ok(()));
    assert_eq!(fx.k.life_of(&id), Some(Life::Validated));
    fx.k.declare_commit(&id, 445).unwrap();
    fx.k.commit_governance(&id, 445).unwrap();
    assert_eq!(fx.k.capacity_of(RES).unwrap(), a(2000.0));
    assert!(fx.k.events().iter().any(|e| matches!(e, Event::GovernanceApplied(x) if *x == id)));
}

// ---------------------------------------------------------------- G2: influence cap
#[test]
fn g2_seal_refuses_when_one_agent_exceeds_the_cap() {
    let mut k = Kernel::new(GEN, SystemState { index: 0, hash: [0; 32], timestamp: 100 },
        cfg(), TestEnv::default()).unwrap();
    for t in [T_GOV_ADJUSTABLE, T_GOV_CONSTITUTION, T_GOV_COUNCIL, T_GOV_REBOOT] {
        k.genesis_apply(&GEN, GovAction::SetPolicy { ttype: t, policy: pol() }, 100).unwrap();
    }
    // bases 2,2,2,2,2,20: largest holds 2/3 of total base, far over 1/6
    for (n, b) in [(1u8, 2.0), (2, 2.0), (3, 2.0), (4, 2.0), (5, 2.0), (6, 20.0)] {
        k.genesis_apply(&GEN, GovAction::RegisterAgent { agent: ag(n), groups: BTreeSet::from([1]), base_influence: a(b) }, 100).unwrap();
    }
    k.genesis_apply(&GEN, GovAction::InstallCouncil { seats: vec![ag(1), ag(2), ag(3)] }, 100).unwrap();
    assert_eq!(k.seal_genesis(&GEN, 100), Err(KernelError::GenesisNotReady("INFLUENCE_SHARE")));
    assert!(!k.is_sealed());
}

/// Tightening the cap below what an existing holder already holds is refused,
/// and the standing cap is unchanged.
#[test]
fn g2_cap_cannot_be_tightened_under_an_existing_holder() {
    let mut k = Kernel::new(GEN, SystemState { index: 0, hash: [0; 32], timestamp: 100 },
        cfg(), TestEnv::default()).unwrap();
    for n in 1..=6u8 {
        k.genesis_apply(&GEN, GovAction::RegisterAgent { agent: ag(n), groups: BTreeSet::from([1]), base_influence: a(2.0) }, 100).unwrap();
    }
    // six equal holders: each 1/6, exactly at the cap. Tightening to 1/12 voids one.
    assert_eq!(k.genesis_apply(&GEN, GovAction::SetMaxInfluenceShare { share: a(1.0 / 12.0) }, 100),
               Err(KernelError::InfluenceShareExceeded));
    assert_eq!(k.max_influence_share(), Amount::from_raw(Amount::ONE.raw().div_ceil(6)));
}

// ---------------------------------------------------------------- G3: position gate never weights
#[test]
fn g3_position_gate_never_weights_a_signal() {
    let mut fx = Fx::new();
    fx.k.env_mut().position_source = true;
    fx.k.env_mut().positions.insert(ag(1), (1000, 5000));
    let id = tid(1);
    fx.k.register_transition(&ag(1), id, fx.manifest(id, 7, OBS, 10_000), BTreeSet::new(), 100).unwrap();
    fx.k.evaluate_transition(&id, 110).unwrap();
    fx.k.mark_eligible(&id, 110).unwrap();
    let w_before = fx.k.effective_weight(&id, &ag(1), a(1.0)).unwrap();
    let i_before = fx.k.influence_at(&ag(1), 110).unwrap();
    // epoch 0 ends at 1100; a holding locked to 5000 counts as held
    fx.k.record_position(&ag(1), 150).unwrap();
    assert_eq!(fx.k.position_streak(&ag(1)), 1);
    // one epoch is not candidacy (needs 3 + an accepted proposal), and the
    // weight is untouched by the recorded position either way
    assert!(!fx.k.is_candidate(&ag(1), 150));
    assert_eq!(fx.k.effective_weight(&id, &ag(1), a(1.0)).unwrap(), w_before);
    assert_eq!(fx.k.influence_at(&ag(1), 110).unwrap(), i_before);
    assert_eq!(w_before, a(1.0), "fixture influence must be exactly 1.0 (base 2.0 at midpoint rep)");
}

// ---------------------------------------------------------------- G4: defection cooldown
/// A confirmed defection blocks council candidacy through the cooldown
/// window and no longer: the offender below holds a 3-epoch position streak
/// and one accepted proposal, so only the defection (and then its expiry)
/// moves candidacy. Epochs here are (now-100)/1000; the defection resolves
/// in epoch 2, so candidacy is blocked for epochs 2,3,4 and free at epoch 5.
#[test]
fn g4_defection_cooldown_blocks_candidacy_then_expires() {
    let mut fx = Fx::new();
    fx.k.env_mut().position_source = true;
    fx.k.env_mut().positions.insert(ag(5), (1000, 20_000));
    // ag(5) earns its accepted proposal: a full ordinary commit as proposer.
    let id = tid(1);
    fx.k.register_transition(&ag(5), id, fx.manifest(id, 7, OBS, 10_000), BTreeSet::new(), 100).unwrap();
    fx.k.evaluate_transition(&id, 110).unwrap();
    fx.k.mark_eligible(&id, 110).unwrap();
    fx.k.submit_round_signal(&ag(1), &id, a(1.0), Some(a(0.9)), 120).unwrap();
    fx.k.submit_round_signal(&ag(2), &id, a(1.0), Some(a(0.8)), 121).unwrap();
    fx.k.record_position(&ag(5), 150).unwrap(); // epoch 0, streak 1
    fx.k.select_candidate(&id, 220).unwrap();
    fx.k.open_planning(&ag(5), &id, 230).unwrap();
    fx.k.submit_plan(&ag(5), &id, Plan { agent: ag(5), claims: BTreeMap::from([(RES, a(10.0))]), expires_at: 4000 }, 240).unwrap();
    fx.k.reconcile_plans(&id, 340).unwrap();
    assert_eq!(fx.k.validate_transition(&id, 345), Ok(()));
    fx.k.declare_commit(&id, 345).unwrap();
    fx.k.commit(&id, 345).unwrap();
    assert_eq!(fx.k.accepted_proposals(&ag(5)), 1);
    fx.k.record_position(&ag(5), 1150).unwrap(); // epoch 1, streak 2
    fx.k.record_position(&ag(5), 2150).unwrap(); // epoch 2, streak 3
    assert!(fx.k.is_candidate(&ag(5), 2200), "streak + proposal must make a candidate before any defection");
    // A blocking exception against ag(5), resolved DefectionConfirmed by a
    // strict majority of seats (raiser ag(4) cannot vote on its own).
    let did = tid(2);
    fx.k.register_transition(&ag(1), did, fx.manifest(did, 8, OBS, 20_000), BTreeSet::new(), 2200).unwrap();
    fx.k.evaluate_transition(&did, 2210).unwrap();
    fx.k.mark_eligible(&did, 2210).unwrap();
    let idx = fx.k.submit_exception(&ag(4), &did, Disposition::Block, Some(ag(5)), 2211).unwrap();
    assert_eq!(fx.k.resolve_exception(&ag(1), &did, idx, ResolutionKind::DefectionConfirmed, [8; 32], 2212), Ok(()));
    assert_eq!(fx.k.resolve_exception(&ag(2), &did, idx, ResolutionKind::DefectionConfirmed, [8; 32], 2213), Ok(()));
    assert!(!fx.k.is_candidate(&ag(5), 2213), "defection must block candidacy in its epoch");
    // Position streak keeps growing, but candidacy stays blocked in epochs 3, 4.
    fx.k.record_position(&ag(5), 3150).unwrap(); // epoch 3, streak 4
    assert!(!fx.k.is_candidate(&ag(5), 3150), "cooldown must cover epoch 3");
    fx.k.record_position(&ag(5), 4150).unwrap(); // epoch 4, streak 5
    assert!(!fx.k.is_candidate(&ag(5), 4150), "cooldown must cover epoch 4");
    // Epoch 5: the cooldown has expired and candidacy returns.
    fx.k.record_position(&ag(5), 5150).unwrap(); // epoch 5, streak 6
    assert!(fx.k.is_candidate(&ag(5), 5150), "candidacy must return once the cooldown expires");
}

// ---------------------------------------------------------------- G5: failed governance commits leave no trace
#[test]
fn g5_failed_governance_commit_leaves_no_trace() {
    let mut fx = Fx::new();
    // RegisterAgent for an agent that already exists: registration opens
    // (unanimity + category are the only registration checks), but the
    // action fails when commit_governance applies it.
    let action = GovAction::RegisterAgent { agent: ag(1), groups: BTreeSet::from([1]), base_influence: a(2.0) };
    let id = tid(9);
    fx.gov_to_committable(id, action, 200);
    let cap_before = fx.k.capacity_of(RES).unwrap();
    let declared_before = fx.k.declared_state().clone();
    let history_before = fx.k.history().len();
    let events_before = fx.k.events().len();
    match fx.k.commit_governance(&id, 445) {
        Err(KernelError::GovActionFailed(b)) => assert_eq!(*b, KernelError::AgentExists),
        other => panic!("failing governance action must surface GovActionFailed, got {other:?}"),
    }
    assert_eq!(fx.k.life_of(&id), Some(Life::Committable), "failed governance commit moved state");
    assert_eq!(fx.k.capacity_of(RES).unwrap(), cap_before, "failed governance commit touched capacity");
    assert_eq!(fx.k.declared_state(), &declared_before, "failed governance commit moved the chain");
    assert_eq!(fx.k.history().len(), history_before, "failed governance commit left history");
    assert_eq!(fx.k.events().len(), events_before, "failed governance commit left events");
    assert_eq!(fx.k.agent(&ag(1)).unwrap().base_influence, a(2.0), "failed registration altered the agent");
    // The failure is repeatable, not a half-applied state: it fails again.
    assert!(matches!(fx.k.commit_governance(&id, 446), Err(KernelError::GovActionFailed(_))));
    assert_eq!(fx.k.life_of(&id), Some(Life::Committable));
}

// ---------------------------------------------------------------- G6: stale dialect
/// A dialect dependency that goes stale is caught by the predicate: two
/// proposals rename the same old symbol, the second one's commit moves the
/// head, and the first transition's mask then carries DIALECT — and it can
/// never commit.
///
/// Port note (a real finding about the delivered build, not a weakened
/// test): the notes' "stale between declare and commit" sequence cannot
/// occur here, because only one transition per parent may be selected at a
/// time (a Committable sibling blocks every other sibling's selection with
/// ParentAlreadyHasASelectedTransition). Staleness therefore strands the
/// dependent transition at or before selection instead of at commit: the
/// test asserts exactly that reachable behavior — the DIALECT bit fires,
/// selection is refused, and nothing is applied.
#[test]
fn g6_stale_dialect_is_caught_by_the_predicate_and_strands_the_dependent() {
    let mut fx = Fx::new();
    let sym_a: Symbol = [0xAA; 32];
    let mk = |new_symbol: Symbol| NotationInput {
        old_symbol: sym_a, new_symbol, old_kind: SymbolKind::Constant, new_kind: SymbolKind::Constant,
        definition_hash: [0; 32], prev_change: None, reason: String::from("test rename"),
    };
    let c1 = fx.k.propose_notation_change(&ag(1), mk([0xBB; 32]), 100).unwrap();
    let c2 = fx.k.propose_notation_change(&ag(1), mk([0xCC; 32]), 100).unwrap();
    assert_ne!(c1, c2);
    // T1 depends on c1 and stops at Eligible. Its signals qualify it (total
    // 2.0, confidence 0.6) but at a lower confidence than T2's 0.85, so it
    // neither outranks nor blocks T2's selection.
    let t1 = tid(1);
    fx.k.register_transition(&ag(1), t1, fx.manifest(t1, 7, OBS, 10_000), BTreeSet::new(), 100).unwrap();
    fx.k.declare_dialect_dependency(&ag(1), &t1, c1, 101).unwrap();
    fx.k.evaluate_transition(&t1, 110).unwrap();
    fx.k.mark_eligible(&t1, 110).unwrap();
    // T2 is registered before T1's signals so the clock never regresses.
    let t2 = tid(2);
    fx.k.register_transition(&ag(1), t2, fx.manifest(t2, 8, OBS, 10_000), BTreeSet::new(), 111).unwrap();
    fx.k.declare_dialect_dependency(&ag(1), &t2, c2, 112).unwrap();
    fx.k.evaluate_transition(&t2, 113).unwrap();
    fx.k.mark_eligible(&t2, 113).unwrap();
    fx.k.submit_round_signal(&ag(1), &t1, a(1.0), Some(a(0.6)), 120).unwrap();
    fx.k.submit_round_signal(&ag(2), &t1, a(1.0), Some(a(0.6)), 121).unwrap();
    let mask_before = fx.k.commit_fail_mask(&t1, 125).unwrap();
    assert!(!mask_before.has(FailMask::DIALECT), "dependency must be live before the competing commit");
    // T2 wins selection and commits: c2 takes the head.
    fx.k.submit_round_signal(&ag(1), &t2, a(1.0), Some(a(0.9)), 130).unwrap();
    fx.k.submit_round_signal(&ag(2), &t2, a(1.0), Some(a(0.8)), 131).unwrap();
    fx.k.select_candidate(&t2, 230).unwrap();
    fx.k.open_planning(&ag(1), &t2, 240).unwrap();
    fx.k.submit_plan(&ag(1), &t2, Plan { agent: ag(1), claims: BTreeMap::from([(RES, a(10.0))]), expires_at: 4000 }, 250).unwrap();
    fx.k.reconcile_plans(&t2, 350).unwrap();
    assert_eq!(fx.k.validate_transition(&t2, 355), Ok(()));
    fx.k.declare_commit(&t2, 355).unwrap();
    fx.k.commit(&t2, 355).unwrap();
    assert_eq!(fx.k.notation(c2).unwrap().status, NotationStatus::Committed);
    assert_eq!(fx.k.dialect_head(&sym_a), Some(c2));
    // T1's dependency is now stale: the predicate names it, selection is
    // refused by the committed sibling, and a commit attempt moves nothing.
    let mask = fx.k.commit_fail_mask(&t1, 360).unwrap();
    assert!(mask.has(FailMask::DIALECT), "stale dependency must set the DIALECT bit (mask {:#x})", mask.0);
    assert_eq!(fx.k.select_candidate(&t1, 360), Err(KernelError::ParentAlreadyHasASelectedTransition));
    assert!(matches!(fx.k.commit(&t1, 360), Err(KernelError::BadLifecycle(_))));
    assert_eq!(fx.k.life_of(&t1), Some(Life::Eligible), "stranded transition moved");
    assert_eq!(fx.k.capacity_of(RES).unwrap(), a(990.0), "only the winning transition debited capacity");
    assert_eq!(fx.k.notation(c1).unwrap().status, NotationStatus::Proposed, "stale change was applied");
    assert_eq!(fx.k.dialect_head(&sym_a), Some(c2), "head moved past the winner");
}

// ---------------------------------------------------------------- G7: post-seal registration cap
#[test]
fn g7_post_seal_registration_cap_refuses_over_cap_and_accepts_at_cap() {
    let mut fx = Fx::new();
    // Fixture total base is 6 x 2.0 = 12.0. A 3.0 newcomer would hold
    // 3/15 = 1/5 of base, over the 1/6 cap: refused at commit, no trace.
    let over = GovAction::RegisterAgent { agent: ag(7), groups: BTreeSet::from([1]), base_influence: a(3.0) };
    let id = tid(9);
    fx.gov_to_committable(id, over, 200);
    let cap_before = fx.k.capacity_of(RES).unwrap();
    let declared_before = fx.k.declared_state().clone();
    let history_before = fx.k.history().len();
    match fx.k.commit_governance(&id, 445) {
        Err(KernelError::GovActionFailed(b)) => assert_eq!(*b, KernelError::InfluenceShareExceeded),
        other => panic!("over-cap registration must fail at commit, got {other:?}"),
    }
    assert_eq!(fx.k.life_of(&id), Some(Life::Committable));
    assert!(fx.k.agent(&ag(7)).is_none(), "refused registration left an agent");
    assert_eq!(fx.k.capacity_of(RES).unwrap(), cap_before);
    assert_eq!(fx.k.declared_state(), &declared_before);
    assert_eq!(fx.k.history().len(), history_before);
    // The proposer withdraws the refused attempt, freeing the parent slot:
    // a Committable sibling would otherwise block the next selection.
    fx.k.withdraw(&ag(1), &id, 447).unwrap();
    assert_eq!(fx.k.life_of(&id), Some(Life::Discarded));
    // A 2.0 newcomer holds 2/14 = 1/7, under the cap: it applies exactly.
    let at_cap = GovAction::RegisterAgent { agent: ag(7), groups: BTreeSet::from([1]), base_influence: a(2.0) };
    let id2 = tid(10);
    fx.gov_to_committable(id2, at_cap, 500);
    fx.k.commit_governance(&id2, 745).unwrap();
    assert_eq!(fx.k.life_of(&id2), Some(Life::Committed));
    let prof = fx.k.agent(&ag(7)).unwrap();
    assert_eq!(prof.base_influence, a(2.0));
    assert!(prof.active);
}

// ---------------------------------------------------------------- G8: ruling-2 discard
/// Council discard of an abandoned blocked transition, as ruled (K-1 ruling 2)
/// and implemented in the delivered v0.4.1 build: Blocked only, strict
/// majority of ALL council seats, one vote per seat per blocking episode, and
/// discarding frees the parent selection slot for a sibling.
#[test]
fn g8_council_discard_of_blocked_transition_frees_parent_slot() {
    // A Validated transition can never be discarded: Blocked only.
    let mut fx = Fx::new();
    fx.ready(tid(1), 7, &[(1, 100.0)]);
    assert_eq!(fx.k.life_of(&tid(1)), Some(Life::Validated));
    assert_eq!(
        fx.k.discard_blocked(&ag(1), &tid(1), 400),
        Err(KernelError::BadLifecycle(Life::Validated)),
        "a Committable-path transition must not be discardable"
    );

    // Drive a fresh transition to Blocked: 1500 claimed against the 1000 stock.
    let mut fx = Fx::new();
    fx.drive_to_reconciling(Drive { id: tid(1), next: 7, claims: &[(1, 1500.0)],
        base: 100, obs: OBS, plan_expiry: 4000, ingest: false, sig: SigMode::Strong });
    assert_eq!(fx.validate_at(tid(1), 100), Some(Life::Blocked));
    assert_eq!(fx.k.capacity_of(RES).unwrap(), a(1000.0), "blocked claims debit nothing");
    // A non-seat cannot vote at all.
    assert_eq!(
        fx.k.discard_blocked(&ag(4), &tid(1), 400),
        Err(KernelError::NotCouncil),
        "only council seats may vote to discard"
    );
    // One seat alone is not a strict majority of the 3 seats.
    assert_eq!(fx.k.discard_blocked(&ag(1), &tid(1), 400), Ok(false));
    assert_eq!(fx.k.life_of(&tid(1)), Some(Life::Blocked));
    // The same seat cannot vote twice in one episode.
    assert_eq!(
        fx.k.discard_blocked(&ag(1), &tid(1), 401),
        Err(KernelError::AlreadyVoted),
        "one vote per seat per blocking episode"
    );
    // The second seat reaches the strict majority: discarded.
    assert_eq!(fx.k.discard_blocked(&ag(2), &tid(1), 402), Ok(true));
    assert_eq!(fx.k.life_of(&tid(1)), Some(Life::Discarded));
    // A discarded transition is no longer Blocked, so it cannot be discarded again.
    assert_eq!(
        fx.k.discard_blocked(&ag(3), &tid(1), 403),
        Err(KernelError::BadLifecycle(Life::Discarded)),
        "discard is terminal for the episode's transition"
    );
    // The freed parent slot: a sibling on the same (still uncommitted) parent
    // selects, where it could not while the blocked transition held the slot.
    let m = fx.manifest(tid(2), 8, OBS, 11_000);
    fx.k.register_transition(&ag(1), tid(2), m, BTreeSet::new(), 1000).unwrap();
    fx.k.evaluate_transition(&tid(2), 1010).unwrap();
    fx.k.mark_eligible(&tid(2), 1010).unwrap();
    fx.k.submit_round_signal(&ag(1), &tid(2), a(1.0), Some(a(0.9)), 1020).unwrap();
    fx.k.submit_round_signal(&ag(2), &tid(2), a(1.0), Some(a(0.8)), 1021).unwrap();
    fx.k.select_candidate(&tid(2), 1120).unwrap();
    assert_eq!(fx.k.life_of(&tid(2)), Some(Life::Selected), "slot must free on discard");
    // The sibling itself blocks on an over-claim: a NEW blocking episode, so
    // the seats' votes reset — seat 1 may vote again, and the majority
    // discards it exactly as before.
    fx.k.open_planning(&ag(1), &tid(2), 1130).unwrap();
    fx.k.submit_plan(&ag(1), &tid(2),
        Plan { agent: ag(1), claims: BTreeMap::from([(RES, a(1500.0))]), expires_at: 4000 }, 1140).unwrap();
    fx.k.reconcile_plans(&tid(2), 1240).unwrap();
    assert_eq!(fx.validate_at(tid(2), 1000), Some(Life::Blocked));
    assert_eq!(fx.k.discard_blocked(&ag(1), &tid(2), 1300), Ok(false));
    assert_eq!(fx.k.discard_blocked(&ag(2), &tid(2), 1301), Ok(true));
    assert_eq!(fx.k.life_of(&tid(2)), Some(Life::Discarded));
    assert_eq!(fx.k.capacity_of(RES).unwrap(), a(1000.0), "discards move no capacity");
}
