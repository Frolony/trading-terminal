use coordination_kernel::*;
use std::collections::{BTreeMap, BTreeSet};

const ADMIN: AgentId = [0xA0; 32];
const ORACLE: AgentId = [0xA1; 32];
const EVAL: AgentId = [0xA2; 32];
const TID: TransitionId = [1; 32];
const TT: TransitionType = [9; 32];
const C1: ConstraintId = [5; 32];
const OBS: Hash32 = [3; 32];
fn a(x: f64) -> Amount { Amount::from_f64(x).unwrap() }
fn ag(n: u8) -> AgentId { [n; 32] }

struct W { k: Kernel }
fn pol() -> Policy {
    Policy { quorum_required: a(1.0), confidence_threshold: a(0.5), min_obs_confidence: a(0.5),
             require_objectives: false, planning_window: 100, max_rounds: 3, max_plans: 4 }
}
fn manifest() -> Manifest {
    Manifest { id: TID, transition_type: TT, parent_index: 0, parent_hash: [0; 32], declared_next_hash: [7; 32],
               observation_id: OBS, expires_at: 10_000, relevant_groups: BTreeSet::from([1]) }
}
impl W {
    fn new() -> W {
        let mut k = Kernel::new(ADMIN, SystemState { index: 0, hash: [0; 32], timestamp: 100 },
            Config { rep_min: a(0.0), rep_max: a(100.0), max_attribution: a(20.0) }).unwrap();
        k.grant_role(&ADMIN, ORACLE, Role::Oracle).unwrap();
        k.grant_role(&ADMIN, EVAL, Role::Evaluator).unwrap();
        k.set_mandatory_constraints(&ADMIN, TT, BTreeSet::from([C1])).unwrap();
        k.set_capacity(&ADMIN, 0, a(1000.0)).unwrap();
        for n in 1..=3u8 {
            k.register_agent(&ADMIN, AgentProfile { id: ag(n), active: true, groups: BTreeSet::from([1]), influence: a(1.0), reputation: a(50.0) }).unwrap();
        }
        k.post_observation(&ORACLE, Observation { id: OBS, timestamp: 100, expiry: 5000, confidence: a(0.9) }, 100).unwrap();
        W { k }
    }
    fn plan(n: u8, claim: f64) -> Plan { Plan { agent: ag(n), claims: BTreeMap::from([(0u16, a(claim))]), expires_at: 4000 } }
    /// Drive a transition to Committable with every term satisfied at t=300.
    fn ready(&mut self) {
        let k = &mut self.k;
        k.register_transition(&ag(1), manifest(), pol(), BTreeSet::new(), 100).unwrap();
        k.report_constraint(&EVAL, &TID, C1, ConstraintStatus::Pass, 110).unwrap();
        k.submit_signal(&ag(1), &TID, a(1.0), a(0.9), 120).unwrap();
        k.submit_signal(&ag(2), &TID, a(1.0), a(0.8), 121).unwrap();
        k.open_planning(&ag(1), &TID, 130).unwrap();
        k.submit_plan(&ag(1), W::plan(1, 100.0), &TID, 140).unwrap();
        k.reconcile(&ag(1), &TID, 300).unwrap();
    }
    fn mask(&self, t: u64) -> u32 { self.k.commit_fail_mask(&TID, t).unwrap().0 }
}

// ---------- the happy path and one test per predicate bit
#[test] fn all_terms_pass() { let mut w = W::new(); w.ready(); assert_eq!(w.mask(300), 0);
    let s = w.k.commit(&TID, 300).unwrap(); assert_eq!(s.index, 1); assert_eq!(s.hash, [7; 32]); }
#[test] fn bit_constraints() { let mut w = W::new();
    w.k.register_transition(&ag(1), manifest(), pol(), BTreeSet::new(), 100).unwrap();
    assert!(w.mask(100) & FailMask::CONSTRAINTS != 0); // Unavailable is not Pass
    w.k.report_constraint(&EVAL, &TID, C1, ConstraintStatus::Fail, 101).unwrap();
    assert!(w.mask(101) & FailMask::CONSTRAINTS != 0);
    w.k.report_constraint(&EVAL, &TID, C1, ConstraintStatus::Pass, 102).unwrap();
    assert!(w.mask(102) & FailMask::CONSTRAINTS == 0); }
