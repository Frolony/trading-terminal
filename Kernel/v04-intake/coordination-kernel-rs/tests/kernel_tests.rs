use coordination_kernel::*;
use std::collections::{BTreeMap, BTreeSet};

fn id(n: u8) -> [u8; 32] { let mut a = [0u8; 32]; a[0] = n; a }
fn nm(s: &str) -> [u8; 32] { tag(s.as_bytes()) }
fn amt(x: f64) -> Amount { Amount::from_f64(x).unwrap() }
fn groups(g: &[u16]) -> BTreeSet<u16> { g.iter().copied().collect() }

const EPOCH: u64 = 10_000;
const T1: TransitionType = tag(b"T1");

/// A configurable world: the kernel asks, the test answers.
#[derive(Clone, Default)]
struct TEnv {
    fail: BTreeSet<ConstraintId>,
    holdings: BTreeMap<AgentId, (u128, Timestamp)>,
    source: bool,
}
impl Env for TEnv {
    fn constraint(&self, c: &ConstraintId, _: &Manifest) -> ConstraintStatus {
        if self.fail.contains(c) { ConstraintStatus::Fail } else { ConstraintStatus::Pass }
    }
    fn objective(&self, _: &ObjectiveId, _: &Manifest) -> Option<bool> { Some(true) }
    fn has_position_source(&self) -> bool { self.source }
    fn locked_holding(&self, a: &AgentId) -> Option<(u128, Timestamp)> { self.holdings.get(a).copied() }
}

fn policy() -> Policy {
    Policy {
        quorum_required: amt(10.0), confidence_threshold: amt(0.5), min_obs_confidence: amt(0.5),
        require_objectives: false, consensus_window: 100, planning_window: 10, execution_window: 1_000,
        max_plans: 8, settlement_threshold: 2, max_rounds: 2,
    }
}

struct H { k: Kernel<TEnv>, t: u64, n: u8, obs: Hash32 }
const ADMIN: u8 = 200;
const OBS: u8 = 201;

impl H {
    /// Sealed world: seven agents (1 base influence each 10), council = agents 1,2,3.
    fn sealed() -> H { Self::sealed_with(|_, _, _| {}) }
    fn sealed_with(extra: impl FnOnce(&mut Kernel<TEnv>, &AgentId, Timestamp)) -> H {
        let admin = id(ADMIN);
        let cfg = Config { rep_min: amt(0.0), rep_max: amt(2000.0), max_attribution: amt(100.0), epoch_length: EPOCH, council_seats: 3, reputation_weighs: true };
        let mut k = Kernel::new(admin, SystemState { index: 0, hash: id(99), timestamp: 1_000 }, cfg, TEnv { source: true, ..Default::default() }).unwrap();
        let t = 1_000;
        let c = nm("C");
        for ty in [T1, T_GOV_ADJUSTABLE, T_GOV_CONSTITUTION, T_GOV_COUNCIL, T_GOV_REBOOT] {
            k.genesis_apply(&admin, GovAction::SetPolicy { ttype: ty, policy: policy() }, t).unwrap();
            k.genesis_apply(&admin, GovAction::SetRequiredConstraints { ttype: ty, set: BTreeSet::from([c]) }, t).unwrap();
        }
        for n in 1..=7u8 {
            k.genesis_apply(&admin, GovAction::RegisterAgent { agent: id(n), groups: groups(&[1]), base_influence: amt(10.0) }, t).unwrap();
        }
        k.genesis_apply(&admin, GovAction::GrantRole { who: id(OBS), role: Role::Observer }, t).unwrap();
        for (n, r) in [(202, Role::Guardian), (203, Role::Executor), (204, Role::Settler), (205, Role::Settler), (206, Role::OutcomeRecorder)] {
            k.genesis_apply(&admin, GovAction::GrantRole { who: id(n), role: r }, t).unwrap();
        }
        extra(&mut k, &admin, t);
        k.genesis_apply(&admin, GovAction::InstallCouncil { seats: vec![id(1), id(2), id(3)] }, t).unwrap();
        k.seal_genesis(&admin, t).unwrap();
        let mut h = H { k, t, n: 0, obs: [0; 32] };
        h.observe();
        h
    }
    fn observe(&mut self) {
        self.n += 1;
        let o = Observation { id: id(150 + self.n), timestamp: self.t, expiry: self.t + 1_000_000, confidence: amt(0.9) };
        self.obs = o.id;
        self.k.ingest_observation(&id(OBS), o, self.t).unwrap();
    }
    fn manifest(&self, ty: TransitionType, next: u8) -> Manifest {
        let d = self.k.declared_state();
        Manifest {
            transition_type: ty, parent_index: d.index, parent_hash: d.hash, declared_next_hash: id(next),
            observation_id: self.obs, expires_at: self.t + 1_000_000, relevant_groups: groups(&[1]), action: None,
        }
    }
    /// Evaluate, make eligible, signal, wait, select, plan, reconcile, validate, declare.
    fn drive(&mut self, tid: TransitionId, proposer: AgentId, signers: &[u8]) {
        self.k.evaluate_transition(&tid, self.t).unwrap();
        self.k.mark_eligible(&tid, self.t).unwrap();
        for s in signers { self.k.submit_round_signal(&id(*s), &tid, Amount::ONE, Some(Amount::ONE), self.t).unwrap(); }
        self.t += 100;
        self.k.select_candidate(&tid, self.t).unwrap();
        self.k.open_planning(&proposer, &tid, self.t).unwrap();
        self.k.submit_plan(&proposer, &tid, Plan { agent: proposer, claims: BTreeMap::new(), expires_at: self.t + 100_000 }, self.t).unwrap();
        self.t += 10;
        self.k.reconcile_plans(&tid, self.t).unwrap();
        self.k.validate_transition(&tid, self.t).unwrap();
        self.k.declare_commit(&tid, self.t).unwrap();
    }
    fn ordinary(&mut self, tid: u8, proposer: u8, signers: &[u8]) -> TransitionId {
        let (t, m) = (id(tid), self.manifest(T1, tid));
        self.k.register_transition(&id(proposer), t, m, BTreeSet::new(), self.t).unwrap();
        self.drive(t, id(proposer), signers);
        t
    }
    /// A governance transition: every seat approves, then the proposer registers.
    fn gov(&mut self, tid: u8, action: GovAction, proposer: u8, signers: &[u8]) -> TransitionId {
        let t = id(tid);
        for s in self.k.council().to_vec() { self.k.approve_governance(&s, t, action.clone(), self.t).unwrap(); }
        let m = self.manifest(action.category(), tid);
        self.k.register_governance_transition(&id(proposer), t, m, action, self.t).unwrap();
        self.drive(t, id(proposer), signers);
        t
    }
    fn new_obs(&mut self) { self.t += 1; self.observe(); }
}

