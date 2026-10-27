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
}

#[derive(Accounts)]
pub struct ProbeKernel<'info> {
    pub payer: Signer<'info>,
}

#[error_code]
pub enum ProgramV04Error {
    #[msg("program-v04 probe: kernel construction failed")]
    KernelProbe,
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
}