#[test] fn bit_objectives() { let mut w = W::new(); let mut p = pol(); p.require_objectives = true;
    w.k.register_transition(&ag(1), manifest(), p, BTreeSet::new(), 100).unwrap();
    assert!(w.mask(100) & FailMask::OBJECTIVES != 0);
    w.k.report_objectives(&EVAL, &TID, true, 101).unwrap();
    assert!(w.mask(101) & FailMask::OBJECTIVES == 0); }
#[test] fn bit_quorum() { let mut w = W::new(); w.k.register_transition(&ag(1), manifest(), pol(), BTreeSet::new(), 100).unwrap();
    assert!(w.mask(100) & FailMask::QUORUM != 0);
    w.k.submit_signal(&ag(1), &TID, a(1.0), a(0.9), 101).unwrap();
    assert!(w.mask(101) & FailMask::QUORUM == 0); }
#[test] fn bit_confidence() { let mut w = W::new(); w.k.register_transition(&ag(1), manifest(), pol(), BTreeSet::new(), 100).unwrap();
    w.k.submit_signal(&ag(1), &TID, a(1.0), a(0.1), 101).unwrap();
    assert!(w.mask(101) & FailMask::CONFIDENCE != 0); }
#[test] fn bit_compat() { let mut w = W::new(); w.k.register_transition(&ag(1), manifest(), pol(), BTreeSet::new(), 100).unwrap();
    w.k.open_planning(&ag(1), &TID, 110).unwrap();
    w.k.submit_plan(&ag(1), W::plan(1, 800.0), &TID, 111).unwrap();
    w.k.submit_plan(&ag(2), W::plan(2, 800.0), &TID, 112).unwrap();
    w.k.reconcile(&ag(1), &TID, 300).unwrap();
    assert!(w.mask(300) & FailMask::COMPAT != 0); }
#[test] fn bit_exceptions() { let mut w = W::new(); w.ready();
    w.k.raise_exception(&EVAL, &TID, [8; 32], Disposition::Block, 301).unwrap();
    assert!(w.mask(301) & FailMask::EXCEPTIONS != 0);
    w.k.resolve_exception(&EVAL, &TID, [8; 32], 302).unwrap();
    assert_eq!(w.mask(302), 0); }
#[test] fn bit_fresh() { let mut w = W::new(); w.ready(); assert!(w.mask(5000) & FailMask::FRESH != 0); }
#[test] fn bit_planning_open() { let mut w = W::new(); let k = &mut w.k;
    k.register_transition(&ag(1), manifest(), pol(), BTreeSet::new(), 100).unwrap();
    k.open_planning(&ag(1), &TID, 130).unwrap();
    assert!(w.mask(150) & FailMask::PLANNING_OPEN != 0); }
#[test] fn bit_expired() { let mut w = W::new(); w.ready(); assert!(w.mask(10_000) & FailMask::EXPIRED != 0); }
#[test] fn bit_lineage() { let mut w = W::new(); let mut m = manifest(); m.parent_hash = [9; 32];
    w.k.register_transition(&ag(1), m, pol(), BTreeSet::new(), 100).unwrap();
    assert!(w.mask(100) & FailMask::LINEAGE != 0); }
#[test] fn bit_plans() { let mut w = W::new(); w.ready(); assert!(w.mask(4000) & FailMask::PLANS != 0); // plan expired
    let mut w = W::new(); w.k.register_transition(&ag(1), manifest(), pol(), BTreeSet::new(), 100).unwrap();
    assert!(w.mask(100) & FailMask::PLANS != 0); } // no plans is NOT ok
#[test] fn bit_capability() { let mut w = W::new(); w.k.require_capability(&ADMIN, TT, [4; 32], a(1.0)).unwrap(); w.ready();
    assert!(w.mask(300) & FailMask::CAPABILITY != 0);
    w.k.grant_capability(&ADMIN, ag(1), [4; 32]).unwrap();
    assert!(w.mask(300) & FailMask::CAPABILITY == 0); }

// ---------- the 17 probes, inverted
#[test] fn p01_weight_exact_at_unit_inputs() { let mut w = W::new();
    w.k.register_transition(&ag(1), manifest(), pol(), BTreeSet::new(), 100).unwrap();
    w.k.submit_signal(&ag(1), &TID, a(1.0), a(1.0), 101).unwrap();
    // weight 1.0 -> quorum 1.0 met exactly
    assert!(w.mask(101) & FailMask::QUORUM == 0); }
