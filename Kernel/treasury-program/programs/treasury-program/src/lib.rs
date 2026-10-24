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
    /// deadline unset (0) until `open_planning` sets it. The payer's key
    /// is recorded as the proposer: the kernel records the proposer at
    /// registration (`kernel.rs` `register_transition`), and
    /// `open_planning` below admits only that proposer or an Admin.
    pub fn register_transition(
        ctx: Context<RegisterTransition>,
        transition_id: u64,
        observation_id: u64,
        expires_at: i64,
        declared_hash: [u8; 32],
    ) -> Result<()> {
        let now = ctx.accounts.clock.unix_timestamp;
        validate_transition_expiry(expires_at, now)?;
        let record = &mut ctx.accounts.transition_record;
        record.transition_id = transition_id;
        record.state = STATE_REGISTERED;
        record.proposer = ctx.accounts.payer.key();
        record.observation_id = observation_id;
        record.expires_at = expires_at;
        record.planning_deadline = 0;
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

/// One kernel transition's on-chain record (design §3 `TransitionRecord`,
/// first fields).
///
/// Per-transition kernel state lives here, serialized (design §1.2:
/// load → call → store). This slice carries the manifest inputs, the
/// recorded proposer, and the state; the frozen constraint set,
/// objectives flag, rounds/signals, plans, per-transition exceptions,
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
    /// The transition's declared next hash (commit/commitment anchor).
    pub declared_hash: [u8; 32],
    /// PDA bump for the `["transition", id]` seeds.
    pub bump: u8,
}

impl TransitionRecord {
    /// 8 (discriminator) + 8 + 1 + 32 + 8 + 8 + 8 + 32 + 1.
    pub const SPACE: usize = 8 + 8 + 1 + 32 + 8 + 8 + 8 + 32 + 1;
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
        // 106: the proposer key (32) joined the record with open_planning.
        assert_eq!(TransitionRecord::SPACE, 106);
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
            declared_hash: [9u8; 32],
            bump: 255,
        };
        assert_eq!(record.state, STATE_REGISTERED);
        assert_eq!(record.proposer, proposer);
        assert_eq!(record.planning_deadline, 0);
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
    fn planning_deadline_is_now_plus_window() {
        assert_eq!(
            planning_deadline(1_000, kernel_bridge::PLANNING_WINDOW).unwrap(),
            1_000 + kernel_bridge::PLANNING_WINDOW as i64
        );
        // Overflow is an error, never a wrapped deadline.
        assert!(planning_deadline(i64::MAX, kernel_bridge::PLANNING_WINDOW).is_err());
    }
}
