use crate::classify::{base_weight_of, classify_stance, sector_of, Contribution, Sector, SECTORS};
use crate::num::{Amount, Delta};
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;

pub type AgentId = [u8; 32];
pub type Hash32 = [u8; 32];
pub type TransitionId = [u8; 32];
pub type ConstraintId = [u8; 32];
pub type ObjectiveId = [u8; 32];
pub type CapabilityId = [u8; 32];
pub type TransitionType = [u8; 32];
pub type Symbol = [u8; 32];
pub type ChangeId = u64;
pub type GroupId = u16;
pub type ResourceId = u16;
pub type Timestamp = u64;

/// Zero-padded ASCII tag, for type, role and symbol names.
pub const fn tag(s: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    let mut i = 0;
    while i < s.len() && i < 32 {
        out[i] = s[i];
        i += 1;
    }
    out
}

pub const T_GOV_ADJUSTABLE: TransitionType = tag(b"GOV_ADJUSTABLE");
pub const T_GOV_CONSTITUTION: TransitionType = tag(b"GOV_CONSTITUTION");
pub const T_GOV_COUNCIL: TransitionType = tag(b"GOV_COUNCIL");
pub const T_GOV_REBOOT: TransitionType = tag(b"GOV_REBOOT");
pub fn is_gov_type(t: &TransitionType) -> bool {
    *t == T_GOV_ADJUSTABLE || *t == T_GOV_CONSTITUTION || *t == T_GOV_COUNCIL || *t == T_GOV_REBOOT
}

// Same values as KernelStorage.sol.
pub const MIN_SITTING_EPOCHS: u64 = 3;
pub const POSITION_EPOCHS: u64 = 3;
pub const DEFECTION_COOLDOWN_EPOCHS: u64 = 3;
pub const PAUSE_MAX: u64 = 3 * 86_400;
pub const MAX_SIBLINGS: usize = 16;
pub const MAX_REPLANS: u32 = 3;
pub const MAX_SEATS: u32 = 15;
pub const MAX_DIALECT_DEPS: usize = 8;
pub const MAX_EXCEPTIONS: usize = 64;
pub const MAX_PLANS_HARD: u32 = 64;
pub const MAX_GROUPS: usize = 16;

// ---------------------------------------------------------------- failure mask
/// Bitset of failed commit terms. Bit values are identical to `KernelStorage.sol`
/// (checked by `tests/parity.rs`, which reads the Solidity file).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FailMask(pub u32);
impl FailMask {
    pub const CONSTRAINTS: u32 = 1;
    pub const OBJECTIVES: u32 = 2;
    pub const QUORUM: u32 = 4;
    pub const CONFIDENCE: u32 = 8;
    pub const COMPAT: u32 = 16;
    pub const EXCEPTIONS: u32 = 32;
    pub const FRESH: u32 = 64;
    pub const PLANNING_OPEN: u32 = 128;
    pub const EXPIRED: u32 = 256;
    pub const LINEAGE: u32 = 512;
    pub const PLANS: u32 = 1024;
    pub const CAPABILITY: u32 = 2048;
    pub const DIALECT: u32 = 4096;
    pub fn is_clear(self) -> bool { self.0 == 0 }
    pub fn has(self, bit: u32) -> bool { self.0 & bit != 0 }
}

// ---------------------------------------------------------------- errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelError {
    Overflow,
    InvalidBounds,
    Unauthorized,
    TimeRegression,
    UnknownAgent,
    InactiveAgent,
    AgentExists,
    UnknownTransition,
    UnknownObservation,
    Duplicate,
    NotEligible,
    BadLifecycle(Life),
    BadWindow,
    ObservationFromFuture,
    ObservationNotFresh,
    ValueOutOfRange,
    AlreadySignalledThisRound,
    MonotoneRevisionViolation,
    ConsensusClosed,
    ConsensusStillOpen,
    QuorumNotMet,
    ConfidenceBelowThreshold,
    SiblingsStillOpen,
    OutrankedBySibling,
    SiblingCap,
    ParentAlreadyHasASelectedTransition,
    ParentNeDeclared,
    TransitionExpired,
    NotExpired,
    PlanningClosed,
    PlanningWindowOpen,
    PlanCap,
    NotParticipant,
    ReplanCap,
    StillBlocked,
    ConstraintsNotSatisfied,
    ObjectivesNotSatisfied,
    ExceptionsNotAccepted,
    ExceptionCap,
    RaiserCannotResolveOwnBlockingException,
    AlreadyVoted,
    AlreadyResolved,
    UnknownException,
    // governance
    NotGenesis,
    GenesisIsSealed,
    GenesisNotReady(&'static str),
    NotCouncil,
    CouncilNotUnanimous(AgentId),
    WrongGovCategory,
    GovernanceTypeOnly,
    UseGovernanceCommit,
    NotGovernanceTransition,
    NoPolicyForType,
    BadPolicy,
    BadSeats,
    SittingTooShort,
    NotCandidate(AgentId),
    NoPositionSource,
    AlreadyRecorded,
    InfluenceShareExceeded,
    NotFrozen,
    GovActionFailed(alloc::boxed::Box<KernelError>),
    // safety
    Paused,
    PauseCooldown,
    SystemFrozen,
    // dialect
    EmptySymbol,
    StalePrevChange,
    NoOpChange,
    PriorKindMismatch,
    DialectDepConflict,
    DialectDepCap,
    UnknownNotationChange,
    /// Commit refused; the mask says exactly which terms failed.
    CommitFailed(FailMask),
}