#[test] fn p02_empty_constraints_not_feasible() { let mut w = W::new(); w.k.set_mandatory_constraints(&ADMIN, TT, BTreeSet::new()).unwrap();
    w.k.register_transition(&ag(1), manifest(), pol(), BTreeSet::new(), 100).unwrap();
    assert!(w.mask(100) & FailMask::CONSTRAINTS != 0); }
#[test] fn p03_p16_negative_claims_and_quorum_unrepresentable() {
    // Amount has no negative constructor: from_f64(-1) is None, raw is u64.
    assert!(Amount::from_f64(-600.0).is_none()); assert!(Amount::from_f64(f64::NAN).is_none()); }
#[test] fn p04_no_plans_not_compatible() { let mut w = W::new(); w.k.register_transition(&ag(1), manifest(), pol(), BTreeSet::new(), 100).unwrap();
    assert!(w.mask(100) & (FailMask::COMPAT | FailMask::PLANS) == (FailMask::COMPAT | FailMask::PLANS)); }
#[test] fn p05_p06_bare_transition_cannot_commit() { let mut w = W::new();
    w.k.register_transition(&ag(1), manifest(), pol(), BTreeSet::new(), 100).unwrap();
    assert_eq!(w.k.commit(&TID, 200), Err(KernelError::BadState)); // not Committable
    assert_eq!(w.k.declared_state().index, 0); }
#[test] fn p07_drift_never_softens_a_penalty() { let run = |realized: Hash32| { let mut w = W::new(); w.ready(); w.k.commit(&TID, 300).unwrap();
    w.k.settle(&ORACLE, &TID, realized, &[(ag(1), -(10_000_000))], 400).unwrap(); w.k.agent(&ag(1)).unwrap().reputation };
    assert_eq!(run([7; 32]), run([9; 32])); assert_eq!(run([7; 32]), a(40.0)); }
#[test] fn p07b_drift_halves_only_credit() { let mut w = W::new(); w.ready(); w.k.commit(&TID, 300).unwrap();
    w.k.settle(&ORACLE, &TID, [9; 32], &[(ag(1), 10_000_000)], 400).unwrap();
    assert_eq!(w.k.agent(&ag(1)).unwrap().reputation, a(55.0)); }
#[test] fn p08_attribution_order_independent() { let run = |d: Vec<Delta>| { let mut w = W::new(); w.ready(); w.k.commit(&TID, 300).unwrap();
    let v: Vec<_> = d.into_iter().map(|x| (ag(1), x)).collect(); w.k.settle(&ORACLE, &TID, [7; 32], &v, 400).unwrap(); w.k.agent(&ag(1)).unwrap().reputation };
    assert_eq!(run(vec![5_000_000, -5_000_000]), run(vec![-5_000_000, 5_000_000])); }
#[test] fn p08b_attribution_capped() { let mut w = W::new(); w.ready(); w.k.commit(&TID, 300).unwrap();
    w.k.settle(&ORACLE, &TID, [7; 32], &[(ag(1), i64::MAX)], 400).unwrap();
    assert_eq!(w.k.agent(&ag(1)).unwrap().reputation, a(70.0)); } // 50 + cap 20
#[test] fn p09_declared_and_realized_kept_apart() { let mut w = W::new(); w.ready(); w.k.commit(&TID, 300).unwrap();
    w.k.settle(&ORACLE, &TID, [9; 32], &[], 400).unwrap();
    assert_eq!(w.k.declared_state().hash, [7; 32]); assert_eq!(w.k.realized_hash(&TID), Some([9; 32])); }
#[test] fn p10_observation_binding_freshness_confidence() { let mut w = W::new();
    let mut m = manifest(); m.observation_id = [77; 32];
    assert_eq!(w.k.register_transition(&ag(1), m, pol(), BTreeSet::new(), 100), Err(KernelError::UnknownObservation));
    assert_eq!(w.k.post_observation(&ORACLE, Observation { id: [8; 32], timestamp: 9_999, expiry: 99_999, confidence: a(1.0) }, 100), Err(KernelError::ObservationFromFuture));
    let mut w = W::new(); let mut p = pol(); p.min_obs_confidence = a(0.95);
    w.k.register_transition(&ag(1), manifest(), p, BTreeSet::new(), 100).unwrap();
    assert!(w.mask(100) & FailMask::FRESH != 0); }
