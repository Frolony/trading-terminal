//! Executable specification of the W2.4 launch profile.
//! If any of these tests ever fails, the profile has drifted from its spec —
//! that is a governance event, not a test nuisance.

use coordination_kernel::*;
use kernel_policy::*;
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

struct Fx { k: LaunchKernel, att: EvidenceAttestor }
impl Fx {
    fn new() -> Fx {
        let att = EvidenceAttestor::new();
        let mut k = launch_kernel(ADMIN, SystemState { index: 0, hash: [0; 32], timestamp: 100 }, att.clone()).unwrap();
        k.grant_role(&ADMIN, ORACLE, Role::Oracle).unwrap();
        k.grant_role(&ADMIN, EVAL, Role::Evaluator).unwrap();
        k.set_mandatory_constraints(&ADMIN, TT, BTreeSet::from([C1])).unwrap();
        k.set_capacity(&ADMIN, RES_TREASURY_TRANCHE, a(1000.0)).unwrap();
        k.register_agent(&ADMIN, AgentProfile { id: ag(1), active: true, groups: BTreeSet::from([1]), influence: a(1.0), reputation: a(50.0) }).unwrap();
        Fx { k, att }
    }
    /// Drive TID to Committed (declared next hash [7;32]).
    fn committed(&mut self) {
        let k = &mut self.k;
        k.post_observation(&ORACLE, Observation { id: OBS, timestamp: 100, expiry: 5000, confidence: a(0.9) }, 100).unwrap();
        k.register_transition(&ag(1), Manifest { id: TID, transition_type: TT, parent_index: 0, parent_hash: [0; 32],
            declared_next_hash: [7; 32], observation_id: OBS, expires_at: 10_000, relevant_groups: BTreeSet::from([1]) },
            Policy { quorum_required: a(1.0), confidence_threshold: a(0.5), min_obs_confidence: a(0.5),
                     require_objectives: false, planning_window: 100, max_rounds: 3, max_plans: 4 },
            BTreeSet::new(), 100).unwrap();
        k.report_constraint(&EVAL, &TID, C1, ConstraintStatus::Pass, 110).unwrap();
        k.submit_signal(&ag(1), &TID, a(1.0), a(0.9), 120).unwrap();
        k.open_planning(&ag(1), &TID, 130).unwrap();
        k.submit_plan(&ag(1), Plan { agent: ag(1), claims: BTreeMap::from([(RES_TREASURY_TRANCHE, a(100.0))]), expires_at: 4000 }, &TID, 140).unwrap();
        k.reconcile(&ag(1), &TID, 300).unwrap();
        k.commit(&TID, 300).unwrap();
        assert_eq!(k.state_of(&TID), Some(TState::Committed));
    }
}

#[test]
fn settle_without_evidence_is_refused_and_retryable() {
    let mut fx = Fx::new();
    fx.committed();
    // The Oracle role alone is NOT enough under this profile: no evidence, no settle.
    assert_eq!(fx.k.settle(&ORACLE, &TID, [7; 32], &[], 400), Err(KernelError::Unauthorized));
    // Fail-closed is not fail-dead: the transition waits, unharmed.
    assert_eq!(fx.k.state_of(&TID), Some(TState::Committed));
    assert_eq!(fx.k.realized_hash(&TID), None);
}

#[test]
fn settle_with_mismatched_evidence_is_refused() {
    let mut fx = Fx::new();
    fx.committed();
    // The attestor observed a DIFFERENT outcome than the settlement claims.
    fx.att.record_evidence(TID, [8; 32]);
    assert_eq!(fx.k.settle(&ORACLE, &TID, [7; 32], &[], 400), Err(KernelError::Unauthorized));
    assert_eq!(fx.k.state_of(&TID), Some(TState::Committed));
}

#[test]
fn settle_with_matching_evidence_succeeds_and_still_needs_the_role() {
    let mut fx = Fx::new();
    fx.committed();
    // Evidence recorded after the outcome exists; hash matches the claim.
    fx.att.record_evidence(TID, [7; 32]);
    // Evidence alone is not enough either — the caller must hold the Oracle role.
    assert_eq!(fx.k.settle(&ag(1), &TID, [7; 32], &[], 400), Err(KernelError::Unauthorized));
    let exact = fx.k.settle(&ORACLE, &TID, [7; 32], &[(ag(1), 5_000_000)], 400).unwrap();
    assert!(exact);
    assert_eq!(fx.k.realized_hash(&TID), Some([7; 32]));
    // Attribution still ran under the profile (reputation is recorded at launch).
    assert_eq!(fx.k.agent(&ag(1)).unwrap().reputation, a(55.0));
}

#[test]
fn drift_settlement_against_evidence_reports_inexact() {
    let mut fx = Fx::new();
    fx.committed();
    // Reality produced [9;32], not the declared [7;32]; the evidence says so,
    // and the settlement that honestly reports it is admitted and marked inexact.
    fx.att.record_evidence(TID, [9; 32]);
    let exact = fx.k.settle(&ORACLE, &TID, [9; 32], &[(ag(1), 10_000_000)], 400).unwrap();
    assert!(!exact);
    // Drift halved the credit: 50 + (10 halved) = 55, per kernel attribution rules.
    assert_eq!(fx.k.agent(&ag(1)).unwrap().reputation, a(55.0));
}

#[test]
fn observations_still_require_the_oracle_role() {
    let mut fx = Fx::new();
    let obs = Observation { id: OBS, timestamp: 100, expiry: 5000, confidence: a(0.9) };
    assert_eq!(fx.k.post_observation(&ag(1), obs.clone(), 100), Err(KernelError::Unauthorized));
    fx.k.post_observation(&ORACLE, obs, 100).unwrap();
}

#[test]
fn reputation_has_zero_influence_on_quorum_at_launch() {
    // Two agents, reputations 0 and 100, identical influence: under FlatWeights
    // each one's lone full-support signal meets quorum identically. Reputation
    // is recorded in this deployment; it does not govern — yet, or by formula.
    for rep in [0.0, 100.0] {
        let att = EvidenceAttestor::new();
        let mut k = launch_kernel(ADMIN, SystemState { index: 0, hash: [0; 32], timestamp: 100 }, att).unwrap();
        k.grant_role(&ADMIN, ORACLE, Role::Oracle).unwrap();
        k.grant_role(&ADMIN, EVAL, Role::Evaluator).unwrap();
        k.set_mandatory_constraints(&ADMIN, TT, BTreeSet::from([C1])).unwrap();
        k.register_agent(&ADMIN, AgentProfile { id: ag(1), active: true, groups: BTreeSet::from([1]), influence: a(1.0), reputation: a(rep) }).unwrap();
        k.post_observation(&ORACLE, Observation { id: OBS, timestamp: 100, expiry: 5000, confidence: a(0.9) }, 100).unwrap();
        k.register_transition(&ag(1), Manifest { id: TID, transition_type: TT, parent_index: 0, parent_hash: [0; 32],
            declared_next_hash: [7; 32], observation_id: OBS, expires_at: 10_000, relevant_groups: BTreeSet::from([1]) },
            Policy { quorum_required: a(1.0), confidence_threshold: a(0.5), min_obs_confidence: a(0.5),
                     require_objectives: false, planning_window: 100, max_rounds: 3, max_plans: 4 },
            BTreeSet::new(), 100).unwrap();
        k.submit_signal(&ag(1), &TID, a(1.0), a(1.0), 101).unwrap();
        assert!(!k.commit_fail_mask(&TID, 101).unwrap().has(FailMask::QUORUM),
                "reputation {rep} changed the quorum outcome under the launch profile");
    }
}
