//! treasury_program — the own treasury program, W4.3 scaffold (skeleton).
//!
//! Design: `files/OWN_IDL_DESIGN.md` (W4.1, validated and revised 2026-10-24;
//! W4.3 GO given by Carlos 2026-10-24 ~03:39 CST). Placement C: the kernel
//! crate compiles into the program (no_std SBF) and the full 13-bit
//! `commit_fail_mask` is computed on-chain; there is NO digest publisher —
//! the W3.3 mock's trust root does not survive into this program.
//!
//! # Scaffold status (this increment: kernel vendoring + predicate proof)
//!
//! One genesis instruction (`initialize_config`, design §4.1) and the
//! `Config` account (design §3) exist, and the pristine kernel crate is now
//! vendored in (`kernel_bridge`): placement C is wired, the launch policy
//! and config are named constants/functions, and host tests prove the full
//! lifecycle clears the 13-bit mask and commits — while an over-claim
//! fails COMPAT and commits nothing. **SBF status: GREEN (2026-10-24).**
//! D-12 closed this morning: the author's `map_or` revision landed
//! (`3ef5c412`), Carlos adopted Solana 2.2.20 and pushed the revert
//! (`f3086ac6`) restoring the original `is_none_or` text byte-for-byte,
//! and `anchor build` on platform-tools v1.48 / rustc 1.84.1 exits 0 with
//! the full predicate compiled in (see `treasury-program/README.md`).
//! The vendoring increment is PUSHED (remote `0df20ec7`, 2026-10-24 ~07:06 CST).
//! This increment adds the §4.1 admin-setup slice: `AgentRecord` plus
//! `register_agent` / `grant_role` / `revoke_role`. Admin setup comes
//! before the role-gated lifecycle instructions by necessity —
//! `post_observation` requires an Oracle, `submit_constraint_report` an
//! Evaluator, and no agent can hold a role until an AgentRecord exists.
//! Genesis-direct form only: the caller must be the `Config` authority.
//! After genesis, changes to what these calls control are executed as
//! committed `treasury.policy.v1` transitions (design §4.1), never by a
//! fresh authority signature alone — that execution path is a later
//! increment. This increment adds the first lifecycle instructions in
//! design order: `post_observation` (Oracle-only, Clock-judged freshness)
//! and `register_transition` (creates the TransitionRecord in Registered
//! state; the manifest's observation must already exist — the account
//! constraint enforces it; lineage is judged at commit, not here).
//! Constraint reports, plans, signals, and the remaining lifecycle
//! instructions land next, in design order, then §4.2 treasury, then
//! §4.3 distribution. This increment adds `open_planning` (design §4.1):
//! only the transition's recorded proposer or an Admin may open planning,
//! the state moves Registered → Planning, and the planning deadline is
//! `now + PLANNING_WINDOW` from the carried launch policy. Recording the
//! proposer required one honest change to the previous slice: the
//! TransitionRecord now carries the proposer's key (set from the payer at
//! `register_transition`), so its SPACE moves 74 → 106. No on-chain
//! account exists yet (no deployment in W4.3), so no migration is owed.
//! This increment adds `submit_plan` (design §4.1): Planning-only,
//! before the deadline, plan agent == caller, one plan per agent (the
//! PDA seeds enforce the duplicate rule — a second plan from the same
//! agent fails at account creation, no state changes), capacity
//! deliberately NOT checked here (the kernel checks it at
//! reconcile/commit, COMPAT — design §4.1: "there is no courtesy
//! check"). Plans live in standalone `PlanRecord` accounts (design §3
//! lists Plan separately from the TransitionRecord, unlike constraint
//! reports which live on it), and the TransitionRecord gains a
//! `plan_count` so the kernel's `TooManyPlans` rule (max 4 at launch)
//! is enforceable on-chain — SPACE 106 → 110. Honest gap, stated in
//! the handler docs: the kernel's eligibility rule also requires the
//! agent's groups to overlap the manifest's `relevant_groups`, but the
//! record does not carry `relevant_groups` yet (manifest completion
//! lands with its owning increment), so this slice enforces the
//! active half of eligibility and defers the group-overlap half — the
//! gap is documented, not papered over.
//! This increment adds `submit_signal` (design §4.1): Registered-or-
//! Planning state, support in range, eligible (active) agent, one signal
//! per agent per round (the SignalRecord PDA seeds enforce the duplicate
//! rule — a second signal from the same agent in the same round fails at
//! account creation). The kernel's monotone-revision rule is pure-helper
//! tested here but its on-chain wiring awaited `advance_round`: only
//! round 0 was reachable until rounds advanced, and the kernel's monotone
//! branch is vacuous in round 0 (it compares against a previous round
//! that does not exist yet). `advance_round` now snapshots the previous
//! round's weighted mean onto the TransitionRecord; the remaining
//! wiring — the agent's own previous-round support, read from that
//! round's SignalRecord — is a follow-up signal-side increment, and
//! until then commit replay enforces the check. The TransitionRecord
//! gains `round_count` (SPACE 110 → 114), starting at 1 — the kernel
//! starts every transition with one empty round.
//! This increment adds `submit_constraint_report` (design §4.1) and the
//! frozen constraint set it reports against: `register_transition` now
//! takes the frozen set (at most [`MAX_CONSTRAINTS`] distinct ids, each
//! starting Unavailable — the kernel freezes mandatory ∪ proposer
//! additions at registration), and an Evaluator reports Pass/Fail
//! against a member of that set while the transition is Registered or
//! Planning. A report against an id outside the frozen set is
//! `UnknownConstraint`, never a silent new constraint. Scaffold-form
//! note, stated plainly: the caller supplies the frozen set at
//! registration; when PolicyRecords land, registration will derive the
//! mandatory part from the transition type's policy and union the
//! proposer's additions, as the kernel does — the set's shape (frozen
//! at registration, reported against, never extended) does not change.
//! The TransitionRecord gains `constraints` (SPACE 114 → 382).
//! This increment adds `submit_objectives_check` (design §4.1): an
//! Evaluator reports the independent objectives verdict (a bool) while
//! the transition is Registered or Planning; the report overwrites the
//! TransitionRecord's `objectives_ok` flag (SPACE 382 → 383). The
//! OBJECTIVES bit at commit is `require_objectives && !objectives_ok` —
//! at launch the policy does not require objectives, so the bit is
//! clear regardless, but the flag is recorded now so a policy
//! transition that does require it is judged on a real report, never
//! on an assumed default.
//! This increment adds `advance_round` (design §4.1): permissionless,
//! like the kernel's (the kernel ignores the caller) — Registered or
//! Planning state, the current round non-empty, and room under
//! `max_rounds` (3 at launch), in that check order. The RoundEmpty
//! gate is faithful, not a signal-count proxy: `submit_signal` now
//! accumulates the current round's effective-weight total and weighted
//! support sum onto the TransitionRecord (FlatWeights: relevance and
//! group policy are 1.0, so effective weight is raw × the signer's
//! recorded influence, Amount arithmetic — the same computation the
//! kernel performs, from on-chain records, never a client total), and
//! a round whose total is zero — no signals, or only zero-weight
//! signals — cannot advance, exactly as the kernel's
//! `total == Amount::ZERO` test refuses. Advancing snapshots the
//! previous round's weighted mean onto the record
//! (`prev_round_mean`) and resets the current-round aggregates, so
//! the monotone-revision check can read its mean. The TransitionRecord
//! gains `current_round_total`, `current_round_weighted`, and
//! `prev_round_mean` (SPACE 383 → 407). One gap stays open, stated
//! plainly: the monotone check also needs the agent's own previous-round
//! support, which lives on that round's SignalRecord — `submit_signal`
//! does not yet take it as an account, so the check is enforced at
//! commit replay (the kernel's `submit_signal` during replay refuses
//! a violating sequence), not at signal time. That wiring is the next
//! signal-side increment, not this one.
//! Nothing here is deployed: NO devnet deployment in W4.3, NO mainnet step
//! exists in W4. No instruction here moves value.
//!
//! The program id in `declare_id!` is a scaffold placeholder generated for
//! local build / IDL generation only; its keypair was never committed and
//! no deployment was made from it. W4.4 deployment uses a fresh keypair.
//!
//! # Canonical numbers (Carlos's stated assumptions — labeled, never promises)
//!
//! * `FEE_PPM = 7_300`: the 60 bps transaction fee and the 13 bps IDL fee
//!   ride the SAME flow (D-5 CLOSED 2026-10-23 ~16:00 CST; engine-sim T14).
//! * `LOAN_RATE_PPM = 120_000`: 12%, final (Carlos, 2026-10-22).
//! * `PENALTY_FLOOR_PPM = 120_000`: 12% extraction-penalty floor — a
//!   deterrent, not a revenue line (D-4 curve shape above the floor is open).
//! * `CASH_SLEEVE_PPM = 250_000`: 25% cash sleeve (50/25/25 portfolio,
//!   Carlos's 2026-10-22 revision). The lending book's maximum draw is
//!   exactly 1/3 of the sleeve at 60-day terms (math doc §4), so the max
//!   outstanding at 100% uptake is T/12, and movable liquidity — the cash
//!   sleeve NET of that maximum draw — is T/6 (D-10, refined ~16:09 CST).
//! * `LOAN_TERM_DAYS = 60`: loans are 60-day only at pilot (design §4.2);
//!   90-day terms need a term-aware cap (T/8) and Carlos's ruling (§9).
//! * `MAX_LOAN_TO_POSITION_PPM = 500_000`: up to $50 per $100 position.
//!
//! # Claims boundary (design §8, committed-inventory ONLY)
//!
//! This program governs committed inventory only. Secondary-market transfers
//! of the native token outside the program are not bound by rotational/exit
//! rules. The Token-2022 transfer-hook extension is a designed future phase
//! needing its own Carlos decision + legal review — NOT W4.3 scope.

use anchor_lang::prelude::*;

declare_id!("FPGSeLBWEiFKdTuqDbJeKhjXeah5H4c3rqsW4Tzn3RAS");

/// Bridge to the pristine kernel crate (design §1.2, §4.1).
/// See `kernel_bridge.rs` for the launch profile, the evidence-check
/// placement, and the replay-at-commit state discipline.
pub mod kernel_bridge;

/// Parts-per-million denominator for all rate constants.
pub const PPM: u128 = 1_000_000;

/// Combined fee stream, single flow (D-5): 60 bps + 13 bps = 73 bps.
pub const FEE_PPM: u64 = 7_300;

/// Loan rate, 12% — Carlos, final 2026-10-22.
pub const LOAN_RATE_PPM: u64 = 120_000;

/// Extraction-penalty floor, 12% — deterrent, not a revenue line.
pub const PENALTY_FLOOR_PPM: u64 = 120_000;

/// Cash sleeve, 25% of T (50/25/25 portfolio).
pub const CASH_SLEEVE_PPM: u64 = 250_000;

/// Maximum loan per unit of position: $50 per $100.
pub const MAX_LOAN_TO_POSITION_PPM: u64 = 500_000;

/// Loan term at pilot, days. 60-day only (design §4.2).
pub const LOAN_TERM_DAYS: u64 = 60;

/// Stage flag values on [`Config`].
pub const STAGE_1: u8 = 1;
pub const STAGE_2: u8 = 2;

/// Movable liquidity at the reference sleeve: T/6, ratio-exact.
/// Movable = cash sleeve NET of the lending book's maximum draw
/// (1/3 of the sleeve at 60-day terms): 2/3 × 25% × T = T/6.
pub fn movable_liquidity(treasury: u128) -> u128 {
    treasury * (CASH_SLEEVE_PPM as u128) * 2 / PPM / 3
}

/// Maximum lending-book outstanding at 100% uptake: T/12.
pub fn max_outstanding(treasury: u128) -> u128 {
    treasury * (CASH_SLEEVE_PPM as u128) / PPM / 3
}

#[program]
pub mod treasury_program {
    use super::*;

