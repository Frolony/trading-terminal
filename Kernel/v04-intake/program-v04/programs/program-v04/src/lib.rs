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
//!
//! Slice 9 (this slice): the third §3 account, `ObservationRecord`,
//! the `ingest_observation` instruction, and the pubkey-to-agent mapping
//! slice 8 deferred. The mapping rule is identity, not a registry: kernel
//! agent ids are 32-byte key identities, so a signer's agent id IS the
//! signer's pubkey bytes — the signer proves the agent by signing, and no
//! account invents authority a signature did not grant. `resolve_proposer`
//! gates that identity against the sealed launch profile (the agent must
//! be registered and active there; the Observer-role holder is NOT a
//! registered agent and is refused as a proposer, and an unknown pubkey is
//! refused outright), and `ingest_observation` gates the observer the same
//! way (the agent must hold the Observer role in the sealed launch
//! profile). Both gates replay the sealed launch kernel — the same
//! genesis constants slice 7 seals — so they judge the pinned profile,
//! not a caller-supplied roster; a governance-changed roster is a later
//! slice's replay scope, not claimed here. The observation builder mirrors
//! the kernel's ingest rules against `KernelState.clock` (timestamp at or
//! before the clock, expiry after it, confidence at or below SCALE); at the
//! sealed genesis clock of 100 that admits the bridge's t=100 observations
//! and refuses anything from the future, so clock advancement is a later
//! slice before post-genesis observations can ingest. Like slice 8, the
//! instruction stores the record — it does not run kernel
//! `ingest_observation`, because the kernel object itself is not persisted
//! — and the later replay slice runs the kernel call from the stored
//! record. What slice 9 does prove, in a combined host test, is that the
//! resolved ids actually drive kernel registration: the observer pubkey
//! ingests and the proposer pubkey registers on a sealed kernel.
//!
//! Slice 10 (this slice): the first replay instruction,
//! `replay_transition`. It runs kernel `ingest_observation` +
//! `register_transition` from a stored `ObservationRecord` +
//! `TransitionRecord` pair against a freshly rebuilt sealed launch
//! kernel, and advances the transition record's `stage` from
//! `REGISTERED` (0) to `KERNEL_REGISTERED` (1). The kernel's manifest
//! needs `relevant_groups`, which the stored record does not carry, so
//! slice 10 derives them from the sealed profile: the proposer's own
//! groups there (every seeded agent is in group 1, matching the bridge's
//! ordinary record). That derivation is this slice's rule, recorded
//! here, not a kernel rule — a transition needing groups beyond its
//! proposer's is a later manifest-completion slice's work, and it must
//! store those groups in the record rather than derive them.
//! Replay creates no new authority: it executes steps the stored
//! records already authorized (slice 8's register signature, slice 9's
//! ingest signature), judged against the sealed profile, and the
//! instruction's payer is a fee payer only. Replay is once-only per
//! record (`WrongStage` on a second replay) because the rebuilt kernel
//! is fresh each time and cannot itself refuse a duplicate — the stage
//! byte is the duplicate guard, not the kernel. And replay proves
//! registration only: the kernel is rebuilt at the genesis head, so a
//! replayed transition is `Proposed` against head 0 and `KernelState`
//! does not advance; sequencing past the genesis head (commit replay
//! moving the head) and the evaluate/eligible/select/reconcile/
//! validate/declare steps are later slices, not claimed here. Clock
//! advancement is still a later slice: replay runs at
//! `KernelState.clock` (100 at genesis), so only genesis-clock pairs
//! replay.
//!
//! Slice 11 (this slice): the clock instruction, `advance_clock`,
//! closing the slice-9/10 limitation that only genesis-clock pairs
//! could ingest or replay. `KernelState.clock` is the `now` later
//! instructions pass to the freshly rebuilt kernel (the kernel's own
//! `tick` is private, so the program's clock is the advancement
//! surface, not a kernel call). The helper enforces sealed state and
//! forward-only movement — a regression is refused as
//! `ClockRegression`, an equal value is an accepted no-op mirroring
//! `tick` — and never touches the head or the seal flag: clock
//! advancement is not chain advancement, and only a later commit
//! replay moves the head. Authority is the signing payer, the same
//! explicitly recorded gap as slices 6/7 (multisig/role wiring is a
//! later §4.1 slice); advancing the clock by itself moves no money
//! and expires nothing on-chain — expiry bites only when a later
//! instruction replays against the new clock, which the tests prove
//! both ways (a later observation ingests; a window lapsed by the
//! advance fails replay through the kernel). No CU measurement —
//! that rides the cu-golden freeze.
//!
//! Slice 12 (this slice): the second replay instruction,
//! `evaluate_transition`. It takes a `KERNEL_REGISTERED` record and
//! runs kernel `evaluate_transition` on it — after replaying the
//! slice-10 ingest + register sequence on a freshly rebuilt sealed
//! launch kernel, because the kernel object itself is not persisted
//! and evaluation needs a `Proposed` transition to judge. The kernel
//! here runs under `ProgramEnv`, the program-side port of the step-2
//! launch env (C1 = Pass, O1 = true): under the delivered crate's
//! default `NoEnv`, C1 answers Unavailable and `evaluate_transition`
//! leaves every transition `Proposed` — constraints recorded
//! unsatisfied, life unmoved — so a NoEnv evaluate could never advance
//! a stage and this slice would be a no-op (proven by test, not
//! assumed). The env answers are the administrator's answers, baked
//! into the program, never taken from an instruction argument: the
//! kernel asks the env, callers never assert a verdict. Only
//! `Life::Evaluated` advances the record's stage, to `EVALUATED` (2);
//! a kernel refusal, and a non-Evaluated life (a `Rejected` verdict
//! under a failing env, or a still-`Proposed` transition under an
//! unavailable one), both map to `EvaluateFailed` and leave the stage
//! untouched — the stage records only a proven evaluation, and a
//! verdict-bearing `Rejected` stage is a later slice's work, not claimed
//! here. Eligible/select/reconcile/validate/declare, commit replay
//! moving the head, and CU measurement are later slices, not this one.
//!
//! Slice 13 (this slice): the third replay instruction,
//! `mark_eligible`. It takes an `EVALUATED` record and runs kernel
//! `mark_eligible` on it — after replaying the ingest + register +
//! evaluate sequence on a freshly rebuilt sealed launch kernel under
//! `ProgramEnv`, because the kernel object itself is not persisted and
//! eligibility needs an `Evaluated` life whose `constraints_ok` /
//! `objectives_ok` flags evaluate recorded. Only `Life::Eligible`
//! advances the record's stage, to `ELIGIBLE` (3); a kernel refusal,
//! and a non-Eligible life, both map to `EligibleFailed` and leave the
//! stage untouched. The feasibility gate is the kernel's, not a
//! program guess: `constraints_ok` must be true (evaluate under
//! `ProgramEnv` records it, since C1 passes there), and the objectives
//! gate binds only when the policy requires objectives — the launch
//! policy's `require_objectives` is false, so that check is inert for
//! this profile and is not claimed as a gate here. Observation
//! freshness and manifest expiry are re-checked by the kernel at this
//! step, so a window lapsed since evaluation fails here (proven by
//! test at the advanced clock), not silently carried forward. Honesty
//! bound, stated not smoothed: the sibling field `mark_eligible`
//! joins lives only inside the freshly rebuilt kernel and is not
//! persisted — the program records the stage only, not
//! `eligible_at` / `consensus_deadline` — so a later slice must persist
//! those fields before `select_candidate` can be proven against a real
//! field rather than a fresh one. Select/reconcile/validate/declare,
//! commit replay moving the head, and CU measurement are later slices,
//! not this one.
//!
//! Slice 14 (this slice): the persistence slice 13 named —
//! `TransitionRecord` gains `eligible_at` / `consensus_deadline`, and
//! `mark_eligible_from_records` writes them on success (the state's
//! clock, and that clock plus the sealed launch policy's
//! `consensus_window` of 100). No new instruction: the `mark_eligible`
//! instruction's meaning grows by exactly this persistence, and its
//! stage gate is unchanged. The values are derived from the same inputs
//! the kernel's own `mark_eligible` used — the delivered crate exposes
//! no `eligible_at` accessor, so the program derives rather than
//! reads, and the derivation is checked by test at two clocks (100 →
//! 100/200, 200 → 200/300), not assumed. A refusal still leaves both
//! fields at 0, so 0 means "eligibility never proven" and can never be
//! mistaken for a window. What slice 14 deliberately does NOT close:
//! the signal tally — `select_candidate` also needs persisted round
//! signals to rebuild a quorum, and no signal record exists yet, so a
//! select instruction this slice would be an instruction that can only
//! refuse, the no-op pattern slice 12 rejected. Select therefore stays
//! a later slice, now unblocked on the window field and still blocked
//! on signal persistence.

