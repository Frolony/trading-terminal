use crate::num::{Amount, Delta};
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

pub type AgentId = [u8; 32];
pub type Hash32 = [u8; 32];
pub type TransitionId = [u8; 32];
pub type ConstraintId = [u8; 32];
pub type ExceptionId = [u8; 32];
pub type CapabilityId = [u8; 32];
pub type TransitionType = [u8; 32];
pub type GroupId = u16;
pub type ResourceId = u16;
pub type Timestamp = u64;

// ---------------------------------------------------------------- failure mask
/// Bitset of failed commit terms. Bit values are identical to `CoordinationKernel.sol`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FailMask(pub u32);
impl FailMask {
    pub const CONSTRAINTS: u32 = 1;
    pub const OBJECTIVES: u32 = 2;
    pub const QUORUM: u32 = 4;
    pub const CONFIDENCE: u32 = 8;
    pub const COMPAT: u32 = 16;
    pub const EXCEPTIONS: u32 = 32;
    /// Defined in Solidity but not yet set by its `commitFailMask`; set here.
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
    UnknownTransition,
    UnknownObservation,
    UnknownConstraint,
    UnknownException,
    Duplicate,
    NotEligible,
    BadState,
    BadWindow,
    ObservationFromFuture,
    SupportOutOfRange,
    AlreadySignalled,
    MonotoneRevision,
    RoundsExhausted,
    RoundEmpty,
    PlanningClosed,
    PlanningStillOpen,
    TooManyPlans,
    NotParticipant,
    /// Commit refused; the mask says exactly which terms failed.
    CommitFailed(FailMask),
}

// ---------------------------------------------------------------- domain types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role { Admin, Oracle, Evaluator }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintStatus { Pass, Fail, Unavailable }

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Disposition { Informational, Review, Block, Emergency }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TState { Registered, Planning, Committable, Committed, Settled, Rejected, Expired }

