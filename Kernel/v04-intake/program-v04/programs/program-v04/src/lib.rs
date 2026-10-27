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
//! Slice 6: the §4.1 genesis instruction — `initialize_kernel`
//! — and the first §3 account, `KernelState`. `KernelState` carries the
//! design's `DeclaredChain` fields (`head_index`/`head_hash`) alongside
//! `sealed`/`clock`; no standalone `DeclaredChain` account is split out
//! in this slice. The instruction constructs the real probe-config kernel
//! and persists its genesis state; it does not claim a seal.
//!
//! Slice 6 deliberately had NO `seal_kernel`, and why: the kernel's
//! `seal_genesis` refuses until the launch genesis is complete — a full
//! council, the four governance policies, and the influence-share bound
//! — and those genesis steps lived in the step-2 launch crate, which is
//! std-only (`std` collections, `vec!`) and cannot compile to SBF as-is.
//! An account-only seal flag would claim a seal the kernel never granted,
//! so no such flag was written.
//!
//! Slice 7 (this slice): the no_std port of the launch genesis steps,
//! and the `seal_kernel` instruction. The port replays the step-2 launch
//! profile exactly — same genesis authority, same policy values, same
//! six agents at base 2.0, same operator roles, same three capacity
//! stocks, same 3-seat council — but with `alloc` collections and
//! `Amount::from_raw` only (the step-2 crate's `from_f64` convenience is
//! std-only and is never used here), so the sequence compiles to SBF.
//! `seal_kernel` runs that sequence through the real kernel, seals it,
//! and persists the sealed flag, the kernel clock, and the declared head
//! into `KernelState`. It does NOT persist the kernel object itself:
//! the account records the seal outcome, and later slices' per-record
//! accounts are what let instructions continue from a sealed kernel.
//! Seal authority in this slice is the signing payer — the same gap
//! slice 6 recorded for genesis authority (the design's multisig-signed
//! `initialize_config` wiring is a later §4.1 slice, not claimed here).
//!
//! Slice 8 (this slice): the second §3 account, `TransitionRecord`,
//! and the first transition-lifecycle instruction, `register_transition`.
//! The record stores the manifest fields a later replay needs — id, type,
//! parent index/hash, declared next hash, observation id, expiry, proposer
//! — plus a `stage` byte (0 = Registered) that later slices advance.
//! The instruction gates on the sealed head: it refuses an unsealed
//! `KernelState` (`NotSealed`) and refuses any parent that is not exactly
//! the sealed head (`ParentMismatch`, index AND hash) — the bridge's
//! registration-time lineage refusal, against the account head rather
//! than a publisher's say-so. It does NOT claim kernel registration:
//! kernel `register_transition` additionally needs an ingested observation
//! and a registered-agent proposer, and the pubkey-to-agent mapping for
//! both is a later slice — so no kernel call is made here, and the record
//! is a stored manifest awaiting that replay, not a registered transition.

extern crate alloc;