    /// Genesis (design §4.1): one-time program configuration.
    ///
    /// This is the genesis exception to the "every state-changing
    /// instruction names its kernel transition" rule — no chain exists yet,
    /// so no kernel transition authorizes it. The `init` constraint makes
    /// it one-time: a second call fails and changes no state. Policy lives
    /// in `treasury.policy.v1` transitions, NOT in this account — `Config`
    /// holds only program parameters that are not policy (authority, fee
    /// vault) plus the stage flag (Stage 1 until a `treasury.launch.v1`
    /// transition settles, design §4.2).
    pub fn initialize_config(ctx: Context<InitializeConfig>, fee_vault: Pubkey) -> Result<()> {
        let config = &mut ctx.accounts.config;
        config.authority = ctx.accounts.authority.key();
        config.fee_vault = fee_vault;
        config.stage = STAGE_1;
        config.bump = ctx.bumps.config;
        Ok(())
    }

    /// Admin setup, genesis-direct form (design §4.1): register an agent.
    ///
    /// The caller must be the `Config` authority. The new record starts
    /// with no roles — roles arrive only via `grant_role` below, so no
    /// agent is born privileged. `influence_raw` / `reputation_raw` are
    /// the kernel's fixed-point raw values (SCALE 1_000_000); reputation
    /// is bounded 0–100 units, matching the launch config (kernel_bridge).
    /// Groups are the kernel's `GroupId` (u16) values, at most
    /// [`MAX_GROUPS`] — the account reserves exactly that many slots.
    pub fn register_agent(
        ctx: Context<RegisterAgent>,
        agent: Pubkey,
        groups: Vec<u16>,
        influence_raw: u64,
        reputation_raw: u64,
    ) -> Result<()> {
        require!(
            ctx.accounts.config.authority == ctx.accounts.authority.key(),
            TreasuryError::Unauthorized
        );
        require!(groups.len() <= MAX_GROUPS, TreasuryError::InvalidGroups);
        require!(
            reputation_raw <= REP_MAX_RAW,
            TreasuryError::InvalidReputation
        );
        let record = &mut ctx.accounts.agent_record;
        record.agent = agent;
        record.active = true;
        record.is_admin = false;
        record.is_oracle = false;
        record.is_evaluator = false;
        record.influence_raw = influence_raw;
        record.reputation_raw = reputation_raw;
        record.groups = groups;
        record.bump = ctx.bumps.agent_record;
        Ok(())
    }

    /// Admin setup, genesis-direct form (design §4.1): grant one role.
    ///
    /// Role codes mirror the kernel's `Role` enum order: 0 = Admin,
    /// 1 = Oracle, 2 = Evaluator. Any other code is an error and changes
    /// nothing. The caller must be the `Config` authority.
    pub fn grant_role(ctx: Context<RoleChange>, agent: Pubkey, role: u8) -> Result<()> {
        require!(
            ctx.accounts.config.authority == ctx.accounts.authority.key(),
            TreasuryError::Unauthorized
        );
        // The PDA seeds already bind this account to `agent`; the check
        // below states that binding explicitly so a future seeds change
        // cannot silently decouple the record from its key.
        require!(
            ctx.accounts.agent_record.agent == agent,
            TreasuryError::AgentMismatch
        );
        ctx.accounts.agent_record.apply_role(role, true)
    }

    /// Admin setup, genesis-direct form (design §4.1): revoke one role.
    ///
    /// Same role codes and authority rule as `grant_role`. Revoking a
    /// role the agent does not hold is a no-op success, not an error —
    /// the end state (role absent) is what the caller asked for.
    pub fn revoke_role(ctx: Context<RoleChange>, agent: Pubkey, role: u8) -> Result<()> {
        require!(
            ctx.accounts.config.authority == ctx.accounts.authority.key(),
            TreasuryError::Unauthorized
        );
        require!(
            ctx.accounts.agent_record.agent == agent,
            TreasuryError::AgentMismatch
        );
        ctx.accounts.agent_record.apply_role(role, false)
    }

    /// Governance lifecycle (design §4.1): post an observation.
    ///
    /// Feeds every transition: a transition's manifest names its
    /// observation, and the FRESH bit is judged through it at commit.
    /// Only an active Oracle may post (design §4.1: Oracle role, via the
    /// Attestor's `may_observe`). Freshness is judged against the Clock
    /// sysvar, never a client timestamp: a future-dated observation is
    /// rejected, and an expiry at or before the timestamp is a bad
    /// window. Confidence is recorded as recorded; the policy minimum
    /// (`min_obs_confidence`) is enforced at commit, not here. The
    /// `init` constraint makes the observation id one-time — a second
    /// post with the same id fails and changes no state (Duplicate in
    /// the design taxonomy).
    pub fn post_observation(
        ctx: Context<PostObservation>,
        observation_id: u64,
        timestamp: i64,
        expiry: i64,
        confidence: u64,
        content_hash: [u8; 32],
    ) -> Result<()> {
        let record = &ctx.accounts.agent_record;
        require!(
            record.agent == ctx.accounts.oracle.key(),
            TreasuryError::AgentMismatch
        );
        require!(
            record.active && record.is_oracle,
            TreasuryError::Unauthorized
        );
        let now = ctx.accounts.clock.unix_timestamp;
        validate_observation_window(timestamp, expiry, now)?;
        let observation = &mut ctx.accounts.observation;
        observation.observation_id = observation_id;
        observation.oracle = ctx.accounts.oracle.key();
        observation.timestamp = timestamp;
        observation.expiry = expiry;
        observation.confidence = confidence;
        observation.content_hash = content_hash;
        observation.bump = ctx.bumps.observation;
        Ok(())
    }

    /// Governance lifecycle (design §4.1): register a transition.
    ///
    /// Creates the TransitionRecord in Registered state and freezes the
    /// transition's manifest inputs (observation, expiry, declared hash).
    /// The manifest's observation must already exist: the account
    /// constraint on `observation` enforces that (UnknownObservation in
    /// the design taxonomy — enforced by account existence, not a
    /// returned error). The manifest expiry must be in the future
    /// (BadWindow otherwise). Lineage is deliberately NOT checked here:
    /// it is judged at commit against the head at commit time (design
    /// §1.1, bit 10). The frozen constraint set, plans, rounds, and
    /// signals are stored on this record by later lifecycle
    /// instructions; this slice creates the record with the planning
    /// deadline unset (0) until `open_planning` sets it, and
    /// `objectives_ok` false until `submit_objectives_check` reports.
    /// The payer's key
    /// is recorded as the proposer: the kernel records the proposer at
    /// registration (`kernel.rs` `register_transition`), and
    /// `open_planning` below admits only that proposer or an Admin.
    pub fn register_transition(
        ctx: Context<RegisterTransition>,
        transition_id: u64,
        observation_id: u64,
        expires_at: i64,
        declared_hash: [u8; 32],
        constraint_ids: Vec<[u8; 32]>,
    ) -> Result<()> {
        let now = ctx.accounts.clock.unix_timestamp;
        validate_transition_expiry(expires_at, now)?;
        validate_constraint_ids(&constraint_ids)?;
        let record = &mut ctx.accounts.transition_record;
        record.transition_id = transition_id;
        record.state = STATE_REGISTERED;
        record.proposer = ctx.accounts.payer.key();
        record.observation_id = observation_id;
        record.expires_at = expires_at;
        record.planning_deadline = 0;
        record.plan_count = 0;
        record.round_count = 1;
        record.current_round_total = 0;
        record.current_round_weighted = 0;
        record.prev_round_mean = 0;
        record.objectives_ok = false;
        record.constraints = constraint_ids
            .iter()
            .map(|id| ConstraintEntry {
                id: *id,
                status: STATUS_UNAVAILABLE,
            })
            .collect();
        record.declared_hash = declared_hash;
        record.bump = ctx.bumps.transition_record;
        Ok(())
    }

    /// Governance lifecycle (design §4.1): open planning for a transition.
    ///
    /// Mirrors the kernel's `open_planning` exactly: only the recorded
    /// proposer or an Admin may call it, the state must be Registered
    /// (the kernel's `set_state` allows Registered → Planning and
    /// nothing else into Planning), and the deadline is `now +
    /// planning_window` from the launch policy (the carried genesis
    /// parameter in `kernel_bridge`; design §9 — its seconds value is a
    /// parameter, not a ruling). Time comes from the Clock sysvar, never
    /// a client timestamp. Program-layer note, stated plainly: the
    /// caller must present their AgentRecord, because Admin status lives
    /// only there — a proposer who has never been registered as an agent
    /// cannot open planning until registered. That is stricter than the
    /// kernel (which records any caller as proposer), and strictly
    /// fail-closed: it can only refuse, never admit an extra caller.
    pub fn open_planning(
        ctx: Context<OpenPlanning>,
        transition_id: u64,
    ) -> Result<()> {
        let _ = transition_id;
        let record = &ctx.accounts.agent_record;
        require!(
            record.agent == ctx.accounts.caller.key(),
            TreasuryError::AgentMismatch
        );
        let transition = &mut ctx.accounts.transition_record;
        can_open_planning(
            ctx.accounts.caller.key(),
            transition.proposer,
            record.is_admin,
            transition.state,
        )?;
        let now = ctx.accounts.clock.unix_timestamp;
        transition.planning_deadline =
            planning_deadline(now, kernel_bridge::PLANNING_WINDOW)?;
        transition.state = STATE_PLANNING;
        Ok(())
    }

    /// Governance lifecycle (design §4.1): submit a plan for a transition.
    ///
    /// Mirrors the kernel's `submit_plan` in check order: Planning state
    /// and before the deadline (`PlanningClosed` otherwise — the kernel
    /// returns `PlanningClosed` for both a non-Planning state and a
    /// lapsed deadline), the agent active (`NotEligible`), room under
    /// `max_plans` from the launch policy (`TooManyPlans`), and a plan
    /// expiry in the future (`BadWindow`). The kernel's plan-agent ==
    /// caller rule is enforced structurally, not by a returned error:
    /// the `PlanRecord` PDA seeds bind the plan to the signer, and the
    /// AgentRecord must match the signer (`AgentMismatch` otherwise).
    /// The duplicate rule is enforced one layer down:
    /// the `PlanRecord` PDA seeds bind it to (transition, caller), so
    /// a second plan from the same agent fails at account creation and
    /// changes no state (Duplicate in the design taxonomy, enforced by
    /// account existence, like the observation duplicate before it).
    /// Capacity is NOT checked here — the kernel checks aggregate
    /// claims against stocks at reconcile/commit (COMPAT); there is no
    /// courtesy check, so a plan that will not fit is still recorded
    /// and still fails closed later. Claims are stored as a
    /// `Vec<Claim>` (at most [`MAX_CLAIMS`] — an
    /// account-size bound, not a kernel rule; the launch surface is 3
    /// stocks plus allocation classes, so 16 carries headroom) with
    /// distinct resource ids enforced (`InvalidClaims` otherwise — the
    /// kernel's claims are a keyed map, so a duplicate key would be a
    /// silent double-count at reconcile). The event carries the
    /// claims hash (SHA-256 over the little-endian
    /// resource/amount encoding, see [`claims_hash`]), not the claims
    /// themselves. Eligibility gap, stated plainly: the kernel's
    /// `eligible` also requires the agent's groups to overlap the
    /// manifest's `relevant_groups`, but the TransitionRecord does not
    /// carry `relevant_groups` yet — that field lands with the manifest
    /// completion, and the group-overlap check lands with it. This
    /// slice enforces the active half of eligibility; it does not claim
    /// the full kernel rule.
    pub fn submit_plan(
        ctx: Context<SubmitPlan>,
        transition_id: u64,
        claims: Vec<Claim>,
        expires_at: i64,
    ) -> Result<()> {
        let _ = transition_id;
        let caller_key = ctx.accounts.caller.key();
        let record = &ctx.accounts.agent_record;
        require!(
            record.agent == caller_key,
            TreasuryError::AgentMismatch
        );
        validate_claims(&claims)?;
        let now = ctx.accounts.clock.unix_timestamp;
        let transition = &mut ctx.accounts.transition_record;
        can_submit_plan(
            record.active,
            transition.state,
            now,
            transition.planning_deadline,
            transition.plan_count,
            kernel_bridge::launch_policy().max_plans,
            expires_at,
        )?;
        let hash = claims_hash(&claims);
        let plan = &mut ctx.accounts.plan_record;
        plan.transition_id = transition.transition_id;
        plan.agent = caller_key;
        plan.expires_at = expires_at;
        plan.claims = claims;
        plan.claims_hash = hash;
        plan.bump = ctx.bumps.plan_record;
        transition.plan_count = transition
            .plan_count
            .checked_add(1)
            .ok_or(TreasuryError::Overflow)?;
        emit!(PlanSubmitted {
            transition_id: transition.transition_id,
            agent: caller_key,
            claims_hash: hash,
        });
        Ok(())
    }