#[test] fn p11_time_cannot_regress() { let mut w = W::new(); w.ready();
    assert_eq!(w.k.commit(&TID, 150), Err(KernelError::TimeRegression)); }
#[test] fn p12_unregistered_or_ineligible_plan_agent_rejected() { let mut w = W::new();
    w.k.register_transition(&ag(1), manifest(), pol(), BTreeSet::new(), 100).unwrap(); w.k.open_planning(&ag(1), &TID, 110).unwrap();
    assert_eq!(w.k.submit_plan(&ag(99), Plan { agent: ag(99), claims: BTreeMap::new(), expires_at: 4000 }, &TID, 111), Err(KernelError::UnknownAgent));
    assert_eq!(w.k.submit_plan(&ag(2), W::plan(1, 1.0), &TID, 111), Err(KernelError::Unauthorized)); } // cannot plan for someone else
#[test] fn p13_unknown_attribution_agent_errors() { let mut w = W::new(); w.ready(); w.k.commit(&TID, 300).unwrap();
    assert_eq!(w.k.settle(&ORACLE, &TID, [7; 32], &[(ag(42), 1)], 400), Err(KernelError::UnknownAgent));
    assert_eq!(w.k.settle(&ORACLE, &TID, [7; 32], &[(ag(3), 1)], 400), Err(KernelError::NotParticipant)); }
#[test] fn p14_bounds_unforgeable() { assert!(Kernel::new(ADMIN, SystemState { index: 0, hash: [0; 32], timestamp: 0 },
    Config { rep_min: a(10.0), rep_max: a(0.0), max_attribution: a(1.0) }).is_err()); } // and fields are private
#[test] fn p15_capacity_is_consumed() { let mut w = W::new(); w.ready(); w.k.commit(&TID, 300).unwrap();
    assert_eq!(w.k.capacity_of(0), Some(a(900.0))); }
#[test] fn p17_planning_open_blocks_early_commit() { let _w = W::new(); 
    let mut w = W::new(); let k = &mut w.k; k.register_transition(&ag(1), manifest(), pol(), BTreeSet::new(), 100).unwrap(); k.open_planning(&ag(1), &TID, 130).unwrap();
    assert!(w.mask(229) & FailMask::PLANNING_OPEN != 0); assert!(w.mask(230) & FailMask::PLANNING_OPEN == 0); }

// ---------- lifecycle / authority / signals
#[test] fn roles_enforced() { let mut w = W::new();
    assert_eq!(w.k.set_capacity(&ag(1), 0, a(1.0)), Err(KernelError::Unauthorized));
    assert_eq!(w.k.post_observation(&ag(1), Observation { id: [1; 32], timestamp: 1, expiry: 2, confidence: a(1.0) }, 100), Err(KernelError::Unauthorized)); }
#[test] fn one_signal_per_agent_per_round() { let mut w = W::new(); w.k.register_transition(&ag(1), manifest(), pol(), BTreeSet::new(), 100).unwrap();
    w.k.submit_signal(&ag(1), &TID, a(1.0), a(0.9), 101).unwrap();
    assert_eq!(w.k.submit_signal(&ag(1), &TID, a(1.0), a(0.9), 102), Err(KernelError::AlreadySignalled)); }
#[test] fn monotone_revision_and_rounds() { let mut w = W::new(); w.k.register_transition(&ag(1), manifest(), pol(), BTreeSet::new(), 100).unwrap();
    w.k.submit_signal(&ag(1), &TID, a(1.0), a(1.0), 101).unwrap(); w.k.submit_signal(&ag(2), &TID, a(1.0), a(0.0), 102).unwrap();
    w.k.advance_round(&ag(1), &TID, 103).unwrap(); // mean 0.5
    assert_eq!(w.k.submit_signal(&ag(1), &TID, a(1.0), a(1.0), 104), Ok(())); // equal distance ok
    w.k.advance_round(&ag(1), &TID, 105).unwrap();
    assert_eq!(w.k.submit_signal(&ag(2), &TID, a(1.0), a(0.0), 106), Ok(()));
    assert_eq!(w.k.advance_round(&ag(1), &TID, 107), Err(KernelError::RoundsExhausted)); }
#[test] fn lifecycle_never_regresses() { let mut w = W::new(); w.ready(); w.k.commit(&TID, 300).unwrap();
    assert_eq!(w.k.commit(&TID, 301), Err(KernelError::BadState));
    assert_eq!(w.k.expire(&TID, 20_000), Err(KernelError::BadState));
    assert_eq!(w.k.state_of(&TID), Some(TState::Committed)); }
