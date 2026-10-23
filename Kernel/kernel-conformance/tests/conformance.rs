//! Conformance tests for the Coordination Kernel — OUR requirements (roadmap W2.3),
//! tested against the crate as-is. The kernel package is not modified by this crate.
//!
//! Properties under test:
//!   P1  commit succeeds  <=>  state is Committable AND commit_fail_mask == 0
//!       (and a refused commit reports exactly the mask the predicate computes,
//!        and leaves state and capacity untouched)
//!   P2  capacity is a conserved stock: debits are exact, over-claims never commit
//!   P3  settle discipline: only after commit, only via the attestor, exactly once
//!   P4  time is monotonic at every mutating entry point
//!   P5  attribution: permutation-invariant, per-agent summed, capped, drift halves
//!       credit only — checked against an independent hand computation
//!   P6  lifecycle: transitions only ever move along legal edges; terminal states
//!       are absorbing (randomized operation fuzz)
//!   P7  lineage: the declared chain advances by exactly the committed manifests,
//!       in order, and stale parents can never commit
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

// ---------------------------------------------------------------- fixtures
const ADMIN: AgentId = [0xA0; 32];
const ORACLE: AgentId = [0xA1; 32];
const EVAL: AgentId = [0xA2; 32];
const TT: TransitionType = [9; 32];
const C1: ConstraintId = [5; 32];
const OBS: Hash32 = [3; 32];
const RES: ResourceId = 0;

fn a(x: f64) -> Amount { Amount::from_f64(x).unwrap() }
fn ag(n: u8) -> AgentId { [n; 32] }
fn tid(n: u8) -> TransitionId { [n; 32] }

fn pol() -> Policy {
    Policy { quorum_required: a(1.0), confidence_threshold: a(0.5), min_obs_confidence: a(0.5),
             require_objectives: false, planning_window: 100, max_rounds: 3, max_plans: 6 }
}