    /// Governance lifecycle (design §4.1): submit a signal for a transition.
    ///
    /// Mirrors the kernel's `submit_signal` in check order: support in
    /// range (`SupportOutOfRange` — support is a kernel `Amount`, so
    /// 1_000_000 raw is 1.0 and anything above it is out of range),
    /// Registered-or-Planning state (`BadState` otherwise — signals are
    /// accepted in both states, exactly as the kernel allows), the
    /// round index under `max_rounds` from the launch policy
    /// (`RoundsExhausted`), the round matching the transition's
    /// current round (`BadState` — a stale round index names a PDA
    /// that is not the live round), and the agent active (`NotEligible`;
    /// the group-overlap half of kernel eligibility awaits
    /// `relevant_groups` on the record, the same carried gap as plans).
    /// The signal agent == caller rule is structural: the
    /// `SignalRecord` PDA seeds bind the signal to the signer, and the
    /// AgentRecord must match the signer (`AgentMismatch` otherwise).
    /// The duplicate rule is structural too: the seeds bind one signal
    /// per (transition, round, agent), so a second signal in the same
    /// round fails at account creation and changes no state
    /// (`AlreadySignalled` in the design taxonomy, enforced by account
    /// existence, like the plan duplicate before it). The raw weight is
    /// stored as supplied; the effective weight (raw × relevance ×
    /// influence × group policy, FlatWeights at launch) is computed
    /// when the recorded signals are replayed through the kernel at
    /// commit — the program never trusts a client-computed total.
    /// Monotone revision, stated plainly: the kernel refuses a
    /// re-signal that moves farther from the previous round's weighted
    /// mean than the agent's previous signal did. The mean half of
    /// that check now exists on-chain: `advance_round` snapshots the
    /// previous round's weighted mean onto the TransitionRecord
    /// (`prev_round_mean`). The other half — the agent's own
    /// previous-round support, which lives on that round's
    /// SignalRecord — is not yet wired: this instruction does not take
    /// the previous SignalRecord as an account, so the check is
    /// enforced at commit replay (the kernel's `submit_signal` during
    /// replay refuses a violating sequence), not at signal time. The
    /// pure check is written and tested (see
    /// [`check_monotone_revision`]); its signal-time wiring is the
    /// follow-up signal-side increment. In round 0 the kernel's
    /// monotone branch is vacuous, as it has no previous round.
    pub fn submit_signal(
        ctx: Context<SubmitSignal>,
        transition_id: u64,
        round: u32,
        raw: u64,
        support: u64,
    ) -> Result<()> {
        let _ = transition_id;
        let caller_key = ctx.accounts.caller.key();
        let record = &ctx.accounts.agent_record;
        require!(
            record.agent == caller_key,
            TreasuryError::AgentMismatch
        );
        let transition = &mut ctx.accounts.transition_record;
        can_submit_signal(
            record.active,
            transition.state,
            support,
            round,
            kernel_bridge::launch_policy().max_rounds,
            transition.round_count.checked_sub(1).unwrap_or(0),
        )?;
        // Round aggregates, maintained on-chain so `advance_round`
        // can judge RoundEmpty the way the kernel does (total == ZERO).
        // FlatWeights: relevance and group policy are 1.0, so the
        // effective weight is raw x the signer's recorded influence —
        // computed here from on-chain records, never trusted from the
        // client, and replayed identically at commit.
        let (eff, term) = signal_aggregates(raw, support, record.influence_raw)?;
        transition.current_round_total = transition
            .current_round_total
            .checked_add(eff)
            .ok_or(TreasuryError::Overflow)?;
        transition.current_round_weighted = transition
            .current_round_weighted
            .checked_add(term)
            .ok_or(TreasuryError::Overflow)?;
        let signal = &mut ctx.accounts.signal_record;
        signal.transition_id = transition.transition_id;
        signal.round = round;
        signal.agent = caller_key;
        signal.raw = raw;
        signal.support = support;
        signal.bump = ctx.bumps.signal_record;
        emit!(SignalSubmitted {
            transition_id: transition.transition_id,
            agent: caller_key,
            raw,
            support,
        });
        Ok(())
    }

    /// Governance lifecycle (design §4.1): submit a constraint report.
    ///
    /// Mirrors the kernel's `report_constraint` in check order:
    /// Evaluator role first (`Unauthorized` otherwise — the kernel's
    /// `need(caller, Role::Evaluator)`), then Registered-or-Planning
    /// state (`BadState` otherwise), then membership in the frozen set
    /// (`UnknownConstraint` — a report cannot invent a constraint the
    /// transition was not frozen with). The report overwrites the
    /// entry's status: an Evaluator may correct a report, and the
    /// CONSTRAINTS bit at commit judges the final set (every entry
    /// Pass, and the set non-empty — an empty frozen set can never
    /// commit, exactly as the kernel's mask computes it). The caller
    /// binding is structural, as with plans and signals: the
    /// AgentRecord must match the signer (`AgentMismatch` otherwise).
    /// No new account is created: reports live on the TransitionRecord
    /// as the frozen set (design §3), not as standalone accounts —
    /// so a report is one account write, and commit replay reads the
    /// set from the same record it replays everything else from.
    pub fn submit_constraint_report(
        ctx: Context<SubmitConstraintReport>,
        transition_id: u64,
        constraint_id: [u8; 32],
        status: u8,
    ) -> Result<()> {
        let _ = transition_id;
        let caller_key = ctx.accounts.caller.key();
        let record = &ctx.accounts.agent_record;
        require!(
            record.agent == caller_key,
            TreasuryError::AgentMismatch
        );
        let transition = &mut ctx.accounts.transition_record;
        can_submit_constraint_report(
            record.is_evaluator,
            transition.state,
            transition.constraints.iter().any(|e| e.id == constraint_id),
            status,
        )?;
        set_constraint_status(&mut transition.constraints, constraint_id, status)?;
        emit!(ConstraintReported {
            transition_id: transition.transition_id,
            evaluator: caller_key,
            constraint_id,
            status,
        });
        Ok(())
    }

    /// Governance lifecycle (design §4.1): submit an objectives check.
    ///
    /// Mirrors the kernel's `report_objectives` in check order:
    /// Evaluator role first (`Unauthorized` otherwise — the kernel's
    /// `need(caller, Role::Evaluator)`), then Registered-or-Planning
    /// state (`BadState` otherwise). The report is an independent
    /// Evaluator verdict — a bool on the TransitionRecord, never
    /// derived from plan fields (design §1.1, bit 2) — and it
    /// overwrites: a later report replaces the earlier verdict, and
    /// the OBJECTIVES bit at commit judges the final flag
    /// (`require_objectives && !objectives_ok`). At launch the policy
    /// does not require objectives, so the bit is clear with or
    /// without a report; recording the flag now is what keeps a future
    /// requiring policy honest. The caller binding is structural, as
    /// with constraint reports: the AgentRecord must match the signer
    /// (`AgentMismatch` otherwise). No new account is created: the flag
    /// lives on the TransitionRecord (design §3), so a check is one
    /// account write.
    pub fn submit_objectives_check(
        ctx: Context<SubmitObjectivesCheck>,
        transition_id: u64,
        ok: bool,
    ) -> Result<()> {
        let _ = transition_id;
        let caller_key = ctx.accounts.caller.key();
        let record = &ctx.accounts.agent_record;
        require!(
            record.agent == caller_key,
            TreasuryError::AgentMismatch
        );
        let transition = &mut ctx.accounts.transition_record;
        can_submit_objectives_check(record.is_evaluator, transition.state)?;
        transition.objectives_ok = ok;
        emit!(ObjectivesReported {
            transition_id: transition.transition_id,
            evaluator: caller_key,
            ok,
        });
        Ok(())
    }

    /// Governance lifecycle (design §4.1): advance to the next round.
    ///
    /// Mirrors the kernel's `advance_round` in check order: the
    /// transition must be Registered or Planning (`BadState`
    /// otherwise), the current round must be non-empty — its
    /// effective-weight total, maintained by `submit_signal`, must be
    /// non-zero (`RoundEmpty` otherwise; a round of only zero-weight
    /// signals is empty in the kernel's sense, and this gate refuses
    /// it exactly as the kernel does) — and room must remain under
    /// `max_rounds` from the launch policy (`RoundsExhausted`
    /// otherwise; the empty check precedes the exhaustion check, as
    /// in the kernel). The caller is permissionless, as in the kernel
    /// (the kernel ignores its caller): no AgentRecord is required,
    /// and revision happens between rounds, never within one. On
    /// success the previous round's weighted mean is snapshotted onto
    /// the record (`prev_round_mean`, for the monotone-revision check),
    /// the current-round aggregates reset to zero, and the round count
    /// increments — the new current round is `round_count - 1`.
    pub fn advance_round(
        ctx: Context<AdvanceRound>,
        transition_id: u64,
    ) -> Result<()> {
        let _ = transition_id;
        let transition = &mut ctx.accounts.transition_record;
        can_advance_round(
            transition.state,
            transition.current_round_total,
            transition.round_count,
            kernel_bridge::launch_policy().max_rounds,
        )?;
        transition.prev_round_mean =
            round_mean(transition.current_round_weighted, transition.current_round_total)?;
        transition.current_round_total = 0;
        transition.current_round_weighted = 0;
        transition.round_count = transition
            .round_count
            .checked_add(1)
            .ok_or(TreasuryError::Overflow)?;
        emit!(RoundAdvanced {
            transition_id: transition.transition_id,
            round: transition.round_count - 1,
        });
        Ok(())
    }
}

/// Who may open planning, as a pure check (design §4.1, kernel
/// `open_planning`): the recorded proposer, or any Admin. Any other
/// caller is unauthorized; a transition not in Registered state cannot
/// enter Planning (the kernel's `set_state` gate, mirrored here).
pub fn can_open_planning(
    caller: Pubkey,
    proposer: Pubkey,
    is_admin: bool,
    state: u8,
) -> Result<()> {
    require!(state == STATE_REGISTERED, TreasuryError::BadState);
    require!(
        caller == proposer || is_admin,
        TreasuryError::Unauthorized
    );
    Ok(())
}

/// The planning deadline for a planning window opened at `now`:
/// `now + planning_window`, checked — an overflow is an error, never a
/// wrapped deadline.
pub fn planning_deadline(now: i64, planning_window: u64) -> Result<i64> {
    now.checked_add(planning_window as i64)
        .ok_or(TreasuryError::Overflow.into())
}

/// Who may submit a plan, as a pure check (design §4.1, kernel
/// `submit_plan`, in the kernel's check order minus the caller
/// binding): the transition must be Planning and `now` must precede
/// the deadline (both failures are `PlanningClosed` in the kernel);
/// the agent must be active (the active half of the kernel's
/// eligibility — the group-overlap half awaits `relevant_groups` on
/// the record, see the handler docs); room must remain under
/// `max_plans`; the plan's expiry must be in the future. The
/// plan-agent == caller rule is NOT re-checked here: it is structural
/// — the `PlanRecord` PDA seeds bind the plan to the signer, and the
/// handler requires the AgentRecord to match the signer
/// (`AgentMismatch` otherwise) — so a seven-argument helper stays
/// within clippy's argument-count lint without duplicating a check
/// the account constraints already enforce.
pub fn can_submit_plan(
    agent_active: bool,
    state: u8,
    now: i64,
    planning_deadline: i64,
    plan_count: u32,
    max_plans: u32,
    plan_expires_at: i64,
) -> Result<()> {
    require!(
        state == STATE_PLANNING && now < planning_deadline,
        TreasuryError::PlanningClosed
    );
    require!(agent_active, TreasuryError::NotEligible);
    require!(plan_count < max_plans, TreasuryError::TooManyPlans);
    require!(plan_expires_at > now, TreasuryError::BadWindow);
    Ok(())
}