// ---------------------------------------------------------------- domain types
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role { Guardian, Observer, Executor, Settler, OutcomeRecorder }
fn role_bit(r: Role) -> u8 {
    match r { Role::Guardian => 1, Role::Observer => 2, Role::Executor => 4, Role::Settler => 8, Role::OutcomeRecorder => 16 }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintStatus { Pass, Fail, Unavailable }

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Disposition { Informational, Review, Block, Emergency }

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResolutionKind { Remediated, Waived, DefectionConfirmed, DefectionDismissed }

/// Same order and numbering as the Solidity `TransitionLifecycle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Life {
    Proposed = 1, Evaluated = 2, Eligible = 3, Selected = 4, Planning = 5, Reconciling = 6,
    Validated = 7, Committable = 8, Committed = 9, Executing = 10, Settled = 11, EvaluatedOutcome = 12,
    Discarded = 13, Rejected = 14, Expired = 15, Conflicted = 16, Blocked = 17,
}
impl Life {
    pub fn is_pre_commit(self) -> bool { ((self as u8) >= 1 && (self as u8) <= 8) || self == Life::Blocked }
}
fn allowed(from: Life, to: Life) -> bool {
    use Life::*;
    if from.is_pre_commit() && matches!(to, Expired | Conflicted | Discarded) { return true; }
    matches!(
        (from, to),
        (Proposed, Evaluated) | (Proposed, Rejected) | (Evaluated, Eligible) | (Eligible, Selected)
            | (Selected, Planning) | (Planning, Reconciling) | (Reconciling, Validated) | (Reconciling, Blocked)
            | (Validated, Committable) | (Validated, Blocked) | (Committable, Committed) | (Committable, Blocked)
            | (Committed, Executing) | (Executing, Settled) | (Settled, EvaluatedOutcome)
            | (Blocked, Validated) | (Blocked, Planning)
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemMode { Operational, CollapsedFrozen }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemState { pub index: u64, pub hash: Hash32, pub timestamp: Timestamp }

#[derive(Debug, Clone)]
pub struct Observation { pub id: Hash32, pub timestamp: Timestamp, pub expiry: Timestamp, pub confidence: Amount }

#[derive(Debug, Clone)]
pub struct AgentProfile {
    pub active: bool,
    pub groups: BTreeSet<GroupId>,
    pub base_influence: Amount,
    pub reputation: Amount,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    pub quorum_required: Amount,
    pub confidence_threshold: Amount,
    pub min_obs_confidence: Amount,
    pub require_objectives: bool,
    pub consensus_window: u64,
    pub planning_window: u64,
    pub execution_window: u64,
    pub max_plans: u32,
    pub settlement_threshold: u32,
    pub max_rounds: u32,
}

#[derive(Debug, Clone)]
pub struct Manifest {
    pub transition_type: TransitionType,
    pub parent_index: u64,
    pub parent_hash: Hash32,
    pub declared_next_hash: Hash32,
    pub observation_id: Hash32,
    pub expires_at: Timestamp,
    pub relevant_groups: BTreeSet<GroupId>,
    /// Set by the kernel for governance transitions, so evaluators can read the exact
    /// action before any vote. Whatever a caller puts here is ignored.
    pub action: Option<GovAction>,
}

#[derive(Debug, Clone)]
pub struct Plan { pub agent: AgentId, pub claims: BTreeMap<ResourceId, Amount>, pub expires_at: Timestamp }

#[derive(Debug, Clone)]
pub struct Config {
    pub rep_min: Amount,
    pub rep_max: Amount,
    pub max_attribution: Amount,
    pub epoch_length: u64,
    pub council_seats: u32,
    /// Whether reputation scales influence (Solidity parity: true). A launch profile that rules
    /// "reputation is recorded but has zero influence" sets this false: influence is then the
    /// administrator-set base, identical for every reputation.
    pub reputation_weighs: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SymbolKind { Constant, Function, Predicate, Type, Role }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotationStatus { Proposed, Committed }

#[derive(Debug, Clone)]
pub struct NotationChange {
    pub old_symbol: Symbol,
    pub new_symbol: Symbol,
    pub old_kind: SymbolKind,
    pub new_kind: SymbolKind,
    pub definition_hash: Hash32,
    pub prev_change: Option<ChangeId>,
    pub reason: String,
    pub proposer: AgentId,
    pub committed_epoch: u64,
    pub status: NotationStatus,
}
#[derive(Debug, Clone)]
pub struct NotationInput {
    pub old_symbol: Symbol,
    pub new_symbol: Symbol,
    pub old_kind: SymbolKind,
    pub new_kind: SymbolKind,
    pub definition_hash: Hash32,
    pub prev_change: Option<ChangeId>,
    pub reason: String,
}

/// Every rule change is one of these, and each is reachable only through a committed
/// governance transition (or the genesis authority before the seal).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GovAction {
    GrantRole { who: AgentId, role: Role },
    RevokeRole { who: AgentId, role: Role },
    SetPolicy { ttype: TransitionType, policy: Policy },
    SetRequiredConstraints { ttype: TransitionType, set: BTreeSet<ConstraintId> },
    SetRequiredObjectives { ttype: TransitionType, set: BTreeSet<ObjectiveId> },
    SetRequiredCapabilities { ttype: TransitionType, reqs: Vec<(CapabilityId, Amount)> },
    SetAgentCapability { agent: AgentId, cap: CapabilityId, held: bool },
    SetCapacity { resource: ResourceId, capacity: Amount },
    /// A weight lever: always constitutional.
    SetGroupMultiplier { ttype: TransitionType, group: GroupId, mult: Amount },
    SetMaxInfluenceShare { share: Amount },
    RegisterAgent { agent: AgentId, groups: BTreeSet<GroupId>, base_influence: Amount },
    SetAgentActive { agent: AgentId, active: bool },
    SetCouncilSeats { seats: u32 },
    InstallCouncil { seats: Vec<AgentId> },
    Reboot,
}
impl GovAction {
    /// The transition type this action must travel under. Policy and required sets that
    /// target a governance type are constitutional, not adjustable.
    pub fn category(&self) -> TransitionType {
        match self {
            GovAction::SetPolicy { ttype, .. }
            | GovAction::SetRequiredConstraints { ttype, .. }
            | GovAction::SetRequiredObjectives { ttype, .. } => {
                if is_gov_type(ttype) { T_GOV_CONSTITUTION } else { T_GOV_ADJUSTABLE }
            }
            GovAction::SetGroupMultiplier { .. } | GovAction::SetMaxInfluenceShare { .. } | GovAction::SetCouncilSeats { .. } => T_GOV_CONSTITUTION,
            GovAction::InstallCouncil { .. } => T_GOV_COUNCIL,
            GovAction::Reboot => T_GOV_REBOOT,
            _ => T_GOV_ADJUSTABLE,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    Registered(TransitionId),
    Committed { id: TransitionId, index: u64 },
    GovernanceApplied(TransitionId),
    Settled { id: TransitionId, exact: bool },
    CouncilInstalled { epoch: u64 },
    NotationCommitted { change: ChangeId, by: Option<TransitionId> },
}

// ---------------------------------------------------------------- environment hooks
/// The outside world the kernel asks questions of. Callers never assert any of these
/// terms: the kernel calls the hook itself and treats a missing answer as "no".
pub trait Env {
    /// UNAVAILABLE is never PASS.
    fn constraint(&self, _c: &ConstraintId, _m: &Manifest) -> ConstraintStatus { ConstraintStatus::Unavailable }
    fn objective(&self, _o: &ObjectiveId, _m: &Manifest) -> Option<bool> { None }
    /// The R term; capped by the kernel at `RELEVANCE_CAP`.
    fn relevance(&self, _agent: &AgentId, _m: &Manifest) -> Amount { Amount::ONE }
    /// Whether a locked-holding source is configured at all.
    fn has_position_source(&self) -> bool { false }
    /// `(amount, locked_until)`. Only `amount > 0` and `locked_until >= epoch end` are ever read;
    /// the amount is never stored and never weights anything.
    fn locked_holding(&self, _agent: &AgentId) -> Option<(u128, Timestamp)> { None }
}
/// Default: everything unavailable. Nothing commits until the administrator wires evaluators.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoEnv;
impl Env for NoEnv {}

pub const RELEVANCE_CAP_UNITS: u64 = 1_000_000;

// ---------------------------------------------------------------- records
struct CommitPlan { next_cap: BTreeMap<ResourceId, Amount>, index: u64, next_hash: Hash32, proposer: AgentId }
#[derive(Debug, Default, Clone, Copy)]
struct Tally { total: Amount, weighted: Amount, count: u32 }
#[derive(Debug, Clone, Copy)]
struct Sig { support: Option<Amount>, eff: Amount }
#[derive(Debug, Clone)]
struct Exc { raiser: AgentId, offender: Option<AgentId>, disposition: Disposition, resolved: bool }

#[derive(Debug, Clone)]
struct Rec {
    proposer: AgentId,
    m: Manifest,
    pol: Policy,
    life: Life,
    constraint_ids: Vec<ConstraintId>,
    objective_ids: Vec<ObjectiveId>,
    constraints_ok: bool,
    objectives_ok: bool,
    eligible_at: Timestamp,
    consensus_deadline: Timestamp,
    round_sigs: BTreeMap<(usize, AgentId), Sig>,
    last_round: BTreeMap<AgentId, usize>,
    round_tally: BTreeMap<usize, Tally>,
    tally: Tally,
    last_sector: BTreeMap<AgentId, Sector>,
    plans: BTreeMap<AgentId, Plan>,
    planning_deadline: Timestamp,
    min_plan_expiry: Option<Timestamp>,
    compatible: bool,
    replans: u32,
    block_episode: u32,
    exceptions: Vec<Exc>,
    unresolved_blocking: u32,
    blocking_by: BTreeMap<AgentId, u32>,
    dialect_deps: Vec<ChangeId>,
    action: Option<GovAction>,
    committed_index: u64,
    execution_deadline: Timestamp,
    attested: BTreeSet<AgentId>,
    votes: BTreeMap<Hash32, u32>,
    realized: Option<Hash32>,
}

// ---------------------------------------------------------------- kernel
pub struct Kernel<E: Env = NoEnv> {
    env: E,
    cfg: Config,
    // authority
    genesis_admin: Option<AgentId>,
    sealed: bool,
    roles: BTreeMap<AgentId, u8>,
    // rules (written only by governance)
    policies: BTreeMap<TransitionType, Policy>,
    req_constraints: BTreeMap<TransitionType, BTreeSet<ConstraintId>>,
    req_objectives: BTreeMap<TransitionType, BTreeSet<ObjectiveId>>,
    req_caps: BTreeMap<TransitionType, Vec<(CapabilityId, Amount)>>,
    group_mult: BTreeMap<(TransitionType, GroupId), Amount>,
    max_share: Amount,
    capacity: BTreeMap<ResourceId, Amount>,
    // agents
    agents: BTreeMap<AgentId, AgentProfile>,
    rep_history: BTreeMap<AgentId, Vec<(Timestamp, Amount)>>,
    capabilities: BTreeMap<AgentId, BTreeSet<CapabilityId>>,
    total_base: Amount,
    largest_base: Amount,
    accepted: BTreeMap<AgentId, u64>,
    niche: BTreeMap<AgentId, [Amount; SECTORS]>,
    // council and position
    council: Vec<AgentId>,
    council_since: u64,
    council_seats: u32,
    streak: BTreeMap<AgentId, u64>,
    last_pos1: BTreeMap<AgentId, u64>,
    last_defection1: BTreeMap<AgentId, u64>,
    approvals: BTreeMap<TransitionId, BTreeMap<AgentId, GovAction>>,
    res_votes: BTreeMap<(TransitionId, usize, ResolutionKind, Hash32), BTreeSet<AgentId>>,
    discard_votes: BTreeMap<(TransitionId, u32), BTreeSet<AgentId>>,
    // world
    observations: BTreeMap<Hash32, Observation>,
    transitions: BTreeMap<TransitionId, Rec>,
    siblings: BTreeMap<(u64, Hash32), Vec<TransitionId>>,
    selected_for: BTreeMap<(u64, Hash32), TransitionId>,
    declared: SystemState,
    mode: SystemMode,
    paused_until: Timestamp,
    next_pause_allowed: Timestamp,
    clock: Timestamp,
    epoch_zero: Timestamp,
    // dialect
    changes: Vec<NotationChange>,
    head: BTreeMap<Symbol, ChangeId>,
    dialect_epoch: u64,
    history: Vec<(u64, TransitionId, Hash32)>,
    events: Vec<Event>,
}

impl<E: Env> Kernel<E> {
    pub fn new(genesis_admin: AgentId, genesis: SystemState, cfg: Config, env: E) -> Result<Self, KernelError> {
        if cfg.rep_min > cfg.rep_max || cfg.epoch_length == 0 || cfg.council_seats == 0 || cfg.council_seats > MAX_SEATS || cfg.council_seats.is_multiple_of(2) {
            return Err(KernelError::InvalidBounds);
        }
        let clock = genesis.timestamp;
        let seats = cfg.council_seats;
        let mut k = Self {
            env, cfg, genesis_admin: Some(genesis_admin), sealed: false, roles: BTreeMap::new(),
            policies: BTreeMap::new(), req_constraints: BTreeMap::new(), req_objectives: BTreeMap::new(),
            req_caps: BTreeMap::new(), group_mult: BTreeMap::new(),
            max_share: Amount::from_raw(Amount::ONE.raw().div_ceil(6)), capacity: BTreeMap::new(),
            agents: BTreeMap::new(), rep_history: BTreeMap::new(), capabilities: BTreeMap::new(),
            total_base: Amount::ZERO, largest_base: Amount::ZERO, accepted: BTreeMap::new(), niche: BTreeMap::new(),
            council: Vec::new(), council_since: 0, council_seats: seats, streak: BTreeMap::new(),
            last_pos1: BTreeMap::new(), last_defection1: BTreeMap::new(), approvals: BTreeMap::new(), res_votes: BTreeMap::new(), discard_votes: BTreeMap::new(),
            observations: BTreeMap::new(), transitions: BTreeMap::new(), siblings: BTreeMap::new(), selected_for: BTreeMap::new(),
            declared: genesis, mode: SystemMode::Operational, paused_until: 0, next_pause_allowed: 0, clock, epoch_zero: clock,
            changes: Vec::new(), head: BTreeMap::new(), dialect_epoch: 0, history: Vec::new(), events: Vec::new(),
        };
        k.record_genesis_notations(genesis_admin);
        Ok(k)
    }

    // ---- read access
    pub fn declared_state(&self) -> &SystemState { &self.declared }
    pub fn agent(&self, id: &AgentId) -> Option<&AgentProfile> { self.agents.get(id) }
    pub fn capacity_of(&self, r: ResourceId) -> Option<Amount> { self.capacity.get(&r).copied() }
    pub fn life_of(&self, id: &TransitionId) -> Option<Life> { self.transitions.get(id).map(|r| r.life) }
    pub fn realized_hash(&self, id: &TransitionId) -> Option<Hash32> { self.transitions.get(id).and_then(|r| r.realized) }
    pub fn events(&self) -> &[Event] { &self.events }
    pub fn history(&self) -> &[(u64, TransitionId, Hash32)] { &self.history }
    pub fn now(&self) -> Timestamp { self.clock }
    pub fn env(&self) -> &E { &self.env }
    pub fn env_mut(&mut self) -> &mut E { &mut self.env }
    pub fn is_sealed(&self) -> bool { self.sealed }
    pub fn council(&self) -> &[AgentId] { &self.council }
    pub fn is_seated(&self, a: &AgentId) -> bool { self.council.contains(a) }
    pub fn council_since(&self) -> u64 { self.council_since }
    pub fn max_influence_share(&self) -> Amount { self.max_share }
    pub fn mode(&self) -> SystemMode { self.mode }
    pub fn policy_of(&self, t: &TransitionType) -> Option<&Policy> { self.policies.get(t) }
    pub fn accepted_proposals(&self, a: &AgentId) -> u64 { self.accepted.get(a).copied().unwrap_or(0) }
    pub fn position_streak(&self, a: &AgentId) -> u64 { self.streak.get(a).copied().unwrap_or(0) }
    pub fn has_role(&self, who: &AgentId, r: Role) -> bool { self.roles.get(who).is_some_and(|b| b & role_bit(r) != 0) }
    pub fn dialect_head(&self, s: &Symbol) -> Option<ChangeId> { self.head.get(s).copied() }
    pub fn dialect_epoch(&self) -> u64 { self.dialect_epoch }
    pub fn notation(&self, id: ChangeId) -> Option<&NotationChange> { self.changes.get(id as usize) }
    pub fn notation_count(&self) -> usize { self.changes.len() }
    pub fn epoch(&self, now: Timestamp) -> u64 { now.saturating_sub(self.epoch_zero) / self.cfg.epoch_length }
    pub fn is_paused(&self, now: Timestamp) -> bool { now < self.paused_until }

    /// The agent's niche: the sector with the highest accumulated table weight
    /// (ties go to the lower sector). `None` for an agent with no weight yet.
    pub fn niche_of(&self, a: &AgentId) -> Option<(Sector, Amount)> {
        let n = self.niche.get(a)?;
        let mut best: Option<(usize, Amount)> = None;
        for (i, v) in n.iter().enumerate() {
            if *v > best.map_or(Amount::ZERO, |b| b.1) { best = Some((i, *v)); }
        }
        best.map(|(i, v)| (sector_from_index(i), v))
    }
    pub fn niche_score(&self, a: &AgentId, s: Sector) -> Amount {
        self.niche.get(a).map_or(Amount::ZERO, |n| n[s as usize])
    }

    /// I(a) = base * (rep - REP_MIN) / (REP_MAX - REP_MIN), at the reputation held at `ts`.
    pub fn influence_at(&self, a: &AgentId, ts: Timestamp) -> Result<Amount, KernelError> {
        let (Some(h), Some(p)) = (self.rep_history.get(a), self.agents.get(a)) else { return Ok(Amount::ZERO) };
        if !self.cfg.reputation_weighs { return Ok(p.base_influence); }
        let idx = h.partition_point(|(t, _)| *t <= ts);
        if idx == 0 { return Ok(Amount::ZERO); }
        let rep = h[idx - 1].1;
        let span = self.cfg.rep_max.checked_sub(self.cfg.rep_min)?;
        if span == Amount::ZERO { return Ok(Amount::ZERO); }
        p.base_influence.checked_mul_div(rep.checked_sub(self.cfg.rep_min)?, span)
    }

    // ---- guards
    fn tick(&mut self, now: Timestamp) -> Result<(), KernelError> {
        if now < self.clock { return Err(KernelError::TimeRegression); }
        self.clock = now;
        Ok(())
    }
    fn not_paused(&self, now: Timestamp) -> Result<(), KernelError> {
        if now < self.paused_until { Err(KernelError::Paused) } else { Ok(()) }
    }
    fn operational(&self, id: &TransitionId, now: Timestamp) -> Result<(), KernelError> {
        self.not_paused(now)?;
        let rec = self.rec(id)?;
        if self.mode != SystemMode::Operational && rec.m.transition_type != T_GOV_REBOOT { return Err(KernelError::SystemFrozen); }
        Ok(())
    }
    fn need_role(&self, who: &AgentId, r: Role) -> Result<(), KernelError> {
        if self.has_role(who, r) { Ok(()) } else { Err(KernelError::Unauthorized) }
    }
    fn active(&self, who: &AgentId) -> Result<&AgentProfile, KernelError> {
        match self.agents.get(who) { Some(p) if p.active => Ok(p), _ => Err(KernelError::InactiveAgent) }
    }
    fn rec(&self, id: &TransitionId) -> Result<&Rec, KernelError> {
        self.transitions.get(id).ok_or(KernelError::UnknownTransition)
    }
    fn rec_mut(&mut self, id: &TransitionId) -> Result<&mut Rec, KernelError> {
        self.transitions.get_mut(id).ok_or(KernelError::UnknownTransition)
    }
    fn want(&self, id: &TransitionId, l: Life) -> Result<(), KernelError> {
        let c = self.rec(id)?.life;
        if c == l { Ok(()) } else { Err(KernelError::BadLifecycle(c)) }
    }
    fn set_life(rec: &mut Rec, to: Life) -> Result<(), KernelError> {
        if !allowed(rec.life, to) { return Err(KernelError::BadLifecycle(rec.life)); }
        rec.life = to;
        if to == Life::Blocked { rec.block_episode += 1; }
        Ok(())
    }
    fn not_expired(&self, id: &TransitionId, now: Timestamp) -> Result<(), KernelError> {
        if now < self.rec(id)?.m.expires_at { Ok(()) } else { Err(KernelError::TransitionExpired) }
    }
    fn observation_fresh(&self, obs: &Hash32, min_conf: Amount, now: Timestamp) -> bool {
        self.observations.get(obs).is_some_and(|o| now < o.expiry && o.confidence >= min_conf)
    }

    // ================================================================ genesis
    /// Before the seal, the genesis authority may apply any rule action directly. After the
    /// seal it is erased and only `commit_governance` can apply one.
    pub fn genesis_apply(&mut self, caller: &AgentId, action: GovAction, now: Timestamp) -> Result<(), KernelError> {
        if self.sealed { return Err(KernelError::GenesisIsSealed); }
        if self.genesis_admin.as_ref() != Some(caller) { return Err(KernelError::NotGenesis); }
        self.tick(now)?;
        self.apply(action, true, now)
    }

    /// Retires the bootstrap authority for good. Refused until the machinery that replaces it
    /// exists: a full council, a policy for each governance type, and a weight bound that holds.
    pub fn seal_genesis(&mut self, caller: &AgentId, now: Timestamp) -> Result<(), KernelError> {
        if self.genesis_admin.as_ref() != Some(caller) { return Err(KernelError::NotGenesis); }
        if self.sealed { return Err(KernelError::GenesisIsSealed); }
        self.tick(now)?;
        if self.council.len() as u32 != self.council_seats { return Err(KernelError::GenesisNotReady("COUNCIL")); }
        for (t, n) in [(T_GOV_ADJUSTABLE, "GOV_ADJUSTABLE"), (T_GOV_CONSTITUTION, "GOV_CONSTITUTION"), (T_GOV_COUNCIL, "GOV_COUNCIL"), (T_GOV_REBOOT, "GOV_REBOOT")] {
            if !self.policies.contains_key(&t) { return Err(KernelError::GenesisNotReady(n)); }
        }
        if !self.share_ok(self.largest_base, self.total_base, self.max_share) { return Err(KernelError::GenesisNotReady("INFLUENCE_SHARE")); }
        self.sealed = true;
        self.genesis_admin = None;
        Ok(())
    }

    /// largest/total <= share, in exact integer arithmetic.
    fn share_ok(&self, largest: Amount, total: Amount, share: Amount) -> bool {
        (largest.raw() as u128) * (Amount::ONE.raw() as u128) <= (share.raw() as u128) * (total.raw() as u128)
    }

    // ================================================================ rule changes
    /// Validates and applies one action. Reached only from `genesis_apply` (pre-seal) and from
    /// `commit_governance`, which has already validated the action with `check_action`.
    fn apply(&mut self, action: GovAction, via_genesis: bool, now: Timestamp) -> Result<(), KernelError> {
        self.check_action(&action, via_genesis, now, None)?;
        self.do_action(action, via_genesis, now);
        Ok(())
    }

    /// Every way an action can fail, with no mutation. `proposer` is the agent whose accepted-proposal
    /// count the pending commit is about to raise by one (so a nominee who is also the proposer is
    /// judged as the commit leaves them).
    fn check_action(&self, action: &GovAction, via_genesis: bool, now: Timestamp, proposer: Option<&AgentId>) -> Result<(), KernelError> {
        match action {
            GovAction::SetPolicy { policy, .. } => {
                let ok = policy.confidence_threshold <= Amount::ONE && policy.min_obs_confidence <= Amount::ONE
                    && policy.max_plans > 0 && policy.max_plans <= MAX_PLANS_HARD
                    && policy.settlement_threshold > 0
                    && policy.max_rounds > 0 && policy.consensus_window >= policy.max_rounds as u64;
                if !ok { return Err(KernelError::BadPolicy); }
            }
            GovAction::SetAgentCapability { agent, .. } | GovAction::SetAgentActive { agent, .. } => {
                if !self.agents.contains_key(agent) { return Err(KernelError::UnknownAgent); }
            }
            GovAction::SetMaxInfluenceShare { share } => {
                if *share == Amount::ZERO || *share > Amount::ONE { return Err(KernelError::InvalidBounds); }
                // Never tighten below what an existing agent already holds: that would void a seated agent.
                if !self.share_ok(self.largest_base, self.total_base, *share) { return Err(KernelError::InfluenceShareExceeded); }
            }
            GovAction::RegisterAgent { agent, groups, base_influence } => {
                if self.rep_history.contains_key(agent) { return Err(KernelError::AgentExists); }
                if groups.len() > MAX_GROUPS { return Err(KernelError::InvalidBounds); }
                // The whale bound, in weight form: no one agent may carry more than `max_share`
                // of all base influence. Influence never exceeds base, so bounding base bounds weight.
                let total = self.total_base.checked_add(*base_influence)?;
                if self.sealed && !self.share_ok(*base_influence, total, self.max_share) { return Err(KernelError::InfluenceShareExceeded); }
            }
            GovAction::SetCouncilSeats { seats } => {
                if *seats == 0 || *seats > MAX_SEATS || seats % 2 == 0 { return Err(KernelError::BadSeats); }
            }
            GovAction::InstallCouncil { seats } => {
                if seats.len() as u32 != self.council_seats || seats.is_empty() { return Err(KernelError::BadSeats); }
                let e = self.epoch(now);
                if !via_genesis && !self.council.is_empty() && e < self.council_since + MIN_SITTING_EPOCHS { return Err(KernelError::SittingTooShort); }
                let mut seen = BTreeSet::new();
                for a in seats {
                    if !seen.insert(*a) { return Err(KernelError::BadSeats); }
                    self.active(a)?;
                    let bonus = u64::from(proposer == Some(a));
                    if !via_genesis && !self.candidate_at(a, now, bonus) { return Err(KernelError::NotCandidate(*a)); }
                }
            }
            GovAction::Reboot => {
                if self.mode != SystemMode::CollapsedFrozen { return Err(KernelError::NotFrozen); }
            }
            GovAction::GrantRole { .. } | GovAction::RevokeRole { .. } | GovAction::SetRequiredConstraints { .. }
            | GovAction::SetRequiredObjectives { .. } | GovAction::SetRequiredCapabilities { .. }
            | GovAction::SetCapacity { .. } | GovAction::SetGroupMultiplier { .. } => {}
        }
        Ok(())
    }

    /// Applies an action that `check_action` accepted. Cannot fail.
    fn do_action(&mut self, action: GovAction, _via_genesis: bool, now: Timestamp) {
        match action {
            GovAction::GrantRole { who, role } => { *self.roles.entry(who).or_insert(0) |= role_bit(role); }
            GovAction::RevokeRole { who, role } => { *self.roles.entry(who).or_insert(0) &= !role_bit(role); }
            GovAction::SetPolicy { ttype, policy } => { self.policies.insert(ttype, policy); }
            GovAction::SetRequiredConstraints { ttype, set } => { self.req_constraints.insert(ttype, set); }
            GovAction::SetRequiredObjectives { ttype, set } => { self.req_objectives.insert(ttype, set); }
            GovAction::SetRequiredCapabilities { ttype, reqs } => { self.req_caps.insert(ttype, reqs); }
            GovAction::SetAgentCapability { agent, cap, held } => {
                let s = self.capabilities.entry(agent).or_default();
                if held { s.insert(cap); } else { s.remove(&cap); }
            }
            GovAction::SetCapacity { resource, capacity } => { self.capacity.insert(resource, capacity); }
            GovAction::SetGroupMultiplier { ttype, group, mult } => { self.group_mult.insert((ttype, group), mult); }
            GovAction::SetMaxInfluenceShare { share } => { self.max_share = share; }
            GovAction::RegisterAgent { agent, groups, base_influence } => {
                if let Ok(t) = self.total_base.checked_add(base_influence) { self.total_base = t; }
                if base_influence > self.largest_base { self.largest_base = base_influence; }
                // New agents start at the midpoint of the range (0 in the symmetric Solidity range).
                let rep = Amount::from_raw(self.cfg.rep_min.raw() + (self.cfg.rep_max.raw() - self.cfg.rep_min.raw()) / 2);
                self.agents.insert(agent, AgentProfile { active: true, groups, base_influence, reputation: rep });
                self.rep_history.insert(agent, alloc::vec![(now, rep)]);
            }
            GovAction::SetAgentActive { agent, active } => {
                if let Some(p) = self.agents.get_mut(&agent) { p.active = active; }
            }
            GovAction::SetCouncilSeats { seats } => { self.council_seats = seats; }
            GovAction::InstallCouncil { seats } => {
                let e = self.epoch(now);
                self.council = seats;
                self.council_since = e;
                self.events.push(Event::CouncilInstalled { epoch: e });
            }
            GovAction::Reboot => { self.mode = SystemMode::Operational; }
        }
    }

    // ================================================================ council and position
    /// Epoch e counts as "held" only if, when recorded during e, the holding is locked past the
    /// END of e. Anyone may record for anyone, once per epoch; a missed epoch resets the streak.
    pub fn record_position(&mut self, agent: &AgentId, now: Timestamp) -> Result<(), KernelError> {
        self.tick(now)?;
        self.not_paused(now)?;
        if !self.env.has_position_source() { return Err(KernelError::NoPositionSource); }
        self.active(agent)?;
        let e = self.epoch(now);
        let last = self.last_pos1.get(agent).copied().unwrap_or(0);
        if last == e + 1 { return Err(KernelError::AlreadyRecorded); }
        let epoch_end = self.epoch_zero.saturating_add((e + 1).saturating_mul(self.cfg.epoch_length));
        let held = matches!(self.env.locked_holding(agent), Some((amt, until)) if amt > 0 && until >= epoch_end);
        let s = if held { if last != 0 && last == e { self.position_streak(agent) + 1 } else { 1 } } else { 0 };
        self.streak.insert(*agent, s);
        self.last_pos1.insert(*agent, e + 1);
        Ok(())
    }

    /// Holds a verified position (recorded this or last epoch), has held it for POSITION_EPOCHS,
    /// has at least one accepted proposal, is active, and has no confirmed defection in the cooldown.
    /// The amount is never read: position gates candidacy and nothing else.
    pub fn is_candidate(&self, agent: &AgentId, now: Timestamp) -> bool { self.candidate_at(agent, now, 0) }

    fn candidate_at(&self, agent: &AgentId, now: Timestamp, bonus_accepted: u64) -> bool {
        if !self.agents.get(agent).is_some_and(|p| p.active) { return false; }
        if self.accepted_proposals(agent) + bonus_accepted == 0 { return false; }
        let e = self.epoch(now);
        if self.last_pos1.get(agent).copied().unwrap_or(0) < e { return false; }
        if self.position_streak(agent) < POSITION_EPOCHS { return false; }
        if let Some(d) = self.last_defection1.get(agent) {
            if *d != 0 && e < (d - 1) + DEFECTION_COOLDOWN_EPOCHS { return false; }
        }
        true
    }

    /// A seated member signs off on one exact proposal (id + action). Registration of a governance
    /// transition refuses to open until every sitting seat has approved that exact pair.
    pub fn approve_governance(&mut self, caller: &AgentId, id: TransitionId, action: GovAction, now: Timestamp) -> Result<(), KernelError> {
        self.tick(now)?;
        self.not_paused(now)?;
        if !self.is_seated(caller) { return Err(KernelError::NotCouncil); }
        self.approvals.entry(id).or_default().insert(*caller, action);
        Ok(())
    }

    // ================================================================ emergency brake
    pub fn pause(&mut self, caller: &AgentId, now: Timestamp) -> Result<(), KernelError> {
        let pre_seal_admin = !self.sealed && self.genesis_admin.as_ref() == Some(caller);
        if !(self.has_role(caller, Role::Guardian) || pre_seal_admin) { return Err(KernelError::Unauthorized); }
        self.tick(now)?;
        if now < self.next_pause_allowed { return Err(KernelError::PauseCooldown); }
        self.paused_until = now + PAUSE_MAX;
        self.next_pause_allowed = self.paused_until + PAUSE_MAX; // an equal window in which governance can run
        Ok(())
    }
    pub fn unpause(&mut self, caller: &AgentId, now: Timestamp) -> Result<(), KernelError> {
        let pre_seal_admin = !self.sealed && self.genesis_admin.as_ref() == Some(caller);
        if !(self.has_role(caller, Role::Guardian) || pre_seal_admin) { return Err(KernelError::Unauthorized); }
        self.tick(now)?;
        self.paused_until = 0;
        Ok(())
    }

    /// A measurement, like an observation: freezes the substrate when the minimum collapses to zero.
    /// Leaving the frozen state is a governance transition, never a role call.
    pub fn record_branching_health(&mut self, caller: &AgentId, scores: [Amount; 4], now: Timestamp) -> Result<Amount, KernelError> {
        self.need_role(caller, Role::Observer)?;
        self.tick(now)?;
        self.not_paused(now)?;
        let min = scores.iter().copied().min().unwrap_or(Amount::ZERO);
        if min == Amount::ZERO { self.mode = SystemMode::CollapsedFrozen; }
        Ok(min)
    }

    // ================================================================ observation
    pub fn ingest_observation(&mut self, caller: &AgentId, o: Observation, now: Timestamp) -> Result<(), KernelError> {
        self.need_role(caller, Role::Observer)?;
        self.tick(now)?;
        self.not_paused(now)?;
        if o.timestamp > now { return Err(KernelError::ObservationFromFuture); }
        if o.expiry <= now || o.confidence > Amount::ONE { return Err(KernelError::BadWindow); }
        if self.observations.contains_key(&o.id) { return Err(KernelError::Duplicate); }
        self.observations.insert(o.id, o);
        Ok(())
    }

    // ================================================================ registration
    /// Policy comes only from `policies[type]`, which only governance writes. Unknown types fail.
    pub fn register_transition(&mut self, caller: &AgentId, id: TransitionId, m: Manifest, extra: BTreeSet<ConstraintId>, now: Timestamp) -> Result<(), KernelError> {
        if is_gov_type(&m.transition_type) { return Err(KernelError::GovernanceTypeOnly); }
        self.register(caller, id, m, extra, None, now)
    }

    /// Only a seated member may propose a rule change; the payload is the exact action, stored so
    /// evaluators can judge it before any vote. Every sitting seat must have approved this exact pair.
    pub fn register_governance_transition(&mut self, caller: &AgentId, id: TransitionId, m: Manifest, action: GovAction, now: Timestamp) -> Result<(), KernelError> {
        if !self.is_seated(caller) { return Err(KernelError::NotCouncil); }
        if m.transition_type != action.category() { return Err(KernelError::WrongGovCategory); }
        let appr = self.approvals.get(&id);
        for seat in &self.council {
            if appr.and_then(|a| a.get(seat)) != Some(&action) { return Err(KernelError::CouncilNotUnanimous(*seat)); }
        }
        self.register(caller, id, m, BTreeSet::new(), Some(action), now)
    }

    fn register(&mut self, caller: &AgentId, id: TransitionId, mut m: Manifest, extra: BTreeSet<ConstraintId>, action: Option<GovAction>, now: Timestamp) -> Result<(), KernelError> {
        self.tick(now)?;
        self.not_paused(now)?;
        self.active(caller)?;
        if self.mode != SystemMode::Operational && m.transition_type != T_GOV_REBOOT { return Err(KernelError::SystemFrozen); }
        if self.transitions.contains_key(&id) { return Err(KernelError::Duplicate); }
        if m.parent_index != self.declared.index || m.parent_hash != self.declared.hash { return Err(KernelError::ParentNeDeclared); }
        if m.expires_at <= now { return Err(KernelError::BadWindow); }
        if !self.observations.contains_key(&m.observation_id) { return Err(KernelError::UnknownObservation); }
        if m.relevant_groups.is_empty() || m.relevant_groups.len() > MAX_GROUPS { return Err(KernelError::InvalidBounds); }
        let pol = self.policies.get(&m.transition_type).cloned().ok_or(KernelError::NoPolicyForType)?;
        m.action = action.clone();
        let mut cs: BTreeSet<ConstraintId> = self.req_constraints.get(&m.transition_type).cloned().unwrap_or_default();
        cs.extend(extra);
        let os: Vec<ObjectiveId> = self.req_objectives.get(&m.transition_type).map(|s| s.iter().copied().collect()).unwrap_or_default();
        self.transitions.insert(id, Rec {
            proposer: *caller, m, pol, life: Life::Proposed,
            constraint_ids: cs.into_iter().collect(), objective_ids: os, constraints_ok: false, objectives_ok: false,
            eligible_at: 0, consensus_deadline: 0, round_sigs: BTreeMap::new(), last_round: BTreeMap::new(),
            round_tally: BTreeMap::new(), tally: Tally::default(), last_sector: BTreeMap::new(),
            plans: BTreeMap::new(), planning_deadline: 0, min_plan_expiry: None, compatible: false, replans: 0, block_episode: 0,
            exceptions: Vec::new(), unresolved_blocking: 0, blocking_by: BTreeMap::new(), dialect_deps: Vec::new(),
            action, committed_index: 0, execution_deadline: 0, attested: BTreeSet::new(), votes: BTreeMap::new(), realized: None,
        });
        self.events.push(Event::Registered(id));
        Ok(())
    }

    // ================================================================ evaluation and eligibility
    /// Anyone may trigger evaluation. FAIL on any constraint rejects; only UNAVAILABLE leaves the
    /// transition Proposed and retryable. UNAVAILABLE is never PASS.
    pub fn evaluate_transition(&mut self, id: &TransitionId, now: Timestamp) -> Result<(), KernelError> {
        self.tick(now)?;
        self.operational(id, now)?;
        self.want(id, Life::Proposed)?;
        self.not_expired(id, now)?;
        let rec = self.rec(id)?;
        if !self.observation_fresh(&rec.m.observation_id, rec.pol.min_obs_confidence, now) { return Err(KernelError::ObservationNotFresh); }
        let (mut any_fail, mut any_unavail) = (false, false);
        for c in &rec.constraint_ids {
            match self.env.constraint(c, &rec.m) {
                ConstraintStatus::Pass => {}
                ConstraintStatus::Fail => any_fail = true,
                ConstraintStatus::Unavailable => any_unavail = true,
            }
        }
        let objectives_ok = rec.objective_ids.iter().all(|o| self.env.objective(o, &rec.m) == Some(true));
        let rec = self.rec_mut(id)?;
        rec.constraints_ok = !any_fail && !any_unavail;
        rec.objectives_ok = objectives_ok;
        if any_fail { Self::set_life(rec, Life::Rejected)?; }
        else if !any_unavail { Self::set_life(rec, Life::Evaluated)?; }
        Ok(())
    }

    /// Evaluated -> Eligible. Feasibility gate: no objective score can compensate for a failed constraint.
    pub fn mark_eligible(&mut self, id: &TransitionId, now: Timestamp) -> Result<(), KernelError> {
        self.tick(now)?;
        self.operational(id, now)?;
        self.want(id, Life::Evaluated)?;
        self.not_expired(id, now)?;
        let rec = self.rec(id)?;
        if !rec.constraints_ok { return Err(KernelError::ConstraintsNotSatisfied); }
        if rec.pol.require_objectives && !rec.objectives_ok { return Err(KernelError::ObjectivesNotSatisfied); }
        if !self.observation_fresh(&rec.m.observation_id, rec.pol.min_obs_confidence, now) { return Err(KernelError::ObservationNotFresh); }
        let key = (rec.m.parent_index, rec.m.parent_hash);
        let deadline = now.checked_add(rec.pol.consensus_window).ok_or(KernelError::Overflow)?;
        // Join the field of siblings competing for this parent. A slot held by a transition that
        // is no longer Eligible can be reclaimed; otherwise the field is full.
        let lives: BTreeMap<TransitionId, Life> = self.siblings.get(&key).map(|v| v.iter().filter_map(|s| self.transitions.get(s).map(|r| (*s, r.life))).collect()).unwrap_or_default();
        let sib = self.siblings.entry(key).or_default();
        if sib.len() < MAX_SIBLINGS {
            sib.push(*id);
        } else {
            let slot = sib.iter().position(|s| lives.get(s) != Some(&Life::Eligible)).ok_or(KernelError::SiblingCap)?;
            sib[slot] = *id;
        }
        let rec = self.rec_mut(id)?;
        rec.eligible_at = now;
        rec.consensus_deadline = deadline;
        Self::set_life(rec, Life::Eligible)
    }

    // ================================================================ consensus
    /// Rounds are slices of the consensus window, derived from time: nobody advances a round,
    /// so nobody can advance one early to cut others off.
    pub fn round_of(&self, id: &TransitionId, now: Timestamp) -> Result<usize, KernelError> {
        let rec = self.rec(id)?;
        let n = rec.pol.max_rounds as u64;
        let slice = ((rec.consensus_deadline - rec.eligible_at) / n).max(1);
        let r = now.saturating_sub(rec.eligible_at) / slice;
        Ok(if r >= n { (n - 1) as usize } else { r as usize })
    }

    fn group_policy(&self, a: &AgentProfile, rec: &Rec) -> (bool, Amount) {
        // The HIGHEST-MATCHING group multiplier: a max, never a sum.
        let mut ok = false;
        let mut best = Amount::ZERO;
        for g in a.groups.intersection(&rec.m.relevant_groups) {
            let m = self.group_mult.get(&(rec.m.transition_type, *g)).copied().filter(|m| *m != Amount::ZERO).unwrap_or(Amount::ONE);
            if m > best { best = m; }
            ok = true;
        }
        (ok, best)
    }

    /// W_eff = raw · R · I(snapshot at eligibility) · GPolicy.
    pub fn effective_weight(&self, id: &TransitionId, agent: &AgentId, raw: Amount) -> Result<Amount, KernelError> {
        let rec = self.rec(id)?;
        let p = self.agents.get(agent).ok_or(KernelError::UnknownAgent)?;
        let (_, mult) = self.group_policy(p, rec);
        let cap = Amount::units(RELEVANCE_CAP_UNITS)?;
        let r = self.env.relevance(agent, &rec.m);
        let r = if r > cap { cap } else { r };
        let infl = self.influence_at(agent, rec.eligible_at)?;
        raw.checked_mul(r)?.checked_mul(infl)?.checked_mul(mult)
    }

    /// One signal per agent per round; revising means signalling in a later round, and a revision must
    /// move toward the mean the agent last saw. The latest signal per agent is the one that counts.
    /// `support == None` is "no position": recorded and classified, but adds no weight.
    pub fn submit_round_signal(&mut self, caller: &AgentId, id: &TransitionId, raw: Amount, support: Option<Amount>, now: Timestamp) -> Result<(), KernelError> {
        self.tick(now)?;
        self.operational(id, now)?;
        self.want(id, Life::Eligible)?;
        self.not_expired(id, now)?;
        let rec = self.rec(id)?;
        if now >= rec.consensus_deadline { return Err(KernelError::ConsensusClosed); }
        if raw > Amount::ONE || support.is_some_and(|s| s > Amount::ONE) { return Err(KernelError::ValueOutOfRange); }
        let round = self.round_of(id, now)?;
        if rec.round_sigs.contains_key(&(round, *caller)) { return Err(KernelError::AlreadySignalledThisRound); }
        let p = self.agents.get(caller).ok_or(KernelError::UnknownAgent)?;
        let (ok, _) = self.group_policy(p, rec);
        if !p.active || !ok { return Err(KernelError::NotEligible); }

        if let (Some(&last), Some(s)) = (rec.last_round.get(caller), support) {
            if let Some(prev) = rec.round_sigs.get(&(last, *caller)).and_then(|p| p.support) {
                if let Some(rt) = rec.round_tally.get(&last).filter(|t| t.total != Amount::ZERO) {
                    let mean = rt.weighted.checked_mul_div(Amount::ONE, rt.total)?;
                    let d = |x: Amount| x.raw().abs_diff(mean.raw());
                    if d(s) > d(prev) { return Err(KernelError::MonotoneRevisionViolation); }
                }
            }
        }
        let w = if support.is_none() { Amount::ZERO } else { self.effective_weight(id, caller, raw)? };
        let contribution = self.contribution_of(id, caller, raw, w)?;

        let rec = self.rec_mut(id)?;
        let s = support.unwrap_or(Amount::ZERO);
        if let Some(&last) = rec.last_round.get(caller) {
            let prev = rec.round_sigs[&(last, *caller)];
            let ps = prev.support.unwrap_or(Amount::ZERO);
            rec.tally.total = rec.tally.total.checked_sub(prev.eff)?;
            rec.tally.weighted = rec.tally.weighted.checked_sub(prev.eff.checked_mul(ps)?)?;
        } else {
            rec.tally.count += 1;
        }
        let term = w.checked_mul(s)?;
        rec.tally.total = rec.tally.total.checked_add(w)?;
        rec.tally.weighted = rec.tally.weighted.checked_add(term)?;
        let rt = rec.round_tally.entry(round).or_default();
        rt.total = rt.total.checked_add(w)?;
        rt.weighted = rt.weighted.checked_add(term)?;
        rt.count += 1;
        rec.round_sigs.insert((round, *caller), Sig { support, eff: w });
        rec.last_round.insert(*caller, round);

        // Classification: informational, never enters the arithmetic above.
        let stance = classify_stance(support);
        let sector = sector_of(stance, contribution);
        rec.last_sector.insert(*caller, sector);
        let slot = &mut self.niche.entry(*caller).or_insert([Amount::ZERO; SECTORS])[sector as usize];
        *slot = slot.checked_add(base_weight_of(stance, contribution))?;
        Ok(())
    }

    /// Read from facts the kernel already holds; nobody sets it. Earlier lines win.
    fn contribution_of(&self, id: &TransitionId, agent: &AgentId, raw: Amount, w: Amount) -> Result<Contribution, KernelError> {
        let rec = self.rec(id)?;
        if raw == Amount::ZERO { return Ok(Contribution::Abstaining); }
        if rec.blocking_by.get(agent).copied().unwrap_or(0) > 0 { return Ok(Contribution::Obstructive); }
        if let Some(reqs) = self.req_caps.get(&rec.m.transition_type) {
            if reqs.iter().any(|(c, _)| self.capabilities.get(agent).is_some_and(|s| s.contains(c))) { return Ok(Contribution::Specialist); }
        }
        if rec.proposer == *agent { return Ok(Contribution::Procedural); }
        if w == Amount::ZERO { return Ok(Contribution::Informational); }
        Ok(Contribution::Substantive)
    }

    pub fn confidence(&self, id: &TransitionId) -> Result<Amount, KernelError> { Ok(Self::confidence_of(self.rec(id)?)) }
    pub fn total_weight(&self, id: &TransitionId) -> Result<Amount, KernelError> { Ok(self.rec(id)?.tally.total) }
    fn confidence_of(rec: &Rec) -> Amount {
        if rec.tally.total == Amount::ZERO { return Amount::ZERO; }
        rec.tally.weighted.checked_mul_div(Amount::ONE, rec.tally.total).unwrap_or(Amount::ZERO)
    }
    fn qualifies(rec: &Rec) -> bool {
        rec.tally.total > Amount::ZERO && rec.tally.total >= rec.pol.quorum_required && Self::confidence_of(rec) >= rec.pol.confidence_threshold
    }

    /// Permissionless, after the consensus window closes. Among the siblings that qualify, the highest
    /// confidence wins; a tie goes to the sibling that became eligible first. A sibling whose window is
    /// still open blocks selection, so nobody can win by calling early.
    pub fn select_candidate(&mut self, id: &TransitionId, now: Timestamp) -> Result<(), KernelError> {
        self.tick(now)?;
        self.operational(id, now)?;
        self.want(id, Life::Eligible)?;
        self.not_expired(id, now)?;
        let rec = self.rec(id)?;
        if now < rec.consensus_deadline { return Err(KernelError::ConsensusStillOpen); }
        if !Self::qualifies(rec) {
            if !(rec.tally.total > Amount::ZERO && rec.tally.total >= rec.pol.quorum_required) { return Err(KernelError::QuorumNotMet); }
            return Err(KernelError::ConfidenceBelowThreshold);
        }
        let key = (rec.m.parent_index, rec.m.parent_hash);
        let sib = self.siblings.get(&key).cloned().unwrap_or_default();
        let mine = sib.iter().position(|s| s == id).unwrap_or(usize::MAX);
        let my_conf = Self::confidence_of(rec);
        for (i, s) in sib.iter().enumerate() {
            if s == id { continue; }
            let Some(sr) = self.transitions.get(s) else { continue };
            if sr.life != Life::Eligible || now >= sr.m.expires_at { continue; }
            if now < sr.consensus_deadline { return Err(KernelError::SiblingsStillOpen); }
            if Self::qualifies(sr) {
                let c = Self::confidence_of(sr);
                if c > my_conf || (c == my_conf && i < mine) { return Err(KernelError::OutrankedBySibling); }
            }
        }
        if let Some(prev) = self.selected_for.get(&key) {
            if prev != id {
                if let Some(pr) = self.transitions.get(prev) {
                    let l = pr.life;
                    if ((l as u8) >= 4 && (l as u8) <= 8) || l == Life::Blocked || l == Life::Committed { return Err(KernelError::ParentAlreadyHasASelectedTransition); }
                }
            }
        }
        self.selected_for.insert(key, *id);
        Self::set_life(self.rec_mut(id)?, Life::Selected)
    }

    // ================================================================ planning
    /// Selected -> Planning (anyone), or Blocked -> Planning for a re-plan (proposer or a seat, at most MAX_REPLANS).
    pub fn open_planning(&mut self, caller: &AgentId, id: &TransitionId, now: Timestamp) -> Result<(), KernelError> {
        self.tick(now)?;
        self.operational(id, now)?;
        let (life, proposer) = { let r = self.rec(id)?; (r.life, r.proposer) };
        let mut replan = false;
        if life == Life::Blocked {
            if !(*caller == proposer || self.is_seated(caller)) { return Err(KernelError::Unauthorized); }
            if self.rec(id)?.replans >= MAX_REPLANS { return Err(KernelError::ReplanCap); }
            replan = true;
        } else if life != Life::Selected {
            return Err(KernelError::BadLifecycle(life));
        }
        self.not_expired(id, now)?;
        let rec = self.rec_mut(id)?;
        if replan { rec.replans += 1; }
        rec.planning_deadline = now.checked_add(rec.pol.planning_window).ok_or(KernelError::Overflow)?;
        Self::set_life(rec, Life::Planning)
    }

    pub fn submit_plan(&mut self, caller: &AgentId, id: &TransitionId, plan: Plan, now: Timestamp) -> Result<(), KernelError> {
        self.tick(now)?;
        self.operational(id, now)?;
        self.want(id, Life::Planning)?;
        self.not_expired(id, now)?;
        let rec = self.rec(id)?;
        if now >= rec.planning_deadline { return Err(KernelError::PlanningClosed); }
        if plan.agent != *caller { return Err(KernelError::Unauthorized); }
        if plan.expires_at <= rec.planning_deadline { return Err(KernelError::BadWindow); }
        let p = self.agents.get(caller).ok_or(KernelError::UnknownAgent)?;
        let (ok, _) = self.group_policy(p, rec);
        if !p.active || !ok { return Err(KernelError::NotEligible); }
        if rec.plans.contains_key(caller) { return Err(KernelError::Duplicate); }
        if rec.plans.len() as u32 >= rec.pol.max_plans { return Err(KernelError::PlanCap); }
        let rec = self.rec_mut(id)?;
        rec.min_plan_expiry = Some(rec.min_plan_expiry.map_or(plan.expires_at, |c| c.min(plan.expires_at)));
        rec.plans.insert(*caller, plan);
        Ok(())
    }

    fn fits_capacity(&self, rec: &Rec) -> bool {
        let mut agg: BTreeMap<ResourceId, Amount> = BTreeMap::new();
        for p in rec.plans.values() {
            for (r, c) in &p.claims {
                let e = agg.entry(*r).or_insert(Amount::ZERO);
                match e.checked_add(*c) { Ok(v) => *e = v, Err(_) => return false }
            }
        }
        agg.iter().all(|(r, t)| self.capacity.get(r).is_some_and(|cap| t <= cap))
    }

    /// Planning closed -> compute compatibility once. The built-in K is the capacity fit; a transition
    /// with no plans is never compatible.
    pub fn reconcile_plans(&mut self, id: &TransitionId, now: Timestamp) -> Result<(), KernelError> {
        self.tick(now)?;
        self.operational(id, now)?;
        self.want(id, Life::Planning)?;
        self.not_expired(id, now)?;
        let rec = self.rec(id)?;
        if now < rec.planning_deadline { return Err(KernelError::PlanningWindowOpen); }
        let ok = !rec.plans.is_empty() && self.fits_capacity(rec);
        let rec = self.rec_mut(id)?;
        rec.compatible = ok;
        Self::set_life(rec, Life::Reconciling)
    }

    /// Reconciling or Blocked -> Validated (else Reconciling -> Blocked). Structural terms only;
    /// quorum, confidence, capability, dialect and the rest are re-checked by the one predicate.
    pub fn validate_transition(&mut self, id: &TransitionId, now: Timestamp) -> Result<(), KernelError> {
        self.tick(now)?;
        self.operational(id, now)?;
        let rec = self.rec(id)?;
        let cur = rec.life;
        if cur != Life::Reconciling && cur != Life::Blocked { return Err(KernelError::BadLifecycle(cur)); }
        let ok = rec.constraints_ok && rec.compatible && rec.unresolved_blocking == 0 && now < rec.m.expires_at
            && self.observation_fresh(&rec.m.observation_id, rec.pol.min_obs_confidence, now);
        let rec = self.rec_mut(id)?;
        if ok { Self::set_life(rec, Life::Validated) }
        else if cur == Life::Reconciling { Self::set_life(rec, Life::Blocked) }
        else { Err(KernelError::StillBlocked) }
    }

    // ================================================================ exceptions
    /// Safety is separate from consensus. A blocking exception on a Validated or Committable
    /// transition sends it back to Blocked.
    pub fn submit_exception(&mut self, caller: &AgentId, id: &TransitionId, disposition: Disposition, offender: Option<AgentId>, now: Timestamp) -> Result<usize, KernelError> {
        self.tick(now)?;
        self.operational(id, now)?;
        let cur = self.rec(id)?.life;
        if !(cur.is_pre_commit() && (cur as u8) >= 3) { return Err(KernelError::ExceptionsNotAccepted); }
        self.active(caller)?;
        let rec = self.rec_mut(id)?;
        if rec.exceptions.len() >= MAX_EXCEPTIONS { return Err(KernelError::ExceptionCap); }
        let idx = rec.exceptions.len();
        rec.exceptions.push(Exc { raiser: *caller, offender, disposition, resolved: false });
        if disposition >= Disposition::Block {
            rec.unresolved_blocking += 1;
            *rec.blocking_by.entry(*caller).or_insert(0) += 1;
            if cur == Life::Validated || cur == Life::Committable { Self::set_life(rec, Life::Blocked)?; }
        }
        Ok(idx)
    }

    /// Council seats adjudicate facts, not rules: an exception resolves when a strict majority of ALL
    /// seats agree on the same (kind, evidence). The raiser cannot vote on its own exception. A confirmed
    /// defection is recorded against the offender and keeps them off the council for the cooldown.
    pub fn resolve_exception(&mut self, caller: &AgentId, id: &TransitionId, index: usize, kind: ResolutionKind, evidence: Hash32, now: Timestamp) -> Result<(), KernelError> {
        self.tick(now)?;
        self.not_paused(now)?;
        if !self.is_seated(caller) { return Err(KernelError::NotCouncil); }
        let rec = self.rec(id)?;
        let e = rec.exceptions.get(index).ok_or(KernelError::UnknownException)?;
        if e.resolved { return Err(KernelError::AlreadyResolved); }
        if e.raiser == *caller { return Err(KernelError::RaiserCannotResolveOwnBlockingException); }
        let (disp, raiser, offender) = (e.disposition, e.raiser, e.offender);
        let voters = self.res_votes.entry((*id, index, kind, evidence)).or_default();
        if !voters.insert(*caller) { return Err(KernelError::AlreadyVoted); }
        if voters.len() * 2 <= self.council.len() { return Ok(()); }
        let epoch = self.epoch(now);
        let rec = self.rec_mut(id)?;
        rec.exceptions[index].resolved = true;
        if disp >= Disposition::Block {
            rec.unresolved_blocking -= 1;
            if let Some(n) = rec.blocking_by.get_mut(&raiser) { *n -= 1; }
        }
        if kind == ResolutionKind::DefectionConfirmed {
            if let Some(o) = offender { self.last_defection1.insert(o, epoch + 1); }
        }
        Ok(())
    }

    /// A Blocked transition holds its parent's selection slot, and only its proposer can withdraw it.
    /// If the proposer walks away, a strict majority of ALL council seats may discard it, which frees
    /// the slot. This adjudicates a fact (the transition is abandoned), not a rule. It works only from
    /// Blocked, never from Committable or later, and a seat votes once per blocking episode.
    /// Returns `true` once the transition is discarded.
    pub fn discard_blocked(&mut self, caller: &AgentId, id: &TransitionId, now: Timestamp) -> Result<bool, KernelError> {
        self.tick(now)?;
        self.operational(id, now)?;
        if !self.is_seated(caller) { return Err(KernelError::NotCouncil); }
        self.want(id, Life::Blocked)?;
        let episode = self.rec(id)?.block_episode;
        let voters = self.discard_votes.entry((*id, episode)).or_default();
        if !voters.insert(*caller) { return Err(KernelError::AlreadyVoted); }
        if voters.len() * 2 <= self.council.len() { return Ok(false); }
        Self::set_life(self.rec_mut(id)?, Life::Discarded)?;
        Ok(true)
    }

    // ================================================================ dialect
    fn record_genesis_notations(&mut self, by: AgentId) {
        let k = |a: &str, b: &str, ka, kb, r: &str| (tag(a.as_bytes()), tag(b.as_bytes()), ka, kb, String::from(r));
        use SymbolKind::*;
        let list = [
            k("F_DEADLINE", "F_PLANNING_OPEN", Constant, Constant, "bit tests whether the planning window is still open, not whether a deadline was missed; logic unchanged"),
            k("declareNotationChange", "proposeNotationChange", Function, Function, "governance-only log with immediate effect became an agent proposal that takes effect only when a dependent transition commits"),
            k("BadLifecycleTransition", "IllegalMove", Type, Type, "two names for one error: the guarded-lifecycle revert is IllegalMove everywhere"),
            k("submitConsensusSignal", "submitRoundSignal", Function, Function, "two names for one entry point; the round form is canonical and carries the one-signal-per-round rule"),
            k("AlreadySignalled", "AlreadySignalledThisRound", Type, Type, "the guard is per round, not per transition; the name now says so"),
            k("GOVERNANCE", "commitGovernance", Role, Function, "authority over rules moved from a role to a transition: a rule changes only when a governance transition passes the full commit predicate"),
            k("PROPOSER", "ACTIVE_AGENT", Role, Predicate, "any active agent may propose; governance transitions additionally require a council seat"),
            k("selectCandidate", "selectCandidate", Function, Function, "same name, new meaning: permissionless after the consensus window closes; highest-confidence qualifying sibling wins, earlier eligibility breaks ties"),
            k("GPolicy", "GPolicy", Function, Function, "same name, pinned meaning: the highest-matching group multiplier, not the sum of matching groups"),
        ];
        for (o, n, ko, kn, reason) in list {
            let id = self.changes.len() as ChangeId;
            self.dialect_epoch += 1;
            let prev = self.head.get(&o).copied();
            self.changes.push(NotationChange {
                old_symbol: o, new_symbol: n, old_kind: ko, new_kind: kn, definition_hash: [0; 32],
                prev_change: prev, reason, proposer: by, committed_epoch: self.dialect_epoch, status: NotationStatus::Committed,
            });
            self.head.insert(o, id);
            self.head.insert(n, id);
        }
    }

    /// A proposal, not an effect: it takes effect only when a transition that declared a dependency on it commits.
    pub fn propose_notation_change(&mut self, caller: &AgentId, p: NotationInput, now: Timestamp) -> Result<ChangeId, KernelError> {
        self.tick(now)?;
        self.not_paused(now)?;
        self.active(caller)?;
        if p.old_symbol == [0; 32] || p.new_symbol == [0; 32] { return Err(KernelError::EmptySymbol); }
        let head = self.head.get(&p.old_symbol).copied();
        if p.prev_change != head { return Err(KernelError::StalePrevChange); }
        if let Some(h) = head.and_then(|h| self.changes.get(h as usize)) {
            if h.new_symbol == p.old_symbol {
                if p.old_kind != h.new_kind { return Err(KernelError::PriorKindMismatch); }
                if p.new_symbol == p.old_symbol && p.new_kind == h.new_kind && p.definition_hash == h.definition_hash { return Err(KernelError::NoOpChange); }
            }
        }
        let id = self.changes.len() as ChangeId;
        self.changes.push(NotationChange {
            old_symbol: p.old_symbol, new_symbol: p.new_symbol, old_kind: p.old_kind, new_kind: p.new_kind,
            definition_hash: p.definition_hash, prev_change: p.prev_change, reason: p.reason, proposer: *caller,
            committed_epoch: 0, status: NotationStatus::Proposed,
        });
        Ok(id)
    }

    pub fn declare_dialect_dependency(&mut self, caller: &AgentId, id: &TransitionId, change: ChangeId, now: Timestamp) -> Result<(), KernelError> {
        self.tick(now)?;
        self.not_paused(now)?;
        let rec = self.rec(id)?;
        if rec.proposer != *caller { return Err(KernelError::Unauthorized); }
        if !rec.life.is_pre_commit() { return Err(KernelError::BadLifecycle(rec.life)); }
        let c = self.changes.get(change as usize).filter(|c| c.status == NotationStatus::Proposed).ok_or(KernelError::UnknownNotationChange)?;
        if rec.dialect_deps.len() >= MAX_DIALECT_DEPS { return Err(KernelError::DialectDepCap); }
        // Every change moves the head for both of its names, so two dependencies of one transition
        // may not share ANY symbol (old or new).
        for d in &rec.dialect_deps {
            let d = &self.changes[*d as usize];
            if d.old_symbol == c.old_symbol || d.old_symbol == c.new_symbol || d.new_symbol == c.old_symbol || d.new_symbol == c.new_symbol {
                return Err(KernelError::DialectDepConflict);
            }
        }
        self.rec_mut(id)?.dialect_deps.push(change);
        Ok(())
    }

    /// True iff every dialect change the transition depends on is committed, or is still live:
    /// proposed against the dialect that is currently committed.
    fn dialect_deps_ok(&self, rec: &Rec) -> bool {
        rec.dialect_deps.iter().all(|d| {
            let c = &self.changes[*d as usize];
            match c.status {
                NotationStatus::Committed => true,
                NotationStatus::Proposed => c.prev_change == self.head.get(&c.old_symbol).copied(),
            }
        })
    }

    fn commit_dialect_deps(&mut self, id: &TransitionId) -> Result<(), KernelError> {
        let deps = self.rec(id)?.dialect_deps.clone();
        for d in deps {
            let c = self.changes[d as usize].clone();
            if c.status == NotationStatus::Committed { continue; }
            if c.prev_change != self.head.get(&c.old_symbol).copied() { return Err(KernelError::StalePrevChange); }
            self.dialect_epoch += 1;
            let ch = &mut self.changes[d as usize];
            ch.status = NotationStatus::Committed;
            ch.committed_epoch = self.dialect_epoch;
            self.head.insert(c.old_symbol, d);
            self.head.insert(c.new_symbol, d);
            self.events.push(Event::NotationCommitted { change: d, by: Some(*id) });
        }
        Ok(())
    }

    // ================================================================ the one predicate
    /// Every capability the type requires must be covered by the plans' agents (base influence, as in Solidity).
    fn capability_covered(&self, rec: &Rec) -> bool {
        let Some(reqs) = self.req_caps.get(&rec.m.transition_type) else { return true };
        reqs.iter().all(|(cap, min)| {
            let mut acc = Amount::ZERO;
            for a in rec.plans.keys() {
                if self.capabilities.get(a).is_some_and(|s| s.contains(cap)) {
                    if let Some(p) = self.agents.get(a) {
                        match acc.checked_add(p.base_influence) { Ok(v) => acc = v, Err(_) => return false }
                    }
                }
            }
            acc >= *min
        })
    }

    /// Commit(T) = C ∧ (J if required) ∧ Quorum ∧ Conf≥θ ∧ K ∧ ExceptionsCleared ∧ Fresh
    ///           ∧ ¬PlanningOpen ∧ ¬Expired ∧ Lineage ∧ PlansLive ∧ CapabilityCovered ∧ DialectOk
    /// Same terms, same bits, same order as `commitFailMask` in Solidity. 0 means the predicate holds;
    /// it is the only definition of "may commit" and the only gate on changing the rules.
    pub fn commit_fail_mask(&self, id: &TransitionId, now: Timestamp) -> Result<FailMask, KernelError> {
        let rec = self.rec(id)?;
        let mut m = 0u32;
        if !rec.constraints_ok { m |= FailMask::CONSTRAINTS; }
        if rec.pol.require_objectives && !rec.objectives_ok { m |= FailMask::OBJECTIVES; }
        if rec.tally.total == Amount::ZERO || rec.tally.total < rec.pol.quorum_required { m |= FailMask::QUORUM; }
        if Self::confidence_of(rec) < rec.pol.confidence_threshold { m |= FailMask::CONFIDENCE; }
        // K: the engine verdict, re-read against live capacity (capacity is a consumed stock).
        if !(rec.compatible && self.fits_capacity(rec)) { m |= FailMask::COMPAT; }
        if rec.unresolved_blocking != 0 { m |= FailMask::EXCEPTIONS; }
        if !self.observation_fresh(&rec.m.observation_id, rec.pol.min_obs_confidence, now) { m |= FailMask::FRESH; }
        if now < rec.planning_deadline { m |= FailMask::PLANNING_OPEN; }
        if now >= rec.m.expires_at { m |= FailMask::EXPIRED; }
        if rec.m.parent_index != self.declared.index || rec.m.parent_hash != self.declared.hash { m |= FailMask::LINEAGE; }
        if rec.min_plan_expiry.is_none_or(|e| now >= e) { m |= FailMask::PLANS; }
        if !self.capability_covered(rec) { m |= FailMask::CAPABILITY; }
        if !self.dialect_deps_ok(rec) { m |= FailMask::DIALECT; }
        Ok(FailMask(m))
    }

    /// Validated -> Committable. Permissionless: the predicate is the gate, not a person.
    pub fn declare_commit(&mut self, id: &TransitionId, now: Timestamp) -> Result<(), KernelError> {
        self.tick(now)?;
        self.operational(id, now)?;
        self.want(id, Life::Validated)?;
        let mask = self.commit_fail_mask(id, now)?;
        if !mask.is_clear() { return Err(KernelError::CommitFailed(mask)); }
        Self::set_life(self.rec_mut(id)?, Life::Committable)
    }

    /// Re-runs the ENTIRE predicate (an exception or stale observation that appeared after
    /// `declare_commit` stops the commit) and works out everything the commit will do, with no
    /// mutation. Whatever this accepts, `commit_apply` carries out without failing.
    fn commit_check(&self, id: &TransitionId, now: Timestamp) -> Result<CommitPlan, KernelError> {
        self.want(id, Life::Committable)?;
        let mask = self.commit_fail_mask(id, now)?;
        if !mask.is_clear() { return Err(KernelError::CommitFailed(mask)); }
        let rec = self.rec(id)?;
        let mut debit: BTreeMap<ResourceId, Amount> = BTreeMap::new();
        for p in rec.plans.values() {
            for (r, c) in &p.claims {
                let e = debit.entry(*r).or_insert(Amount::ZERO);
                *e = e.checked_add(*c)?;
            }
        }
        let mut next_cap = self.capacity.clone();
        for (r, d) in &debit {
            let cap = next_cap.get_mut(r).ok_or(KernelError::CommitFailed(FailMask(FailMask::COMPAT)))?;
            *cap = cap.checked_sub(*d).map_err(|_| KernelError::CommitFailed(FailMask(FailMask::COMPAT)))?;
        }
        let index = self.declared.index.checked_add(1).ok_or(KernelError::Overflow)?;
        // A dialect change that went stale must stop the commit before anything moves.
        for d in &rec.dialect_deps {
            let c = &self.changes[*d as usize];
            if c.status == NotationStatus::Proposed && c.prev_change != self.head.get(&c.old_symbol).copied() {
                return Err(KernelError::StalePrevChange);
            }
        }
        Ok(CommitPlan { next_cap, index, next_hash: rec.m.declared_next_hash, proposer: rec.proposer })
    }

    fn commit_apply(&mut self, id: &TransitionId, now: Timestamp, plan: CommitPlan) {
        if let Some(rec) = self.transitions.get_mut(id) {
            rec.life = Life::Committed; // Committable -> Committed was verified by `commit_check`
            rec.committed_index = plan.index;
            rec.execution_deadline = now.saturating_add(rec.pol.execution_window);
        }
        self.capacity = plan.next_cap;
        self.declared = SystemState { index: plan.index, hash: plan.next_hash, timestamp: now };
        self.history.push((plan.index, *id, plan.next_hash));
        let _ = self.commit_dialect_deps(id); // dialect changes take effect with the commit that relies on them
        *self.accepted.entry(plan.proposer).or_insert(0) += 1; // counts toward council candidacy
        self.events.push(Event::Committed { id: *id, index: plan.index });
    }

    /// Ordinary commit. Permissionless; the predicate decides. All checks run before anything moves.
    pub fn commit(&mut self, id: &TransitionId, now: Timestamp) -> Result<SystemState, KernelError> {
        self.tick(now)?;
        self.operational(id, now)?;
        if is_gov_type(&self.rec(id)?.m.transition_type) { return Err(KernelError::UseGovernanceCommit); }
        let plan = self.commit_check(id, now)?;
        self.commit_apply(id, now, plan);
        Ok(self.declared.clone())
    }

    /// Commits a governance transition and applies its action in the same step. The whole commit and
    /// the action are validated first and applied only if both would succeed, so a committed
    /// governance transition always means an applied one, with no copy of the kernel needed.
    pub fn commit_governance(&mut self, id: &TransitionId, now: Timestamp) -> Result<SystemState, KernelError> {
        self.tick(now)?;
        self.operational(id, now)?;
        if !is_gov_type(&self.rec(id)?.m.transition_type) { return Err(KernelError::NotGovernanceTransition); }
        let action = self.rec(id)?.action.clone().ok_or(KernelError::NotGovernanceTransition)?;
        let plan = self.commit_check(id, now)?;
        self.check_action(&action, false, now, Some(&plan.proposer))
            .map_err(|e| KernelError::GovActionFailed(alloc::boxed::Box::new(e)))?;
        self.commit_apply(id, now, plan);
        self.do_action(action, false, now);
        self.events.push(Event::GovernanceApplied(*id));
        Ok(self.declared.clone())
    }

    // ================================================================ housekeeping
    pub fn expire(&mut self, id: &TransitionId, now: Timestamp) -> Result<(), KernelError> {
        self.tick(now)?;
        let rec = self.rec_mut(id)?;
        if !rec.life.is_pre_commit() { return Err(KernelError::BadLifecycle(rec.life)); }
        if now < rec.m.expires_at { return Err(KernelError::NotExpired); }
        Self::set_life(rec, Life::Expired)
    }
    /// Siblings lose once another transition advanced the declared state.
    pub fn mark_conflicted(&mut self, id: &TransitionId, now: Timestamp) -> Result<(), KernelError> {
        self.tick(now)?;
        let (pi, ph) = { let r = self.rec(id)?; (r.m.parent_index, r.m.parent_hash) };
        let current = pi == self.declared.index && ph == self.declared.hash;
        let rec = self.rec_mut(id)?;
        if !rec.life.is_pre_commit() { return Err(KernelError::BadLifecycle(rec.life)); }
        if current { return Err(KernelError::ParentNeDeclared); }
        Self::set_life(rec, Life::Conflicted)
    }
    /// A proposer may withdraw its own pre-commit transition.
    pub fn withdraw(&mut self, caller: &AgentId, id: &TransitionId, now: Timestamp) -> Result<(), KernelError> {
        self.tick(now)?;
        let rec = self.rec_mut(id)?;
        if rec.proposer != *caller { return Err(KernelError::Unauthorized); }
        if !rec.life.is_pre_commit() { return Err(KernelError::BadLifecycle(rec.life)); }
        Self::set_life(rec, Life::Discarded)
    }

    // ================================================================ execution and settlement
    pub fn start_execution(&mut self, caller: &AgentId, id: &TransitionId, now: Timestamp) -> Result<(), KernelError> {
        self.need_role(caller, Role::Executor)?;
        self.tick(now)?;
        self.operational(id, now)?;
        self.want(id, Life::Committed)?;
        let rec = self.rec_mut(id)?;
        if now > rec.execution_deadline { return Err(KernelError::BadWindow); }
        Self::set_life(rec, Life::Executing)
    }

    /// Reality is asserted by `settlement_threshold` distinct Settlers agreeing on one realized hash.
    /// This is the attestor logic, and the kernel owns all of it.
    pub fn attest_settlement(&mut self, caller: &AgentId, id: &TransitionId, realized: Hash32, now: Timestamp) -> Result<bool, KernelError> {
        self.need_role(caller, Role::Settler)?;
        self.tick(now)?;
        self.operational(id, now)?;
        self.want(id, Life::Executing)?;
        let rec = self.rec_mut(id)?;
        if !rec.attested.insert(*caller) { return Err(KernelError::Duplicate); }
        let v = rec.votes.entry(realized).or_insert(0);
        *v += 1;
        if *v >= rec.pol.settlement_threshold {
            rec.realized = Some(realized);
            Self::set_life(rec, Life::Settled)?;
            let exact = realized == rec.m.declared_next_hash;
            self.events.push(Event::Settled { id: *id, exact });
            return Ok(true);
        }
        Ok(false)
    }

    /// Settled -> EvaluatedOutcome. Per agent, deltas are summed (order-independent), capped at
    /// `±max_attribution`, then drift halves only positive credit. Success is derived, never asserted.
    pub fn record_outcome(&mut self, caller: &AgentId, id: &TransitionId, attributions: &[(AgentId, Delta)], now: Timestamp) -> Result<bool, KernelError> {
        self.need_role(caller, Role::OutcomeRecorder)?;
        self.tick(now)?;
        self.not_paused(now)?;
        self.want(id, Life::Settled)?;
        let rec = self.rec(id)?;
        let mut net: BTreeMap<AgentId, i128> = BTreeMap::new();
        for (a, d) in attributions {
            if !self.agents.contains_key(a) { return Err(KernelError::UnknownAgent); }
            if !rec.plans.contains_key(a) { return Err(KernelError::NotParticipant); }
            *net.entry(*a).or_insert(0) += *d as i128;
        }
        let exact = rec.realized == Some(rec.m.declared_next_hash);
        let cap = self.cfg.max_attribution.raw() as i128;
        let (lo, hi) = (self.cfg.rep_min, self.cfg.rep_max);
        for (a, n) in net {
            let mut n = n.clamp(-cap, cap);
            if !exact && n > 0 { n /= 2; }
            let p = self.agents.get_mut(&a).ok_or(KernelError::UnknownAgent)?;
            p.reputation = p.reputation.apply_delta(n, lo, hi);
            let r = p.reputation;
            let h = self.rep_history.entry(a).or_default();
            match h.last_mut() { Some(last) if last.0 == now => last.1 = r, _ => h.push((now, r)) }
        }
        Self::set_life(self.rec_mut(id)?, Life::EvaluatedOutcome)?;
        Ok(exact)
    }
}

fn sector_from_index(i: usize) -> Sector {
    use Sector::*;
    [Aligned, Misaligned, Collaborative, Divergent, Adversarial, Specialist, Disassociated, Abstaining, Uncertain][i]
}