struct Fx { k: Kernel }
impl Fx {
    fn new() -> Fx {
        let mut k = Kernel::new(ADMIN, SystemState { index: 0, hash: [0; 32], timestamp: 100 },
            Config { rep_min: a(0.0), rep_max: a(100.0), max_attribution: a(20.0) }).unwrap();
        k.grant_role(&ADMIN, ORACLE, Role::Oracle).unwrap();
        k.grant_role(&ADMIN, EVAL, Role::Evaluator).unwrap();
        k.set_mandatory_constraints(&ADMIN, TT, BTreeSet::from([C1])).unwrap();
        k.set_capacity(&ADMIN, RES, a(1000.0)).unwrap();
        for n in 1..=4u8 {
            k.register_agent(&ADMIN, AgentProfile { id: ag(n), active: true, groups: BTreeSet::from([1]),
                influence: a(1.0), reputation: a(50.0) }).unwrap();
        }
        k.post_observation(&ORACLE, Observation { id: OBS, timestamp: 100, expiry: 5000, confidence: a(0.9) }, 100).unwrap();
        Fx { k }
    }
    fn manifest(&self, id: TransitionId, next: u8) -> Manifest {
        let d = self.k.declared_state();
        Manifest { id, transition_type: TT, parent_index: d.index, parent_hash: d.hash,
                   declared_next_hash: [next; 32], observation_id: OBS, expires_at: 10_000,
                   relevant_groups: BTreeSet::from([1]) }
    }
    /// Drive `id` to Committable with all terms satisfied; commits at t=300 are legal.
    /// claims: (agent, claim) plan set. Returns nothing; asserts each step works.
    fn ready(&mut self, id: TransitionId, next: u8, claims: &[(u8, f64)]) {
        let m = self.manifest(id, next);
        let k = &mut self.k;
        k.register_transition(&ag(1), m, pol(), BTreeSet::new(), 100).unwrap();
        k.report_constraint(&EVAL, &id, C1, ConstraintStatus::Pass, 110).unwrap();
        k.submit_signal(&ag(1), &id, a(1.0), a(0.9), 120).unwrap();
        k.submit_signal(&ag(2), &id, a(1.0), a(0.8), 121).unwrap();
        k.open_planning(&ag(1), &id, 130).unwrap();
        for (n, c) in claims {
            k.submit_plan(&ag(*n), Plan { agent: ag(*n), claims: BTreeMap::from([(RES, a(*c))]), expires_at: 4000 }, &id, 140).unwrap();
        }
        k.reconcile(&ag(1), &id, 300).unwrap();
        assert_eq!(k.state_of(&id), Some(TState::Committable));
    }
    /// Time-shifted variant for chained scenarios: the kernel clock is monotonic,
    /// so each subsequent transition must live at a later base time, with its own
    /// fresh observation. Commits are legal at base+200.
    fn ready_at(&mut self, id: TransitionId, next: u8, claims: &[(u8, f64)], base: u64, obs_id: Hash32) {
        self.k.post_observation(&ORACLE, Observation { id: obs_id, timestamp: base, expiry: base + 4900, confidence: a(0.9) }, base).unwrap();
        let d = self.k.declared_state();
        let m = Manifest { id, transition_type: TT, parent_index: d.index, parent_hash: d.hash,
                           declared_next_hash: [next; 32], observation_id: obs_id, expires_at: 100_000,
                           relevant_groups: BTreeSet::from([1]) };
        let k = &mut self.k;
        k.register_transition(&ag(1), m, pol(), BTreeSet::new(), base).unwrap();
        k.report_constraint(&EVAL, &id, C1, ConstraintStatus::Pass, base + 10).unwrap();
        k.submit_signal(&ag(1), &id, a(1.0), a(0.9), base + 20).unwrap();
        k.submit_signal(&ag(2), &id, a(1.0), a(0.8), base + 21).unwrap();
        k.open_planning(&ag(1), &id, base + 30).unwrap();
        for (n, c) in claims {
            k.submit_plan(&ag(*n), Plan { agent: ag(*n), claims: BTreeMap::from([(RES, a(*c))]), expires_at: base + 3000 }, &id, base + 40).unwrap();
        }
        k.reconcile(&ag(1), &id, base + 200).unwrap();
        assert_eq!(k.state_of(&id), Some(TState::Committable));
    }
}