fn at_epoch(h: &mut H, e: u64) { h.t = (1_000 + e * EPOCH + 1).max(h.t + 1); h.new_obs(); }
fn a(n: u8) -> AgentId { id(n) }

// ---------------------------------------------------------------- genesis
#[test]
fn the_genesis_authority_is_erased_by_the_seal() {
    let mut h = H::sealed();
    let admin = id(ADMIN);
    assert_eq!(h.k.genesis_apply(&admin, GovAction::GrantRole { who: a(1), role: Role::Guardian }, h.t), Err(KernelError::GenesisIsSealed));
    assert_eq!(h.k.seal_genesis(&admin, h.t), Err(KernelError::NotGenesis));
    assert!(h.k.is_sealed());
}

#[test]
fn seal_is_refused_until_the_replacement_machinery_exists() {
    let admin = id(ADMIN);
    let cfg = Config { rep_min: amt(0.0), rep_max: amt(2000.0), max_attribution: amt(100.0), epoch_length: EPOCH, council_seats: 3, reputation_weighs: true };
    let mut k = Kernel::new(admin, SystemState { index: 0, hash: id(99), timestamp: 1_000 }, cfg, TEnv::default()).unwrap();
    assert_eq!(k.seal_genesis(&admin, 1_000), Err(KernelError::GenesisNotReady("COUNCIL")));
    assert_eq!(k.seal_genesis(&id(1), 1_000), Err(KernelError::NotGenesis));
    // a stranger cannot change a rule
    assert_eq!(k.genesis_apply(&id(1), GovAction::Reboot, 1_000), Err(KernelError::NotGenesis));
}

#[test]
fn seal_is_refused_while_one_agent_holds_more_than_its_share() {
    let admin = id(ADMIN);
    let cfg = Config { rep_min: amt(0.0), rep_max: amt(2000.0), max_attribution: amt(100.0), epoch_length: EPOCH, council_seats: 3, reputation_weighs: true };
    let mut k = Kernel::new(admin, SystemState { index: 0, hash: id(99), timestamp: 1_000 }, cfg, TEnv::default()).unwrap();
    let t = 1_000;
    for ty in [T_GOV_ADJUSTABLE, T_GOV_CONSTITUTION, T_GOV_COUNCIL, T_GOV_REBOOT] {
        k.genesis_apply(&admin, GovAction::SetPolicy { ttype: ty, policy: policy() }, t).unwrap();
    }
    for n in 1..=3u8 { k.genesis_apply(&admin, GovAction::RegisterAgent { agent: id(n), groups: groups(&[1]), base_influence: amt(10.0) }, t).unwrap(); }
    k.genesis_apply(&admin, GovAction::InstallCouncil { seats: vec![id(1), id(2), id(3)] }, t).unwrap();
    assert_eq!(k.seal_genesis(&admin, t), Err(KernelError::GenesisNotReady("INFLUENCE_SHARE")));
}

// ---------------------------------------------------------------- the predicate
#[test]
fn an_ordinary_transition_commits_only_when_the_mask_is_zero() {
    let mut h = H::sealed();
    let t = h.ordinary(10, 4, &[4, 5]);
    assert_eq!(h.k.commit_fail_mask(&t, h.t).unwrap(), FailMask(0));
    let s = h.k.commit(&t, h.t).unwrap();
    assert_eq!(s.index, 1);
    assert_eq!(h.k.accepted_proposals(&a(4)), 1);
    assert_eq!(h.k.commit(&t, h.t), Err(KernelError::BadLifecycle(Life::Committed)));
}

#[test]
fn quorum_below_the_bar_cannot_be_selected() {
    let mut h = H::sealed();
    let (t, m) = (id(10), h.manifest(T1, 10));
    h.k.register_transition(&a(4), t, m, BTreeSet::new(), h.t).unwrap();
    h.k.evaluate_transition(&t, h.t).unwrap();
    h.k.mark_eligible(&t, h.t).unwrap();
    h.k.submit_round_signal(&a(4), &t, Amount::ONE, Some(Amount::ONE), h.t).unwrap();
    h.t += 100;
    assert_eq!(h.k.select_candidate(&t, h.t), Err(KernelError::QuorumNotMet));
}

#[test]
fn a_failed_constraint_rejects_and_unavailable_never_passes() {
    let mut h = H::sealed();
    h.k.env_mut().fail.insert(nm("C"));
    let (t, m) = (id(10), h.manifest(T1, 10));
    h.k.register_transition(&a(4), t, m, BTreeSet::new(), h.t).unwrap();
    h.k.evaluate_transition(&t, h.t).unwrap();
    assert_eq!(h.k.life_of(&t), Some(Life::Rejected));
    // the default environment answers UNAVAILABLE to everything: nothing commits
    let admin = id(ADMIN);
    let cfg = Config { rep_min: amt(0.0), rep_max: amt(2000.0), max_attribution: amt(100.0), epoch_length: EPOCH, council_seats: 3, reputation_weighs: true };
    let mut k: Kernel<NoEnv> = Kernel::new(admin, SystemState { index: 0, hash: id(99), timestamp: 1_000 }, cfg, NoEnv).unwrap();
    k.genesis_apply(&admin, GovAction::SetPolicy { ttype: T1, policy: policy() }, 1_000).unwrap();
    k.genesis_apply(&admin, GovAction::SetRequiredConstraints { ttype: T1, set: BTreeSet::from([nm("C")]) }, 1_000).unwrap();
    k.genesis_apply(&admin, GovAction::GrantRole { who: id(OBS), role: Role::Observer }, 1_000).unwrap();
    k.genesis_apply(&admin, GovAction::RegisterAgent { agent: id(1), groups: groups(&[1]), base_influence: amt(1.0) }, 1_000).unwrap();
    let o = Observation { id: id(150), timestamp: 1_000, expiry: 100_000, confidence: amt(0.9) };
    k.ingest_observation(&id(OBS), o, 1_000).unwrap();
    let m = Manifest { transition_type: T1, parent_index: 0, parent_hash: id(99), declared_next_hash: id(1), observation_id: id(150), expires_at: 50_000, relevant_groups: groups(&[1]), action: None };
    k.register_transition(&id(1), id(7), m, BTreeSet::new(), 1_001).unwrap();
    k.evaluate_transition(&id(7), 1_001).unwrap();
    assert_eq!(k.life_of(&id(7)), Some(Life::Proposed));
    assert_eq!(k.mark_eligible(&id(7), 1_001), Err(KernelError::BadLifecycle(Life::Proposed)));
}

