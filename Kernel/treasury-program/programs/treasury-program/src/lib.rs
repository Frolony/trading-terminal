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
//! increment. The lifecycle instructions (`post_observation`,
//! `register_transition`, …) land next, in design order, then §4.2
//! treasury, then §4.3 distribution.
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
}