/// Support ceiling, kernel `Amount::ONE` in raw units (SCALE 1_000_000):
/// a signal's support is a fraction of the round's weight, so anything
/// above 1.0 is out of range by construction.
pub const SUPPORT_ONE_RAW: u64 = 1_000_000;

/// Who may submit a signal, as a pure check (design §4.1, kernel
/// `submit_signal`, in the kernel's check order minus the caller
/// binding and the duplicate rule, both structural — PDA seeds plus the
/// AgentRecord match, and one SignalRecord per (transition, round,
/// agent)): support in range first, then Registered-or-Planning state,
/// then the round index under `max_rounds` and equal to the
/// transition's current round (a stale round is a bad state, not a
/// signal for a round that is not live), then the agent active (the
/// active half of kernel eligibility — the group-overlap half awaits
/// `relevant_groups`, as with plans).
pub fn can_submit_signal(
    agent_active: bool,
    state: u8,
    support: u64,
    round: u32,
    max_rounds: u32,
    current_round: u32,
) -> Result<()> {
    require!(
        support <= SUPPORT_ONE_RAW,
        TreasuryError::SupportOutOfRange
    );
    require!(
        state == STATE_REGISTERED || state == STATE_PLANNING,
        TreasuryError::BadState
    );
    require!(round < max_rounds, TreasuryError::RoundsExhausted);
    require!(round == current_round, TreasuryError::BadState);
    require!(agent_active, TreasuryError::NotEligible);
    Ok(())
}

/// The kernel's monotone-revision rule for a re-signal (kernel
/// `submit_signal`): let `mean` be the previous round's weighted mean
/// support and `prev_support` the agent's previous-round signal. The
/// new support is admissible only if it is no farther from the mean
/// than the previous signal was: `|support - mean| <=
/// |prev_support - mean|`. A revision that moves away from the mean
/// (revision happens between rounds, never by jumping farther away)
/// is refused. Pure and tested now; the on-chain wiring lands with
/// `advance_round`, which snapshots the previous round's mean — until
/// then only round 0 is reachable and this branch is vacuous.
pub fn check_monotone_revision(
    prev_support: u64,
    prev_mean: u64,
    support: u64,
) -> Result<()> {
    require!(
        support.abs_diff(prev_mean) <= prev_support.abs_diff(prev_mean),
        TreasuryError::MonotoneRevision
    );
    Ok(())
}

/// Constraint status codes, in the kernel's `ConstraintStatus` enum
/// order (kernel.rs): Pass, Fail, Unavailable. A fresh frozen entry is
/// Unavailable; only an all-Pass, non-empty set clears the CONSTRAINTS
/// bit at commit.
pub const STATUS_PASS: u8 = 0;
pub const STATUS_FAIL: u8 = 1;
pub const STATUS_UNAVAILABLE: u8 = 2;

/// Who may submit a constraint report, as a pure check (design §4.1,
/// kernel `report_constraint`, in the kernel's check order minus the
/// caller binding, which is structural — PDA seeds plus the
/// AgentRecord match): the caller must hold the Evaluator role, the
/// transition must be Registered or Planning, the constraint must be a
/// member of the frozen set, and the status code must be a real status.
/// An unknown constraint is refused here, not at commit: the frozen set
/// is fixed at registration, so a report outside it has no entry to
/// write and must not be recorded anywhere.
pub fn can_submit_constraint_report(
    is_evaluator: bool,
    state: u8,
    constraint_known: bool,
    status: u8,
) -> Result<()> {
    require!(is_evaluator, TreasuryError::Unauthorized);
    require!(
        state == STATE_REGISTERED || state == STATE_PLANNING,
        TreasuryError::BadState
    );
    require!(
        constraint_known,
        TreasuryError::UnknownConstraint
    );
    require!(
        status <= STATUS_UNAVAILABLE,
        TreasuryError::InvalidConstraintStatus
    );
    Ok(())
}

/// Who may submit an objectives check, as a pure check (design §4.1,
/// kernel `report_objectives`, in the kernel's check order minus the
/// caller binding, which is structural — PDA seeds plus the
/// AgentRecord match): the caller must hold the Evaluator role, and
/// the transition must be Registered or Planning. The verdict itself
/// is a bool and needs no validation — any bool is a real verdict, and
/// a later report overwrites it, so the OBJECTIVES bit judges the
/// final flag at commit.
pub fn can_submit_objectives_check(is_evaluator: bool, state: u8) -> Result<()> {
    require!(is_evaluator, TreasuryError::Unauthorized);
    require!(
        state == STATE_REGISTERED || state == STATE_PLANNING,
        TreasuryError::BadState
    );
    Ok(())
}

/// Who may advance a round, as a pure check (design §4.1, kernel
/// `advance_round`, in the kernel's check order): the transition must
/// be Registered or Planning, the current round must be non-empty
/// (its effective-weight total non-zero — the kernel's
/// `total == Amount::ZERO` test, not a signal count), and room must
/// remain under `max_rounds`. The empty check precedes the exhaustion
/// check, as in the kernel: an empty final round reports RoundEmpty,
/// not RoundsExhausted. The caller is permissionless (the kernel
/// ignores its caller), so no caller argument appears here.
pub fn can_advance_round(
    state: u8,
    current_round_total: u64,
    round_count: u32,
    max_rounds: u32,
) -> Result<()> {
    require!(
        state == STATE_REGISTERED || state == STATE_PLANNING,
        TreasuryError::BadState
    );
    require!(current_round_total != 0, TreasuryError::RoundEmpty);
    require!(round_count < max_rounds, TreasuryError::RoundsExhausted);
    Ok(())
}

/// A round's weighted mean support, as the kernel computes it at
/// `submit_signal` time for the previous round: `weighted * ONE /
/// total`, in u128, floored — the kernel's
/// `checked_mul_div(Amount::ONE, total)`. A zero total has no mean
/// and is an overflow error; `advance_round` gates on non-zero total
/// before calling this, so the error is unreachable through the
/// handler and exists only to keep the helper total.
pub fn round_mean(weighted: u64, total: u64) -> Result<u64> {
    if total == 0 {
        return Err(TreasuryError::Overflow.into());
    }
    let mean = (weighted as u128) * (SUPPORT_ONE_RAW as u128) / (total as u128);
    u64::try_from(mean).map_err(|_| TreasuryError::Overflow.into())
}

/// One signal's contribution to its round's aggregates, in kernel
/// `Amount` arithmetic (SCALE 1_000_000, u128 intermediates, floored).
/// Under FlatWeights, relevance and group policy are 1.0, so the
/// effective weight is raw x influence / SCALE, and the weighted term
/// is effective weight x support / SCALE — exactly the kernel's
/// `submit_signal` computation (`eff` then `term`). An intermediate
/// that does not fit u64 is an overflow, never a wrap, matching the
/// kernel's checked `Amount` operations.
pub fn signal_aggregates(raw: u64, support: u64, influence_raw: u64) -> Result<(u64, u64)> {
    let eff = (raw as u128) * (influence_raw as u128) / (SUPPORT_ONE_RAW as u128);
    let term = eff * (support as u128) / (SUPPORT_ONE_RAW as u128);
    let eff = u64::try_from(eff).map_err(|_| TreasuryError::Overflow)?;
    let term = u64::try_from(term).map_err(|_| TreasuryError::Overflow)?;
    Ok((eff, term))
}

/// Write one report into a frozen set: the entry must exist (an unknown
/// id is an error, never an append — the set is frozen), and the status
/// must be a real status code. Re-reporting overwrites: the CONSTRAINTS
/// bit judges the final set, so a corrected report replaces, not
/// appends.
pub fn set_constraint_status(
    constraints: &mut [ConstraintEntry],
    constraint_id: [u8; 32],
    status: u8,
) -> Result<()> {
    require!(
        status <= STATUS_UNAVAILABLE,
        TreasuryError::InvalidConstraintStatus
    );
    let entry = constraints
        .iter_mut()
        .find(|e| e.id == constraint_id)
        .ok_or(TreasuryError::UnknownConstraint)?;
    entry.status = status;
    Ok(())
}

/// A frozen constraint set must fit the account and the kernel's set
/// shape: at most [`MAX_CONSTRAINTS`] entries, no id twice (a duplicate
/// id would be one constraint with two statuses at commit replay).
pub fn validate_constraint_ids(ids: &[[u8; 32]]) -> Result<()> {
    require!(ids.len() <= MAX_CONSTRAINTS, TreasuryError::InvalidConstraints);
    for (i, id) in ids.iter().enumerate() {
        for other in &ids[..i] {
            require!(id != other, TreasuryError::InvalidConstraints);
        }
    }
    Ok(())
}

/// Event for [`treasury_program::submit_constraint_report`] (design
/// §4.1): the transition, the reporting Evaluator, and the report as
/// submitted — the authorization context the design requires on every
/// event. The CONSTRAINTS bit is judged at commit over the final set,
/// never from this event alone.
#[event]
pub struct ConstraintReported {
    pub transition_id: u64,
    pub evaluator: Pubkey,
    pub constraint_id: [u8; 32],
    pub status: u8,
}

/// Event for [`treasury_program::submit_objectives_check`] (design
/// §4.1): the transition, the reporting Evaluator, and the verdict as
/// submitted — the authorization context the design requires on every
/// event. The OBJECTIVES bit is judged at commit over the final flag
/// (and only when the transition type's policy requires objectives),
/// never from this event alone.
#[event]
pub struct ObjectivesReported {
    pub transition_id: u64,
    pub evaluator: Pubkey,
    pub ok: bool,
}

/// Event for [`treasury_program::advance_round`] (design §4.1, §6):
/// the transition and the new current round index — the round the
/// next signal belongs to. The previous round's weighted mean, which
/// the monotone-revision check reads, is snapshotted on the
/// TransitionRecord by the same instruction, not carried in the event.
#[event]
pub struct RoundAdvanced {
    pub transition_id: u64,
    pub round: u32,
}

/// Event for [`treasury_program::submit_signal`] (design §4.1): the
/// transition, the signalling agent, and the signal as submitted — the
/// authorization context the design requires on every event. The
/// effective weight is computed at commit replay, never trusted from
/// the client, so it is not in the event.
#[event]
pub struct SignalSubmitted {
    pub transition_id: u64,
    pub agent: Pubkey,
    pub raw: u64,
    pub support: u64,
}

/// A plan's claims must fit the account and the kernel's keyed-map
/// shape: at most [`MAX_CLAIMS`] entries, no resource id twice (a
/// duplicate key would double-count at reconcile).
pub fn validate_claims(claims: &[Claim]) -> Result<()> {
    require!(claims.len() <= MAX_CLAIMS, TreasuryError::InvalidClaims);
    for (i, claim) in claims.iter().enumerate() {
        for other in &claims[..i] {
            require!(
                claim.resource_id != other.resource_id,
                TreasuryError::InvalidClaims
            );
        }
    }
    Ok(())
}

/// The claims hash carried by the `PlanSubmitted` event: SHA-256 over
/// the concatenated little-endian (resource_id, amount) pairs, in the
/// order submitted. The encoding is the commitment — reconcile and
/// commit re-derive it from the stored claims, so a stored plan whose
/// hash does not match its claims is evidence of tampering, not a
/// second copy of the data.
pub fn claims_hash(claims: &[Claim]) -> [u8; 32] {
    let mut bytes = Vec::new();
    for claim in claims {
        bytes.extend_from_slice(&claim.resource_id.to_le_bytes());
        bytes.extend_from_slice(&claim.amount.to_le_bytes());
    }
    anchor_lang::solana_program::hash::hash(&bytes).to_bytes()
}