#[test]
fn the_mandatory_constraint_applies_even_when_the_proposer_omits_it() {
    let mut h = H::sealed();
    h.k.env_mut().fail.insert(nm("C")); // C is mandatory for T1; the proposer supplies no extras
    let (t, m) = (id(10), h.manifest(T1, 10));
    h.k.register_transition(&a(4), t, m, BTreeSet::new(), h.t).unwrap();
    h.k.evaluate_transition(&t, h.t).unwrap();
    assert_eq!(h.k.life_of(&t), Some(Life::Rejected));
}

#[test]
fn unknown_types_and_governance_types_do_not_enter_as_ordinary_transitions() {
    let mut h = H::sealed();
    let m = h.manifest(nm("MYSTERY"), 10);
    assert_eq!(h.k.register_transition(&a(4), id(10), m, BTreeSet::new(), h.t), Err(KernelError::NoPolicyForType));
    let m = h.manifest(T_GOV_ADJUSTABLE, 10);
    assert_eq!(h.k.register_transition(&a(4), id(10), m, BTreeSet::new(), h.t), Err(KernelError::GovernanceTypeOnly));
    let m = h.manifest(T1, 10);
    assert_eq!(h.k.register_transition(&a(150), id(10), m, BTreeSet::new(), h.t), Err(KernelError::InactiveAgent));
}

#[test]
fn a_sibling_that_advanced_the_state_makes_the_other_stale() {
    let mut h = H::sealed();
    let (ta, tb) = (id(10), id(11));
    for (t, nx) in [(ta, 10u8), (tb, 11)] {
        let m = h.manifest(T1, nx);
        h.k.register_transition(&a(4), t, m, BTreeSet::new(), h.t).unwrap();
        h.k.evaluate_transition(&t, h.t).unwrap();
        h.k.mark_eligible(&t, h.t).unwrap();
        for s in [4u8, 5] { h.k.submit_round_signal(&a(s), &t, Amount::ONE, Some(Amount::ONE), h.t).unwrap(); }
    }
    assert_eq!(h.k.select_candidate(&ta, h.t), Err(KernelError::ConsensusStillOpen));
    h.t += 100;
    // equal confidence: the earlier-eligible sibling wins, so B is outranked
    assert_eq!(h.k.select_candidate(&tb, h.t), Err(KernelError::OutrankedBySibling));
    h.k.select_candidate(&ta, h.t).unwrap();
    h.k.open_planning(&a(4), &ta, h.t).unwrap();
    h.k.submit_plan(&a(4), &ta, Plan { agent: a(4), claims: BTreeMap::new(), expires_at: h.t + 100_000 }, h.t).unwrap();
    h.t += 10;
    h.k.reconcile_plans(&ta, h.t).unwrap();
    h.k.validate_transition(&ta, h.t).unwrap();
    h.k.declare_commit(&ta, h.t).unwrap();
    h.k.commit(&ta, h.t).unwrap();
    let mask = h.k.commit_fail_mask(&tb, h.t).unwrap();
    assert!(mask.has(FailMask::LINEAGE));
    h.k.mark_conflicted(&tb, h.t).unwrap();
    assert_eq!(h.k.life_of(&tb), Some(Life::Conflicted));
}

#[test]
fn rounds_one_signal_each_revision_toward_the_mean_and_uncertain_adds_no_weight() {
    let mut h = H::sealed();
    let (t, m) = (id(10), h.manifest(T1, 10));
    h.k.register_transition(&a(4), t, m, BTreeSet::new(), h.t).unwrap();
    h.k.evaluate_transition(&t, h.t).unwrap();
    h.k.mark_eligible(&t, h.t).unwrap();
    assert_eq!(h.k.submit_round_signal(&a(4), &t, amt(1.5), Some(Amount::ONE), h.t), Err(KernelError::ValueOutOfRange));
    assert_eq!(h.k.submit_round_signal(&a(4), &t, Amount::ONE, Some(amt(1.5)), h.t), Err(KernelError::ValueOutOfRange));
    assert_eq!(h.k.submit_round_signal(&a(150), &t, Amount::ONE, Some(Amount::ONE), h.t), Err(KernelError::UnknownAgent));
    h.k.submit_round_signal(&a(4), &t, Amount::ONE, Some(amt(0.8)), h.t).unwrap();
    h.k.submit_round_signal(&a(5), &t, Amount::ONE, Some(amt(0.2)), h.t).unwrap();
    assert_eq!(h.k.submit_round_signal(&a(4), &t, Amount::ONE, Some(amt(0.7)), h.t), Err(KernelError::AlreadySignalledThisRound));
    assert_eq!(h.k.total_weight(&t).unwrap(), amt(10.0));
    h.t += 50; // round 1
    assert_eq!(h.k.round_of(&t, h.t).unwrap(), 1);
    assert_eq!(h.k.submit_round_signal(&a(4), &t, Amount::ONE, Some(amt(0.9)), h.t), Err(KernelError::MonotoneRevisionViolation));
    h.k.submit_round_signal(&a(4), &t, Amount::ONE, Some(amt(0.6)), h.t).unwrap();
    // the revision replaced the earlier signal: no double count
    assert_eq!(h.k.total_weight(&t).unwrap(), amt(10.0));
    // "no position" is recorded but adds nothing
    h.k.submit_round_signal(&a(6), &t, Amount::ONE, None, h.t).unwrap();
    assert_eq!(h.k.total_weight(&t).unwrap(), amt(10.0));
    h.t += 50;
    assert_eq!(h.k.submit_round_signal(&a(7), &t, Amount::ONE, Some(Amount::ONE), h.t), Err(KernelError::ConsensusClosed));
}

