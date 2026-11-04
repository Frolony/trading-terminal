#![allow(dead_code)]
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

// ------------------------------------------------------------ allocation meter
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst};

struct Meter;
static ARMED: AtomicBool = AtomicBool::new(false);
static CUM: AtomicUsize = AtomicUsize::new(0);
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static COUNT: AtomicUsize = AtomicUsize::new(0);
static BIGGEST: AtomicUsize = AtomicUsize::new(0);
/// The meter is process-global, so the measuring tests take turns.
static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

unsafe impl GlobalAlloc for Meter {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if ARMED.load(SeqCst) {
            CUM.fetch_add(l.size(), SeqCst); COUNT.fetch_add(1, SeqCst);
            BIGGEST.fetch_max(l.size(), SeqCst);
            let now = LIVE.fetch_add(l.size(), SeqCst) + l.size();
            PEAK.fetch_max(now, SeqCst);
        }
        System.alloc(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        if ARMED.load(SeqCst) { let _ = LIVE.try_update(SeqCst, SeqCst, |v| Some(v.saturating_sub(l.size()))); }
        System.dealloc(p, l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        if ARMED.load(SeqCst) {
            // accounted as free(old) + alloc(new), as in the program-side meter
            let _ = LIVE.try_update(SeqCst, SeqCst, |v| Some(v.saturating_sub(l.size())));
            CUM.fetch_add(n, SeqCst); COUNT.fetch_add(1, SeqCst);
            BIGGEST.fetch_max(n, SeqCst);
            let now = LIVE.fetch_add(n, SeqCst) + n;
            PEAK.fetch_max(now, SeqCst);
        }
        System.realloc(p, l, n)
    }
}
#[global_allocator]
static METER: Meter = Meter;

#[derive(Debug, Clone, Copy)]
struct Stat { cum: usize, peak: usize, count: usize, biggest: usize }
fn measure<R>(f: impl FnOnce() -> R) -> (R, Stat) {
    CUM.store(0, SeqCst); LIVE.store(0, SeqCst); PEAK.store(0, SeqCst); COUNT.store(0, SeqCst); BIGGEST.store(0, SeqCst);
    ARMED.store(true, SeqCst);
    let r = f();
    ARMED.store(false, SeqCst);
    (r, Stat { cum: CUM.load(SeqCst), peak: PEAK.load(SeqCst), count: COUNT.load(SeqCst), biggest: BIGGEST.load(SeqCst) })
}

/// One test only: the meter is process-global, so stages are measured serially.
#[test]
fn allocation_profile_of_the_stages() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let admin = id(ADMIN);
    let cfg = Config { rep_min: amt(0.0), rep_max: amt(2000.0), max_attribution: amt(100.0), epoch_length: EPOCH, council_seats: 3, reputation_weighs: true };
    let (_k, genesis) = measure(|| Kernel::new(admin, SystemState { index: 0, hash: id(99), timestamp: 1_000 }, cfg, TEnv { source: true, ..Default::default() }).unwrap());

    let mut h = H::sealed();
    let (tid, m) = (id(1), h.manifest(T1, 1));
    let (_, register) = measure(|| h.k.register_transition(&id(1), tid, m, BTreeSet::new(), h.t).unwrap());
    let (_, evaluate) = measure(|| h.k.evaluate_transition(&tid, h.t).unwrap());
    let (_, eligible) = measure(|| h.k.mark_eligible(&tid, h.t).unwrap());
    for s in [1u8, 2, 3] { h.k.submit_round_signal(&id(s), &tid, Amount::ONE, Some(Amount::ONE), h.t).unwrap(); }
    h.t += 100;
    let (_, select) = measure(|| h.k.select_candidate(&tid, h.t).unwrap());

    // a whole sealed world built from scratch, as every replay rebuilds it
    let (_, world) = measure(H::sealed);

    println!("STAGE            cum_bytes  live_peak  allocs  biggest");
    for (n, s) in [("genesis(new)", genesis), ("register", register), ("evaluate", evaluate), ("mark_eligible", eligible), ("select", select), ("sealed world", world)] {
        println!("{:<15}{:>11}{:>11}{:>8}{:>9}", n, s.cum, s.peak, s.count, s.biggest);
    }

    // Regression ceilings (v0.4.1 measured 7,557 / 10,448): a record is boxed in the map and the genesis
    // notations are static data, so a head build must stay far below the old figures.
    assert!(genesis.cum <= 3_500, "genesis build allocates {} bytes", genesis.cum);
    assert!(register.cum <= 2_000 && register.biggest <= 1_000, "register allocates {} bytes, largest {}", register.cum, register.biggest);
    assert!(world.cum <= 12_000, "a sealed world allocates {} bytes", world.cum);
}

/// Replay cost of a chain: build the sealed world, then drive and commit `k` ordinary transitions in sequence,
/// all inside one metered window — what a replay of `k` priors allocates in this crate. Printed for the slope;
/// asserted for linearity (a quadratic term would show as a growing per-prior cost).
fn chain(k: u8) -> Stat {
    let (_, s) = measure(|| {
        let mut h = H::sealed();
        for n in 1..=k {
            let t = h.ordinary(10 + n, 1, &[1, 2, 3]);
            h.k.commit(&t, h.t).unwrap();
            h.t += 1;
            h.new_obs();
        }
    });
    s
}

#[test]
fn chain_replay_is_linear_in_priors() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let ks = [0u8, 1, 2, 3, 5, 8, 16, 24, 32];
    let rows: Vec<(u8, Stat)> = ks.iter().map(|&k| (k, chain(k))).collect();
    println!("PRIORS   cum_bytes  live_peak  allocs   cum_per_prior (vs 0)");
    let base = rows[0].1;
    for (k, s) in &rows {
        let per = if *k == 0 { 0 } else { (s.cum - base.cum) / *k as usize };
        println!("{:<8}{:>10}{:>11}{:>8}{:>10}", k, s.cum, s.peak, s.count, per);
    }
    // Marginal cost per prior over successive windows. A quadratic term would show as a rising trend; the
    // first prior pays one-off set-up and collections grow in steps (node splits, vec doubling), so single
    // windows wobble by ~10% and only the trend is asserted: the last window may not exceed the first of the
    // late windows by more than 10%, and no late window may exceed 1.5x the whole-chain average.
    let marginal = |a: usize, b: usize| (rows[b].1.cum - rows[a].1.cum) / (rows[b].0 - rows[a].0) as usize;
    let late: Vec<usize> = vec![marginal(5, 6), marginal(6, 7), marginal(7, 8)]; // 8->16, 16->24, 24->32
    let avg = (rows[8].1.cum - rows[0].1.cum) / 32;
    println!("marginal per prior: 8->16 {}, 16->24 {}, 24->32 {}; whole-chain average {}", late[0], late[1], late[2], avg);
    assert!(late[2] * 10 <= late[0] * 11, "per-prior cost trends upward: {late:?}");
    assert!(late.iter().all(|m| m * 2 <= avg * 3), "a window exceeds 1.5x the average: {late:?} vs {avg}");
}