/// Event for [`treasury_program::submit_plan`] (design §4.1): the
/// transition, the submitting agent, and the claims hash — the
/// authorization context the design requires on every event.
#[event]
pub struct PlanSubmitted {
    pub transition_id: u64,
    pub agent: Pubkey,
    pub claims_hash: [u8; 32],
}

/// TransitionRecord states (design §3): the kernel's seven, exactly, in
/// the kernel's order — Registered, Planning, Committable, Committed,
/// Settled, Rejected, Expired.
pub const STATE_REGISTERED: u8 = 0;
pub const STATE_PLANNING: u8 = 1;
pub const STATE_COMMITTABLE: u8 = 2;
pub const STATE_COMMITTED: u8 = 3;
pub const STATE_SETTLED: u8 = 4;
pub const STATE_REJECTED: u8 = 5;
pub const STATE_EXPIRED: u8 = 6;

/// Validate an observation's window against on-chain time (design §4.1).
///
/// Future-dated observations are verification theater, not data: the
/// timestamp must not exceed `now`. An expiry at or before the
/// timestamp is not a window at all.
pub fn validate_observation_window(timestamp: i64, expiry: i64, now: i64) -> Result<()> {
    require!(
        timestamp <= now,
        TreasuryError::ObservationFromFuture
    );
    require!(expiry > timestamp, TreasuryError::BadWindow);
    Ok(())
}

/// Validate a transition manifest's expiry against on-chain time
/// (design §4.1): a manifest already expired at registration is a bad
/// window and creates nothing.
pub fn validate_transition_expiry(expires_at: i64, now: i64) -> Result<()> {
    require!(expires_at > now, TreasuryError::BadWindow);
    Ok(())
}

/// Genesis accounts for [`treasury_program::initialize_config`].
#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    /// Program configuration PDA, seeds `["config"]`. One-time init.
    #[account(
        init,
        payer = authority,
        space = Config::SPACE,
        seeds = [b"config"],
        bump
    )]
    pub config: Account<'info, Config>,
    /// Genesis authority (the governance multisig at genesis). Pays rent.
    #[account(mut)]
    pub authority: Signer<'info>,
    /// Rent sysvar, required by the `init` constraint.
    pub system_program: Program<'info, System>,
}

/// Program parameters that are NOT policy (design §3).
///
/// Policy (quorum, confidence thresholds, planning windows, mandatory
/// constraints, required capabilities) lives in PolicyRecords and is amended
/// only via `treasury.policy.v1` transitions — never by editing this account.
#[account]
pub struct Config {
    /// Genesis authority (governance multisig).
    pub authority: Pubkey,
    /// Collection account for the 73 bps single fee flow (D-5).
    pub fee_vault: Pubkey,
    /// Stage flag: [`STAGE_1`] until `treasury.launch.v1` settles.
    pub stage: u8,
    /// PDA bump for the `["config"]` seed.
    pub bump: u8,
}

impl Config {
    /// 8 (discriminator) + 32 + 32 + 1 + 1.
    pub const SPACE: usize = 8 + 32 + 32 + 1 + 1;
}

/// Maximum groups per agent (design §3: groups are the kernel's u16
/// `GroupId`). The account reserves exactly this many `Vec` slots so
/// `SPACE` is a fixed upper bound, not a function of the caller's input.
pub const MAX_GROUPS: usize = 8;

/// Reputation ceiling in kernel fixed-point raw units: 100 units at
/// SCALE 1_000_000, matching the launch config (kernel_bridge).
pub const REP_MAX_RAW: u64 = 100_000_000;

/// Genesis accounts for [`treasury_program::register_agent`].
#[derive(Accounts)]
#[instruction(agent: Pubkey)]
pub struct RegisterAgent<'info> {
    /// Program configuration PDA — read to check the authority.
    #[account(seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, Config>,
    /// Agent record PDA, seeds `["agent", agent]`. One-time init.
    #[account(
        init,
        payer = authority,
        space = AgentRecord::SPACE,
        seeds = [b"agent", agent.as_ref()],
        bump
    )]
    pub agent_record: Account<'info, AgentRecord>,
    /// Genesis authority (the governance multisig at genesis). Pays rent.
    #[account(mut)]
    pub authority: Signer<'info>,
    /// Rent sysvar, required by the `init` constraint.
    pub system_program: Program<'info, System>,
}

/// Accounts for [`treasury_program::grant_role`] and
/// [`treasury_program::revoke_role`].
#[derive(Accounts)]
#[instruction(agent: Pubkey)]
pub struct RoleChange<'info> {
    /// Program configuration PDA — read to check the authority.
    #[account(seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, Config>,
    /// The agent's existing record (must already exist).
    #[account(mut, seeds = [b"agent", agent.as_ref()], bump = agent_record.bump)]
    pub agent_record: Account<'info, AgentRecord>,
    /// Genesis authority (the governance multisig at genesis).
    pub authority: Signer<'info>,
}

/// One registered agent (design §3 `AgentRecord`).
///
/// Mirrors the kernel's `AgentProfile` plus the role set the kernel
/// keeps separately: the kernel's three roles (Admin, Oracle, Evaluator)
/// are the only roles — "Operator" is not a role, it is an agent class
/// holding capabilities (design §5). Influence and reputation are the
/// kernel's fixed-point raw values (SCALE 1_000_000); under FlatWeights
/// at launch, reputation is recorded and influence on quorum is zero
/// by policy, not by this account's contents.
#[account]
pub struct AgentRecord {
    /// The agent's key — the kernel's `AgentId` ([u8; 32]).
    pub agent: Pubkey,
    /// Whether the agent is active (kernel `AgentProfile.active`).
    pub active: bool,
    /// Role flags, kernel `Role` order: Admin / Oracle / Evaluator.
    pub is_admin: bool,
    pub is_oracle: bool,
    pub is_evaluator: bool,
    /// Influence, kernel fixed-point raw (kernel `Amount`).
    pub influence_raw: u64,
    /// Reputation, kernel fixed-point raw, bounded 0–100 units.
    pub reputation_raw: u64,
    /// Group memberships (kernel `GroupId`, u16), at most [`MAX_GROUPS`].
    pub groups: Vec<u16>,
    /// PDA bump for the `["agent", agent]` seeds.
    pub bump: u8,
}

impl AgentRecord {
    /// 8 (discriminator) + 32 + 1 + 1 + 1 + 1 + 8 + 8 + (4 + 8×2) + 1.
    pub const SPACE: usize = 8 + 32 + 4 + 8 + 8 + 20 + 1;

    /// Set or clear one role flag. Role codes: 0 = Admin, 1 = Oracle,
    /// 2 = Evaluator — the kernel's `Role` enum order. Any other code is
    /// an error and changes nothing.
    pub fn apply_role(&mut self, role: u8, grant: bool) -> Result<()> {
        match role {
            0 => self.is_admin = grant,
            1 => self.is_oracle = grant,
            2 => self.is_evaluator = grant,
            _ => return Err(TreasuryError::InvalidRole.into()),
        }
        Ok(())
    }
}

/// Accounts for [`treasury_program::post_observation`].
#[derive(Accounts)]
#[instruction(observation_id: u64)]
pub struct PostObservation<'info> {
    /// The posting Oracle's agent record (must exist, be active, and
    /// hold the Oracle role). Seeds bind it to the signer.
    #[account(mut, seeds = [b"agent", oracle.key().as_ref()], bump = agent_record.bump)]
    pub agent_record: Account<'info, AgentRecord>,
    /// The posting Oracle (signer; pays rent for the observation).
    #[account(mut)]
    pub oracle: Signer<'info>,
    /// Observation PDA, seeds `["observation", observation_id]`.
    /// One-time init: a duplicate id fails and changes no state.
    #[account(
        init,
        payer = oracle,
        space = Observation::SPACE,
        seeds = [b"observation", observation_id.to_le_bytes().as_ref()],
        bump
    )]
    pub observation: Account<'info, Observation>,
    /// Rent sysvar, required by the `init` constraint.
    pub system_program: Program<'info, System>,
    /// Clock sysvar: freshness is judged on-chain, never from a client
    /// timestamp (design §4.1, the FRESH bit's anchor).
    pub clock: Sysvar<'info, Clock>,
}

/// One Oracle-posted observation (design §3 `Observation`).
///
/// The FRESH bit reads through this account at commit: `timestamp ≤ now
/// < expiry` and `confidence ≥ Policy.min_obs_confidence`. The content
/// hash is the observation's content binding; the content itself lives
/// off-chain and is attested by the hash.
#[account]
pub struct Observation {
    /// Caller-chosen id (PDA seed component).
    pub observation_id: u64,
    /// The posting Oracle's key.
    pub oracle: Pubkey,
    /// Observation timestamp (must be ≤ Clock time at posting).
    pub timestamp: i64,
    /// Expiry: the observation is fresh only while `now < expiry`.
    pub expiry: i64,
    /// Confidence, kernel fixed-point raw (SCALE 1_000_000).
    pub confidence: u64,
    /// Content hash binding the off-chain observation content.
    pub content_hash: [u8; 32],
    /// PDA bump for the `["observation", id]` seeds.
    pub bump: u8,
}

impl Observation {
    /// 8 (discriminator) + 8 + 32 + 8 + 8 + 8 + 32 + 1.
    pub const SPACE: usize = 8 + 8 + 32 + 8 + 8 + 8 + 32 + 1;
}

/// Accounts for [`treasury_program::register_transition`].
#[derive(Accounts)]
#[instruction(transition_id: u64, observation_id: u64)]
pub struct RegisterTransition<'info> {
    /// The manifest's observation — must already exist (design §4.1:
    /// UnknownObservation if it does not; enforced by this account
    /// constraint, not by a returned error).
    #[account(seeds = [b"observation", observation_id.to_le_bytes().as_ref()], bump = observation.bump)]
    pub observation: Account<'info, Observation>,
    /// TransitionRecord PDA, seeds `["transition", transition_id]`.
    /// One-time init: a duplicate id fails and changes no state.
    #[account(
        init,
        payer = payer,
        space = TransitionRecord::SPACE,
        seeds = [b"transition", transition_id.to_le_bytes().as_ref()],
        bump
    )]
    pub transition_record: Account<'info, TransitionRecord>,
    /// Caller (pays rent). Registration itself is permissionless in the
    /// design table — no role gates who may create a transition; the
    /// predicate, not the caller, decides at commit.
    #[account(mut)]
    pub payer: Signer<'info>,
    /// Rent sysvar, required by the `init` constraint.
    pub system_program: Program<'info, System>,
    /// Clock sysvar: the manifest expiry is judged on-chain.
    pub clock: Sysvar<'info, Clock>,
}

/// Accounts for [`treasury_program::open_planning`].
#[derive(Accounts)]
#[instruction(transition_id: u64)]
pub struct OpenPlanning<'info> {
    /// The caller's agent record (must exist and be bound to the
    /// signer). Admin status is read from here: only the proposer or
    /// an Admin may open planning (design §4.1).
    #[account(seeds = [b"agent", caller.key().as_ref()], bump = agent_record.bump)]
    pub agent_record: Account<'info, AgentRecord>,
    /// The transition record to move Registered → Planning.
    #[account(mut, seeds = [b"transition", transition_id.to_le_bytes().as_ref()], bump = transition_record.bump)]
    pub transition_record: Account<'info, TransitionRecord>,
    /// The caller (proposer or Admin). Pays no rent here.
    pub caller: Signer<'info>,
    /// Clock sysvar: the planning deadline is judged on-chain.
    pub clock: Sysvar<'info, Clock>,
}

/// Maximum constraints in a frozen set (design §3: the frozen set is
/// mandatory ∪ proposer additions, fixed at registration). An
/// account-size bound, not a kernel rule — the launch policy types
/// carry at most 4 mandatory constraints (alloc), so 8 carries
/// headroom for proposer additions, and raising the bound is a SPACE
/// change, not a policy change.
pub const MAX_CONSTRAINTS: usize = 8;

/// One entry of a transition's frozen constraint set: the kernel's
/// `ConstraintId` ([u8; 32]) plus its current status, one of the
/// `STATUS_*` codes. Every entry starts Unavailable at registration;
/// `submit_constraint_report` overwrites the status, never the set.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConstraintEntry {
    /// The constraint's id (kernel `ConstraintId`).
    pub id: [u8; 32],
    /// Current status: Pass (0), Fail (1), or Unavailable (2).
    pub status: u8,
}

