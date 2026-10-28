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
//!
//! Slice 15 (this slice): the signal persistence slice 14 named —
//! the fourth §3 account, `SignalRecord`, and the `submit_signal`
//! instruction. One record per (transition, agent, round): the PDA
//! seeds bind the transition id, the signing agent's pubkey, and the
//! round, so a same-round duplicate is refused twice over — by the
//! `init` constraint on-chain and by the kernel's
//! `AlreadySignalledThisRound` in the helper replay. The helper
//! replays the full chain before the new signal — ingest + register +
//! evaluate + mark_eligible at the stored `eligible_at` (so the
//! rebuilt kernel's window is exactly the stored window), then every
//! supplied prior signal at its own `submitted_at`, then the new
//! signal at the state's clock — and returns the kernel's tally
//! (total weight, confidence) alongside the record, so the tally a
//! later select slice reads is a kernel-proven number, not a
//! program-side sum. Signals do not advance the transition stage: it
//! stays `ELIGIBLE`, and the record's window is untouched. What slice
//! 15 deliberately does NOT close: select itself, and the completeness
//! anchor select needs — `submit_signal` replays the priors the
//! caller supplies (remaining accounts on-chain), so a record that
//! could only pass by omitting a prior can never survive the select
//! replay, which replays the full set, but select cannot yet prove a
//! supplied set IS the full set; a count anchored on the transition
//! record, or an equivalent, is the select slice's work, named here
//! rather than claimed here.
//!
//! Slice 16 (this slice): the select instruction, `select_candidate`,
//! and the completeness anchor slice 15 named. The anchor is a
//! `signal_count` field on `TransitionRecord`, written by the same
//! instruction that creates a `SignalRecord` — `submit_signal` now
//! increments it on success only, so on-chain the count and the record
//! set cannot diverge: one instruction creates both or neither (a
//! failed instruction persists nothing), and a refusal leaves the count
//! untouched. Slice 15's "the transition record is not mutated" is
//! amended by this slice, not contradicted silently: signals still do
//! not advance the stage and still do not touch the window — only the
//! count moves, by exactly one per created record, revisions included
//! (a new round is a new record). Select's completeness proof is two
//! layers: the supplied prior set must number exactly `signal_count`,
//! and every supplied record must be genuine and distinct — a set that
//! omits a real record can only reach the count by duplicating one,
//! and a duplicated (agent, round) is refused explicitly before the
//! replay and again by the kernel's `AlreadySignalledThisRound` on
//! replay, so no omitted-record set can pass. The helper replays the
//! full chain — ingest + register + evaluate + mark_eligible at the
//! stored `eligible_at`, then every supplied prior at its own
//! `submitted_at` in order — and runs kernel `select_candidate` at
//! the state's clock, so the window-closure and quorum/confidence
//! gates are the kernel's, not a program-side sum. Honesty bound,
//! stated not smoothed: the rebuilt kernel holds only this transition,
//! so select proves this transition's own qualification and window
//! closure, not the kernel's sibling comparison — a higher-confidence
//! sibling that exists on-chain but is not replayed here cannot be
//! judged, and `OutrankedBySibling` / `SiblingsStillOpen` /
//! `ParentAlreadyHasASelectedTransition` are unreachable in this
//! slice. Sibling replay is the next slice's work, named here rather
//! than claimed here. Select is permissionless, as the kernel's is:
//! the payer is a fee payer only, and the stage advances to
//! `SELECTED` (4) only on a kernel-proven selection.
//!
//! Slice 17 (this slice): sibling replay for select — the
//! `select_candidate_with_siblings` instruction. The caller supplies
//! the target's priors plus, for each competing sibling, its stored
//! transition record, its observation record, and its stored signals;
//! the helper replays every supplied transition into one rebuilt
//! kernel and runs kernel `select_candidate` on the target there, so
//! the kernel's sibling arms can now fire: a sibling whose window is
//! still open refuses with `SiblingsStillOpen`, a qualifying sibling
//! with higher confidence (or equal confidence and an earlier
//! eligibility) refuses with `OutrankedBySibling`, and a target that
//! outranks every supplied sibling selects. Replay runs as ONE
//! merged timeline, not two phases: eligibility chains (ingest +
//! register + evaluate + mark_eligible at the stored `eligible_at`)
//! and signals (at their `submitted_at`) are interleaved in ascending
//! time order, chains before signals at an equal time — two phases
//! would regress the kernel clock whenever a transition's signal
//! predates another transition's eligibility, which is the normal
//! interleaved case. The mark-eligible order is the kernel's
//! siblings-vec order, and `eligible_at` IS that order — the clock
//! only moves forward, so an earlier `eligible_at` was necessarily
//! marked eligible earlier. The one case the clock cannot order is
//! equal `eligible_at`: siblings replay before the target there, so
//! an equal-confidence tie at equal eligibility refuses the target —
//! the fail-closed direction, stated not smoothed. Each sibling's supplied signal set must number exactly
//! its own `signal_count` (the slice-16 anchor, per transition), and
//! a sibling at any stage but `ELIGIBLE` refuses: a `SELECTED`
//! sibling's own select replay — the `ParentAlreadyHasASelected`
//! arm — is the next slice's work, not smuggled in here. Honesty
//! bounds, stated not smoothed: this slice proves the comparison
//! over the siblings SUPPLIED; it does not prove the supplied set is
//! complete — no count anchor for siblings exists on `KernelState`
//! (slice 16's anchor covers one transition's signals, not the
//! sibling field), so omitting a blocking sibling is still a caller
//! omission the program cannot catch, and that anchor is the next
//! slice's work after the already-selected replay.
//!
//! Slice 18 (this slice): the already-selected sibling replay slice
//! 17 named. Slice 17's "a sibling at any stage but `ELIGIBLE`
//! refuses" is amended, not contradicted silently: a sibling at
//! stage `SELECTED` (4) is now admitted, and every other non-`ELIGIBLE`
//! stage still refuses. A `SELECTED` stage label is NOT trusted as
//! proof of selection — the helper replays the sibling's own select
//! in the rebuilt kernel, at the state's clock, after the merged
//! timeline and before the target's select, and verifies the
//! sibling's life becomes `Selected` there. Only then can the
//! kernel's `ParentAlreadyHasASelectedTransition` arm fire on the
//! target's select: the sibling is no longer `Eligible`, so the
//! sibling loop skips it, and `selected_for` already names it for
//! this parent. A `SELECTED` sibling whose own select cannot be
//! proven — unqualified tally, an outranking target, a window not
//! yet closed — refuses the whole helper with `SelectFailed`, and
//! the target stays `ELIGIBLE`: a stored stage that history cannot
//! reproduce is an inconsistent record, not a block. At most one
//! `SELECTED` sibling is admitted: a parent can have only one
//! selected transition, so two supplied `SELECTED` siblings refuse
//! before any replay. The selected sibling's select runs at the
//! state's clock, the same clock the target's select runs at —
//! `tick` accepts an equal time, and call order, not a stored
//! selection timestamp (no such field exists on the record), is
//! what makes the sibling "already" selected: its select is called
//! first. Honesty bounds, stated not smoothed: the sibling-set
//! completeness anchor is still open — this slice proves the
//! comparison over the siblings supplied, selected included, not
//! that the supplied set is complete; and later-stage siblings
//! (planning onward, stages 5+) do not exist in the program yet,
//! so their `ParentAlreadyHasASelected` lives (4..=8, `Blocked`,
//! `Committed` in the kernel) are not replayable here until those
//! stages land.
//!
//! Slice 19 (this slice): the sibling-set completeness anchor slice
//! 17 named and slice 18 left open. `KernelState` gains a census of
//! the eligible field at one parent — `eligible_sibling_parent_index`
//! / `eligible_sibling_parent_hash` plus `eligible_sibling_count` —
//! written by the same instruction that proves eligibility, on
//! success only, exactly as slice 16's `signal_count` is written by
//! `submit_signal`: one instruction advances the stage and the
//! census or neither, so the count and the eligible set cannot
//! diverge on-chain. A transition that becomes eligible at the
//! census parent increments the count; the first eligible transition
//! at a new parent resets the census to that parent with count 1.
//! The kernel's sibling loop judges only `Eligible` siblings at the
//! target's parent (plus the `selected_for` record, which was itself
//! eligible before it was selected), so the census counts exactly
//! the field a select must judge: every transition that ever became
//! eligible at this parent, selected or still eligible, expired or
//! not — expiry is judged by the kernel at select time, and an
//! omitted expired sibling is indistinguishable on-chain from an
//! omitted live one, so the anchor counts it and the caller supplies
//! it. Both select helpers read the census before any replay: the
//! plain select requires count exactly 1 at the target's parent
//! (otherwise a second eligible transition exists and the plain
//! path — which rebuilds a kernel holding only the target — would
//! silently bypass the comparison), and the sibling select requires
//! supplied siblings + target to equal the count at the target's
//! parent. A stale census cannot authorize a select at a new parent:
//! both helpers also require the census parent to equal the target's
//! parent, and the next `mark_eligible` at the new parent resets the
//! census lazily — the future commit-replay slice that moves the head
//! need not reset it, and must not trust it across the move either.
//! Honesty bounds, stated not smoothed: the census is a count, not
//! an enumeration — it proves the supplied field has the right
//! size, and the per-sibling gates (distinct id, parent, linkage,
//! window, per-sibling `signal_count`, no duplicated agent-round)
//! prove each supplied member is genuine; a caller who supplies a
//! wrong-but-right-sized field is caught by the replay, not by the
//! count. Later-stage siblings (planning onward) are still not
//! suppliable: the with-siblings gate admits only `ELIGIBLE` /
//! `SELECTED`, so a sibling advanced past selection cannot be
//! supplied — but that select must fail anyway, since
//! `selected_for` already names it, so the gate's refusal is the
//! correct outcome by the wrong door, and the planning slice must
//! widen the gate or name that refusal properly.
//!
//! Slice 20 (this slice): the first planning instruction,
//! `open_planning` — `SELECTED` (4) -> `PLANNING` (5). The helper
//! re-proves the whole selection chain in a rebuilt kernel (chain +
//! priors + select at the state's clock, exactly as slice 16's
//! select helper proves it) and only then runs kernel
//! `open_planning` at the state's clock, so a stored `SELECTED`
//! label a replay cannot reproduce never opens planning — the
//! stage label is re-proven, never trusted, as in slices 17–18.
//! The kernel sets the planning deadline to `now +
//! planning_window`; the program persists exactly that value, with
//! `now` the state's clock and the window the launch policy's
//! (the sealed launch kernel installs `launch_policy()` for every
//! type, so this is the kernel's own policy value, not a
//! program-side guess), on `TransitionRecord.planning_deadline`
//! — 0 until planning opens. The caller is the signing payer's
//! agent id, and the kernel's Selected-path rule is "anyone": the
//! tests prove a non-proposer, non-agent payer opens planning,
//! and a proposer-only reading would contradict the kernel.
//! Honesty bounds, stated not smoothed: this slice opens planning
//! only — no plan is submitted here (`submit_plan` is the next
//! slice, with its own record account), and the Blocked -> Planning
//! re-plan path is NOT opened by this instruction: the stage gate
//! is exactly `SELECTED`, so a `Blocked` record refuses here even
//! though the kernel would admit a proposer-or-seat re-plan —
//! re-plan is later work, named here rather than smuggled in.
//! Planning on a sibling field is likewise not re-judged here:
//! selection was already proven by the select slices, and this
//! helper's rebuilt kernel holds only this transition.

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
    /// so a later select slice reads a real field, and advances the
    /// `KernelState` sibling census (slice 19) for the record's parent,
    /// so a later select slice reads a real field count, not a
    /// caller assertion. The payer is a fee payer only — eligibility
    /// executes steps the stored records already authorized and
    /// creates no new authority.
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
            &mut ctx.accounts.kernel_state,
            &mut ctx.accounts.transition_record,
            &ctx.accounts.observation_record,
        )?;
        emit!(TransitionMarkedEligible { transition_id, stage: TransitionRecord::ELIGIBLE });
        Ok(())
    }

    /// Submit a signal (§4.1, fourth replay row, v0.4 form): run kernel
    /// `submit_round_signal` for the signing payer's agent on an
    /// `ELIGIBLE` record pair and persist it as a `SignalRecord` PDA.
    /// The PDA seeds bind the transition id, the signer's pubkey, and
    /// the round (an instruction argument the helper re-derives and
    /// refuses on mismatch, never trusts). Prior signals for the
    /// transition arrive as remaining accounts and are replayed by the
    /// helper in `submitted_at` order, so the kernel judges the new
    /// signal against the same tally history the chain holds — a
    /// same-round duplicate and a revision away from the last round's
    /// mean are refused by the kernel, not by a program-side guess.
    /// Signals do not advance the transition record's stage, and its
    /// window is untouched; slice 16 adds exactly one mutation — the
    /// record's `signal_count` increments on success, the completeness
    /// anchor the select slice reads. The payer is a fee payer
    /// and the signal's agent in one: the identity mapping means the
    /// signer proves the agent by signing, and no other agent's signal
    /// can be submitted through this instruction.
    pub fn submit_signal(
        ctx: Context<SubmitSignal>,
        transition_id: [u8; 32],
        observation_id: [u8; 32],
        round: u64,
        raw: u64,
        support_raw: u64,
        has_support: bool,
    ) -> Result<()> {
        if ctx.accounts.transition_record.transition_id != transition_id
            || ctx.accounts.observation_record.observation_id != observation_id
        {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        let mut priors: alloc::vec::Vec<SignalRecord> = alloc::vec::Vec::new();
        for acc in ctx.remaining_accounts {
            let parsed = Account::<SignalRecord>::try_from(acc)
                .map_err(|_| error!(ProgramV04Error::RecordMismatch))?;
            priors.push(parsed.into_inner());
        }
        let (record, total, confidence) = submit_signal_from_records(
            &ctx.accounts.kernel_state,
            &mut ctx.accounts.transition_record,
            &ctx.accounts.observation_record,
            &priors,
            ctx.accounts.payer.key(),
            round,
            raw,
            support_raw,
            has_support,
            ctx.bumps.signal_record,
        )?;
        *ctx.accounts.signal_record = record;
        emit!(SignalSubmitted {
            transition_id,
            agent: ctx.accounts.payer.key(),
            round,
            total_weight_raw: total.raw(),
            confidence_raw: confidence.raw(),
        });
        Ok(())
    }

    /// Select a candidate (§4.1, fifth replay row, v0.4 form): run
    /// kernel `select_candidate` on an `ELIGIBLE` record pair and
    /// advance the transition record's stage to `SELECTED`. Prior
    /// signals for the transition arrive as remaining accounts and
    /// must number exactly the record's `signal_count` — the slice-16
    /// completeness anchor — or the instruction refuses before any
    /// replay; a set that omits a real record can only reach the
    /// count by duplicating one, and duplicates are refused. The
    /// payer is a fee payer only: selection is permissionless once
    /// the consensus window has closed, exactly as the kernel's is.
    /// Slice 19: this instruction is now the sole-eligible path —
    /// the `KernelState` sibling census at the target's parent must
    /// read exactly 1, so a second eligible competitor cannot be
    /// bypassed through this instruction; when competitors exist,
    /// the sibling-aware instruction is the only select path. The
    /// rebuilt kernel holds only this transition, so this
    /// instruction proves own qualification and window closure, and
    /// the census proves no competitor exists to judge.
    pub fn select_candidate(
        ctx: Context<SelectCandidate>,
        transition_id: [u8; 32],
        observation_id: [u8; 32],
    ) -> Result<()> {
        if ctx.accounts.transition_record.transition_id != transition_id
            || ctx.accounts.observation_record.observation_id != observation_id
        {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        let mut priors: alloc::vec::Vec<SignalRecord> = alloc::vec::Vec::new();
        for acc in ctx.remaining_accounts {
            let parsed = Account::<SignalRecord>::try_from(acc)
                .map_err(|_| error!(ProgramV04Error::RecordMismatch))?;
            priors.push(parsed.into_inner());
        }
        select_candidate_from_records(
            &ctx.accounts.kernel_state,
            &mut ctx.accounts.transition_record,
            &ctx.accounts.observation_record,
            &priors,
        )?;
        emit!(TransitionSelected { transition_id, stage: TransitionRecord::SELECTED });
        Ok(())
    }

    /// Select a candidate with sibling replay (§4.1, fifth replay row,
    /// v0.4 form, slice 17): the slice-16 select plus the competing
    /// field. Remaining accounts carry the sibling evidence, one
    /// account per stored record, classified by discriminator:
    /// sibling `TransitionRecord`s, their `ObservationRecord`s, and
    /// `SignalRecord`s (target priors and sibling priors alike, told
    /// apart by each signal's `transition_id`). A signal for no
    /// supplied transition, a sibling without exactly one matching
    /// observation, an observation matching no sibling, a duplicated
    /// sibling id, or a remaining account that is none of the three
    /// record types refuses with `RecordMismatch` before any replay.
    /// The payer is a fee payer only, as in slice 16; only the target
    /// record's stage advances, to `SELECTED`, and only on a
    /// kernel-proven selection over the supplied field. Slice 18:
    /// a sibling at stage `SELECTED` is replayed through its own
    /// select at the state's clock before the target's select, so
    /// the kernel's `ParentAlreadyHasASelectedTransition` arm can
    /// fire; a `SELECTED` sibling that cannot be proven selected
    /// refuses the whole instruction, and at most one `SELECTED`
    /// sibling may be supplied. Slice 19: the supplied field plus
    /// the target must number exactly the `KernelState` sibling
    /// census at the target's parent, or the instruction refuses
    /// before any replay — an omitted eligible competitor can no
    /// longer pass unjudged.
    pub fn select_candidate_with_siblings(
        ctx: Context<SelectCandidateWithSiblings>,
        transition_id: [u8; 32],
        observation_id: [u8; 32],
    ) -> Result<()> {
        if ctx.accounts.transition_record.transition_id != transition_id
            || ctx.accounts.observation_record.observation_id != observation_id
        {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        let mut sibling_transitions: alloc::vec::Vec<TransitionRecord> = alloc::vec::Vec::new();
        let mut sibling_observations: alloc::vec::Vec<ObservationRecord> = alloc::vec::Vec::new();
        let mut all_signals: alloc::vec::Vec<SignalRecord> = alloc::vec::Vec::new();
        for acc in ctx.remaining_accounts {
            if let Ok(parsed) = Account::<TransitionRecord>::try_from(acc) {
                sibling_transitions.push(parsed.into_inner());
                continue;
            }
            if let Ok(parsed) = Account::<ObservationRecord>::try_from(acc) {
                sibling_observations.push(parsed.into_inner());
                continue;
            }
            let parsed = Account::<SignalRecord>::try_from(acc)
                .map_err(|_| error!(ProgramV04Error::RecordMismatch))?;
            all_signals.push(parsed.into_inner());
        }
        for (i, tr) in sibling_transitions.iter().enumerate() {
            if tr.transition_id == transition_id
                || sibling_transitions[..i]
                    .iter()
                    .any(|t| t.transition_id == tr.transition_id)
            {
                return Err(error!(ProgramV04Error::RecordMismatch));
            }
        }
        let mut sibling_signal_sets: alloc::vec::Vec<alloc::vec::Vec<SignalRecord>> =
            alloc::vec::Vec::new();
        for _ in &sibling_transitions {
            sibling_signal_sets.push(alloc::vec::Vec::new());
        }
        let mut target_priors: alloc::vec::Vec<SignalRecord> = alloc::vec::Vec::new();
        for sig in all_signals {
            if sig.transition_id == transition_id {
                target_priors.push(sig);
                continue;
            }
            match sibling_transitions
                .iter()
                .position(|t| t.transition_id == sig.transition_id)
            {
                Some(i) => sibling_signal_sets[i].push(sig),
                None => return Err(error!(ProgramV04Error::RecordMismatch)),
            }
        }
        let mut siblings: alloc::vec::Vec<SiblingReplay> = alloc::vec::Vec::new();
        for (i, tr) in sibling_transitions.iter().enumerate() {
            let matches: alloc::vec::Vec<&ObservationRecord> = sibling_observations
                .iter()
                .filter(|o| o.observation_id == tr.observation_id)
                .collect();
            if matches.len() != 1 {
                return Err(error!(ProgramV04Error::RecordMismatch));
            }
            siblings.push(SiblingReplay {
                transition_record: tr,
                observation_record: matches[0],
                prior_signals: &sibling_signal_sets[i],
            });
        }
        for obs in &sibling_observations {
            if !sibling_transitions
                .iter()
                .any(|t| t.observation_id == obs.observation_id)
            {
                return Err(error!(ProgramV04Error::RecordMismatch));
            }
        }
        select_candidate_from_records_with_siblings(
            &ctx.accounts.kernel_state,
            &mut ctx.accounts.transition_record,
            &ctx.accounts.observation_record,
            &target_priors,
            &siblings,
        )?;
        emit!(TransitionSelected { transition_id, stage: TransitionRecord::SELECTED });
        Ok(())
    }

    /// Open planning (§4.1, sixth replay row, v0.4 form, slice 20):
    /// run kernel `open_planning` on a `SELECTED` record pair and
    /// advance the transition record's stage to `PLANNING`, persisting
    /// the planning deadline the kernel sets (`clock +
    /// planning_window`). Prior signals for the transition arrive as
    /// remaining accounts and must number exactly the record's
    /// `signal_count` — the slice-16 anchor — because the helper
    /// re-proves the selection chain before opening planning; a
    /// stored `SELECTED` label alone opens nothing. The payer is a
    /// fee payer and, for the Selected path, the kernel's planning
    /// caller: the kernel admits anyone there, so no proposer or
    /// seat gate applies in this instruction. A `Blocked` record
    /// refuses here: the kernel's proposer-or-seat re-plan path is
    /// later work, not opened by this instruction.
    pub fn open_planning(
        ctx: Context<OpenPlanning>,
        transition_id: [u8; 32],
        observation_id: [u8; 32],
    ) -> Result<()> {
        if ctx.accounts.transition_record.transition_id != transition_id
            || ctx.accounts.observation_record.observation_id != observation_id
        {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        let mut priors: alloc::vec::Vec<SignalRecord> = alloc::vec::Vec::new();
        for acc in ctx.remaining_accounts {
            let parsed = Account::<SignalRecord>::try_from(acc)
                .map_err(|_| error!(ProgramV04Error::RecordMismatch))?;
            priors.push(parsed.into_inner());
        }
        let life = open_planning_from_records(
            &ctx.accounts.kernel_state,
            &mut ctx.accounts.transition_record,
            &ctx.accounts.observation_record,
            &priors,
            resolve_agent_id(ctx.accounts.payer.key()),
        )?;
        debug_assert_eq!(life, Life::Planning);
        emit!(TransitionPlanningOpened {
            transition_id,
            stage: TransitionRecord::PLANNING,
            planning_deadline: ctx.accounts.transition_record.planning_deadline,
        });
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(transition_id: [u8; 32], observation_id: [u8; 32])]
pub struct OpenPlanning<'info> {
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
pub struct SelectCandidate<'info> {
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
pub struct SelectCandidateWithSiblings<'info> {
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
#[instruction(transition_id: [u8; 32], observation_id: [u8; 32], round: u64, raw: u64, support_raw: u64, has_support: bool)]
pub struct SubmitSignal<'info> {
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
    #[account(
        init,
        payer = payer,
        space = SignalRecord::SPACE,
        seeds = [b"signal", transition_id.as_ref(), payer.key().as_ref(), &round.to_le_bytes()],
        bump,
    )]
    pub signal_record: Account<'info, SignalRecord>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(transition_id: [u8; 32], observation_id: [u8; 32])]
pub struct MarkEligible<'info> {
    #[account(
        mut,
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
    /// Parent index of the sibling census (slice 19): the parent at
    /// which `eligible_sibling_count` transitions became eligible.
    /// Genesis: the genesis head (0).
    pub eligible_sibling_parent_index: u64,
    /// Parent hash of the sibling census (slice 19). Genesis: the
    /// genesis head hash (zero).
    pub eligible_sibling_parent_hash: [u8; 32],
    /// Number of transitions that became eligible at the census
    /// parent (slice 19: the sibling-set completeness anchor).
    /// Written by `mark_eligible` on success only; genesis: 0.
    pub eligible_sibling_count: u64,
    /// PDA bump for the `kernel-state` seeds.
    pub bump: u8,
}

impl KernelState {
    /// 8 (discriminator) + 1 + 8 + 32 + 8 + 8 + 32 + 8 + 1.
    pub const SPACE: usize = 106;
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
    /// (slice 13 advances it), 4 = Selected (slice 16 advances it).
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
    /// Planning deadline `clock + planning_window` of the sealed
    /// launch policy (slice 20 persists it). 0 until planning opens.
    pub planning_deadline: u64,
    /// Number of `SignalRecord`s created for this transition (slice 16:
    /// the select completeness anchor). Written by `submit_signal` on
    /// success only — one per created record, revisions included —
    /// so the count and the record set cannot diverge on-chain.
    pub signal_count: u64,
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
    /// Stage value written by `select_candidate` (slice 16).
    pub const SELECTED: u8 = 4;
    /// Stage value written by `open_planning` (slice 20).
    pub const PLANNING: u8 = 5;
    /// 8 (discriminator) + 32 + 32 + 8 + 32 + 32 + 32 + 8 + 32 + 1 + 8 + 8 + 8 + 8 + 1.
    pub const SPACE: usize = 250;
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
        planning_deadline: 0,
        signal_count: 0,
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
    kernel_state: &mut KernelState,
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
    // Slice 19: advance the sibling census in the same success path
    // as the stage — one instruction writes both or neither, so the
    // count and the eligible set cannot diverge. A transition
    // becoming eligible at a new parent resets the census to that
    // parent with count 1 (the head moved since the last census
    // write; the select gates require the census parent to equal the
    // target's parent, so a stale census can never authorize a
    // select at the new parent before this reset lands). Overflow
    // is fail-closed: the count cannot wrap into a smaller field.
    if kernel_state.eligible_sibling_parent_index != transition_record.parent_index
        || kernel_state.eligible_sibling_parent_hash != transition_record.parent_hash
    {
        kernel_state.eligible_sibling_parent_index = transition_record.parent_index;
        kernel_state.eligible_sibling_parent_hash = transition_record.parent_hash;
        kernel_state.eligible_sibling_count = 1;
    } else {
        kernel_state.eligible_sibling_count = kernel_state
            .eligible_sibling_count
            .checked_add(1)
            .ok_or(error!(ProgramV04Error::EligibleFailed))?;
    }
    Ok(life)
}

/// The fourth §3 account: one stored round signal (slice 15). One
/// record per (transition, agent, round) — the PDA seeds are
/// `["signal", transition_id, agent, round_le]` — because the kernel
/// allows one signal per agent per round and a revision means a later
/// round: a new round is a new record, never an overwrite, so the
/// record set is the audit trail the select replay walks. The latest
/// record per agent is the one the kernel counts; earlier rounds stay
/// as history, exactly as the kernel's `round_sigs` map keeps them.
/// `submitted_at` is the state's clock at submission: the replay clock
/// a later slice uses to rebuild that signal's round exactly.
#[account]
#[derive(Debug, Copy)]
pub struct SignalRecord {
    /// Transition this signal belongs to (the PDA seed after `"signal"`).
    pub transition_id: [u8; 32],
    /// Signalling agent (the signer's pubkey, by the identity mapping).
    pub agent: Pubkey,
    /// Consensus round this signal was submitted in, derived from the
    /// stored eligibility window and the clock at submission.
    pub round: u64,
    /// Raw signal weight in kernel `Amount` raw units (SCALE = 1.0).
    pub raw: u64,
    /// Support in kernel `Amount` raw units; 0 when `has_support` is false.
    pub support_raw: u64,
    /// False means "no position": recorded and classified by the
    /// kernel, but adding no weight (the kernel's `support == None`).
    pub has_support: bool,
    /// State clock at submission (the replay clock for this record).
    pub submitted_at: u64,
    /// PDA bump for the `["signal", transition_id, agent, round]` seeds.
    pub bump: u8,
}

impl SignalRecord {
    /// 8 (discriminator) + 32 + 32 + 8 + 8 + 8 + 1 + 8 + 1.
    pub const SPACE: usize = 106;
}

/// Derive a consensus round from a stored eligibility window and a
/// clock, mirroring the kernel's `round_of` exactly: rounds are slices
/// of the window, `(deadline - eligible_at) / max_rounds` ticks each
/// (at least 1), capped at the last round. The program derives rather
/// than trusts: the instruction's round argument must equal this, and
/// every prior record's stored round must equal this at its own
/// `submitted_at`, or the helper refuses. The policy is the sealed
/// launch policy's, never a caller-supplied value.
pub fn signal_round(eligible_at: u64, consensus_deadline: u64, now: u64) -> u64 {
    let max_rounds = launch_policy().max_rounds as u64;
    let slice = (consensus_deadline.saturating_sub(eligible_at) / max_rounds).max(1);
    let r = now.saturating_sub(eligible_at) / slice;
    if r >= max_rounds {
        max_rounds - 1
    } else {
        r
    }
}

/// Submit a signal through the real kernel (slice 15; slice 16 amends
/// the mutation rule). Pure except for the returned record and the
/// record's `signal_count`, which increments by one on success only:
/// signals do not advance the stage and do not touch the window, and
/// every refusal leaves everything untouched. Gates, in order: sealed state; stage exactly `ELIGIBLE`
/// (a merely-evaluated record has no consensus window to signal in,
/// and signalling never moves the stage); parent exactly the sealed
/// head; observation linkage; a proven window (`eligible_at` nonzero
/// and the deadline after it — a stage-ELIGIBLE record with a zero
/// window was never proven eligible, so it cannot anchor a round);
/// the round argument equal to the round derived from the stored
/// window and the state's clock; every prior record belonging to this
/// transition with a stored round matching its own `submitted_at`
/// (a prior that contradicts the window math is not a record this
/// transition produced). The kernel calls run against a freshly
/// rebuilt sealed launch kernel under `ProgramEnv`: ingest, register,
/// evaluate, and mark_eligible, all at the stored `eligible_at` (so the
/// rebuilt kernel's window IS the stored window — the kernel exposes
/// no `eligible_at` accessor, so replaying at the stored clock is what
/// reproduces it), then each prior signal at its `submitted_at` in
/// order, then the new signal at the state's clock. A kernel refusal
/// in any call maps to `SignalFailed`: a same-round duplicate, a
/// revision away from the last round's mean, a signal at or past the
/// deadline, an unknown or inactive agent, and an over-range raw or
/// support all fail here, through the kernel, not through a
/// program-side guess about them. On success the return is the new
/// record plus the kernel's tally after it — total weight and
/// confidence in raw units — so a later select slice reads a
/// kernel-proven tally, not a program-side sum. A caller-supplied
/// `support_raw` with `has_support` false is normalized to 0 in the
/// record: the record is canonical, `has_support` false ⟺
/// `support_raw` 0. Completeness bound, stated not smoothed: this
/// helper replays the priors it is given; a record that could only
/// pass by omitting a prior can never survive the select replay,
/// which replays the full set — and slice 16's `signal_count` on the
/// record is the anchor select checks that set against (see module
/// docs).
#[allow(clippy::too_many_arguments)]
pub fn submit_signal_from_records(
    kernel_state: &KernelState,
    transition_record: &mut TransitionRecord,
    observation_record: &ObservationRecord,
    prior_signals: &[SignalRecord],
    agent: Pubkey,
    round: u64,
    raw: u64,
    support_raw: u64,
    has_support: bool,
    bump: u8,
) -> Result<(SignalRecord, Amount, Amount)> {
    if !kernel_state.sealed {
        return Err(error!(ProgramV04Error::NotSealed));
    }
    if transition_record.stage != TransitionRecord::ELIGIBLE {
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
    if transition_record.eligible_at == 0
        || transition_record.consensus_deadline <= transition_record.eligible_at
    {
        return Err(error!(ProgramV04Error::SignalFailed));
    }
    let derived = signal_round(
        transition_record.eligible_at,
        transition_record.consensus_deadline,
        kernel_state.clock,
    );
    if derived != round {
        return Err(error!(ProgramV04Error::SignalFailed));
    }
    for prior in prior_signals {
        if prior.transition_id != transition_record.transition_id {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        if signal_round(
            transition_record.eligible_at,
            transition_record.consensus_deadline,
            prior.submitted_at,
        ) != prior.round
        {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
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
    let eligible_at = transition_record.eligible_at;
    k.ingest_observation(&observer_id, observation, eligible_at)
        .map_err(|_| error!(ProgramV04Error::SignalFailed))?;
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
        eligible_at,
    )
    .map_err(|_| error!(ProgramV04Error::SignalFailed))?;
    k.evaluate_transition(&transition_record.transition_id, eligible_at)
        .map_err(|_| error!(ProgramV04Error::SignalFailed))?;
    k.mark_eligible(&transition_record.transition_id, eligible_at)
        .map_err(|_| error!(ProgramV04Error::SignalFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Eligible) => {}
        _ => return Err(error!(ProgramV04Error::SignalFailed)),
    }
    let mut priors: alloc::vec::Vec<SignalRecord> = prior_signals.to_vec();
    priors.sort_by_key(|s| s.submitted_at);
    for prior in &priors {
        let support = if prior.has_support {
            Some(Amount::from_raw(prior.support_raw))
        } else {
            None
        };
        k.submit_round_signal(
            &prior.agent.to_bytes(),
            &transition_record.transition_id,
            Amount::from_raw(prior.raw),
            support,
            prior.submitted_at,
        )
        .map_err(|_| error!(ProgramV04Error::SignalFailed))?;
    }
    let agent_id = resolve_agent_id(agent);
    let support = if has_support {
        Some(Amount::from_raw(support_raw))
    } else {
        None
    };
    k.submit_round_signal(
        &agent_id,
        &transition_record.transition_id,
        Amount::from_raw(raw),
        support,
        kernel_state.clock,
    )
    .map_err(|_| error!(ProgramV04Error::SignalFailed))?;
    let total = k
        .total_weight(&transition_record.transition_id)
        .map_err(|_| error!(ProgramV04Error::SignalFailed))?;
    let confidence = k
        .confidence(&transition_record.transition_id)
        .map_err(|_| error!(ProgramV04Error::SignalFailed))?;
    let record = SignalRecord {
        transition_id: transition_record.transition_id,
        agent,
        round,
        raw,
        support_raw: if has_support { support_raw } else { 0 },
        has_support,
        submitted_at: kernel_state.clock,
        bump,
    };
    // Slice 16: the completeness anchor moves here, after the kernel
    // has proven the signal — a refusal above leaves the count
    // untouched, and the count increments exactly once per created
    // record. The kernel object is local and discarded on error, so a
    // count failure here cannot strand a half-created signal.
    transition_record.signal_count = transition_record
        .signal_count
        .checked_add(1)
        .ok_or_else(|| error!(ProgramV04Error::SignalFailed))?;
    Ok((record, total, confidence))
}

/// Select a candidate through the real kernel (slice 16). Pure except
/// for the record's stage, which advances to `SELECTED` only on
/// success — every refusal leaves the record untouched. Gates, in
/// order: sealed state; stage exactly `ELIGIBLE` (select runs only on
/// a proven eligibility window); parent exactly the sealed head;
/// observation linkage; a proven window (`eligible_at` nonzero and
/// the deadline after it); the completeness anchor — the supplied
/// prior set must number exactly `signal_count`, or the select
/// refuses before any replay, since an incomplete set can never prove
/// the kernel's tally; every prior belonging to this transition with
/// a stored round matching its own `submitted_at`; no duplicated
/// (agent, round) in the supplied set. The kernel calls run against a
/// freshly rebuilt sealed launch kernel under `ProgramEnv`: ingest,
/// register, evaluate, and mark_eligible, all at the stored
/// `eligible_at` (so the rebuilt kernel's window IS the stored
/// window), then each prior signal at its `submitted_at` in order,
/// then kernel `select_candidate` at the state's clock. A kernel
/// refusal in any call maps to `SelectFailed`: a still-open window,
/// an unmet quorum, confidence below the launch threshold, an
/// expired manifest, and a signal at or past the deadline during the
/// prior replay all fail here, through the kernel, not through a
/// program-side guess about them. Only a `Life::Selected` life
/// advances the stage. Sibling competition is NOT proven here: the
/// rebuilt kernel holds only this transition, so the kernel's
/// sibling arms cannot fire in this slice — a later slice must replay
/// sibling transitions before select can claim the full comparison
/// (see module docs).
pub fn select_candidate_from_records(
    kernel_state: &KernelState,
    transition_record: &mut TransitionRecord,
    observation_record: &ObservationRecord,
    prior_signals: &[SignalRecord],
) -> Result<Life> {
    if !kernel_state.sealed {
        return Err(error!(ProgramV04Error::NotSealed));
    }
    if transition_record.stage != TransitionRecord::ELIGIBLE {
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
    if transition_record.eligible_at == 0
        || transition_record.consensus_deadline <= transition_record.eligible_at
    {
        return Err(error!(ProgramV04Error::SelectFailed));
    }
    if prior_signals.len() as u64 != transition_record.signal_count {
        return Err(error!(ProgramV04Error::SelectFailed));
    }
    for prior in prior_signals {
        if prior.transition_id != transition_record.transition_id {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        if signal_round(
            transition_record.eligible_at,
            transition_record.consensus_deadline,
            prior.submitted_at,
        ) != prior.round
        {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
    }
    {
        let mut seen: BTreeSet<([u8; 32], u64)> = BTreeSet::new();
        for prior in prior_signals {
            if !seen.insert((prior.agent.to_bytes(), prior.round)) {
                return Err(error!(ProgramV04Error::SelectFailed));
            }
        }
    }
    // Slice 19: the sibling-set completeness anchor for the plain
    // path. The rebuilt kernel below holds only this transition, so
    // without this gate a second eligible transition at the same
    // parent would be silently bypassed — the plain select would
    // prove own qualification while a competitor exists unjudged.
    // The target itself is eligible, so the census at its parent
    // must read exactly 1; anything else (a missing census, a
    // census at another parent, a count above 1) refuses before any
    // replay, and the sibling-aware instruction is the only path
    // when competitors exist.
    if kernel_state.eligible_sibling_parent_index != transition_record.parent_index
        || kernel_state.eligible_sibling_parent_hash != transition_record.parent_hash
        || kernel_state.eligible_sibling_count != 1
    {
        return Err(error!(ProgramV04Error::SelectFailed));
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
    let eligible_at = transition_record.eligible_at;
    k.ingest_observation(&observer_id, observation, eligible_at)
        .map_err(|_| error!(ProgramV04Error::SelectFailed))?;
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
        eligible_at,
    )
    .map_err(|_| error!(ProgramV04Error::SelectFailed))?;
    k.evaluate_transition(&transition_record.transition_id, eligible_at)
        .map_err(|_| error!(ProgramV04Error::SelectFailed))?;
    k.mark_eligible(&transition_record.transition_id, eligible_at)
        .map_err(|_| error!(ProgramV04Error::SelectFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Eligible) => {}
        _ => return Err(error!(ProgramV04Error::SelectFailed)),
    }
    let mut priors: alloc::vec::Vec<SignalRecord> = prior_signals.to_vec();
    priors.sort_by_key(|s| s.submitted_at);
    for prior in &priors {
        let support = if prior.has_support {
            Some(Amount::from_raw(prior.support_raw))
        } else {
            None
        };
        k.submit_round_signal(
            &prior.agent.to_bytes(),
            &transition_record.transition_id,
            Amount::from_raw(prior.raw),
            support,
            prior.submitted_at,
        )
        .map_err(|_| error!(ProgramV04Error::SelectFailed))?;
    }
    k.select_candidate(&transition_record.transition_id, kernel_state.clock)
        .map_err(|_| error!(ProgramV04Error::SelectFailed))?;
    let life = match k.life_of(&transition_record.transition_id) {
        Some(Life::Selected) => Life::Selected,
        _ => return Err(error!(ProgramV04Error::SelectFailed)),
    };
    transition_record.stage = TransitionRecord::SELECTED;
    Ok(life)
}

/// Open planning on a selected transition through the real kernel
/// (slice 20). Pure except for the target record's stage and planning
/// deadline, which advance only on success — every refusal leaves the
/// record untouched. Gates, in order: sealed state; stage exactly
/// `SELECTED` (a `Blocked` record refuses here — the kernel's
/// proposer-or-seat re-plan path is later work, not this
/// instruction); parent exactly the sealed head; observation linkage;
/// a proven window; the completeness anchor — the supplied prior set
/// must number exactly `signal_count`; every prior belonging to the
/// target with a stored round matching its own `submitted_at`; no
/// duplicated (agent, round) in the set. Replay then rebuilds the
/// sealed launch kernel under `ProgramEnv`, replays the eligibility
/// chain at the stored `eligible_at`, replays every prior signal at
/// its own `submitted_at` in order, and runs kernel
/// `select_candidate` at the state's clock — the stored `SELECTED`
/// label is re-proven, never trusted — and only then runs kernel
/// `open_planning` with the caller's agent id at the same clock,
/// where the kernel's Selected-path rule admits anyone. Only a
/// `Life::Planning` life advances the stage, and the persisted
/// deadline is the state's clock plus the launch policy's
/// planning window: `apply_launch_genesis` installs exactly
/// `launch_policy()` for this transition's type, so that window is
/// the kernel's own policy value, and a checked add refuses an
/// overflowing deadline rather than wrapping it.
pub fn open_planning_from_records(
    kernel_state: &KernelState,
    transition_record: &mut TransitionRecord,
    observation_record: &ObservationRecord,
    prior_signals: &[SignalRecord],
    caller: [u8; 32],
) -> Result<Life> {
    if !kernel_state.sealed {
        return Err(error!(ProgramV04Error::NotSealed));
    }
    if transition_record.stage != TransitionRecord::SELECTED {
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
    if transition_record.eligible_at == 0
        || transition_record.consensus_deadline <= transition_record.eligible_at
    {
        return Err(error!(ProgramV04Error::PlanningFailed));
    }
    if prior_signals.len() as u64 != transition_record.signal_count {
        return Err(error!(ProgramV04Error::PlanningFailed));
    }
    for prior in prior_signals {
        if prior.transition_id != transition_record.transition_id {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        if signal_round(
            transition_record.eligible_at,
            transition_record.consensus_deadline,
            prior.submitted_at,
        ) != prior.round
        {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
    }
    {
        let mut seen: BTreeSet<([u8; 32], u64)> = BTreeSet::new();
        for prior in prior_signals {
            if !seen.insert((prior.agent.to_bytes(), prior.round)) {
                return Err(error!(ProgramV04Error::PlanningFailed));
            }
        }
    }
    let proposer = resolve_proposer(kernel_state, transition_record.proposer)
        .map_err(|_| error!(ProgramV04Error::PlanningFailed))?;
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
    let eligible_at = transition_record.eligible_at;
    k.ingest_observation(&observer_id, observation, eligible_at)
        .map_err(|_| error!(ProgramV04Error::PlanningFailed))?;
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
        eligible_at,
    )
    .map_err(|_| error!(ProgramV04Error::PlanningFailed))?;
    k.evaluate_transition(&transition_record.transition_id, eligible_at)
        .map_err(|_| error!(ProgramV04Error::PlanningFailed))?;
    k.mark_eligible(&transition_record.transition_id, eligible_at)
        .map_err(|_| error!(ProgramV04Error::PlanningFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Eligible) => {}
        _ => return Err(error!(ProgramV04Error::PlanningFailed)),
    }
    let mut priors: alloc::vec::Vec<SignalRecord> = prior_signals.to_vec();
    priors.sort_by_key(|s| s.submitted_at);
    for prior in &priors {
        let support = if prior.has_support {
            Some(Amount::from_raw(prior.support_raw))
        } else {
            None
        };
        k.submit_round_signal(
            &prior.agent.to_bytes(),
            &transition_record.transition_id,
            Amount::from_raw(prior.raw),
            support,
            prior.submitted_at,
        )
        .map_err(|_| error!(ProgramV04Error::PlanningFailed))?;
    }
    k.select_candidate(&transition_record.transition_id, kernel_state.clock)
        .map_err(|_| error!(ProgramV04Error::PlanningFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Selected) => {}
        _ => return Err(error!(ProgramV04Error::PlanningFailed)),
    }
    k.open_planning(&caller, &transition_record.transition_id, kernel_state.clock)
        .map_err(|_| error!(ProgramV04Error::PlanningFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Planning) => {}
        _ => return Err(error!(ProgramV04Error::PlanningFailed)),
    }
    let deadline = kernel_state
        .clock
        .checked_add(launch_policy().planning_window)
        .ok_or(error!(ProgramV04Error::PlanningFailed))?;
    transition_record.stage = TransitionRecord::PLANNING;
    transition_record.planning_deadline = deadline;
    Ok(Life::Planning)
}

/// One sibling transition offered to a sibling-aware select (slice 17):
/// the sibling's stored transition record, its observation record, and
/// its stored signal records. All three are read, never written: the
/// replay proves the sibling's tally in a rebuilt kernel, and only the
/// target record's stage advances.
#[derive(Clone, Copy)]
pub struct SiblingReplay<'a> {
    pub transition_record: &'a TransitionRecord,
    pub observation_record: &'a ObservationRecord,
    pub prior_signals: &'a [SignalRecord],
}

/// Replay one transition's eligibility chain into a rebuilt kernel
/// (slice 17's phase 1): ingest + register + evaluate +
/// mark_eligible, all at the stored `eligible_at`, so the rebuilt
/// kernel's window IS the stored window. Every failure maps to
/// `SelectFailed` — this helper judges a supplied replay set, not a
/// single transition's gates, so the pre-replay gates of the calling
/// helper carry the distinct errors.
fn replay_transition_chain(
    kernel_state: &KernelState,
    k: &mut LaunchKernelProgramEnv,
    transition_record: &TransitionRecord,
    observation_record: &ObservationRecord,
) -> Result<()> {
    let proposer = resolve_proposer(kernel_state, transition_record.proposer)
        .map_err(|_| error!(ProgramV04Error::SelectFailed))?;
    let observer_id = resolve_agent_id(observation_record.observer);
    if !k.has_role(&observer_id, Role::Observer) {
        return Err(error!(ProgramV04Error::SelectFailed));
    }
    let relevant_groups = match k.agent(&proposer) {
        Some(profile) => profile.groups.clone(),
        None => return Err(error!(ProgramV04Error::SelectFailed)),
    };
    let observation = Observation {
        id: observation_record.observation_id,
        timestamp: observation_record.timestamp,
        expiry: observation_record.expiry,
        confidence: Amount::from_raw(observation_record.confidence_raw),
    };
    let eligible_at = transition_record.eligible_at;
    k.ingest_observation(&observer_id, observation, eligible_at)
        .map_err(|_| error!(ProgramV04Error::SelectFailed))?;
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
        eligible_at,
    )
    .map_err(|_| error!(ProgramV04Error::SelectFailed))?;
    k.evaluate_transition(&transition_record.transition_id, eligible_at)
        .map_err(|_| error!(ProgramV04Error::SelectFailed))?;
    k.mark_eligible(&transition_record.transition_id, eligible_at)
        .map_err(|_| error!(ProgramV04Error::SelectFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Eligible) => Ok(()),
        _ => Err(error!(ProgramV04Error::SelectFailed)),
    }
}

/// Select a candidate over a supplied sibling field through the real
/// kernel (slices 17–18). Pure except for the target record's stage,
/// which advances to `SELECTED` only on success — every refusal
/// leaves every record untouched. Target gates, in order: sealed
/// state; stage exactly `ELIGIBLE`; parent exactly the sealed head;
/// observation linkage; a proven window; the completeness anchor —
/// the supplied target prior set must number exactly `signal_count`;
/// every target prior belonging to the target with a stored round
/// matching its own `submitted_at`; no duplicated (agent, round) in
/// the target set. Sibling gates, per sibling: an id distinct from
/// the target and from every other supplied sibling; stage exactly
/// `ELIGIBLE` or `SELECTED` (slice 18: any other stage refuses, and
/// at most one `SELECTED` sibling is admitted, since a parent can
/// have only one selected transition); parent exactly the target's parent
/// (hence the sealed head); observation linkage to its own
/// observation record; a proven window; its supplied signal set
/// numbering exactly its own `signal_count`; every sibling prior
/// belonging to the sibling with a stored round matching its own
/// `submitted_at`; no duplicated (agent, round) within the sibling's
/// set. Replay then runs as one merged timeline against a freshly
/// rebuilt sealed launch kernel under `ProgramEnv`: eligibility
/// chains and signals interleaved in ascending time order, chains
/// before signals at an equal time (siblings before the target on an
/// equal `eligible_at` — the fail-closed tie rule); then, if a
/// `SELECTED` sibling was supplied, kernel `select_candidate` runs
/// on that sibling at the state's clock and its life must become
/// `Selected` — its stage label is re-proven, never trusted — and
/// only then kernel `select_candidate` runs on the target at the
/// state's clock, where the kernel's
/// `ParentAlreadyHasASelectedTransition` arm can fire. A kernel refusal in any
/// call maps to `SelectFailed`: a still-open target or sibling
/// window, an unmet quorum, confidence below the launch threshold,
/// an outranking sibling, an expired manifest, and a signal at or
/// past a deadline during the prior replay all fail here, through
/// the kernel, not through a program-side guess about them. Only a
/// `Life::Selected` life advances the stage. Slice 19 closes the
/// completeness gap slices 17–18 left open: before any replay the
/// supplied field plus the target must number exactly the
/// `KernelState` sibling census at the target's parent (written by
/// `mark_eligible` on success only), so an omitted eligible
/// competitor refuses here rather than passing unjudged.
#[allow(clippy::too_many_arguments)]
pub fn select_candidate_from_records_with_siblings(
    kernel_state: &KernelState,
    transition_record: &mut TransitionRecord,
    observation_record: &ObservationRecord,
    prior_signals: &[SignalRecord],
    siblings: &[SiblingReplay],
) -> Result<Life> {
    if !kernel_state.sealed {
        return Err(error!(ProgramV04Error::NotSealed));
    }
    if transition_record.stage != TransitionRecord::ELIGIBLE {
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
    if transition_record.eligible_at == 0
        || transition_record.consensus_deadline <= transition_record.eligible_at
    {
        return Err(error!(ProgramV04Error::SelectFailed));
    }
    if prior_signals.len() as u64 != transition_record.signal_count {
        return Err(error!(ProgramV04Error::SelectFailed));
    }
    for prior in prior_signals {
        if prior.transition_id != transition_record.transition_id {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        if signal_round(
            transition_record.eligible_at,
            transition_record.consensus_deadline,
            prior.submitted_at,
        ) != prior.round
        {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
    }
    {
        let mut seen: BTreeSet<([u8; 32], u64)> = BTreeSet::new();
        for prior in prior_signals {
            if !seen.insert((prior.agent.to_bytes(), prior.round)) {
                return Err(error!(ProgramV04Error::SelectFailed));
            }
        }
    }
    {
        let mut seen_ids: BTreeSet<[u8; 32]> = BTreeSet::new();
        for sibling in siblings {
            let srec = sibling.transition_record;
            if srec.transition_id == transition_record.transition_id
                || !seen_ids.insert(srec.transition_id)
            {
                return Err(error!(ProgramV04Error::SelectFailed));
            }
            if srec.stage != TransitionRecord::ELIGIBLE
                && srec.stage != TransitionRecord::SELECTED
            {
                return Err(error!(ProgramV04Error::SelectFailed));
            }
            if srec.parent_index != transition_record.parent_index
                || srec.parent_hash != transition_record.parent_hash
            {
                return Err(error!(ProgramV04Error::ParentMismatch));
            }
            if srec.observation_id != sibling.observation_record.observation_id {
                return Err(error!(ProgramV04Error::RecordMismatch));
            }
            if srec.eligible_at == 0 || srec.consensus_deadline <= srec.eligible_at {
                return Err(error!(ProgramV04Error::SelectFailed));
            }
            if sibling.prior_signals.len() as u64 != srec.signal_count {
                return Err(error!(ProgramV04Error::SelectFailed));
            }
            let mut seen: BTreeSet<([u8; 32], u64)> = BTreeSet::new();
            for prior in sibling.prior_signals {
                if prior.transition_id != srec.transition_id {
                    return Err(error!(ProgramV04Error::RecordMismatch));
                }
                if signal_round(srec.eligible_at, srec.consensus_deadline, prior.submitted_at)
                    != prior.round
                {
                    return Err(error!(ProgramV04Error::RecordMismatch));
                }
                if !seen.insert((prior.agent.to_bytes(), prior.round)) {
                    return Err(error!(ProgramV04Error::SelectFailed));
                }
            }
        }
    }
    // Slice 18: a parent can have only one selected transition, so
    // two supplied SELECTED siblings are an inconsistent field even
    // before any replay — the first one's select would already make
    // the second's impossible, and the kernel would refuse it there;
    // refusing here names the inconsistency at the gate instead.
    if siblings
        .iter()
        .filter(|s| s.transition_record.stage == TransitionRecord::SELECTED)
        .count()
        > 1
    {
        return Err(error!(ProgramV04Error::SelectFailed));
    }
    // Slice 19: the sibling-set completeness anchor. The supplied
    // field plus the target must number exactly the census at the
    // target's parent — a field short of the census omits an
    // eligible competitor the kernel would have judged, and a field
    // over it cannot exist without a duplicated id (refused above),
    // so either direction refuses before any replay. The census
    // parent must equal the target's parent: a count recorded at
    // another parent says nothing about this field.
    if kernel_state.eligible_sibling_parent_index != transition_record.parent_index
        || kernel_state.eligible_sibling_parent_hash != transition_record.parent_hash
        || siblings.len() as u64 + 1 != kernel_state.eligible_sibling_count
    {
        return Err(error!(ProgramV04Error::SelectFailed));
    }
    let mut k = sealed_launch_kernel_env().map_err(|_| error!(ProgramV04Error::KernelSeal))?;
    // One merged timeline: (time, kind rank, tie rank, index) with
    // kind rank 0 = eligibility chain, 1 = signal, so a chain always
    // precedes same-time signals; tie rank 0 = sibling, 1 = target,
    // so siblings precede the target on an equal eligible_at. The
    // index is usize::MAX for the target, the sibling's supply index
    // otherwise; sort_by is stable, so equal keys keep supply order.
    // A two-phase replay would regress the kernel clock here: a
    // transition's signal routinely predates another transition's
    // eligibility, and the kernel's tick refuses backward time.
    enum TimelineEvent<'a> {
        Chain(usize),
        Signal(&'a SignalRecord, [u8; 32]),
    }
    let mut timeline: alloc::vec::Vec<(u64, u8, u8, TimelineEvent)> = alloc::vec::Vec::new();
    for (i, sibling) in siblings.iter().enumerate() {
        timeline.push((
            sibling.transition_record.eligible_at,
            0,
            0,
            TimelineEvent::Chain(i),
        ));
    }
    timeline.push((
        transition_record.eligible_at,
        0,
        1,
        TimelineEvent::Chain(usize::MAX),
    ));
    for prior in prior_signals {
        timeline.push((
            prior.submitted_at,
            1,
            1,
            TimelineEvent::Signal(prior, transition_record.transition_id),
        ));
    }
    for sibling in siblings {
        for prior in sibling.prior_signals {
            timeline.push((
                prior.submitted_at,
                1,
                0,
                TimelineEvent::Signal(prior, sibling.transition_record.transition_id),
            ));
        }
    }
    timeline.sort_by_key(|t| (t.0, t.1, t.2));
    for (_, _, _, event) in &timeline {
        match event {
            TimelineEvent::Chain(idx) => {
                if *idx == usize::MAX {
                    replay_transition_chain(
                        kernel_state,
                        &mut k,
                        transition_record,
                        observation_record,
                    )?;
                } else {
                    let sibling = &siblings[*idx];
                    replay_transition_chain(
                        kernel_state,
                        &mut k,
                        sibling.transition_record,
                        sibling.observation_record,
                    )?;
                }
            }
            TimelineEvent::Signal(prior, tid) => {
                let support = if prior.has_support {
                    Some(Amount::from_raw(prior.support_raw))
                } else {
                    None
                };
                k.submit_round_signal(
                    &prior.agent.to_bytes(),
                    tid,
                    Amount::from_raw(prior.raw),
                    support,
                    prior.submitted_at,
                )
                .map_err(|_| error!(ProgramV04Error::SelectFailed))?;
            }
        }
    }
    // Slice 18: re-prove a supplied SELECTED sibling's own select
    // before the target's. The sibling's chain and signals are
    // already in the rebuilt kernel from the merged timeline above,
    // so this call judges exactly what its stored stage claims: that
    // it won a select. It runs at the state's clock — the same clock
    // the target's select runs at next, `tick` accepting the equal
    // time — and call order is what makes it "already" selected:
    // after this call `selected_for` names the sibling, so the
    // target's select below meets the kernel's
    // `ParentAlreadyHasASelectedTransition` arm rather than a
    // sibling-loop comparison against a merely eligible record. A
    // sibling that cannot win here (unqualified tally, an outranking
    // target, a window not yet closed) refuses the whole helper,
    // and the target's stage is left untouched.
    for sibling in siblings {
        if sibling.transition_record.stage != TransitionRecord::SELECTED {
            continue;
        }
        k.select_candidate(
            &sibling.transition_record.transition_id,
            kernel_state.clock,
        )
        .map_err(|_| error!(ProgramV04Error::SelectFailed))?;
        match k.life_of(&sibling.transition_record.transition_id) {
            Some(Life::Selected) => {}
            _ => return Err(error!(ProgramV04Error::SelectFailed)),
        }
    }
    k.select_candidate(&transition_record.transition_id, kernel_state.clock)
        .map_err(|_| error!(ProgramV04Error::SelectFailed))?;
    let life = match k.life_of(&transition_record.transition_id) {
        Some(Life::Selected) => Life::Selected,
        _ => return Err(error!(ProgramV04Error::SelectFailed)),
    };
    transition_record.stage = TransitionRecord::SELECTED;
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
    Ok(KernelState {
        sealed,
        head_index: index,
        head_hash: [0; 32],
        clock: now,
        eligible_sibling_parent_index: index,
        eligible_sibling_parent_hash: [0; 32],
        eligible_sibling_count: 0,
        bump: 0,
    })
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

#[event]
pub struct SignalSubmitted {
    pub transition_id: [u8; 32],
    pub agent: Pubkey,
    pub round: u64,
    pub total_weight_raw: u64,
    pub confidence_raw: u64,
}

#[event]
pub struct TransitionSelected {
    pub transition_id: [u8; 32],
    pub stage: u8,
}

#[event]
pub struct TransitionPlanningOpened {
    pub transition_id: [u8; 32],
    pub stage: u8,
    pub planning_deadline: u64,
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
    #[msg("program-v04 signal: kernel signal refused, window unproven, or round mismatch")]
    SignalFailed,
    #[msg("program-v04 select: kernel selection refused, window unproven, or supplied signal set incomplete")]
    SelectFailed,
    #[msg("program-v04 planning: kernel planning refused, selection unproven, or supplied signal set incomplete")]
    PlanningFailed,
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
        // clock 8 + census parent index 8 + census parent hash 32 +
        // census count 8 + bump 1.
        assert_eq!(KernelState::SPACE, 8 + 1 + 8 + 32 + 8 + 8 + 32 + 8 + 1);
    }

    #[test]
    fn kernel_state_borsh_round_trips() {
        use anchor_lang::{AccountDeserialize, AccountSerialize};
        let state = KernelState {
            sealed: false,
            head_index: 7,
            head_hash: [0xAB; 32],
            clock: 100,
            eligible_sibling_parent_index: 7,
            eligible_sibling_parent_hash: [0xAB; 32],
            eligible_sibling_count: 3,
            bump: 254,
        };
        let mut bytes = Vec::new();
        state.try_serialize(&mut bytes).expect("serialize must succeed");
        // Discriminator (8) + fields (98) = SPACE.
        assert_eq!(bytes.len(), KernelState::SPACE);
        let back = <KernelState as AccountDeserialize>::try_deserialize(&mut &bytes[..])
            .expect("deserialize must succeed");
        assert!(!back.sealed);
        assert_eq!(back.head_index, 7);
        assert_eq!(back.head_hash, [0xAB; 32]);
        assert_eq!(back.clock, 100);
        assert_eq!(back.eligible_sibling_parent_index, 7);
        assert_eq!(back.eligible_sibling_parent_hash, [0xAB; 32]);
        assert_eq!(back.eligible_sibling_count, 3);
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
        // proposer 32 + stage 1 + eligible_at 8 + consensus_deadline 8 +
        // planning_deadline 8 + signal_count 8 + bump 1.
        assert_eq!(
            TransitionRecord::SPACE,
            8 + 32 + 32 + 8 + 32 + 32 + 32 + 8 + 32 + 1 + 8 + 8 + 8 + 8 + 1
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
            planning_deadline: 400,
            signal_count: 2,
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
        assert_eq!(back.planning_deadline, 400);
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
        let (mut state, mut record, obs) = eligible_pair();
        let life = mark_eligible_from_records(&mut state, &mut record, &obs)
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
        let (mut state, mut record, obs) = evaluated_pair();
        let err = mark_eligible_from_records(&mut state, &mut record, &obs)
            .expect_err("a merely kernel-registered record has no evaluation to make eligible");
        assert!(format!("{:?}", err).contains("WrongStage"));
        assert_eq!(record.stage, TransitionRecord::KERNEL_REGISTERED);
    }

    #[test]
    fn mark_eligible_is_once_only() {
        let (mut state, mut record, obs) = eligible_pair();
        mark_eligible_from_records(&mut state, &mut record, &obs)
            .expect("first mark_eligible must succeed");
        let err = mark_eligible_from_records(&mut state, &mut record, &obs)
            .expect_err("second mark_eligible must refuse");
        assert!(format!("{:?}", err).contains("WrongStage"));
        assert_eq!(record.stage, TransitionRecord::ELIGIBLE);
    }

    #[test]
    fn mark_eligible_refuses_unsealed() {
        let (mut state, mut record, obs) = eligible_pair();
        state.sealed = false;
        let err = mark_eligible_from_records(&mut state, &mut record, &obs)
            .expect_err("unsealed mark_eligible must refuse");
        assert!(format!("{:?}", err).contains("NotSealed"));
        assert_eq!(record.stage, TransitionRecord::EVALUATED);
    }

    #[test]
    fn mark_eligible_refuses_parent_mismatch() {
        let (mut state, mut record, obs) = eligible_pair();
        record.parent_index = 1;
        let err = mark_eligible_from_records(&mut state, &mut record, &obs)
            .expect_err("parent off the sealed head must refuse");
        assert!(format!("{:?}", err).contains("ParentMismatch"));
        assert_eq!(record.stage, TransitionRecord::EVALUATED);
    }

    #[test]
    fn mark_eligible_refuses_observation_mismatch() {
        let (mut state, mut record, _) = eligible_pair();
        let other = build_observation_record(
            &state, [0xC1; 32], 100, 9100, 900_000, observer_pubkey(), 7,
        )
        .expect("second observation must build");
        let err = mark_eligible_from_records(&mut state, &mut record, &other)
            .expect_err("observation id mismatch must refuse");
        assert!(format!("{:?}", err).contains("RecordMismatch"));
        assert_eq!(record.stage, TransitionRecord::EVALUATED);
    }

    #[test]
    fn mark_eligible_refuses_unknown_proposer() {
        let (mut state, mut record, obs) = eligible_pair();
        record.proposer = Pubkey::new_from_array([0x99; 32]);
        let err = mark_eligible_from_records(&mut state, &mut record, &obs)
            .expect_err("unknown proposer must refuse");
        assert!(format!("{:?}", err).contains("UnknownProposer"));
        assert_eq!(record.stage, TransitionRecord::EVALUATED);
    }

    #[test]
    fn mark_eligible_refuses_non_observer_observation() {
        // Built as a struct literal: the slice-9 builder would refuse
        // this observer, so mark_eligible must catch a record that
        // bypassed it.
        let (mut state, mut record, _) = eligible_pair();
        let obs = ObservationRecord {
            observation_id: [0xC0; 32],
            timestamp: 100,
            expiry: 9100,
            confidence_raw: 900_000,
            observer: Pubkey::new_from_array(ag(1)),
            bump: 7,
        };
        let err = mark_eligible_from_records(&mut state, &mut record, &obs)
            .expect_err("non-observer observation must refuse");
        assert!(format!("{:?}", err).contains("NotObserver"));
        assert_eq!(record.stage, TransitionRecord::EVALUATED);
    }

    #[test]
    fn mark_eligible_at_advanced_clock() {
        let (state, mut record, obs) = eligible_pair();
        let mut advanced = state;
        advance_clock_on_state(&mut advanced, 200).expect("advance must succeed");
        let life = mark_eligible_from_records(&mut advanced, &mut record, &obs)
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
        mark_eligible_from_records(&mut advanced, &mut record, &obs)
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
        let err = mark_eligible_from_records(&mut advanced, &mut record, &obs)
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
        let err = mark_eligible_from_records(&mut advanced, &mut record, &obs)
            .expect_err("lapsed observation must refuse eligibility in the kernel");
        assert!(format!("{:?}", err).contains("EligibleFailed"));
        assert_eq!(record.stage, TransitionRecord::EVALUATED);
    }

    fn signal_pair() -> (KernelState, TransitionRecord, ObservationRecord) {
        let (mut state, mut record, obs) = eligible_pair();
        mark_eligible_from_records(&mut state, &mut record, &obs)
            .expect("pair must mark eligible before signalling");
        assert_eq!(record.stage, TransitionRecord::ELIGIBLE);
        assert_eq!(record.eligible_at, 100);
        assert_eq!(record.consensus_deadline, 200);
        (state, record, obs)
    }

    #[test]
    fn signal_round_matches_kernel_round_slices() {
        // The window is 100/200 with max_rounds 3, so rounds are
        // 33-tick slices: 100–132 round 0, 133–165 round 1, 166–199
        // round 2, and the last round caps rather than overflows.
        assert_eq!(signal_round(100, 200, 100), 0);
        assert_eq!(signal_round(100, 200, 132), 0);
        assert_eq!(signal_round(100, 200, 133), 1);
        assert_eq!(signal_round(100, 200, 165), 1);
        assert_eq!(signal_round(100, 200, 166), 2);
        assert_eq!(signal_round(100, 200, 199), 2);
    }

    #[test]
    fn signal_record_space_accounts_for_every_field() {
        // Discriminator 8 + transition 32 + agent 32 + round 8 +
        // raw 8 + support 8 + has_support 1 + submitted_at 8 + bump 1.
        assert_eq!(
            SignalRecord::SPACE,
            8 + 32 + 32 + 8 + 8 + 8 + 1 + 8 + 1
        );
    }

    #[test]
    fn signal_record_borsh_round_trips() {
        use anchor_lang::{AccountDeserialize, AccountSerialize};
        let record = SignalRecord {
            transition_id: [0xD1; 32],
            agent: Pubkey::new_from_array(ag(2)),
            round: 1,
            raw: SCALE,
            support_raw: SCALE,
            has_support: true,
            submitted_at: 133,
            bump: 254,
        };
        let mut bytes = Vec::new();
        record.try_serialize(&mut bytes).expect("serialize must succeed");
        assert_eq!(bytes.len(), SignalRecord::SPACE);
        let back = <SignalRecord as AccountDeserialize>::try_deserialize(&mut &bytes[..])
            .expect("deserialize must succeed");
        assert_eq!(back.transition_id, [0xD1; 32]);
        assert_eq!(back.agent, Pubkey::new_from_array(ag(2)));
        assert_eq!(back.round, 1);
        assert_eq!(back.raw, SCALE);
        assert_eq!(back.support_raw, SCALE);
        assert!(back.has_support);
        assert_eq!(back.submitted_at, 133);
        assert_eq!(back.bump, 254);
    }

    #[test]
    fn submit_signal_accepts_first_signal_and_reports_kernel_tally() {
        // Agent 2 is not the proposer: any seeded agent may signal.
        // Raw 1.0 at base influence 2.0 (reputation pin-OFF) weighs
        // 2.0, which clears the 1.0 quorum alone; support 1.0 gives
        // confidence 1.0. The tally comes from the kernel, not from a
        // program-side sum.
        let (state, mut record, obs) = signal_pair();
        let (signal, total, confidence) = submit_signal_from_records(
            &state, &mut record, &obs, &[], Pubkey::new_from_array(ag(2)),
            0, SCALE, SCALE, true, 7,
        )
        .expect("first signal at the genesis clock must submit");
        assert_eq!(signal.transition_id, [0xD1; 32]);
        assert_eq!(signal.agent, Pubkey::new_from_array(ag(2)));
        assert_eq!(signal.round, 0);
        assert_eq!(signal.raw, SCALE);
        assert_eq!(signal.support_raw, SCALE);
        assert!(signal.has_support);
        assert_eq!(signal.submitted_at, 100);
        assert_eq!(signal.bump, 7);
        assert_eq!(total.raw(), 2 * SCALE);
        assert_eq!(confidence.raw(), SCALE);
        // Signals do not advance the stage or touch the window.
        assert_eq!(record.stage, TransitionRecord::ELIGIBLE);
        assert_eq!(record.eligible_at, 100);
        assert_eq!(record.consensus_deadline, 200);
        // Slice 16: the one mutation signals do make — the count anchor.
        assert_eq!(record.signal_count, 1);
    }

    #[test]
    fn submit_signal_accepts_no_position_signal_at_zero_weight() {
        // support == None is recorded and classified but adds no
        // weight: the tally stays zero, and a caller-supplied
        // support_raw with has_support false normalizes to 0.
        let (state, mut record, obs) = signal_pair();
        let (signal, total, confidence) = submit_signal_from_records(
            &state, &mut record, &obs, &[], Pubkey::new_from_array(ag(3)),
            0, SCALE, 5, false, 7,
        )
        .expect("no-position signal must submit");
        assert!(!signal.has_support);
        assert_eq!(signal.support_raw, 0);
        assert_eq!(total.raw(), 0);
        assert_eq!(confidence.raw(), 0);
    }

    #[test]
    fn submit_signal_refuses_wrong_stage() {
        let (state, mut record, obs) = evaluated_pair();
        let err = submit_signal_from_records(
            &state, &mut record, &obs, &[], Pubkey::new_from_array(ag(2)),
            0, SCALE, SCALE, true, 7,
        )
        .expect_err("a merely-evaluated record has no window to signal in");
        assert!(format!("{:?}", err).contains("WrongStage"));
    }

    #[test]
    fn submit_signal_refuses_observation_mismatch() {
        let (state, mut record, _) = signal_pair();
        let other = build_observation_record(
            &state, [0xC1; 32], 100, 9100, 900_000, observer_pubkey(), 7,
        )
        .expect("second observation must build");
        let err = submit_signal_from_records(
            &state, &mut record, &other, &[], Pubkey::new_from_array(ag(2)),
            0, SCALE, SCALE, true, 7,
        )
        .expect_err("observation id mismatch must refuse");
        assert!(format!("{:?}", err).contains("RecordMismatch"));
    }

    #[test]
    fn submit_signal_refuses_unknown_agent_and_over_range_raw() {
        let (state, mut record, obs) = signal_pair();
        let err = submit_signal_from_records(
            &state, &mut record, &obs, &[], Pubkey::new_from_array([0x77; 32]),
            0, SCALE, SCALE, true, 7,
        )
        .expect_err("unknown pubkey is no agent and must refuse in the kernel");
        assert!(format!("{:?}", err).contains("SignalFailed"));
        let err = submit_signal_from_records(
            &state, &mut record, &obs, &[], Pubkey::new_from_array(ag(2)),
            0, SCALE + 1, SCALE, true, 7,
        )
        .expect_err("raw above SCALE must refuse in the kernel");
        assert!(format!("{:?}", err).contains("SignalFailed"));
    }

    #[test]
    fn submit_signal_refuses_wrong_round_arg() {
        let (state, mut record, obs) = signal_pair();
        let err = submit_signal_from_records(
            &state, &mut record, &obs, &[], Pubkey::new_from_array(ag(2)),
            1, SCALE, SCALE, true, 7,
        )
        .expect_err("round 1 at clock 100 contradicts the derived round 0");
        assert!(format!("{:?}", err).contains("SignalFailed"));
    }

    #[test]
    fn submit_signal_refuses_foreign_prior() {
        let (state, mut record, obs) = signal_pair();
        let foreign = SignalRecord {
            transition_id: [0xD9; 32],
            agent: Pubkey::new_from_array(ag(2)),
            round: 0,
            raw: SCALE,
            support_raw: SCALE,
            has_support: true,
            submitted_at: 100,
            bump: 7,
        };
        let err = submit_signal_from_records(
            &state, &mut record, &obs, &[foreign], Pubkey::new_from_array(ag(3)),
            0, SCALE, SCALE, true, 7,
        )
        .expect_err("a prior from another transition must refuse");
        assert!(format!("{:?}", err).contains("RecordMismatch"));
    }

    fn first_signal_at_genesis(
        state: &KernelState,
        record: &mut TransitionRecord,
        obs: &ObservationRecord,
    ) -> SignalRecord {
        let (signal, _, _) = submit_signal_from_records(
            state, record, obs, &[], Pubkey::new_from_array(ag(2)),
            0, SCALE, SCALE, true, 7,
        )
        .expect("first signal must submit");
        signal
    }

    #[test]
    fn submit_signal_refuses_same_round_duplicate() {
        let (state, mut record, obs) = signal_pair();
        let first = first_signal_at_genesis(&state, &mut record, &obs);
        let err = submit_signal_from_records(
            &state, &mut record, &obs, &[first], Pubkey::new_from_array(ag(2)),
            0, SCALE, SCALE, true, 7,
        )
        .expect_err("one signal per agent per round: the kernel must refuse");
        assert!(format!("{:?}", err).contains("SignalFailed"));
    }

    #[test]
    fn submit_signal_accepts_later_round_revision_same_support() {
        // Round 1 starts at clock 133. A revision repeating the same
        // support sits exactly on the last round's mean, so the
        // monotone rule admits it; the tally replaces, not adds, the
        // agent's earlier weight.
        let (state, mut record, obs) = signal_pair();
        let first = first_signal_at_genesis(&state, &mut record, &obs);
        let mut advanced = state;
        advance_clock_on_state(&mut advanced, 133).expect("advance must succeed");
        let (signal, total, confidence) = submit_signal_from_records(
            &advanced, &mut record, &obs, &[first], Pubkey::new_from_array(ag(2)),
            1, SCALE, SCALE, true, 7,
        )
        .expect("same-support revision in round 1 must submit");
        assert_eq!(signal.round, 1);
        assert_eq!(signal.submitted_at, 133);
        assert_eq!(total.raw(), 2 * SCALE);
        assert_eq!(confidence.raw(), SCALE);
    }

    #[test]
    fn submit_signal_refuses_revision_away_from_mean() {
        // Single agent, so the last round's mean IS the agent's own
        // support: moving to 0 moves strictly away from it, and the
        // kernel's monotone rule must refuse.
        let (state, mut record, obs) = signal_pair();
        let first = first_signal_at_genesis(&state, &mut record, &obs);
        let mut advanced = state;
        advance_clock_on_state(&mut advanced, 133).expect("advance must succeed");
        let err = submit_signal_from_records(
            &advanced, &mut record, &obs, &[first], Pubkey::new_from_array(ag(2)),
            1, SCALE, 0, true, 7,
        )
        .expect_err("revision away from the mean must refuse in the kernel");
        assert!(format!("{:?}", err).contains("SignalFailed"));
    }

    #[test]
    fn submit_signal_two_agents_accumulate_tally() {
        // Agent 2 at support 1.0, then agent 3 at support 0: total
        // weight 4.0, weighted 2.0, confidence exactly 0.5 — the launch
        // policy's threshold. This proves the helper's prior replay
        // accumulates across agents, not just the submitting one.
        let (state, mut record, obs) = signal_pair();
        let first = first_signal_at_genesis(&state, &mut record, &obs);
        let (second, total, confidence) = submit_signal_from_records(
            &state, &mut record, &obs, &[first], Pubkey::new_from_array(ag(3)),
            0, SCALE, 0, true, 7,
        )
        .expect("second agent's round-0 signal must submit");
        assert_eq!(second.agent, Pubkey::new_from_array(ag(3)));
        assert_eq!(total.raw(), 4 * SCALE);
        assert_eq!(confidence.raw(), SCALE / 2);
    }

    #[test]
    fn submit_signal_refuses_at_and_past_deadline() {
        // The window is 100/200: at clock 200 the round derives to the
        // capped last round (2), but the kernel's consensus window is
        // closed at the deadline itself, so the signal must refuse.
        let (state, mut record, obs) = signal_pair();
        let mut advanced = state;
        advance_clock_on_state(&mut advanced, 200).expect("advance must succeed");
        let err = submit_signal_from_records(
            &advanced, &mut record, &obs, &[], Pubkey::new_from_array(ag(2)),
            2, SCALE, SCALE, true, 7,
        )
        .expect_err("signal at the deadline must refuse in the kernel");
        assert!(format!("{:?}", err).contains("SignalFailed"));
    }

    fn selected_pair() -> (KernelState, TransitionRecord, ObservationRecord, SignalRecord) {
        let (state, mut record, obs) = signal_pair();
        let (signal, _, _) = submit_signal_from_records(
            &state, &mut record, &obs, &[], Pubkey::new_from_array(ag(2)),
            0, SCALE, SCALE, true, 7,
        )
        .expect("first signal must submit");
        assert_eq!(record.signal_count, 1);
        (state, record, obs, signal)
    }

    fn two_signals() -> (KernelState, TransitionRecord, ObservationRecord, SignalRecord, SignalRecord)
    {
        let (state, mut record, obs, first) = selected_pair();
        let (second, _, _) = submit_signal_from_records(
            &state, &mut record, &obs, &[first], Pubkey::new_from_array(ag(3)),
            0, SCALE, 0, true, 7,
        )
        .expect("second agent signal must submit");
        assert_eq!(record.signal_count, 2);
        (state, record, obs, first, second)
    }

    #[test]
    fn select_advances_eligible_to_selected() {
        // One qualifying signal (weight 2.0, confidence 1.0) at clock 100;
        // at the deadline 200 the window is closed and the tally
        // qualifies, so the kernel selects and the stage advances to 4.
        let (mut state, mut record, obs, first) = selected_pair();
        advance_clock_on_state(&mut state, 200).expect("advance to the deadline must succeed");
        let life = select_candidate_from_records(&state, &mut record, &obs, &[first])
            .expect("closed window with a qualifying tally must select");
        assert_eq!(life, Life::Selected);
        assert_eq!(record.stage, TransitionRecord::SELECTED);
        assert_eq!(record.stage, 4);
        assert_eq!(record.signal_count, 1);
    }

    #[test]
    fn select_refuses_before_deadline() {
        // At clock 199 the window is still open: the kernel's
        // ConsensusStillOpen must refuse, and the stage stays ELIGIBLE.
        let (mut state, mut record, obs, first) = selected_pair();
        advance_clock_on_state(&mut state, 199).expect("advance must succeed");
        let err = select_candidate_from_records(&state, &mut record, &obs, &[first])
            .expect_err("an open window must refuse selection");
        assert!(format!("{:?}", err).contains("SelectFailed"));
        assert_eq!(record.stage, TransitionRecord::ELIGIBLE);
    }

    #[test]
    fn select_refuses_wrong_stage() {
        let (state, mut record, obs) = evaluated_pair();
        let err = select_candidate_from_records(&state, &mut record, &obs, &[])
            .expect_err("a merely-evaluated record has no window to select in");
        assert!(format!("{:?}", err).contains("WrongStage"));
    }

    #[test]
    fn select_refuses_incomplete_prior_set() {
        // Two records exist (count 2) but only the first is supplied:
        // the completeness anchor refuses before any replay, so an
        // omitted-record set can never prove the tally.
        let (mut state, mut record, obs, first, _) = two_signals();
        advance_clock_on_state(&mut state, 200).expect("advance must succeed");
        let err = select_candidate_from_records(&state, &mut record, &obs, &[first])
            .expect_err("a set short of signal_count must refuse");
        assert!(format!("{:?}", err).contains("SelectFailed"));
        assert_eq!(record.stage, TransitionRecord::ELIGIBLE);
    }

    #[test]
    fn select_refuses_duplicate_prior() {
        // Supplied 2 equals count 2, but the set is one record twice:
        // the duplicate (agent, round) is refused explicitly, so
        // reaching the count by duplication cannot pass either.
        let (mut state, mut record, obs, first, _) = two_signals();
        advance_clock_on_state(&mut state, 200).expect("advance must succeed");
        let err = select_candidate_from_records(&state, &mut record, &obs, &[first, first])
            .expect_err("a duplicated prior must refuse");
        assert!(format!("{:?}", err).contains("SelectFailed"));
        assert_eq!(record.stage, TransitionRecord::ELIGIBLE);
    }

    #[test]
    fn select_refuses_unqualified_tally() {
        // A single support-0 signal: total weight 2.0 clears the 1.0
        // quorum, but confidence 0 is below the 0.5 threshold, so the
        // kernel refuses and the stage stays ELIGIBLE.
        let (state, mut record, obs) = signal_pair();
        let (signal, _, _) = submit_signal_from_records(
            &state, &mut record, &obs, &[], Pubkey::new_from_array(ag(2)),
            0, SCALE, 0, true, 7,
        )
        .expect("support-0 signal must submit");
        let mut advanced = state;
        advance_clock_on_state(&mut advanced, 200).expect("advance must succeed");
        let err = select_candidate_from_records(&advanced, &mut record, &obs, &[signal])
            .expect_err("confidence below threshold must refuse selection");
        assert!(format!("{:?}", err).contains("SelectFailed"));
        assert_eq!(record.stage, TransitionRecord::ELIGIBLE);
    }

    #[test]
    fn select_refuses_foreign_prior() {
        let (mut state, mut record, obs, _) = selected_pair();
        advance_clock_on_state(&mut state, 200).expect("advance must succeed");
        let foreign = SignalRecord {
            transition_id: [0xD9; 32],
            agent: Pubkey::new_from_array(ag(2)),
            round: 0,
            raw: SCALE,
            support_raw: SCALE,
            has_support: true,
            submitted_at: 100,
            bump: 7,
        };
        let err = select_candidate_from_records(&state, &mut record, &obs, &[foreign])
            .expect_err("a prior from another transition must refuse");
        assert!(format!("{:?}", err).contains("RecordMismatch"));
        assert_eq!(record.stage, TransitionRecord::ELIGIBLE);
    }

    #[test]
    fn select_is_once_only() {
        let (mut state, mut record, obs, first) = selected_pair();
        advance_clock_on_state(&mut state, 200).expect("advance must succeed");
        select_candidate_from_records(&state, &mut record, &obs, &[first])
            .expect("first select must succeed");
        let err = select_candidate_from_records(&state, &mut record, &obs, &[first])
            .expect_err("a second select must refuse on stage");
        assert!(format!("{:?}", err).contains("WrongStage"));
    }

    #[test]
    fn signal_count_increments_per_record_and_refusal_leaves_it() {
        // Slice 16's amendment to submit, proven directly: the count
        // moves by exactly one per created record, and a refused
        // signal leaves it untouched.
        let (state, mut record, obs) = signal_pair();
        let first = first_signal_at_genesis(&state, &mut record, &obs);
        assert_eq!(record.signal_count, 1);
        let err = submit_signal_from_records(
            &state, &mut record, &obs, &[first], Pubkey::new_from_array(ag(2)),
            0, SCALE, SCALE, true, 7,
        )
        .expect_err("same-round duplicate must refuse");
        assert!(format!("{:?}", err).contains("SignalFailed"));
        assert_eq!(record.signal_count, 1);
        let mut advanced = state;
        advance_clock_on_state(&mut advanced, 133).expect("advance must succeed");
        let (_, _, _) = submit_signal_from_records(
            &advanced, &mut record, &obs, &[first], Pubkey::new_from_array(ag(2)),
            1, SCALE, SCALE, true, 7,
        )
        .expect("round-1 revision must submit");
        assert_eq!(record.signal_count, 2);
        assert_eq!(record.stage, TransitionRecord::ELIGIBLE);
    }

    // Slice 17 fixtures: build an ELIGIBLE transition pair at a chosen
    // mark-eligible clock (100 = the sealed clock, 150 = one advance
    // later), always proposer agent 1, observer OBSERVER, window
    // mark_clock / mark_clock + 100. The select-time state is built
    // separately — records carry their own windows, so one select
    // state serves pairs marked at different clocks.
    fn eligible_sibling_at(
        mark_clock: u64,
        id: [u8; 32],
        obs_id: [u8; 32],
        declared: [u8; 32],
        expires: u64,
    ) -> (TransitionRecord, ObservationRecord) {
        let mut state = sealed_state();
        if mark_clock > state.clock {
            advance_clock_on_state(&mut state, mark_clock).expect("advance must succeed");
        }
        let mut record = build_transition_record(
            &state, id, TT, 0, [0; 32], declared, obs_id, expires,
            Pubkey::new_from_array(ag(1)), 7,
        )
        .expect("sibling register must build");
        let obs = build_observation_record(
            &state, obs_id, 100, 9100, 900_000, observer_pubkey(), 7,
        )
        .expect("sibling observation must build");
        replay_transition_from_records(&state, &mut record, &obs)
            .expect("sibling replay must succeed");
        evaluate_transition_from_records(&state, &mut record, &obs)
            .expect("sibling evaluate must succeed");
        mark_eligible_from_records(&mut state, &mut record, &obs)
            .expect("sibling mark eligible must succeed");
        assert_eq!(record.stage, TransitionRecord::ELIGIBLE);
        assert_eq!(record.eligible_at, mark_clock);
        assert_eq!(record.consensus_deadline, mark_clock + 100);
        (record, obs)
    }

    fn sibling_signal_at(
        record: &mut TransitionRecord,
        obs: &ObservationRecord,
        priors: &[SignalRecord],
        agent_n: u8,
        support_raw: u64,
        at_clock: u64,
    ) -> SignalRecord {
        let mut state = sealed_state();
        if at_clock > state.clock {
            advance_clock_on_state(&mut state, at_clock).expect("advance must succeed");
        }
        let (sig, _, _) = submit_signal_from_records(
            &state, record, obs, priors, Pubkey::new_from_array(ag(agent_n)),
            0, SCALE, support_raw, true, 7,
        )
        .expect("sibling signal must submit");
        sig
    }

    fn select_state_at(clock: u64) -> KernelState {
        let mut state = sealed_state();
        if clock > state.clock {
            advance_clock_on_state(&mut state, clock).expect("advance must succeed");
        }
        state
    }

    #[test]
    fn select_with_siblings_target_wins_over_weaker_sibling() {
        // Target confidence 1.0, sibling confidence 0, both windows
        // 100/200: the sibling qualifies (weight clears quorum) but
        // does not outrank, so the target selects at clock 200.
        let mut state = select_state_at(200);
        state.eligible_sibling_count = 2; // slice 19: target + 1 supplied sibling(s)
        let (_, mut target, target_obs, first) = selected_pair();
        let (mut sib, sib_obs) = eligible_sibling_at(100, [0xD2; 32], [0xC1; 32], [0xE2; 32], 10_100);
        let sib_sig = sibling_signal_at(&mut sib, &sib_obs, &[], 2, 0, 100);
        let sibling = SiblingReplay {
            transition_record: &sib,
            observation_record: &sib_obs,
            prior_signals: &[sib_sig],
        };
        let life = select_candidate_from_records_with_siblings(
            &state, &mut target, &target_obs, &[first], &[sibling],
        )
        .expect("target outranking its sibling must select");
        assert_eq!(life, Life::Selected);
        assert_eq!(target.stage, TransitionRecord::SELECTED);
    }

    #[test]
    fn select_with_siblings_refuses_outranked() {
        // Target confidence exactly 0.5 (two signals: support 1.0 and
        // support 0), sibling confidence 1.0, both windows closed at
        // 200: the kernel's OutrankedBySibling arm must refuse, and
        // the target's stage stays ELIGIBLE. (A 0-confidence target
        // would fail its own threshold first, so it cannot prove
        // this arm.)
        let mut state = select_state_at(200);
        state.eligible_sibling_count = 2; // slice 19: target + 1 supplied sibling(s)
        let (_, mut target, target_obs, first, second) = two_signals();
        let (mut sib, sib_obs) = eligible_sibling_at(100, [0xD2; 32], [0xC1; 32], [0xE2; 32], 10_100);
        let sib_sig = sibling_signal_at(&mut sib, &sib_obs, &[], 2, SCALE, 100);
        let sibling = SiblingReplay {
            transition_record: &sib,
            observation_record: &sib_obs,
            prior_signals: &[sib_sig],
        };
        let err = select_candidate_from_records_with_siblings(
            &state, &mut target, &target_obs, &[first, second], &[sibling],
        )
        .expect_err("a higher-confidence sibling must refuse selection");
        assert!(format!("{:?}", err).contains("SelectFailed"));
        assert_eq!(target.stage, TransitionRecord::ELIGIBLE);
    }

    #[test]
    fn select_with_siblings_refuses_open_sibling() {
        // Target window 100/200 closed at clock 200, sibling window
        // 150/250 still open: the kernel's SiblingsStillOpen arm must
        // refuse even though the sibling would otherwise qualify.
        let mut state = select_state_at(200);
        state.eligible_sibling_count = 2; // slice 19: target + 1 supplied sibling(s)
        let (_, mut target, target_obs, first) = selected_pair();
        let (mut sib, sib_obs) = eligible_sibling_at(150, [0xD2; 32], [0xC1; 32], [0xE2; 32], 10_100);
        let sib_sig = sibling_signal_at(&mut sib, &sib_obs, &[], 2, SCALE, 150);
        let sibling = SiblingReplay {
            transition_record: &sib,
            observation_record: &sib_obs,
            prior_signals: &[sib_sig],
        };
        let err = select_candidate_from_records_with_siblings(
            &state, &mut target, &target_obs, &[first], &[sibling],
        )
        .expect_err("an open sibling window must refuse selection");
        assert!(format!("{:?}", err).contains("SelectFailed"));
        assert_eq!(target.stage, TransitionRecord::ELIGIBLE);
    }

    #[test]
    fn select_with_siblings_tie_earlier_sibling_wins() {
        // Equal confidence 1.0, sibling eligible at 100, target at
        // 150, both windows closed at 250: the earlier sibling wins
        // the kernel's tie-break, so the target refuses.
        let mut state = select_state_at(250);
        state.eligible_sibling_count = 2; // slice 19: target + 1 supplied sibling(s)
        let (mut sib, sib_obs) = eligible_sibling_at(100, [0xD2; 32], [0xC1; 32], [0xE2; 32], 10_100);
        let sib_sig = sibling_signal_at(&mut sib, &sib_obs, &[], 2, SCALE, 100);
        let (mut target, target_obs) = eligible_sibling_at(150, [0xD1; 32], [0xC0; 32], [0xE1; 32], 10_100);
        let target_sig = sibling_signal_at(&mut target, &target_obs, &[], 2, SCALE, 150);
        let sibling = SiblingReplay {
            transition_record: &sib,
            observation_record: &sib_obs,
            prior_signals: &[sib_sig],
        };
        let err = select_candidate_from_records_with_siblings(
            &state, &mut target, &target_obs, &[target_sig], &[sibling],
        )
        .expect_err("an earlier equal-confidence sibling must refuse selection");
        assert!(format!("{:?}", err).contains("SelectFailed"));
        assert_eq!(target.stage, TransitionRecord::ELIGIBLE);
    }

    #[test]
    fn select_with_siblings_tie_earlier_target_wins() {
        // Mirror of the previous test: target eligible at 100,
        // sibling at 150, equal confidence, both windows closed at
        // 250 — the target wins its own tie-break and selects.
        let mut state = select_state_at(250);
        state.eligible_sibling_count = 2; // slice 19: target + 1 supplied sibling(s)
        let (_, mut target, target_obs, first) = selected_pair();
        let (mut sib, sib_obs) = eligible_sibling_at(150, [0xD2; 32], [0xC1; 32], [0xE2; 32], 10_100);
        let sib_sig = sibling_signal_at(&mut sib, &sib_obs, &[], 2, SCALE, 150);
        let sibling = SiblingReplay {
            transition_record: &sib,
            observation_record: &sib_obs,
            prior_signals: &[sib_sig],
        };
        let life = select_candidate_from_records_with_siblings(
            &state, &mut target, &target_obs, &[first], &[sibling],
        )
        .expect("the earlier target must win its tie");
        assert_eq!(life, Life::Selected);
        assert_eq!(target.stage, TransitionRecord::SELECTED);
    }

    #[test]
    fn select_with_siblings_tie_equal_eligibility_refuses() {
        // Equal confidence 1.0 AND equal eligibility (both 100/200):
        // the slice-17 tie rule replays siblings before the target,
        // so the sibling wins and the target refuses — the fail-closed
        // direction for an order the clock cannot distinguish.
        let mut state = select_state_at(200);
        state.eligible_sibling_count = 2; // slice 19: target + 1 supplied sibling(s)
        let (_, mut target, target_obs, first) = selected_pair();
        let (mut sib, sib_obs) = eligible_sibling_at(100, [0xD2; 32], [0xC1; 32], [0xE2; 32], 10_100);
        let sib_sig = sibling_signal_at(&mut sib, &sib_obs, &[], 2, SCALE, 100);
        let sibling = SiblingReplay {
            transition_record: &sib,
            observation_record: &sib_obs,
            prior_signals: &[sib_sig],
        };
        let err = select_candidate_from_records_with_siblings(
            &state, &mut target, &target_obs, &[first], &[sibling],
        )
        .expect_err("an equal-eligibility tie must refuse the target");
        assert!(format!("{:?}", err).contains("SelectFailed"));
        assert_eq!(target.stage, TransitionRecord::ELIGIBLE);
    }

    #[test]
    fn select_with_siblings_ignores_expired_sibling() {
        // Sibling eligible at 100 with confidence 1.0 would win the
        // tie against the target (eligible 150, confidence 1.0) — but
        // the sibling expires at 150, so at the select clock 250 the
        // kernel skips it and the target selects.
        let mut state = select_state_at(250);
        state.eligible_sibling_count = 2; // slice 19: target + 1 supplied sibling(s)
        let (mut sib, sib_obs) = eligible_sibling_at(100, [0xD2; 32], [0xC1; 32], [0xE2; 32], 150);
        let sib_sig = sibling_signal_at(&mut sib, &sib_obs, &[], 2, SCALE, 100);
        let (mut target, target_obs) = eligible_sibling_at(150, [0xD1; 32], [0xC0; 32], [0xE1; 32], 10_100);
        let target_sig = sibling_signal_at(&mut target, &target_obs, &[], 2, SCALE, 150);
        let sibling = SiblingReplay {
            transition_record: &sib,
            observation_record: &sib_obs,
            prior_signals: &[sib_sig],
        };
        let life = select_candidate_from_records_with_siblings(
            &state, &mut target, &target_obs, &[target_sig], &[sibling],
        )
        .expect("an expired sibling must not block selection");
        assert_eq!(life, Life::Selected);
        assert_eq!(target.stage, TransitionRecord::SELECTED);
    }

    #[test]
    fn select_with_siblings_refuses_incomplete_sibling_set() {
        // The sibling's count is 2 but only its first signal is
        // supplied: the per-sibling completeness anchor refuses
        // before any replay, and the target's stage stays ELIGIBLE.
        let mut state = select_state_at(200);
        state.eligible_sibling_count = 2; // slice 19: target + 1 supplied sibling(s)
        let (_, mut target, target_obs, first) = selected_pair();
        let (mut sib, sib_obs) = eligible_sibling_at(100, [0xD2; 32], [0xC1; 32], [0xE2; 32], 10_100);
        let sib_first = sibling_signal_at(&mut sib, &sib_obs, &[], 2, SCALE, 100);
        let _sib_second = sibling_signal_at(&mut sib, &sib_obs, &[sib_first], 3, 0, 100);
        assert_eq!(sib.signal_count, 2);
        let sibling = SiblingReplay {
            transition_record: &sib,
            observation_record: &sib_obs,
            prior_signals: &[sib_first],
        };
        let err = select_candidate_from_records_with_siblings(
            &state, &mut target, &target_obs, &[first], &[sibling],
        )
        .expect_err("a short sibling signal set must refuse");
        assert!(format!("{:?}", err).contains("SelectFailed"));
        assert_eq!(target.stage, TransitionRecord::ELIGIBLE);
    }

    #[test]
    fn select_with_siblings_refuses_non_sibling_parent() {
        // A bundle whose parent hash is not the target's parent is
        // not a sibling at all: ParentMismatch, before any replay.
        let mut state = select_state_at(200);
        state.eligible_sibling_count = 2; // slice 19: target + 1 supplied sibling(s)
        let (_, mut target, target_obs, first) = selected_pair();
        let sib = TransitionRecord {
            transition_id: [0xD2; 32],
            transition_type: TT,
            parent_index: 0,
            parent_hash: [0xAB; 32],
            declared_next_hash: [0xE2; 32],
            observation_id: [0xC9; 32],
            expires_at: 10_100,
            proposer: Pubkey::new_from_array(ag(1)),
            stage: TransitionRecord::ELIGIBLE,
            eligible_at: 100,
            consensus_deadline: 200,
            planning_deadline: 0,
            signal_count: 0,
            bump: 7,
        };
        let sib_obs = ObservationRecord {
            observation_id: [0xC9; 32],
            timestamp: 100,
            expiry: 9100,
            confidence_raw: 900_000,
            observer: observer_pubkey(),
            bump: 7,
        };
        let sibling = SiblingReplay {
            transition_record: &sib,
            observation_record: &sib_obs,
            prior_signals: &[],
        };
        let err = select_candidate_from_records_with_siblings(
            &state, &mut target, &target_obs, &[first], &[sibling],
        )
        .expect_err("a non-sibling parent must refuse");
        assert!(format!("{:?}", err).contains("ParentMismatch"));
        assert_eq!(target.stage, TransitionRecord::ELIGIBLE);
    }

    #[test]
    fn select_with_siblings_refuses_wrong_stage_sibling() {
        // A sibling stopped at EVALUATED (never marked eligible) is
        // not a field member yet: SelectFailed, and the target's
        // stage stays ELIGIBLE.
        let mut state = select_state_at(200);
        state.eligible_sibling_count = 2; // slice 19: target + 1 supplied sibling(s)
        let (_, mut target, target_obs, first) = selected_pair();
        let eval_state = sealed_state();
        let mut sib = build_transition_record(
            &eval_state, [0xD2; 32], TT, 0, [0; 32], [0xE2; 32], [0xC1; 32], 10_100,
            Pubkey::new_from_array(ag(1)), 7,
        )
        .expect("sibling register must build");
        let sib_obs = build_observation_record(
            &eval_state, [0xC1; 32], 100, 9100, 900_000, observer_pubkey(), 7,
        )
        .expect("sibling observation must build");
        replay_transition_from_records(&eval_state, &mut sib, &sib_obs)
            .expect("sibling replay must succeed");
        evaluate_transition_from_records(&eval_state, &mut sib, &sib_obs)
            .expect("sibling evaluate must succeed");
        assert_eq!(sib.stage, TransitionRecord::EVALUATED);
        let sibling = SiblingReplay {
            transition_record: &sib,
            observation_record: &sib_obs,
            prior_signals: &[],
        };
        let err = select_candidate_from_records_with_siblings(
            &state, &mut target, &target_obs, &[first], &[sibling],
        )
        .expect_err("a non-eligible sibling must refuse");
        assert!(format!("{:?}", err).contains("SelectFailed"));
        assert_eq!(target.stage, TransitionRecord::ELIGIBLE);
    }

    #[test]
    fn select_with_siblings_refuses_duplicate_sibling() {
        // The same sibling bundle supplied twice is not two
        // siblings: SelectFailed, before any replay.
        let mut state = select_state_at(200);
        state.eligible_sibling_count = 3; // slice 19: target + 2 supplied sibling(s)
        let (_, mut target, target_obs, first) = selected_pair();
        let (mut sib, sib_obs) = eligible_sibling_at(100, [0xD2; 32], [0xC1; 32], [0xE2; 32], 10_100);
        let sib_sig = sibling_signal_at(&mut sib, &sib_obs, &[], 2, SCALE, 100);
        let sibling = SiblingReplay {
            transition_record: &sib,
            observation_record: &sib_obs,
            prior_signals: &[sib_sig],
        };
        let err = select_candidate_from_records_with_siblings(
            &state, &mut target, &target_obs, &[first], &[sibling, sibling],
        )
        .expect_err("a duplicated sibling must refuse");
        assert!(format!("{:?}", err).contains("SelectFailed"));
        assert_eq!(target.stage, TransitionRecord::ELIGIBLE);
    }

    #[test]
    fn select_with_siblings_refuses_mismatched_sibling_observation() {
        // The sibling's observation record carries a different id
        // than the sibling transition's linkage: RecordMismatch.
        let mut state = select_state_at(200);
        state.eligible_sibling_count = 2; // slice 19: target + 1 supplied sibling(s)
        let (_, mut target, target_obs, first) = selected_pair();
        let (mut sib, sib_obs) = eligible_sibling_at(100, [0xD2; 32], [0xC1; 32], [0xE2; 32], 10_100);
        let sib_sig = sibling_signal_at(&mut sib, &sib_obs, &[], 2, SCALE, 100);
        let sibling = SiblingReplay {
            transition_record: &sib,
            observation_record: &target_obs,
            prior_signals: &[sib_sig],
        };
        let err = select_candidate_from_records_with_siblings(
            &state, &mut target, &target_obs, &[first], &[sibling],
        )
        .expect_err("a mismatched sibling observation must refuse");
        assert!(format!("{:?}", err).contains("RecordMismatch"));
        assert_eq!(target.stage, TransitionRecord::ELIGIBLE);
    }

    #[test]
    fn select_with_siblings_empty_is_plain_select() {
        // No siblings supplied: the new helper reduces to the
        // slice-16 select, and the target selects at clock 200.
        let mut state = select_state_at(200);
        state.eligible_sibling_count = 1; // slice 19: target + 0 supplied sibling(s)
        let (_, mut target, target_obs, first) = selected_pair();
        let life = select_candidate_from_records_with_siblings(
            &state, &mut target, &target_obs, &[first], &[],
        )
        .expect("an empty sibling field must select like plain select");
        assert_eq!(life, Life::Selected);
        assert_eq!(target.stage, TransitionRecord::SELECTED);
    }

    // Slice 18 fixtures: a SELECTED sibling is an ELIGIBLE sibling
    // whose stored stage has been advanced to 4, exactly as the
    // slice-16 select instruction would have advanced it after its
    // own selection — the helper must re-prove that selection from
    // the chain and signals, not trust the stage value.

    #[test]
    fn select_with_selected_sibling_blocks_target() {
        // Sibling eligible at 100, target at 150, both confidence
        // 1.0, both windows closed at 250: the sibling's own select
        // is replayed first and wins its tie (earlier eligibility),
        // so the target's select meets the kernel's
        // ParentAlreadyHasASelected arm and refuses. The target
        // stays ELIGIBLE and the sibling record is untouched — the
        // helper never writes sibling records.
        let mut state = select_state_at(250);
        state.eligible_sibling_count = 2; // slice 19: target + 1 supplied sibling(s)
        let (mut sib, sib_obs) = eligible_sibling_at(100, [0xD2; 32], [0xC1; 32], [0xE2; 32], 10_100);
        let sib_sig = sibling_signal_at(&mut sib, &sib_obs, &[], 2, SCALE, 100);
        sib.stage = TransitionRecord::SELECTED;
        let (mut target, target_obs) = eligible_sibling_at(150, [0xD1; 32], [0xC0; 32], [0xE1; 32], 10_100);
        let target_sig = sibling_signal_at(&mut target, &target_obs, &[], 2, SCALE, 150);
        let sibling = SiblingReplay {
            transition_record: &sib,
            observation_record: &sib_obs,
            prior_signals: &[sib_sig],
        };
        let err = select_candidate_from_records_with_siblings(
            &state, &mut target, &target_obs, &[target_sig], &[sibling],
        )
        .expect_err("an already-selected sibling must block the target");
        assert!(format!("{:?}", err).contains("SelectFailed"));
        assert_eq!(target.stage, TransitionRecord::ELIGIBLE);
        assert_eq!(sib.stage, TransitionRecord::SELECTED);
    }

    #[test]
    fn select_with_selected_sibling_refuses_unqualified() {
        // A SELECTED stage label with a confidence-0 tally (one
        // support-0 signal) cannot be proven selected: the sibling's
        // own select fails its confidence threshold, the whole
        // helper refuses, and the target — which would otherwise
        // select over a merely eligible confidence-0 sibling — stays
        // ELIGIBLE. The stage label alone never blocks.
        let mut state = select_state_at(200);
        state.eligible_sibling_count = 2; // slice 19: target + 1 supplied sibling(s)
        let (_, mut target, target_obs, first) = selected_pair();
        let (mut sib, sib_obs) = eligible_sibling_at(100, [0xD2; 32], [0xC1; 32], [0xE2; 32], 10_100);
        let sib_sig = sibling_signal_at(&mut sib, &sib_obs, &[], 2, 0, 100);
        sib.stage = TransitionRecord::SELECTED;
        let sibling = SiblingReplay {
            transition_record: &sib,
            observation_record: &sib_obs,
            prior_signals: &[sib_sig],
        };
        let err = select_candidate_from_records_with_siblings(
            &state, &mut target, &target_obs, &[first], &[sibling],
        )
        .expect_err("an unqualified selected sibling must refuse");
        assert!(format!("{:?}", err).contains("SelectFailed"));
        assert_eq!(target.stage, TransitionRecord::ELIGIBLE);
    }

    #[test]
    fn select_with_selected_sibling_refuses_outranked() {
        // A SELECTED sibling at confidence 0.5 (supports 1.0 and 0)
        // against a confidence-1.0 target: the sibling's own select
        // fails OutrankedBySibling, so the helper refuses instead of
        // letting the stage label block a target that genuinely
        // outranks it.
        let mut state = select_state_at(200);
        state.eligible_sibling_count = 2; // slice 19: target + 1 supplied sibling(s)
        let (_, mut target, target_obs, first) = selected_pair();
        let (mut sib, sib_obs) = eligible_sibling_at(100, [0xD2; 32], [0xC1; 32], [0xE2; 32], 10_100);
        let sib_first = sibling_signal_at(&mut sib, &sib_obs, &[], 2, SCALE, 100);
        let sib_second = sibling_signal_at(&mut sib, &sib_obs, &[sib_first], 3, 0, 100);
        sib.stage = TransitionRecord::SELECTED;
        let sibling = SiblingReplay {
            transition_record: &sib,
            observation_record: &sib_obs,
            prior_signals: &[sib_first, sib_second],
        };
        let err = select_candidate_from_records_with_siblings(
            &state, &mut target, &target_obs, &[first], &[sibling],
        )
        .expect_err("an outranked selected sibling must refuse");
        assert!(format!("{:?}", err).contains("SelectFailed"));
        assert_eq!(target.stage, TransitionRecord::ELIGIBLE);
    }

    #[test]
    fn select_with_selected_sibling_refuses_open_window() {
        // The selected sibling's window (150/250) is still open at
        // the select clock 200 while the target's (100/200) is
        // closed: the sibling's own select cannot run before its
        // deadline, so the helper refuses and the target stays
        // ELIGIBLE.
        let mut state = select_state_at(200);
        state.eligible_sibling_count = 2; // slice 19: target + 1 supplied sibling(s)
        let (_, mut target, target_obs, first) = selected_pair();
        let (mut sib, sib_obs) = eligible_sibling_at(150, [0xD2; 32], [0xC1; 32], [0xE2; 32], 10_100);
        let sib_sig = sibling_signal_at(&mut sib, &sib_obs, &[], 2, SCALE, 150);
        sib.stage = TransitionRecord::SELECTED;
        let sibling = SiblingReplay {
            transition_record: &sib,
            observation_record: &sib_obs,
            prior_signals: &[sib_sig],
        };
        let err = select_candidate_from_records_with_siblings(
            &state, &mut target, &target_obs, &[first], &[sibling],
        )
        .expect_err("a selected sibling with an open window must refuse");
        assert!(format!("{:?}", err).contains("SelectFailed"));
        assert_eq!(target.stage, TransitionRecord::ELIGIBLE);
    }

    #[test]
    fn select_with_two_selected_siblings_refuses() {
        // A parent can have only one selected transition: two
        // supplied SELECTED siblings are an inconsistent field and
        // refuse at the gate, before any replay, with the target
        // left ELIGIBLE.
        let mut state = select_state_at(200);
        state.eligible_sibling_count = 3; // slice 19: target + 2 supplied sibling(s)
        let (_, mut target, target_obs, first) = selected_pair();
        let (mut sib1, sib1_obs) = eligible_sibling_at(100, [0xD2; 32], [0xC1; 32], [0xE2; 32], 10_100);
        let sib1_sig = sibling_signal_at(&mut sib1, &sib1_obs, &[], 2, SCALE, 100);
        sib1.stage = TransitionRecord::SELECTED;
        let (mut sib2, sib2_obs) = eligible_sibling_at(100, [0xD3; 32], [0xC2; 32], [0xE3; 32], 10_100);
        let sib2_sig = sibling_signal_at(&mut sib2, &sib2_obs, &[], 2, SCALE, 100);
        sib2.stage = TransitionRecord::SELECTED;
        let first_sibling = SiblingReplay {
            transition_record: &sib1,
            observation_record: &sib1_obs,
            prior_signals: &[sib1_sig],
        };
        let second_sibling = SiblingReplay {
            transition_record: &sib2,
            observation_record: &sib2_obs,
            prior_signals: &[sib2_sig],
        };
        let err = select_candidate_from_records_with_siblings(
            &state, &mut target, &target_obs, &[first], &[first_sibling, second_sibling],
        )
        .expect_err("two selected siblings must refuse");
        assert!(format!("{:?}", err).contains("SelectFailed"));
        assert_eq!(target.stage, TransitionRecord::ELIGIBLE);
    }

    #[test]
    fn select_with_selected_and_eligible_siblings_blocks_target() {
        // Mixed field: a winning SELECTED sibling (eligible 100,
        // confidence 1.0) plus a weaker ELIGIBLE sibling (confidence
        // 0) against a confidence-1.0 target eligible at 150, all
        // windows closed at 250. The selected sibling's own select
        // succeeds over both, then the target meets
        // ParentAlreadyHasASelected and refuses.
        let mut state = select_state_at(250);
        state.eligible_sibling_count = 3; // slice 19: target + 2 supplied sibling(s)
        let (mut selected, selected_obs) = eligible_sibling_at(100, [0xD2; 32], [0xC1; 32], [0xE2; 32], 10_100);
        let selected_sig = sibling_signal_at(&mut selected, &selected_obs, &[], 2, SCALE, 100);
        selected.stage = TransitionRecord::SELECTED;
        let (mut weaker, weaker_obs) = eligible_sibling_at(100, [0xD3; 32], [0xC2; 32], [0xE3; 32], 10_100);
        let weaker_sig = sibling_signal_at(&mut weaker, &weaker_obs, &[], 2, 0, 100);
        let (mut target, target_obs) = eligible_sibling_at(150, [0xD1; 32], [0xC0; 32], [0xE1; 32], 10_100);
        let target_sig = sibling_signal_at(&mut target, &target_obs, &[], 2, SCALE, 150);
        let selected_sibling = SiblingReplay {
            transition_record: &selected,
            observation_record: &selected_obs,
            prior_signals: &[selected_sig],
        };
        let weaker_sibling = SiblingReplay {
            transition_record: &weaker,
            observation_record: &weaker_obs,
            prior_signals: &[weaker_sig],
        };
        let err = select_candidate_from_records_with_siblings(
            &state, &mut target, &target_obs, &[target_sig], &[selected_sibling, weaker_sibling],
        )
        .expect_err("a mixed field with a selected winner must block the target");
        assert!(format!("{:?}", err).contains("SelectFailed"));
        assert_eq!(target.stage, TransitionRecord::ELIGIBLE);
    }

    #[test]
    fn genesis_kernel_state_census_is_empty_at_genesis_head() {
        // Slice 19: the census starts empty at the genesis parent —
        // no transition has become eligible anywhere yet, so a
        // select against this state must fail the census gate until
        // mark_eligible writes it.
        let state = genesis_kernel_state().expect("genesis must construct");
        assert_eq!(state.eligible_sibling_parent_index, 0);
        assert_eq!(state.eligible_sibling_parent_hash, [0; 32]);
        assert_eq!(state.eligible_sibling_count, 0);
    }

    #[test]
    fn select_with_selected_sibling_refuses_incomplete_set() {
        // The selected sibling's count is 2 but only its first
        // signal is supplied: the per-sibling completeness anchor
        // refuses before any replay, so the target cannot select
        // over a partially proven field.
        let mut state = select_state_at(200);
        state.eligible_sibling_count = 2; // slice 19: target + 1 supplied sibling(s)
        let (_, mut target, target_obs, first) = selected_pair();
        let (mut sib, sib_obs) = eligible_sibling_at(100, [0xD2; 32], [0xC1; 32], [0xE2; 32], 10_100);
        let sib_first = sibling_signal_at(&mut sib, &sib_obs, &[], 2, SCALE, 100);
        let _sib_second = sibling_signal_at(&mut sib, &sib_obs, &[sib_first], 3, 0, 100);
        assert_eq!(sib.signal_count, 2);
        sib.stage = TransitionRecord::SELECTED;
        let sibling = SiblingReplay {
            transition_record: &sib,
            observation_record: &sib_obs,
            prior_signals: &[sib_first],
        };
        let err = select_candidate_from_records_with_siblings(
            &state, &mut target, &target_obs, &[first], &[sibling],
        )
        .expect_err("a short selected sibling signal set must refuse");
        assert!(format!("{:?}", err).contains("SelectFailed"));
        assert_eq!(target.stage, TransitionRecord::ELIGIBLE);
    }

    // Slice 19 tests: the sibling-set completeness anchor.

    #[test]
    fn mark_eligible_advances_sibling_census() {
        // First eligible transition at the genesis parent: census
        // parent genesis, count 1. A second, distinct transition
        // becoming eligible at the same parent increments it to 2 —
        // the count tracks eligible transitions, not mark calls per
        // record (a second mark on the same record refuses on stage
        // and leaves the census untouched, proven elsewhere).
        let (mut state, mut record, obs) = eligible_pair();
        assert_eq!(state.eligible_sibling_count, 0);
        mark_eligible_from_records(&mut state, &mut record, &obs)
            .expect("first mark eligible must succeed");
        assert_eq!(state.eligible_sibling_parent_index, 0);
        assert_eq!(state.eligible_sibling_parent_hash, [0; 32]);
        assert_eq!(state.eligible_sibling_count, 1);
        let mut second = build_transition_record(
            &state, [0xD2; 32], TT, 0, [0; 32], [0xE2; 32], [0xC1; 32], 10_100,
            Pubkey::new_from_array(ag(1)), 7,
        )
        .expect("second register must build");
        let second_obs = build_observation_record(
            &state, [0xC1; 32], 100, 9100, 900_000, observer_pubkey(), 7,
        )
        .expect("second observation must build");
        replay_transition_from_records(&state, &mut second, &second_obs)
            .expect("second replay must succeed");
        evaluate_transition_from_records(&state, &mut second, &second_obs)
            .expect("second evaluate must succeed");
        mark_eligible_from_records(&mut state, &mut second, &second_obs)
            .expect("second mark eligible must succeed");
        assert_eq!(state.eligible_sibling_parent_index, 0);
        assert_eq!(state.eligible_sibling_parent_hash, [0; 32]);
        assert_eq!(state.eligible_sibling_count, 2);
    }

    #[test]
    fn mark_eligible_resets_sibling_census_at_new_parent() {
        // A census recorded at another parent (stale: the head moved
        // since) resets to the new parent with count 1 on the first
        // eligibility at that parent — it is never carried across a
        // parent change, and it is never added to.
        let (mut state, mut record, obs) = eligible_pair();
        state.eligible_sibling_parent_index = 9;
        state.eligible_sibling_parent_hash = [0xAB; 32];
        state.eligible_sibling_count = 99;
        mark_eligible_from_records(&mut state, &mut record, &obs)
            .expect("mark eligible must succeed");
        assert_eq!(state.eligible_sibling_parent_index, 0);
        assert_eq!(state.eligible_sibling_parent_hash, [0; 32]);
        assert_eq!(state.eligible_sibling_count, 1);
    }

    #[test]
    fn mark_eligible_refusal_leaves_sibling_census() {
        // A refused eligibility writes nothing: the census stays
        // empty at the genesis parent, so a later select cannot be
        // authorized by a transition that never became eligible.
        let (state, mut record, obs) = eligible_pair();
        let mut advanced = state;
        advance_clock_on_state(&mut advanced, 9100).expect("advance must succeed");
        let err = mark_eligible_from_records(&mut advanced, &mut record, &obs)
            .expect_err("lapsed observation must refuse eligibility");
        assert!(format!("{:?}", err).contains("EligibleFailed"));
        assert_eq!(advanced.eligible_sibling_parent_index, 0);
        assert_eq!(advanced.eligible_sibling_parent_hash, [0; 32]);
        assert_eq!(advanced.eligible_sibling_count, 0);
        assert_eq!(record.stage, TransitionRecord::EVALUATED);
    }

    #[test]
    fn select_refuses_when_census_reports_another_eligible() {
        // The plain path's bypass, closed: the target qualifies and
        // its window is closed, but the census reports 2 eligible
        // transitions at this parent — a competitor exists that
        // the rebuilt single-transition kernel cannot judge, so the
        // plain select refuses and the sibling-aware instruction is
        // the only path. The target stays ELIGIBLE.
        let (mut state, mut record, obs, first) = selected_pair();
        advance_clock_on_state(&mut state, 200).expect("advance must succeed");
        assert_eq!(state.eligible_sibling_count, 1);
        state.eligible_sibling_count = 2;
        let err = select_candidate_from_records(&state, &mut record, &obs, &[first])
            .expect_err("a census above 1 must refuse the plain select");
        assert!(format!("{:?}", err).contains("SelectFailed"));
        assert_eq!(record.stage, TransitionRecord::ELIGIBLE);
    }

    #[test]
    fn select_with_siblings_refuses_omitted_sibling() {
        // The slice-17 honesty gap, closed: two eligible siblings
        // exist at this parent (census 3 with the target), but only
        // the first is supplied. The supplied field is otherwise
        // complete — the sibling's signal set matches its count —
        // so only the census can catch the omission, and it refuses
        // before any replay. The target stays ELIGIBLE.
        let mut state = select_state_at(200);
        state.eligible_sibling_count = 3;
        let (_, mut target, target_obs, first) = selected_pair();
        let (mut sib1, sib1_obs) = eligible_sibling_at(100, [0xD2; 32], [0xC1; 32], [0xE2; 32], 10_100);
        let sib1_sig = sibling_signal_at(&mut sib1, &sib1_obs, &[], 2, 0, 100);
        let (_sib2, _sib2_obs) = eligible_sibling_at(100, [0xD3; 32], [0xC2; 32], [0xE3; 32], 10_100);
        let sibling = SiblingReplay {
            transition_record: &sib1,
            observation_record: &sib1_obs,
            prior_signals: &[sib1_sig],
        };
        let err = select_candidate_from_records_with_siblings(
            &state, &mut target, &target_obs, &[first], &[sibling],
        )
        .expect_err("a field short of the census must refuse");
        assert!(format!("{:?}", err).contains("SelectFailed"));
        assert_eq!(target.stage, TransitionRecord::ELIGIBLE);
    }

    #[test]
    fn select_with_siblings_refuses_census_parent_mismatch() {
        // The count is right (2: target + 1 sibling) but the census
        // parent is another parent's: a count recorded elsewhere
        // says nothing about this field, so the helper refuses
        // before any replay rather than trusting the number.
        let mut state = select_state_at(200);
        state.eligible_sibling_count = 2;
        state.eligible_sibling_parent_index = 9;
        let (_, mut target, target_obs, first) = selected_pair();
        let (mut sib, sib_obs) = eligible_sibling_at(100, [0xD2; 32], [0xC1; 32], [0xE2; 32], 10_100);
        let sib_sig = sibling_signal_at(&mut sib, &sib_obs, &[], 2, 0, 100);
        let sibling = SiblingReplay {
            transition_record: &sib,
            observation_record: &sib_obs,
            prior_signals: &[sib_sig],
        };
        let err = select_candidate_from_records_with_siblings(
            &state, &mut target, &target_obs, &[first], &[sibling],
        )
        .expect_err("a census at another parent must refuse");
        assert!(format!("{:?}", err).contains("SelectFailed"));
        assert_eq!(target.stage, TransitionRecord::ELIGIBLE);
    }

    fn planning_pair() -> (KernelState, TransitionRecord, ObservationRecord, SignalRecord) {
        // One qualifying signal at clock 100; at the deadline 200
        // the window is closed and the tally qualifies, so the
        // kernel selects and the stage advances to 4.
        let (mut state, mut record, obs, first) = selected_pair();
        advance_clock_on_state(&mut state, 200).expect("advance to the deadline must succeed");
        let life = select_candidate_from_records(&state, &mut record, &obs, &[first])
            .expect("closed window with a qualifying tally must select");
        assert_eq!(life, Life::Selected);
        assert_eq!(record.stage, TransitionRecord::SELECTED);
        assert_eq!(record.planning_deadline, 0);
        (state, record, obs, first)
    }

    #[test]
    fn open_planning_advances_selected_to_planning() {
        // At clock 200 the planning window is 100, so the deadline
        // is 300. The caller is a fresh id — not the proposer, not
        // a seeded agent — because the kernel's Selected-path rule
        // is "anyone", and a proposer-only reading would contradict
        // it.
        let (state, mut record, obs, first) = planning_pair();
        let life = open_planning_from_records(&state, &mut record, &obs, &[first], [0x77; 32])
            .expect("a kernel-proven selection must open planning");
        assert_eq!(life, Life::Planning);
        assert_eq!(record.stage, TransitionRecord::PLANNING);
        assert_eq!(record.stage, 5);
        assert_eq!(record.planning_deadline, 300);
        assert_eq!(record.signal_count, 1);
    }

    #[test]
    fn open_planning_refuses_wrong_stage() {
        // Still ELIGIBLE, never selected: the stage gate refuses
        // before any replay, and nothing on the record moves.
        let (state, mut record, obs, first) = selected_pair();
        let err = open_planning_from_records(&state, &mut record, &obs, &[first], ag(1))
            .expect_err("an unselected record must refuse planning");
        assert!(format!("{:?}", err).contains("WrongStage"));
        assert_eq!(record.stage, TransitionRecord::ELIGIBLE);
        assert_eq!(record.planning_deadline, 0);
    }

    #[test]
    fn open_planning_is_once_only() {
        let (state, mut record, obs, first) = planning_pair();
        open_planning_from_records(&state, &mut record, &obs, &[first], ag(1))
            .expect("first planning open must succeed");
        let err = open_planning_from_records(&state, &mut record, &obs, &[first], ag(1))
            .expect_err("a second planning open must refuse");
        assert!(format!("{:?}", err).contains("WrongStage"));
        assert_eq!(record.stage, TransitionRecord::PLANNING);
        assert_eq!(record.planning_deadline, 300);
    }

    #[test]
    fn open_planning_refuses_incomplete_prior_set() {
        // The record claims a second signal that was never
        // supplied: the slice-16 anchor refuses before any replay,
        // and the SELECTED stage and zero deadline stand.
        let (state, mut record, obs, first) = planning_pair();
        record.signal_count = 2;
        let err = open_planning_from_records(&state, &mut record, &obs, &[first], ag(1))
            .expect_err("a short prior set must refuse planning");
        assert!(format!("{:?}", err).contains("PlanningFailed"));
        assert_eq!(record.stage, TransitionRecord::SELECTED);
        assert_eq!(record.planning_deadline, 0);
    }

    #[test]
    fn open_planning_reproves_selection_at_the_clock() {
        // A SELECTED label with the clock moved back before the
        // consensus deadline: the label alone opens nothing — the
        // replayed select refuses (window still open) and planning
        // stays closed with the deadline untouched.
        let (mut state, mut record, obs, first) = planning_pair();
        state.clock = 199;
        let err = open_planning_from_records(&state, &mut record, &obs, &[first], ag(1))
            .expect_err("an unreproducible selection must refuse planning");
        assert!(format!("{:?}", err).contains("PlanningFailed"));
        assert_eq!(record.stage, TransitionRecord::SELECTED);
        assert_eq!(record.planning_deadline, 0);
    }

    #[test]
    fn open_planning_at_later_clock_sets_later_deadline() {
        // Planning opens at the clock it runs at, not the clock
        // selection ran at: at 500 the deadline is 600.
        let (mut state, mut record, obs, first) = planning_pair();
        advance_clock_on_state(&mut state, 500).expect("advance past selection must succeed");
        open_planning_from_records(&state, &mut record, &obs, &[first], ag(1))
            .expect("planning at a later clock must succeed");
        assert_eq!(record.stage, TransitionRecord::PLANNING);
        assert_eq!(record.planning_deadline, 600);
    }

    #[test]
    fn open_planning_refuses_unsealed() {
        let (mut state, mut record, obs, first) = planning_pair();
        state.sealed = false;
        let err = open_planning_from_records(&state, &mut record, &obs, &[first], ag(1))
            .expect_err("an unsealed state must refuse planning");
        assert!(format!("{:?}", err).contains("NotSealed"));
        assert_eq!(record.stage, TransitionRecord::SELECTED);
        assert_eq!(record.planning_deadline, 0);
    }

    #[test]
    fn open_planning_refuses_parent_mismatch() {
        let (mut state, mut record, obs, first) = planning_pair();
        state.head_index = 9;
        let err = open_planning_from_records(&state, &mut record, &obs, &[first], ag(1))
            .expect_err("a record off the sealed head must refuse planning");
        assert!(format!("{:?}", err).contains("ParentMismatch"));
        assert_eq!(record.stage, TransitionRecord::SELECTED);
        assert_eq!(record.planning_deadline, 0);
    }
}
