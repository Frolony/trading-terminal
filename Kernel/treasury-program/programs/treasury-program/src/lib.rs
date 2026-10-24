//! treasury_program — the own treasury program, W4.3 scaffold (skeleton).
//!
//! Design: `files/OWN_IDL_DESIGN.md` (W4.1, validated and revised 2026-10-24;
//! W4.3 GO given by Carlos 2026-10-24 ~03:39 CST). Placement C: the kernel
//! crate compiles into the program (no_std SBF) and the full 13-bit
//! `commit_fail_mask` is computed on-chain; there is NO digest publisher —
//! the W3.3 mock's trust root does not survive into this program.
//!
//! # Scaffold status (this increment: skeleton + `anchor build` + IDL)
//!
//! This file is the skeleton only: one genesis instruction
//! (`initialize_config`, design §4.1) and the `Config` account (design §3).
//! The kernel crate is NOT yet vendored in, and no governance / treasury /
//! distribution instruction exists yet — those land in design order in the
//! next increments (§4.1 governance first, then §4.2 treasury, then §4.3
//! distribution). Nothing here is deployed: NO devnet deployment in W4.3,
//! NO mainnet step exists in W4. No instruction here moves value.
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
}