#[test]
fn the_niche_is_built_from_the_weight_table() {
    let mut h = H::sealed();
    let (t, m) = (id(10), h.manifest(T1, 10));
    h.k.register_transition(&a(5), t, m, BTreeSet::new(), h.t).unwrap();
    h.k.evaluate_transition(&t, h.t).unwrap();
    h.k.mark_eligible(&t, h.t).unwrap();
    assert_eq!(h.k.niche_of(&a(4)), None);
    h.k.submit_round_signal(&a(4), &t, Amount::ONE, Some(Amount::ONE), h.t).unwrap();
    assert_eq!(h.k.niche_of(&a(4)), Some((Sector::Collaborative, amt(1.0))));
    h.k.submit_round_signal(&a(6), &t, Amount::ONE, None, h.t).unwrap();
    // no position + no effective weight reads as INFORMATIONAL, which the table files under UNCERTAIN
    assert_eq!(h.k.niche_of(&a(6)), Some((Sector::Uncertain, amt(0.1))));
    // the proposer's own signal is PROCEDURAL
    h.k.submit_round_signal(&a(5), &t, Amount::ONE, Some(Amount::ONE), h.t).unwrap();
    assert_eq!(h.k.niche_score(&a(5), Sector::Aligned), amt(1.0));
    // the niche never touched the tally: only the two weighted signals count
    assert_eq!(h.k.total_weight(&t).unwrap(), amt(10.0));
}

// ---------------------------------------------------------------- group policy
#[test]
fn group_policy_is_the_highest_matching_multiplier_never_the_sum() {
    let mut h = H::sealed_with(|k, admin, t| {
        k.genesis_apply(admin, GovAction::SetGroupMultiplier { ttype: T1, group: 1, mult: amt(2.0) }, t).unwrap();
        k.genesis_apply(admin, GovAction::SetGroupMultiplier { ttype: T1, group: 2, mult: amt(3.0) }, t).unwrap();
        k.genesis_apply(admin, GovAction::RegisterAgent { agent: id(8), groups: groups(&[1, 2]), base_influence: amt(10.0) }, t).unwrap();
    });
    let (t, mut m) = (id(10), h.manifest(T1, 10));
    m.relevant_groups = groups(&[1, 2]);
    h.k.register_transition(&a(4), t, m, BTreeSet::new(), h.t).unwrap();
    h.k.evaluate_transition(&t, h.t).unwrap();
    h.k.mark_eligible(&t, h.t).unwrap();
    // I = 10 * 0.5 = 5.  agent 6: group 1 only (x2) => 10.  agent 8: groups 1 and 2: max(2,3) = 3 => 15, not 5*5 = 25.
    assert_eq!(h.k.effective_weight(&t, &a(6), Amount::ONE).unwrap(), amt(10.0));
    assert_eq!(h.k.effective_weight(&t, &a(8), Amount::ONE).unwrap(), amt(15.0));
}

// ---------------------------------------------------------------- governance
#[test]
fn a_rule_changes_only_after_unanimous_approval_and_the_full_predicate() {
    let mut h = H::sealed();
    let grant = GovAction::GrantRole { who: a(60), role: Role::Guardian };
    let t = id(30);
    // two of three seats approving is not unanimous
    h.k.approve_governance(&a(1), t, grant.clone(), h.t).unwrap();
    h.k.approve_governance(&a(2), t, grant.clone(), h.t).unwrap();
    let m = h.manifest(T_GOV_ADJUSTABLE, 30);
    assert_eq!(h.k.register_governance_transition(&a(1), t, m.clone(), grant.clone(), h.t), Err(KernelError::CouncilNotUnanimous(a(3))));
    // approval is bound to the exact action
    h.k.approve_governance(&a(3), t, GovAction::GrantRole { who: a(61), role: Role::Guardian }, h.t).unwrap();
    assert_eq!(h.k.register_governance_transition(&a(1), t, m.clone(), grant.clone(), h.t), Err(KernelError::CouncilNotUnanimous(a(3))));
    h.k.approve_governance(&a(3), t, grant.clone(), h.t).unwrap();
    // only a seat may propose; a non-seat cannot even approve
    assert_eq!(h.k.register_governance_transition(&a(4), t, m.clone(), grant.clone(), h.t), Err(KernelError::NotCouncil));
    assert_eq!(h.k.approve_governance(&a(4), t, grant.clone(), h.t), Err(KernelError::NotCouncil));
    // the type must match the action's category
    let wrong = h.manifest(T_GOV_CONSTITUTION, 30);
    assert_eq!(h.k.register_governance_transition(&a(1), t, wrong, grant.clone(), h.t), Err(KernelError::WrongGovCategory));
    h.k.register_governance_transition(&a(1), t, m, grant, h.t).unwrap();
    h.drive(t, a(1), &[4, 5]);
    assert!(!h.k.has_role(&a(60), Role::Guardian));
    assert_eq!(h.k.commit(&t, h.t), Err(KernelError::UseGovernanceCommit));
    h.k.commit_governance(&t, h.t).unwrap();
    assert!(h.k.has_role(&a(60), Role::Guardian));
    assert_eq!(h.k.commit_governance(&t, h.t), Err(KernelError::BadLifecycle(Life::Committed)));
}

#[test]
fn weight_levers_and_policies_of_governance_types_are_constitutional() {
    assert_eq!(GovAction::SetGroupMultiplier { ttype: T1, group: 1, mult: amt(2.0) }.category(), T_GOV_CONSTITUTION);
    assert_eq!(GovAction::SetMaxInfluenceShare { share: amt(0.2) }.category(), T_GOV_CONSTITUTION);
    assert_eq!(GovAction::SetPolicy { ttype: T_GOV_COUNCIL, policy: policy() }.category(), T_GOV_CONSTITUTION);
    assert_eq!(GovAction::SetPolicy { ttype: T1, policy: policy() }.category(), T_GOV_ADJUSTABLE);
    assert_eq!(GovAction::SetRequiredConstraints { ttype: T_GOV_REBOOT, set: BTreeSet::new() }.category(), T_GOV_CONSTITUTION);
    assert_eq!(GovAction::InstallCouncil { seats: vec![] }.category(), T_GOV_COUNCIL);
    assert_eq!(GovAction::Reboot.category(), T_GOV_REBOOT);
    assert_eq!(GovAction::GrantRole { who: a(1), role: Role::Guardian }.category(), T_GOV_ADJUSTABLE);
}

