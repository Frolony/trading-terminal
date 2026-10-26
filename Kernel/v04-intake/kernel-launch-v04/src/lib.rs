//! Migration step 2: the treasury launch profile (W2.4) rebuilt as configuration
//! over Jeffrey Beitel's v0.4 kernel crate.
//!
//! This crate never edits the delivered v0.4 crate's source: it consumes it by
//! path, exactly as the v0.2 stack does, and it never touches `Kernel/` in the
//! repo. All v0.4 work stays in `hidden_files/v04-intake/` until Carlos rules
//! the notes' branching question (branch vs replace outright once step 1 passed).
//!
//! What step 2 builds, in the notes' order:
//!   1. Genesis seeding — policies for the ordinary type and all four governance
//!      types, required constraints/objectives, six agents at base influence 2.0
//!      (each exactly 1/6 of total base, so the seal's share check passes),
//!      the operator roles, and the three capacity stocks.
//!   2. The seal — `seal_genesis` retires the genesis authority for good; after
//!      it, rules change only through committed governance transitions.
//!   3. The evidence-bound settlement composition — see [`SettlerGate`].
//!
//! One honest gap, author-side (roadmap Part B4), is demonstrated — not
//! papered over — by the tests in `tests/launch_v04.rs`:
//!   * F1 — CLOSED in the delivered v0.4.1 build: `Config::reputation_weighs`
//!     now exists, and the launch profile itself is pin-OFF (`config()`
//!     below sets it `false`): reputation is recorded, influence stays at
//!     the administrator-set base. L11 proves the profile default is OFF;
//!     L10 proves the explicit switch-OFF form; L8 keeps the switch-ON
//!     behavior pinned via an explicit `true` configuration, so the ON
//!     form stays proven without being the default.
//!   * Evidence lookup — the delivered `Env` trait has no evidence method, and
//!     `attest_settlement` does not consult any evidence record. Until the
//!     author adds an `Env` evidence method consulted by `attest_settlement`,
//!     evidence binding lives in [`SettlerGate`]: a composition the deploying
//!     process runs around the kernel, not a property the kernel enforces.
//!
//! Fixture levels (1000 / 500 / 250) are staging-test numbers that prove the
//! three stocks are set and debited independently. They are NOT pilot
//! parameters — stock sizing lives in the treasury mathematics documents and
//! is set by governance, never by this crate.

use coordination_kernel::*;
use std::collections::{BTreeMap, BTreeSet};

// ---------------------------------------------------------------- capacity stocks
/// Monthly distribution tranches (mirrors v0.2 `kernel-policy` id 0).
pub const RES_TREASURY_TRANCHE: ResourceId = 0;
/// The secured lending book (mirrors v0.2 `kernel-policy` id 1).
pub const RES_LENDING_BOOK: ResourceId = 1;
/// Rotational exit windows (mirrors v0.2 `kernel-policy` id 2).
pub const RES_EXIT_WINDOW: ResourceId = 2;

/// Staging fixture levels only — see the module docs. Not pilot parameters.
pub const TRANCHE_CAP: Amount = Amount::from_raw(1000 * SCALE);
pub const LENDING_CAP: Amount = Amount::from_raw(500 * SCALE);
pub const EXIT_CAP: Amount = Amount::from_raw(250 * SCALE);

// ---------------------------------------------------------------- identities
/// Genesis authority (retired by the seal).
pub const GEN: AgentId = [0xA0; 32];
/// Observer (posts observations).
pub const OBSERVER: AgentId = [0xA1; 32];
/// Executor (starts execution).
pub const EXECUTOR: AgentId = [0xB1; 32];
/// Settlers (attest realized hashes; threshold 2).
pub const SETTLER_A: AgentId = [0xB2; 32];
pub const SETTLER_B: AgentId = [0xB3; 32];
/// Outcome recorder (records attribution after settlement).
pub const RECORDER: AgentId = [0xB4; 32];

/// Ordinary transition type used by the launch-profile tests.
pub const TT: TransitionType = [9; 32];
/// Required constraint / objective ids for TT.
pub const C1: ConstraintId = [5; 32];
pub const O1: ObjectiveId = [6; 32];
/// Genesis observation id.
pub const OBS: Hash32 = [3; 32];

/// Agent ids 1..=6 for the seeded agent set.
pub fn ag(n: u8) -> AgentId {
    [n; 32]
}

/// Whole-unit amount, e.g. `amt(2.0)` == 2.0.
pub fn amt(x: f64) -> Amount {
    Amount::from_f64(x).expect("test amount must be finite and non-negative")
}

// ---------------------------------------------------------------- environment
/// The administrator's answers. v0.4 asks the Env itself; callers never assert
/// constraints or objectives. Identical in shape to the conformance port's env.
#[derive(Clone, Default)]
pub struct LaunchEnv {
    pub constraints: BTreeMap<ConstraintId, ConstraintStatus>,
    pub objectives: BTreeMap<ObjectiveId, bool>,
    pub positions: BTreeMap<AgentId, (u128, Timestamp)>,
    pub position_source: bool,
}