fn allowed(from: TState, to: TState) -> bool {
    use TState::*;
    matches!(
        (from, to),
        (Registered, Planning) | (Planning, Committable) | (Committable, Committed) | (Committed, Settled)
            | (Registered, Expired) | (Planning, Expired) | (Committable, Expired)
            | (Registered, Rejected) | (Planning, Rejected) | (Committable, Rejected)
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemState { pub index: u64, pub hash: Hash32, pub timestamp: Timestamp }

#[derive(Debug, Clone)]
pub struct Observation { pub id: Hash32, pub timestamp: Timestamp, pub expiry: Timestamp, pub confidence: Amount }

#[derive(Debug, Clone)]
pub struct AgentProfile { pub id: AgentId, pub active: bool, pub groups: BTreeSet<GroupId>, pub influence: Amount, pub reputation: Amount }

#[derive(Debug, Clone)]
pub struct Policy {
    pub quorum_required: Amount,
    pub confidence_threshold: Amount,
    pub min_obs_confidence: Amount,
    pub require_objectives: bool,
    pub planning_window: u64,
    pub max_rounds: u32,
    pub max_plans: u32,
}

#[derive(Debug, Clone)]
pub struct Manifest {
    pub id: TransitionId,
    pub transition_type: TransitionType,
    pub parent_index: u64,
    pub parent_hash: Hash32,
    pub declared_next_hash: Hash32,
    pub observation_id: Hash32,
    pub expires_at: Timestamp,
    pub relevant_groups: BTreeSet<GroupId>,
}

#[derive(Debug, Clone)]
pub struct Plan { pub agent: AgentId, pub claims: BTreeMap<ResourceId, Amount>, pub expires_at: Timestamp }

#[derive(Debug, Clone)]
pub struct Config { pub rep_min: Amount, pub rep_max: Amount, pub max_attribution: Amount }

#[derive(Debug, Clone)]
pub enum Event {
    Registered(TransitionId),
    Committed { id: TransitionId, index: u64 },
    Settled { id: TransitionId, exact: bool },
    Expired(TransitionId),
    Rejected(TransitionId),
}

#[derive(Debug, Clone)]
struct Signal { support: Amount }
#[derive(Debug, Default, Clone)]
struct Round { signals: BTreeMap<AgentId, Signal>, total: Amount, weighted: Amount }
#[derive(Debug, Clone)]
struct Exc { disposition: Disposition, resolved: bool }

#[derive(Debug, Clone)]
struct Rec {
    proposer: AgentId,
    m: Manifest,
    pol: Policy,
    state: TState,
    constraints: BTreeMap<ConstraintId, ConstraintStatus>,
    objectives_ok: bool,
    rounds: Vec<Round>,
    plans: BTreeMap<AgentId, Plan>,
    exceptions: BTreeMap<ExceptionId, Exc>,
    unresolved_blocking: u32,
    planning_deadline: Timestamp,
    reconciled: bool,
    realized: Option<Hash32>,
}


// ---------------------------------------------------------------- administrator hooks
/// Roles held by the caller of a guarded entry point.
#[derive(Debug, Clone, Copy)]
pub struct CallerRoles(u8);
impl CallerRoles {
    pub fn has(&self, r: Role) -> bool { self.0 & role_bit(r) != 0 }
}

/// Who may observe and settle, and what counts as sufficient proof, is the swarm
/// administrator's decision. The kernel only asks.
pub trait Attestor {
    fn may_observe(&self, caller: &CallerRoles, who: &AgentId, o: &Observation) -> bool;
    fn may_settle(&self, caller: &CallerRoles, who: &AgentId, id: &TransitionId, realized: &Hash32, attributions: &[(AgentId, Delta)]) -> bool;
}
/// Default: the Oracle role may observe and settle.
#[derive(Debug, Default, Clone, Copy)]
pub struct RoleAttestor;
impl Attestor for RoleAttestor {
    fn may_observe(&self, c: &CallerRoles, _: &AgentId, _: &Observation) -> bool { c.has(Role::Oracle) }
    fn may_settle(&self, c: &CallerRoles, _: &AgentId, _: &TransitionId, _: &Hash32, _: &[(AgentId, Delta)]) -> bool { c.has(Role::Oracle) }
}

/// How reputation, influence and relevance combine into W_eff = raw · R · I · GPolicy.
/// The administrator decides; the kernel multiplies with checked math.
pub trait WeightPolicy {
    fn relevance(&self, _a: &AgentProfile, _m: &Manifest) -> Amount { Amount::ONE }
    fn group_policy(&self, _a: &AgentProfile, _m: &Manifest) -> Amount { Amount::ONE }
    /// The I term. Default ignores reputation; override to make learning govern.
    fn influence(&self, a: &AgentProfile) -> Amount { a.influence }
}
/// Default: flat relevance and group policy, admin-set influence (matches Solidity `_eligible`).
#[derive(Debug, Default, Clone, Copy)]
pub struct FlatWeights;
impl WeightPolicy for FlatWeights {}

// ---------------------------------------------------------------- kernel
pub struct Kernel<A = RoleAttestor, W = FlatWeights> {
    attestor: A,
    weights: W,
    cfg: Config,
    roles: BTreeMap<AgentId, BTreeSet<u8>>,
    agents: BTreeMap<AgentId, AgentProfile>,
    capabilities: BTreeMap<AgentId, BTreeSet<CapabilityId>>,
    required_caps: BTreeMap<TransitionType, Vec<(CapabilityId, Amount)>>,
    mandatory: BTreeMap<TransitionType, BTreeSet<ConstraintId>>,
    capacity: BTreeMap<ResourceId, Amount>,
    observations: BTreeMap<Hash32, Observation>,
    transitions: BTreeMap<TransitionId, Rec>,
    declared: SystemState,
    clock: Timestamp,
    history: Vec<(u64, TransitionId, Hash32)>,
    events: Vec<Event>,
}

fn role_bit(r: Role) -> u8 { match r { Role::Admin => 1, Role::Oracle => 2, Role::Evaluator => 4 } }

impl Kernel<RoleAttestor, FlatWeights> {
    /// Default policies: role-based oracle, flat weights.
    pub fn new(admin: AgentId, genesis: SystemState, cfg: Config) -> Result<Self, KernelError> {
        Self::with_policies(admin, genesis, cfg, RoleAttestor, FlatWeights)
    }
}

impl<A: Attestor, W: WeightPolicy> Kernel<A, W> {
    pub fn with_policies(admin: AgentId, genesis: SystemState, cfg: Config, attestor: A, weights: W) -> Result<Self, KernelError> {
        if cfg.rep_min > cfg.rep_max { return Err(KernelError::InvalidBounds); }
        let mut roles = BTreeMap::new();
        roles.insert(admin, BTreeSet::from([role_bit(Role::Admin)]));
        let clock = genesis.timestamp;
        Ok(Self {
            attestor, weights,
            cfg, roles, agents: BTreeMap::new(), capabilities: BTreeMap::new(),
            required_caps: BTreeMap::new(), mandatory: BTreeMap::new(), capacity: BTreeMap::new(),
            observations: BTreeMap::new(), transitions: BTreeMap::new(), declared: genesis,
            clock, history: Vec::new(), events: Vec::new(),
        })
    }
    fn caller_roles(&self, who: &AgentId) -> CallerRoles {
        CallerRoles(self.roles.get(who).map_or(0, |s| s.iter().fold(0u8, |a, b| a | b)))
    }

    // ---- read access
    pub fn declared_state(&self) -> &SystemState { &self.declared }
    pub fn agent(&self, id: &AgentId) -> Option<&AgentProfile> { self.agents.get(id) }
    pub fn capacity_of(&self, r: ResourceId) -> Option<Amount> { self.capacity.get(&r).copied() }
    pub fn state_of(&self, id: &TransitionId) -> Option<TState> { self.transitions.get(id).map(|r| r.state) }
    pub fn realized_hash(&self, id: &TransitionId) -> Option<Hash32> { self.transitions.get(id).and_then(|r| r.realized) }
    pub fn events(&self) -> &[Event] { &self.events }
    pub fn history(&self) -> &[(u64, TransitionId, Hash32)] { &self.history }
    pub fn now(&self) -> Timestamp { self.clock }

    // ---- guards
    fn has_role(&self, who: &AgentId, r: Role) -> bool {
        self.roles.get(who).is_some_and(|s| s.contains(&role_bit(r)))
    }
    fn need(&self, who: &AgentId, r: Role) -> Result<(), KernelError> {
        if self.has_role(who, r) { Ok(()) } else { Err(KernelError::Unauthorized) }
    }
    fn tick(&mut self, now: Timestamp) -> Result<(), KernelError> {
        if now < self.clock { return Err(KernelError::TimeRegression); }
        self.clock = now;
        Ok(())
    }
    fn rec(&self, id: &TransitionId) -> Result<&Rec, KernelError> {
        self.transitions.get(id).ok_or(KernelError::UnknownTransition)
    }
    fn set_state(rec: &mut Rec, to: TState) -> Result<(), KernelError> {
        if !allowed(rec.state, to) { return Err(KernelError::BadState); }
        rec.state = to;
        Ok(())
    }

    // ---- governance (admin)
    pub fn grant_role(&mut self, caller: &AgentId, who: AgentId, role: Role) -> Result<(), KernelError> {
        self.need(caller, Role::Admin)?;
        self.roles.entry(who).or_default().insert(role_bit(role));
        Ok(())
    }
    pub fn register_agent(&mut self, caller: &AgentId, mut p: AgentProfile) -> Result<(), KernelError> {
        self.need(caller, Role::Admin)?;
        p.reputation = p.reputation.clamp_to(self.cfg.rep_min, self.cfg.rep_max);
        self.agents.insert(p.id, p);
        Ok(())
    }
    pub fn grant_capability(&mut self, caller: &AgentId, agent: AgentId, cap: CapabilityId) -> Result<(), KernelError> {
        self.need(caller, Role::Admin)?;
        if !self.agents.contains_key(&agent) { return Err(KernelError::UnknownAgent); }
        self.capabilities.entry(agent).or_default().insert(cap);
        Ok(())
    }
    pub fn require_capability(&mut self, caller: &AgentId, tt: TransitionType, cap: CapabilityId, min_weight: Amount) -> Result<(), KernelError> {
        self.need(caller, Role::Admin)?;
        self.required_caps.entry(tt).or_default().push((cap, min_weight));
        Ok(())
    }
    /// Capacity is a consumed stock: commit debits it.
    pub fn set_capacity(&mut self, caller: &AgentId, r: ResourceId, c: Amount) -> Result<(), KernelError> {
        self.need(caller, Role::Admin)?;
        self.capacity.insert(r, c);
        Ok(())
    }
    pub fn set_mandatory_constraints(&mut self, caller: &AgentId, tt: TransitionType, set: BTreeSet<ConstraintId>) -> Result<(), KernelError> {
        self.need(caller, Role::Admin)?;
        self.mandatory.insert(tt, set);
        Ok(())
    }

    // ---- observation (oracle)
    pub fn post_observation(&mut self, caller: &AgentId, o: Observation, now: Timestamp) -> Result<(), KernelError> {
        if !self.attestor.may_observe(&self.caller_roles(caller), caller, &o) { return Err(KernelError::Unauthorized); }
        self.tick(now)?;
        if o.timestamp > now { return Err(KernelError::ObservationFromFuture); }
        if o.expiry <= o.timestamp || o.confidence > Amount::ONE { return Err(KernelError::BadWindow); }
        if self.observations.contains_key(&o.id) { return Err(KernelError::Duplicate); }
        self.observations.insert(o.id, o);
        Ok(())
    }

    // ---- lifecycle
    /// Constraint set is frozen here: mandatory(type) ∪ proposer additions.
    pub fn register_transition(&mut self, caller: &AgentId, m: Manifest, pol: Policy, extra: BTreeSet<ConstraintId>, now: Timestamp) -> Result<(), KernelError> {
        self.tick(now)?;
        if self.transitions.contains_key(&m.id) { return Err(KernelError::Duplicate); }
        if !self.observations.contains_key(&m.observation_id) { return Err(KernelError::UnknownObservation); }
        if m.expires_at <= now || pol.max_rounds == 0 || pol.max_plans == 0 || pol.confidence_threshold > Amount::ONE {
            return Err(KernelError::BadWindow);
        }
        let mut set: BTreeSet<ConstraintId> = self.mandatory.get(&m.transition_type).cloned().unwrap_or_default();
        set.extend(extra);
        let constraints = set.into_iter().map(|c| (c, ConstraintStatus::Unavailable)).collect();
        let id = m.id;
        self.transitions.insert(id, Rec {
            proposer: *caller, m, pol, state: TState::Registered, constraints, objectives_ok: false,
            rounds: alloc::vec![Round::default()], plans: BTreeMap::new(), exceptions: BTreeMap::new(),
            unresolved_blocking: 0, planning_deadline: 0, reconciled: false, realized: None,
        });
        self.events.push(Event::Registered(id));
        Ok(())
    }

    pub fn report_constraint(&mut self, caller: &AgentId, id: &TransitionId, c: ConstraintId, s: ConstraintStatus, now: Timestamp) -> Result<(), KernelError> {
        self.need(caller, Role::Evaluator)?;
        self.tick(now)?;
        let rec = self.transitions.get_mut(id).ok_or(KernelError::UnknownTransition)?;
        if !matches!(rec.state, TState::Registered | TState::Planning) { return Err(KernelError::BadState); }
        *rec.constraints.get_mut(&c).ok_or(KernelError::UnknownConstraint)? = s;
        Ok(())
    }
    pub fn report_objectives(&mut self, caller: &AgentId, id: &TransitionId, ok: bool, now: Timestamp) -> Result<(), KernelError> {
        self.need(caller, Role::Evaluator)?;
        self.tick(now)?;
        let rec = self.transitions.get_mut(id).ok_or(KernelError::UnknownTransition)?;
        if !matches!(rec.state, TState::Registered | TState::Planning) { return Err(KernelError::BadState); }
        rec.objectives_ok = ok;
        Ok(())
    }

    fn eligible(&self, a: &AgentId, rec: &Rec) -> Result<&AgentProfile, KernelError> {
        let p = self.agents.get(a).ok_or(KernelError::UnknownAgent)?;
        if p.active && !p.groups.is_disjoint(&rec.m.relevant_groups) { Ok(p) } else { Err(KernelError::NotEligible) }
    }

    /// W_eff = raw · R · I · GPolicy, each term supplied by the `WeightPolicy` (default flat 1.0).
    pub fn submit_signal(&mut self, caller: &AgentId, id: &TransitionId, raw: Amount, support: Amount, now: Timestamp) -> Result<(), KernelError> {
        self.tick(now)?;
        if support > Amount::ONE { return Err(KernelError::SupportOutOfRange); }
        let rec = self.rec(id)?;
        if !matches!(rec.state, TState::Registered | TState::Planning) { return Err(KernelError::BadState); }
        let r = rec.rounds.len() - 1;
        if r as u32 >= rec.pol.max_rounds { return Err(KernelError::RoundsExhausted); }
        let prof = self.eligible(caller, rec)?;
        let (infl, rel, grp) = (self.weights.influence(prof), self.weights.relevance(prof, &rec.m), self.weights.group_policy(prof, &rec.m));
        if rec.rounds[r].signals.contains_key(caller) { return Err(KernelError::AlreadySignalled); }
        if r > 0 {
            let prev = &rec.rounds[r - 1];
            if let Some(ps) = prev.signals.get(caller) {
                let mean = prev.weighted.checked_mul_div(Amount::ONE, prev.total)?;
                let d = |x: Amount| x.raw().abs_diff(mean.raw());
                if d(support) > d(ps.support) { return Err(KernelError::MonotoneRevision); }
            }
        }
        let eff = raw.checked_mul(rel)?.checked_mul(infl)?.checked_mul(grp)?;
        let term = eff.checked_mul(support)?;
        let rec = self.transitions.get_mut(id).ok_or(KernelError::UnknownTransition)?;
        let round = &mut rec.rounds[r];
        round.total = round.total.checked_add(eff)?;
        round.weighted = round.weighted.checked_add(term)?;
        round.signals.insert(*caller, Signal { support });
        Ok(())
    }

    /// Open the next consensus round (revision happens here, never within a round).
    pub fn advance_round(&mut self, caller: &AgentId, id: &TransitionId, now: Timestamp) -> Result<(), KernelError> {
        self.tick(now)?;
        let rec = self.transitions.get_mut(id).ok_or(KernelError::UnknownTransition)?;
        if !matches!(rec.state, TState::Registered | TState::Planning) { return Err(KernelError::BadState); }
        let _ = caller;
        if rec.rounds.last().map_or(true, |r| r.total == Amount::ZERO) { return Err(KernelError::RoundEmpty); }
        if rec.rounds.len() as u32 >= rec.pol.max_rounds { return Err(KernelError::RoundsExhausted); }
        rec.rounds.push(Round::default());
        Ok(())
    }

    /// Starts the planning clock, so only the recorded proposer or an Admin may call it.
    pub fn open_planning(&mut self, caller: &AgentId, id: &TransitionId, now: Timestamp) -> Result<(), KernelError> {
        let is_admin = self.has_role(caller, Role::Admin);
        self.tick(now)?;
        let rec = self.transitions.get_mut(id).ok_or(KernelError::UnknownTransition)?;
        if rec.proposer != *caller && !is_admin { return Err(KernelError::Unauthorized); }
        Self::set_state(rec, TState::Planning)?;
        rec.planning_deadline = now.checked_add(rec.pol.planning_window).ok_or(KernelError::Overflow)?;
        Ok(())
    }

    pub fn submit_plan(&mut self, caller: &AgentId, plan: Plan, id: &TransitionId, now: Timestamp) -> Result<(), KernelError> {
        self.tick(now)?;
        let rec = self.rec(id)?;
        if rec.state != TState::Planning || now >= rec.planning_deadline { return Err(KernelError::PlanningClosed); }
        if plan.agent != *caller { return Err(KernelError::Unauthorized); }
        self.eligible(caller, rec)?;
        if rec.plans.contains_key(caller) { return Err(KernelError::Duplicate); }
        if rec.plans.len() as u32 >= rec.pol.max_plans { return Err(KernelError::TooManyPlans); }
        if plan.expires_at <= now { return Err(KernelError::BadWindow); }
        let rec = self.transitions.get_mut(id).ok_or(KernelError::UnknownTransition)?;
        rec.plans.insert(*caller, plan);
        Ok(())
    }

    pub fn raise_exception(&mut self, caller: &AgentId, id: &TransitionId, ex: ExceptionId, d: Disposition, now: Timestamp) -> Result<(), KernelError> {
        self.need(caller, Role::Evaluator)?;
        self.tick(now)?;
        let rec = self.transitions.get_mut(id).ok_or(KernelError::UnknownTransition)?;
        if !matches!(rec.state, TState::Registered | TState::Planning | TState::Committable) { return Err(KernelError::BadState); }
        if rec.exceptions.contains_key(&ex) { return Err(KernelError::Duplicate); }
        if d >= Disposition::Block { rec.unresolved_blocking += 1; }
        rec.exceptions.insert(ex, Exc { disposition: d, resolved: false });
        Ok(())
    }
    pub fn resolve_exception(&mut self, caller: &AgentId, id: &TransitionId, ex: ExceptionId, now: Timestamp) -> Result<(), KernelError> {
        self.need(caller, Role::Evaluator)?;
        self.tick(now)?;
        let rec = self.transitions.get_mut(id).ok_or(KernelError::UnknownTransition)?;
        let e = rec.exceptions.get_mut(&ex).ok_or(KernelError::UnknownException)?;
        if !e.resolved {
            e.resolved = true;
            if e.disposition >= Disposition::Block { rec.unresolved_blocking -= 1; }
        }
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

    /// Planning closed -> compute compatibility once, move to Committable.
    pub fn reconcile(&mut self, _caller: &AgentId, id: &TransitionId, now: Timestamp) -> Result<(), KernelError> {
        self.tick(now)?;
        let rec = self.rec(id)?;
        if rec.state != TState::Planning { return Err(KernelError::BadState); }
        if now < rec.planning_deadline { return Err(KernelError::PlanningStillOpen); }
        let ok = self.fits_capacity(rec);
        let rec = self.transitions.get_mut(id).ok_or(KernelError::UnknownTransition)?;
        rec.reconciled = ok;
        Self::set_state(rec, TState::Committable)
    }

    /// Evaluator/Admin veto from any pre-commit state. Terminal.
    pub fn reject(&mut self, caller: &AgentId, id: &TransitionId, now: Timestamp) -> Result<(), KernelError> {
        if !self.has_role(caller, Role::Evaluator) && !self.has_role(caller, Role::Admin) { return Err(KernelError::Unauthorized); }
        self.tick(now)?;
        let rec = self.transitions.get_mut(id).ok_or(KernelError::UnknownTransition)?;
        Self::set_state(rec, TState::Rejected)?;
        self.events.push(Event::Rejected(*id));
        Ok(())
    }

    /// Mark a transition Expired once its manifest expiry has passed and it never committed.
    pub fn expire(&mut self, id: &TransitionId, now: Timestamp) -> Result<(), KernelError> {
        self.tick(now)?;
        let rec = self.transitions.get_mut(id).ok_or(KernelError::UnknownTransition)?;
        if now < rec.m.expires_at { return Err(KernelError::BadWindow); }
        Self::set_state(rec, TState::Expired)?;
        self.events.push(Event::Expired(*id));
        Ok(())
    }

    // ---- the one predicate
    fn confidence_of(rec: &Rec) -> Amount {
        let r = rec.rounds.last().expect("at least one round");
        if r.total == Amount::ZERO { return Amount::ZERO; }
        r.weighted.checked_mul_div(Amount::ONE, r.total).unwrap_or(Amount::ZERO)
    }
    fn capability_covered(&self, rec: &Rec) -> bool {
        let Some(reqs) = self.required_caps.get(&rec.m.transition_type) else { return true };
        reqs.iter().all(|(cap, min)| {
            let mut acc = Amount::ZERO;
            for a in rec.plans.keys() {
                if self.capabilities.get(a).is_some_and(|s| s.contains(cap)) {
                    if let Some(p) = self.agents.get(a) {
                        match acc.checked_add(p.influence) { Ok(v) => acc = v, Err(_) => return false }
                    }
                }
            }
            acc >= *min
        })
    }
    /// Phase 7 hook (dialect registry). No dependencies are declarable yet, so always true.
    fn dialect_deps_ok(&self, _rec: &Rec) -> bool { true }

    pub fn commit_fail_mask(&self, id: &TransitionId, now: Timestamp) -> Result<FailMask, KernelError> {
        let rec = self.rec(id)?;
        let obs = self.observations.get(&rec.m.observation_id).ok_or(KernelError::UnknownObservation)?;
        let mut m = 0u32;
        // empty constraint set is NOT feasible; Unavailable is NOT Pass
        if rec.constraints.is_empty() || !rec.constraints.values().all(|s| *s == ConstraintStatus::Pass) { m |= FailMask::CONSTRAINTS; }
        if rec.pol.require_objectives && !rec.objectives_ok { m |= FailMask::OBJECTIVES; }
        let round = rec.rounds.last().expect("at least one round");
        if round.total < rec.pol.quorum_required { m |= FailMask::QUORUM; }
        if Self::confidence_of(rec) < rec.pol.confidence_threshold { m |= FailMask::CONFIDENCE; }
        if !(rec.reconciled && self.fits_capacity(rec)) { m |= FailMask::COMPAT; }
        if rec.unresolved_blocking != 0 { m |= FailMask::EXCEPTIONS; }
        if now < obs.timestamp || now >= obs.expiry || obs.confidence < rec.pol.min_obs_confidence { m |= FailMask::FRESH; }
        if now < rec.planning_deadline { m |= FailMask::PLANNING_OPEN; }
        if now >= rec.m.expires_at { m |= FailMask::EXPIRED; }
        if rec.m.parent_index != self.declared.index || rec.m.parent_hash != self.declared.hash { m |= FailMask::LINEAGE; }
        let min_exp = rec.plans.values().map(|p| p.expires_at).min();
        if min_exp.map_or(true, |e| now >= e) { m |= FailMask::PLANS; }
        if !self.capability_covered(rec) { m |= FailMask::CAPABILITY; }
        if !self.dialect_deps_ok(rec) { m |= FailMask::DIALECT; }
        Ok(FailMask(m))
    }

    /// Permissionless: the predicate decides, not the caller.
    pub fn commit(&mut self, id: &TransitionId, now: Timestamp) -> Result<SystemState, KernelError> {
        self.tick(now)?;
        if self.rec(id)?.state != TState::Committable { return Err(KernelError::BadState); }
        let mask = self.commit_fail_mask(id, now)?;
        if !mask.is_clear() { return Err(KernelError::CommitFailed(mask)); }
        // debit capacity (checked); everything below is infallible after this loop succeeds
        let mut debit: BTreeMap<ResourceId, Amount> = BTreeMap::new();
        for p in self.rec(id)?.plans.values() {
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
        let rec = self.transitions.get_mut(id).ok_or(KernelError::UnknownTransition)?;
        Self::set_state(rec, TState::Committed)?;
        self.capacity = next_cap;
        self.declared = SystemState { index, hash: rec.m.declared_next_hash, timestamp: now };
        self.history.push((index, *id, rec.m.declared_next_hash));
        self.events.push(Event::Committed { id: *id, index });
        Ok(self.declared.clone())
    }

    /// Record the realized hash and apply attribution. Per agent, deltas are summed
    /// (order-independent), capped at `±max_attribution`, then drift halves only positive credit.
    pub fn settle(&mut self, caller: &AgentId, id: &TransitionId, realized: Hash32, attributions: &[(AgentId, Delta)], now: Timestamp) -> Result<bool, KernelError> {
        if !self.attestor.may_settle(&self.caller_roles(caller), caller, id, &realized, attributions) { return Err(KernelError::Unauthorized); }
        self.tick(now)?;
        let rec = self.rec(id)?;
        if rec.state != TState::Committed { return Err(KernelError::BadState); }
        let mut net: BTreeMap<AgentId, i128> = BTreeMap::new();
        for (a, d) in attributions {
            if !self.agents.contains_key(a) { return Err(KernelError::UnknownAgent); }
            if !rec.plans.contains_key(a) { return Err(KernelError::NotParticipant); }
            *net.entry(*a).or_insert(0) += *d as i128;
        }
        let exact = realized == rec.m.declared_next_hash;
        let cap = self.cfg.max_attribution.raw() as i128;
        let (lo, hi) = (self.cfg.rep_min, self.cfg.rep_max);
        for (a, n) in net {
            let mut n = n.clamp(-cap, cap);
            if !exact && n > 0 { n /= 2; }
            let p = self.agents.get_mut(&a).ok_or(KernelError::UnknownAgent)?;
            p.reputation = p.reputation.apply_delta(n, lo, hi);
        }
        let rec = self.transitions.get_mut(id).ok_or(KernelError::UnknownTransition)?;
        Self::set_state(rec, TState::Settled)?;
        rec.realized = Some(realized);
        self.events.push(Event::Settled { id: *id, exact });
        Ok(exact)
    }
}