extern crate alloc;

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec;
use anchor_lang::prelude::*;
use coordination_kernel::{
    Amount, Config, ConstraintStatus, Env, GovAction, Kernel, KernelError, Life, Manifest, NoEnv,
    Observation, Policy, Role, SystemState, T_GOV_ADJUSTABLE, T_GOV_CONSTITUTION,
    T_GOV_COUNCIL, T_GOV_REBOOT, SCALE,
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

    /// Ingest (§4.1, second lifecycle row, v0.4 form): store an
    /// observation as an `ObservationRecord` PDA, gated on the sealed
    /// state, the Observer role of the signing payer's agent id, and the
    /// kernel's ingest window rules against the state's clock. This
    /// stores the observation — it does not run kernel
    /// `ingest_observation` (see module docs: the kernel object is not
    /// persisted); the later replay slice runs the kernel call from this
    /// stored record. A second record for the same id is refused by the
    /// `init` constraint.
    pub fn ingest_observation(
        ctx: Context<IngestObservation>,
        observation_id: [u8; 32],
        timestamp: u64,
        expiry: u64,
        confidence_raw: u64,
    ) -> Result<()> {
        let record = build_observation_record(
            &ctx.accounts.kernel_state,
            observation_id,
            timestamp,
            expiry,
            confidence_raw,
            ctx.accounts.payer.key(),
            ctx.bumps.observation_record,
        )?;
        *ctx.accounts.observation_record = record;
        emit!(ObservationIngested { observation_id, observer: ctx.accounts.payer.key(), timestamp });
        Ok(())
    }

    /// Replay (§4.1, first replay row, v0.4 form): run kernel
    /// `ingest_observation` + `register_transition` from the stored
    /// `ObservationRecord` + `TransitionRecord` pair and advance the
    /// transition record's stage to `KERNEL_REGISTERED`. The PDA seeds
    /// bind the accounts to the instruction's ids, and the body re-checks
    /// the stored ids against them; the helper enforces the sealed
    /// state, the once-only stage, the sealed-head parent, the
    /// observation linkage, and the sealed-profile proposer/observer
    /// gates, then drives the real kernel calls (see module docs for
    /// the relevant-groups derivation and the genesis-head-only scope).
    /// The payer is a fee payer only — replay executes steps the stored
    /// records already authorized and creates no new authority.
    pub fn replay_transition(
        ctx: Context<ReplayTransition>,
        transition_id: [u8; 32],
        observation_id: [u8; 32],
    ) -> Result<()> {
        if ctx.accounts.transition_record.transition_id != transition_id
            || ctx.accounts.observation_record.observation_id != observation_id
        {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        replay_transition_from_records(
            &ctx.accounts.kernel_state,
            &mut ctx.accounts.transition_record,
            &ctx.accounts.observation_record,
        )?;
        emit!(TransitionReplayed {
            transition_id,
            stage: TransitionRecord::KERNEL_REGISTERED,
        });
        Ok(())
    }

    /// Advance (§4.1, clock row, v0.4 form): move `KernelState.clock`
    /// forward to `new_clock`. Sealed-state and forward-only gates
    /// live in `advance_clock_on_state`; the instruction only persists
    /// the result and emits the event. Authority in this slice is the
    /// signing payer — the same gap slices 6/7 record for genesis and
    /// seal authority (multisig/role wiring is a later §4.1 slice, not
    /// claimed here). Advancing the clock moves no money and advances
    /// no head: it changes the `now` later instructions pass to the
    /// rebuilt kernel, which is what unblocks post-genesis observations
    /// and what lets windows expire.
    pub fn advance_clock(ctx: Context<AdvanceClock>, new_clock: u64) -> Result<()> {
        let clock = advance_clock_on_state(&mut ctx.accounts.kernel_state, new_clock)?;
        emit!(ClockAdvanced { clock });
        Ok(())
    }

    /// Evaluate (§4.1, second replay row, v0.4 form): run kernel
    /// `evaluate_transition` on a `KERNEL_REGISTERED` record pair and
    /// advance the transition record's stage to `EVALUATED`. The PDA
    /// seeds bind the accounts to the instruction's ids, and the body
    /// re-checks the stored ids against them; the helper enforces the
    /// sealed state, the stage gate, the sealed-head parent, the
    /// observation linkage, and the sealed-profile proposer/observer
    /// gates, then drives the real kernel calls under `ProgramEnv`
    /// (see module docs for why NoEnv cannot serve this step). The
    /// payer is a fee payer only — evaluation executes steps the
    /// stored records already authorized and creates no new authority.
    pub fn evaluate_transition(
        ctx: Context<EvaluateTransition>,
        transition_id: [u8; 32],
        observation_id: [u8; 32],
    ) -> Result<()> {
        if ctx.accounts.transition_record.transition_id != transition_id
            || ctx.accounts.observation_record.observation_id != observation_id
        {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        evaluate_transition_from_records(
            &ctx.accounts.kernel_state,
            &mut ctx.accounts.transition_record,
            &ctx.accounts.observation_record,
        )?;
        emit!(TransitionEvaluated { transition_id, stage: TransitionRecord::EVALUATED });
        Ok(())
    }

    /// Mark eligible (§4.1, third replay row, v0.4 form): run kernel
    /// `mark_eligible` on an `EVALUATED` record pair and advance the
    /// transition record's stage to `ELIGIBLE`. The PDA seeds bind the
    /// accounts to the instruction's ids, and the body re-checks the
    /// stored ids against them; the helper enforces the sealed state,
    /// the stage gate, the sealed-head parent, the observation linkage,
    /// and the sealed-profile proposer/observer gates, then drives the
    /// real kernel calls under `ProgramEnv` (see module docs for why
    /// the ingest + register + evaluate sequence must be replayed
    /// first). On success the helper also persists the proven window
    /// (`eligible_at` / `consensus_deadline`, slice 14) into the record,
    /// so a later select slice reads a real field. The payer is a fee
    /// payer only — eligibility executes steps the stored records
    /// already authorized and creates no new authority.
    pub fn mark_eligible(
        ctx: Context<MarkEligible>,
        transition_id: [u8; 32],
        observation_id: [u8; 32],
    ) -> Result<()> {
        if ctx.accounts.transition_record.transition_id != transition_id
            || ctx.accounts.observation_record.observation_id != observation_id
        {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        mark_eligible_from_records(
            &ctx.accounts.kernel_state,
            &mut ctx.accounts.transition_record,
            &ctx.accounts.observation_record,
        )?;
        emit!(TransitionMarkedEligible { transition_id, stage: TransitionRecord::ELIGIBLE });
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(transition_id: [u8; 32], observation_id: [u8; 32])]
pub struct EvaluateTransition<'info> {
    #[account(
        seeds = [b"kernel-state"],
        bump = kernel_state.bump,
    )]
    pub kernel_state: Account<'info, KernelState>,
    #[account(
        mut,
        seeds = [b"transition", transition_id.as_ref()],
        bump = transition_record.bump,
    )]
    pub transition_record: Account<'info, TransitionRecord>,
    #[account(
        seeds = [b"observation", observation_id.as_ref()],
        bump = observation_record.bump,
    )]
    pub observation_record: Account<'info, ObservationRecord>,
    pub payer: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(transition_id: [u8; 32], observation_id: [u8; 32])]
pub struct MarkEligible<'info> {
    #[account(
        seeds = [b"kernel-state"],
        bump = kernel_state.bump,
    )]
    pub kernel_state: Account<'info, KernelState>,
    #[account(
        mut,
        seeds = [b"transition", transition_id.as_ref()],
        bump = transition_record.bump,
    )]
    pub transition_record: Account<'info, TransitionRecord>,
    #[account(
        seeds = [b"observation", observation_id.as_ref()],
        bump = observation_record.bump,
    )]
    pub observation_record: Account<'info, ObservationRecord>,
    pub payer: Signer<'info>,
}

#[derive(Accounts)]
pub struct AdvanceClock<'info> {
    #[account(
        mut,
        seeds = [b"kernel-state"],
        bump = kernel_state.bump,
    )]
    pub kernel_state: Account<'info, KernelState>,
    pub payer: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(transition_id: [u8; 32], observation_id: [u8; 32])]
pub struct ReplayTransition<'info> {
    #[account(
        seeds = [b"kernel-state"],
        bump = kernel_state.bump,
    )]
    pub kernel_state: Account<'info, KernelState>,
    #[account(
        mut,
        seeds = [b"transition", transition_id.as_ref()],
        bump = transition_record.bump,
    )]
    pub transition_record: Account<'info, TransitionRecord>,
    #[account(
        seeds = [b"observation", observation_id.as_ref()],
        bump = observation_record.bump,
    )]
    pub observation_record: Account<'info, ObservationRecord>,
    pub payer: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(observation_id: [u8; 32])]
pub struct IngestObservation<'info> {
    #[account(
        seeds = [b"kernel-state"],
        bump = kernel_state.bump,
    )]
    pub kernel_state: Account<'info, KernelState>,
    #[account(
        init,
        payer = payer,
        space = ObservationRecord::SPACE,
        seeds = [b"observation", observation_id.as_ref()],
        bump,
    )]
    pub observation_record: Account<'info, ObservationRecord>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
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
    /// Lifecycle stage: 0 = Registered, 1 = Kernel-registered (slice 10
    /// advances it), 2 = Evaluated (slice 12 advances it), 3 = Eligible
    /// (slice 13 advances it; no instruction moves it past 3 in this slice).
    pub stage: u8,
    /// Kernel clock at which `mark_eligible` proved eligibility (slice 14
    /// persists it; 0 until eligibility is proven). This is the
    /// `eligible_at` the kernel's `select_candidate` window runs from —
    /// persisted here so a later select slice reads a real field, not a
    /// freshly rebuilt one (slice 13's named honesty gap, closed here).
    pub eligible_at: u64,
    /// Consensus deadline `eligible_at + consensus_window` of the sealed
    /// launch policy (slice 14 persists it alongside `eligible_at`).
    /// 0 until eligibility is proven.
    pub consensus_deadline: u64,
    /// PDA bump for the `["transition", transition_id]` seeds.
    pub bump: u8,
}