#[test] fn stale_parent_cannot_commit_after_chain_moves() { let mut w = W::new(); w.ready(); w.k.commit(&TID, 300).unwrap();
    let mut m = manifest(); m.id = [2; 32]; w.k.register_transition(&ag(1), m, pol(), BTreeSet::new(), 301).unwrap();
    assert!(w.k.commit_fail_mask(&[2; 32], 301).unwrap().has(FailMask::LINEAGE)); }
#[test] fn overflow_is_an_error_not_a_silent_saturate() { assert!(Amount::units(u64::MAX).is_err());
    assert!(Amount::from_raw(u64::MAX).checked_add(Amount::from_raw(1)).is_err());
    assert!(Amount::from_raw(u64::MAX).checked_mul(Amount::from_raw(u64::MAX)).is_err()); }

// ---------- non-circular parity: parse the shipped Solidity source, compare literals
#[test] fn bit_parity_with_solidity_source() {
    let sol = include_str!("../reference/CoordinationKernel.sol");
    let mut found = BTreeMap::new();
    for line in sol.lines() {
        if let Some(rest) = line.trim().strip_prefix("uint256 public constant F_") {
            let (name, val) = rest.split_once('=').unwrap();
            found.insert(name.trim().to_string(), val.trim().trim_end_matches(';').parse::<u32>().unwrap());
        }
    }
    let rust = [("CONSTRAINTS", FailMask::CONSTRAINTS), ("OBJECTIVES", FailMask::OBJECTIVES), ("QUORUM", FailMask::QUORUM),
        ("CONFIDENCE", FailMask::CONFIDENCE), ("COMPAT", FailMask::COMPAT), ("EXCEPTIONS", FailMask::EXCEPTIONS),
        ("FRESH", FailMask::FRESH), ("PLANNING_OPEN", FailMask::PLANNING_OPEN), ("EXPIRED", FailMask::EXPIRED),
        ("LINEAGE", FailMask::LINEAGE), ("PLANS", FailMask::PLANS), ("CAPABILITY", FailMask::CAPABILITY), ("DIALECT", FailMask::DIALECT)];
    assert_eq!(found.len(), rust.len(), "Solidity and Rust must define the same set of bits");
    for (n, v) in rust { assert_eq!(found.get(n), Some(&v), "bit {n}"); }
}
#[test] fn reject_is_reachable_and_terminal() { let mut w = W::new();
    w.k.register_transition(&ag(1), manifest(), pol(), BTreeSet::new(), 100).unwrap();
    assert_eq!(w.k.reject(&ag(2), &TID, 101), Err(KernelError::Unauthorized));
    w.k.reject(&EVAL, &TID, 101).unwrap();
    assert_eq!(w.k.state_of(&TID), Some(TState::Rejected));
    assert_eq!(w.k.open_planning(&ag(1), &TID, 102), Err(KernelError::BadState)); }
#[test] fn only_proposer_or_admin_starts_planning() { let mut w = W::new();
    w.k.register_transition(&ag(1), manifest(), pol(), BTreeSet::new(), 100).unwrap();
    assert_eq!(w.k.open_planning(&ag(2), &TID, 101), Err(KernelError::Unauthorized));
    w.k.open_planning(&ADMIN, &TID, 102).unwrap(); }

