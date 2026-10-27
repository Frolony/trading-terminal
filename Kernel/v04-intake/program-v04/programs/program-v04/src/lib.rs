//! Migration step 5, slice 5: the v0.4 program scaffold.
//!
//! The delivered v0.4.1 kernel crate compiled into an Anchor program
//! under the pinned toolchain (Anchor 1.2.1 / Solana 4.1.2 /
//! platform-tools v1.54) — the SBF-compilation proof the program rewrite
//! needs before any instruction family lands. One probe instruction only:
//! `probe_kernel` constructs a kernel from the launch-pinned config
//! (reputation influence OFF, 3 council seats) and reports its genesis
//! state. It proves the crate links, constructs, and runs its accessors
//! inside program code; it does not replay a transition, hold an
//! account, or measure CU.
//!
//! Explicitly not this slice (later step-5 slices, not this one):
//! * No instruction families (§4.1/§4.2/§4.3) and no §3 account
//!   inventory — the bridge crate (`program-bridge-v04`, slices 1–4)
//!   already proves those shapes on the host; the on-chain account
//!   layout is the next slice's design, not a scaffold guess.
//! * No CU measurement — cu-golden rides the program rewrite once
//!   instructions exist to measure; nothing here assumes a CU figure.
//! * No deployment — the program id is staging-only (see Anchor.toml),
//!   and no keypair for it is kept.
//! * The delivered v0.4 crate is consumed by path with
//!   `default-features = false` (no_std + alloc), exactly as the v0.2
//!   program consumes the pristine v0.2 crate. Only the
//!   no_std-compatible API is used (consts / `from_raw`, never the
//!   std-only `from_f64` convenience).
//!
//! Slice 6 (this slice): the §4.1 genesis instruction — `initialize_kernel`
//! — and the first §3 account, `KernelState`. `KernelState` carries the
//! design's `DeclaredChain` fields (`head_index`/`head_hash`) alongside
//! `sealed`/`clock`; no standalone `DeclaredChain` account is split out
//! in this slice. The instruction constructs the real probe-config kernel
//! and persists its genesis state; it does not claim a seal.
//!
//! Deliberately NOT slice 6, and why: there is no `seal_kernel`
//! instruction here. The kernel's `seal_genesis` refuses until the
//! launch genesis is complete — a full council, the four governance
//! policies, and the influence-share bound — and those genesis steps
//! live in the step-2 launch crate, which is std-only (`std` collections,
//! `vec!`) and cannot compile to SBF as-is. An account-only seal flag
//! would claim a seal the kernel never granted, so no such flag is
//! written: the seal instruction is slice 7's work, gated on a no_std
//! port of the launch genesis steps (or the equivalent direct
//! `genesis_apply` sequence in program code). The host test
//! `probe_kernel_seal_refused_without_launch_genesis` pins this: the
//! probe kernel's seal is refused `GenesisNotReady`, it is not faked.

use anchor_lang::prelude::*;
use coordination_kernel::{Amount, Config, Kernel, KernelError, NoEnv, SystemState};

declare_id!("FzxnCtig4asxnCxGhjcbCDY5zkfcpfTb5GcXNL9UaGg6");

#[program]
pub mod program_v04 {
    use super::*;

    /// Probe: construct a kernel from the launch-pinned config and report
    /// its genesis state. Any kernel construction failure maps to
    /// `KernelProbe` — the instruction never panics on kernel input.
    pub fn probe_kernel(ctx: Context<ProbeKernel>) -> Result<()> {
        let _ = ctx.accounts.payer.key();
        let (sealed, now, index) =
            probe_kernel_state().map_err(|_| error!(ProgramV04Error::KernelProbe))?;
        msg!(
            "program-v04 probe: sealed={} now={} genesis_index={}",
            sealed,
            now,
            index
        );
        Ok(())
    }

    /// Genesis (§4.1, first row, v0.4 form): persist the probe-config
    /// kernel's genesis state into the `KernelState` PDA. One-time by
    /// construction — the `init` constraint refuses a second creation.
    /// The design's genesis row is a multisig-signed `initialize_config`;
    /// this slice's genesis authority is the initializing payer, and the
    /// multisig wiring is a later §4.1 slice, not claimed here. The
    /// account starts unsealed: the seal is slice 7 (see module docs).
    pub fn initialize_kernel(ctx: Context<InitializeKernel>) -> Result<()> {
        let mut state =
            genesis_kernel_state().map_err(|_| error!(ProgramV04Error::KernelInit))?;
        state.bump = ctx.bumps.kernel_state;
        let sealed = state.sealed;
        let head_index = state.head_index;
        *ctx.accounts.kernel_state = state;
        emit!(KernelInitialized { sealed, head_index });
        Ok(())
    }
}

#[derive(Accounts)]
pub struct ProbeKernel<'info> {
    pub payer: Signer<'info>,
}

#[derive(Accounts)]
pub struct InitializeKernel<'info> {
    #[account(
        init,
        payer = payer,
        space = KernelState::SPACE,
        seeds = [b"kernel-state"],
        bump,
    )]
    pub kernel_state: Account<'info, KernelState>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

/// The first §3 account: the on-chain kernel state. It carries the
/// design's `DeclaredChain` fields (`head_index`/`head_hash`) — the
/// LINEAGE anchor — alongside `sealed` and the kernel clock. Per-transition
/// kernel state (records, rounds, signals) is later slices' accounts, not
/// folded in here: this account is the chain head, nothing more.
#[account]
#[derive(Debug)]
pub struct KernelState {
    /// False until the slice-7 seal instruction lands (see module docs).
    pub sealed: bool,
    /// Declared-chain head index (genesis: 0).
    pub head_index: u64,
    /// Declared-chain head hash (genesis: the zero hash).
    pub head_hash: [u8; 32],
    /// Kernel clock at the last persisted state (genesis: 0).
    pub clock: u64,
    /// PDA bump for the `kernel-state` seeds.
    pub bump: u8,
}