impl TransitionRecord {
    /// Stage value written by `register_transition`.
    pub const REGISTERED: u8 = 0;
    /// Stage value written by `replay_transition` (slice 10).
    pub const KERNEL_REGISTERED: u8 = 1;
    /// Stage value written by `evaluate_transition` (slice 12).
    pub const EVALUATED: u8 = 2;
    /// Stage value written by `mark_eligible` (slice 13).
    pub const ELIGIBLE: u8 = 3;
    /// 8 (discriminator) + 32 + 32 + 8 + 32 + 32 + 32 + 8 + 32 + 1 + 8 + 8 + 1.
    pub const SPACE: usize = 234;
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
        eligible_at: 0,
        consensus_deadline: 0,
        bump,
    })
}

/// The third §3 account: one stored observation. This is the record the
/// later replay slice feeds to kernel `ingest_observation` — the fields
/// are exactly the kernel's `Observation` fields plus the observing
/// signer's pubkey and the PDA bump. No stage byte: an observation is
/// either stored (ingested, in this slice's sense) or absent.
#[account]
#[derive(Debug)]
pub struct ObservationRecord {
    /// Observation id (the PDA seed after `"observation"`).
    pub observation_id: [u8; 32],
    /// Observation timestamp — at or before `KernelState.clock`.
    pub timestamp: u64,
    /// Observation expiry — after `KernelState.clock`.
    pub expiry: u64,
    /// Confidence in kernel `Amount` raw units (SCALE = 1.0).
    pub confidence_raw: u64,
    /// Signing payer that ingested the record (its agent id, by the
    /// identity mapping, must hold the Observer role).
    pub observer: Pubkey,
    /// PDA bump for the `["observation", observation_id]` seeds.
    pub bump: u8,
}

impl ObservationRecord {
    /// 8 (discriminator) + 32 + 8 + 8 + 8 + 32 + 1.
    pub const SPACE: usize = 97;
}

/// The pubkey-to-agent mapping, slice 9's rule: identity. Kernel agent
/// ids are 32-byte key identities, so a signer's agent id is the signer's
/// pubkey bytes — the signature is the proof, and no mapping account can
/// grant an agent id the signer does not hold. Pure and total: every
/// pubkey resolves to exactly one agent id, and the gates below decide
/// whether that agent may act.
pub fn resolve_agent_id(payer: Pubkey) -> [u8; 32] {
    payer.to_bytes()
}

/// Resolve a transition proposer: the signer's agent id must be a
/// registered, active agent in the sealed launch profile. The Observer
/// role holder is role-bearing but NOT a registered agent, so it is
/// refused here exactly as kernel `register_transition` would refuse it
/// via its active-agent gate; an unknown pubkey is refused outright.
/// The gate replays the sealed launch kernel (slice 7's constants), so it
/// judges the pinned profile, not a caller-supplied roster.
pub fn resolve_proposer(
    kernel_state: &KernelState,
    payer: Pubkey,
) -> Result<[u8; 32]> {
    if !kernel_state.sealed {
        return Err(error!(ProgramV04Error::NotSealed));
    }
    let agent_id = resolve_agent_id(payer);
    let k = sealed_launch_kernel().map_err(|_| error!(ProgramV04Error::KernelSeal))?;
    match k.agent(&agent_id) {
        Some(profile) if profile.active => Ok(agent_id),
        _ => Err(error!(ProgramV04Error::UnknownProposer)),
    }
}

/// Build an `ObservationRecord` against a `KernelState`, enforcing the
/// sealed-state gate, the Observer-role gate (via the sealed launch
/// profile, as in `resolve_proposer`), and the kernel's ingest window
/// rules against the state's clock. Pure: a refused record leaves the
/// state untouched, and the caller (instruction or test) owns persistence.
pub fn build_observation_record(
    kernel_state: &KernelState,
    observation_id: [u8; 32],
    timestamp: u64,
    expiry: u64,
    confidence_raw: u64,
    observer: Pubkey,
    bump: u8,
) -> Result<ObservationRecord> {
    if !kernel_state.sealed {
        return Err(error!(ProgramV04Error::NotSealed));
    }
    let agent_id = resolve_agent_id(observer);
    let k = sealed_launch_kernel().map_err(|_| error!(ProgramV04Error::KernelSeal))?;
    if !k.has_role(&agent_id, Role::Observer) {
        return Err(error!(ProgramV04Error::NotObserver));
    }
    let now = kernel_state.clock;
    if timestamp > now {
        return Err(error!(ProgramV04Error::ObservationFromFuture));
    }
    if expiry <= now || confidence_raw > SCALE {
        return Err(error!(ProgramV04Error::ObservationBadWindow));
    }
    Ok(ObservationRecord { observation_id, timestamp, expiry, confidence_raw, observer, bump })
}

/// Replay a stored transition pair through the real kernel (slice 10).
/// Pure except for the record's stage, which advances to
/// `KERNEL_REGISTERED` only on success — every refusal leaves the record
/// untouched. Gates, in order: sealed state; once-only stage; parent
/// exactly the sealed head; observation linkage; proposer resolved
/// against the sealed profile; observer holding the Observer role there.
/// The kernel calls run at `KernelState.clock` against a freshly rebuilt
/// sealed launch kernel, so this proves registration against the genesis
/// head only (see module docs). The manifest's `relevant_groups` are the
/// proposer's groups in the sealed profile — slice 10's derivation, not a
/// kernel rule. A kernel refusal in either call maps to `ReplayFailed`:
/// governance and unknown types, an expired manifest, and an
/// overconfident observation all fail here, through the kernel, not
/// through a program-side guess about them.
pub fn replay_transition_from_records(
    kernel_state: &KernelState,
    transition_record: &mut TransitionRecord,
    observation_record: &ObservationRecord,
) -> Result<Life> {
    if !kernel_state.sealed {
        return Err(error!(ProgramV04Error::NotSealed));
    }
    if transition_record.stage != TransitionRecord::REGISTERED {
        return Err(error!(ProgramV04Error::WrongStage));
    }
    if transition_record.parent_index != kernel_state.head_index
        || transition_record.parent_hash != kernel_state.head_hash
    {
        return Err(error!(ProgramV04Error::ParentMismatch));
    }
    if transition_record.observation_id != observation_record.observation_id {
        return Err(error!(ProgramV04Error::RecordMismatch));
    }
    let proposer = resolve_proposer(kernel_state, transition_record.proposer)?;
    let observer_id = resolve_agent_id(observation_record.observer);
    let mut k = sealed_launch_kernel().map_err(|_| error!(ProgramV04Error::KernelSeal))?;
    if !k.has_role(&observer_id, Role::Observer) {
        return Err(error!(ProgramV04Error::NotObserver));
    }
    let relevant_groups = match k.agent(&proposer) {
        Some(profile) => profile.groups.clone(),
        None => return Err(error!(ProgramV04Error::UnknownProposer)),
    };
    let observation = Observation {
        id: observation_record.observation_id,
        timestamp: observation_record.timestamp,
        expiry: observation_record.expiry,
        confidence: Amount::from_raw(observation_record.confidence_raw),
    };
    k.ingest_observation(&observer_id, observation, kernel_state.clock)
        .map_err(|_| error!(ProgramV04Error::ReplayFailed))?;
    let manifest = Manifest {
        transition_type: transition_record.transition_type,
        parent_index: transition_record.parent_index,
        parent_hash: transition_record.parent_hash,
        declared_next_hash: transition_record.declared_next_hash,
        observation_id: transition_record.observation_id,
        expires_at: transition_record.expires_at,
        relevant_groups,
        action: None,
    };
    k.register_transition(
        &proposer,
        transition_record.transition_id,
        manifest,
        BTreeSet::new(),
        kernel_state.clock,
    )
    .map_err(|_| error!(ProgramV04Error::ReplayFailed))?;
    let life = match k.life_of(&transition_record.transition_id) {
        Some(Life::Proposed) => Life::Proposed,
        _ => return Err(error!(ProgramV04Error::ReplayFailed)),
    };
    transition_record.stage = TransitionRecord::KERNEL_REGISTERED;
    Ok(life)
}