impl Env for LaunchEnv {
    fn constraint(&self, c: &ConstraintId, _m: &Manifest) -> ConstraintStatus {
        self.constraints.get(c).copied().unwrap_or(ConstraintStatus::Unavailable)
    }
    fn objective(&self, o: &ObjectiveId, _m: &Manifest) -> Option<bool> {
        self.objectives.get(o).copied()
    }
    fn has_position_source(&self) -> bool {
        self.position_source
    }
    fn locked_holding(&self, a: &AgentId) -> Option<(u128, Timestamp)> {
        self.positions.get(a).copied()
    }
}

/// The kernel type the launch profile configures.
pub type LaunchKernel = Kernel<LaunchEnv>;

// ---------------------------------------------------------------- profile
/// Policy shared by every type in the launch profile (same shape as the v0.2
/// profile's test policy and the v0.4 conformance fixture).
pub fn policy() -> Policy {
    Policy {
        quorum_required: amt(1.0),
        confidence_threshold: amt(0.5),
        min_obs_confidence: amt(0.5),
        require_objectives: false,
        consensus_window: 100,
        planning_window: 100,
        execution_window: 1000,
        max_plans: 6,
        settlement_threshold: 2,
        max_rounds: 3,
    }
}

/// Launch configuration: reputation 0..100, attribution capped at ±20, 1000s
/// epochs, a 3-seat council. `reputation_weighs` is false — the treasury
/// launch profile (F1 closed in the delivered v0.4.1 build): reputation is
/// recorded from outcomes and gives zero influence; influence is the
/// administrator-set base. Flipped from `true` in the pin-OFF step
/// (2026-10-26 BUILD run, run log `hidden_files/kernel-build-run-2026-10-26-0343.md`);
/// the switch-ON form stays proven by L8 via an explicit configuration.
pub fn config() -> Config {
    Config {
        rep_min: amt(0.0),
        rep_max: amt(100.0),
        max_attribution: amt(20.0),
        epoch_length: 1000,
        council_seats: 3,
        reputation_weighs: false,
    }
}

/// Fresh kernel at genesis time `t = 100`, with the launch env answering
/// C1 = Pass and O1 = true. Unsealed; the caller applies rules and seals.
pub fn fresh_kernel() -> LaunchKernel {
    fresh_kernel_with_config(config())
}

/// `fresh_kernel` under an explicit configuration — the seam L8/L10 use to
/// prove the switch-ON / switch-OFF forms without changing the profile.
pub fn fresh_kernel_with_config(cfg: Config) -> LaunchKernel {
    let mut env = LaunchEnv::default();
    env.constraints.insert(C1, ConstraintStatus::Pass);
    env.objectives.insert(O1, true);
    Kernel::new(
        GEN,
        SystemState { index: 0, hash: [0; 32], timestamp: 100 },
        cfg,
        env,
    )
    .expect("launch config must be valid")
}

/// Genesis seeding step, split for composition: policies for TT and all four
/// governance types, plus the required constraint/objective sets for TT.
pub fn apply_genesis_policies(k: &mut LaunchKernel, now: Timestamp) {
    k.genesis_apply(&GEN, GovAction::SetPolicy { ttype: TT, policy: policy() }, now)
        .expect("TT policy must apply at genesis");
    for t in [T_GOV_ADJUSTABLE, T_GOV_CONSTITUTION, T_GOV_COUNCIL, T_GOV_REBOOT] {
        k.genesis_apply(&GEN, GovAction::SetPolicy { ttype: t, policy: policy() }, now)
            .expect("governance policy must apply at genesis");
    }
    k.genesis_apply(
        &GEN,
        GovAction::SetRequiredConstraints { ttype: TT, set: BTreeSet::from([C1]) },
        now,
    )
    .expect("required constraints must apply at genesis");
    k.genesis_apply(
        &GEN,
        GovAction::SetRequiredObjectives { ttype: TT, set: BTreeSet::from([O1]) },
        now,
    )
    .expect("required objectives must apply at genesis");
}

/// Genesis seeding step: six agents, operator roles, three capacity stocks.
pub fn apply_genesis_agents_roles_stocks(k: &mut LaunchKernel, now: Timestamp) {
    for n in 1..=6u8 {
        k.genesis_apply(
            &GEN,
            GovAction::RegisterAgent {
                agent: ag(n),
                groups: BTreeSet::from([1]),
                base_influence: amt(2.0),
            },
            now,
        )
        .expect("agent must register at genesis");
    }
    for (who, role) in [
        (OBSERVER, Role::Observer),
        (EXECUTOR, Role::Executor),
        (SETTLER_A, Role::Settler),
        (SETTLER_B, Role::Settler),
        (RECORDER, Role::OutcomeRecorder),
    ] {
        k.genesis_apply(&GEN, GovAction::GrantRole { who, role }, now)
            .expect("role must grant at genesis");
    }
    for (resource, capacity) in [
        (RES_TREASURY_TRANCHE, TRANCHE_CAP),
        (RES_LENDING_BOOK, LENDING_CAP),
        (RES_EXIT_WINDOW, EXIT_CAP),
    ] {
        k.genesis_apply(&GEN, GovAction::SetCapacity { resource, capacity }, now)
            .expect("capacity stock must set at genesis");
    }
}