impl KernelState {
    /// 8 (discriminator) + 1 + 8 + 32 + 8 + 1.
    pub const SPACE: usize = 58;
}

/// The genesis `KernelState`: the probe-config kernel's genesis values
/// (unsealed, head 0, zero hash, clock 0), with a zero bump — the
/// instruction overwrites the bump with the PDA bump.
pub fn genesis_kernel_state() -> core::result::Result<KernelState, KernelError> {
    let (sealed, now, index) = probe_kernel_state()?;
    Ok(KernelState { sealed, head_index: index, head_hash: [0; 32], clock: now, bump: 0 })
}

#[event]
pub struct KernelInitialized {
    pub sealed: bool,
    pub head_index: u64,
}

#[error_code]
pub enum ProgramV04Error {
    #[msg("program-v04 probe: kernel construction failed")]
    KernelProbe,
    #[msg("program-v04 initialize: kernel genesis failed")]
    KernelInit,
}

/// The launch-pinned probe config: reputation recorded but zero
/// influence (step-2 pin-OFF), 3 council seats (odd, within MAX_SEATS),
/// 100-tick epochs. This is the probe's config, not a program policy —
/// program policy arrives with the instruction families.
pub fn probe_config() -> Config {
    Config {
        rep_min: Amount::ZERO,
        rep_max: Amount::ONE,
        max_attribution: Amount::ONE,
        epoch_length: 100,
        council_seats: 3,
        reputation_weighs: false,
    }
}

/// Construct a kernel from `cfg` against the zero genesis state and
/// report (sealed, clock, declared index). The genesis admin is the zero
/// id: the probe owns no authority and installs no council.
pub fn probe_kernel_with_config(
    cfg: Config,
) -> core::result::Result<(bool, u64, u64), KernelError> {
    let k = Kernel::new(
        [0; 32],
        SystemState { index: 0, hash: [0; 32], timestamp: 0 },
        cfg,
        NoEnv,
    )?;
    Ok((k.is_sealed(), k.now(), k.declared_state().index))
}

/// The probe instruction's kernel state: unsealed, clock 0, genesis
/// index 0 under the launch-pinned config.
pub fn probe_kernel_state() -> core::result::Result<(bool, u64, u64), KernelError> {
    probe_kernel_with_config(probe_config())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_state_is_genesis_unsealed() {
        assert_eq!(probe_kernel_state(), Ok((false, 0, 0)));
    }

    #[test]
    fn even_seats_refused() {
        let mut cfg = probe_config();
        cfg.council_seats = 2;
        assert!(probe_kernel_with_config(cfg).is_err());
    }

    #[test]
    fn zero_seats_refused() {
        let mut cfg = probe_config();
        cfg.council_seats = 0;
        assert!(probe_kernel_with_config(cfg).is_err());
    }

    #[test]
    fn zero_epoch_refused() {
        let mut cfg = probe_config();
        cfg.epoch_length = 0;
        assert!(probe_kernel_with_config(cfg).is_err());
    }

    #[test]
    fn genesis_kernel_state_is_unsealed_genesis() {
        let state = genesis_kernel_state().expect("genesis must construct");
        assert!(!state.sealed);
        assert_eq!(state.head_index, 0);
        assert_eq!(state.head_hash, [0; 32]);
        assert_eq!(state.clock, 0);
    }

    #[test]
    fn kernel_state_space_accounts_for_every_field() {
        // Discriminator 8 + sealed 1 + head_index 8 + head_hash 32 +
        // clock 8 + bump 1.
        assert_eq!(KernelState::SPACE, 8 + 1 + 8 + 32 + 8 + 1);
    }

    #[test]
    fn kernel_state_borsh_round_trips() {
        use anchor_lang::{AccountDeserialize, AccountSerialize};
        let state = KernelState {
            sealed: false,
            head_index: 7,
            head_hash: [0xAB; 32],
            clock: 100,
            bump: 254,
        };
        let mut bytes = Vec::new();
        state.try_serialize(&mut bytes).expect("serialize must succeed");
        // Discriminator (8) + fields (50) = SPACE.
        assert_eq!(bytes.len(), KernelState::SPACE);
        let back = <KernelState as AccountDeserialize>::try_deserialize(&mut &bytes[..])
            .expect("deserialize must succeed");
        assert!(!back.sealed);
        assert_eq!(back.head_index, 7);
        assert_eq!(back.head_hash, [0xAB; 32]);
        assert_eq!(back.clock, 100);
        assert_eq!(back.bump, 254);
    }

    #[test]
    fn probe_kernel_seal_refused_without_launch_genesis() {
        // The probe kernel has no council, no governance policies, and
        // no share bound — seal_genesis must refuse, and the program
        // must not paper over that with an account-only seal flag.
        // Genesis admin is the zero id (see probe_kernel_with_config).
        let mut k = Kernel::new(
            [0; 32],
            SystemState { index: 0, hash: [0; 32], timestamp: 0 },
            probe_config(),
            NoEnv,
        )
        .expect("probe kernel must construct");
        assert!(matches!(
            k.seal_genesis(&[0; 32], 100),
            Err(KernelError::GenesisNotReady(_))
        ));
        assert!(!k.is_sealed());
    }
}