// ---------------------------------------------------------------- P1
/// Randomized defect injection: whatever is broken, commit must succeed exactly
/// when the predicate is fully clear, and a refusal must quote the same mask.
#[test]
fn p1_commit_iff_mask_clear_under_random_defects() {
    for iter in 0..300u64 {
        let mut rng = Rng::new(iter + 1);
        let mut fx = Fx::new();
        let id = tid(1);
        let mut p = pol();
        let defect_objectives = rng.coin();
        if defect_objectives { p.require_objectives = true; }
        fx.k.register_transition(&ag(1), fx.manifest(id, 7), p, BTreeSet::new(), 100).unwrap();

        // constraint: pass / fail / never reported
        match rng.below(3) {
            0 => { fx.k.report_constraint(&EVAL, &id, C1, ConstraintStatus::Pass, 110).unwrap(); }
            1 => { fx.k.report_constraint(&EVAL, &id, C1, ConstraintStatus::Fail, 110).unwrap(); }
            _ => {}
        }
        if defect_objectives && rng.coin() {
            fx.k.report_objectives(&EVAL, &id, true, 111).unwrap();
        }
        // signals: none / weak support / strong support
        match rng.below(3) {
            0 => {}
            1 => { fx.k.submit_signal(&ag(1), &id, a(1.0), a(0.1), 120).unwrap(); }
            _ => {
                fx.k.submit_signal(&ag(1), &id, a(1.0), a(0.9), 120).unwrap();
                fx.k.submit_signal(&ag(2), &id, a(1.0), a(0.8), 121).unwrap();
            }
        }
        // planning + plans: within capacity / over capacity / plan expiring before commit
        fx.k.open_planning(&ag(1), &id, 130).unwrap();
        let plan_expiry = if rng.below(4) == 0 { 350 } else { 4000 };
        let c1 = if rng.coin() { 100.0 } else { 700.0 };
        let c2 = if rng.coin() { 100.0 } else { 700.0 };
        fx.k.submit_plan(&ag(1), Plan { agent: ag(1), claims: BTreeMap::from([(RES, a(c1))]), expires_at: plan_expiry }, &id, 140).unwrap();
        fx.k.submit_plan(&ag(2), Plan { agent: ag(2), claims: BTreeMap::from([(RES, a(c2))]), expires_at: plan_expiry }, &id, 141).unwrap();
        // blocking exception, sometimes resolved
        let blocking = rng.below(3) == 0;
        if blocking {
            fx.k.raise_exception(&EVAL, &id, [8; 32], Disposition::Block, 150).unwrap();
            if rng.coin() { fx.k.resolve_exception(&EVAL, &id, [8; 32], 151).unwrap(); }
        }
        fx.k.reconcile(&ag(1), &id, 300).unwrap();
        assert_eq!(fx.k.state_of(&id), Some(TState::Committable), "iter {iter}");

        // commit attempt time: on-time / after plan expiry (350) / after manifest expiry
        let t = match rng.below(3) { 0 => 300, 1 => 400, _ => 10_000 };
        let mask = fx.k.commit_fail_mask(&id, t).unwrap();
        let cap_before = fx.k.capacity_of(RES).unwrap();
        match fx.k.commit(&id, t) {
            Ok(s) => {
                assert!(mask.is_clear(), "iter {iter}: committed with mask {:#x}", mask.0);
                assert_eq!(s.index, 1, "iter {iter}");
                assert_eq!(fx.k.state_of(&id), Some(TState::Committed));
            }
            Err(KernelError::CommitFailed(m)) => {
                assert!(!mask.is_clear(), "iter {iter}: refused though mask clear");
                assert_eq!(m.0, mask.0, "iter {iter}: refusal must quote the predicate's mask");
                assert_eq!(fx.k.state_of(&id), Some(TState::Committable), "iter {iter}: failed commit moved state");
                assert_eq!(fx.k.capacity_of(RES).unwrap(), cap_before, "iter {iter}: failed commit touched capacity");
            }
            Err(e) => panic!("iter {iter}: unexpected commit error {e:?}"),
        }
    }
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
            let base = 100 + (seq as u64 - 1) * 400;
            let claim = 50.0 + rng.below(20) as f64 * 25.0; // 50..=525
            fx.ready_at(id, 10 + seq, &[(1, claim)], base, [60 + seq; 32]);
            let mask = fx.k.commit_fail_mask(&id, base + 200).unwrap();
            let res = fx.k.commit(&id, base + 200);
            if claim <= remaining.to_f64() {
                assert!(mask.is_clear(), "iter {iter} seq {seq}: claim {claim} <= remaining {} but mask {:#x}", remaining.to_f64(), mask.0);
                res.unwrap();
                remaining = remaining.checked_sub(a(claim)).unwrap();
                assert_eq!(fx.k.capacity_of(RES).unwrap(), remaining, "iter {iter} seq {seq}: debit must be exact");
            } else {
                assert!(mask.has(FailMask::COMPAT), "iter {iter} seq {seq}: over-claim without COMPAT bit");
                assert!(matches!(res, Err(KernelError::CommitFailed(_))), "iter {iter} seq {seq}: over-claim committed");
                assert_eq!(fx.k.capacity_of(RES).unwrap(), remaining, "iter {iter} seq {seq}: refused commit debited capacity");
                break;
            }
            if seq > 12 { break; }
        }
        assert!(seq >= 2, "iter {iter}: chain too short to be evidence");
    }
}