/// One kernel transition's on-chain record (design §3 `TransitionRecord`,
/// first fields).
///
/// Per-transition kernel state lives here, serialized (design §1.2:
/// load → call → store). This slice carries the manifest inputs, the
/// recorded proposer, the state, the plan count, the round count, the
/// current-round aggregates, the previous-round mean, the frozen
/// constraint set, and the objectives flag; per-transition exceptions,
/// reconciled flag, and realized hash are added by the lifecycle
/// instructions that own them — each field lands with the instruction
/// that writes it, not before.
#[account]
pub struct TransitionRecord {
    /// Caller-chosen id (PDA seed component).
    pub transition_id: u64,
    /// One of the `STATE_*` constants (kernel's seven states).
    pub state: u8,
    /// The registering payer's key — the kernel's recorded proposer.
    /// `open_planning` admits only this key or an Admin.
    pub proposer: Pubkey,
    /// The manifest's observation id (FRESH reads through it).
    pub observation_id: u64,
    /// Manifest expiry (past it, the transition can only expire).
    pub expires_at: i64,
    /// Planning deadline, set by `open_planning`; 0 until then.
    pub planning_deadline: i64,
    /// Plans submitted so far, counted by `submit_plan` so the
    /// kernel's `TooManyPlans` rule is enforceable on-chain.
    pub plan_count: u32,
    /// Rounds so far, starting at 1 — the kernel starts every
    /// transition with one empty round, and `advance_round` adds one.
    /// `submit_signal` signals the current round, `round_count - 1`.
    pub round_count: u32,
    /// Effective-weight total of the current round, maintained by
    /// `submit_signal` (FlatWeights: raw x signer influence). The
    /// kernel's RoundEmpty test is `total == Amount::ZERO`, and this
    /// is the field that test reads on-chain.
    pub current_round_total: u64,
    /// Weighted-support sum of the current round, maintained by
    /// `submit_signal` (effective weight x support, Amount arithmetic).
    /// `advance_round` derives the previous round's mean from this
    /// and the total, then resets both to zero for the new round.
    pub current_round_weighted: u64,
    /// Weighted mean support of the most recently closed round,
    /// snapshotted by `advance_round` (weighted x ONE / total). Zero
    /// until the first advance — the monotone-revision check reads
    /// this field, and round 0 has no previous round to read.
    pub prev_round_mean: u64,
    /// The frozen constraint set (design §3): fixed at
    /// `register_transition`, reported against by
    /// `submit_constraint_report`, never extended after registration.
    /// Each entry is 32 (id) + 1 (status) bytes.
    pub constraints: Vec<ConstraintEntry>,
    /// The independent objectives verdict (kernel `objectives_ok`):
    /// false at registration, overwritten by each
    /// `submit_objectives_check` report. The OBJECTIVES bit at commit
    /// is `require_objectives && !objectives_ok` — at launch the
    /// policy does not require objectives, so the bit is clear
    /// regardless, but the flag is recorded, not assumed.
    pub objectives_ok: bool,
    /// The transition's declared next hash (commit/commitment anchor).
    pub declared_hash: [u8; 32],
    /// PDA bump for the `["transition", id]` seeds.
    pub bump: u8,
}

impl TransitionRecord {
    /// 8 (discriminator) + 8 + 1 + 32 + 8 + 8 + 8 + 4 + 4
    /// + 8 + 8 + 8 + (4 + 8 × 33) + 1 + 32 + 1 = 407.
    pub const SPACE: usize = 8 + 8 + 1 + 32 + 8 + 8 + 8 + 4 + 4 + 8 + 8 + 8 + 268 + 1 + 32 + 1;
}

/// Maximum claims per plan (design §3: a plan is a capacity-claims
/// map). An account-size bound, not a kernel rule — the kernel's map
/// is unbounded, but an on-chain account is not; the launch surface
/// (3 stocks plus allocation classes) fits 16 with headroom, and
/// raising the bound is a SPACE change, not a policy change.
pub const MAX_CLAIMS: usize = 16;

/// Accounts for [`treasury_program::submit_plan`].
#[derive(Accounts)]
#[instruction(transition_id: u64)]
pub struct SubmitPlan<'info> {
    /// The submitting agent's record (must exist and be bound to the
    /// signer; active flag is the eligibility half enforced here).
    #[account(seeds = [b"agent", caller.key().as_ref()], bump = agent_record.bump)]
    pub agent_record: Account<'info, AgentRecord>,
    /// The transition record to plan against (must be Planning).
    #[account(mut, seeds = [b"transition", transition_id.to_le_bytes().as_ref()], bump = transition_record.bump)]
    pub transition_record: Account<'info, TransitionRecord>,
    /// PlanRecord PDA, seeds `["plan", transition_id, caller]`.
    /// One-time init: a second plan from the same agent fails and
    /// changes no state (Duplicate in the design taxonomy).
    #[account(
        init,
        payer = caller,
        space = PlanRecord::SPACE,
        seeds = [b"plan", transition_id.to_le_bytes().as_ref(), caller.key().as_ref()],
        bump
    )]
    pub plan_record: Account<'info, PlanRecord>,
    /// The submitting agent (signer; pays rent for the plan record).
    #[account(mut)]
    pub caller: Signer<'info>,
    /// Rent sysvar, required by the `init` constraint.
    pub system_program: Program<'info, System>,
    /// Clock sysvar: Planning state and the deadline are judged
    /// on-chain, never from a client timestamp.
    pub clock: Sysvar<'info, Clock>,
}

/// One capacity claim inside a plan: the kernel's claims map is
/// `ResourceId → Amount`, and Anchor's IDL cannot express a tuple
/// type — so the map entry is a named struct, not `(u16, u64)`.
/// Borsh layout is identical (2 + 8 = 10 bytes), and the struct is
/// what the generated IDL carries as a defined type.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Claim {
    /// The capacity stock claimed (launch stocks 0/1/2; allocation
    /// class stocks are ids ≥ 3, design §4.1).
    pub resource_id: u16,
    /// The claimed amount, kernel fixed-point raw (SCALE 1_000_000).
    pub amount: u64,
}

/// One agent's plan for a transition (design §3 `Plan`).
///
/// Capacity claims `{resource_id: amount}` plus expiry, stored so
/// `reconcile` can sum them against the capacity stocks (COMPAT) and
/// `commit` can debit them atomically. The claims hash is the event's
/// commitment over the stored claims (see [`claims_hash`]).
/// Claims are [`Claim`] structs, not tuples: Anchor's IDL cannot
/// express a tuple type, and the first `anchor build` of this slice
/// failed on exactly that (fixed by the struct, same byte layout).
#[account]
pub struct PlanRecord {
    /// The transition this plan belongs to (PDA seed component).
    pub transition_id: u64,
    /// The submitting agent's key (PDA seed component).
    pub agent: Pubkey,
    /// Plan expiry: the kernel refuses a plan expired at submission,
    /// and reconcile/commit judge the plan against its expiry.
    pub expires_at: i64,
    /// Capacity claims, at most [`MAX_CLAIMS`], distinct resource ids.
    pub claims: Vec<Claim>,
    /// SHA-256 over the claims encoding (see [`claims_hash`]).
    pub claims_hash: [u8; 32],
    /// PDA bump for the `["plan", transition_id, agent]` seeds.
    pub bump: u8,
}

impl PlanRecord {
    /// 8 (discriminator) + 8 + 32 + 8 + (4 + 16×10) + 32 + 1.
    pub const SPACE: usize = 8 + 8 + 32 + 8 + 164 + 32 + 1;
}

/// Accounts for [`treasury_program::submit_signal`].
#[derive(Accounts)]
#[instruction(transition_id: u64, round: u32)]
pub struct SubmitSignal<'info> {
    /// The signalling agent's record (must exist and be bound to the
    /// signer; active flag is the eligibility half enforced here).
    #[account(seeds = [b"agent", caller.key().as_ref()], bump = agent_record.bump)]
    pub agent_record: Account<'info, AgentRecord>,
    /// The transition record to signal against (Registered or Planning).
    #[account(mut, seeds = [b"transition", transition_id.to_le_bytes().as_ref()], bump = transition_record.bump)]
    pub transition_record: Account<'info, TransitionRecord>,
    /// SignalRecord PDA, seeds `["signal", transition_id, round, caller]`.
    /// One-time init: a second signal from the same agent in the same
    /// round fails and changes no state (AlreadySignalled in the
    /// design taxonomy).
    #[account(
        init,
        payer = caller,
        space = SignalRecord::SPACE,
        seeds = [b"signal", transition_id.to_le_bytes().as_ref(), round.to_le_bytes().as_ref(), caller.key().as_ref()],
        bump
    )]
    pub signal_record: Account<'info, SignalRecord>,
    /// The signalling agent (signer; pays rent for the signal record).
    #[account(mut)]
    pub caller: Signer<'info>,
    /// Rent sysvar, required by the `init` constraint.
    pub system_program: Program<'info, System>,
}

/// Accounts for [`treasury_program::advance_round`].
#[derive(Accounts)]
#[instruction(transition_id: u64)]
pub struct AdvanceRound<'info> {
    /// The transition record to advance (must be Registered or
    /// Planning, current round non-empty, room under `max_rounds`).
    /// No new account: the round count, the snapshotted previous-round
    /// mean, and the reset current-round aggregates all live on this
    /// record (design §3).
    #[account(mut, seeds = [b"transition", transition_id.to_le_bytes().as_ref()], bump = transition_record.bump)]
    pub transition_record: Account<'info, TransitionRecord>,
    /// The caller (signer; pays no rent here). Permissionless, as in
    /// the kernel — the kernel ignores its advance_round caller, and
    /// no AgentRecord is required, so revision timing cannot be gated
    /// by a role the kernel does not gate it by.
    pub caller: Signer<'info>,
}

/// Accounts for [`treasury_program::submit_objectives_check`].
#[derive(Accounts)]
#[instruction(transition_id: u64)]
pub struct SubmitObjectivesCheck<'info> {
    /// The reporting agent's record (must exist and be bound to the
    /// signer; the Evaluator role is the gate enforced here).
    #[account(seeds = [b"agent", caller.key().as_ref()], bump = agent_record.bump)]
    pub agent_record: Account<'info, AgentRecord>,
    /// The transition record to check against (Registered or
    /// Planning). No new account: the verdict overwrites the flag on
    /// this record in place (design §3).
    #[account(mut, seeds = [b"transition", transition_id.to_le_bytes().as_ref()], bump = transition_record.bump)]
    pub transition_record: Account<'info, TransitionRecord>,
    /// The reporting Evaluator (signer; pays no rent here).
    pub caller: Signer<'info>,
}

/// Accounts for [`treasury_program::submit_constraint_report`].
#[derive(Accounts)]
#[instruction(transition_id: u64)]
pub struct SubmitConstraintReport<'info> {
    /// The reporting agent's record (must exist and be bound to the
    /// signer; the Evaluator role is the gate enforced here).
    #[account(seeds = [b"agent", caller.key().as_ref()], bump = agent_record.bump)]
    pub agent_record: Account<'info, AgentRecord>,
    /// The transition record to report against (Registered or
    /// Planning). No new account: the report mutates this record's
    /// frozen constraint set in place (design §3).
    #[account(mut, seeds = [b"transition", transition_id.to_le_bytes().as_ref()], bump = transition_record.bump)]
    pub transition_record: Account<'info, TransitionRecord>,
    /// The reporting Evaluator (signer; pays no rent here).
    pub caller: Signer<'info>,
}