/// Genesis seeding step: policies, required sets, six agents, operator roles,
/// and the three capacity stocks. Deliberately NOT the council or the seal,
/// so tests can assert the seal's readiness gates compositionally.
pub fn apply_genesis_rules(k: &mut LaunchKernel, now: Timestamp) {
    apply_genesis_policies(k, now);
    apply_genesis_agents_roles_stocks(k, now);
}

/// Install the 3-seat launch council (seats 1, 2, 3).
pub fn install_launch_council(k: &mut LaunchKernel, now: Timestamp) {
    k.genesis_apply(
        &GEN,
        GovAction::InstallCouncil { seats: vec![ag(1), ag(2), ag(3)] },
        now,
    )
    .expect("launch council must install at genesis");
}

/// The sealed launch profile: fresh kernel, all genesis rules, council, seal,
/// and the genesis observation ingested. This is migration step 2's artifact.
pub fn sealed_launch_kernel() -> LaunchKernel {
    sealed_launch_kernel_with_config(config())
}

/// `sealed_launch_kernel` under an explicit configuration.
pub fn sealed_launch_kernel_with_config(cfg: Config) -> LaunchKernel {
    let mut k = fresh_kernel_with_config(cfg);
    apply_genesis_rules(&mut k, 100);
    install_launch_council(&mut k, 100);
    k.seal_genesis(&GEN, 100).expect("launch profile must seal");
    k.ingest_observation(
        &OBSERVER,
        Observation { id: OBS, timestamp: 100, expiry: 5000, confidence: amt(0.9) },
        100,
    )
    .expect("genesis observation must ingest");
    k
}

// ---------------------------------------------------------------- evidence gate
/// The evidence-bound settlement composition (Carlos's standing rule: settlers
/// may attest only the recorded evidence hash; no evidence, no settlement).
///
/// The deploying process records each transition's realized outcome hash in
/// this ledger *after* the outcome exists, then routes every settler
/// attestation through [`SettlerGate::attest_settlement`]. A refusal consumes
/// no kernel vote: the settler may still attest the correct hash afterwards,
/// which the tests prove by asserting the refused settler's next attestation
/// succeeds rather than returning `Duplicate`.
///
/// This gate is a composition around the kernel, not a kernel property: the
/// delivered v0.4 `Env` has no evidence method and `attest_settlement` does
/// not consult any record. When the author adds the `Env` evidence method,
/// this gate's ledger becomes that method's source and the gate itself can
/// retire — the tests that prove the composition stay valid either way.
#[derive(Clone, Default)]
pub struct EvidenceLedger {
    records: BTreeMap<TransitionId, Hash32>,
}

impl EvidenceLedger {
    pub fn new() -> Self {
        Self::default()
    }
    /// Record the realized outcome for `id`. Called by the observing process
    /// after the outcome exists — never before.
    pub fn record(&mut self, id: TransitionId, realized: Hash32) {
        self.records.insert(id, realized);
    }
    pub fn evidence_of(&self, id: &TransitionId) -> Option<Hash32> {
        self.records.get(id).copied()
    }
}

/// Outcome of a gated attestation attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateDecision {
    /// No evidence recorded for this transition: refused, no vote consumed.
    NoEvidence,
    /// Evidence recorded but this hash is not it: refused, no vote consumed.
    HashMismatch,
    /// Forwarded to the kernel; the bool is the kernel's settled flag.
    Attested(bool),
}

/// The gate itself: stateless over a ledger reference.
pub struct SettlerGate<'a> {
    ledger: &'a EvidenceLedger,
}

impl<'a> SettlerGate<'a> {
    pub fn new(ledger: &'a EvidenceLedger) -> Self {
        Self { ledger }
    }

    /// Attest `realized` for `id` only if it equals the recorded evidence.
    /// Refusals never reach the kernel, so they consume no vote and move no
    /// state; the transition stays `Executing` and retryable.
    pub fn attest_settlement(
        &self,
        k: &mut LaunchKernel,
        caller: &AgentId,
        id: &TransitionId,
        realized: Hash32,
        now: Timestamp,
    ) -> Result<GateDecision, KernelError> {
        match self.ledger.evidence_of(id) {
            None => Ok(GateDecision::NoEvidence),
            Some(evidence) if evidence != realized => Ok(GateDecision::HashMismatch),
            Some(_) => Ok(GateDecision::Attested(k.attest_settlement(caller, id, realized, now)?)),
        }
    }
}