/// Evaluate a stored transition pair through the real kernel (slice 12).
/// Pure except for the record's stage, which advances to `EVALUATED`
/// only on success — every refusal leaves the record untouched. Gates,
/// in order: sealed state; stage exactly `KERNEL_REGISTERED` (a
/// merely-registered record has no kernel registration to evaluate,
/// and an already-evaluated record evaluates once only); parent
/// exactly the sealed head; observation linkage; proposer resolved
/// against the sealed profile; observer holding the Observer role
/// there. The kernel calls run at `KernelState.clock` against a freshly
/// rebuilt sealed launch kernel under `ProgramEnv`: ingest +
/// register (the slice-10 sequence, because evaluation needs a
/// `Proposed` transition and the kernel object is not persisted),
/// then `evaluate_transition`. Only `Life::Evaluated` advances the
/// stage — a kernel refusal maps to `EvaluateFailed`, and so does a
/// non-Evaluated life (a `Rejected` verdict under a failing env, or a
/// still-`Proposed` transition under an unavailable one), because the
/// stage records only a proven evaluation. The relevant-groups
/// derivation is slice 10's rule, reused here, not a kernel rule.
pub fn evaluate_transition_from_records(
    kernel_state: &KernelState,
    transition_record: &mut TransitionRecord,
    observation_record: &ObservationRecord,
) -> Result<Life> {
    if !kernel_state.sealed {
        return Err(error!(ProgramV04Error::NotSealed));
    }
    if transition_record.stage != TransitionRecord::KERNEL_REGISTERED {
        return Err(error!(ProgramV04Error::WrongStage));
    }
    if transition_record.parent_index != kernel_state.head_index
        || transition_record.parent_hash != kernel_state.head_hash
    {
        return Err(error!(ProgramV04Error::ParentMismatch));
    }
    if transition_record.observation_id != observation_record.observation_id {
        return Err(error!(ProgramV04Error::RecordMismatch));
    }
    let proposer = resolve_proposer(kernel_state, transition_record.proposer)?;
    let observer_id = resolve_agent_id(observation_record.observer);
    let mut k = sealed_launch_kernel_env().map_err(|_| error!(ProgramV04Error::KernelSeal))?;
    if !k.has_role(&observer_id, Role::Observer) {
        return Err(error!(ProgramV04Error::NotObserver));
    }
    let relevant_groups = match k.agent(&proposer) {
        Some(profile) => profile.groups.clone(),
        None => return Err(error!(ProgramV04Error::UnknownProposer)),
    };
    let observation = Observation {
        id: observation_record.observation_id,
        timestamp: observation_record.timestamp,
        expiry: observation_record.expiry,
        confidence: Amount::from_raw(observation_record.confidence_raw),
    };
    k.ingest_observation(&observer_id, observation, kernel_state.clock)
        .map_err(|_| error!(ProgramV04Error::EvaluateFailed))?;
    let manifest = Manifest {
        transition_type: transition_record.transition_type,
        parent_index: transition_record.parent_index,
        parent_hash: transition_record.parent_hash,
        declared_next_hash: transition_record.declared_next_hash,
        observation_id: transition_record.observation_id,
        expires_at: transition_record.expires_at,
        relevant_groups,
        action: None,
    };
    k.register_transition(
        &proposer,
        transition_record.transition_id,
        manifest,
        BTreeSet::new(),
        kernel_state.clock,
    )
    .map_err(|_| error!(ProgramV04Error::EvaluateFailed))?;
    k.evaluate_transition(&transition_record.transition_id, kernel_state.clock)
        .map_err(|_| error!(ProgramV04Error::EvaluateFailed))?;
    let life = match k.life_of(&transition_record.transition_id) {
        Some(Life::Evaluated) => Life::Evaluated,
        _ => return Err(error!(ProgramV04Error::EvaluateFailed)),
    };
    transition_record.stage = TransitionRecord::EVALUATED;
    Ok(life)
}

/// Mark a stored transition pair eligible through the real kernel (slice 13;
/// slice 14 persists the proven window).
/// Pure except for the record's stage and eligibility window, which
/// advance to `ELIGIBLE` with `eligible_at` / `consensus_deadline`
/// written only on success — every refusal leaves the record
/// untouched. Gates, in order: sealed state; stage exactly `EVALUATED`
/// (a merely kernel-registered record has no evaluation to make
/// eligible, and an already-eligible record marks once only); parent
/// exactly the sealed head; observation linkage; proposer resolved
/// against the sealed profile; observer holding the Observer role
/// there. The kernel calls run at `KernelState.clock` against a freshly
/// rebuilt sealed launch kernel under `ProgramEnv`: ingest +
/// register (the slice-10 sequence) + evaluate (the slice-12 sequence,
/// because eligibility needs an `Evaluated` life with its
/// `constraints_ok` / `objectives_ok` flags recorded, and the kernel
/// object is not persisted), then `mark_eligible`. Only
/// `Life::Eligible` advances the stage — a kernel refusal in any of
/// the four calls maps to `EligibleFailed`, and so does a non-Eligible
/// life, because the stage records only a proven eligibility. The
/// relevant-groups derivation is slice 10's rule, reused here, not a
/// kernel rule. The sibling field `mark_eligible` joins is the fresh
/// kernel's, not a persisted one (see module docs): this slice proves
/// the eligibility gate and the stage advance, not field persistence.
pub fn mark_eligible_from_records(
    kernel_state: &KernelState,
    transition_record: &mut TransitionRecord,
    observation_record: &ObservationRecord,
) -> Result<Life> {
    if !kernel_state.sealed {
        return Err(error!(ProgramV04Error::NotSealed));
    }
    if transition_record.stage != TransitionRecord::EVALUATED {
        return Err(error!(ProgramV04Error::WrongStage));
    }
    if transition_record.parent_index != kernel_state.head_index
        || transition_record.parent_hash != kernel_state.head_hash
    {
        return Err(error!(ProgramV04Error::ParentMismatch));
    }
    if transition_record.observation_id != observation_record.observation_id {
        return Err(error!(ProgramV04Error::RecordMismatch));
    }
    let proposer = resolve_proposer(kernel_state, transition_record.proposer)?;
    let observer_id = resolve_agent_id(observation_record.observer);
    let mut k = sealed_launch_kernel_env().map_err(|_| error!(ProgramV04Error::KernelSeal))?;
    if !k.has_role(&observer_id, Role::Observer) {
        return Err(error!(ProgramV04Error::NotObserver));
    }
    let relevant_groups = match k.agent(&proposer) {
        Some(profile) => profile.groups.clone(),
        None => return Err(error!(ProgramV04Error::UnknownProposer)),
    };
    let observation = Observation {
        id: observation_record.observation_id,
        timestamp: observation_record.timestamp,
        expiry: observation_record.expiry,
        confidence: Amount::from_raw(observation_record.confidence_raw),
    };
    k.ingest_observation(&observer_id, observation, kernel_state.clock)
        .map_err(|_| error!(ProgramV04Error::EligibleFailed))?;
    let manifest = Manifest {
        transition_type: transition_record.transition_type,
        parent_index: transition_record.parent_index,
        parent_hash: transition_record.parent_hash,
        declared_next_hash: transition_record.declared_next_hash,
        observation_id: transition_record.observation_id,
        expires_at: transition_record.expires_at,
        relevant_groups,
        action: None,
    };
    k.register_transition(
        &proposer,
        transition_record.transition_id,
        manifest,
        BTreeSet::new(),
        kernel_state.clock,
    )
    .map_err(|_| error!(ProgramV04Error::EligibleFailed))?;
    k.evaluate_transition(&transition_record.transition_id, kernel_state.clock)
        .map_err(|_| error!(ProgramV04Error::EligibleFailed))?;
    k.mark_eligible(&transition_record.transition_id, kernel_state.clock)
        .map_err(|_| error!(ProgramV04Error::EligibleFailed))?;
    let life = match k.life_of(&transition_record.transition_id) {
        Some(Life::Eligible) => Life::Eligible,
        _ => return Err(error!(ProgramV04Error::EligibleFailed)),
    };
    // Slice 14: persist the eligibility window the kernel just proved.
    // The kernel sets eligible_at to the `now` it was called with and
    // the deadline to now + the policy's consensus_window; the program
    // derives the same two values from the same inputs (this state's
    // clock, the sealed launch policy's window) — never from an
    // instruction argument — and only after the kernel proved
    // Eligible, so a refusal leaves both fields at 0. The kernel
    // exposes no eligible_at accessor, so this derivation, not a read,
    // is what the record stores; the window the kernel enforced and
    // the window the record stores agree exactly because both run from
    // the same clock and the same policy constant.
    transition_record.stage = TransitionRecord::ELIGIBLE;
    transition_record.eligible_at = kernel_state.clock;
    transition_record.consensus_deadline = kernel_state
        .clock
        .checked_add(launch_policy().consensus_window)
        .ok_or(error!(ProgramV04Error::EligibleFailed))?;
    Ok(life)
}