use alloc::collections::BTreeSet;
use alloc::vec;
use anchor_lang::prelude::*;
use coordination_kernel::{
    Amount, Config, GovAction, Kernel, KernelError, NoEnv, Policy, Role, SystemState,
    T_GOV_ADJUSTABLE, T_GOV_CONSTITUTION, T_GOV_COUNCIL, T_GOV_REBOOT, SCALE,
};

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

    /// Seal (§4.1, second row, v0.4 form): run the full launch-genesis
    /// sequence through the real kernel, seal it, and persist the sealed
    /// flag, clock, and declared head into the `KernelState` PDA created
    /// by `initialize_kernel`. Refused `AlreadySealed` if the account
    /// already records a seal — a second seal never re-runs genesis.
    /// The kernel object itself is not persisted (see module docs);
    /// what the account records is the seal outcome, proven by a kernel
    /// that actually sealed. Seal authority is the signing payer; the
    /// multisig wiring is a later §4.1 slice.
    pub fn seal_kernel(ctx: Context<SealKernel>) -> Result<()> {
        let state = &mut ctx.accounts.kernel_state;
        apply_seal_to_state(state)?;
        emit!(KernelSealed { head_index: state.head_index, clock: state.clock });
        Ok(())
    }

    /// Register (§4.1, first lifecycle row, v0.4 form): store a transition
    /// manifest as a `TransitionRecord` PDA, gated on the sealed head.
    /// Refuses an unsealed kernel and any parent that is not exactly the
    /// sealed head; a second record for the same id is refused by the
    /// `init` constraint. This stores the manifest — it does not run
    /// kernel `register_transition` (see module docs: observation
    /// ingestion and the pubkey-to-agent proposer mapping are later
    /// slices, so a kernel call here would fake authority it does not
    /// have). Later slices advance `stage` from this record.
    #[allow(clippy::too_many_arguments)]
    pub fn register_transition(
        ctx: Context<RegisterTransition>,
        transition_id: [u8; 32],
        transition_type: [u8; 32],
        parent_index: u64,
        parent_hash: [u8; 32],
        declared_next_hash: [u8; 32],
        observation_id: [u8; 32],
        expires_at: u64,
    ) -> Result<()> {
        let record = build_transition_record(
            &ctx.accounts.kernel_state,
            transition_id,
            transition_type,
            parent_index,
            parent_hash,
            declared_next_hash,
            observation_id,
            expires_at,
            ctx.accounts.payer.key(),
            ctx.bumps.transition_record,
        )?;
        *ctx.accounts.transition_record = record;
        emit!(TransitionRegistered { transition_id, parent_index });
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(transition_id: [u8; 32])]
pub struct RegisterTransition<'info> {
    #[account(
        seeds = [b"kernel-state"],
        bump = kernel_state.bump,
    )]
    pub kernel_state: Account<'info, KernelState>,
    #[account(
        init,
        payer = payer,
        space = TransitionRecord::SPACE,
        seeds = [b"transition", transition_id.as_ref()],
        bump,
    )]
    pub transition_record: Account<'info, TransitionRecord>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SealKernel<'info> {
    #[account(
        mut,
        seeds = [b"kernel-state"],
        bump = kernel_state.bump,
    )]
    pub kernel_state: Account<'info, KernelState>,
    pub payer: Signer<'info>,
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
    /// False until `seal_kernel` seals it (slice 7; see module docs).
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

/// The second §3 account: one stored transition manifest. This is the
/// per-record account slice 7 deferred to: later replay slices rebuild
/// the sealed kernel and drive this record's lifecycle from these fields,
/// instead of trusting a publisher's summary. `stage` starts at
/// `REGISTERED` (0); later instructions advance it — no instruction in
/// this slice moves it past registration.
#[account]
#[derive(Debug)]
pub struct TransitionRecord {
    /// Transition id (the PDA seed after `"transition"`).
    pub transition_id: [u8; 32],
    /// Transition type (must have a policy in the sealed profile).
    pub transition_type: [u8; 32],
    /// Parent declared-chain index — must equal `KernelState.head_index`.
    pub parent_index: u64,
    /// Parent declared-chain hash — must equal `KernelState.head_hash`.
    pub parent_hash: [u8; 32],
    /// Declared next hash this transition proposes.
    pub declared_next_hash: [u8; 32],
    /// Observation this transition's manifest points at.
    pub observation_id: [u8; 32],
    /// Manifest expiry (kernel `BadWindow` backstop is a later slice).
    pub expires_at: u64,
    /// Signing payer that registered the record (not yet a kernel agent
    /// id — the pubkey-to-agent mapping is a later slice).
    pub proposer: Pubkey,
    /// Lifecycle stage: 0 = Registered (this slice's only stage).
    pub stage: u8,
    /// PDA bump for the `["transition", transition_id]` seeds.
    pub bump: u8,
}

impl TransitionRecord {
    /// Stage value written by `register_transition`.
    pub const REGISTERED: u8 = 0;
    /// 8 (discriminator) + 32 + 32 + 8 + 32 + 32 + 32 + 8 + 32 + 1 + 1.
    pub const SPACE: usize = 218;
}