// ---------------------------------------------------------------- P3
#[test]
fn p3_settle_only_after_commit_only_by_attestor_exactly_once() {
    let mut fx = Fx::new();
    let id = tid(1);
    // settle before commit — even by the oracle — is a state error
    assert_eq!(fx.k.settle(&ORACLE, &id, [7; 32], &[], 100), Err(KernelError::UnknownTransition));
    fx.ready(id, 7, &[(1, 100.0)]);
    assert_eq!(fx.k.settle(&ORACLE, &id, [7; 32], &[], 300), Err(KernelError::BadState));
    fx.k.commit(&id, 300).unwrap();
    // committed: a non-oracle (default attestor = Oracle role) may not settle
    assert_eq!(fx.k.settle(&ag(1), &id, [7; 32], &[], 400), Err(KernelError::Unauthorized));
    assert_eq!(fx.k.state_of(&id), Some(TState::Committed));
    // drifted realized hash: settles, reports inexact, records the realized hash
    let exact = fx.k.settle(&ORACLE, &id, [9; 32], &[(ag(1), 5_000_000)], 400).unwrap();
    assert!(!exact);
    assert_eq!(fx.k.realized_hash(&id), Some([9; 32]));
    // exactly once: a second settle is refused
    assert_eq!(fx.k.settle(&ORACLE, &id, [9; 32], &[], 401), Err(KernelError::BadState));

    // exact path on a fresh kernel: realized == declared reports exact
    let mut fx = Fx::new();
    fx.ready(id, 7, &[(1, 100.0)]);
    fx.k.commit(&id, 300).unwrap();
    assert!(fx.k.settle(&ORACLE, &id, [7; 32], &[], 400).unwrap());
    assert_eq!(fx.k.realized_hash(&id), Some([7; 32]));
}