/// One agent's signal in one round of a transition (design §3
/// `Round / Signal`, per-agent leg).
///
/// The kernel's round aggregates (total, weighted) are computed at
/// commit replay from these records — they are not stored here, so no
/// client-supplied total can be trusted. The monotone-revision mean
/// likewise comes from replay, not from this record.
#[account]
pub struct SignalRecord {
    /// The transition this signal belongs to (PDA seed component).
    pub transition_id: u64,
    /// The round signalled (PDA seed component).
    pub round: u32,
    /// The signalling agent's key (PDA seed component).
    pub agent: Pubkey,
    /// The raw weight supplied (kernel `Amount` raw, SCALE 1_000_000).
    pub raw: u64,
    /// Support, kernel `Amount` raw, at most [`SUPPORT_ONE_RAW`].
    pub support: u64,
    /// PDA bump for the `["signal", transition_id, round, agent]` seeds.
    pub bump: u8,
}

impl SignalRecord {
    /// 8 (discriminator) + 8 + 4 + 32 + 8 + 8 + 1.
    pub const SPACE: usize = 8 + 8 + 4 + 32 + 8 + 8 + 1;
}

/// Program errors (design §4 taxonomy, first entries).
#[error_code]
pub enum TreasuryError {
    /// The caller is not the `Config` authority.
    #[msg("Caller is not the program authority.")]
    Unauthorized,
    /// The record's agent key does not match the instruction's agent.
    #[msg("Agent record does not match the instruction agent.")]
    AgentMismatch,
    /// Role code outside 0 (Admin), 1 (Oracle), 2 (Evaluator).
    #[msg("Unknown role code: 0 = Admin, 1 = Oracle, 2 = Evaluator.")]
    InvalidRole,
    /// More than MAX_GROUPS groups supplied at registration.
    #[msg("Too many groups: at most 8 per agent.")]
    InvalidGroups,
    /// Reputation outside 0–100 units.
    #[msg("Reputation out of bounds: 0 to 100 units.")]
    InvalidReputation,
    /// Observation timestamp is after on-chain time — future-dated.
    #[msg("Observation timestamp is in the future.")]
    ObservationFromFuture,
    /// The window is not a window: expiry at/before timestamp, or a
    /// manifest already expired at registration.
    #[msg("Bad window: expiry must be after the timestamp / in the future.")]
    BadWindow,
    /// The transition is not in the state this instruction requires
    /// (e.g. opening planning on a non-Registered transition).
    #[msg("Transition is not in the required state.")]
    BadState,
    /// Checked arithmetic overflowed (e.g. a planning deadline beyond
    /// the timestamp range). An overflow is an error, never a wrap.
    #[msg("Arithmetic overflow.")]
    Overflow,
    /// Planning is not open: the transition is not Planning, or `now`
    /// has reached the planning deadline (kernel `PlanningClosed`).
    #[msg("Planning is closed: transition not Planning or deadline reached.")]
    PlanningClosed,
    /// The agent is not eligible to plan (e.g. inactive). The
    /// group-overlap half of kernel eligibility awaits `relevant_groups`
    /// on the record (see `submit_plan` docs).
    #[msg("Agent is not eligible to submit a plan.")]
    NotEligible,
    /// The transition already holds `max_plans` plans (4 at launch).
    #[msg("Too many plans for this transition.")]
    TooManyPlans,
    /// Claims break the account/keyed-map shape: more than MAX_CLAIMS
    /// entries, or a resource id claimed twice.
    #[msg("Invalid claims: too many, or a resource claimed twice.")]
    InvalidClaims,
    /// Signal support above kernel `Amount::ONE` (1_000_000 raw).
    #[msg("Signal support out of range: at most 1.0.")]
    SupportOutOfRange,
    /// The round index is at or beyond `max_rounds` (3 at launch).
    #[msg("Rounds exhausted: round index at or beyond max_rounds.")]
    RoundsExhausted,
    /// A re-signal moved farther from the previous round's weighted
    /// mean than the agent's previous signal — revision must move
    /// toward the mean, never away from it.
    #[msg("Monotone revision violated: re-signal moved away from the previous round's mean.")]
    MonotoneRevision,
    /// The constraint id is not a member of the transition's frozen
    /// set — a report cannot invent a constraint (design §4.1).
    #[msg("Unknown constraint: not a member of the transition's frozen set.")]
    UnknownConstraint,
    /// The status code is not a kernel `ConstraintStatus` (0 = Pass,
    /// 1 = Fail, 2 = Unavailable).
    #[msg("Invalid constraint status: 0 = Pass, 1 = Fail, 2 = Unavailable.")]
    InvalidConstraintStatus,
    /// The frozen set breaks its shape: more than MAX_CONSTRAINTS
    /// entries, or a constraint id supplied twice.
    #[msg("Invalid constraint set: too many, or a constraint id twice.")]
    InvalidConstraints,
    /// The current round is empty: its effective-weight total is zero
    /// — no signals, or only zero-weight signals. Advancing an empty
    /// round would create a round with nothing to revise from.
    #[msg("Current round is empty: effective-weight total is zero.")]
    RoundEmpty,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fee_stream_is_the_single_combined_flow() {
        // D-5: 60 bps transaction + 13 bps IDL on the SAME flow.
        assert_eq!(FEE_PPM, 6_000 + 1_300);
    }

    #[test]
    fn movable_liquidity_is_one_sixth_of_treasury() {
        // D-10, ratio-exact at the 25% sleeve: 2/3 x 25% = 1/6.
        assert_eq!(movable_liquidity(100_000_000), 16_666_666);
        assert_eq!(max_outstanding(100_000_000), 8_333_333);
        assert_eq!(movable_liquidity(100_000_000), 2 * max_outstanding(100_000_000));
    }

    #[test]
    fn config_space_matches_fields() {
        assert_eq!(Config::SPACE, 74);
    }

    #[test]
    fn agent_record_space_matches_fields() {
        assert_eq!(AgentRecord::SPACE, 81);
        assert_eq!(MAX_GROUPS, 8);
        assert_eq!(REP_MAX_RAW, 100_000_000);
    }

    #[test]
    fn role_codes_follow_kernel_role_order() {
        // Kernel Role enum order: Admin, Oracle, Evaluator.
        let mut record = AgentRecord {
            agent: Pubkey::default(),
            active: true,
            is_admin: false,
            is_oracle: false,
            is_evaluator: false,
            influence_raw: 1_000_000,
            reputation_raw: 50_000_000,
            groups: vec![1],
            bump: 0,
        };
        record.apply_role(1, true).expect("oracle grant lands");
        assert!(record.is_oracle);
        assert!(!record.is_admin && !record.is_evaluator);
        record.apply_role(0, true).expect("admin grant lands");
        record.apply_role(1, false).expect("oracle revoke lands");
        assert!(record.is_admin && !record.is_oracle);
        // Revoking an unheld role is a no-op success.
        record.apply_role(2, false).expect("unheld revoke is fine");
        assert!(!record.is_evaluator);
        // An unknown code changes nothing and errors.
        assert!(record.apply_role(3, true).is_err());
        assert!(record.is_admin && !record.is_oracle && !record.is_evaluator);
    }

    #[test]
    fn observation_and_transition_spaces_match_fields() {
        assert_eq!(Observation::SPACE, 105);
        // 407: the proposer key (32) joined with open_planning, the
        // plan count (4) with submit_plan, the round count (4) with
        // submit_signal, the frozen constraint set (4 + 8 x 33)
        // with submit_constraint_report, the objectives flag (1)
        // with submit_objectives_check, and the round aggregates
        // (8 + 8) plus the previous-round mean (8) with advance_round.
        assert_eq!(TransitionRecord::SPACE, 407);
        assert_eq!(MAX_CONSTRAINTS, 8);
        assert_eq!(STATUS_PASS, 0);
        assert_eq!(STATUS_FAIL, 1);
        assert_eq!(STATUS_UNAVAILABLE, 2);
    }

    #[test]
    fn submit_constraint_report_gate_matrix() {
        // Caller binding is structural (AgentRecord match in the
        // handler), so the helper takes no caller argument.
        let base = |evaluator: bool, state: u8, known: bool, status: u8| {
            can_submit_constraint_report(evaluator, state, known, status)
        };
        // The clean cases pass: Evaluator, Registered or Planning,
        // known constraint, real status.
        assert!(base(true, STATE_REGISTERED, true, STATUS_PASS).is_ok());
        assert!(base(true, STATE_PLANNING, true, STATUS_FAIL).is_ok());
        // A non-Evaluator is refused first, whatever else holds.
        assert!(base(false, STATE_REGISTERED, true, STATUS_PASS).is_err());
        // Wrong state refuses (Committed, Settled, Rejected, Expired).
        assert!(base(true, STATE_COMMITTED, true, STATUS_PASS).is_err());
        assert!(base(true, STATE_SETTLED, true, STATUS_PASS).is_err());
        // An id outside the frozen set is unknown, never recorded.
        assert!(base(true, STATE_REGISTERED, false, STATUS_PASS).is_err());
        // A status code outside 0..=2 is not a status.
        assert!(base(true, STATE_REGISTERED, true, 3).is_err());
    }

    #[test]
    fn submit_objectives_check_gate_matrix() {
        // Caller binding is structural (AgentRecord match in the
        // handler), so the helper takes no caller argument. The
        // verdict itself is a bool and needs no validation — any bool
        // is a real verdict, and a later report overwrites it.
        let base =
            |evaluator: bool, state: u8| can_submit_objectives_check(evaluator, state);
        // The clean cases pass: Evaluator, Registered or Planning.
        assert!(base(true, STATE_REGISTERED).is_ok());
        assert!(base(true, STATE_PLANNING).is_ok());
        // A non-Evaluator is refused first, whatever else holds.
        assert!(base(false, STATE_REGISTERED).is_err());
        assert!(base(false, STATE_PLANNING).is_err());
        // Wrong state refuses (Committed, Settled, Rejected, Expired).
        assert!(base(true, STATE_COMMITTED).is_err());
        assert!(base(true, STATE_SETTLED).is_err());
        assert!(base(true, STATE_REJECTED).is_err());
        assert!(base(true, STATE_EXPIRED).is_err());
    }

    #[test]
    fn constraint_set_validation_and_reporting() {
        // Empty and distinct sets pass; the bound is 8.
        assert!(validate_constraint_ids(&[]).is_ok());
        assert!(validate_constraint_ids(&[[1u8; 32], [2u8; 32]]).is_ok());
        assert!(validate_constraint_ids(&[[1u8; 32], [1u8; 32]]).is_err());
        let too_many: Vec<[u8; 32]> = (0..9).map(|b| [b; 32]).collect();
        assert!(validate_constraint_ids(&too_many).is_err());
        // Reporting overwrites the entry's status in place.
        let mut set = vec![
            ConstraintEntry { id: [1u8; 32], status: STATUS_UNAVAILABLE },
            ConstraintEntry { id: [2u8; 32], status: STATUS_UNAVAILABLE },
        ];
        set_constraint_status(&mut set, [1u8; 32], STATUS_PASS).expect("known id reports");
        assert_eq!(set[0].status, STATUS_PASS);
        assert_eq!(set[1].status, STATUS_UNAVAILABLE);
        // Re-reporting corrects; the set never grows.
        set_constraint_status(&mut set, [1u8; 32], STATUS_FAIL).expect("re-report lands");
        assert_eq!(set[0].status, STATUS_FAIL);
        assert_eq!(set.len(), 2);
        // An unknown id reports nothing and changes nothing.
        assert!(set_constraint_status(&mut set, [9u8; 32], STATUS_PASS).is_err());
        assert_eq!(set[0].status, STATUS_FAIL);
        // A bad status code is refused before any write.
        assert!(set_constraint_status(&mut set, [1u8; 32], 3).is_err());
        assert_eq!(set[0].status, STATUS_FAIL);
    }

    #[test]
    fn signal_record_space_matches_fields() {
        // 8 + 8 + 4 + 32 + 8 + 8 + 1 = 69.
        assert_eq!(SignalRecord::SPACE, 69);
        assert_eq!(SUPPORT_ONE_RAW, 1_000_000);
    }