#[test]
fn a_blocking_exception_after_declare_stops_a_governance_commit() {
    let mut h = H::sealed();
    let t = h.gov(30, GovAction::GrantRole { who: a(60), role: Role::Guardian }, 1, &[4, 5]);
    h.k.submit_exception(&a(6), &t, Disposition::Block, None, h.t).unwrap();
    assert_eq!(h.k.life_of(&t), Some(Life::Blocked));
    assert!(matches!(h.k.commit_governance(&t, h.t), Err(KernelError::BadLifecycle(Life::Blocked))));
    assert!(!h.k.has_role(&a(60), Role::Guardian));
}

#[test]
fn a_failing_action_undoes_the_whole_commit() {
    let mut h = H::sealed();
    // one agent with 100 against a bound of 1/6: the whale bound
    let whale = GovAction::RegisterAgent { agent: a(70), groups: groups(&[1]), base_influence: amt(100.0) };
    let t = h.gov(30, whale, 1, &[4, 5]);
    let before = h.k.declared_state().clone();
    match h.k.commit_governance(&t, h.t) {
        Err(KernelError::GovActionFailed(e)) => assert_eq!(*e, KernelError::InfluenceShareExceeded),
        other => panic!("{other:?}"),
    }
    assert_eq!(h.k.life_of(&t), Some(Life::Committable));
    assert_eq!(h.k.declared_state(), &before);
    assert_eq!(h.k.accepted_proposals(&a(1)), 0);
    assert!(h.k.agent(&a(70)).is_none());
}

#[test]
fn the_cap_cannot_be_tightened_under_an_existing_holder() {
    let mut h = H::sealed();
    // 7 equal agents hold 1/7 each; a cap of 1/10 would void every one of them
    let t = h.gov(30, GovAction::SetMaxInfluenceShare { share: amt(0.1) }, 1, &[4, 5]);
    assert!(matches!(h.k.commit_governance(&t, h.t), Err(KernelError::GovActionFailed(_))));
    assert_eq!(h.k.max_influence_share(), Amount::from_raw(166_667));
}

// ---------------------------------------------------------------- position and council
#[test]
fn position_is_a_gate_never_a_weight() {
    let mut h = H::sealed();
    // identical standing, wildly different locked amounts
    let (t, m) = (id(10), h.manifest(T1, 10));
    h.k.register_transition(&a(4), t, m, BTreeSet::new(), h.t).unwrap();
    h.k.evaluate_transition(&t, h.t).unwrap();
    h.k.mark_eligible(&t, h.t).unwrap();
    h.k.env_mut().holdings.insert(a(5), (1, u64::MAX));
    let w1 = h.k.effective_weight(&t, &a(5), Amount::ONE).unwrap();
    h.k.env_mut().holdings.insert(a(5), (u128::MAX, u64::MAX));
    let w2 = h.k.effective_weight(&t, &a(5), Amount::ONE).unwrap();
    assert_eq!(w1, w2);
    assert!(w1 > Amount::ZERO);
}

#[test]
fn council_candidacy_streaks_cooldown_and_rotation() {
    let mut h = H::sealed();
    // four accepted proposals
    for (tid, who) in [(10u8, 4u8), (11, 5), (12, 6), (13, 7)] {
        let t = h.ordinary(tid, who, &[4, 5]);
        h.k.commit(&t, h.t).unwrap();
        h.new_obs();
    }
    for n in [4u8, 5, 6, 7] { h.k.env_mut().holdings.insert(a(n), (7, u64::MAX)); }
    assert!(!h.k.is_candidate(&a(4), h.t), "an accepted proposal alone is not candidacy");
    for e in 0..3u64 {
        at_epoch(&mut h, e);
        for n in [4u8, 5, 6, 7] { h.k.record_position(&a(n), h.t).unwrap(); }
    }
    assert_eq!(h.k.record_position(&a(4), h.t), Err(KernelError::AlreadyRecorded));
    assert_eq!(h.k.position_streak(&a(4)), 3);
    for n in [4u8, 5, 6, 7] { assert!(h.k.is_candidate(&a(n), h.t), "agent {n}"); }
    assert!(!h.k.is_candidate(&a(1), h.t), "no position: not a candidate");
    // a holding that is not locked through the epoch does not count
    h.k.env_mut().holdings.insert(a(7), (7, h.t + 10));
    at_epoch(&mut h, 3);
    h.k.record_position(&a(7), h.t).unwrap();
    assert_eq!(h.k.position_streak(&a(7)), 0);
    for n in [4u8, 5, 6] { h.k.record_position(&a(n), h.t).unwrap(); }
    h.k.env_mut().holdings.insert(a(7), (7, u64::MAX));
    assert!(!h.k.is_candidate(&a(7), h.t));

    // epoch 4: a confirmed defection against agent 6 (raised by seat 2, resolved by seats 1 and 3)
    at_epoch(&mut h, 4);
    for n in [4u8, 5, 6] { h.k.record_position(&a(n), h.t).unwrap(); }
    let (t, m) = (id(20), h.manifest(T1, 20));
    h.k.register_transition(&a(4), t, m, BTreeSet::new(), h.t).unwrap();
    h.drive(t, a(4), &[4, 5]);
    let idx = h.k.submit_exception(&a(2), &t, Disposition::Block, Some(a(6)), h.t).unwrap();
    assert_eq!(h.k.life_of(&t), Some(Life::Blocked));
    assert!(h.k.commit_fail_mask(&t, h.t).unwrap().has(FailMask::EXCEPTIONS));
    let ev = id(88);
    assert_eq!(h.k.resolve_exception(&a(4), &t, idx, ResolutionKind::DefectionConfirmed, ev, h.t), Err(KernelError::NotCouncil));
    assert_eq!(h.k.resolve_exception(&a(2), &t, idx, ResolutionKind::DefectionConfirmed, ev, h.t), Err(KernelError::RaiserCannotResolveOwnBlockingException));
    h.k.resolve_exception(&a(1), &t, idx, ResolutionKind::DefectionConfirmed, ev, h.t).unwrap();
    assert_eq!(h.k.resolve_exception(&a(1), &t, idx, ResolutionKind::DefectionConfirmed, ev, h.t), Err(KernelError::AlreadyVoted));
    assert!(h.k.is_candidate(&a(6), h.t), "one seat of three is not a majority");
    h.k.resolve_exception(&a(3), &t, idx, ResolutionKind::DefectionConfirmed, ev, h.t).unwrap();
    assert!(!h.k.is_candidate(&a(6), h.t), "a confirmed defection keeps the offender off the council for the cooldown");
    assert!(h.k.is_candidate(&a(5), h.t));
    h.k.withdraw(&a(4), &t, h.t).unwrap();

    // rotation: a nominee inside the defection cooldown fails, and the whole commit is undone
    let bad = h.gov(31, GovAction::InstallCouncil { seats: vec![a(4), a(5), a(6)] }, 1, &[4, 5]);
    match h.k.commit_governance(&bad, h.t) {
        Err(KernelError::GovActionFailed(e)) => assert_eq!(*e, KernelError::NotCandidate(a(6))),
        other => panic!("{other:?}"),
    }
    h.k.withdraw(&a(1), &bad, h.t).unwrap();
    // a nominee with no position fails too
    let bad2 = h.gov(32, GovAction::InstallCouncil { seats: vec![a(4), a(5), a(1)] }, 1, &[4, 5]);
    assert!(matches!(h.k.commit_governance(&bad2, h.t), Err(KernelError::GovActionFailed(_))));
    h.k.withdraw(&a(1), &bad2, h.t).unwrap();
    assert_eq!(h.k.council(), &[a(1), a(2), a(3)]);
}