/// Advance a `KernelState`'s clock (slice 11). Pure except for the
/// clock itself, which moves only on success — every refusal leaves
/// the state untouched. Gates, in order: sealed state; forward-only
/// movement, mirroring the kernel's own `tick` (a regression, strictly
/// less than the current clock, is refused; an equal value is an
/// accepted no-op, exactly as `tick` accepts it). The head
/// (`head_index`/`head_hash`) and the seal flag are never touched:
/// clock advancement is not chain advancement, and a later commit
/// replay is what moves the head. Authority is the instruction's
/// signing payer (see the instruction docs) — this helper judges
/// time, not authority.
pub fn advance_clock_on_state(state: &mut KernelState, new_clock: u64) -> Result<u64> {
    if !state.sealed {
        return Err(error!(ProgramV04Error::NotSealed));
    }
    if new_clock < state.clock {
        return Err(error!(ProgramV04Error::ClockRegression));
    }
    state.clock = new_clock;
    Ok(state.clock)
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

#[event]
pub struct ObservationIngested {
    pub observation_id: [u8; 32],
    pub observer: Pubkey,
    pub timestamp: u64,
}

#[event]
pub struct TransitionReplayed {
    pub transition_id: [u8; 32],
    pub stage: u8,
}

#[event]
pub struct ClockAdvanced {
    pub clock: u64,
}

#[event]
pub struct TransitionEvaluated {
    pub transition_id: [u8; 32],
    pub stage: u8,
}

#[event]
pub struct TransitionMarkedEligible {
    pub transition_id: [u8; 32],
    pub stage: u8,
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
    #[msg("program-v04 ingest: observation timestamp is after the kernel clock")]
    ObservationFromFuture,
    #[msg("program-v04 ingest: observation expiry or confidence outside the kernel window")]
    ObservationBadWindow,
    #[msg("program-v04 mapping: signer is not a registered, active agent of the sealed profile")]
    UnknownProposer,
    #[msg("program-v04 ingest: signer agent does not hold the Observer role")]
    NotObserver,
    #[msg("program-v04 replay: transition and observation records do not belong together")]
    RecordMismatch,
    #[msg("program-v04 replay: transition record is not at the Registered stage")]
    WrongStage,
    #[msg("program-v04 replay: kernel ingest or registration refused")]
    ReplayFailed,
    #[msg("program-v04 clock: new clock is before the current kernel clock")]
    ClockRegression,
    #[msg("program-v04 evaluate: kernel evaluation refused or did not reach Evaluated")]
    EvaluateFailed,
    #[msg("program-v04 eligible: kernel eligibility refused or did not reach Eligible")]
    EligibleFailed,
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
/// steps never consult the environment, so `NoEnv` is exact for the
/// genesis/seal sequence — not a shortcut. Evaluation steps, which do
/// consult the environment, build the same sequence under `ProgramEnv`
/// (slice 12) instead of reusing this type.
pub type LaunchKernelNoEnv = Kernel<NoEnv>;

/// The administrator's environment answers, ported to no_std (slice 12):
/// the step-2 launch env's answers — required constraint C1 passes,
/// required objective O1 holds — over `alloc` maps. Everything else
/// keeps the trait defaults (unavailable constraints, no objective
/// answer, relevance ONE, no position source), so an unlisted constraint
/// still answers Unavailable and can never pass by omission. The
/// program never takes a constraint or objective verdict from an
/// instruction argument: these answers are baked in here.
#[derive(Debug, Default)]
pub struct ProgramEnv {
    constraints: BTreeMap<[u8; 32], ConstraintStatus>,
    objectives: BTreeMap<[u8; 32], bool>,
}

impl Env for ProgramEnv {
    fn constraint(
        &self,
        c: &[u8; 32],
        _m: &Manifest,
    ) -> ConstraintStatus {
        self.constraints.get(c).copied().unwrap_or(ConstraintStatus::Unavailable)
    }

    fn objective(&self, o: &[u8; 32], _m: &Manifest) -> Option<bool> {
        self.objectives.get(o).copied()
    }
}

/// The program environment: C1 = Pass, O1 = true, nothing else answered.
pub fn program_env() -> ProgramEnv {
    let mut env = ProgramEnv::default();
    env.constraints.insert(C1, ConstraintStatus::Pass);
    env.objectives.insert(O1, true);
    env
}

/// The launch kernel type under the program environment (slice 12).
pub type LaunchKernelProgramEnv = Kernel<ProgramEnv>;

/// Fresh kernel at genesis time `t = 100`, genesis authority GEN.
pub fn fresh_launch_kernel() -> core::result::Result<LaunchKernelNoEnv, KernelError> {
    Kernel::new(
        GEN,
        SystemState { index: 0, hash: [0; 32], timestamp: 100 },
        launch_config(),
        NoEnv,
    )
}

/// Fresh kernel at genesis time `t = 100` under a caller-supplied
/// environment (slice 12: `program_env()`). Genesis steps never consult
/// the environment, so the sealed state this builds is the same sealed
/// state `fresh_launch_kernel` builds — only the answers later steps
/// receive differ.
pub fn fresh_launch_kernel_with_env<E: Env>(
    env: E,
) -> core::result::Result<Kernel<E>, KernelError> {
    Kernel::new(
        GEN,
        SystemState { index: 0, hash: [0; 32], timestamp: 100 },
        launch_config(),
        env,
    )
}

/// Genesis seeding: policies for TT and all four governance types, the
/// required constraint/objective sets for TT, six agents at base 2.0,
/// the operator roles, and the three capacity stocks. Deliberately NOT
/// the council or the seal, so tests can assert the seal's readiness
/// gates compositionally (see `seal_refused_without_council`).
pub fn apply_launch_genesis<E: Env>(
    k: &mut Kernel<E>,
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
pub fn install_launch_council<E: Env>(
    k: &mut Kernel<E>,
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

/// The sealed launch kernel under the program environment (slice 12):
/// the same genesis sequence, the same sealed state, but later steps
/// receive the administrator's env answers instead of NoEnv's
/// all-unavailable defaults. Evaluation replays build from this, never
/// from a caller-supplied environment.
pub fn sealed_launch_kernel_env(
) -> core::result::Result<LaunchKernelProgramEnv, KernelError> {
    let mut k = fresh_launch_kernel_with_env(program_env())?;
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
    use coordination_kernel::{Manifest, Observation};

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
        // proposer 32 + stage 1 + eligible_at 8 + consensus_deadline 8 + bump 1.
        assert_eq!(
            TransitionRecord::SPACE,
            8 + 32 + 32 + 8 + 32 + 32 + 32 + 8 + 32 + 1 + 8 + 8 + 1
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
            stage: TransitionRecord::ELIGIBLE,
            eligible_at: 200,
            consensus_deadline: 300,
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
        assert_eq!(back.stage, 3);
        assert_eq!(back.eligible_at, 200);
        assert_eq!(back.consensus_deadline, 300);
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

    #[test]
    fn observation_record_space_accounts_for_every_field() {
        // Discriminator 8 + id 32 + timestamp 8 + expiry 8 +
        // confidence 8 + observer 32 + bump 1.
        assert_eq!(
            ObservationRecord::SPACE,
            8 + 32 + 8 + 8 + 8 + 32 + 1
        );
    }

    #[test]
    fn observation_record_borsh_round_trips() {
        use anchor_lang::{AccountDeserialize, AccountSerialize};
        let record = ObservationRecord {
            observation_id: [0xC0; 32],
            timestamp: 100,
            expiry: 9100,
            confidence_raw: 900_000,
            observer: Pubkey::new_from_array(OBSERVER),
            bump: 254,
        };
        let mut bytes = Vec::new();
        record.try_serialize(&mut bytes).expect("serialize must succeed");
        assert_eq!(bytes.len(), ObservationRecord::SPACE);
        let back = <ObservationRecord as AccountDeserialize>::try_deserialize(&mut &bytes[..])
            .expect("deserialize must succeed");
        assert_eq!(back.observation_id, [0xC0; 32]);
        assert_eq!(back.timestamp, 100);
        assert_eq!(back.expiry, 9100);
        assert_eq!(back.confidence_raw, 900_000);
        assert_eq!(back.observer, Pubkey::new_from_array(OBSERVER));
        assert_eq!(back.bump, 254);
    }

    fn observer_pubkey() -> Pubkey {
        Pubkey::new_from_array(OBSERVER)
    }

    #[test]
    fn resolve_agent_id_is_identity() {
        let pubkey = Pubkey::new_from_array([0x42; 32]);
        assert_eq!(resolve_agent_id(pubkey), [0x42; 32]);
        assert_eq!(resolve_agent_id(observer_pubkey()), OBSERVER);
        assert_eq!(resolve_agent_id(Pubkey::new_from_array(ag(1))), ag(1));
    }

    #[test]
    fn build_observation_record_refuses_unsealed() {
        let state = genesis_kernel_state().expect("genesis must construct");
        let err = build_observation_record(
            &state, [0xC0; 32], 100, 9100, 900_000, observer_pubkey(), 1,
        )
        .expect_err("unsealed ingest must refuse");
        assert!(format!("{:?}", err).contains("NotSealed"));
    }

    #[test]
    fn build_observation_record_refuses_non_observer_agent() {
        // Agent 1 is registered and active but holds no Observer role.
        let state = sealed_state();
        let err = build_observation_record(
            &state, [0xC0; 32], 100, 9100, 900_000,
            Pubkey::new_from_array(ag(1)), 1,
        )
        .expect_err("agent without Observer role must refuse");
        assert!(format!("{:?}", err).contains("NotObserver"));
    }

    #[test]
    fn build_observation_record_refuses_unknown_pubkey() {
        let state = sealed_state();
        let err = build_observation_record(
            &state, [0xC0; 32], 100, 9100, 900_000,
            Pubkey::new_from_array([0x77; 32]), 1,
        )
        .expect_err("unknown pubkey must refuse");
        assert!(format!("{:?}", err).contains("NotObserver"));
    }

    #[test]
    fn build_observation_record_refuses_future_timestamp() {
        let state = sealed_state();
        assert_eq!(state.clock, 100);
        let err = build_observation_record(
            &state, [0xC0; 32], 101, 9100, 900_000, observer_pubkey(), 1,
        )
        .expect_err("timestamp after the clock must refuse");
        assert!(format!("{:?}", err).contains("ObservationFromFuture"));
    }

    #[test]
    fn build_observation_record_refuses_expired_and_overconfident() {
        let state = sealed_state();
        let expired = build_observation_record(
            &state, [0xC0; 32], 100, 100, 900_000, observer_pubkey(), 1,
        )
        .expect_err("expiry at the clock must refuse");
        assert!(format!("{:?}", expired).contains("ObservationBadWindow"));
        let overconfident = build_observation_record(
            &state, [0xC0; 32], 100, 9100, SCALE + 1, observer_pubkey(), 1,
        )
        .expect_err("confidence above SCALE must refuse");
        assert!(format!("{:?}", overconfident).contains("ObservationBadWindow"));
    }

    #[test]
    fn build_observation_record_accepts_observer_at_genesis_clock() {
        let state = sealed_state();
        let record = build_observation_record(
            &state, [0xC0; 32], 100, 9100, 900_000, observer_pubkey(), 7,
        )
        .expect("observer at the genesis clock must ingest");
        assert_eq!(record.observation_id, [0xC0; 32]);
        assert_eq!(record.timestamp, 100);
        assert_eq!(record.expiry, 9100);
        assert_eq!(record.confidence_raw, 900_000);
        assert_eq!(record.observer, observer_pubkey());
        assert_eq!(record.bump, 7);
    }

    #[test]
    fn resolve_proposer_accepts_seeded_agents() {
        let state = sealed_state();
        for n in [1u8, 6u8] {
            let resolved = resolve_proposer(&state, Pubkey::new_from_array(ag(n)))
                .expect("seeded agent must resolve");
            assert_eq!(resolved, ag(n));
        }
    }

    #[test]
    fn resolve_proposer_refuses_observer_and_unknown() {
        let state = sealed_state();
        let observer_err = resolve_proposer(&state, observer_pubkey())
            .expect_err("role holder without agent registration must refuse");
        assert!(format!("{:?}", observer_err).contains("UnknownProposer"));
        let unknown_err = resolve_proposer(
            &state, Pubkey::new_from_array([0x77; 32]),
        )
        .expect_err("unknown pubkey must refuse");
        assert!(format!("{:?}", unknown_err).contains("UnknownProposer"));
        let unsealed = genesis_kernel_state().expect("genesis must construct");
        let unsealed_err = resolve_proposer(&unsealed, Pubkey::new_from_array(ag(1)))
            .expect_err("unsealed proposer must refuse");
        assert!(format!("{:?}", unsealed_err).contains("NotSealed"));
    }

    #[test]
    fn resolved_ids_drive_kernel_registration() {
        // The slice-9 proof: the pubkey-derived ids are not just stored —
        // the observer id ingests and the proposer id registers on a real
        // sealed kernel, which is exactly what the later replay slice
        // needs from the stored records.
        let state = sealed_state();
        let observer_id =
            resolve_agent_id(observer_pubkey());
        let proposer_id =
            resolve_proposer(&state, Pubkey::new_from_array(ag(1)))
                .expect("agent 1 must resolve as proposer");
        assert_eq!(observer_id, OBSERVER);
        assert_eq!(proposer_id, ag(1));
        let mut k = sealed_launch_kernel().expect("launch sequence must seal");
        let obs = Observation {
            id: [0xC0; 32],
            timestamp: 100,
            expiry: 9100,
            confidence: Amount::from_raw(900_000),
        };
        k.ingest_observation(&observer_id, obs, 100)
            .expect("observer id must ingest on the sealed kernel");
        let manifest = Manifest {
            transition_type: TT,
            parent_index: 0,
            parent_hash: [0; 32],
            declared_next_hash: [0xE1; 32],
            observation_id: [0xC0; 32],
            expires_at: 10_100,
            relevant_groups: BTreeSet::from([1u16]),
            action: None,
        };
        k.register_transition(&proposer_id, [0xD1; 32], manifest, BTreeSet::new(), 100)
            .expect("proposer id must register on the sealed kernel");
        assert_eq!(k.life_of(&[0xD1; 32]), Some(coordination_kernel::Life::Proposed));
    }

    fn replay_pair() -> (KernelState, TransitionRecord, ObservationRecord) {
        let state = sealed_state();
        let record = build_transition_record(
            &state, [0xD1; 32], TT, 0, [0; 32], [0xE1; 32], [0xC0; 32], 10_100,
            Pubkey::new_from_array(ag(1)), 7,
        )
        .expect("sealed-head register must build");
        let obs = build_observation_record(
            &state, [0xC0; 32], 100, 9100, 900_000, observer_pubkey(), 7,
        )
        .expect("observer at the genesis clock must ingest");
        (state, record, obs)
    }

    #[test]
    fn replay_advances_registered_record_to_kernel_registered() {
        let (state, mut record, obs) = replay_pair();
        let life = replay_transition_from_records(&state, &mut record, &obs)
            .expect("genesis-clock pair must replay");
        assert_eq!(life, Life::Proposed);
        assert_eq!(record.stage, TransitionRecord::KERNEL_REGISTERED);
        assert_eq!(record.stage, 1);
        // The record's other fields are untouched by the stage advance,
        // and it still serializes at exactly SPACE bytes.
        assert_eq!(record.transition_type, TT);
        assert_eq!(record.declared_next_hash, [0xE1; 32]);
        use anchor_lang::AccountSerialize;
        let mut bytes = Vec::new();
        record.try_serialize(&mut bytes).expect("serialize must succeed");
        assert_eq!(bytes.len(), TransitionRecord::SPACE);
    }

    #[test]
    fn replay_refuses_unsealed() {
        let (mut state, mut record, obs) = replay_pair();
        state.sealed = false;
        let err = replay_transition_from_records(&state, &mut record, &obs)
            .expect_err("unsealed replay must refuse");
        assert!(format!("{:?}", err).contains("NotSealed"));
        assert_eq!(record.stage, TransitionRecord::REGISTERED, "a refused replay must not move the stage");
    }

    #[test]
    fn replay_is_once_only() {
        let (state, mut record, obs) = replay_pair();
        replay_transition_from_records(&state, &mut record, &obs)
            .expect("first replay must succeed");
        let err = replay_transition_from_records(&state, &mut record, &obs)
            .expect_err("second replay must refuse");
        assert!(format!("{:?}", err).contains("WrongStage"));
        assert_eq!(record.stage, TransitionRecord::KERNEL_REGISTERED);
    }

    #[test]
    fn replay_refuses_parent_mismatch() {
        let (state, mut record, obs) = replay_pair();
        record.parent_index = 1;
        let err = replay_transition_from_records(&state, &mut record, &obs)
            .expect_err("parent off the sealed head must refuse");
        assert!(format!("{:?}", err).contains("ParentMismatch"));
        assert_eq!(record.stage, TransitionRecord::REGISTERED);
    }

    #[test]
    fn replay_refuses_observation_mismatch() {
        let (state, mut record, _) = replay_pair();
        let other = build_observation_record(
            &state, [0xC1; 32], 100, 9100, 900_000, observer_pubkey(), 7,
        )
        .expect("second observation must build");
        let err = replay_transition_from_records(&state, &mut record, &other)
            .expect_err("observation id mismatch must refuse");
        assert!(format!("{:?}", err).contains("RecordMismatch"));
        assert_eq!(record.stage, TransitionRecord::REGISTERED);
    }

    #[test]
    fn replay_refuses_unknown_proposer() {
        // The slice-8 builder does not gate the proposer, so such a
        // record can exist — replay is where it must be refused.
        let (state, mut record, obs) = replay_pair();
        record.proposer = Pubkey::new_from_array([0x99; 32]);
        let err = replay_transition_from_records(&state, &mut record, &obs)
            .expect_err("unknown proposer must refuse");
        assert!(format!("{:?}", err).contains("UnknownProposer"));
        assert_eq!(record.stage, TransitionRecord::REGISTERED);
    }

    #[test]
    fn replay_refuses_non_observer_observation() {
        // Built as a struct literal: the slice-9 builder would refuse
        // this observer, so replay must catch a record that bypassed it.
        let (state, mut record, _) = replay_pair();
        let obs = ObservationRecord {
            observation_id: [0xC0; 32],
            timestamp: 100,
            expiry: 9100,
            confidence_raw: 900_000,
            observer: Pubkey::new_from_array(ag(1)),
            bump: 7,
        };
        let err = replay_transition_from_records(&state, &mut record, &obs)
            .expect_err("non-observer observation must refuse");
        assert!(format!("{:?}", err).contains("NotObserver"));
        assert_eq!(record.stage, TransitionRecord::REGISTERED);
    }

    #[test]
    fn replay_refuses_expired_transition_and_overconfident_observation() {
        let (state, mut record, obs) = replay_pair();
        record.expires_at = 100;
        let err = replay_transition_from_records(&state, &mut record, &obs)
            .expect_err("expiry at the clock must refuse in the kernel");
        assert!(format!("{:?}", err).contains("ReplayFailed"));
        assert_eq!(record.stage, TransitionRecord::REGISTERED);
        let (state, mut record, _) = replay_pair();
        let obs = ObservationRecord {
            observation_id: [0xC0; 32],
            timestamp: 100,
            expiry: 9100,
            confidence_raw: SCALE + 1,
            observer: observer_pubkey(),
            bump: 7,
        };
        let err = replay_transition_from_records(&state, &mut record, &obs)
            .expect_err("confidence above SCALE must refuse in the kernel");
        assert!(format!("{:?}", err).contains("ReplayFailed"));
        assert_eq!(record.stage, TransitionRecord::REGISTERED);
    }

    #[test]
    fn replay_refuses_governance_and_unknown_types() {
        let (state, mut record, obs) = replay_pair();
        record.transition_type = T_GOV_COUNCIL;
        let err = replay_transition_from_records(&state, &mut record, &obs)
            .expect_err("governance type must refuse in the kernel");
        assert!(format!("{:?}", err).contains("ReplayFailed"));
        assert_eq!(record.stage, TransitionRecord::REGISTERED);
        let (state, mut record, obs) = replay_pair();
        record.transition_type = [0x77; 32];
        let err = replay_transition_from_records(&state, &mut record, &obs)
            .expect_err("type without a policy must refuse in the kernel");
        assert!(format!("{:?}", err).contains("ReplayFailed"));
        assert_eq!(record.stage, TransitionRecord::REGISTERED);
    }

    fn advanced_state_to(new_clock: u64) -> KernelState {
        let mut state = sealed_state();
        advance_clock_on_state(&mut state, new_clock).expect("advance must succeed");
        state
    }

    #[test]
    fn advance_clock_moves_sealed_clock_forward_and_keeps_head() {
        let mut state = sealed_state();
        assert_eq!(state.clock, 100);
        let clock = advance_clock_on_state(&mut state, 200).expect("forward advance must succeed");
        assert_eq!(clock, 200);
        assert_eq!(state.clock, 200);
        assert!(state.sealed);
        assert_eq!(state.head_index, 0);
        assert_eq!(state.head_hash, [0; 32]);
    }

    #[test]
    fn advance_clock_refuses_unsealed() {
        let mut state = genesis_kernel_state().expect("genesis must construct");
        assert_eq!(state.clock, 0);
        let err = advance_clock_on_state(&mut state, 200).expect_err("unsealed advance must refuse");
        assert!(format!("{:?}", err).contains("NotSealed"));
        assert_eq!(state.clock, 0, "a refused advance must not move the clock");
    }

    #[test]
    fn advance_clock_refuses_regression() {
        let mut state = sealed_state();
        let err = advance_clock_on_state(&mut state, 99).expect_err("regression must refuse");
        assert!(format!("{:?}", err).contains("ClockRegression"));
        assert_eq!(state.clock, 100);
        advance_clock_on_state(&mut state, 200).expect("forward advance must succeed");
        let err = advance_clock_on_state(&mut state, 150)
            .expect_err("backwards after advance must refuse");
        assert!(format!("{:?}", err).contains("ClockRegression"));
        assert_eq!(state.clock, 200);
    }

    #[test]
    fn advance_clock_equal_is_accepted_noop() {
        let mut state = sealed_state();
        let clock = advance_clock_on_state(&mut state, 100).expect("equal advance must succeed");
        assert_eq!(clock, 100);
        assert_eq!(state.clock, 100);
    }

    #[test]
    fn advanced_clock_unblocks_later_observation() {
        let mut state = sealed_state();
        let refused = build_observation_record(
            &state, [0xC0; 32], 150, 9100, 900_000, observer_pubkey(), 7,
        )
        .expect_err("timestamp after the genesis clock must refuse");
        assert!(format!("{:?}", refused).contains("ObservationFromFuture"));
        advance_clock_on_state(&mut state, 200).expect("advance must succeed");
        let record = build_observation_record(
            &state, [0xC0; 32], 150, 9100, 900_000, observer_pubkey(), 7,
        )
        .expect("timestamp at or before the advanced clock must ingest");
        assert_eq!(record.timestamp, 150);
    }

    #[test]
    fn advanced_clock_unblocks_post_genesis_replay() {
        let state = advanced_state_to(200);
        let mut record = build_transition_record(
            &state, [0xD1; 32], TT, 0, [0; 32], [0xE1; 32], [0xC0; 32], 10_100,
            Pubkey::new_from_array(ag(1)), 7,
        )
        .expect("sealed-head register must build at the advanced clock");
        let obs = build_observation_record(
            &state, [0xC0; 32], 150, 9100, 900_000, observer_pubkey(), 7,
        )
        .expect("post-genesis observation must build");
        let life = replay_transition_from_records(&state, &mut record, &obs)
            .expect("post-genesis pair must replay at the advanced clock");
        assert_eq!(life, Life::Proposed);
        assert_eq!(record.stage, TransitionRecord::KERNEL_REGISTERED);
    }

    #[test]
    fn advance_past_expiry_kills_replay_through_the_kernel() {
        let (state, mut record, obs) = replay_pair();
        let mut advanced = state;
        advance_clock_on_state(&mut advanced, 9100).expect("advance must succeed");
        let err = replay_transition_from_records(&advanced, &mut record, &obs)
            .expect_err("observation lapsed by the advance must refuse in the kernel");
        assert!(format!("{:?}", err).contains("ReplayFailed"));
        assert_eq!(record.stage, TransitionRecord::REGISTERED);
    }

    #[test]
    fn chained_advances_then_post_genesis_replay() {
        let mut state = sealed_state();
        advance_clock_on_state(&mut state, 150).expect("first advance must succeed");
        advance_clock_on_state(&mut state, 300).expect("second advance must succeed");
        assert_eq!(state.clock, 300);
        let mut record = build_transition_record(
            &state, [0xD1; 32], TT, 0, [0; 32], [0xE1; 32], [0xC0; 32], 10_100,
            Pubkey::new_from_array(ag(1)), 7,
        )
        .expect("sealed-head register must build");
        let obs = build_observation_record(
            &state, [0xC0; 32], 250, 9100, 900_000, observer_pubkey(), 7,
        )
        .expect("observation inside the advanced window must build");
        let life = replay_transition_from_records(&state, &mut record, &obs)
            .expect("pair must replay after chained advances");
        assert_eq!(life, Life::Proposed);
        assert_eq!(record.stage, TransitionRecord::KERNEL_REGISTERED);
    }

    fn evaluated_pair() -> (KernelState, TransitionRecord, ObservationRecord) {
        let (state, mut record, obs) = replay_pair();
        replay_transition_from_records(&state, &mut record, &obs)
            .expect("pair must replay before evaluate");
        assert_eq!(record.stage, TransitionRecord::KERNEL_REGISTERED);
        (state, record, obs)
    }

    #[test]
    fn evaluate_advances_kernel_registered_to_evaluated() {
        let (state, mut record, obs) = evaluated_pair();
        let life = evaluate_transition_from_records(&state, &mut record, &obs)
            .expect("kernel-registered pair must evaluate");
        assert_eq!(life, Life::Evaluated);
        assert_eq!(record.stage, TransitionRecord::EVALUATED);
        assert_eq!(record.stage, 2);
        // The record's other fields are untouched by the stage advance,
        // and it still serializes at exactly SPACE bytes.
        assert_eq!(record.transition_type, TT);
        assert_eq!(record.declared_next_hash, [0xE1; 32]);
        use anchor_lang::AccountSerialize;
        let mut bytes = Vec::new();
        record.try_serialize(&mut bytes).expect("serialize must succeed");
        assert_eq!(bytes.len(), TransitionRecord::SPACE);
    }

    #[test]
    fn evaluate_refuses_registered_stage() {
        let (state, mut record, obs) = replay_pair();
        let err = evaluate_transition_from_records(&state, &mut record, &obs)
            .expect_err("a merely-registered record has nothing to evaluate");
        assert!(format!("{:?}", err).contains("WrongStage"));
        assert_eq!(record.stage, TransitionRecord::REGISTERED);
    }

    #[test]
    fn evaluate_is_once_only() {
        let (state, mut record, obs) = evaluated_pair();
        evaluate_transition_from_records(&state, &mut record, &obs)
            .expect("first evaluate must succeed");
        let err = evaluate_transition_from_records(&state, &mut record, &obs)
            .expect_err("second evaluate must refuse");
        assert!(format!("{:?}", err).contains("WrongStage"));
        assert_eq!(record.stage, TransitionRecord::EVALUATED);
    }

    #[test]
    fn evaluate_refuses_unsealed() {
        let (mut state, mut record, obs) = evaluated_pair();
        state.sealed = false;
        let err = evaluate_transition_from_records(&state, &mut record, &obs)
            .expect_err("unsealed evaluate must refuse");
        assert!(format!("{:?}", err).contains("NotSealed"));
        assert_eq!(record.stage, TransitionRecord::KERNEL_REGISTERED);
    }

    #[test]
    fn evaluate_refuses_parent_mismatch() {
        let (state, mut record, obs) = evaluated_pair();
        record.parent_index = 1;
        let err = evaluate_transition_from_records(&state, &mut record, &obs)
            .expect_err("parent off the sealed head must refuse");
        assert!(format!("{:?}", err).contains("ParentMismatch"));
        assert_eq!(record.stage, TransitionRecord::KERNEL_REGISTERED);
    }

    #[test]
    fn evaluate_refuses_observation_mismatch() {
        let (state, mut record, _) = evaluated_pair();
        let other = build_observation_record(
            &state, [0xC1; 32], 100, 9100, 900_000, observer_pubkey(), 7,
        )
        .expect("second observation must build");
        let err = evaluate_transition_from_records(&state, &mut record, &other)
            .expect_err("observation id mismatch must refuse");
        assert!(format!("{:?}", err).contains("RecordMismatch"));
        assert_eq!(record.stage, TransitionRecord::KERNEL_REGISTERED);
    }

    #[test]
    fn evaluate_refuses_unknown_proposer() {
        let (state, mut record, obs) = evaluated_pair();
        record.proposer = Pubkey::new_from_array([0x99; 32]);
        let err = evaluate_transition_from_records(&state, &mut record, &obs)
            .expect_err("unknown proposer must refuse");
        assert!(format!("{:?}", err).contains("UnknownProposer"));
        assert_eq!(record.stage, TransitionRecord::KERNEL_REGISTERED);
    }

    #[test]
    fn evaluate_at_advanced_clock() {
        let (state, mut record, obs) = evaluated_pair();
        let mut advanced = state;
        advance_clock_on_state(&mut advanced, 200).expect("advance must succeed");
        let life = evaluate_transition_from_records(&advanced, &mut record, &obs)
            .expect("pair must evaluate at the advanced clock");
        assert_eq!(life, Life::Evaluated);
        assert_eq!(record.stage, TransitionRecord::EVALUATED);
    }

    #[test]
    fn program_env_answers_c1_pass_o1_true_and_nothing_else() {
        let env = program_env();
        let manifest = Manifest {
            transition_type: TT,
            parent_index: 0,
            parent_hash: [0; 32],
            declared_next_hash: [0xE1; 32],
            observation_id: [0xC0; 32],
            expires_at: 10_100,
            relevant_groups: BTreeSet::from([1u16]),
            action: None,
        };
        assert_eq!(env.constraint(&C1, &manifest), ConstraintStatus::Pass);
        assert_eq!(env.objective(&O1, &manifest), Some(true));
        assert_eq!(env.constraint(&[0x77; 32], &manifest), ConstraintStatus::Unavailable);
        assert_eq!(env.objective(&[0x78; 32], &manifest), None);
    }

    #[test]
    fn evaluate_under_noenv_stays_proposed() {
        // The premise slice 12 is built on, proven rather than assumed:
        // under the delivered crate's default NoEnv, C1 answers
        // Unavailable, so evaluate records unsatisfied constraints and
        // returns Ok without moving the life — a NoEnv evaluate could
        // never advance a stage, which is why the program runs this
        // step under ProgramEnv.
        let mut k = sealed_launch_kernel().expect("sealed NoEnv kernel must build");
        let observation = Observation {
            id: [0xC0; 32],
            timestamp: 100,
            expiry: 9100,
            confidence: Amount::from_raw(900_000),
        };
        k.ingest_observation(&OBSERVER, observation, 100)
            .expect("observer must ingest at the genesis clock");
        let manifest = Manifest {
            transition_type: TT,
            parent_index: 0,
            parent_hash: [0; 32],
            declared_next_hash: [0xE1; 32],
            observation_id: [0xC0; 32],
            expires_at: 10_100,
            relevant_groups: BTreeSet::from([1u16]),
            action: None,
        };
        k.register_transition(&ag(1), [0xD1; 32], manifest, BTreeSet::new(), 100)
            .expect("proposer must register at the genesis head");
        k.evaluate_transition(&[0xD1; 32], 100)
            .expect("evaluate under NoEnv returns Ok even when it cannot evaluate");
        assert_eq!(k.life_of(&[0xD1; 32]), Some(Life::Proposed));
    }

    fn eligible_pair() -> (KernelState, TransitionRecord, ObservationRecord) {
        let (state, mut record, obs) = evaluated_pair();
        evaluate_transition_from_records(&state, &mut record, &obs)
            .expect("pair must evaluate before marking eligible");
        assert_eq!(record.stage, TransitionRecord::EVALUATED);
        (state, record, obs)
    }

    #[test]
    fn mark_eligible_advances_evaluated_to_eligible() {
        let (state, mut record, obs) = eligible_pair();
        let life = mark_eligible_from_records(&state, &mut record, &obs)
            .expect("evaluated pair must mark eligible");
        assert_eq!(life, Life::Eligible);
        assert_eq!(record.stage, TransitionRecord::ELIGIBLE);
        assert_eq!(record.stage, 3);
        // Slice 14: the proven window is persisted alongside the stage —
        // the sealed genesis clock is 100, the launch policy's
        // consensus_window is 100, so the window is 100/200. The
        // record's other fields are untouched by the advance, and it
        // still serializes at exactly SPACE bytes.
        assert_eq!(record.eligible_at, 100);
        assert_eq!(record.consensus_deadline, 200);
        assert_eq!(record.transition_type, TT);
        assert_eq!(record.declared_next_hash, [0xE1; 32]);
        use anchor_lang::AccountSerialize;
        let mut bytes = Vec::new();
        record.try_serialize(&mut bytes).expect("serialize must succeed");
        assert_eq!(bytes.len(), TransitionRecord::SPACE);
    }

    #[test]
    fn mark_eligible_refuses_kernel_registered_stage() {
        let (state, mut record, obs) = evaluated_pair();
        let err = mark_eligible_from_records(&state, &mut record, &obs)
            .expect_err("a merely kernel-registered record has no evaluation to make eligible");
        assert!(format!("{:?}", err).contains("WrongStage"));
        assert_eq!(record.stage, TransitionRecord::KERNEL_REGISTERED);
    }

    #[test]
    fn mark_eligible_is_once_only() {
        let (state, mut record, obs) = eligible_pair();
        mark_eligible_from_records(&state, &mut record, &obs)
            .expect("first mark_eligible must succeed");
        let err = mark_eligible_from_records(&state, &mut record, &obs)
            .expect_err("second mark_eligible must refuse");
        assert!(format!("{:?}", err).contains("WrongStage"));
        assert_eq!(record.stage, TransitionRecord::ELIGIBLE);
    }

    #[test]
    fn mark_eligible_refuses_unsealed() {
        let (mut state, mut record, obs) = eligible_pair();
        state.sealed = false;
        let err = mark_eligible_from_records(&state, &mut record, &obs)
            .expect_err("unsealed mark_eligible must refuse");
        assert!(format!("{:?}", err).contains("NotSealed"));
        assert_eq!(record.stage, TransitionRecord::EVALUATED);
    }

    #[test]
    fn mark_eligible_refuses_parent_mismatch() {
        let (state, mut record, obs) = eligible_pair();
        record.parent_index = 1;
        let err = mark_eligible_from_records(&state, &mut record, &obs)
            .expect_err("parent off the sealed head must refuse");
        assert!(format!("{:?}", err).contains("ParentMismatch"));
        assert_eq!(record.stage, TransitionRecord::EVALUATED);
    }

    #[test]
    fn mark_eligible_refuses_observation_mismatch() {
        let (state, mut record, _) = eligible_pair();
        let other = build_observation_record(
            &state, [0xC1; 32], 100, 9100, 900_000, observer_pubkey(), 7,
        )
        .expect("second observation must build");
        let err = mark_eligible_from_records(&state, &mut record, &other)
            .expect_err("observation id mismatch must refuse");
        assert!(format!("{:?}", err).contains("RecordMismatch"));
        assert_eq!(record.stage, TransitionRecord::EVALUATED);
    }

    #[test]
    fn mark_eligible_refuses_unknown_proposer() {
        let (state, mut record, obs) = eligible_pair();
        record.proposer = Pubkey::new_from_array([0x99; 32]);
        let err = mark_eligible_from_records(&state, &mut record, &obs)
            .expect_err("unknown proposer must refuse");
        assert!(format!("{:?}", err).contains("UnknownProposer"));
        assert_eq!(record.stage, TransitionRecord::EVALUATED);
    }

    #[test]
    fn mark_eligible_refuses_non_observer_observation() {
        // Built as a struct literal: the slice-9 builder would refuse
        // this observer, so mark_eligible must catch a record that
        // bypassed it.
        let (state, mut record, _) = eligible_pair();
        let obs = ObservationRecord {
            observation_id: [0xC0; 32],
            timestamp: 100,
            expiry: 9100,
            confidence_raw: 900_000,
            observer: Pubkey::new_from_array(ag(1)),
            bump: 7,
        };
        let err = mark_eligible_from_records(&state, &mut record, &obs)
            .expect_err("non-observer observation must refuse");
        assert!(format!("{:?}", err).contains("NotObserver"));
        assert_eq!(record.stage, TransitionRecord::EVALUATED);
    }

    #[test]
    fn mark_eligible_at_advanced_clock() {
        let (state, mut record, obs) = eligible_pair();
        let mut advanced = state;
        advance_clock_on_state(&mut advanced, 200).expect("advance must succeed");
        let life = mark_eligible_from_records(&advanced, &mut record, &obs)
            .expect("pair must mark eligible at the advanced clock");
        assert_eq!(life, Life::Eligible);
        assert_eq!(record.stage, TransitionRecord::ELIGIBLE);
    }

    #[test]
    fn mark_eligible_persists_window_at_advanced_clock() {
        // Slice 14: the persisted window tracks the clock the kernel
        // actually ran at, not the genesis clock — 200 + the policy's
        // 100-tick consensus_window = a 200/300 window.
        let (state, mut record, obs) = eligible_pair();
        assert_eq!(record.eligible_at, 0);
        assert_eq!(record.consensus_deadline, 0);
        let mut advanced = state;
        advance_clock_on_state(&mut advanced, 200).expect("advance must succeed");
        mark_eligible_from_records(&advanced, &mut record, &obs)
            .expect("pair must mark eligible at the advanced clock");
        assert_eq!(record.stage, TransitionRecord::ELIGIBLE);
        assert_eq!(record.eligible_at, 200);
        assert_eq!(record.consensus_deadline, 300);
    }

    #[test]
    fn mark_eligible_refusal_leaves_window_zero() {
        // Slice 14: 0 means "eligibility never proven" — a refusal must
        // not write a window that could later be mistaken for one.
        let (state, mut record, obs) = eligible_pair();
        let mut advanced = state;
        advance_clock_on_state(&mut advanced, 9100).expect("advance must succeed");
        let err = mark_eligible_from_records(&advanced, &mut record, &obs)
            .expect_err("lapsed observation must refuse eligibility in the kernel");
        assert!(format!("{:?}", err).contains("EligibleFailed"));
        assert_eq!(record.stage, TransitionRecord::EVALUATED);
        assert_eq!(record.eligible_at, 0);
        assert_eq!(record.consensus_deadline, 0);
    }

    #[test]
    fn mark_eligible_refuses_lapsed_observation() {
        // Evaluation passed at the genesis clock, but the observation
        // window (expiry 9100) has lapsed by the advanced clock: the
        // kernel's freshness check at this step must refuse, not carry
        // a stale evaluation forward into eligibility.
        let (state, mut record, obs) = eligible_pair();
        let mut advanced = state;
        advance_clock_on_state(&mut advanced, 9100).expect("advance must succeed");
        let err = mark_eligible_from_records(&advanced, &mut record, &obs)
            .expect_err("lapsed observation must refuse eligibility in the kernel");
        assert!(format!("{:?}", err).contains("EligibleFailed"));
        assert_eq!(record.stage, TransitionRecord::EVALUATED);
    }
}