// ---------- administrator hooks
struct RepWeighted; // I := reputation normalized so rep_max -> 1.0
impl WeightPolicy for RepWeighted {
    fn influence(&self, a: &AgentProfile) -> Amount { a.reputation.checked_mul_div(Amount::ONE, a_max()).unwrap() }
}
fn a_max() -> Amount { a(100.0) }
struct DenyAll;
impl Attestor for DenyAll {
    fn may_observe(&self, _: &CallerRoles, _: &AgentId, _: &Observation) -> bool { false }
    fn may_settle(&self, _: &CallerRoles, _: &AgentId, _: &TransitionId, _: &Hash32, _: &[(AgentId, Delta)]) -> bool { false }
}
struct ObserveOnly; // observation allowed for Oracle; settlement never allowed
impl Attestor for ObserveOnly {
    fn may_observe(&self, c: &CallerRoles, _: &AgentId, _: &Observation) -> bool { c.has(Role::Oracle) }
    fn may_settle(&self, _: &CallerRoles, _: &AgentId, _: &TransitionId, _: &Hash32, _: &[(AgentId, Delta)]) -> bool { false }
}
fn custom<A: Attestor, Wp: WeightPolicy>(at: A, wp: Wp) -> Kernel<A, Wp> {
    let mut k = Kernel::with_policies(ADMIN, SystemState { index: 0, hash: [0; 32], timestamp: 100 },
        Config { rep_min: a(0.0), rep_max: a(100.0), max_attribution: a(20.0) }, at, wp).unwrap();
    k.grant_role(&ADMIN, ORACLE, Role::Oracle).unwrap();
    k.register_agent(&ADMIN, AgentProfile { id: ag(1), active: true, groups: BTreeSet::from([1]), influence: a(1.0), reputation: a(25.0) }).unwrap();
    k.post_observation(&ORACLE, Observation { id: OBS, timestamp: 100, expiry: 5000, confidence: a(0.9) }, 100).ok();
    k
}
#[test] fn weight_policy_changes_quorum_outcome() {
    // default: influence 1.0 -> weight 1.0 meets quorum 1.0. reputation policy: 25/100 -> 0.25 does not.
    let mut d = custom(RoleAttestor, FlatWeights);
    d.register_transition(&ag(1), manifest(), pol(), BTreeSet::new(), 100).unwrap();
    d.submit_signal(&ag(1), &TID, a(1.0), a(1.0), 101).unwrap();
    assert!(!d.commit_fail_mask(&TID, 101).unwrap().has(FailMask::QUORUM));
    let mut r = custom(RoleAttestor, RepWeighted);
    r.register_transition(&ag(1), manifest(), pol(), BTreeSet::new(), 100).unwrap();
    r.submit_signal(&ag(1), &TID, a(1.0), a(1.0), 101).unwrap();
    assert!(r.commit_fail_mask(&TID, 101).unwrap().has(FailMask::QUORUM)); }
#[test] fn attestor_hook_controls_observe_and_settle() {
    let mut k = Kernel::with_policies(ADMIN, SystemState { index: 0, hash: [0; 32], timestamp: 100 },
        Config { rep_min: a(0.0), rep_max: a(100.0), max_attribution: a(20.0) }, DenyAll, FlatWeights).unwrap();
    k.grant_role(&ADMIN, ORACLE, Role::Oracle).unwrap(); // role alone is no longer enough
    assert_eq!(k.post_observation(&ORACLE, Observation { id: OBS, timestamp: 100, expiry: 5000, confidence: a(0.9) }, 100), Err(KernelError::Unauthorized));
}
#[test] fn attestor_can_deny_settlement_even_for_oracle_role() {
    let mut k = Kernel::with_policies(ADMIN, SystemState { index: 0, hash: [0; 32], timestamp: 100 },
        Config { rep_min: a(0.0), rep_max: a(100.0), max_attribution: a(20.0) }, ObserveOnly, FlatWeights).unwrap();
    for (r, who) in [(Role::Oracle, ORACLE), (Role::Evaluator, EVAL)] { k.grant_role(&ADMIN, who, r).unwrap(); }
    k.set_mandatory_constraints(&ADMIN, TT, BTreeSet::from([C1])).unwrap();
    k.set_capacity(&ADMIN, 0, a(1000.0)).unwrap();
    k.register_agent(&ADMIN, AgentProfile { id: ag(1), active: true, groups: BTreeSet::from([1]), influence: a(1.0), reputation: a(50.0) }).unwrap();
    k.post_observation(&ORACLE, Observation { id: OBS, timestamp: 100, expiry: 5000, confidence: a(0.9) }, 100).unwrap();
    k.register_transition(&ag(1), manifest(), pol(), BTreeSet::new(), 100).unwrap();
    k.report_constraint(&EVAL, &TID, C1, ConstraintStatus::Pass, 101).unwrap();
    k.submit_signal(&ag(1), &TID, a(1.0), a(1.0), 102).unwrap();
    k.open_planning(&ag(1), &TID, 103).unwrap();
    k.submit_plan(&ag(1), W::plan(1, 10.0), &TID, 104).unwrap();
    k.reconcile(&ag(1), &TID, 300).unwrap();
    k.commit(&TID, 300).unwrap();
    assert_eq!(k.settle(&ORACLE, &TID, [7; 32], &[], 400), Err(KernelError::Unauthorized));
    assert_eq!(k.state_of(&TID), Some(TState::Committed)); }