#[test]
fn a_council_sits_at_least_three_epochs() {
    let mut h = H::sealed();
    for (tid, who) in [(10u8, 4u8), (11, 5), (12, 6)] {
        let t = h.ordinary(tid, who, &[4, 5]);
        h.k.commit(&t, h.t).unwrap();
        h.new_obs();
    }
    for n in [4u8, 5, 6] { h.k.env_mut().holdings.insert(a(n), (7, u64::MAX)); }
    for e in 0..3u64 {
        at_epoch(&mut h, e);
        for n in [4u8, 5, 6] { h.k.record_position(&a(n), h.t).unwrap(); }
    }
    // still epoch 2: the sitting council has not served three epochs
    let t = h.gov(31, GovAction::InstallCouncil { seats: vec![a(4), a(5), a(6)] }, 1, &[4, 5]);
    match h.k.commit_governance(&t, h.t) {
        Err(KernelError::GovActionFailed(e)) => assert_eq!(*e, KernelError::SittingTooShort),
        other => panic!("{other:?}"),
    }
    h.k.withdraw(&a(1), &t, h.t).unwrap();
    at_epoch(&mut h, 3);
    for n in [4u8, 5, 6] { h.k.record_position(&a(n), h.t).unwrap(); }
    let t = h.gov(33, GovAction::InstallCouncil { seats: vec![a(4), a(5), a(6)] }, 1, &[4, 5]);
    h.k.commit_governance(&t, h.t).unwrap();
    assert_eq!(h.k.council(), &[a(4), a(5), a(6)]);
    assert_eq!(h.k.council_since(), 3);
    // approvals do not carry over: the new council must itself be unanimous, and the retired seat cannot propose
    let grant = GovAction::GrantRole { who: a(60), role: Role::Guardian };
    let m = h.manifest(T_GOV_ADJUSTABLE, 40);
    assert_eq!(h.k.register_governance_transition(&a(4), id(40), m.clone(), grant.clone(), h.t), Err(KernelError::CouncilNotUnanimous(a(4))));
    assert_eq!(h.k.register_governance_transition(&a(1), id(40), m, grant, h.t), Err(KernelError::NotCouncil));
}

// ---------------------------------------------------------------- dialect
#[test]
fn a_notation_takes_effect_only_when_a_dependent_transition_commits() {
    let mut h = H::sealed();
    assert_eq!(h.k.notation_count(), 9, "the changes of this merge are notated at genesis");
    let input = |old: &str, new: &str, prev| NotationInput {
        old_symbol: nm(old), new_symbol: nm(new), old_kind: SymbolKind::Function, new_kind: SymbolKind::Function,
        definition_hash: id(1), prev_change: prev, reason: "test".into(),
    };
    // a stale citation of the prior state is refused
    assert_eq!(h.k.propose_notation_change(&a(4), input("F_DEADLINE", "X", None), h.t), Err(KernelError::StalePrevChange));
    let c1 = h.k.propose_notation_change(&a(4), input("foo", "bar", None), h.t).unwrap();
    let c2 = h.k.propose_notation_change(&a(5), input("foo", "baz", None), h.t).unwrap(); // coexists: declared divergence
    let c3 = h.k.propose_notation_change(&a(5), input("bar", "qux", None), h.t).unwrap();
    assert_eq!(h.k.dialect_head(&nm("foo")), None, "a proposal changes nothing");

    let (t1, t2) = (id(10), id(11));
    for (t, nx) in [(t1, 10u8), (t2, 11)] {
        let m = h.manifest(T1, nx);
        h.k.register_transition(&a(4), t, m, BTreeSet::new(), h.t).unwrap();
    }
    h.k.declare_dialect_dependency(&a(4), &t1, c1, h.t).unwrap();
    h.k.declare_dialect_dependency(&a(4), &t2, c2, h.t).unwrap();
    assert_eq!(h.k.declare_dialect_dependency(&a(5), &t1, c3, h.t), Err(KernelError::Unauthorized));
    // a transition cannot depend on two changes that share a symbol (bar is c1's new and c3's old)
    assert_eq!(h.k.declare_dialect_dependency(&a(4), &t1, c3, h.t), Err(KernelError::DialectDepConflict));
    assert!(!h.k.commit_fail_mask(&t1, h.t).unwrap().has(FailMask::DIALECT));
    h.drive(t1, a(4), &[4, 5]);
    h.k.commit(&t1, h.t).unwrap();
    assert_eq!(h.k.dialect_head(&nm("foo")), Some(c1));
    assert_eq!(h.k.notation(c1).unwrap().status, NotationStatus::Committed);
    // the rival went stale: it now fails the dialect term of the predicate
    assert!(h.k.commit_fail_mask(&t2, h.t).unwrap().has(FailMask::DIALECT));
}