/// Build a `TransitionRecord` against a `KernelState`, enforcing the
/// sealed-head lineage gate. Pure: a refused record leaves the state
/// untouched, and the caller (instruction or test) owns persistence.
#[allow(clippy::too_many_arguments)]
pub fn build_transition_record(
    kernel_state: &KernelState,
    transition_id: [u8; 32],
    transition_type: [u8; 32],
    parent_index: u64,
    parent_hash: [u8; 32],
    declared_next_hash: [u8; 32],
    observation_id: [u8; 32],
    expires_at: u64,
    proposer: Pubkey,
    bump: u8,
) -> Result<TransitionRecord> {
    if !kernel_state.sealed {
        return Err(error!(ProgramV04Error::NotSealed));
    }
    if parent_index != kernel_state.head_index || parent_hash != kernel_state.head_hash {
        return Err(error!(ProgramV04Error::ParentMismatch));
    }
    Ok(TransitionRecord {
        transition_id,
        transition_type,
        parent_index,
        parent_hash,
        declared_next_hash,
        observation_id,
        expires_at,
        proposer,
        stage: TransitionRecord::REGISTERED,
        bump,
    })
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

#[event]
pub struct KernelSealed {
    pub head_index: u64,
    pub clock: u64,
}

#[event]
pub struct TransitionRegistered {
    pub transition_id: [u8; 32],
    pub parent_index: u64,
}

#[error_code]
pub enum ProgramV04Error {
    #[msg("program-v04 probe: kernel construction failed")]
    KernelProbe,
    #[msg("program-v04 initialize: kernel genesis failed")]
    KernelInit,
    #[msg("program-v04 seal: launch-genesis sequence or seal failed")]
    KernelSeal,
    #[msg("program-v04 seal: kernel state already sealed")]
    AlreadySealed,
    #[msg("program-v04 register: kernel state is not sealed")]
    NotSealed,
    #[msg("program-v04 register: parent is not the sealed head")]
    ParentMismatch,
}

// ---------------------------------------------------------------- launch genesis
// The step-2 launch profile, ported to no_std: `alloc` collections and
// `Amount::from_raw` only. Values are pinned to the step-2 crate —
// `launch_config_matches_step2_profile` asserts the config half — so a
// drift between the host profile and the on-chain sequence fails a test
// instead of silently forking the genesis.

/// Genesis authority (retired by the seal).
pub const GEN: [u8; 32] = [0xA0; 32];
/// Observer (posts observations).
pub const OBSERVER: [u8; 32] = [0xA1; 32];
/// Executor (starts execution).
pub const EXECUTOR: [u8; 32] = [0xB1; 32];
/// Settlers (attest realized hashes; threshold 2).
pub const SETTLER_A: [u8; 32] = [0xB2; 32];
/// Settlers (attest realized hashes; threshold 2).
pub const SETTLER_B: [u8; 32] = [0xB3; 32];
/// Outcome recorder (records attribution after settlement).
pub const RECORDER: [u8; 32] = [0xB4; 32];

/// Ordinary transition type used by the launch profile.
pub const TT: [u8; 32] = [9; 32];
/// Required constraint id for TT.
pub const C1: [u8; 32] = [5; 32];
/// Required objective id for TT.
pub const O1: [u8; 32] = [6; 32];

/// Monthly distribution tranches (mirrors v0.2 `kernel-policy` id 0).
pub const RES_TREASURY_TRANCHE: u16 = 0;
/// The secured lending book (mirrors v0.2 `kernel-policy` id 1).
pub const RES_LENDING_BOOK: u16 = 1;
/// Rotational exit windows (mirrors v0.2 `kernel-policy` id 2).
pub const RES_EXIT_WINDOW: u16 = 2;

/// Agent ids 1..=6 for the seeded agent set.
pub fn ag(n: u8) -> [u8; 32] {
    [n; 32]
}

/// The step-2 launch configuration, in `from_raw` form: reputation
/// 0..100, attribution capped at ±20, 1000-tick epochs, a 3-seat council,
/// reputation recorded but zero influence (pin-OFF).
pub fn launch_config() -> Config {
    Config {
        rep_min: Amount::from_raw(0),
        rep_max: Amount::from_raw(100 * SCALE),
        max_attribution: Amount::from_raw(20 * SCALE),
        epoch_length: 1000,
        council_seats: 3,
        reputation_weighs: false,
    }
}

/// Policy shared by every type in the launch profile (same shape as the
/// step-2 crate's test policy).
pub fn launch_policy() -> Policy {
    Policy {
        quorum_required: Amount::ONE,
        confidence_threshold: Amount::from_raw(SCALE / 2),
        min_obs_confidence: Amount::from_raw(SCALE / 2),
        require_objectives: false,
        consensus_window: 100,
        planning_window: 100,
        execution_window: 1000,
        max_plans: 6,
        settlement_threshold: 2,
        max_rounds: 3,
    }
}

/// The launch kernel type: the delivered crate under `NoEnv`. Genesis
/// steps never consult the environment — the launch crate's env answers
/// (C1 = Pass, O1 = true) are only needed once transitions are evaluated,
/// not while rules are seeded — so `NoEnv` is exact here, not a shortcut.
pub type LaunchKernelNoEnv = Kernel<NoEnv>;

/// Fresh kernel at genesis time `t = 100`, genesis authority GEN.
pub fn fresh_launch_kernel() -> core::result::Result<LaunchKernelNoEnv, KernelError> {
    Kernel::new(
        GEN,
        SystemState { index: 0, hash: [0; 32], timestamp: 100 },
        launch_config(),
        NoEnv,
    )
}

/// Genesis seeding: policies for TT and all four governance types, the
/// required constraint/objective sets for TT, six agents at base 2.0,
/// the operator roles, and the three capacity stocks. Deliberately NOT
/// the council or the seal, so tests can assert the seal's readiness
/// gates compositionally (see `seal_refused_without_council`).
pub fn apply_launch_genesis(
    k: &mut LaunchKernelNoEnv,
) -> core::result::Result<(), KernelError> {
    let policy = launch_policy();
    k.genesis_apply(&GEN, GovAction::SetPolicy { ttype: TT, policy: policy.clone() }, 100)?;
    for t in [T_GOV_ADJUSTABLE, T_GOV_CONSTITUTION, T_GOV_COUNCIL, T_GOV_REBOOT] {
        k.genesis_apply(&GEN, GovAction::SetPolicy { ttype: t, policy: policy.clone() }, 100)?;
    }
    k.genesis_apply(
        &GEN,
        GovAction::SetRequiredConstraints { ttype: TT, set: BTreeSet::from([C1]) },
        100,
    )?;
    k.genesis_apply(
        &GEN,
        GovAction::SetRequiredObjectives { ttype: TT, set: BTreeSet::from([O1]) },
        100,
    )?;
    for n in 1..=6u8 {
        k.genesis_apply(
            &GEN,
            GovAction::RegisterAgent {
                agent: ag(n),
                groups: BTreeSet::from([1u16]),
                base_influence: Amount::from_raw(2 * SCALE),
            },
            100,
        )?;
    }
    for (who, role) in [
        (OBSERVER, Role::Observer),
        (EXECUTOR, Role::Executor),
        (SETTLER_A, Role::Settler),
        (SETTLER_B, Role::Settler),
        (RECORDER, Role::OutcomeRecorder),
    ] {
        k.genesis_apply(&GEN, GovAction::GrantRole { who, role }, 100)?;
    }
    for (resource, capacity) in [
        (RES_TREASURY_TRANCHE, 1000 * SCALE),
        (RES_LENDING_BOOK, 500 * SCALE),
        (RES_EXIT_WINDOW, 250 * SCALE),
    ] {
        k.genesis_apply(
            &GEN,
            GovAction::SetCapacity { resource, capacity: Amount::from_raw(capacity) },
            100,
        )?;
    }
    Ok(())
}

/// Install the 3-seat launch council (seats 1, 2, 3).
pub fn install_launch_council(
    k: &mut LaunchKernelNoEnv,
) -> core::result::Result<(), KernelError> {
    k.genesis_apply(
        &GEN,
        GovAction::InstallCouncil { seats: vec![ag(1), ag(2), ag(3)] },
        100,
    )
}

/// The sealed launch kernel: fresh kernel, all genesis rules, council,
/// seal. This is the sequence `seal_kernel` runs on-chain.
pub fn sealed_launch_kernel() -> core::result::Result<LaunchKernelNoEnv, KernelError> {
    let mut k = fresh_launch_kernel()?;
    apply_launch_genesis(&mut k)?;
    install_launch_council(&mut k)?;
    k.seal_genesis(&GEN, 100)?;
    Ok(k)
}

/// The instruction-level seal: refuse an already-sealed account, else
/// run the sealed launch sequence and persist its outcome (sealed flag,
/// kernel clock, declared head) into the account. The kernel object is
/// not persisted — see the module docs.
pub fn apply_seal_to_state(state: &mut KernelState) -> Result<()> {
    if state.sealed {
        return Err(error!(ProgramV04Error::AlreadySealed));
    }
    let k = sealed_launch_kernel().map_err(|_| error!(ProgramV04Error::KernelSeal))?;
    state.sealed = true;
    state.clock = k.now();
    let declared = k.declared_state();
    state.head_index = declared.index;
    state.head_hash = declared.hash;
    Ok(())
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
    fn launch_config_matches_step2_profile() {
        let cfg = launch_config();
        assert_eq!(cfg.rep_min, Amount::from_raw(0));
        assert_eq!(cfg.rep_max, Amount::from_raw(100 * SCALE));
        assert_eq!(cfg.max_attribution, Amount::from_raw(20 * SCALE));
        assert_eq!(cfg.epoch_length, 1000);
        assert_eq!(cfg.council_seats, 3);
        assert!(!cfg.reputation_weighs);
    }

    #[test]
    fn sealed_launch_kernel_is_sealed_at_genesis_head() {
        let k = sealed_launch_kernel().expect("launch sequence must seal");
        assert!(k.is_sealed());
        assert_eq!(k.now(), 100);
        assert_eq!(k.declared_state().index, 0);
        assert_eq!(k.declared_state().hash, [0; 32]);
    }

    #[test]
    fn seal_refused_without_council() {
        // Genesis rules without the council: the seal's COUNCIL gate
        // must refuse, proving the instruction cannot seal a half-built
        // genesis even if the caller skips the council step.
        let mut k = fresh_launch_kernel().expect("fresh launch kernel must construct");
        apply_launch_genesis(&mut k).expect("genesis rules must apply");
        assert!(matches!(
            k.seal_genesis(&GEN, 100),
            Err(KernelError::GenesisNotReady("COUNCIL"))
        ));
        assert!(!k.is_sealed());
    }

    #[test]
    fn sealed_kernel_reseal_refused() {
        // After the seal the genesis authority is erased, so a second
        // seal is refused as NotGenesis — the kernel itself, not just
        // the account guard, bars a second seal.
        let mut k = sealed_launch_kernel().expect("launch sequence must seal");
        assert!(matches!(k.seal_genesis(&GEN, 100), Err(KernelError::NotGenesis)));
        assert!(k.is_sealed());
    }

    #[test]
    fn apply_seal_to_state_seals_genesis_state() {
        let mut state = genesis_kernel_state().expect("genesis must construct");
        assert!(!state.sealed);
        apply_seal_to_state(&mut state).expect("seal must apply");
        assert!(state.sealed);
        assert_eq!(state.clock, 100);
        assert_eq!(state.head_index, 0);
        assert_eq!(state.head_hash, [0; 32]);
    }

    #[test]
    fn apply_seal_to_state_refuses_when_sealed() {
        let mut state = genesis_kernel_state().expect("genesis must construct");
        state.sealed = true;
        state.clock = 999;
        let err = apply_seal_to_state(&mut state).expect_err("second seal must refuse");
        assert_eq!(state.clock, 999, "a refused seal must not move the account");
        // The error is the AlreadySealed program error, not a kernel failure.
        assert!(format!("{:?}", err).contains("AlreadySealed"));
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

    fn sealed_state() -> KernelState {
        let mut state = genesis_kernel_state().expect("genesis must construct");
        apply_seal_to_state(&mut state).expect("seal must apply");
        state
    }

    fn record_args() -> ([u8; 32], [u8; 32], u64, [u8; 32], [u8; 32], [u8; 32], u64) {
        ([0xD1; 32], TT, 0, [0; 32], [0xE1; 32], [0xC0; 32], 10_100)
    }

    #[test]
    fn transition_record_space_accounts_for_every_field() {
        // Discriminator 8 + id 32 + type 32 + parent_index 8 +
        // parent_hash 32 + declared 32 + observation 32 + expires 8 +
        // proposer 32 + stage 1 + bump 1.
        assert_eq!(
            TransitionRecord::SPACE,
            8 + 32 + 32 + 8 + 32 + 32 + 32 + 8 + 32 + 1 + 1
        );
    }

    #[test]
    fn transition_record_borsh_round_trips() {
        use anchor_lang::{AccountDeserialize, AccountSerialize};
        let record = TransitionRecord {
            transition_id: [0xD1; 32],
            transition_type: TT,
            parent_index: 3,
            parent_hash: [0xAA; 32],
            declared_next_hash: [0xE1; 32],
            observation_id: [0xC0; 32],
            expires_at: 10_100,
            proposer: Pubkey::new_from_array([0x99; 32]),
            stage: TransitionRecord::REGISTERED,
            bump: 254,
        };
        let mut bytes = Vec::new();
        record.try_serialize(&mut bytes).expect("serialize must succeed");
        assert_eq!(bytes.len(), TransitionRecord::SPACE);
        let back = <TransitionRecord as AccountDeserialize>::try_deserialize(&mut &bytes[..])
            .expect("deserialize must succeed");
        assert_eq!(back.transition_id, [0xD1; 32]);
        assert_eq!(back.parent_index, 3);
        assert_eq!(back.parent_hash, [0xAA; 32]);
        assert_eq!(back.stage, 0);
        assert_eq!(back.bump, 254);
    }

    #[test]
    fn build_transition_record_refuses_unsealed() {
        let state = genesis_kernel_state().expect("genesis must construct");
        let (id, ttype, parent_index, parent_hash, declared, obs, expires) = record_args();
        let err = build_transition_record(
            &state, id, ttype, parent_index, parent_hash, declared, obs, expires,
            Pubkey::new_from_array([0x99; 32]), 1,
        )
        .expect_err("unsealed register must refuse");
        assert!(format!("{:?}", err).contains("NotSealed"));
    }

    #[test]
    fn build_transition_record_refuses_index_mismatch() {
        let state = sealed_state();
        let (id, ttype, _, parent_hash, declared, obs, expires) = record_args();
        let err = build_transition_record(
            &state, id, ttype, 1, parent_hash, declared, obs, expires,
            Pubkey::new_from_array([0x99; 32]), 1,
        )
        .expect_err("index mismatch must refuse");
        assert!(format!("{:?}", err).contains("ParentMismatch"));
    }

    #[test]
    fn build_transition_record_refuses_hash_mismatch() {
        let state = sealed_state();
        let (id, ttype, parent_index, _, declared, obs, expires) = record_args();
        let err = build_transition_record(
            &state, id, ttype, parent_index, [0xAB; 32], declared, obs, expires,
            Pubkey::new_from_array([0x99; 32]), 1,
        )
        .expect_err("hash mismatch must refuse");
        assert!(format!("{:?}", err).contains("ParentMismatch"));
    }

    #[test]
    fn build_transition_record_accepts_sealed_genesis_head() {
        let state = sealed_state();
        assert!(state.sealed);
        assert_eq!(state.head_index, 0);
        assert_eq!(state.head_hash, [0; 32]);
        let (id, ttype, parent_index, parent_hash, declared, obs, expires) = record_args();
        let record = build_transition_record(
            &state, id, ttype, parent_index, parent_hash, declared, obs, expires,
            Pubkey::new_from_array([0x99; 32]), 7,
        )
        .expect("sealed-head register must build");
        assert_eq!(record.transition_id, [0xD1; 32]);
        assert_eq!(record.transition_type, TT);
        assert_eq!(record.parent_index, 0);
        assert_eq!(record.declared_next_hash, [0xE1; 32]);
        assert_eq!(record.stage, TransitionRecord::REGISTERED);
        assert_eq!(record.stage, 0);
        assert_eq!(record.bump, 7);
    }
}