// ---------------------------------------------------------------- P4
#[test]
fn p4_time_regression_rejected_at_entry_points() {
    let mut fx = Fx::new();
    let id = tid(1);
    fx.k.register_transition(&ag(1), fx.manifest(id, 7), pol(), BTreeSet::new(), 500).unwrap();
    // clock is now 500; everything earlier is a regression
    assert_eq!(fx.k.post_observation(&ORACLE, Observation { id: [4; 32], timestamp: 100, expiry: 900, confidence: a(0.9) }, 499),
               Err(KernelError::TimeRegression));
    assert_eq!(fx.k.report_constraint(&EVAL, &id, C1, ConstraintStatus::Pass, 499), Err(KernelError::TimeRegression));
    assert_eq!(fx.k.submit_signal(&ag(1), &id, a(1.0), a(1.0), 499), Err(KernelError::TimeRegression));
    assert_eq!(fx.k.open_planning(&ag(1), &id, 499), Err(KernelError::TimeRegression));
    assert_eq!(fx.k.commit(&id, 499), Err(KernelError::TimeRegression));
    assert_eq!(fx.k.settle(&ORACLE, &id, [7; 32], &[], 499), Err(KernelError::TimeRegression));
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
        // attribution list for agent 1: 1..=5 entries, wild magnitudes incl. cap-busters
        let n = 1 + rng.below(5) as usize;
        let mut deltas: Vec<Delta> = (0..n).map(|_| (rng.next() % 90_000_001) as i64 - 45_000_000).collect();
        let exact = rng.coin();
        let realized: Hash32 = if exact { [7; 32] } else { [9; 32] };
        let run = |ds: &[Delta]| -> Amount {
            let mut fx = Fx::new();
            let id = tid(1);
            fx.ready(id, 7, &[(1, 100.0)]);
            fx.k.commit(&id, 300).unwrap();
            let v: Vec<(AgentId, Delta)> = ds.iter().map(|d| (ag(1), *d)).collect();
            fx.k.settle(&ORACLE, &id, realized, &v, 400).unwrap();
            fx.k.agent(&ag(1)).unwrap().reputation
        };
        let want = expected_rep(50.0, &deltas, exact);
        assert_eq!(run(&deltas), want, "iter {iter}: kernel disagrees with independent computation");
        // permutations of the same list must land on the same reputation
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
fn legal_edge(from: TState, to: TState) -> bool {
    use TState::*;
    from == to || matches!(
        (from, to),
        (Registered, Planning) | (Planning, Committable) | (Committable, Committed) | (Committed, Settled)
            | (Registered, Expired) | (Planning, Expired) | (Committable, Expired)
            | (Registered, Rejected) | (Planning, Rejected) | (Committable, Rejected)
    )
}

#[test]
fn p6_lifecycle_never_regresses_under_random_ops() {
    for seed in 0..3u64 {
        let mut rng = Rng::new(90_000 + seed);
        let mut fx = Fx::new();
        let ids: Vec<TransitionId> = (1..=4u8).map(tid).collect();
        let mut last: BTreeMap<TransitionId, TState> = BTreeMap::new();
        let mut clock: u64 = 100;
        for step in 0..2000u64 {
            // non-decreasing time, with an occasional attempted regression
            let now = if rng.below(20) == 0 { clock.saturating_sub(1) } else { clock + rng.below(3) };
            if now >= clock { clock = now; }
            let id = ids[rng.below(4) as usize];
            let who = [ag(1), ag(2), EVAL, ORACLE, ADMIN][rng.below(5) as usize];
            match rng.below(12) {
                0 => { let _ = fx.k.register_transition(&ag(1), Manifest { id, transition_type: TT, parent_index: 0, parent_hash: [0; 32], declared_next_hash: [7; 32], observation_id: OBS, expires_at: 10_000, relevant_groups: BTreeSet::from([1]) }, pol(), BTreeSet::new(), now); }
                1 => { let _ = fx.k.report_constraint(&EVAL, &id, C1, if rng.coin() { ConstraintStatus::Pass } else { ConstraintStatus::Fail }, now); }
                2 => { let _ = fx.k.submit_signal(&who, &id, a(1.0), rng.amount(1), now); }
                3 => { let _ = fx.k.advance_round(&who, &id, now); }
                4 => { let _ = fx.k.open_planning(&ag(1), &id, now); }
                5 => { let _ = fx.k.submit_plan(&who, Plan { agent: who, claims: BTreeMap::from([(RES, rng.amount(600))]), expires_at: 4000 }, &id, now); }
                6 => { let _ = fx.k.raise_exception(&EVAL, &id, [8; 32], Disposition::Block, now); }
                7 => { let _ = fx.k.resolve_exception(&EVAL, &id, [8; 32], now); }
                8 => { let _ = fx.k.reconcile(&who, &id, now); }
                9 => { let _ = fx.k.commit(&id, now); }
                10 => { let _ = fx.k.settle(&ORACLE, &id, [7; 32], &[], now); }
                _ => { if rng.coin() { let _ = fx.k.reject(&EVAL, &id, now); } else { let _ = fx.k.expire(&id, now); } }
            }
            if let Some(s) = fx.k.state_of(&id) {
                if let Some(prev) = last.get(&id) {
                    assert!(legal_edge(*prev, s), "seed {seed} step {step}: illegal transition {prev:?} -> {s:?}");
                    if matches!(prev, TState::Settled | TState::Rejected | TState::Expired) {
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
    // three sequential commits, each naming the previous declared state as parent
    for seq in 1..=3u8 {
        let id = tid(seq);
        let base = 100 + (seq as u64 - 1) * 400;
        fx.ready_at(id, 20 + seq, &[(1, 10.0)], base, [70 + seq; 32]);
        let before = fx.k.declared_state().clone();
        let s = fx.k.commit(&id, base + 200).unwrap();
        assert_eq!(s.index, before.index + 1);
        assert_eq!(s.hash, [20 + seq; 32]);
        assert_eq!(fx.k.history().len(), seq as usize);
        assert_eq!(fx.k.history()[(seq - 1) as usize], (s.index, id, s.hash));
    }
    // a fourth transition registered against the ORIGINAL genesis parent is stale:
    // its LINEAGE bit must be set and its commit refused, forever
    let stale = tid(9);
    let mut m = fx.manifest(stale, 99);
    m.parent_index = 0;
    m.parent_hash = [0; 32];
    fx.k.register_transition(&ag(1), m, pol(), BTreeSet::new(), 1400).unwrap();
    assert!(fx.k.commit_fail_mask(&stale, 1400).unwrap().has(FailMask::LINEAGE));
    assert_eq!(fx.k.state_of(&stale), Some(TState::Registered));
}