// ---------------------------------------------------------------- safety
#[test]
fn the_emergency_brake_is_bounded_and_self_expiring() {
    let mut h = H::sealed();
    assert_eq!(h.k.pause(&a(4), h.t), Err(KernelError::Unauthorized));
    h.k.pause(&a(202), h.t).unwrap();
    assert!(h.k.is_paused(h.t));
    let m = h.manifest(T1, 10);
    assert_eq!(h.k.register_transition(&a(4), id(10), m, BTreeSet::new(), h.t), Err(KernelError::Paused));
    assert_eq!(h.k.pause(&a(202), h.t), Err(KernelError::PauseCooldown));
    h.t += PAUSE_MAX + 5;
    assert!(!h.k.is_paused(h.t));
    assert_eq!(h.k.pause(&a(202), h.t), Err(KernelError::PauseCooldown), "a cooldown follows, so governance has a window");
}

#[test]
fn a_frozen_substrate_runs_only_a_reboot_transition() {
    let mut h = H::sealed();
    assert_eq!(h.k.record_branching_health(&a(4), [Amount::ONE; 4], h.t), Err(KernelError::Unauthorized));
    h.k.record_branching_health(&id(OBS), [Amount::ONE, Amount::ZERO, Amount::ONE, Amount::ONE], h.t).unwrap();
    assert_eq!(h.k.mode(), SystemMode::CollapsedFrozen);
    let m = h.manifest(T1, 10);
    assert_eq!(h.k.register_transition(&a(4), id(10), m, BTreeSet::new(), h.t), Err(KernelError::SystemFrozen));
    let t = h.gov(30, GovAction::Reboot, 1, &[4, 5]);
    h.k.commit_governance(&t, h.t).unwrap();
    assert_eq!(h.k.mode(), SystemMode::Operational);
}

// ---------------------------------------------------------------- settlement and outcome
#[test]
fn settlement_needs_a_threshold_and_the_outcome_moves_reputation() {
    let mut h = H::sealed();
    let t = h.ordinary(10, 4, &[4, 5]);
    h.k.commit(&t, h.t).unwrap();
    let before = h.k.influence_at(&a(4), h.t).unwrap();
    assert_eq!(h.k.start_execution(&a(4), &t, h.t), Err(KernelError::Unauthorized));
    h.k.start_execution(&a(203), &t, h.t).unwrap();
    assert_eq!(h.k.attest_settlement(&a(204), &t, id(10), h.t), Ok(false), "one attestation is below the threshold of two");
    assert_eq!(h.k.attest_settlement(&a(204), &t, id(10), h.t), Err(KernelError::Duplicate));
    assert_eq!(h.k.attest_settlement(&a(205), &t, id(10), h.t), Ok(true));
    assert_eq!(h.k.realized_hash(&t), Some(id(10)));
    assert_eq!(h.k.record_outcome(&a(206), &t, &[(a(6), 1)], h.t), Err(KernelError::NotParticipant));
    let exact = h.k.record_outcome(&a(206), &t, &[(a(4), 50_000_000)], h.t).unwrap();
    assert!(exact);
    assert_eq!(h.k.agent(&a(4)).unwrap().reputation, amt(1050.0));
    // the influence snapshot taken at a later eligibility sees the new reputation
    assert!(h.k.influence_at(&a(4), h.t + 1).unwrap() > before);
    assert_eq!(h.k.life_of(&t), Some(Life::EvaluatedOutcome));
}

#[test]
fn drift_halves_only_positive_credit() {
    let mut h = H::sealed();
    let t = h.ordinary(10, 4, &[4, 5]);
    h.k.commit(&t, h.t).unwrap();
    h.k.start_execution(&a(203), &t, h.t).unwrap();
    h.k.attest_settlement(&a(204), &t, id(77), h.t).unwrap();
    h.k.attest_settlement(&a(205), &t, id(77), h.t).unwrap();
    let exact = h.k.record_outcome(&a(206), &t, &[(a(4), 50_000_000)], h.t).unwrap();
    assert!(!exact);
    assert_eq!(h.k.agent(&a(4)).unwrap().reputation, amt(1025.0));
}

// ---------------------------------------------------------------- launch profile
#[test]
fn reputation_can_be_recorded_with_zero_influence() {
    fn weight_after_outcome(weighs: bool) -> (Amount, Amount) {
        let admin = id(ADMIN);
        let cfg = Config { rep_min: amt(0.0), rep_max: amt(2000.0), max_attribution: amt(100.0), epoch_length: EPOCH, council_seats: 3, reputation_weighs: weighs };
        let mut k = Kernel::new(admin, SystemState { index: 0, hash: id(99), timestamp: 1_000 }, cfg, TEnv::default()).unwrap();
        let t = 1_000;
        for ty in [T1, T_GOV_ADJUSTABLE, T_GOV_CONSTITUTION, T_GOV_COUNCIL, T_GOV_REBOOT] {
            k.genesis_apply(&admin, GovAction::SetPolicy { ttype: ty, policy: policy() }, t).unwrap();
            k.genesis_apply(&admin, GovAction::SetRequiredConstraints { ttype: ty, set: BTreeSet::from([nm("C")]) }, t).unwrap();
        }
        for n in 1..=7u8 { k.genesis_apply(&admin, GovAction::RegisterAgent { agent: id(n), groups: groups(&[1]), base_influence: amt(10.0) }, t).unwrap(); }
        for (n, r) in [(OBS, Role::Observer), (203, Role::Executor), (204, Role::Settler), (205, Role::Settler), (206, Role::OutcomeRecorder)] {
            k.genesis_apply(&admin, GovAction::GrantRole { who: id(n), role: r }, t).unwrap();
        }
        k.genesis_apply(&admin, GovAction::InstallCouncil { seats: vec![id(1), id(2), id(3)] }, t).unwrap();
        k.seal_genesis(&admin, t).unwrap();
        let mut h = H { k, t, n: 0, obs: [0; 32] };
        h.observe();
        let before = h.k.influence_at(&a(4), h.t).unwrap();
        let tid = h.ordinary(10, 4, &[4, 5]);
        h.k.commit(&tid, h.t).unwrap();
        h.k.start_execution(&a(203), &tid, h.t).unwrap();
        h.k.attest_settlement(&a(204), &tid, id(10), h.t).unwrap();
        h.k.attest_settlement(&a(205), &tid, id(10), h.t).unwrap();
        h.k.record_outcome(&a(206), &tid, &[(a(4), 90_000_000)], h.t).unwrap();
        assert!(h.k.agent(&a(4)).unwrap().reputation > amt(1000.0), "reputation is recorded either way");
        (before, h.k.influence_at(&a(4), h.t + 1).unwrap())
    }
    let (b, after) = weight_after_outcome(false);
    assert_eq!(b, after, "launch profile: reputation moved, influence did not");
    assert_eq!(b, amt(10.0));
    let (b, after) = weight_after_outcome(true);
    assert!(after > b, "reputation-weighted: influence follows reputation");
}