    #[test]
    fn submit_signal_gate_matrix() {
        // Caller binding and the duplicate rule are structural (PDA
        // seeds + AgentRecord match; one SignalRecord per
        // (transition, round, agent)), so the helper takes no caller
        // argument. max_rounds 3, current round 0 at launch.
        let base = |state: u8, support: u64, round: u32, active: bool| {
            can_submit_signal(active, state, support, round, 3, 0)
        };
        // The clean cases pass: Registered or Planning, round 0.
        assert!(base(STATE_REGISTERED, 500_000, 0, true).is_ok());
        assert!(base(STATE_PLANNING, 1_000_000, 0, true).is_ok());
        // Support above 1.0 is out of range, checked first.
        assert!(base(STATE_REGISTERED, 1_000_001, 0, true).is_err());
        // Wrong state refuses (Committed, Settled, Rejected, Expired).
        assert!(base(STATE_COMMITTED, 500_000, 0, true).is_err());
        assert!(base(STATE_SETTLED, 500_000, 0, true).is_err());
        // Round at max_rounds is exhausted; a stale round is bad state.
        assert!(base(STATE_PLANNING, 500_000, 3, true).is_err());
        assert!(base(STATE_PLANNING, 500_000, 1, true).is_err());
        // Inactive agents are not eligible.
        assert!(base(STATE_PLANNING, 500_000, 0, false).is_err());
    }

    #[test]
    fn monotone_revision_gate() {
        // Mean 600_000, previous signal 500_000 (|diff| 100_000).
        // Moving toward the mean passes; holding passes; moving away fails.
        assert!(check_monotone_revision(500_000, 600_000, 600_000).is_ok());
        assert!(check_monotone_revision(500_000, 600_000, 550_000).is_ok());
        assert!(check_monotone_revision(500_000, 600_000, 500_000).is_ok());
        assert!(check_monotone_revision(500_000, 600_000, 499_999).is_err());
        assert!(check_monotone_revision(500_000, 600_000, 700_001).is_err());
        // Exactly as far as before is admissible (no farther).
        assert!(check_monotone_revision(500_000, 600_000, 700_000).is_ok());
    }

    #[test]
    fn advance_round_gate_matrix() {
        // Caller binding is permissionless (the kernel ignores its
        // advance_round caller), so the helper takes no caller
        // argument. max_rounds 3 at launch.
        let base = |state: u8, total: u64, count: u32| {
            can_advance_round(state, total, count, 3)
        };
        // The clean cases pass: Registered or Planning, non-empty
        // round, room under max_rounds.
        assert!(base(STATE_REGISTERED, 1, 1).is_ok());
        assert!(base(STATE_PLANNING, 500_000, 2).is_ok());
        // Wrong state refuses (Committable, Committed, Settled,
        // Rejected, Expired) — checked before emptiness.
        assert!(base(STATE_COMMITTABLE, 500_000, 1).is_err());
        assert!(base(STATE_COMMITTED, 500_000, 1).is_err());
        assert!(base(STATE_SETTLED, 500_000, 1).is_err());
        // An empty round refuses: no signals, or only zero-weight
        // signals — both are a zero total, and the kernel's test is
        // total == ZERO, not a signal count.
        assert!(base(STATE_REGISTERED, 0, 1).is_err());
        assert!(base(STATE_PLANNING, 0, 2).is_err());
        // At max_rounds a non-empty round is exhausted.
        assert!(base(STATE_PLANNING, 500_000, 3).is_err());
        // Check order, as in the kernel: an empty round at max_rounds
        // reports RoundEmpty, not RoundsExhausted.
        let empty_at_max = base(STATE_PLANNING, 0, 3);
        assert!(empty_at_max.is_err());
        assert_eq!(
            empty_at_max.unwrap_err(),
            anchor_lang::error::Error::from(TreasuryError::RoundEmpty)
                .into()
        );
    }

    #[test]
    fn round_aggregates_and_mean_follow_kernel_amount_math() {
        // FlatWeights: relevance and group policy are 1.0, so eff is
        // raw x influence / SCALE, and term is eff x support / SCALE.
        // Full influence (1.0): eff == raw.
        assert_eq!(
            signal_aggregates(1_000_000, 600_000, 1_000_000).unwrap(),
            (1_000_000, 600_000)
        );
        // Half influence (0.5): raw 1.0 -> eff 0.5; support 0.6 ->
        // term 0.3.
        assert_eq!(
            signal_aggregates(1_000_000, 600_000, 500_000).unwrap(),
            (500_000, 300_000)
        );
        // Zero raw or zero influence contributes nothing — the round
        // stays empty in the kernel's sense.
        assert_eq!(
            signal_aggregates(0, 600_000, 1_000_000).unwrap(),
            (0, 0)
        );
        assert_eq!(
            signal_aggregates(1_000_000, 600_000, 0).unwrap(),
            (0, 0)
        );
        // The mean is weighted x ONE / total, floored, as the kernel
        // computes it for the previous round.
        assert_eq!(round_mean(600_000, 1_000_000).unwrap(), 600_000);
        assert_eq!(round_mean(300_000, 500_000).unwrap(), 600_000);
        assert_eq!(round_mean(1, 3).unwrap(), 333_333);
        // A zero total has no mean.
        assert!(round_mean(0, 0).is_err());
        assert!(round_mean(600_000, 0).is_err());
    }

    #[test]
    fn plan_record_space_matches_fields() {
        // 8 + 8 + 32 + 8 + (4 + 16 x 10) + 32 + 1 = 253.
        assert_eq!(PlanRecord::SPACE, 253);
        assert_eq!(MAX_CLAIMS, 16);
    }

    #[test]
    fn transition_states_follow_kernel_order() {
        // Design §3: the kernel's seven states, exactly, in order.
        assert_eq!(STATE_REGISTERED, 0);
        assert_eq!(STATE_PLANNING, 1);
        assert_eq!(STATE_COMMITTABLE, 2);
        assert_eq!(STATE_COMMITTED, 3);
        assert_eq!(STATE_SETTLED, 4);
        assert_eq!(STATE_REJECTED, 5);
        assert_eq!(STATE_EXPIRED, 6);
    }

    #[test]
    fn observation_window_validation() {
        // Valid window passes.
        assert!(validate_observation_window(100, 200, 150).is_ok());
        // Future-dated observation is rejected (verification theater).
        assert!(validate_observation_window(200, 300, 150).is_err());
        // Expiry at the timestamp is not a window.
        assert!(validate_observation_window(100, 100, 150).is_err());
        // Expiry before the timestamp is not a window.
        assert!(validate_observation_window(100, 90, 150).is_err());
        // Timestamp exactly at now is not future-dated.
        assert!(validate_observation_window(150, 200, 150).is_ok());
    }

    #[test]
    fn transition_expiry_validation() {
        assert!(validate_transition_expiry(200, 150).is_ok());
        // Already expired at registration creates nothing.
        assert!(validate_transition_expiry(150, 150).is_err());
        assert!(validate_transition_expiry(100, 150).is_err());
    }

    #[test]
    fn fresh_transition_record_starts_registered() {
        let proposer = Pubkey::new_from_array([1u8; 32]);
        let record = TransitionRecord {
            transition_id: 7,
            state: STATE_REGISTERED,
            proposer,
            observation_id: 3,
            expires_at: 1_000,
            planning_deadline: 0,
            plan_count: 0,
            round_count: 1,
            current_round_total: 0,
            current_round_weighted: 0,
            prev_round_mean: 0,
            constraints: vec![ConstraintEntry {
                id: [5u8; 32],
                status: STATUS_UNAVAILABLE,
            }],
            objectives_ok: false,
            declared_hash: [9u8; 32],
            bump: 255,
        };
        assert_eq!(record.state, STATE_REGISTERED);
        assert_eq!(record.proposer, proposer);
        assert_eq!(record.planning_deadline, 0);
        assert_eq!(record.plan_count, 0);
        assert_eq!(record.round_count, 1);
        assert_eq!(record.current_round_total, 0);
        assert_eq!(record.current_round_weighted, 0);
        assert_eq!(record.prev_round_mean, 0);
        assert!(!record.objectives_ok);
        assert_eq!(record.constraints.len(), 1);
        assert_eq!(record.constraints[0].status, STATUS_UNAVAILABLE);
        assert_eq!(record.observation_id, 3);
    }

    #[test]
    fn open_planning_admits_proposer_or_admin_only() {
        let proposer = Pubkey::new_from_array([1u8; 32]);
        let admin = Pubkey::new_from_array([2u8; 32]);
        let stranger = Pubkey::new_from_array([3u8; 32]);
        // The proposer opens their own transition.
        assert!(can_open_planning(proposer, proposer, false, STATE_REGISTERED).is_ok());
        // An Admin opens anyone's transition.
        assert!(can_open_planning(admin, proposer, true, STATE_REGISTERED).is_ok());
        // A non-proposer non-Admin is refused.
        assert!(can_open_planning(stranger, proposer, false, STATE_REGISTERED).is_err());
        // Even the proposer cannot re-open a transition already Planning.
        assert!(can_open_planning(proposer, proposer, false, STATE_PLANNING).is_err());
        // Even an Admin cannot open a Committed transition.
        assert!(can_open_planning(admin, proposer, true, STATE_COMMITTED).is_err());
    }

    #[test]
    fn submit_plan_gate_matrix() {
        // Caller binding is structural (PDA seeds + AgentRecord match
        // in the handler), so the helper takes no caller argument.
        let base = |state: u8,
                    now: i64,
                    deadline: i64,
                    active: bool,
                    count: u32,
                    expires: i64| {
            can_submit_plan(active, state, now, deadline, count, 4, expires)
        };
        // The clean case passes.
        assert!(base(STATE_PLANNING, 100, 200, true, 0, 300).is_ok());
        // Room under the cap passes; at the cap it does not.
        assert!(base(STATE_PLANNING, 100, 200, true, 3, 300).is_ok());
        assert!(base(STATE_PLANNING, 100, 200, true, 4, 300).is_err());
        // Planning not open: wrong state, or the deadline reached.
        assert!(base(STATE_REGISTERED, 100, 200, true, 0, 300).is_err());
        assert!(base(STATE_PLANNING, 200, 200, true, 0, 300).is_err());
        assert!(base(STATE_PLANNING, 201, 200, true, 0, 300).is_err());
        // Inactive agents are not eligible.
        assert!(base(STATE_PLANNING, 100, 200, false, 0, 300).is_err());
        // Plan expiry at or before now is a bad window.
        assert!(base(STATE_PLANNING, 100, 200, true, 0, 100).is_err());
        assert!(base(STATE_PLANNING, 100, 200, true, 0, 99).is_err());
    }

    #[test]
    fn claims_validation_and_hash() {
        let claim = |resource_id: u16, amount: u64| Claim { resource_id, amount };
        // Empty and distinct claims pass.
        assert!(validate_claims(&[]).is_ok());
        assert!(validate_claims(&[claim(0, 100), claim(1, 200), claim(2, 300)]).is_ok());
        // A resource claimed twice is rejected, not double-counted.
        assert!(validate_claims(&[claim(1, 100), claim(1, 200)]).is_err());
        // Over the account bound is rejected.
        let too_many: Vec<Claim> = (0..17).map(|r| claim(r, 1)).collect();
        assert!(validate_claims(&too_many).is_err());
        // The hash is deterministic, order-sensitive, and content-bound.
        let claims = vec![claim(0, 100), claim(2, 300)];
        assert_eq!(claims_hash(&claims), claims_hash(&claims));
        assert_ne!(claims_hash(&claims), claims_hash(&[claim(0, 100)]));
        assert_ne!(claims_hash(&claims), claims_hash(&[claim(2, 300), claim(0, 100)]));
        // SHA-256 of the empty encoding is the well-known empty hash.
        assert_eq!(
            claims_hash(&[]),
            [
                0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8,
                0x99, 0x6f, 0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c,
                0xa4, 0x95, 0x99, 0x1b, 0x78, 0x52, 0xb8, 0x55,
            ]
        );
    }

    #[test]
    fn planning_deadline_is_now_plus_window() {
        assert_eq!(
            planning_deadline(1_000, kernel_bridge::PLANNING_WINDOW).unwrap(),
            1_000 + kernel_bridge::PLANNING_WINDOW as i64
        );
        // Overflow is an error, never a wrapped deadline.
        assert!(planning_deadline(i64::MAX, kernel_bridge::PLANNING_WINDOW).is_err());
    }
}