// ---------------------------------------------------------------- discarding an abandoned blocked transition
/// A transition whose plans over-claim capacity ends up Blocked and holds its parent's slot.
fn blocked_world() -> (H, TransitionId, TransitionId) {
    let mut h = H::sealed_with(|k, admin, t| {
        k.genesis_apply(admin, GovAction::SetCapacity { resource: 0, capacity: amt(5.0) }, t).unwrap();
    });
    let (ta, tb) = (id(10), id(11));
    for (t, who, nx) in [(ta, 1u8, 10u8), (tb, 5, 11)] {
        let m = h.manifest(T1, nx);
        h.k.register_transition(&a(who), t, m, BTreeSet::new(), h.t).unwrap();
        h.k.evaluate_transition(&t, h.t).unwrap();
        h.k.mark_eligible(&t, h.t).unwrap();
        for s in [4u8, 5] { h.k.submit_round_signal(&a(s), &t, Amount::ONE, Some(Amount::ONE), h.t).unwrap(); }
    }
    h.t += 100;
    h.k.select_candidate(&ta, h.t).unwrap();
    h.k.open_planning(&a(1), &ta, h.t).unwrap();
    let claims = BTreeMap::from([(0u16, amt(10.0))]);
    h.k.submit_plan(&a(1), &ta, Plan { agent: a(1), claims, expires_at: h.t + 100_000 }, h.t).unwrap();
    h.t += 10;
    h.k.reconcile_plans(&ta, h.t).unwrap();
    h.k.validate_transition(&ta, h.t).unwrap();
    assert_eq!(h.k.life_of(&ta), Some(Life::Blocked), "an over-claim blocks");
    (h, ta, tb)
}

#[test]
fn a_strict_majority_of_seats_is_needed_to_discard() {
    let (mut h, ta, tb) = blocked_world();
    // the proposer (seat 1) is abandoned-by-nobody here, but even its own seat cannot discard alone
    assert_eq!(h.k.discard_blocked(&a(4), &ta, h.t), Err(KernelError::NotCouncil));
    assert_eq!(h.k.discard_blocked(&a(1), &ta, h.t), Ok(false));
    assert_eq!(h.k.life_of(&ta), Some(Life::Blocked));
    assert_eq!(h.k.discard_blocked(&a(1), &ta, h.t), Err(KernelError::AlreadyVoted));
    assert_eq!(h.k.select_candidate(&tb, h.t), Err(KernelError::ParentAlreadyHasASelectedTransition), "the slot is still held");
    assert_eq!(h.k.discard_blocked(&a(2), &ta, h.t), Ok(true));
    assert_eq!(h.k.life_of(&ta), Some(Life::Discarded));
}

#[test]
fn discarding_frees_the_parent_slot_for_a_sibling() {
    let (mut h, ta, tb) = blocked_world();
    assert_eq!(h.k.select_candidate(&tb, h.t), Err(KernelError::ParentAlreadyHasASelectedTransition));
    h.k.discard_blocked(&a(2), &ta, h.t).unwrap();
    h.k.discard_blocked(&a(3), &ta, h.t).unwrap();
    h.k.select_candidate(&tb, h.t).unwrap();
    assert_eq!(h.k.life_of(&tb), Some(Life::Selected));
}

#[test]
fn only_a_blocked_transition_can_be_discarded() {
    let mut h = H::sealed();
    let (t, m) = (id(10), h.manifest(T1, 10));
    h.k.register_transition(&a(4), t, m, BTreeSet::new(), h.t).unwrap();
    assert_eq!(h.k.discard_blocked(&a(1), &t, h.t), Err(KernelError::BadLifecycle(Life::Proposed)));
    h.drive(t, a(4), &[4, 5]);
    assert_eq!(h.k.life_of(&t), Some(Life::Committable));
    assert_eq!(h.k.discard_blocked(&a(1), &t, h.t), Err(KernelError::BadLifecycle(Life::Committable)));
    assert_eq!(h.k.discard_blocked(&a(2), &t, h.t), Err(KernelError::BadLifecycle(Life::Committable)));
    h.k.commit(&t, h.t).unwrap();
    assert_eq!(h.k.discard_blocked(&a(1), &t, h.t), Err(KernelError::BadLifecycle(Life::Committed)));
}

#[test]
fn votes_do_not_carry_across_blocking_episodes() {
    let mut h = H::sealed();
    let t = h.gov(30, GovAction::GrantRole { who: a(60), role: Role::Guardian }, 1, &[4, 5]);
    // first episode: a blocking exception on a Committable transition; one seat votes to discard
    let i = h.k.submit_exception(&a(6), &t, Disposition::Block, None, h.t).unwrap();
    assert_eq!(h.k.life_of(&t), Some(Life::Blocked));
    assert_eq!(h.k.discard_blocked(&a(2), &t, h.t), Ok(false));
    // the exception is resolved and the transition validates again
    for s in [1u8, 2] { h.k.resolve_exception(&a(s), &t, i, ResolutionKind::Remediated, id(5), h.t).unwrap(); }
    h.k.validate_transition(&t, h.t).unwrap();
    // second episode: the old vote is gone, so one more seat is still not a majority
    h.k.submit_exception(&a(6), &t, Disposition::Block, None, h.t).unwrap();
    assert_eq!(h.k.life_of(&t), Some(Life::Blocked));
    assert_eq!(h.k.discard_blocked(&a(3), &t, h.t), Ok(false));
    assert_eq!(h.k.life_of(&t), Some(Life::Blocked));
    assert_eq!(h.k.discard_blocked(&a(2), &t, h.t), Ok(true));
}
