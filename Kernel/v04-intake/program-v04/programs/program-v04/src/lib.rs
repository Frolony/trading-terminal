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
//! Slice 21 (this slice): the first plan instruction, `submit_plan` —
//! a `PLANNING` record gains a per-agent `PlanRecord`, and the stage
//! stays `PLANNING` (plans do not advance it; reconcile does, later).
//! The helper re-proves the whole chain in a rebuilt kernel (chain +
//! priors + select + open_planning + every prior plan) and only then
//! runs kernel `submit_plan` at the state's clock, so a stored
//! `PLANNING` label a replay cannot reproduce never accepts a plan.
//! Two replay clocks, not one, and the distinction is the design:
//! select and open_planning replay at the *derived open clock*
//! (`planning_deadline - planning_window`), because that is the
//! clock the stored deadline was set from — replaying them at the
//! state's clock would silently move the deadline a plan is judged
//! against — while prior plans and the new plan run at the state's
//! clock, which the gates require to sit strictly inside the stored
//! window (`open_clock <= clock < planning_deadline`). The new
//! plan's expiry is likewise judged against the *stored* deadline,
//! not a replayed one. The plan's agent is the signing payer's agent
//! id (the kernel's `plan.agent == caller` rule, enforced by the PDA
//! seeds on-chain and by the helper's caller argument in replay),
//! and unlike open_planning's anyone rule, the kernel's agent gate
//! applies here: an unknown or inactive agent, or an agent outside
//! the proposer's groups, refuses through the kernel, not through a
//! program-side guess. Claims are a bounded multi-resource map
//! (`MAX_PLAN_CLAIMS` = 8, parallel fixed arrays on the record): a
//! longer claim list refuses rather than truncating, and a
//! duplicated resource refuses rather than collapsing silently the
//! way a map insert would. `plan_count` on the transition record,
//! written on success only, is the completeness anchor the later
//! reconcile slice checks its supplied plan set against, exactly as
//! `signal_count` anchors select. Honesty bounds, stated not
//! smoothed: the replay horizon is the derived open clock, so a
//! plan submitted after the transition's manifest expiry refuses
//! here — but the kernel would refuse it too (`not_expired` at the
//! state's clock), so no legitimate submission is lost to the
//! replay; and the instruction's remaining accounts split
//! deterministically (the first `signal_count` are signals, the rest
//! plans), so a miscounted remainder refuses at the anchor, not at
//! a parse. Reconcile, validate, declare, and commit replay are
//! later slices, not smuggled in here.
//!
//! Slice 22 (this slice): the Blocked re-plan path — `open_planning`
//! now admits a `BLOCKED` record (stage 6, new `replans: u32` field,
//! SPACE 258 -> 262) as well as `SELECTED`. The kernel's re-plan rule
//! is enforced by the helper from the stored record, in the kernel's
//! own order: caller is the record's proposer or a seat of the sealed
//! launch council (seats 1–3, pinned genesis), and the stored replan
//! count is below the kernel's `MAX_REPLANS` (3, imported from the
//! kernel crate, never re-stated) — a stranger or a capped record
//! refuses with `ReplanRefused` and nothing moves. On a re-plan
//! success the stage advances to `PLANNING`, the deadline resets to
//! `clock + planning_window`, and `replans` increments by exactly
//! one; the Selected path leaves `replans` untouched, and a re-plan
//! never clears `plan_count` or the stored plan set, because the
//! kernel carries its plan map across a re-plan unchanged.
//! Honesty bound, stated not smoothed: the replay kernel reaches
//! `Selected`, so the kernel call itself takes its anyone path —
//! the re-plan authority and cap live in the helper, not in the
//! replay, because no slice can yet replay a transition INTO
//! `Blocked` (reconcile/validate are later slices). The stored
//! `BLOCKED` label is therefore trusted this slice in exactly one
//! narrow sense, and it is safe to trust that far: no instruction
//! writes `BLOCKED` yet, so no live record can carry it, and the
//! label becomes program-written — hence re-provable — the day
//! validate lands. Until then the re-plan tests set the label as a
//! fixture standing in for validate's future output. Selection, the
//! proposer, and the seat roster are all re-proven in every call.
//! (Slice 24 note: validate has now landed and writes `BLOCKED`,
//! so the label is program-produced; the re-plan tests' fixture
//! label is the same label validate produces.)
//! Reconcile, validate, declare, and commit replay are still later
//! slices, not smuggled in here.
//!
//! Slice 23 (this slice): the first reconcile instruction,
//! `reconcile_plans` — `PLANNING` (5) -> `RECONCILING` (7, a new
//! program stage value; program stage numbers are the program's own
//! sequence, not the kernel's `Life` discriminants — kernel
//! `Reconciling` is 6, kernel `Blocked` is 17, program `BLOCKED` is
//! 6). The helper re-proves the whole chain in a rebuilt kernel
//! (chain + priors + select + open_planning + every prior plan)
//! and only then runs kernel `reconcile_plans` at the state's
//! clock, so a stored `PLANNING` label a replay cannot reproduce
//! never reconciles. Two replay clocks, and the second is the
//! slice's design point: select and open_planning replay at the
//! derived open clock (`planning_deadline - planning_window`), as
//! in slice 21, and prior plans replay at that same open clock —
//! because a `PlanRecord` carries no submission time, no replay
//! can run a plan at its true submission clock, and the open clock
//! is the one window clock every stored plan is guaranteed to
//! satisfy (inside the window, with expiry after the deadline —
//! the only clock rules kernel `submit_plan` applies, besides
//! manifest expiry, which the state's clock already proves).
//! Reconcile itself runs at the state's clock, which the gates
//! require to sit at or after the stored deadline: before it the
//! kernel's `PlanningWindowOpen` refuses, and the record stands.
//! The kernel's verdict is the program's verdict: the stage
//! advances on a kernel-proven reconcile whether the plans fit or
//! not — an empty plan set reconciles to incompatible (the kernel
//! never calls a plan-less transition compatible), and an
//! over-capacity set reconciles to incompatible, not to a
//! refusal — and the verdict is persisted on the record
//! (`compatible: bool`, SPACE 262 -> 263) and carried on the
//! event, so slice 24 cross-checks the stored verdict against its
//! own recomputation over the supplied plan set instead of trusting
//! either alone. The mirror that computes the stored verdict is
//! exact, not approximate: the same supplied plan set, the same
//! checked-add sums, the same sealed capacity stocks the kernel
//! reads — a divergence between mirror and kernel would mean a
//! supplied plan the replay did not judge, which the anchors
//! already forbid. The payer is a fee payer only: kernel
//! `reconcile_plans` takes no caller, so no proposer or seat gate
//! applies here. Declare and commit replay are still later
//! slices, not smuggled in here.
//!
//! Slice 24 (this slice): the first validate instruction,
//! `validate_transition` — `RECONCILING` (7) -> `VALIDATED` (8,
//! a new program stage value; program stage numbers are the
//! program's own sequence, not the kernel's `Life`
//! discriminants — kernel `Validated` is 7, kernel `Blocked` is
//! 17, program `BLOCKED` is 6). The helper re-proves the whole
//! chain in a rebuilt kernel (chain + priors + select +
//! open_planning + every prior plan + reconcile at the state's
//! clock) and only then runs kernel `validate_transition` at the
//! state's clock, so a stored `RECONCILING` label a replay cannot
//! reproduce never validates. Two verdicts, and the second is the
//! slice's design point: the stored `compatible` verdict is
//! cross-checked, not trusted — slice 23's exact mirror is
//! recomputed over the supplied plan set, and a stored verdict
//! the set does not reproduce refuses before any kernel validate
//! runs — because a divergence means a supplied plan the replay
//! did not judge, or a stored verdict no replay reproduces, and
//! either way the program must not advance the record on it.
//! The kernel's own validate then judges the rest of the
//! predicate at the state's clock: constraints, compatibility,
//! blocking exceptions, manifest expiry, observation freshness.
//! A passing predicate advances the stage to `VALIDATED`; a
//! failing predicate from `RECONCILING` advances it to `BLOCKED`
//! — the first program-written `BLOCKED` label, which closes the
//! slice-22 honesty bound (the re-plan tests' fixture label is
//! now the label validate produces) and opens the slice-22
//! re-plan path for real; a still-blocked `BLOCKED` record
//! refuses and stands, exactly as the kernel's `StillBlocked`
//! arm dictates — enforced from the stored stage, because the
//! replay kernel reaches `Reconciling` and never carries the
//! record's `Blocked` life into the kernel call. The payer is
//! a fee payer only: kernel
//! `validate_transition` takes no caller, so no proposer or seat
//! gate applies here. The replay carries no exceptions, so
//! `unresolved_blocking` is always zero in this slice — program
//! validate judges the stored chain, not off-program exception
//! state, and the exception instructions that would clear a block
//! are a later slice's work. Commit replay is still a later
//! slice, not smuggled in here.
//! Slice 25 (this slice): the first declare instruction,
//! `declare_transition` — `VALIDATED` (8) -> `DECLARED` (9,
//! a new program stage value; program stage numbers are the
//! program's own sequence, not the kernel's `Life`
//! discriminants — kernel `Committable` is 8, program `DECLARED`
//! is 9). The helper re-proves the whole chain in a rebuilt
//! kernel (chain + priors + select + open_planning + every prior
//! plan + reconcile + validate at the state's clock) and only
//! then runs kernel `declare_commit` at the state's clock, so a
//! stored `VALIDATED` label a replay cannot reproduce never
//! declares. Two refusal arms, and the second is the slice's
//! design point: declare is NOT a verdict writer — the kernel's
//! `declare_commit` never moves `Validated` -> `Blocked`
//! (only the exception path does), so a replay validate that
//! yields `Blocked` (a predicate that no longer holds at this
//! clock, e.g. a stale observation) or a `declare_commit` whose
//! full predicate fails (e.g. a plan expiry reached since
//! validate — the `PLANS` mask bit, which validate's
//! structural check does not judge) refuses with
//! `DeclareFailed` and the record stands `VALIDATED`, exactly
//! as the kernel leaves its life untouched. The stored
//! `compatible` verdict is cross-checked, not trusted, exactly
//! as in slice 24. The payer is a fee payer only: kernel
//! `declare_commit` is permissionless — the predicate is the
//! gate, not a person — so no proposer, seat, or agent gate
//! applies here. The sealed head does NOT move: declare marks
//! a transition committable; commit replay (slice 26) moves
//! the head.
//! Slice 26 (this slice): the first commit instruction,
//! `commit_transition` — `DECLARED` (9) -> `COMMITTED` (10,
//! a new program stage value; program stage numbers are the
//! program's own sequence, not the kernel's `Life`
//! discriminants — kernel `Committed` is 9, program `DECLARED`
//! is 9, program `COMMITTED` is 10). The helper re-proves the
//! whole chain in a rebuilt kernel (chain + priors + select +
//! open_planning + every prior plan + reconcile + validate +
//! declare at the state's clock) and only then runs kernel
//! `commit` at the state's clock, so a stored `DECLARED` label
//! a replay cannot reproduce never commits. The commit itself
//! re-runs the entire predicate a third time (`commit_check`
//! inside `commit`): a predicate that no longer holds at this
//! clock — a plan expiry reached since declare, a stale
//! observation, a capacity stock the claims no longer fit —
//! refuses with `CommitFailed` and the record stands
//! `DECLARED` with the sealed head unmoved, exactly as the
//! kernel leaves its life `Committable` (commit's check is
//! non-mutating; nothing moves until the check passes). On
//! success the program writes the kernel's returned
//! `SystemState` into `KernelState` — head index one past the
//! old head, head hash exactly the record's declared next
//! hash, both asserted before any write — and the record's
//! stage advances to `COMMITTED`. The payer is a fee payer
//! only: kernel `commit` is permissionless, so no proposer,
//! seat, or agent gate applies here. Governance transitions
//! refuse at the gate with `GovernanceCommitRequired`: kernel
//! `commit` itself refuses them (`UseGovernanceCommit`) and
//! their commit path is the slice-28 `commit_governance`
//! helper — and the gate is defense in depth today, stated plainly:
//! no governance record can reach `DECLARED` through the
//! ordinary chain, because kernel `register_transition`
//! refuses governance types outright, so the gate guards a
//! record whose type field was written by a future
//! governance-registration slice, never a live path this
//! slice invents. Honesty bound, stated not smoothed: the
//! capacity debit and the dialect effects of `commit_apply`
//! run in the rebuilt kernel, which is discarded — the
//! program persists the head move, not the post-commit
//! capacity stocks — before slice 27, each commit was judged
//! against the sealed launch stocks, not against post-commit
//! stocks, and two successive commits were not
//! capacity-checked against each other. Slice 27 lands the
//! ledger below, so a commit is now judged against the sealed
//! stocks minus prior committed claims. The head move is real
//! and usable — a new record registers against the new head.
//! CU measurement is still a later slice, not smuggled in
//! here.
//! Slice 27 (this slice): the committed-claims ledger.
//! `KernelState` gains `committed_claims`, the cumulative
//! committed claim per launch resource (treasury tranche,
//! lending book, exit window, in that order), in kernel
//! `Amount` raw units. `commit_transition` computes the new
//! transition's per-resource claim sums from its prior plans
//! (the same sums the slice-23 mirror recomputes), and after
//! the replayed declare succeeds and before kernel `commit`
//! runs, refuses with `CommitFailed` — record standing
//! `DECLARED`, head unmoved, ledger untouched — when any
//! resource's prior committed total plus the new sum would
//! exceed the sealed capacity stock the rebuilt kernel
//! reports. On success the new totals are written together
//! with the stage advance and the head move, so a later
//! commit is judged against what prior commits already
//! consumed, not against the sealed stocks alone. The ledger
//! is totals, not per-commit entries: it enforces the
//! capacity bound across commits; it does not by itself
//! attribute a past debit to a transition. Declaration is
//! still not a reservation: reconcile, validate, and declare
//! judge a transition's own plan set against the sealed
//! stocks, so a transition can declare and then refuse at
//! commit when the ledger is exhausted. And the replay
//! horizon is unchanged: the rebuilt kernel starts from the
//! sealed genesis head, so a second transition whose parent
//! is the moved head still cannot replay its chain through
//! the current helpers — end-to-end two-commit sequencing
//! needs a replay-from-head slice (slice 29 below); the ledger
//! gate is tested at its enforcement point with pre-seeded
//! totals standing in for prior commits.
//!
//! Slice 28 (this slice): the governance commit path,
//! `commit_governance_from_records` — `DECLARED` (9) ->
//! `COMMITTED` (10), the sealed head moving exactly as in
//! slice 26, but for a governance-typed record. The helper
//! mirrors the ordinary commit helper step for step, with
//! two governance differences: council approvals precede
//! registration (each listed seat approves this exact action
//! at the stored `eligible_at`; the kernel's unanimity gate,
//! not the program, judges whether every sitting seat did),
//! and registration runs through kernel
//! `register_governance_transition`, ending in kernel
//! `commit_governance`, which checks the action and applies
//! it atomically with the commit. The action is a helper
//! argument, not a stored field: `TransitionRecord` carries
//! no action, and no program instruction yet produces a
//! `DECLARED` governance record (kernel `register_transition`
//! refuses governance types, so the ordinary chain cannot
//! declare one) — so this slice is helper + host tests only,
//! proven with hand-crafted `DECLARED` governance records
//! whose labels the helper never trusts (it re-proves the
//! whole governance chain from the stored records, the
//! action, and the approvals). The governance-registration
//! instruction, the approval persistence, and the
//! instruction that calls this helper are later slices, not
//! smuggled in here. Honesty bound, stated not smoothed:
//! the kernel's action application runs in the rebuilt
//! kernel, which is discarded — the program persists the
//! stage advance and the head move, not the applied rule.
//! A committed `SetCapacity` therefore does not yet change
//! the stocks later sealed replays judge against (they
//! rebuild from the genesis constants); governance-state
//! persistence is the slice that closes that gap, and until
//! it lands, a governance commit's rule effect is recorded
//! by the head move alone, not enacted in later replays.
//! The ledger gate runs over the supplied plan sums exactly
//! as in slice 27 (governance plans claim no stock in the
//! bridge shape, so the gate is a no-op there — but a
//! claims-carrying governance plan would be ledger-bound
//! identically, not passed through unchecked). CU
//! measurement is still a later slice, not smuggled in here.
//!
//! Slice 29 (this slice): replay-from-head,
//! `kernel_at_head_from_commits` — a rebuilt sealed launch
//! kernel advanced to the program's current head by replaying
//! a supplied chain of prior `COMMITTED` bundles, each driven
//! through its full ordinary commit chain (ingest, register,
//! evaluate, eligible, signals, select, open_planning, plans,
//! reconcile, validate, declare, commit) and
//! checked, link by link, against the running head: a prior
//! whose parent is not the running head, whose stored label
//! is not `COMMITTED`, whose plan set does not reproduce its
//! stored `compatible` verdict, or whose commit does not land
//! exactly one past its parent at its declared next hash
//! refuses, and the returned kernel is discarded on any
//! refusal — a partial history is never a head. The clock
//! rule is the slice's design point, and it differs from the
//! target commit's on purpose: a prior is historical
//! reproduction, so its final steps (reconcile, validate,
//! declare, commit) run at its stored `planning_deadline` —
//! the clock its declare/commit originally proved at — not
//! at the current state clock, because replaying an old
//! commit at today's clock would re-judge it under
//! conditions that no longer hold (a plan whose expiry has
//! passed since) and brick every chain older than one plan
//! expiry. A prior whose deadline lies ahead of the state
//! clock is history from the future and refuses. Because the
//! kernel clock never regresses, a valid successor chain is
//! time-ordered by construction: each prior's window must
//! open after the previous prior's deadline, and a target
//! transition replayed from this head must open after the
//! last prior's deadline — a target created before the last
//! commit is a sibling of it, not its successor, and its
//! parent is the old head, not this one. Governance priors
//! refuse with `GovernanceCommitRequired`: a bundle carries
//! no action and no approvals, so a governance commit cannot
//! be driven from it, and skipping it would silently drop a
//! head move. This slice is helper + host tests only: the
//! thirteen stage helpers and the commit instructions still
//! rebuild from genesis, and threading this head through
//! them is slice 30 below, not smuggled in here. The
//! program's slice-27 ledger is untouched by this helper —
//! the kernel's own post-commit stocks, debited by each
//! replayed commit, are what later replays judge against —
//! and a governance commit's applied rule still awaits the
//! governance-state persistence slice, exactly as in
//! slice 28.
//!
//! Slice 30 (this slice): threading the head through the
//! thirteen stage helpers — replay, evaluate, mark_eligible,
//! submit_signal, select_candidate, select_candidate_with_
//! siblings, open_planning, submit_plan, reconcile_plans,
//! validate_transition, declare_transition,
//! commit_transition, and commit_governance. Each helper
//! now takes `prior_commits: &[PriorCommit]` and builds
//! its kernel with slice 29's `kernel_at_head_from_commits`
//! instead of the sealed genesis kernel; empty priors are
//! exactly the genesis case, and a moved head with empty
//! priors refuses with `ParentMismatch` — a missing chain
//! is never silently replayed against genesis stocks. The
//! replay helper moves from the `NoEnv` genesis kernel to
//! the `ProgramEnv` head kernel: ingest and register never
//! consult the environment, so genesis behaviour is
//! identical, and only a `ProgramEnv` kernel can drive a
//! prior's evaluate step. The successor clock rule is
//! slice 29's, unchanged: a target's window must open
//! after the last prior's planning deadline, or the
//! kernel's tick refuses the regression — a target whose
//! window opened before the head moved is a sibling of the
//! head, not its successor. The slice-27 ledger gate in
//! the commit helpers is unchanged: plan sums plus the
//! persisted ledger totals are still judged against the
//! sealed stocks, as a cross-check on the kernel's own
//! post-prior stocks, which judge the same sums against
//! what prior commits already consumed. Deliberately NOT
//! slice 30, and why (recorded, not smoothed): the
//! thirteen instructions still call the helpers with
//! `&[]`, so on-chain instructions still refuse at a
//! moved head with `ParentMismatch` — threading prior
//! bundles through instruction remaining accounts is
//! slice 31 below, not smuggled in here. The genesis-profile
//! gates are likewise untouched: `resolve_proposer` /
//! `build_observation_record` and open_planning's seated
//! re-plan check still judge the sealed launch profile,
//! because a governance commit's applied rule still
//! awaits the governance-state persistence slice — and a
//! head moved by a governance commit still cannot be
//! threaded at all, since `kernel_at_head_from_commits`
//! refuses governance priors with
//! `GovernanceCommitRequired`; governance TARGETS at an
//! ordinary moved head do thread, and are tested.
//!
//! Slice 31 (this slice): threading prior bundles through the
//! instruction remaining accounts — the twelve stage instructions
//! (replay, evaluate, mark_eligible, submit_signal, select,
//! select_with_siblings, open_planning, submit_plan, reconcile,
//! validate, declare, commit) each gain a `prior_bundle_count`
//! argument, and their remaining accounts begin with exactly that
//! many prior COMMITTED bundles in head order, followed by the
//! target's own evidence exactly as before. A bundle is
//! self-delimiting: its transition record, then the observation it
//! names, then exactly its stored `signal_count` signals, then
//! exactly its stored `plan_count` plans — the anchors slices 16
//! and 21 already trust — so no new instruction argument carries a
//! bundle length and no caller count can shorten a chain. The
//! parser (`split_prior_bundles`, over a classified account list)
//! checks every linkage at parse time and refuses with
//! `RecordMismatch` on a mis-linked, truncated, or over-counted
//! bundle before any kernel replay runs; the stage gate stays in
//! slice 29's driver, not duplicated in the parser. Instructions
//! with no own evidence (replay, evaluate, mark_eligible) refuse
//! any account past the bundles. The commit_governance helper has
//! no instruction yet, so it takes no bundles here — its
//! instruction arrives with governance-registration. Deliberately
//! NOT slice 31: governance-registration, governance-state
//! persistence, the cu-golden freeze, or any main-line work.
//!
//! Slice 32 (this slice): the governance-registration instruction,
//! `register_governance_transition` — the governance mirror of
//! slice 8's `register_transition`. It creates a `TransitionRecord`
//! at stage `REGISTERED` for a governance-typed transition, gated
//! on the sealed head (parent exactly the `KernelState` head, as
//! in slice 8) plus two governance gates the ordinary path does
//! not carry: the type must be a governance type (otherwise
//! `GovernanceTypeRequired`), and the signing payer must be a
//! seated council member of the sealed launch profile (otherwise
//! `NotCouncilSeat`) — the program-side mirror of kernel
//! `register_governance_transition`'s `NotCouncil` gate, judged
//! against the pinned profile, not a caller-supplied roster.
//! The ordinary builder refuses governance types in this slice
//! (`GovernanceRegisterRequired`), so the two registration paths
//! are disjoint exactly as the kernel's `register_transition` /
//! `register_governance_transition` pair is: a governance record
//! can no longer be created through the ordinary instruction
//! without the seat gate, and an ordinary record cannot enter
//! through the governance instruction. Honesty bound, stated
//! not smoothed: this slice stores the governance manifest
//! only. `TransitionRecord` still carries no action, and no
//! approval is persisted here — the action encoding and the
//! approval accounts are the approval-persistence slice's work,
//! so a registered governance record still cannot advance: the
//! ordinary replay refuses governance types, and the
//! governance replay that would drive it (ingest +
//! `register_governance_transition` from stored action and
//! approvals, then the stage chain to the slice-28
//! `commit_governance` helper) does not exist yet. Slice 31's
//! "its instruction arrives with governance-registration" is
//! read here as the governance family, not this slice: the
//! `commit_governance` instruction still awaits stored action
//! and approvals, and is not smuggled in here. CU measurement
//! is still a later slice, not smuggled in here.
//!
//! Slice 33 (this slice): governance approval persistence —
//! the `approve_governance` instruction and the `GovApprovalRecord`
//! account. One record per (transition, seat): the PDA seeds are
//! `["gov-approval", transition_id, seat]`, and the record stores
//! the seat's approval as a bounded `StoredGovAction` — a fixed-size,
//! lossless encoding of every kernel `GovAction` variant, so the
//! later governance replay can rebuild the exact action from
//! stored records instead of trusting a caller-supplied argument.
//! The builder gates mirror the kernel's `approve_governance` plus
//! the registration preconditions: the kernel state must be sealed,
//! the record must be governance-typed and still `REGISTERED`,
//! the signer must be a seated council member of the pinned
//! profile (`NotCouncilSeat` otherwise), and the action's category
//! must equal the record's type (`GovActionCategoryMismatch`
//! otherwise — the kernel's `WrongGovCategory` gate, moved to
//! approval time so a stored approval can never be for an action
//! its record's type cannot carry). Bounded lists carry program
//! caps, decided here for Carlos's audit: id sets cap 16 (the
//! kernel's `MAX_GROUPS`/`MAX_SIBLINGS` pattern), capability pairs
//! cap 8 (the `MAX_PLAN_CLAIMS` precedent), council seats cap 15
//! (the kernel's `MAX_SEATS`); an over-cap action refuses with
//! `GovActionTooLarge` rather than truncating. Honesty bounds,
//! stated not smoothed: approval records are `init`-only, so a
//! seat's first approval stands — the kernel permits re-approval
//! (its map insert overwrites) before registration, and the
//! program does not yet mirror that; a seat that approved the
//! wrong action cannot re-approve this transition id, and the
//! proposal then fails unanimity (fail-closed; a fresh id is the
//! remedy). A revise-approval instruction is a later slice's
//! decision, not smuggled in here. And the slice-28
//! `commit_governance_from_records` helper still takes its action
//! and approvals as arguments: consuming stored approvals at
//! commit time rides the governance-replay slice, not this one.

extern crate alloc;

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec;
use anchor_lang::prelude::*;
use coordination_kernel::{
    Amount, Config, ConstraintStatus, Env, GovAction, is_gov_type, Kernel, KernelError, Life,
    Manifest, NoEnv,
    Observation, Plan, Policy, Role, SystemState, T_GOV_ADJUSTABLE, T_GOV_CONSTITUTION,
    T_GOV_COUNCIL, T_GOV_REBOOT, MAX_GROUPS, MAX_REPLANS, MAX_SEATS, SCALE,
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

    /// Register governance (§4.1, first lifecycle row, governance
    /// form, slice 32): store a governance-typed transition
    /// manifest as a `TransitionRecord` PDA, gated on the sealed
    /// head, on a governance type, and on the signing payer being
    /// a seated council member of the sealed launch profile.
    /// Refuses an unsealed kernel, any parent that is not exactly
    /// the sealed head, a non-governance type, and a non-seat
    /// proposer; a second record for the same id is refused by
    /// the `init` constraint. Like slice 8, this stores the
    /// manifest — it does not run kernel
    /// `register_governance_transition`: that call additionally
    /// needs the exact action and every seat's approval of it,
    /// and neither is stored yet (approval persistence is a
    /// later slice), so no kernel call is made here and the
    /// record is a stored governance manifest awaiting that
    /// replay, not a registered governance transition.
    #[allow(clippy::too_many_arguments)]
    pub fn register_governance_transition(
        ctx: Context<RegisterGovernanceTransition>,
        transition_id: [u8; 32],
        transition_type: [u8; 32],
        parent_index: u64,
        parent_hash: [u8; 32],
        declared_next_hash: [u8; 32],
        observation_id: [u8; 32],
        expires_at: u64,
    ) -> Result<()> {
        let record = build_governance_transition_record(
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
        emit!(GovernanceTransitionRegistered {
            transition_id,
            parent_index,
            proposer: ctx.accounts.payer.key(),
        });
        Ok(())
    }

    /// Approve governance (§4.1, governance lifecycle row, slice 33):
    /// store one seated council member's approval of one exact
    /// action as a `GovApprovalRecord` PDA, gated on the sealed
    /// state, a governance-typed `REGISTERED` record, the signer's
    /// council seat in the pinned profile, and the action's
    /// category matching the record's type. A second record for
    /// the same seat is refused by the `init` constraint, so a
    /// seat's first approval stands. This stores the approval —
    /// it does not run kernel `approve_governance` (the kernel
    /// object is not persisted); the later governance replay
    /// runs the kernel call from these stored records.
    pub fn approve_governance(
        ctx: Context<ApproveGovernance>,
        transition_id: [u8; 32],
        action: StoredGovAction,
    ) -> Result<()> {
        if ctx.accounts.transition_record.transition_id != transition_id {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        let decoded = action.decode()?;
        let record = build_governance_approval_record(
            &ctx.accounts.kernel_state,
            &ctx.accounts.transition_record,
            ctx.accounts.payer.key(),
            &decoded,
            ctx.bumps.approval_record,
        )?;
        let action_kind = record.action.kind;
        *ctx.accounts.approval_record = record;
        emit!(GovernanceApproved {
            transition_id,
            seat: ctx.accounts.payer.key(),
            action_kind,
        });
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
    /// Slice 31: remaining accounts begin with `prior_bundle_count` prior COMMITTED bundles (head order, self-delimited by each bundle's stored signal/plan counts); the tail is this instruction's own evidence, as before.
    pub fn replay_transition(
        ctx: Context<ReplayTransition>,
        transition_id: [u8; 32],
        observation_id: [u8; 32],
        prior_bundle_count: u32,
    ) -> Result<()> {
        if ctx.accounts.transition_record.transition_id != transition_id
            || ctx.accounts.observation_record.observation_id != observation_id
        {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        let (bundles, tail) = split_prior_bundles(
            classify_remaining_accounts(ctx.remaining_accounts)?,
            prior_bundle_count as usize,
        )?;
        let prior_commits = prior_commits_of(&bundles);
        if !tail.is_empty() {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        replay_transition_from_records(
            &ctx.accounts.kernel_state,
            &mut ctx.accounts.transition_record,
            &ctx.accounts.observation_record,
            &prior_commits,
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
    /// Slice 31: remaining accounts begin with `prior_bundle_count` prior COMMITTED bundles (head order, self-delimited by each bundle's stored signal/plan counts); the tail is this instruction's own evidence, as before.
    pub fn evaluate_transition(
        ctx: Context<EvaluateTransition>,
        transition_id: [u8; 32],
        observation_id: [u8; 32],
        prior_bundle_count: u32,
    ) -> Result<()> {
        if ctx.accounts.transition_record.transition_id != transition_id
            || ctx.accounts.observation_record.observation_id != observation_id
        {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        let (bundles, tail) = split_prior_bundles(
            classify_remaining_accounts(ctx.remaining_accounts)?,
            prior_bundle_count as usize,
        )?;
        let prior_commits = prior_commits_of(&bundles);
        if !tail.is_empty() {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        evaluate_transition_from_records(
            &ctx.accounts.kernel_state,
            &mut ctx.accounts.transition_record,
            &ctx.accounts.observation_record,
            &prior_commits,
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
    /// Slice 31: remaining accounts begin with `prior_bundle_count` prior COMMITTED bundles (head order, self-delimited by each bundle's stored signal/plan counts); the tail is this instruction's own evidence, as before.
    pub fn mark_eligible(
        ctx: Context<MarkEligible>,
        transition_id: [u8; 32],
        observation_id: [u8; 32],
        prior_bundle_count: u32,
    ) -> Result<()> {
        if ctx.accounts.transition_record.transition_id != transition_id
            || ctx.accounts.observation_record.observation_id != observation_id
        {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        let (bundles, tail) = split_prior_bundles(
            classify_remaining_accounts(ctx.remaining_accounts)?,
            prior_bundle_count as usize,
        )?;
        let prior_commits = prior_commits_of(&bundles);
        if !tail.is_empty() {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        mark_eligible_from_records(
            &mut ctx.accounts.kernel_state,
            &mut ctx.accounts.transition_record,
            &ctx.accounts.observation_record,
            &prior_commits,
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
    /// Slice 31: remaining accounts begin with `prior_bundle_count` prior COMMITTED bundles (head order, self-delimited by each bundle's stored signal/plan counts); the tail is this instruction's own evidence, as before.
    #[allow(clippy::too_many_arguments)]
    pub fn submit_signal(
        ctx: Context<SubmitSignal>,
        transition_id: [u8; 32],
        observation_id: [u8; 32],
        round: u64,
        raw: u64,
        support_raw: u64,
        has_support: bool,
        prior_bundle_count: u32,
    ) -> Result<()> {
        if ctx.accounts.transition_record.transition_id != transition_id
            || ctx.accounts.observation_record.observation_id != observation_id
        {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        let (bundles, tail) = split_prior_bundles(
            classify_remaining_accounts(ctx.remaining_accounts)?,
            prior_bundle_count as usize,
        )?;
        let prior_commits = prior_commits_of(&bundles);
        let mut priors: alloc::vec::Vec<SignalRecord> = alloc::vec::Vec::new();
        for item in tail {
            match item {
                ParsedAccount::Signal(sig) => priors.push(sig),
                _ => return Err(error!(ProgramV04Error::RecordMismatch)),
            }
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
            &prior_commits,
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
    /// Slice 31: remaining accounts begin with `prior_bundle_count` prior COMMITTED bundles (head order, self-delimited by each bundle's stored signal/plan counts); the tail is this instruction's own evidence, as before.
    pub fn select_candidate(
        ctx: Context<SelectCandidate>,
        transition_id: [u8; 32],
        observation_id: [u8; 32],
        prior_bundle_count: u32,
    ) -> Result<()> {
        if ctx.accounts.transition_record.transition_id != transition_id
            || ctx.accounts.observation_record.observation_id != observation_id
        {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        let (bundles, tail) = split_prior_bundles(
            classify_remaining_accounts(ctx.remaining_accounts)?,
            prior_bundle_count as usize,
        )?;
        let prior_commits = prior_commits_of(&bundles);
        let mut priors: alloc::vec::Vec<SignalRecord> = alloc::vec::Vec::new();
        for item in tail {
            match item {
                ParsedAccount::Signal(sig) => priors.push(sig),
                _ => return Err(error!(ProgramV04Error::RecordMismatch)),
            }
        }
        select_candidate_from_records(
            &ctx.accounts.kernel_state,
            &mut ctx.accounts.transition_record,
            &ctx.accounts.observation_record,
            &priors,
            &prior_commits,
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
    /// Slice 31: remaining accounts begin with `prior_bundle_count` prior COMMITTED bundles (head order, self-delimited by each bundle's stored signal/plan counts); the tail is this instruction's own evidence, as before.
    pub fn select_candidate_with_siblings(
        ctx: Context<SelectCandidateWithSiblings>,
        transition_id: [u8; 32],
        observation_id: [u8; 32],
        prior_bundle_count: u32,
    ) -> Result<()> {
        if ctx.accounts.transition_record.transition_id != transition_id
            || ctx.accounts.observation_record.observation_id != observation_id
        {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        let (bundles, tail) = split_prior_bundles(
            classify_remaining_accounts(ctx.remaining_accounts)?,
            prior_bundle_count as usize,
        )?;
        let prior_commits = prior_commits_of(&bundles);
        let mut sibling_transitions: alloc::vec::Vec<TransitionRecord> = alloc::vec::Vec::new();
        let mut sibling_observations: alloc::vec::Vec<ObservationRecord> = alloc::vec::Vec::new();
        let mut all_signals: alloc::vec::Vec<SignalRecord> = alloc::vec::Vec::new();
        for item in tail {
            match item {
                ParsedAccount::Transition(tr) => sibling_transitions.push(tr),
                ParsedAccount::Observation(obs) => sibling_observations.push(obs),
                ParsedAccount::Signal(sig) => all_signals.push(sig),
                ParsedAccount::Plan(_) => {
                    return Err(error!(ProgramV04Error::RecordMismatch))
                }
            }
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
            &prior_commits,
)?;
        emit!(TransitionSelected { transition_id, stage: TransitionRecord::SELECTED });
        Ok(())
    }

    /// Open planning (§4.1, sixth replay row, v0.4 form, slices 20
    /// and 22): run kernel `open_planning` on a `SELECTED` record
    /// pair and advance the transition record's stage to
    /// `PLANNING`, persisting the planning deadline the kernel
    /// sets (`clock + planning_window`). Prior signals for the
    /// transition arrive as remaining accounts and must number
    /// exactly the record's `signal_count` — the slice-16 anchor —
    /// because the helper re-proves the selection chain before
    /// opening planning; a stored `SELECTED` label alone opens
    /// nothing. The payer is a fee payer and, for the Selected
    /// path, the kernel's planning caller: the kernel admits
    /// anyone there, so no proposer or seat gate applies on that
    /// path. A `BLOCKED` record (slice 22) re-opens planning only
    /// on the kernel's re-plan rule — caller is the proposer or a
    /// launch-council seat, and fewer than `MAX_REPLANS` re-plans
    /// taken — enforced by this helper from the stored record
    /// (see the helper's honesty bound: the replay proves
    /// selection, not Blocked-ness, until reconcile/validate land).
    /// Slice 31: remaining accounts begin with `prior_bundle_count` prior COMMITTED bundles (head order, self-delimited by each bundle's stored signal/plan counts); the tail is this instruction's own evidence, as before.
    pub fn open_planning(
        ctx: Context<OpenPlanning>,
        transition_id: [u8; 32],
        observation_id: [u8; 32],
        prior_bundle_count: u32,
    ) -> Result<()> {
        if ctx.accounts.transition_record.transition_id != transition_id
            || ctx.accounts.observation_record.observation_id != observation_id
        {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        let (bundles, tail) = split_prior_bundles(
            classify_remaining_accounts(ctx.remaining_accounts)?,
            prior_bundle_count as usize,
        )?;
        let prior_commits = prior_commits_of(&bundles);
        let mut priors: alloc::vec::Vec<SignalRecord> = alloc::vec::Vec::new();
        for item in tail {
            match item {
                ParsedAccount::Signal(sig) => priors.push(sig),
                _ => return Err(error!(ProgramV04Error::RecordMismatch)),
            }
        }
        let life = open_planning_from_records(
            &ctx.accounts.kernel_state,
            &mut ctx.accounts.transition_record,
            &ctx.accounts.observation_record,
            &priors,
            resolve_agent_id(ctx.accounts.payer.key()),
            &prior_commits,
)?;
        debug_assert_eq!(life, Life::Planning);
        emit!(TransitionPlanningOpened {
            transition_id,
            stage: TransitionRecord::PLANNING,
            planning_deadline: ctx.accounts.transition_record.planning_deadline,
        });
        Ok(())
    }

    /// Submit a plan (§4.1, second planning row, v0.4 form): run
    /// kernel `submit_plan` on a `PLANNING` record pair and persist
    /// the new plan as a `PlanRecord` PDA, one per (transition,
    /// agent) — a second plan from the same agent fails at account
    /// creation before the body runs, and the kernel's `Duplicate`
    /// arm is the second guard in replay. The transition stage does
    /// not advance, and `plan_count` increments on success only.
    /// Remaining accounts split deterministically: the first
    /// `signal_count` are the transition's prior `SignalRecord`s
    /// (the selection-chain replay needs them), and the rest are
    /// its prior `PlanRecord`s, which must number exactly
    /// `plan_count` — the slice-21 anchor. The payer is a fee payer
    /// and, by the identity mapping, the plan's agent: the kernel's
    /// agent gate (registered, active, in the proposer's groups)
    /// applies here, unlike open_planning's anyone rule.
    /// Slice 31: remaining accounts begin with `prior_bundle_count` prior COMMITTED bundles (head order, self-delimited by each bundle's stored signal/plan counts); the tail is this instruction's own evidence, as before.
    pub fn submit_plan(
        ctx: Context<SubmitPlan>,
        transition_id: [u8; 32],
        observation_id: [u8; 32],
        claims: Vec<PlanClaimArg>,
        expires_at: u64,
        prior_bundle_count: u32,
    ) -> Result<()> {
        if ctx.accounts.transition_record.transition_id != transition_id
            || ctx.accounts.observation_record.observation_id != observation_id
        {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        let (bundles, tail) = split_prior_bundles(
            classify_remaining_accounts(ctx.remaining_accounts)?,
            prior_bundle_count as usize,
        )?;
        let prior_commits = prior_commits_of(&bundles);
        let signal_count = ctx.accounts.transition_record.signal_count as usize;
        if tail.len() < signal_count {
            return Err(error!(ProgramV04Error::PlanFailed));
        }
        let mut tail_iter = tail.into_iter();
        let mut priors: alloc::vec::Vec<SignalRecord> = alloc::vec::Vec::new();
        for _ in 0..signal_count {
            match tail_iter.next() {
                Some(ParsedAccount::Signal(sig)) => priors.push(sig),
                _ => return Err(error!(ProgramV04Error::RecordMismatch)),
            }
        }
        let mut prior_plans: alloc::vec::Vec<PlanRecord> = alloc::vec::Vec::new();
        for item in tail_iter {
            match item {
                ParsedAccount::Plan(plan) => prior_plans.push(plan),
                _ => return Err(error!(ProgramV04Error::RecordMismatch)),
            }
        }
        let plan = submit_plan_from_records(
            &ctx.accounts.kernel_state,
            &mut ctx.accounts.transition_record,
            &ctx.accounts.observation_record,
            &priors,
            &prior_plans,
            &claims,
            expires_at,
            resolve_agent_id(ctx.accounts.payer.key()),
            ctx.bumps.plan_record,
            &prior_commits,
)?;
        *ctx.accounts.plan_record = plan;
        emit!(PlanSubmitted {
            transition_id,
            agent: ctx.accounts.payer.key(),
            expires_at,
            plan_count: ctx.accounts.transition_record.plan_count,
        });
        Ok(())
    }

    /// Reconcile plans (§4.1, third planning row, v0.4 form): run
    /// kernel `reconcile_plans` on a `PLANNING` record pair and
    /// advance the transition record's stage to `RECONCILING`,
    /// persisting the kernel's compatibility verdict. Remaining
    /// accounts split deterministically, as in `submit_plan`: the
    /// first `signal_count` are the transition's prior
    /// `SignalRecord`s (the selection-chain replay needs them),
    /// and the rest are its prior `PlanRecord`s, which must number
    /// exactly `plan_count` — the slice-21 anchor. The payer is a
    /// fee payer only: kernel `reconcile_plans` takes no caller,
    /// so no proposer, seat, or agent gate applies here, and the
    /// instruction refuses nothing on caller identity. A reconcile
    /// that finds no fitting plan set still advances the stage —
    /// the verdict is `compatible: false`, not a refusal — because
    /// the kernel records incompatibility as a state, and the
    /// later validate slice judges it; only a window still open,
    /// an unreproducible chain, or an incomplete prior set
    /// refuses, and then the record stands exactly as it stood.
    /// Slice 31: remaining accounts begin with `prior_bundle_count` prior COMMITTED bundles (head order, self-delimited by each bundle's stored signal/plan counts); the tail is this instruction's own evidence, as before.
    pub fn reconcile_plans(
        ctx: Context<ReconcilePlans>,
        transition_id: [u8; 32],
        observation_id: [u8; 32],
        prior_bundle_count: u32,
    ) -> Result<()> {
        if ctx.accounts.transition_record.transition_id != transition_id
            || ctx.accounts.observation_record.observation_id != observation_id
        {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        let (bundles, tail) = split_prior_bundles(
            classify_remaining_accounts(ctx.remaining_accounts)?,
            prior_bundle_count as usize,
        )?;
        let prior_commits = prior_commits_of(&bundles);
        let signal_count = ctx.accounts.transition_record.signal_count as usize;
        if tail.len() < signal_count {
            return Err(error!(ProgramV04Error::ReconcileFailed));
        }
        let mut tail_iter = tail.into_iter();
        let mut priors: alloc::vec::Vec<SignalRecord> = alloc::vec::Vec::new();
        for _ in 0..signal_count {
            match tail_iter.next() {
                Some(ParsedAccount::Signal(sig)) => priors.push(sig),
                _ => return Err(error!(ProgramV04Error::RecordMismatch)),
            }
        }
        let mut prior_plans: alloc::vec::Vec<PlanRecord> = alloc::vec::Vec::new();
        for item in tail_iter {
            match item {
                ParsedAccount::Plan(plan) => prior_plans.push(plan),
                _ => return Err(error!(ProgramV04Error::RecordMismatch)),
            }
        }
        let life = reconcile_plans_from_records(
            &ctx.accounts.kernel_state,
            &mut ctx.accounts.transition_record,
            &ctx.accounts.observation_record,
            &priors,
            &prior_plans,
            &prior_commits,
)?;
        debug_assert_eq!(life, Life::Reconciling);
        emit!(TransitionReconciled {
            transition_id,
            stage: TransitionRecord::RECONCILING,
            compatible: ctx.accounts.transition_record.compatible,
            plan_count: ctx.accounts.transition_record.plan_count,
        });
        Ok(())
    }

    /// Validate a reconciled transition (slice 24): `RECONCILING`
    /// (7) -> `VALIDATED` (8), or `RECONCILING` -> `BLOCKED` (6)
    /// when the kernel's predicate fails, or a refusal that leaves
    /// a `BLOCKED` record standing when the kernel reports it
    /// still blocked. Remaining accounts split exactly as in
    /// `reconcile_plans`: the first `signal_count` are the prior
    /// `SignalRecord`s and the rest are the prior `PlanRecord`s,
    /// which must number exactly `plan_count`. The payer is a fee
    /// payer only: kernel `validate_transition` takes no caller,
    /// so no proposer, seat, or agent gate applies here. A
    /// validate that finds the predicate failing still advances
    /// the stage — to `BLOCKED`, not a refusal — because the
    /// kernel records blocking as a state and the re-plan path
    /// (slice 22) opens from it; only an unreproducible chain,
    /// an incomplete prior set, a stored verdict the supplied
    /// set does not reproduce, or a still-blocked record
    /// refuses, and then the record stands exactly as it stood.
    /// Slice 31: remaining accounts begin with `prior_bundle_count` prior COMMITTED bundles (head order, self-delimited by each bundle's stored signal/plan counts); the tail is this instruction's own evidence, as before.
    pub fn validate_transition(
        ctx: Context<ValidateTransition>,
        transition_id: [u8; 32],
        observation_id: [u8; 32],
        prior_bundle_count: u32,
    ) -> Result<()> {
        if ctx.accounts.transition_record.transition_id != transition_id
            || ctx.accounts.observation_record.observation_id != observation_id
        {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        let (bundles, tail) = split_prior_bundles(
            classify_remaining_accounts(ctx.remaining_accounts)?,
            prior_bundle_count as usize,
        )?;
        let prior_commits = prior_commits_of(&bundles);
        let signal_count = ctx.accounts.transition_record.signal_count as usize;
        if tail.len() < signal_count {
            return Err(error!(ProgramV04Error::ValidateFailed));
        }
        let mut tail_iter = tail.into_iter();
        let mut priors: alloc::vec::Vec<SignalRecord> = alloc::vec::Vec::new();
        for _ in 0..signal_count {
            match tail_iter.next() {
                Some(ParsedAccount::Signal(sig)) => priors.push(sig),
                _ => return Err(error!(ProgramV04Error::RecordMismatch)),
            }
        }
        let mut prior_plans: alloc::vec::Vec<PlanRecord> = alloc::vec::Vec::new();
        for item in tail_iter {
            match item {
                ParsedAccount::Plan(plan) => prior_plans.push(plan),
                _ => return Err(error!(ProgramV04Error::RecordMismatch)),
            }
        }
        let life = validate_transition_from_records(
            &ctx.accounts.kernel_state,
            &mut ctx.accounts.transition_record,
            &ctx.accounts.observation_record,
            &priors,
            &prior_plans,
            &prior_commits,
)?;
        debug_assert!(life == Life::Validated || life == Life::Blocked);
        emit!(TransitionValidated {
            transition_id,
            stage: ctx.accounts.transition_record.stage,
            compatible: ctx.accounts.transition_record.compatible,
        });
        Ok(())
    }

    /// Declare a validated transition (slice 25): `VALIDATED`
    /// (8) -> `DECLARED` (9), or a refusal that leaves the record
    /// standing `VALIDATED`. Remaining accounts split exactly as
    /// in `validate_transition`: the first `signal_count` are
    /// the prior `SignalRecord`s and the rest are the prior
    /// `PlanRecord`s, which must number exactly `plan_count`.
    /// The payer is a fee payer only: kernel `declare_commit`
    /// is permissionless, so no proposer, seat, or agent gate
    /// applies here. Declare never writes `BLOCKED` and never
    /// moves the sealed head: a replay validate that yields
    /// `Blocked`, or a full predicate that fails at declare
    /// time, refuses — the record stands `VALIDATED` for a
    /// later declare at a clock where the predicate holds, or
    /// for the exception path to judge it.
    /// Slice 31: remaining accounts begin with `prior_bundle_count` prior COMMITTED bundles (head order, self-delimited by each bundle's stored signal/plan counts); the tail is this instruction's own evidence, as before.
    pub fn declare_transition(
        ctx: Context<DeclareTransition>,
        transition_id: [u8; 32],
        observation_id: [u8; 32],
        prior_bundle_count: u32,
    ) -> Result<()> {
        if ctx.accounts.transition_record.transition_id != transition_id
            || ctx.accounts.observation_record.observation_id != observation_id
        {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        let (bundles, tail) = split_prior_bundles(
            classify_remaining_accounts(ctx.remaining_accounts)?,
            prior_bundle_count as usize,
        )?;
        let prior_commits = prior_commits_of(&bundles);
        let signal_count = ctx.accounts.transition_record.signal_count as usize;
        if tail.len() < signal_count {
            return Err(error!(ProgramV04Error::DeclareFailed));
        }
        let mut tail_iter = tail.into_iter();
        let mut priors: alloc::vec::Vec<SignalRecord> = alloc::vec::Vec::new();
        for _ in 0..signal_count {
            match tail_iter.next() {
                Some(ParsedAccount::Signal(sig)) => priors.push(sig),
                _ => return Err(error!(ProgramV04Error::RecordMismatch)),
            }
        }
        let mut prior_plans: alloc::vec::Vec<PlanRecord> = alloc::vec::Vec::new();
        for item in tail_iter {
            match item {
                ParsedAccount::Plan(plan) => prior_plans.push(plan),
                _ => return Err(error!(ProgramV04Error::RecordMismatch)),
            }
        }
        let life = declare_transition_from_records(
            &ctx.accounts.kernel_state,
            &mut ctx.accounts.transition_record,
            &ctx.accounts.observation_record,
            &priors,
            &prior_plans,
            &prior_commits,
)?;
        debug_assert!(life == Life::Committable);
        emit!(TransitionDeclared {
            transition_id,
            stage: ctx.accounts.transition_record.stage,
        });
        Ok(())
    }

    /// Commit a declared transition (slice 26): `DECLARED`
    /// (9) -> `COMMITTED` (10), and the sealed head moves to
    /// the kernel's returned `SystemState` — or a refusal that
    /// leaves the record standing `DECLARED` and the head
    /// unmoved. Remaining accounts split exactly as in
    /// `declare_transition`: the first `signal_count` are
    /// the prior `SignalRecord`s and the rest are the prior
    /// `PlanRecord`s, which must number exactly `plan_count`.
    /// The payer is a fee payer only: kernel `commit` is
    /// permissionless, so no proposer, seat, or agent gate
    /// applies here. A governance-typed record refuses at
    /// the gate with `GovernanceCommitRequired` — its commit
    /// path is the slice-28 `commit_governance_from_records` helper
    /// (no instruction yet — see the slice-28 module note).
    /// Slice 31: remaining accounts begin with `prior_bundle_count` prior COMMITTED bundles (head order, self-delimited by each bundle's stored signal/plan counts); the tail is this instruction's own evidence, as before.
    pub fn commit_transition(
        ctx: Context<CommitTransition>,
        transition_id: [u8; 32],
        observation_id: [u8; 32],
        prior_bundle_count: u32,
    ) -> Result<()> {
        if ctx.accounts.transition_record.transition_id != transition_id
            || ctx.accounts.observation_record.observation_id != observation_id
        {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        let (bundles, tail) = split_prior_bundles(
            classify_remaining_accounts(ctx.remaining_accounts)?,
            prior_bundle_count as usize,
        )?;
        let prior_commits = prior_commits_of(&bundles);
        let signal_count = ctx.accounts.transition_record.signal_count as usize;
        if tail.len() < signal_count {
            return Err(error!(ProgramV04Error::CommitFailed));
        }
        let mut tail_iter = tail.into_iter();
        let mut priors: alloc::vec::Vec<SignalRecord> = alloc::vec::Vec::new();
        for _ in 0..signal_count {
            match tail_iter.next() {
                Some(ParsedAccount::Signal(sig)) => priors.push(sig),
                _ => return Err(error!(ProgramV04Error::RecordMismatch)),
            }
        }
        let mut prior_plans: alloc::vec::Vec<PlanRecord> = alloc::vec::Vec::new();
        for item in tail_iter {
            match item {
                ParsedAccount::Plan(plan) => prior_plans.push(plan),
                _ => return Err(error!(ProgramV04Error::RecordMismatch)),
            }
        }
        let life = commit_transition_from_records(
            &mut ctx.accounts.kernel_state,
            &mut ctx.accounts.transition_record,
            &ctx.accounts.observation_record,
            &priors,
            &prior_plans,
            &prior_commits,
)?;
        debug_assert!(life == Life::Committed);
        emit!(TransitionCommitted {
            transition_id,
            stage: ctx.accounts.transition_record.stage,
            head_index: ctx.accounts.kernel_state.head_index,
            head_hash: ctx.accounts.kernel_state.head_hash,
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
#[instruction(transition_id: [u8; 32], observation_id: [u8; 32], claims: Vec<PlanClaimArg>, expires_at: u64)]
pub struct SubmitPlan<'info> {
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
        space = PlanRecord::SPACE,
        seeds = [b"plan", transition_id.as_ref(), payer.key().as_ref()],
        bump,
    )]
    pub plan_record: Account<'info, PlanRecord>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(transition_id: [u8; 32], observation_id: [u8; 32])]
pub struct ValidateTransition<'info> {
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
pub struct DeclareTransition<'info> {
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
pub struct CommitTransition<'info> {
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
#[instruction(transition_id: [u8; 32], observation_id: [u8; 32])]
pub struct ReconcilePlans<'info> {
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
#[instruction(transition_id: [u8; 32])]
pub struct RegisterGovernanceTransition<'info> {
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
#[instruction(transition_id: [u8; 32])]
pub struct ApproveGovernance<'info> {
    #[account(
        seeds = [b"kernel-state"],
        bump = kernel_state.bump,
    )]
    pub kernel_state: Account<'info, KernelState>,
    #[account(
        seeds = [b"transition", transition_id.as_ref()],
        bump = transition_record.bump,
    )]
    pub transition_record: Account<'info, TransitionRecord>,
    #[account(
        init,
        payer = payer,
        space = GovApprovalRecord::SPACE,
        seeds = [b"gov-approval", transition_id.as_ref(), payer.key().as_ref()],
        bump,
    )]
    pub approval_record: Account<'info, GovApprovalRecord>,
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
    /// Cumulative committed claims per launch resource (slice 27):
    /// index 0 = `RES_TREASURY_TRANCHE`, 1 = `RES_LENDING_BOOK`,
    /// 2 = `RES_EXIT_WINDOW`, in kernel `Amount` raw units.
    /// Written by `commit_transition` on success only, together
    /// with the stage advance and the head move; genesis: zeros.
    pub committed_claims: [u64; 3],
}

impl KernelState {
    /// 8 (discriminator) + 1 + 8 + 32 + 8 + 8 + 32 + 8 + 1 + 24.
    pub const SPACE: usize = 130;
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
    /// (slice 13 advances it), 4 = Selected (slice 16 advances it),
    /// 5 = Planning (slice 20 advances it), 6 = Blocked (slice 24
    /// writes it when validate finds the predicate failing; slice
    /// 22 opens the re-plan from it), 7 = Reconciling (slice 23
    /// advances it, with `compatible`), 8 = Validated (slice 24
    /// advances it when the kernel's predicate passes).
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
    /// Number of `PlanRecord`s created for this transition (slice 21:
    /// the reconcile completeness anchor). Written by `submit_plan`
    /// on success only — one per created record — so the count and
    /// the plan set cannot diverge on-chain.
    pub plan_count: u64,
    /// Number of Blocked re-plans already taken on this transition
    /// (slice 22: the kernel's `MAX_REPLANS` cap, persisted here
    /// because the rebuilt replay kernel starts every replay at
    /// zero — the program, not the replay, enforces the cap).
    /// Written by `open_planning` on a Blocked-path success only.
    pub replans: u32,
    /// Kernel compatibility verdict from `reconcile_plans` (slice 23:
    /// a non-empty plan set whose per-resource sums fit the sealed
    /// capacity stocks). False until reconcile runs; written by
    /// `reconcile_plans` on success only, alongside the stage
    /// advance — a reconcile that finds no fitting set still
    /// advances the stage, with this false, rather than refusing.
    pub compatible: bool,
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
    /// Stage value written by `validate_transition` (slice 24)
    /// when a transition blocks (slice 22 opens the re-plan from
    /// it; the slice-22 fixture label is now program-produced).
    pub const BLOCKED: u8 = 6;
    /// Stage value written by `reconcile_plans` (slice 23).
    pub const RECONCILING: u8 = 7;
    /// Stage value written by `validate_transition` (slice 24).
    pub const VALIDATED: u8 = 8;
    /// Stage value written by `declare_transition` (slice 25).
    pub const DECLARED: u8 = 9;
    /// Stage value written by `commit_transition` (slice 26).
    pub const COMMITTED: u8 = 10;
    /// 8 (discriminator) + 32 + 32 + 8 + 32 + 32 + 32 + 8 + 32 + 1 + 8 + 8 + 8 + 8 + 8 + 4 + 1 + 1.
    pub const SPACE: usize = 263;
}

/// The ungated `TransitionRecord` constructor both builders below
/// share: the gates live in the builders, never here, so a record
/// built through either path carries exactly the same fields.
#[allow(clippy::too_many_arguments)]
fn new_transition_record(
    transition_id: [u8; 32],
    transition_type: [u8; 32],
    parent_index: u64,
    parent_hash: [u8; 32],
    declared_next_hash: [u8; 32],
    observation_id: [u8; 32],
    expires_at: u64,
    proposer: Pubkey,
    bump: u8,
) -> TransitionRecord {
    TransitionRecord {
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
        plan_count: 0,
        replans: 0,
        compatible: false,
        bump,
    }
}

/// Build a `TransitionRecord` against a `KernelState`, enforcing the
/// sealed-head lineage gate. Pure: a refused record leaves the state
/// untouched, and the caller (instruction or test) owns persistence.
/// Slice 32: governance types refuse here — they register through
/// `build_governance_transition_record`, which carries the seat gate
/// this builder does not — exactly as kernel `register_transition`
/// refuses governance types.
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
    if is_gov_type(&transition_type) {
        return Err(error!(ProgramV04Error::GovernanceRegisterRequired));
    }
    Ok(new_transition_record(
        transition_id,
        transition_type,
        parent_index,
        parent_hash,
        declared_next_hash,
        observation_id,
        expires_at,
        proposer,
        bump,
    ))
}

/// Build a governance-typed `TransitionRecord` against a
/// `KernelState` (slice 32): the sealed-head lineage gate as in
/// `build_transition_record`, plus the governance gates — the type
/// must be a governance type, and the proposer must be a seated
/// council member of the sealed launch profile (active, and on the
/// sealed kernel's council list), judged against the pinned profile
/// exactly as `resolve_proposer` judges its gate. Pure, as above:
/// a refused record leaves the state untouched. The action and the
/// approvals are deliberately not built here — they are not stored
/// yet, and this builder stores the manifest only.
#[allow(clippy::too_many_arguments)]
pub fn build_governance_transition_record(
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
    if !is_gov_type(&transition_type) {
        return Err(error!(ProgramV04Error::GovernanceTypeRequired));
    }
    let agent_id = resolve_agent_id(proposer);
    let k = sealed_launch_kernel().map_err(|_| error!(ProgramV04Error::KernelSeal))?;
    match k.agent(&agent_id) {
        Some(profile) if profile.active && k.is_seated(&agent_id) => {}
        _ => return Err(error!(ProgramV04Error::NotCouncilSeat)),
    }
    Ok(new_transition_record(
        transition_id,
        transition_type,
        parent_index,
        parent_hash,
        declared_next_hash,
        observation_id,
        expires_at,
        proposer,
        bump,
    ))
}

/// Program caps for the bounded `StoredGovAction` lists (slice 33):
/// id sets cap at the kernel's `MAX_GROUPS`, capability pairs at
/// the `MAX_PLAN_CLAIMS` precedent, council seats at the kernel's
/// `MAX_SEATS`. An action over any cap refuses with
/// `GovActionTooLarge` — it is never truncated, because a truncated
/// action would be a different action than the one approved.
pub const GOV_ACTION_MAX_SET: usize = MAX_GROUPS;
pub const GOV_ACTION_MAX_CAP_PAIRS: usize = 8;
pub const GOV_ACTION_MAX_COUNCIL: usize = MAX_SEATS as usize;

/// A fixed-size, lossless encoding of a kernel `GovAction` (slice 33),
/// stored on a `GovApprovalRecord` so the later governance replay
/// rebuilds the exact approved action from chain state. Every variant
/// occupies the same layout: `kind` selects the variant (0..=14 in
/// kernel declaration order) and only the fields that variant uses
/// are meaningful — the rest are zeros. `Policy` is stored in full
/// (its ten fields); unbounded kernel collections are stored as
/// fixed arrays plus a count, bounded by the caps above. `role` uses
/// the kernel's declaration order (Guardian 0, Observer 1, Executor 2,
/// Settler 3, OutcomeRecorder 4). Encoding and decoding are exact
/// inverses on every in-cap action; decoding a corrupt stored value
/// (unknown kind, over-cap count) refuses with `GovActionDecode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, AnchorSerialize, AnchorDeserialize)]
pub struct StoredGovAction {
    /// Variant discriminant, 0..=14 in kernel declaration order.
    pub kind: u8,
    /// Target transition type (SetPolicy, SetRequired*, SetGroupMultiplier).
    pub ttype: [u8; 32],
    /// Target agent (Grant/RevokeRole, SetAgentCapability, SetAgentActive, RegisterAgent).
    pub agent: [u8; 32],
    /// Role code (Grant/RevokeRole only).
    pub role: u8,
    /// Target resource id (SetCapacity).
    pub resource: u16,
    /// Target group id (SetGroupMultiplier).
    pub group: u16,
    /// Target capability id (SetAgentCapability).
    pub cap_id: [u8; 32],
    /// Amount raw units (SetCapacity, SetGroupMultiplier, SetMaxInfluenceShare, RegisterAgent).
    pub amount_raw: u64,
    /// Boolean payload (SetAgentCapability held, SetAgentActive active).
    pub flag: bool,
    /// Seat count (SetCouncilSeats).
    pub seats_u32: u32,
    /// Policy quorum (SetPolicy).
    pub policy_quorum_raw: u64,
    /// Policy confidence threshold (SetPolicy).
    pub policy_confidence_raw: u64,
    /// Policy minimum observation confidence (SetPolicy).
    pub policy_min_obs_raw: u64,
    /// Policy objective requirement (SetPolicy).
    pub policy_require_objectives: bool,
    /// Policy consensus window (SetPolicy).
    pub policy_consensus_window: u64,
    /// Policy planning window (SetPolicy).
    pub policy_planning_window: u64,
    /// Policy execution window (SetPolicy).
    pub policy_execution_window: u64,
    /// Policy max plans (SetPolicy).
    pub policy_max_plans: u32,
    /// Policy settlement threshold (SetPolicy).
    pub policy_settlement_threshold: u32,
    /// Policy max rounds (SetPolicy).
    pub policy_max_rounds: u32,
    /// Id list (SetRequiredConstraints, SetRequiredObjectives).
    pub set_ids: [[u8; 32]; GOV_ACTION_MAX_SET],
    /// Length of `set_ids`.
    pub set_count: u8,
    /// Group list (RegisterAgent).
    pub group_ids: [u16; GOV_ACTION_MAX_SET],
    /// Length of `group_ids`.
    pub group_count: u8,
    /// Capability list (SetRequiredCapabilities).
    pub cap_ids: [[u8; 32]; GOV_ACTION_MAX_CAP_PAIRS],
    /// Capability amounts, parallel to `cap_ids`.
    pub cap_amounts: [u64; GOV_ACTION_MAX_CAP_PAIRS],
    /// Length of `cap_ids`/`cap_amounts`.
    pub cap_count: u8,
    /// Council seat list (InstallCouncil).
    pub council_seats: [[u8; 32]; GOV_ACTION_MAX_COUNCIL],
    /// Length of `council_seats`.
    pub council_count: u8,
}

fn role_code(role: Role) -> u8 {
    match role {
        Role::Guardian => 0,
        Role::Observer => 1,
        Role::Executor => 2,
        Role::Settler => 3,
        Role::OutcomeRecorder => 4,
    }
}

fn role_of(code: u8) -> Result<Role> {
    match code {
        0 => Ok(Role::Guardian),
        1 => Ok(Role::Observer),
        2 => Ok(Role::Executor),
        3 => Ok(Role::Settler),
        4 => Ok(Role::OutcomeRecorder),
        _ => Err(error!(ProgramV04Error::GovActionDecode)),
    }
}

impl StoredGovAction {
    /// Encode a kernel action into fixed-size storage. Over-cap
    /// collections refuse with `GovActionTooLarge`.
    pub fn encode(action: &GovAction) -> Result<Self> {
        let mut s = StoredGovAction::default();
        match action {
            GovAction::GrantRole { who, role } => {
                s.kind = 0;
                s.agent = *who;
                s.role = role_code(*role);
            }
            GovAction::RevokeRole { who, role } => {
                s.kind = 1;
                s.agent = *who;
                s.role = role_code(*role);
            }
            GovAction::SetPolicy { ttype, policy } => {
                s.kind = 2;
                s.ttype = *ttype;
                s.policy_quorum_raw = policy.quorum_required.raw();
                s.policy_confidence_raw = policy.confidence_threshold.raw();
                s.policy_min_obs_raw = policy.min_obs_confidence.raw();
                s.policy_require_objectives = policy.require_objectives;
                s.policy_consensus_window = policy.consensus_window;
                s.policy_planning_window = policy.planning_window;
                s.policy_execution_window = policy.execution_window;
                s.policy_max_plans = policy.max_plans;
                s.policy_settlement_threshold = policy.settlement_threshold;
                s.policy_max_rounds = policy.max_rounds;
            }
            GovAction::SetRequiredConstraints { ttype, set } => {
                s.kind = 3;
                s.ttype = *ttype;
                if set.len() > GOV_ACTION_MAX_SET {
                    return Err(error!(ProgramV04Error::GovActionTooLarge));
                }
                s.set_count = set.len() as u8;
                for (i, id) in set.iter().enumerate() {
                    s.set_ids[i] = *id;
                }
            }
            GovAction::SetRequiredObjectives { ttype, set } => {
                s.kind = 4;
                s.ttype = *ttype;
                if set.len() > GOV_ACTION_MAX_SET {
                    return Err(error!(ProgramV04Error::GovActionTooLarge));
                }
                s.set_count = set.len() as u8;
                for (i, id) in set.iter().enumerate() {
                    s.set_ids[i] = *id;
                }
            }
            GovAction::SetRequiredCapabilities { ttype, reqs } => {
                s.kind = 5;
                s.ttype = *ttype;
                if reqs.len() > GOV_ACTION_MAX_CAP_PAIRS {
                    return Err(error!(ProgramV04Error::GovActionTooLarge));
                }
                s.cap_count = reqs.len() as u8;
                for (i, (id, amount)) in reqs.iter().enumerate() {
                    s.cap_ids[i] = *id;
                    s.cap_amounts[i] = amount.raw();
                }
            }
            GovAction::SetAgentCapability { agent, cap, held } => {
                s.kind = 6;
                s.agent = *agent;
                s.cap_id = *cap;
                s.flag = *held;
            }
            GovAction::SetCapacity { resource, capacity } => {
                s.kind = 7;
                s.resource = *resource;
                s.amount_raw = capacity.raw();
            }
            GovAction::SetGroupMultiplier { ttype, group, mult } => {
                s.kind = 8;
                s.ttype = *ttype;
                s.group = *group;
                s.amount_raw = mult.raw();
            }
            GovAction::SetMaxInfluenceShare { share } => {
                s.kind = 9;
                s.amount_raw = share.raw();
            }
            GovAction::RegisterAgent { agent, groups, base_influence } => {
                s.kind = 10;
                s.agent = *agent;
                if groups.len() > GOV_ACTION_MAX_SET {
                    return Err(error!(ProgramV04Error::GovActionTooLarge));
                }
                s.group_count = groups.len() as u8;
                for (i, g) in groups.iter().enumerate() {
                    s.group_ids[i] = *g;
                }
                s.amount_raw = base_influence.raw();
            }
            GovAction::SetAgentActive { agent, active } => {
                s.kind = 11;
                s.agent = *agent;
                s.flag = *active;
            }
            GovAction::SetCouncilSeats { seats } => {
                s.kind = 12;
                s.seats_u32 = *seats;
            }
            GovAction::InstallCouncil { seats } => {
                s.kind = 13;
                if seats.len() > GOV_ACTION_MAX_COUNCIL {
                    return Err(error!(ProgramV04Error::GovActionTooLarge));
                }
                s.council_count = seats.len() as u8;
                for (i, seat) in seats.iter().enumerate() {
                    s.council_seats[i] = *seat;
                }
            }
            GovAction::Reboot => {
                s.kind = 14;
            }
        }
        Ok(s)
    }

    /// Decode stored bytes back into the exact kernel action.
    /// A corrupt stored value refuses with `GovActionDecode`.
    pub fn decode(&self) -> Result<GovAction> {
        if (self.set_count as usize) > GOV_ACTION_MAX_SET
            || (self.group_count as usize) > GOV_ACTION_MAX_SET
            || (self.cap_count as usize) > GOV_ACTION_MAX_CAP_PAIRS
            || (self.council_count as usize) > GOV_ACTION_MAX_COUNCIL
        {
            return Err(error!(ProgramV04Error::GovActionDecode));
        }
        Ok(match self.kind {
            0 => GovAction::GrantRole { who: self.agent, role: role_of(self.role)? },
            1 => GovAction::RevokeRole { who: self.agent, role: role_of(self.role)? },
            2 => GovAction::SetPolicy {
                ttype: self.ttype,
                policy: Policy {
                    quorum_required: Amount::from_raw(self.policy_quorum_raw),
                    confidence_threshold: Amount::from_raw(self.policy_confidence_raw),
                    min_obs_confidence: Amount::from_raw(self.policy_min_obs_raw),
                    require_objectives: self.policy_require_objectives,
                    consensus_window: self.policy_consensus_window,
                    planning_window: self.policy_planning_window,
                    execution_window: self.policy_execution_window,
                    max_plans: self.policy_max_plans,
                    settlement_threshold: self.policy_settlement_threshold,
                    max_rounds: self.policy_max_rounds,
                },
            },
            3 => GovAction::SetRequiredConstraints {
                ttype: self.ttype,
                set: self.set_ids[..self.set_count as usize].iter().copied().collect(),
            },
            4 => GovAction::SetRequiredObjectives {
                ttype: self.ttype,
                set: self.set_ids[..self.set_count as usize].iter().copied().collect(),
            },
            5 => GovAction::SetRequiredCapabilities {
                ttype: self.ttype,
                reqs: self.cap_ids[..self.cap_count as usize]
                    .iter()
                    .zip(self.cap_amounts[..self.cap_count as usize].iter())
                    .map(|(id, raw)| (*id, Amount::from_raw(*raw)))
                    .collect(),
            },
            6 => GovAction::SetAgentCapability { agent: self.agent, cap: self.cap_id, held: self.flag },
            7 => GovAction::SetCapacity {
                resource: self.resource,
                capacity: Amount::from_raw(self.amount_raw),
            },
            8 => GovAction::SetGroupMultiplier {
                ttype: self.ttype,
                group: self.group,
                mult: Amount::from_raw(self.amount_raw),
            },
            9 => GovAction::SetMaxInfluenceShare { share: Amount::from_raw(self.amount_raw) },
            10 => GovAction::RegisterAgent {
                agent: self.agent,
                groups: self.group_ids[..self.group_count as usize].iter().copied().collect(),
                base_influence: Amount::from_raw(self.amount_raw),
            },
            11 => GovAction::SetAgentActive { agent: self.agent, active: self.flag },
            12 => GovAction::SetCouncilSeats { seats: self.seats_u32 },
            13 => GovAction::InstallCouncil {
                seats: self.council_seats[..self.council_count as usize].to_vec(),
            },
            14 => GovAction::Reboot,
            _ => return Err(error!(ProgramV04Error::GovActionDecode)),
        })
    }
}

/// The fifth §3 account: one stored council approval of one exact
/// governance action (slice 33). One record per (transition, seat) —
/// the PDA seeds are `["gov-approval", transition_id, seat]` — because
/// the kernel's unanimity gate judges seats, not votes: each sitting
/// seat must approve the exact (id, action) pair, and this record is
/// that seat's approval, stored with the action it approved. The
/// `init` constraint refuses a second record for the same seat, so a
/// seat's first approval stands (see the module docs for the
/// kernel-overwrite divergence this pins). No stage byte: an approval
/// is either stored or absent, and it is only ever created against a
/// `REGISTERED` governance record.
#[account]
#[derive(Debug)]
pub struct GovApprovalRecord {
    /// Transition this approval belongs to (the PDA seed after `"gov-approval"`).
    pub transition_id: [u8; 32],
    /// Approving council seat (the signer's pubkey, by the identity mapping).
    pub seat: Pubkey,
    /// The exact action this seat approved, in bounded storage form.
    pub action: StoredGovAction,
    /// PDA bump for the `["gov-approval", transition_id, seat]` seeds.
    pub bump: u8,
}

impl GovApprovalRecord {
    /// 8 (discriminator) + 32 + 32 + 1524 (StoredGovAction) + 1.
    pub const SPACE: usize = 1597;
}

/// Build a `GovApprovalRecord` against a `KernelState` and its
/// governance `TransitionRecord` (slice 33): the sealed-state gate,
/// the governance-type gate, the `REGISTERED`-stage gate (approvals
/// are collected while the proposal awaits its governance replay),
/// the seated-council gate judged against the pinned profile, and
/// the action-category gate (the action must travel under exactly
/// the record's type, as kernel `register_governance_transition`
/// requires). Pure, as the registration builders are: a refused
/// approval leaves the state untouched, and the caller (instruction
/// or test) owns persistence.
pub fn build_governance_approval_record(
    kernel_state: &KernelState,
    transition_record: &TransitionRecord,
    seat: Pubkey,
    action: &GovAction,
    bump: u8,
) -> Result<GovApprovalRecord> {
    if !kernel_state.sealed {
        return Err(error!(ProgramV04Error::NotSealed));
    }
    if !is_gov_type(&transition_record.transition_type) {
        return Err(error!(ProgramV04Error::GovernanceTypeRequired));
    }
    if transition_record.stage != TransitionRecord::REGISTERED {
        return Err(error!(ProgramV04Error::WrongStage));
    }
    let agent_id = resolve_agent_id(seat);
    let k = sealed_launch_kernel().map_err(|_| error!(ProgramV04Error::KernelSeal))?;
    match k.agent(&agent_id) {
        Some(profile) if profile.active && k.is_seated(&agent_id) => {}
        _ => return Err(error!(ProgramV04Error::NotCouncilSeat)),
    }
    if action.category() != transition_record.transition_type {
        return Err(error!(ProgramV04Error::GovActionCategoryMismatch));
    }
    Ok(GovApprovalRecord {
        transition_id: transition_record.transition_id,
        seat,
        action: StoredGovAction::encode(action)?,
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
    prior_commits: &[PriorCommit],
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
    let mut k = kernel_at_head_from_commits(kernel_state, prior_commits)?;
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
    prior_commits: &[PriorCommit],
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
    let mut k = kernel_at_head_from_commits(kernel_state, prior_commits)?;
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
    prior_commits: &[PriorCommit],
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
    let mut k = kernel_at_head_from_commits(kernel_state, prior_commits)?;
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
    prior_commits: &[PriorCommit],
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
    let mut k = kernel_at_head_from_commits(kernel_state, prior_commits)?;
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
    prior_commits: &[PriorCommit],
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
    let mut k = kernel_at_head_from_commits(kernel_state, prior_commits)?;
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
/// (slices 20 and 22). Pure except for the target record's stage,
/// planning deadline, and (Blocked path only) `replans` count,
/// which advance only on success — every refusal leaves the record
/// untouched. Gates, in order: sealed state; stage exactly
/// `SELECTED` or `BLOCKED`; on the Blocked path, the kernel's
/// re-plan rule enforced here — caller is the record's proposer or
/// a seat of the sealed launch council, and the stored `replans`
/// count is below the kernel's `MAX_REPLANS` (imported, not
/// re-stated, so the cap cannot drift from the kernel's);
/// parent exactly the sealed head; observation linkage; a proven
/// window; the completeness anchor — the supplied prior set must
/// number exactly `signal_count`; every prior belonging to the
/// target with a stored round matching its own `submitted_at`; no
/// duplicated (agent, round) in the set. Replay then rebuilds the
/// sealed launch kernel under `ProgramEnv`, replays the eligibility
/// chain at the stored `eligible_at`, replays every prior signal at
/// its own `submitted_at` in order, and runs kernel
/// `select_candidate` at the state's clock — the stored `SELECTED`
/// label is re-proven, never trusted — and only then runs kernel
/// `open_planning` with the caller's agent id at the same clock.
/// Honesty bound, stated not smoothed: the replay kernel reaches
/// `Selected`, so the kernel call itself always takes its anyone
/// path — the Blocked path's proposer-or-seat rule and cap are
/// enforced by this helper from the stored record, not by the
/// kernel call, because the replay kernel reaches `Selected` and
/// never replays INTO `Blocked`. The stored `BLOCKED` label is
/// therefore trusted this slice in one narrow sense: since slice
/// 24, `validate_transition` is the only instruction that writes
/// it, after a full-replay proof that the predicate failed — so a
/// live `BLOCKED` record implies a validate-proven block, and a
/// block verdict cannot improve on its own (constraints and
/// compatibility are replay-fixed, expiry and freshness only
/// decay), so the label cannot have gone stale. Selection,
/// proposer, and the seat roster are all re-proven, not trusted.
/// Only a
/// `Life::Planning` life advances the stage, and the persisted
/// deadline is the state's clock plus the launch policy's
/// planning window: `apply_launch_genesis` installs exactly
/// `launch_policy()` for this transition's type, so that window is
/// the kernel's own policy value, and a checked add refuses an
/// overflowing deadline rather than wrapping it. A re-plan does
/// not clear `plan_count` or the stored plan set: the kernel
/// carries its plan map across a re-plan unchanged, and this
/// helper does the same — reconcile judges the carried set.
pub fn open_planning_from_records(
    kernel_state: &KernelState,
    transition_record: &mut TransitionRecord,
    observation_record: &ObservationRecord,
    prior_signals: &[SignalRecord],
    caller: [u8; 32],
    prior_commits: &[PriorCommit],
) -> Result<Life> {
    if !kernel_state.sealed {
        return Err(error!(ProgramV04Error::NotSealed));
    }
    if transition_record.stage != TransitionRecord::SELECTED
        && transition_record.stage != TransitionRecord::BLOCKED
    {
        return Err(error!(ProgramV04Error::WrongStage));
    }
    let is_replan = transition_record.stage == TransitionRecord::BLOCKED;
    if is_replan {
        let proposer_id = resolve_agent_id(transition_record.proposer);
        let seated = sealed_launch_kernel()
            .map(|k| k.is_seated(&caller))
            .map_err(|_| error!(ProgramV04Error::PlanningFailed))?;
        if caller != proposer_id && !seated {
            return Err(error!(ProgramV04Error::ReplanRefused));
        }
        if transition_record.replans >= MAX_REPLANS {
            return Err(error!(ProgramV04Error::ReplanRefused));
        }
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
    let mut k = kernel_at_head_from_commits(kernel_state, prior_commits)?;
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
    if is_replan {
        transition_record.replans += 1;
    }
    Ok(Life::Planning)
}

/// One claim in a submitted plan (slice 21): a kernel resource id
/// and an amount in kernel `Amount` raw units. Instruction args
/// only — the stored record uses parallel fixed arrays (see
/// `PlanRecord`), so a plan's claims are bounded and borsh-stable.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlanClaimArg {
    pub resource: u16,
    pub amount_raw: u64,
}

/// The fifth §3 account (slice 21): one agent's submitted plan for a
/// transition. One record per (transition, agent) — the PDA seeds
/// bind it — so a second plan from the same agent cannot be created
/// twice on-chain, and the kernel's `Duplicate` arm is the second
/// guard in replay. Claims are parallel fixed arrays capped at
/// `MAX_PLAN_CLAIMS`: a plan with more claims refuses rather than
/// truncating, and a duplicated resource refuses rather than
/// collapsing silently the way a map insert would. `claim_count`
/// is the live length; entries at or past it are zero and ignored.
/// A plan does not advance the transition stage: the record stays
/// `PLANNING`, and `TransitionRecord.plan_count` (written by
/// `submit_plan` on success only, like `signal_count`) is the
/// completeness anchor the later reconcile slice checks its
/// supplied plan set against.
#[account]
#[derive(Debug)]
pub struct PlanRecord {
    /// Transition this plan belongs to (the PDA seed after `"plan"`).
    pub transition_id: [u8; 32],
    /// Planning agent (the signer's pubkey, by the identity mapping).
    pub agent: Pubkey,
    /// Live number of claims in the parallel arrays below.
    pub claim_count: u8,
    /// Claimed resource ids, first `claim_count` entries live.
    pub claim_resources: [u16; PlanRecord::MAX_PLAN_CLAIMS],
    /// Claimed amounts in kernel `Amount` raw units, aligned with
    /// `claim_resources`.
    pub claim_amounts: [u64; PlanRecord::MAX_PLAN_CLAIMS],
    /// Plan expiry — must exceed the stored planning deadline
    /// (the kernel's `BadWindow` rule, judged against the stored
    /// deadline, not a replayed one).
    pub expires_at: u64,
    /// PDA bump for the `["plan", transition_id, agent]` seeds.
    pub bump: u8,
}

impl PlanRecord {
    /// Hard cap on claims per plan (slice 21): a longer claim list
    /// refuses rather than truncating.
    pub const MAX_PLAN_CLAIMS: usize = 8;
    /// 8 (discriminator) + 32 + 32 + 1 + 16 + 64 + 8 + 1.
    pub const SPACE: usize = 162;
}

/// Submit a plan through the real kernel (slice 21). Pure except for
/// the transition record's `plan_count`, which increments by one on
/// success only, and the returned record: every refusal leaves both
/// untouched, and the stage stays `PLANNING` throughout. Gates, in
/// order: sealed state; stage exactly `PLANNING`; parent exactly the
/// sealed head; observation linkage; a proven planning window (a
/// nonzero stored deadline, with a derivable open clock at or before
/// the state's clock and the state's clock strictly inside the
/// window — at the deadline the window is closed and planning is
/// over); the new plan's expiry strictly after the stored deadline;
/// the prior-signal anchor (supplied signals numbering exactly
/// `signal_count`, every prior belonging to the transition with a
/// stored round matching its own `submitted_at`, no duplicated
/// (agent, round)); the prior-plan anchor (supplied plans numbering
/// exactly `plan_count`, every prior belonging to the transition,
/// no duplicated resource inside a prior's claims); the new claim
/// list bounded and duplicate-free. The kernel calls run against a
/// freshly rebuilt sealed launch kernel under `ProgramEnv`: ingest,
/// register, evaluate, and mark_eligible, all at the stored
/// `eligible_at`, then each prior signal at its `submitted_at` in
/// order, then select and open_planning at the *derived open
/// clock* (so the replayed deadline equals the stored deadline, by
/// construction — a mismatch refuses rather than overwriting it),
/// then each prior plan at the state's clock, then the new plan at
/// the state's clock. A kernel refusal in any call maps to
/// `PlanFailed`: an unknown or inactive agent, an agent outside the
/// proposer's groups, a duplicate plan, the plan cap, a plan expiry
/// at or before the deadline, and a signal or select that no longer
/// replays all fail here, through the kernel, not through a
/// program-side guess about them. Replay-horizon bound, stated not
/// smoothed: select and open_planning replay at the derived open
/// clock, so a submission whose transition manifest already expired
/// by that clock refuses here — but the kernel would refuse it too
/// (`not_expired` at the state's clock, which is never earlier than
/// the open clock), so no legitimate submission is lost to the
/// replay.
#[allow(clippy::too_many_arguments)]
pub fn submit_plan_from_records(
    kernel_state: &KernelState,
    transition_record: &mut TransitionRecord,
    observation_record: &ObservationRecord,
    prior_signals: &[SignalRecord],
    prior_plans: &[PlanRecord],
    claims: &[PlanClaimArg],
    expires_at: u64,
    caller: [u8; 32],
    bump: u8,
    prior_commits: &[PriorCommit],
) -> Result<PlanRecord> {
    if !kernel_state.sealed {
        return Err(error!(ProgramV04Error::NotSealed));
    }
    if transition_record.stage != TransitionRecord::PLANNING {
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
    if transition_record.planning_deadline == 0 {
        return Err(error!(ProgramV04Error::PlanFailed));
    }
    let open_clock = transition_record
        .planning_deadline
        .checked_sub(launch_policy().planning_window)
        .ok_or(error!(ProgramV04Error::PlanFailed))?;
    if kernel_state.clock < open_clock || kernel_state.clock >= transition_record.planning_deadline {
        return Err(error!(ProgramV04Error::PlanFailed));
    }
    if expires_at <= transition_record.planning_deadline {
        return Err(error!(ProgramV04Error::PlanFailed));
    }
    if prior_signals.len() as u64 != transition_record.signal_count {
        return Err(error!(ProgramV04Error::PlanFailed));
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
                return Err(error!(ProgramV04Error::PlanFailed));
            }
        }
    }
    if prior_plans.len() as u64 != transition_record.plan_count {
        return Err(error!(ProgramV04Error::PlanFailed));
    }
    for prior in prior_plans {
        if prior.transition_id != transition_record.transition_id {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        if prior.claim_count as usize > PlanRecord::MAX_PLAN_CLAIMS {
            return Err(error!(ProgramV04Error::PlanFailed));
        }
        let mut seen: BTreeSet<u16> = BTreeSet::new();
        for resource in &prior.claim_resources[..prior.claim_count as usize] {
            if !seen.insert(*resource) {
                return Err(error!(ProgramV04Error::PlanFailed));
            }
        }
    }
    if claims.len() > PlanRecord::MAX_PLAN_CLAIMS {
        return Err(error!(ProgramV04Error::PlanFailed));
    }
    let mut claim_map: BTreeMap<u16, Amount> = BTreeMap::new();
    for claim in claims {
        if claim_map
            .insert(claim.resource, Amount::from_raw(claim.amount_raw))
            .is_some()
        {
            return Err(error!(ProgramV04Error::PlanFailed));
        }
    }
    let proposer = resolve_proposer(kernel_state, transition_record.proposer)
        .map_err(|_| error!(ProgramV04Error::PlanFailed))?;
    let observer_id = resolve_agent_id(observation_record.observer);
    let mut k = kernel_at_head_from_commits(kernel_state, prior_commits)?;
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
        .map_err(|_| error!(ProgramV04Error::PlanFailed))?;
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
    .map_err(|_| error!(ProgramV04Error::PlanFailed))?;
    k.evaluate_transition(&transition_record.transition_id, eligible_at)
        .map_err(|_| error!(ProgramV04Error::PlanFailed))?;
    k.mark_eligible(&transition_record.transition_id, eligible_at)
        .map_err(|_| error!(ProgramV04Error::PlanFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Eligible) => {}
        _ => return Err(error!(ProgramV04Error::PlanFailed)),
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
        .map_err(|_| error!(ProgramV04Error::PlanFailed))?;
    }
    k.select_candidate(&transition_record.transition_id, open_clock)
        .map_err(|_| error!(ProgramV04Error::PlanFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Selected) => {}
        _ => return Err(error!(ProgramV04Error::PlanFailed)),
    }
    k.open_planning(&caller, &transition_record.transition_id, open_clock)
        .map_err(|_| error!(ProgramV04Error::PlanFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Planning) => {}
        _ => return Err(error!(ProgramV04Error::PlanFailed)),
    }
    let deadline = open_clock
        .checked_add(launch_policy().planning_window)
        .ok_or(error!(ProgramV04Error::PlanFailed))?;
    if deadline != transition_record.planning_deadline {
        return Err(error!(ProgramV04Error::PlanFailed));
    }
    for prior in prior_plans {
        let mut prior_map: BTreeMap<u16, Amount> = BTreeMap::new();
        for (resource, amount) in prior.claim_resources[..prior.claim_count as usize]
            .iter()
            .zip(prior.claim_amounts[..prior.claim_count as usize].iter())
        {
            prior_map.insert(*resource, Amount::from_raw(*amount));
        }
        let prior_plan = Plan {
            agent: prior.agent.to_bytes(),
            claims: prior_map,
            expires_at: prior.expires_at,
        };
        k.submit_plan(
            &prior.agent.to_bytes(),
            &transition_record.transition_id,
            prior_plan,
            kernel_state.clock,
        )
        .map_err(|_| error!(ProgramV04Error::PlanFailed))?;
    }
    let plan = Plan {
        agent: caller,
        claims: claim_map,
        expires_at,
    };
    k.submit_plan(
        &caller,
        &transition_record.transition_id,
        plan,
        kernel_state.clock,
    )
    .map_err(|_| error!(ProgramV04Error::PlanFailed))?;
    transition_record.plan_count = transition_record
        .plan_count
        .checked_add(1)
        .ok_or(error!(ProgramV04Error::PlanFailed))?;
    let mut claim_resources = [0u16; PlanRecord::MAX_PLAN_CLAIMS];
    let mut claim_amounts = [0u64; PlanRecord::MAX_PLAN_CLAIMS];
    for (i, claim) in claims.iter().enumerate() {
        claim_resources[i] = claim.resource;
        claim_amounts[i] = claim.amount_raw;
    }
    Ok(PlanRecord {
        transition_id: transition_record.transition_id,
        agent: Pubkey::new_from_array(caller),
        claim_count: claims.len() as u8,
        claim_resources,
        claim_amounts,
        expires_at,
        bump,
    })
}

/// Reconcile plans through the real kernel (slice 23). Pure except
/// for the transition record's stage and `compatible`, which move
/// on success only, and every refusal leaves both untouched. Gates,
/// in order: sealed state; stage exactly `PLANNING`; parent exactly
/// the sealed head; observation linkage; a proven planning window
/// (a nonzero stored deadline, with a derivable open clock at or
/// before the state's clock and the state's clock at or after the
/// deadline — before the deadline the window is still open and the
/// kernel refuses, so no legitimate reconcile is lost); the
/// prior-signal anchor (supplied signals numbering exactly
/// `signal_count`, every prior belonging to the transition with a
/// stored round matching its own `submitted_at`, no duplicated
/// (agent, round)); the prior-plan anchor (supplied plans numbering
/// exactly `plan_count`, every prior belonging to the transition,
/// no duplicated resource inside a prior's claims). The kernel
/// calls run against the kernel at the program's head, rebuilt by slice 29's `kernel_at_head_from_commits` from the supplied prior commits (empty priors = the genesis head) under
/// `ProgramEnv`: ingest, register, evaluate, and mark_eligible, all
/// at the stored `eligible_at`, then each prior signal at its
/// `submitted_at` in order, then select and open_planning at the
/// *derived open clock* (so the replayed deadline equals the stored
/// deadline, by construction — a mismatch refuses rather than
/// overwriting it), then each prior plan at the open clock, then
/// reconcile at the state's clock. Prior plans replay at the open
/// clock because a `PlanRecord` carries no submission time: the
/// open clock is the one window clock every stored plan is
/// guaranteed to satisfy (inside the window, expiry after the
/// deadline — the only clock rules kernel `submit_plan` applies
/// besides manifest expiry, which the state's clock already
/// proves), so a plan that could not have been submitted inside
/// the window still refuses here, through the kernel's own gates,
/// while no legitimate plan is lost to a clock it never carried.
/// A kernel refusal in any call maps to `ReconcileFailed`. The
/// stored `compatible` verdict is the exact mirror of the kernel's
/// `fits_capacity` over the supplied plan set — non-empty, and
/// every per-resource checked-add sum within the sealed capacity
/// stock for that resource — so the record and the event carry the
/// verdict the kernel just computed, and the later validate slice
/// can cross-check it against its own replay instead of trusting
/// either alone. Replay-horizon bound, stated not smoothed: a
/// reconcile whose transition manifest already expired by the
/// state's clock refuses here — but the kernel would refuse it too
/// (`not_expired` at the state's clock), so no legitimate reconcile
/// is lost to the replay.
pub fn reconcile_plans_from_records(
    kernel_state: &KernelState,
    transition_record: &mut TransitionRecord,
    observation_record: &ObservationRecord,
    prior_signals: &[SignalRecord],
    prior_plans: &[PlanRecord],
    prior_commits: &[PriorCommit],
) -> Result<Life> {
    if !kernel_state.sealed {
        return Err(error!(ProgramV04Error::NotSealed));
    }
    if transition_record.stage != TransitionRecord::PLANNING {
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
    if transition_record.planning_deadline == 0 {
        return Err(error!(ProgramV04Error::ReconcileFailed));
    }
    let open_clock = transition_record
        .planning_deadline
        .checked_sub(launch_policy().planning_window)
        .ok_or(error!(ProgramV04Error::ReconcileFailed))?;
    if kernel_state.clock < transition_record.planning_deadline {
        return Err(error!(ProgramV04Error::ReconcileFailed));
    }
    if prior_signals.len() as u64 != transition_record.signal_count {
        return Err(error!(ProgramV04Error::ReconcileFailed));
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
                return Err(error!(ProgramV04Error::ReconcileFailed));
            }
        }
    }
    if prior_plans.len() as u64 != transition_record.plan_count {
        return Err(error!(ProgramV04Error::ReconcileFailed));
    }
    for prior in prior_plans {
        if prior.transition_id != transition_record.transition_id {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        if prior.claim_count as usize > PlanRecord::MAX_PLAN_CLAIMS {
            return Err(error!(ProgramV04Error::ReconcileFailed));
        }
        let mut seen: BTreeSet<u16> = BTreeSet::new();
        for resource in &prior.claim_resources[..prior.claim_count as usize] {
            if !seen.insert(*resource) {
                return Err(error!(ProgramV04Error::ReconcileFailed));
            }
        }
    }
    let proposer = resolve_proposer(kernel_state, transition_record.proposer)
        .map_err(|_| error!(ProgramV04Error::ReconcileFailed))?;
    let observer_id = resolve_agent_id(observation_record.observer);
    let mut k = kernel_at_head_from_commits(kernel_state, prior_commits)?;
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
        .map_err(|_| error!(ProgramV04Error::ReconcileFailed))?;
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
    .map_err(|_| error!(ProgramV04Error::ReconcileFailed))?;
    k.evaluate_transition(&transition_record.transition_id, eligible_at)
        .map_err(|_| error!(ProgramV04Error::ReconcileFailed))?;
    k.mark_eligible(&transition_record.transition_id, eligible_at)
        .map_err(|_| error!(ProgramV04Error::ReconcileFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Eligible) => {}
        _ => return Err(error!(ProgramV04Error::ReconcileFailed)),
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
        .map_err(|_| error!(ProgramV04Error::ReconcileFailed))?;
    }
    k.select_candidate(&transition_record.transition_id, open_clock)
        .map_err(|_| error!(ProgramV04Error::ReconcileFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Selected) => {}
        _ => return Err(error!(ProgramV04Error::ReconcileFailed)),
    }
    // The kernel's Selected-path planning rule is "anyone": the
    // proposer is the replay caller here, not a gate — any agent id
    // would take the same path, and no caller identity is judged.
    k.open_planning(&proposer, &transition_record.transition_id, open_clock)
        .map_err(|_| error!(ProgramV04Error::ReconcileFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Planning) => {}
        _ => return Err(error!(ProgramV04Error::ReconcileFailed)),
    }
    let deadline = open_clock
        .checked_add(launch_policy().planning_window)
        .ok_or(error!(ProgramV04Error::ReconcileFailed))?;
    if deadline != transition_record.planning_deadline {
        return Err(error!(ProgramV04Error::ReconcileFailed));
    }
    for prior in prior_plans {
        let mut prior_map: BTreeMap<u16, Amount> = BTreeMap::new();
        for (resource, amount) in prior.claim_resources[..prior.claim_count as usize]
            .iter()
            .zip(prior.claim_amounts[..prior.claim_count as usize].iter())
        {
            prior_map.insert(*resource, Amount::from_raw(*amount));
        }
        let prior_plan = Plan {
            agent: prior.agent.to_bytes(),
            claims: prior_map,
            expires_at: prior.expires_at,
        };
        k.submit_plan(
            &prior.agent.to_bytes(),
            &transition_record.transition_id,
            prior_plan,
            open_clock,
        )
        .map_err(|_| error!(ProgramV04Error::ReconcileFailed))?;
    }
    k.reconcile_plans(&transition_record.transition_id, kernel_state.clock)
        .map_err(|_| error!(ProgramV04Error::ReconcileFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Reconciling) => {}
        _ => return Err(error!(ProgramV04Error::ReconcileFailed)),
    }
    // Exact mirror of the kernel's `fits_capacity` over the same
    // supplied set: a missing stock, or an overflowing sum, is
    // incompatible, exactly as the kernel judges it.
    let mut sums: BTreeMap<u16, Amount> = BTreeMap::new();
    let mut fits = !prior_plans.is_empty();
    for prior in prior_plans {
        for (resource, amount) in prior.claim_resources[..prior.claim_count as usize]
            .iter()
            .zip(prior.claim_amounts[..prior.claim_count as usize].iter())
        {
            let entry = sums.entry(*resource).or_insert(Amount::ZERO);
            match entry.checked_add(Amount::from_raw(*amount)) {
                Ok(v) => *entry = v,
                Err(_) => fits = false,
            }
        }
    }
    if fits {
        for (resource, total) in &sums {
            if !k.capacity_of(*resource).is_some_and(|cap| *total <= cap) {
                fits = false;
            }
        }
    }
    transition_record.stage = TransitionRecord::RECONCILING;
    transition_record.compatible = fits;
    Ok(Life::Reconciling)
}

/// Validate a reconciled transition through the real kernel (slice 24).
/// Pure except for the transition record's stage, which moves on
/// success only, and every refusal leaves it untouched. Gates, in
/// order: sealed state; stage exactly `RECONCILING` or `BLOCKED`;
/// parent exactly the sealed head; observation linkage; a proven
/// planning window (a nonzero stored deadline, with a derivable open
/// clock — the reconcile replay below re-proves the deadline and the
/// state's clock sitting at or after it); the prior-signal anchor
/// (supplied signals numbering exactly `signal_count`, every prior
/// belonging to the transition with a stored round matching its own
/// `submitted_at`, no duplicated (agent, round)); the prior-plan
/// anchor (supplied plans numbering exactly `plan_count`, every prior
/// belonging to the transition, no duplicated resource inside a
/// prior's claims). The kernel calls run against a freshly rebuilt
/// sealed launch kernel under `ProgramEnv`: ingest, register,
/// evaluate, and mark_eligible, all at the stored `eligible_at`,
/// then each prior signal at its `submitted_at` in order, then select
/// and open_planning at the derived open clock, then each prior plan
/// at the open clock, then reconcile at the state's clock, then
/// validate at the state's clock. The stored `compatible` verdict is
/// cross-checked, not trusted: slice 23's exact mirror (non-empty,
/// and every per-resource checked-add sum within the sealed capacity
/// stock for that resource) is recomputed over the supplied plan set,
/// and a stored verdict the set does not reproduce refuses with
/// `ValidateFailed` before any kernel validate runs — a divergence
/// means a supplied plan the reconcile replay did not judge, or a
/// stored verdict no replay reproduces. The kernel's own validate
/// then judges the rest of the predicate (constraints, compatibility,
/// blocking exceptions, manifest expiry, observation freshness at the
/// state's clock): a passing predicate advances the stage to
/// `VALIDATED`; a failing predicate from `RECONCILING` advances it
/// to `BLOCKED`, which the slice-22 re-plan path opens from; a
/// still-blocked `BLOCKED` record refuses with `ValidateFailed` and
/// stands, exactly as the kernel's `StillBlocked` arm dictates —
/// enforced here from the stored stage, in the slice-22 manner,
/// because the replay kernel reaches `Reconciling` and never
/// carries the record's `Blocked` life into the kernel call.
/// Honesty bound, stated not smoothed: the replay carries no
/// exceptions, so `unresolved_blocking` is always zero here —
/// program validate judges the stored chain, not off-program
/// exception state, and the exception instructions that would clear
/// a block are a later slice's work, not claimed here. Likewise a
/// validate-produced `BLOCKED` record re-validated over the same
/// chain refuses still-blocked: nothing in the replay changed the
/// predicate inputs, so the kernel cannot newly pass it; the
/// `BLOCKED` -> `VALIDATED` path exists for records whose blocking
/// condition cleared outside this replay. A kernel refusal in any
/// call maps to `ValidateFailed`, except the seal gate (`NotSealed`),
/// the stage gate (`WrongStage`), the parent gate (`ParentMismatch`),
/// and the linkage gates (`RecordMismatch`, `NotObserver`,
/// `UnknownProposer`), which keep their own errors.
pub fn validate_transition_from_records(
    kernel_state: &KernelState,
    transition_record: &mut TransitionRecord,
    observation_record: &ObservationRecord,
    prior_signals: &[SignalRecord],
    prior_plans: &[PlanRecord],
    prior_commits: &[PriorCommit],
) -> Result<Life> {
    if !kernel_state.sealed {
        return Err(error!(ProgramV04Error::NotSealed));
    }
    if transition_record.stage != TransitionRecord::RECONCILING
        && transition_record.stage != TransitionRecord::BLOCKED
    {
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
    if transition_record.planning_deadline == 0 {
        return Err(error!(ProgramV04Error::ValidateFailed));
    }
    let open_clock = transition_record
        .planning_deadline
        .checked_sub(launch_policy().planning_window)
        .ok_or(error!(ProgramV04Error::ValidateFailed))?;
    if prior_signals.len() as u64 != transition_record.signal_count {
        return Err(error!(ProgramV04Error::ValidateFailed));
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
                return Err(error!(ProgramV04Error::ValidateFailed));
            }
        }
    }
    if prior_plans.len() as u64 != transition_record.plan_count {
        return Err(error!(ProgramV04Error::ValidateFailed));
    }
    for prior in prior_plans {
        if prior.transition_id != transition_record.transition_id {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        if prior.claim_count as usize > PlanRecord::MAX_PLAN_CLAIMS {
            return Err(error!(ProgramV04Error::ValidateFailed));
        }
        let mut seen: BTreeSet<u16> = BTreeSet::new();
        for resource in &prior.claim_resources[..prior.claim_count as usize] {
            if !seen.insert(*resource) {
                return Err(error!(ProgramV04Error::ValidateFailed));
            }
        }
    }
    let proposer = resolve_proposer(kernel_state, transition_record.proposer)
        .map_err(|_| error!(ProgramV04Error::ValidateFailed))?;
    let observer_id = resolve_agent_id(observation_record.observer);
    let mut k = kernel_at_head_from_commits(kernel_state, prior_commits)?;
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
        .map_err(|_| error!(ProgramV04Error::ValidateFailed))?;
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
    .map_err(|_| error!(ProgramV04Error::ValidateFailed))?;
    k.evaluate_transition(&transition_record.transition_id, eligible_at)
        .map_err(|_| error!(ProgramV04Error::ValidateFailed))?;
    k.mark_eligible(&transition_record.transition_id, eligible_at)
        .map_err(|_| error!(ProgramV04Error::ValidateFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Eligible) => {}
        _ => return Err(error!(ProgramV04Error::ValidateFailed)),
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
        .map_err(|_| error!(ProgramV04Error::ValidateFailed))?;
    }
    k.select_candidate(&transition_record.transition_id, open_clock)
        .map_err(|_| error!(ProgramV04Error::ValidateFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Selected) => {}
        _ => return Err(error!(ProgramV04Error::ValidateFailed)),
    }
    k.open_planning(&proposer, &transition_record.transition_id, open_clock)
        .map_err(|_| error!(ProgramV04Error::ValidateFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Planning) => {}
        _ => return Err(error!(ProgramV04Error::ValidateFailed)),
    }
    let deadline = open_clock
        .checked_add(launch_policy().planning_window)
        .ok_or(error!(ProgramV04Error::ValidateFailed))?;
    if deadline != transition_record.planning_deadline {
        return Err(error!(ProgramV04Error::ValidateFailed));
    }
    for prior in prior_plans {
        let mut prior_map: BTreeMap<u16, Amount> = BTreeMap::new();
        for (resource, amount) in prior.claim_resources[..prior.claim_count as usize]
            .iter()
            .zip(prior.claim_amounts[..prior.claim_count as usize].iter())
        {
            prior_map.insert(*resource, Amount::from_raw(*amount));
        }
        let prior_plan = Plan {
            agent: prior.agent.to_bytes(),
            claims: prior_map,
            expires_at: prior.expires_at,
        };
        k.submit_plan(
            &prior.agent.to_bytes(),
            &transition_record.transition_id,
            prior_plan,
            open_clock,
        )
        .map_err(|_| error!(ProgramV04Error::ValidateFailed))?;
    }
    k.reconcile_plans(&transition_record.transition_id, kernel_state.clock)
        .map_err(|_| error!(ProgramV04Error::ValidateFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Reconciling) => {}
        _ => return Err(error!(ProgramV04Error::ValidateFailed)),
    }
    // Slice 23's exact mirror, recomputed over the supplied set:
    // a stored verdict the set does not reproduce refuses here,
    // before the kernel's own validate runs, so neither the
    // stored verdict nor the replay is trusted alone.
    let mut sums: BTreeMap<u16, Amount> = BTreeMap::new();
    let mut fits = !prior_plans.is_empty();
    for prior in prior_plans {
        for (resource, amount) in prior.claim_resources[..prior.claim_count as usize]
            .iter()
            .zip(prior.claim_amounts[..prior.claim_count as usize].iter())
        {
            let entry = sums.entry(*resource).or_insert(Amount::ZERO);
            match entry.checked_add(Amount::from_raw(*amount)) {
                Ok(v) => *entry = v,
                Err(_) => fits = false,
            }
        }
    }
    if fits {
        for (resource, total) in &sums {
            if !k.capacity_of(*resource).is_some_and(|cap| *total <= cap) {
                fits = false;
            }
        }
    }
    if fits != transition_record.compatible {
        return Err(error!(ProgramV04Error::ValidateFailed));
    }
    k.validate_transition(&transition_record.transition_id, kernel_state.clock)
        .map_err(|_| error!(ProgramV04Error::ValidateFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Validated) => {
            transition_record.stage = TransitionRecord::VALIDATED;
            Ok(Life::Validated)
        }
        Some(Life::Blocked) => {
            // The replay kernel reached `Reconciling`, so the
            // kernel call itself always takes its Reconciling
            // path — it can never report `StillBlocked`. The
            // kernel's rule is therefore enforced here from the
            // stored stage: a record that entered validate
            // already `BLOCKED` and still fails refuses and
            // stands, exactly as `StillBlocked` dictates, while a
            // record that entered `RECONCILING` advances to
            // `BLOCKED` as a new verdict.
            if transition_record.stage == TransitionRecord::BLOCKED {
                return Err(error!(ProgramV04Error::ValidateFailed));
            }
            transition_record.stage = TransitionRecord::BLOCKED;
            Ok(Life::Blocked)
        }
        _ => Err(error!(ProgramV04Error::ValidateFailed)),
    }
}

/// Declare a validated transition (slice 25). Pure except for the
/// record's stage, which advances to `DECLARED` only on success —
/// every refusal leaves the record exactly as it stood. Gates, in
/// order: sealed state; stage exactly `VALIDATED`; parent exactly
/// the sealed head; observation linkage; a proven planning
/// deadline; the completeness anchors — the supplied prior signal
/// set numbering exactly `signal_count` and the supplied prior
/// plan set numbering exactly `plan_count`, each prior belonging
/// to the transition with a stored round matching its own
/// `submitted_at` and no duplicated (agent, round) in the signal
/// set. Replay then re-proves the whole chain in a rebuilt kernel
/// at the derived open clock (ingest + register + evaluate +
/// mark_eligible, signals sorted by `submitted_at`, select, open
/// planning, prior plans) and reconciles at the state's clock;
/// the stored `compatible` verdict is cross-checked against
/// slice 23's exact mirror recomputed over the supplied plan set,
/// exactly as in slice 24, and a stored verdict the set does not
/// reproduce refuses before any kernel validate runs. Kernel
/// `validate_transition` at the state's clock must then yield
/// `Validated`: a `Blocked` verdict means the stored `VALIDATED`
/// label no longer holds at this clock (a stale observation, an
/// expired manifest), and declare refuses rather than writing a
/// verdict it did not judge — declare is not a verdict writer.
/// Only then kernel `declare_commit` runs at the state's clock:
/// it re-runs the entire predicate (the 13-bit
/// `commit_fail_mask`), and a non-clear mask refuses with
/// `CommitFailed` mapped here to `DeclareFailed` — the case
/// validate cannot see is a plan expiry reached since validate
/// (the `PLANS` bit), since validate's structural check does not
/// judge plan expiry. A successful declare leaves the sealed head
/// untouched: the head moves only when commit replay lands, in
/// slice 26. The payer is a fee payer only: kernel
/// `declare_commit` is permissionless, so no proposer, seat, or
/// agent gate applies here.
pub fn declare_transition_from_records(
    kernel_state: &KernelState,
    transition_record: &mut TransitionRecord,
    observation_record: &ObservationRecord,
    prior_signals: &[SignalRecord],
    prior_plans: &[PlanRecord],
    prior_commits: &[PriorCommit],
) -> Result<Life> {
    if !kernel_state.sealed {
        return Err(error!(ProgramV04Error::NotSealed));
    }
    if transition_record.stage != TransitionRecord::VALIDATED {
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
    if transition_record.planning_deadline == 0 {
        return Err(error!(ProgramV04Error::DeclareFailed));
    }
    let open_clock = transition_record
        .planning_deadline
        .checked_sub(launch_policy().planning_window)
        .ok_or(error!(ProgramV04Error::DeclareFailed))?;
    if prior_signals.len() as u64 != transition_record.signal_count {
        return Err(error!(ProgramV04Error::DeclareFailed));
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
                return Err(error!(ProgramV04Error::DeclareFailed));
            }
        }
    }
    if prior_plans.len() as u64 != transition_record.plan_count {
        return Err(error!(ProgramV04Error::DeclareFailed));
    }
    for prior in prior_plans {
        if prior.transition_id != transition_record.transition_id {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        if prior.claim_count as usize > PlanRecord::MAX_PLAN_CLAIMS {
            return Err(error!(ProgramV04Error::DeclareFailed));
        }
        let mut seen: BTreeSet<u16> = BTreeSet::new();
        for resource in &prior.claim_resources[..prior.claim_count as usize] {
            if !seen.insert(*resource) {
                return Err(error!(ProgramV04Error::DeclareFailed));
            }
        }
    }
    let proposer = resolve_proposer(kernel_state, transition_record.proposer)
        .map_err(|_| error!(ProgramV04Error::DeclareFailed))?;
    let observer_id = resolve_agent_id(observation_record.observer);
    let mut k = kernel_at_head_from_commits(kernel_state, prior_commits)?;
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
        .map_err(|_| error!(ProgramV04Error::DeclareFailed))?;
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
    .map_err(|_| error!(ProgramV04Error::DeclareFailed))?;
    k.evaluate_transition(&transition_record.transition_id, eligible_at)
        .map_err(|_| error!(ProgramV04Error::DeclareFailed))?;
    k.mark_eligible(&transition_record.transition_id, eligible_at)
        .map_err(|_| error!(ProgramV04Error::DeclareFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Eligible) => {}
        _ => return Err(error!(ProgramV04Error::DeclareFailed)),
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
        .map_err(|_| error!(ProgramV04Error::DeclareFailed))?;
    }
    k.select_candidate(&transition_record.transition_id, open_clock)
        .map_err(|_| error!(ProgramV04Error::DeclareFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Selected) => {}
        _ => return Err(error!(ProgramV04Error::DeclareFailed)),
    }
    k.open_planning(&proposer, &transition_record.transition_id, open_clock)
        .map_err(|_| error!(ProgramV04Error::DeclareFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Planning) => {}
        _ => return Err(error!(ProgramV04Error::DeclareFailed)),
    }
    let deadline = open_clock
        .checked_add(launch_policy().planning_window)
        .ok_or(error!(ProgramV04Error::DeclareFailed))?;
    if deadline != transition_record.planning_deadline {
        return Err(error!(ProgramV04Error::DeclareFailed));
    }
    for prior in prior_plans {
        let mut prior_map: BTreeMap<u16, Amount> = BTreeMap::new();
        for (resource, amount) in prior.claim_resources[..prior.claim_count as usize]
            .iter()
            .zip(prior.claim_amounts[..prior.claim_count as usize].iter())
        {
            prior_map.insert(*resource, Amount::from_raw(*amount));
        }
        let prior_plan = Plan {
            agent: prior.agent.to_bytes(),
            claims: prior_map,
            expires_at: prior.expires_at,
        };
        k.submit_plan(
            &prior.agent.to_bytes(),
            &transition_record.transition_id,
            prior_plan,
            open_clock,
        )
        .map_err(|_| error!(ProgramV04Error::DeclareFailed))?;
    }
    k.reconcile_plans(&transition_record.transition_id, kernel_state.clock)
        .map_err(|_| error!(ProgramV04Error::DeclareFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Reconciling) => {}
        _ => return Err(error!(ProgramV04Error::DeclareFailed)),
    }
    // Slice 23's exact mirror, recomputed over the supplied set:
    // a stored verdict the set does not reproduce refuses here,
    // before the kernel's own validate runs, exactly as in
    // slice 24 — neither the stored verdict nor the replay is
    // trusted alone.
    let mut sums: BTreeMap<u16, Amount> = BTreeMap::new();
    let mut fits = !prior_plans.is_empty();
    for prior in prior_plans {
        for (resource, amount) in prior.claim_resources[..prior.claim_count as usize]
            .iter()
            .zip(prior.claim_amounts[..prior.claim_count as usize].iter())
        {
            let entry = sums.entry(*resource).or_insert(Amount::ZERO);
            match entry.checked_add(Amount::from_raw(*amount)) {
                Ok(v) => *entry = v,
                Err(_) => fits = false,
            }
        }
    }
    if fits {
        for (resource, total) in &sums {
            if !k.capacity_of(*resource).is_some_and(|cap| *total <= cap) {
                fits = false;
            }
        }
    }
    if fits != transition_record.compatible {
        return Err(error!(ProgramV04Error::DeclareFailed));
    }
    k.validate_transition(&transition_record.transition_id, kernel_state.clock)
        .map_err(|_| error!(ProgramV04Error::DeclareFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Validated) => {}
        // Declare is not a verdict writer: a replay validate
        // that yields Blocked means the stored VALIDATED label
        // no longer holds at this clock, and the record stands
        // VALIDATED — it is not re-labeled BLOCKED by declare.
        Some(Life::Blocked) => return Err(error!(ProgramV04Error::DeclareFailed)),
        _ => return Err(error!(ProgramV04Error::DeclareFailed)),
    }
    k.declare_commit(&transition_record.transition_id, kernel_state.clock)
        .map_err(|_| error!(ProgramV04Error::DeclareFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Committable) => {
            transition_record.stage = TransitionRecord::DECLARED;
            Ok(Life::Committable)
        }
        _ => Err(error!(ProgramV04Error::DeclareFailed)),
    }
}

/// Commit a declared transition and move the sealed head (slice 26).
/// Pure except for the record's stage and the state's head, which
/// advance only on success — every refusal leaves both exactly as
/// they stood. Gates, in order: sealed state; stage exactly
/// `DECLARED`; parent exactly the sealed head; observation
/// linkage; the governance gate — a governance-typed record
/// refuses with `GovernanceCommitRequired` before any replay,
/// because its commit path is `commit_governance`, not the
/// ordinary commit (kernel `commit` would refuse it the same
/// way, with `UseGovernanceCommit`); a proven planning
/// deadline; the completeness anchors — the supplied prior
/// signal set numbering exactly `signal_count` and the
/// supplied prior plan set numbering exactly `plan_count`,
/// each prior belonging to the transition with a stored round
/// matching its own `submitted_at` and no duplicated (agent,
/// round) in the signal set. Replay then re-proves the whole
/// chain in a rebuilt kernel at the derived open clock
/// (ingest, register, evaluate, mark_eligible; signals sorted
/// by `submitted_at`; select; open planning; prior plans) and
/// reconciles, validates, and declares at the state's clock;
/// the stored `compatible` verdict is cross-checked against
/// slice 23's exact mirror recomputed over the supplied plan
/// set, exactly as in slices 24/25, and a stored verdict the
/// set does not reproduce refuses before any kernel validate
/// runs. Only then kernel `commit` runs at the state's clock:
/// it re-runs the entire predicate a third time, and a
/// non-clear mask refuses with `CommitFailed` — the kernel
/// leaves its life `Committable`, and the program mirrors it:
/// the record stands `DECLARED`, not committed, not blocked.
/// On success the kernel's returned `SystemState` is asserted
/// against the program's own expectation — index exactly one
/// past the old head, hash exactly the record's declared next
/// hash — before any write; only then does the stage advance
/// to `COMMITTED` and the sealed head move. The capacity
/// debit and dialect effects of `commit_apply` run in the
/// rebuilt kernel, which is discarded — but slice 27 persists
/// the debit as ledger totals on `KernelState` (see the
/// slice-27 module docs): the new transition's per-resource
/// claim sums are checked against the sealed stocks minus
/// prior committed claims before kernel `commit` runs, and
/// the new totals are written with the head move. The payer
/// is a fee payer only: kernel `commit` is permissionless,
/// so no proposer, seat, or agent gate applies here.
pub fn commit_transition_from_records(
    kernel_state: &mut KernelState,
    transition_record: &mut TransitionRecord,
    observation_record: &ObservationRecord,
    prior_signals: &[SignalRecord],
    prior_plans: &[PlanRecord],
    prior_commits: &[PriorCommit],
) -> Result<Life> {
    if !kernel_state.sealed {
        return Err(error!(ProgramV04Error::NotSealed));
    }
    if transition_record.stage != TransitionRecord::DECLARED {
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
    if is_gov_type(&transition_record.transition_type) {
        return Err(error!(ProgramV04Error::GovernanceCommitRequired));
    }
    if transition_record.planning_deadline == 0 {
        return Err(error!(ProgramV04Error::CommitFailed));
    }
    let open_clock = transition_record
        .planning_deadline
        .checked_sub(launch_policy().planning_window)
        .ok_or(error!(ProgramV04Error::CommitFailed))?;
    if prior_signals.len() as u64 != transition_record.signal_count {
        return Err(error!(ProgramV04Error::CommitFailed));
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
                return Err(error!(ProgramV04Error::CommitFailed));
            }
        }
    }
    if prior_plans.len() as u64 != transition_record.plan_count {
        return Err(error!(ProgramV04Error::CommitFailed));
    }
    for prior in prior_plans {
        if prior.transition_id != transition_record.transition_id {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        if prior.claim_count as usize > PlanRecord::MAX_PLAN_CLAIMS {
            return Err(error!(ProgramV04Error::CommitFailed));
        }
        let mut seen: BTreeSet<u16> = BTreeSet::new();
        for resource in &prior.claim_resources[..prior.claim_count as usize] {
            if !seen.insert(*resource) {
                return Err(error!(ProgramV04Error::CommitFailed));
            }
        }
    }
    let proposer = resolve_proposer(kernel_state, transition_record.proposer)
        .map_err(|_| error!(ProgramV04Error::CommitFailed))?;
    let observer_id = resolve_agent_id(observation_record.observer);
    let mut k = kernel_at_head_from_commits(kernel_state, prior_commits)?;
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
        .map_err(|_| error!(ProgramV04Error::CommitFailed))?;
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
    .map_err(|_| error!(ProgramV04Error::CommitFailed))?;
    k.evaluate_transition(&transition_record.transition_id, eligible_at)
        .map_err(|_| error!(ProgramV04Error::CommitFailed))?;
    k.mark_eligible(&transition_record.transition_id, eligible_at)
        .map_err(|_| error!(ProgramV04Error::CommitFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Eligible) => {}
        _ => return Err(error!(ProgramV04Error::CommitFailed)),
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
        .map_err(|_| error!(ProgramV04Error::CommitFailed))?;
    }
    k.select_candidate(&transition_record.transition_id, open_clock)
        .map_err(|_| error!(ProgramV04Error::CommitFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Selected) => {}
        _ => return Err(error!(ProgramV04Error::CommitFailed)),
    }
    k.open_planning(&proposer, &transition_record.transition_id, open_clock)
        .map_err(|_| error!(ProgramV04Error::CommitFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Planning) => {}
        _ => return Err(error!(ProgramV04Error::CommitFailed)),
    }
    let deadline = open_clock
        .checked_add(launch_policy().planning_window)
        .ok_or(error!(ProgramV04Error::CommitFailed))?;
    if deadline != transition_record.planning_deadline {
        return Err(error!(ProgramV04Error::CommitFailed));
    }
    for prior in prior_plans {
        let mut prior_map: BTreeMap<u16, Amount> = BTreeMap::new();
        for (resource, amount) in prior.claim_resources[..prior.claim_count as usize]
            .iter()
            .zip(prior.claim_amounts[..prior.claim_count as usize].iter())
        {
            prior_map.insert(*resource, Amount::from_raw(*amount));
        }
        let prior_plan = Plan {
            agent: prior.agent.to_bytes(),
            claims: prior_map,
            expires_at: prior.expires_at,
        };
        k.submit_plan(
            &prior.agent.to_bytes(),
            &transition_record.transition_id,
            prior_plan,
            open_clock,
        )
        .map_err(|_| error!(ProgramV04Error::CommitFailed))?;
    }
    k.reconcile_plans(&transition_record.transition_id, kernel_state.clock)
        .map_err(|_| error!(ProgramV04Error::CommitFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Reconciling) => {}
        _ => return Err(error!(ProgramV04Error::CommitFailed)),
    }
    // Slice 23's exact mirror, recomputed over the supplied set:
    // a stored verdict the set does not reproduce refuses here,
    // before the kernel's own validate runs, exactly as in
    // slices 24/25 — neither the stored verdict nor the replay
    // is trusted alone.
    let mut sums: BTreeMap<u16, Amount> = BTreeMap::new();
    let mut fits = !prior_plans.is_empty();
    for prior in prior_plans {
        for (resource, amount) in prior.claim_resources[..prior.claim_count as usize]
            .iter()
            .zip(prior.claim_amounts[..prior.claim_count as usize].iter())
        {
            let entry = sums.entry(*resource).or_insert(Amount::ZERO);
            match entry.checked_add(Amount::from_raw(*amount)) {
                Ok(v) => *entry = v,
                Err(_) => fits = false,
            }
        }
    }
    if fits {
        for (resource, total) in &sums {
            if !k.capacity_of(*resource).is_some_and(|cap| *total <= cap) {
                fits = false;
            }
        }
    }
    if fits != transition_record.compatible {
        return Err(error!(ProgramV04Error::CommitFailed));
    }
    k.validate_transition(&transition_record.transition_id, kernel_state.clock)
        .map_err(|_| error!(ProgramV04Error::CommitFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Validated) => {}
        // Commit is not a verdict writer either: a replay
        // validate that yields Blocked means the stored
        // DECLARED label no longer holds at this clock, and
        // the record stands DECLARED — it is not re-labeled
        // BLOCKED by commit.
        Some(Life::Blocked) => return Err(error!(ProgramV04Error::CommitFailed)),
        _ => return Err(error!(ProgramV04Error::CommitFailed)),
    }
    k.declare_commit(&transition_record.transition_id, kernel_state.clock)
        .map_err(|_| error!(ProgramV04Error::CommitFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Committable) => {}
        _ => return Err(error!(ProgramV04Error::CommitFailed)),
    }
    // Slice 27 ledger gate: the rebuilt kernel judges the new
    // claims against the sealed stocks, but the program's live
    // capacity is the sealed stocks minus prior committed
    // claims. Refuse here — before kernel commit, before any
    // program write — when any resource's committed total plus
    // the new sum would exceed its sealed stock. The `sums`
    // map is the transition's own per-resource claim totals,
    // recomputed from the supplied prior plans by the mirror
    // above; a resource with no ledger slot, or with no sealed
    // stock in the rebuilt kernel, refuses rather than
    // passing through unchecked.
    let mut new_committed = kernel_state.committed_claims;
    for (resource, total) in &sums {
        let slot = committed_ledger_index(*resource)
            .ok_or(error!(ProgramV04Error::CommitFailed))?;
        let cap = k
            .capacity_of(*resource)
            .ok_or(error!(ProgramV04Error::CommitFailed))?;
        let already = Amount::from_raw(new_committed[slot]);
        let next = already
            .checked_add(*total)
            .map_err(|_| error!(ProgramV04Error::CommitFailed))?;
        if next > cap {
            return Err(error!(ProgramV04Error::CommitFailed));
        }
        new_committed[slot] = next.raw();
    }
    let new_state = k
        .commit(&transition_record.transition_id, kernel_state.clock)
        .map_err(|_| error!(ProgramV04Error::CommitFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Committed) => {}
        _ => return Err(error!(ProgramV04Error::CommitFailed)),
    }
    // The head moves only on a kernel-proven commit whose
    // returned state matches the program's own expectation:
    // one past the old head, exactly the declared next hash.
    // Both are asserted before any program state is written,
    // so a mismatch refuses with the head and the record
    // exactly as they stood.
    let expected_index = kernel_state
        .head_index
        .checked_add(1)
        .ok_or(error!(ProgramV04Error::CommitFailed))?;
    if new_state.index != expected_index || new_state.hash != transition_record.declared_next_hash {
        return Err(error!(ProgramV04Error::CommitFailed));
    }
    transition_record.stage = TransitionRecord::COMMITTED;
    kernel_state.head_index = new_state.index;
    kernel_state.head_hash = new_state.hash;
    kernel_state.committed_claims = new_committed;
    Ok(Life::Committed)
}

/// Commit a declared governance transition (slice 28): `DECLARED`
/// (9) -> `COMMITTED` (10), and the sealed head moves to the
/// kernel's returned `SystemState` — or a refusal that leaves
/// the record standing `DECLARED` and the head unmoved. This
/// mirrors `commit_transition_from_records` step for step,
/// with the governance differences: `action` is the exact rule
/// change being committed (a helper argument, because no
/// stored field carries it yet), and `approvals` is the list
/// of council seats approving exactly that action — each seat
/// is approved at the stored `eligible_at` before registration,
/// and the kernel's unanimity gate judges whether every
/// sitting seat approved this exact (id, action) pair. A seat
/// approving a different action is, to that gate, a seat that
/// did not approve this one, so the missing-seat refusal
/// covers it. A non-governance record refuses at the gate
/// with `NotGovernanceTransition`, mirroring the ordinary
/// helper's `GovernanceCommitRequired` gate in reverse. The
/// action's application runs in the rebuilt kernel, which is
/// discarded: this helper persists the stage advance and the
/// head move, not the applied rule (see the slice-28 module
/// note — governance-state persistence is a later slice).
/// The ledger gate runs over the supplied plan sums exactly
/// as in slice 27.
#[allow(clippy::too_many_arguments)]
pub fn commit_governance_from_records(
    kernel_state: &mut KernelState,
    transition_record: &mut TransitionRecord,
    observation_record: &ObservationRecord,
    prior_signals: &[SignalRecord],
    prior_plans: &[PlanRecord],
    action: &GovAction,
    approvals: &[[u8; 32]],
    prior_commits: &[PriorCommit],
) -> Result<Life> {
    if !kernel_state.sealed {
        return Err(error!(ProgramV04Error::NotSealed));
    }
    if transition_record.stage != TransitionRecord::DECLARED {
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
    if !is_gov_type(&transition_record.transition_type) {
        return Err(error!(ProgramV04Error::NotGovernanceTransition));
    }
    if transition_record.planning_deadline == 0 {
        return Err(error!(ProgramV04Error::GovernanceCommitFailed));
    }
    let open_clock = transition_record
        .planning_deadline
        .checked_sub(launch_policy().planning_window)
        .ok_or(error!(ProgramV04Error::GovernanceCommitFailed))?;
    if prior_signals.len() as u64 != transition_record.signal_count {
        return Err(error!(ProgramV04Error::GovernanceCommitFailed));
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
                return Err(error!(ProgramV04Error::GovernanceCommitFailed));
            }
        }
    }
    if prior_plans.len() as u64 != transition_record.plan_count {
        return Err(error!(ProgramV04Error::GovernanceCommitFailed));
    }
    for prior in prior_plans {
        if prior.transition_id != transition_record.transition_id {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        if prior.claim_count as usize > PlanRecord::MAX_PLAN_CLAIMS {
            return Err(error!(ProgramV04Error::GovernanceCommitFailed));
        }
        let mut seen: BTreeSet<u16> = BTreeSet::new();
        for resource in &prior.claim_resources[..prior.claim_count as usize] {
            if !seen.insert(*resource) {
                return Err(error!(ProgramV04Error::GovernanceCommitFailed));
            }
        }
    }
    let proposer = resolve_proposer(kernel_state, transition_record.proposer)
        .map_err(|_| error!(ProgramV04Error::GovernanceCommitFailed))?;
    let observer_id = resolve_agent_id(observation_record.observer);
    let mut k = kernel_at_head_from_commits(kernel_state, prior_commits)?;
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
    // Council approvals precede registration, exactly as the
    // bridge's governance replay drives them: every listed
    // seat approves this exact (id, action) pair, and the
    // kernel refuses registration unless every sitting seat
    // has done so.
    for seat in approvals {
        k.approve_governance(seat, transition_record.transition_id, action.clone(), eligible_at)
            .map_err(|_| error!(ProgramV04Error::GovernanceCommitFailed))?;
    }
    k.ingest_observation(&observer_id, observation, eligible_at)
        .map_err(|_| error!(ProgramV04Error::GovernanceCommitFailed))?;
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
    k.register_governance_transition(
        &proposer,
        transition_record.transition_id,
        manifest,
        action.clone(),
        eligible_at,
    )
    .map_err(|_| error!(ProgramV04Error::GovernanceCommitFailed))?;
    k.evaluate_transition(&transition_record.transition_id, eligible_at)
        .map_err(|_| error!(ProgramV04Error::GovernanceCommitFailed))?;
    k.mark_eligible(&transition_record.transition_id, eligible_at)
        .map_err(|_| error!(ProgramV04Error::GovernanceCommitFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Eligible) => {}
        _ => return Err(error!(ProgramV04Error::GovernanceCommitFailed)),
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
        .map_err(|_| error!(ProgramV04Error::GovernanceCommitFailed))?;
    }
    k.select_candidate(&transition_record.transition_id, open_clock)
        .map_err(|_| error!(ProgramV04Error::GovernanceCommitFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Selected) => {}
        _ => return Err(error!(ProgramV04Error::GovernanceCommitFailed)),
    }
    k.open_planning(&proposer, &transition_record.transition_id, open_clock)
        .map_err(|_| error!(ProgramV04Error::GovernanceCommitFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Planning) => {}
        _ => return Err(error!(ProgramV04Error::GovernanceCommitFailed)),
    }
    let deadline = open_clock
        .checked_add(launch_policy().planning_window)
        .ok_or(error!(ProgramV04Error::GovernanceCommitFailed))?;
    if deadline != transition_record.planning_deadline {
        return Err(error!(ProgramV04Error::GovernanceCommitFailed));
    }
    for prior in prior_plans {
        let mut prior_map: BTreeMap<u16, Amount> = BTreeMap::new();
        for (resource, amount) in prior.claim_resources[..prior.claim_count as usize]
            .iter()
            .zip(prior.claim_amounts[..prior.claim_count as usize].iter())
        {
            prior_map.insert(*resource, Amount::from_raw(*amount));
        }
        let prior_plan = Plan {
            agent: prior.agent.to_bytes(),
            claims: prior_map,
            expires_at: prior.expires_at,
        };
        k.submit_plan(
            &prior.agent.to_bytes(),
            &transition_record.transition_id,
            prior_plan,
            open_clock,
        )
        .map_err(|_| error!(ProgramV04Error::GovernanceCommitFailed))?;
    }
    k.reconcile_plans(&transition_record.transition_id, kernel_state.clock)
        .map_err(|_| error!(ProgramV04Error::GovernanceCommitFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Reconciling) => {}
        _ => return Err(error!(ProgramV04Error::GovernanceCommitFailed)),
    }
    // Slice 23's exact mirror, recomputed over the supplied set,
    // exactly as in the ordinary commit helper: a stored verdict
    // the set does not reproduce refuses here, before the
    // kernel's own validate runs.
    let mut sums: BTreeMap<u16, Amount> = BTreeMap::new();
    let mut fits = !prior_plans.is_empty();
    for prior in prior_plans {
        for (resource, amount) in prior.claim_resources[..prior.claim_count as usize]
            .iter()
            .zip(prior.claim_amounts[..prior.claim_count as usize].iter())
        {
            let entry = sums.entry(*resource).or_insert(Amount::ZERO);
            match entry.checked_add(Amount::from_raw(*amount)) {
                Ok(v) => *entry = v,
                Err(_) => fits = false,
            }
        }
    }
    if fits {
        for (resource, total) in &sums {
            if !k.capacity_of(*resource).is_some_and(|cap| *total <= cap) {
                fits = false;
            }
        }
    }
    if fits != transition_record.compatible {
        return Err(error!(ProgramV04Error::GovernanceCommitFailed));
    }
    k.validate_transition(&transition_record.transition_id, kernel_state.clock)
        .map_err(|_| error!(ProgramV04Error::GovernanceCommitFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Validated) => {}
        // Governance commit is not a verdict writer either: a
        // replay validate that yields Blocked means the stored
        // DECLARED label no longer holds at this clock, and
        // the record stands DECLARED.
        Some(Life::Blocked) => return Err(error!(ProgramV04Error::GovernanceCommitFailed)),
        _ => return Err(error!(ProgramV04Error::GovernanceCommitFailed)),
    }
    k.declare_commit(&transition_record.transition_id, kernel_state.clock)
        .map_err(|_| error!(ProgramV04Error::GovernanceCommitFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Committable) => {}
        _ => return Err(error!(ProgramV04Error::GovernanceCommitFailed)),
    }
    // Slice 27 ledger gate, unchanged: the rebuilt kernel
    // judges the new claims against the sealed stocks, but
    // the program's live capacity is the sealed stocks minus
    // prior committed claims. Governance plans claim no stock
    // in the bridge shape, so the gate is a no-op there; it
    // still runs, so a claims-carrying governance plan is
    // ledger-bound identically rather than passed unchecked.
    let mut new_committed = kernel_state.committed_claims;
    for (resource, total) in &sums {
        let slot = committed_ledger_index(*resource)
            .ok_or(error!(ProgramV04Error::GovernanceCommitFailed))?;
        let cap = k
            .capacity_of(*resource)
            .ok_or(error!(ProgramV04Error::GovernanceCommitFailed))?;
        let already = Amount::from_raw(new_committed[slot]);
        let next = already
            .checked_add(*total)
            .map_err(|_| error!(ProgramV04Error::GovernanceCommitFailed))?;
        if next > cap {
            return Err(error!(ProgramV04Error::GovernanceCommitFailed));
        }
        new_committed[slot] = next.raw();
    }
    // The governance commit itself: the kernel checks the
    // action and applies it atomically with the commit, in
    // the rebuilt kernel. A failure here — an action that
    // does not check out at commit time, or a predicate that
    // no longer holds — refuses with the record standing
    // DECLARED, exactly as the ordinary commit's check.
    let new_state = k
        .commit_governance(&transition_record.transition_id, kernel_state.clock)
        .map_err(|_| error!(ProgramV04Error::GovernanceCommitFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Committed) => {}
        _ => return Err(error!(ProgramV04Error::GovernanceCommitFailed)),
    }
    // The head moves only on a kernel-proven commit whose
    // returned state matches the program's own expectation:
    // one past the old head, exactly the declared next hash.
    // Both are asserted before any program state is written,
    // so a mismatch refuses with the head and the record
    // exactly as they stood.
    let expected_index = kernel_state
        .head_index
        .checked_add(1)
        .ok_or(error!(ProgramV04Error::GovernanceCommitFailed))?;
    if new_state.index != expected_index || new_state.hash != transition_record.declared_next_hash {
        return Err(error!(ProgramV04Error::GovernanceCommitFailed));
    }
    transition_record.stage = TransitionRecord::COMMITTED;
    kernel_state.head_index = new_state.index;
    kernel_state.head_hash = new_state.hash;
    kernel_state.committed_claims = new_committed;
    Ok(Life::Committed)
}

/// One prior committed transition offered to a replay-from-head
/// (slice 29): the stored transition record (stage `COMMITTED`),
/// its observation record, and its stored signal and plan records.
/// All four are read, never written: the replay re-proves the
/// prior's commit in a rebuilt kernel, and the caller owns the
/// returned kernel. A bundle carries no governance action and no
/// council approvals, so a governance-typed prior refuses rather
/// than replaying without its commit path.
#[derive(Clone, Copy)]
pub struct PriorCommit<'a> {
    pub transition_record: &'a TransitionRecord,
    pub observation_record: &'a ObservationRecord,
    pub prior_signals: &'a [SignalRecord],
    pub prior_plans: &'a [PlanRecord],
}

/// One classified remaining account (slice 31): the instruction-side
/// parse of a raw `AccountInfo` into the record it holds, by
/// discriminator in the sibling instruction's order — Transition,
/// Observation, Signal, Plan — so a mis-typed account refuses with
/// `RecordMismatch` before any replay runs, exactly as the sibling
/// classifier already did for its own field.
#[derive(Debug)]
pub enum ParsedAccount {
    Transition(TransitionRecord),
    Observation(ObservationRecord),
    Signal(SignalRecord),
    Plan(PlanRecord),
}

/// Classify every remaining account, in order, into its record type.
/// An account that is none of the four record types refuses — there
/// is no "skip unknown" path, because an unparsed bundle member would
/// silently shorten a chain.
pub fn classify_remaining_accounts<'a>(
    accounts: &'a [AccountInfo<'a>],
) -> Result<alloc::vec::Vec<ParsedAccount>> {
    let mut parsed = alloc::vec::Vec::new();
    for acc in accounts {
        if let Ok(p) = Account::<TransitionRecord>::try_from(acc) {
            parsed.push(ParsedAccount::Transition(p.into_inner()));
            continue;
        }
        if let Ok(p) = Account::<ObservationRecord>::try_from(acc) {
            parsed.push(ParsedAccount::Observation(p.into_inner()));
            continue;
        }
        if let Ok(p) = Account::<SignalRecord>::try_from(acc) {
            parsed.push(ParsedAccount::Signal(p.into_inner()));
            continue;
        }
        let p = Account::<PlanRecord>::try_from(acc)
            .map_err(|_| error!(ProgramV04Error::RecordMismatch))?;
        parsed.push(ParsedAccount::Plan(p.into_inner()));
    }
    Ok(parsed)
}

/// One prior commit bundle, owned (slice 31): the stored records a
/// prior `COMMITTED` transition's replay needs — its transition and
/// observation records plus its signal and plan sets, in chain order.
#[derive(Debug)]
pub struct OwnedPriorBundle {
    pub transition_record: TransitionRecord,
    pub observation_record: ObservationRecord,
    pub signals: alloc::vec::Vec<SignalRecord>,
    pub plans: alloc::vec::Vec<PlanRecord>,
}

/// Split the first `count` prior bundles off the head of a classified
/// remaining-account list, returning the bundles and the tail — the
/// target's own evidence, which the caller parses exactly as before.
/// A bundle is self-delimiting: the transition record, then the
/// observation it names, then exactly its stored `signal_count`
/// signals, then exactly its stored `plan_count` plans. Every linkage
/// is checked at parse time: the observation id must equal the
/// transition's, and every signal and plan must name the bundle's
/// transition — a bundle that mis-links refuses with `RecordMismatch`
/// before any kernel replay runs. A bundle short of its counts, or a
/// count running past the list, refuses the same way; the stage gate
/// stays in `drive_prior_commit_to_commit` (a merely-declared bundle
/// is not history), not duplicated here.
pub fn split_prior_bundles(
    parsed: alloc::vec::Vec<ParsedAccount>,
    count: usize,
) -> Result<(alloc::vec::Vec<OwnedPriorBundle>, alloc::vec::Vec<ParsedAccount>)> {
    let mut iter = parsed.into_iter();
    let mut bundles = alloc::vec::Vec::new();
    for _ in 0..count {
        let transition_record = match iter.next() {
            Some(ParsedAccount::Transition(tr)) => tr,
            _ => return Err(error!(ProgramV04Error::RecordMismatch)),
        };
        let observation_record = match iter.next() {
            Some(ParsedAccount::Observation(obs))
                if obs.observation_id == transition_record.observation_id =>
            {
                obs
            }
            _ => return Err(error!(ProgramV04Error::RecordMismatch)),
        };
        let mut signals = alloc::vec::Vec::new();
        for _ in 0..transition_record.signal_count as usize {
            match iter.next() {
                Some(ParsedAccount::Signal(sig))
                    if sig.transition_id == transition_record.transition_id =>
                {
                    signals.push(sig)
                }
                _ => return Err(error!(ProgramV04Error::RecordMismatch)),
            }
        }
        let mut plans = alloc::vec::Vec::new();
        for _ in 0..transition_record.plan_count as usize {
            match iter.next() {
                Some(ParsedAccount::Plan(plan))
                    if plan.transition_id == transition_record.transition_id =>
                {
                    plans.push(plan)
                }
                _ => return Err(error!(ProgramV04Error::RecordMismatch)),
            }
        }
        bundles.push(OwnedPriorBundle {
            transition_record,
            observation_record,
            signals,
            plans,
        });
    }
    Ok((bundles, iter.collect()))
}

/// Build the helper-side `PriorCommit` views over owned bundles. The
/// views borrow the bundles, so they live only as long as the
/// instruction body that holds them — the helper call runs inside
/// that body, never past it.
pub fn prior_commits_of(bundles: &[OwnedPriorBundle]) -> alloc::vec::Vec<PriorCommit<'_>> {
    bundles
        .iter()
        .map(|b| PriorCommit {
            transition_record: &b.transition_record,
            observation_record: &b.observation_record,
            prior_signals: &b.signals,
            prior_plans: &b.plans,
        })
        .collect()
}

/// Drive one prior commit through its full ordinary chain inside a
/// rebuilt kernel that already stands at the prior's parent (slice 29).
/// Pure except for the kernel: every refusal leaves the caller to
/// discard it, so a failed history never becomes a head. Gates, in
/// order: the record stands `COMMITTED` (a merely-declared prior is
/// not history yet); the type is ordinary (governance priors refuse
/// with `GovernanceCommitRequired`, for want of an action and
/// approvals); observation linkage; a proven planning window — a
/// nonzero stored `planning_deadline` at or before the state clock,
/// with a derivable open clock — because the final steps replay at
/// that deadline, the clock the chain originally proved at (see the
/// slice-29 module note for why the current state clock is the wrong
/// clock for history); the signal and plan completeness anchors, the
/// stored rounds, and the per-plan claim uniqueness, exactly as the
/// slice-26 commit helper checks them. The chain itself mirrors that
/// helper step for step — ingest + register + evaluate +
/// mark_eligible at `eligible_at`, signals in `submitted_at` order,
/// select + open_planning at the derived open clock with the
/// replayed deadline checked equal to the stored deadline, plans at
/// the open clock, then reconcile + validate + declare + commit at
/// the planning deadline — except that the capacity mirror judges
/// the plan sums against the kernel's *current* stocks, which already
/// carry every earlier prior's debit, so a second commit is judged
/// against what the first consumed, not against the sealed stocks
/// alone. The slice-27 program ledger is deliberately not consulted
/// here: it lives on `KernelState`, and this helper reconstructs
/// kernel state, not program state. The returned state must land
/// exactly one past the prior's parent at its declared next hash,
/// or the history refuses.
fn drive_prior_commit_to_commit(
    kernel_state: &KernelState,
    k: &mut LaunchKernelProgramEnv,
    prior: &PriorCommit,
) -> Result<SystemState> {
    let transition_record = prior.transition_record;
    let observation_record = prior.observation_record;
    let prior_signals = prior.prior_signals;
    let prior_plans = prior.prior_plans;
    if transition_record.stage != TransitionRecord::COMMITTED {
        return Err(error!(ProgramV04Error::WrongStage));
    }
    if is_gov_type(&transition_record.transition_type) {
        return Err(error!(ProgramV04Error::GovernanceCommitRequired));
    }
    if transition_record.observation_id != observation_record.observation_id {
        return Err(error!(ProgramV04Error::RecordMismatch));
    }
    if transition_record.planning_deadline == 0
        || transition_record.planning_deadline > kernel_state.clock
    {
        return Err(error!(ProgramV04Error::CommitFailed));
    }
    let open_clock = transition_record
        .planning_deadline
        .checked_sub(launch_policy().planning_window)
        .ok_or(error!(ProgramV04Error::CommitFailed))?;
    if prior_signals.len() as u64 != transition_record.signal_count {
        return Err(error!(ProgramV04Error::CommitFailed));
    }
    for prior_signal in prior_signals {
        if prior_signal.transition_id != transition_record.transition_id {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        if signal_round(
            transition_record.eligible_at,
            transition_record.consensus_deadline,
            prior_signal.submitted_at,
        ) != prior_signal.round
        {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
    }
    {
        let mut seen: BTreeSet<([u8; 32], u64)> = BTreeSet::new();
        for prior_signal in prior_signals {
            if !seen.insert((prior_signal.agent.to_bytes(), prior_signal.round)) {
                return Err(error!(ProgramV04Error::CommitFailed));
            }
        }
    }
    if prior_plans.len() as u64 != transition_record.plan_count {
        return Err(error!(ProgramV04Error::CommitFailed));
    }
    for prior_plan in prior_plans {
        if prior_plan.transition_id != transition_record.transition_id {
            return Err(error!(ProgramV04Error::RecordMismatch));
        }
        if prior_plan.claim_count as usize > PlanRecord::MAX_PLAN_CLAIMS {
            return Err(error!(ProgramV04Error::CommitFailed));
        }
        let mut seen: BTreeSet<u16> = BTreeSet::new();
        for resource in &prior_plan.claim_resources[..prior_plan.claim_count as usize] {
            if !seen.insert(*resource) {
                return Err(error!(ProgramV04Error::CommitFailed));
            }
        }
    }
    let proposer = resolve_proposer(kernel_state, transition_record.proposer)
        .map_err(|_| error!(ProgramV04Error::CommitFailed))?;
    let observer_id = resolve_agent_id(observation_record.observer);
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
        .map_err(|_| error!(ProgramV04Error::CommitFailed))?;
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
    .map_err(|_| error!(ProgramV04Error::CommitFailed))?;
    k.evaluate_transition(&transition_record.transition_id, eligible_at)
        .map_err(|_| error!(ProgramV04Error::CommitFailed))?;
    k.mark_eligible(&transition_record.transition_id, eligible_at)
        .map_err(|_| error!(ProgramV04Error::CommitFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Eligible) => {}
        _ => return Err(error!(ProgramV04Error::CommitFailed)),
    }
    let mut priors: alloc::vec::Vec<SignalRecord> = prior_signals.to_vec();
    priors.sort_by_key(|s| s.submitted_at);
    for prior_signal in &priors {
        let support = if prior_signal.has_support {
            Some(Amount::from_raw(prior_signal.support_raw))
        } else {
            None
        };
        k.submit_round_signal(
            &prior_signal.agent.to_bytes(),
            &transition_record.transition_id,
            Amount::from_raw(prior_signal.raw),
            support,
            prior_signal.submitted_at,
        )
        .map_err(|_| error!(ProgramV04Error::CommitFailed))?;
    }
    k.select_candidate(&transition_record.transition_id, open_clock)
        .map_err(|_| error!(ProgramV04Error::CommitFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Selected) => {}
        _ => return Err(error!(ProgramV04Error::CommitFailed)),
    }
    k.open_planning(&proposer, &transition_record.transition_id, open_clock)
        .map_err(|_| error!(ProgramV04Error::CommitFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Planning) => {}
        _ => return Err(error!(ProgramV04Error::CommitFailed)),
    }
    let deadline = open_clock
        .checked_add(launch_policy().planning_window)
        .ok_or(error!(ProgramV04Error::CommitFailed))?;
    if deadline != transition_record.planning_deadline {
        return Err(error!(ProgramV04Error::CommitFailed));
    }
    for prior_plan in prior_plans {
        let mut prior_map: BTreeMap<u16, Amount> = BTreeMap::new();
        for (resource, amount) in prior_plan.claim_resources[..prior_plan.claim_count as usize]
            .iter()
            .zip(prior_plan.claim_amounts[..prior_plan.claim_count as usize].iter())
        {
            prior_map.insert(*resource, Amount::from_raw(*amount));
        }
        let prior_plan_kernel = Plan {
            agent: prior_plan.agent.to_bytes(),
            claims: prior_map,
            expires_at: prior_plan.expires_at,
        };
        k.submit_plan(
            &prior_plan.agent.to_bytes(),
            &transition_record.transition_id,
            prior_plan_kernel,
            open_clock,
        )
        .map_err(|_| error!(ProgramV04Error::CommitFailed))?;
    }
    let final_clock = transition_record.planning_deadline;
    k.reconcile_plans(&transition_record.transition_id, final_clock)
        .map_err(|_| error!(ProgramV04Error::CommitFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Reconciling) => {}
        _ => return Err(error!(ProgramV04Error::CommitFailed)),
    }
    // Slice 23's exact mirror, recomputed over the supplied set and
    // judged against the kernel's current stocks (post-earlier-priors):
    // a stored verdict the set does not reproduce refuses here,
    // before the kernel's own validate runs, exactly as in slices 24/25.
    let mut sums: BTreeMap<u16, Amount> = BTreeMap::new();
    let mut fits = !prior_plans.is_empty();
    for prior_plan in prior_plans {
        for (resource, amount) in prior_plan.claim_resources[..prior_plan.claim_count as usize]
            .iter()
            .zip(prior_plan.claim_amounts[..prior_plan.claim_count as usize].iter())
        {
            let entry = sums.entry(*resource).or_insert(Amount::ZERO);
            match entry.checked_add(Amount::from_raw(*amount)) {
                Ok(v) => *entry = v,
                Err(_) => fits = false,
            }
        }
    }
    if fits {
        for (resource, total) in &sums {
            if !k.capacity_of(*resource).is_some_and(|cap| *total <= cap) {
                fits = false;
            }
        }
    }
    if fits != transition_record.compatible {
        return Err(error!(ProgramV04Error::CommitFailed));
    }
    k.validate_transition(&transition_record.transition_id, final_clock)
        .map_err(|_| error!(ProgramV04Error::CommitFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Validated) => {}
        Some(Life::Blocked) => return Err(error!(ProgramV04Error::CommitFailed)),
        _ => return Err(error!(ProgramV04Error::CommitFailed)),
    }
    k.declare_commit(&transition_record.transition_id, final_clock)
        .map_err(|_| error!(ProgramV04Error::CommitFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Committable) => {}
        _ => return Err(error!(ProgramV04Error::CommitFailed)),
    }
    let new_state = k
        .commit(&transition_record.transition_id, final_clock)
        .map_err(|_| error!(ProgramV04Error::CommitFailed))?;
    match k.life_of(&transition_record.transition_id) {
        Some(Life::Committed) => {}
        _ => return Err(error!(ProgramV04Error::CommitFailed)),
    }
    let expected_index = transition_record
        .parent_index
        .checked_add(1)
        .ok_or(error!(ProgramV04Error::CommitFailed))?;
    if new_state.index != expected_index || new_state.hash != transition_record.declared_next_hash {
        return Err(error!(ProgramV04Error::CommitFailed));
    }
    Ok(new_state)
}

/// Rebuild the sealed launch kernel at the program's current head
/// (slice 29): start from the sealed genesis kernel and drive each
/// supplied prior commit, in order, through
/// `drive_prior_commit_to_commit`, checking each prior's parent
/// against the running head before it drives. Pure: the returned
/// kernel is a fresh object, and a refusal returns nothing — the
/// caller discards the partially advanced kernel, so no partial
/// history is ever mistaken for the head. An empty prior set
/// returns the genesis kernel, and it only passes when the
/// program's head IS genesis: a moved head with no supplied
/// history refuses with `ParentMismatch`, because the chain that
/// moved it is missing. Likewise a supplied chain that stops
/// short of, or overshoots, the program's head refuses with
/// `ParentMismatch` at the end, not with a kernel at the wrong
/// head. The kernel's internal clock after the last prior is that
/// prior's planning deadline — the next transition's window must
/// open after it (see the slice-29 module note).
pub fn kernel_at_head_from_commits(
    kernel_state: &KernelState,
    prior_commits: &[PriorCommit],
) -> Result<LaunchKernelProgramEnv> {
    if !kernel_state.sealed {
        return Err(error!(ProgramV04Error::NotSealed));
    }
    let mut k = sealed_launch_kernel_env().map_err(|_| error!(ProgramV04Error::KernelSeal))?;
    let mut running_index = k.declared_state().index;
    let mut running_hash = k.declared_state().hash;
    for prior in prior_commits {
        if prior.transition_record.parent_index != running_index
            || prior.transition_record.parent_hash != running_hash
        {
            return Err(error!(ProgramV04Error::ParentMismatch));
        }
        let new_state = drive_prior_commit_to_commit(kernel_state, &mut k, prior)?;
        running_index = new_state.index;
        running_hash = new_state.hash;
    }
    if running_index != kernel_state.head_index || running_hash != kernel_state.head_hash {
        return Err(error!(ProgramV04Error::ParentMismatch));
    }
    Ok(k)
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
    prior_commits: &[PriorCommit],
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
    let mut k = kernel_at_head_from_commits(kernel_state, prior_commits)?;
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
        committed_claims: [0, 0, 0],
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
pub struct GovernanceTransitionRegistered {
    pub transition_id: [u8; 32],
    pub parent_index: u64,
    pub proposer: Pubkey,
}

#[event]
pub struct GovernanceApproved {
    pub transition_id: [u8; 32],
    pub seat: Pubkey,
    pub action_kind: u8,
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

#[event]
pub struct PlanSubmitted {
    pub transition_id: [u8; 32],
    pub agent: Pubkey,
    pub expires_at: u64,
    pub plan_count: u64,
}

#[event]
pub struct TransitionReconciled {
    pub transition_id: [u8; 32],
    pub stage: u8,
    pub compatible: bool,
    pub plan_count: u64,
}

#[event]
pub struct TransitionValidated {
    pub transition_id: [u8; 32],
    pub stage: u8,
    pub compatible: bool,
}

#[event]
pub struct TransitionDeclared {
    pub transition_id: [u8; 32],
    pub stage: u8,
}

#[event]
pub struct TransitionCommitted {
    pub transition_id: [u8; 32],
    pub stage: u8,
    pub head_index: u64,
    pub head_hash: [u8; 32],
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
    #[msg("program-v04 planning: Blocked re-plan refused — caller is neither the proposer nor a council seat, or the re-plan cap is reached")]
    ReplanRefused,
    #[msg("program-v04 plan: kernel plan refused, planning window unproven, or supplied plan set incomplete")]
    PlanFailed,
    #[msg("program-v04 reconcile: kernel reconcile refused, planning window still open or unproven, or supplied plan set incomplete")]
    ReconcileFailed,
    #[msg("program-v04 validate: kernel validate refused, reconciled chain unproven, stored verdict unreproduced, or record still blocked")]
    ValidateFailed,
    #[msg("program-v04 declare: kernel declare refused, validated chain unproven, stored verdict unreproduced, or full predicate failed at declare time")]
    DeclareFailed,
    #[msg("program-v04 commit: kernel commit refused, declared chain unproven, stored verdict unreproduced, or full predicate failed at commit time")]
    CommitFailed,
    #[msg("program-v04 commit: governance transitions commit through commit_governance, not the ordinary commit path")]
    GovernanceCommitRequired,
    #[msg("program-v04 governance commit: transition is not a governance transition")]
    NotGovernanceTransition,
    #[msg("program-v04 governance commit: kernel governance commit refused, declared chain unproven, stored verdict unreproduced, action check failed, or full predicate failed at commit time")]
    GovernanceCommitFailed,
    #[msg("program-v04 register: governance types register through register_governance_transition, not register_transition")]
    GovernanceRegisterRequired,
    #[msg("program-v04 governance register: transition type is not a governance type")]
    GovernanceTypeRequired,
    #[msg("program-v04 governance register: proposer is not a seated council member of the sealed profile")]
    NotCouncilSeat,
    #[msg("program-v04 governance approval: action category does not match the record's governance type")]
    GovActionCategoryMismatch,
    #[msg("program-v04 governance approval: action list exceeds the stored-action cap")]
    GovActionTooLarge,
    #[msg("program-v04 governance approval: stored action does not decode to a kernel action")]
    GovActionDecode,
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

/// Ledger slot of a launch resource in
/// `KernelState.committed_claims` (slice 27): tranche 0, lending
/// book 1, exit window 2. Any other resource has no slot — the
/// kernel's own commit refuses it for want of a capacity stock,
/// and the ledger gate refuses it first, so no unslotted claim
/// can ever be written to the ledger.
pub fn committed_ledger_index(resource: u16) -> Option<usize> {
    match resource {
        RES_TREASURY_TRANCHE => Some(0),
        RES_LENDING_BOOK => Some(1),
        RES_EXIT_WINDOW => Some(2),
        _ => None,
    }
}

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
        // census count 8 + bump 1 + committed claims 24 (3 x u64).
        assert_eq!(KernelState::SPACE, 8 + 1 + 8 + 32 + 8 + 8 + 32 + 8 + 1 + 24);
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
            committed_claims: [11, 22, 33],
        };
        let mut bytes = Vec::new();
        state.try_serialize(&mut bytes).expect("serialize must succeed");
        // Discriminator (8) + fields (122) = SPACE.
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
        assert_eq!(back.committed_claims, [11, 22, 33]);
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
        // planning_deadline 8 + signal_count 8 + plan_count 8 + replans 4 + compatible 1 + bump 1.
        assert_eq!(
            TransitionRecord::SPACE,
            8 + 32 + 32 + 8 + 32 + 32 + 32 + 8 + 32 + 1 + 8 + 8 + 8 + 8 + 8 + 4 + 1 + 1
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
            plan_count: 1,
            replans: 2,
            compatible: false,
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
        assert_eq!(back.plan_count, 1);
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
    fn build_governance_transition_record_accepts_seat_at_sealed_head() {
        // Slice 32: a launch-council seat (ag(1)) registering a
        // governance-typed transition at the sealed genesis head
        // builds a REGISTERED record carrying the gov type, the
        // head parent, and the seat proposer.
        let state = sealed_state();
        let (id, _, parent_index, parent_hash, declared, obs, expires) = record_args();
        let record = build_governance_transition_record(
            &state, id, T_GOV_ADJUSTABLE, parent_index, parent_hash, declared, obs, expires,
            Pubkey::new_from_array(ag(1)), 7,
        )
        .expect("seat governance register must build");
        assert_eq!(record.transition_id, [0xD1; 32]);
        assert_eq!(record.transition_type, T_GOV_ADJUSTABLE);
        assert_eq!(record.parent_index, 0);
        assert_eq!(record.declared_next_hash, [0xE1; 32]);
        assert_eq!(record.stage, TransitionRecord::REGISTERED);
        assert_eq!(record.proposer, Pubkey::new_from_array(ag(1)));
        assert_eq!(record.bump, 7);
    }

    #[test]
    fn build_governance_transition_record_refuses_non_gov_type() {
        // The governance path admits governance types only: an
        // ordinary type refuses, and the ordinary path is its
        // only route (see the builder test below).
        let state = sealed_state();
        let (id, ttype, parent_index, parent_hash, declared, obs, expires) = record_args();
        let err = build_governance_transition_record(
            &state, id, ttype, parent_index, parent_hash, declared, obs, expires,
            Pubkey::new_from_array(ag(1)), 1,
        )
        .expect_err("ordinary type must refuse governance register");
        assert!(format!("{:?}", err).contains("GovernanceTypeRequired"));
    }

    #[test]
    fn build_governance_transition_record_refuses_non_seat_agent() {
        // ag(4) is a seeded, active agent but not a council seat:
        // the seat gate, not mere registration, is what the
        // governance path requires.
        let state = sealed_state();
        let (id, _, parent_index, parent_hash, declared, obs, expires) = record_args();
        let err = build_governance_transition_record(
            &state, id, T_GOV_COUNCIL, parent_index, parent_hash, declared, obs, expires,
            Pubkey::new_from_array(ag(4)), 1,
        )
        .expect_err("non-seat agent must refuse governance register");
        assert!(format!("{:?}", err).contains("NotCouncilSeat"));
    }

    #[test]
    fn build_governance_transition_record_refuses_unknown_proposer() {
        // An unknown pubkey is not a seat either: the gate judges
        // the pinned profile, and an outsider is refused outright.
        let state = sealed_state();
        let (id, _, parent_index, parent_hash, declared, obs, expires) = record_args();
        let err = build_governance_transition_record(
            &state, id, T_GOV_ADJUSTABLE, parent_index, parent_hash, declared, obs, expires,
            Pubkey::new_from_array([0x99; 32]), 1,
        )
        .expect_err("unknown proposer must refuse governance register");
        assert!(format!("{:?}", err).contains("NotCouncilSeat"));
    }

    #[test]
    fn build_governance_transition_record_refuses_unsealed() {
        let state = genesis_kernel_state().expect("genesis must construct");
        let (id, _, parent_index, parent_hash, declared, obs, expires) = record_args();
        let err = build_governance_transition_record(
            &state, id, T_GOV_ADJUSTABLE, parent_index, parent_hash, declared, obs, expires,
            Pubkey::new_from_array(ag(1)), 1,
        )
        .expect_err("unsealed governance register must refuse");
        assert!(format!("{:?}", err).contains("NotSealed"));
    }

    #[test]
    fn build_governance_transition_record_refuses_parent_mismatch() {
        let state = sealed_state();
        let (id, _, _, parent_hash, declared, obs, expires) = record_args();
        let err = build_governance_transition_record(
            &state, id, T_GOV_ADJUSTABLE, 1, parent_hash, declared, obs, expires,
            Pubkey::new_from_array(ag(1)), 1,
        )
        .expect_err("wrong parent must refuse governance register");
        assert!(format!("{:?}", err).contains("ParentMismatch"));
    }

    #[test]
    fn build_transition_record_refuses_gov_type() {
        // Slice 32: the ordinary path and the governance path are
        // disjoint — a governance type through the ordinary
        // builder refuses and names the governance instruction.
        let state = sealed_state();
        let (id, _, parent_index, parent_hash, declared, obs, expires) = record_args();
        let err = build_transition_record(
            &state, id, T_GOV_ADJUSTABLE, parent_index, parent_hash, declared, obs, expires,
            Pubkey::new_from_array(ag(1)), 1,
        )
        .expect_err("gov type must refuse ordinary register");
        assert!(format!("{:?}", err).contains("GovernanceRegisterRequired"));
    }

    fn gov_record(ttype: [u8; 32]) -> TransitionRecord {
        // Slice 33 helper: a REGISTERED governance record at the
        // sealed head, proposed by seat ag(1).
        let state = sealed_state();
        let (id, _, parent_index, parent_hash, declared, obs, expires) = record_args();
        build_governance_transition_record(
            &state, id, ttype, parent_index, parent_hash, declared, obs, expires,
            Pubkey::new_from_array(ag(1)), 7,
        )
        .expect("seat governance record must build")
    }

    fn all_gov_actions() -> alloc::vec::Vec<GovAction> {
        // One of every kernel GovAction variant, in declaration
        // order — the round-trip corpus for slice 33.
        alloc::vec![
            GovAction::GrantRole { who: ag(2), role: Role::Observer },
            GovAction::RevokeRole { who: ag(2), role: Role::Executor },
            GovAction::SetPolicy { ttype: TT, policy: launch_policy() },
            GovAction::SetRequiredConstraints {
                ttype: TT,
                set: alloc::collections::BTreeSet::from([C1, [9; 32]]),
            },
            GovAction::SetRequiredObjectives {
                ttype: TT,
                set: alloc::collections::BTreeSet::from([O1]),
            },
            GovAction::SetRequiredCapabilities {
                ttype: TT,
                reqs: alloc::vec![(C1, Amount::from_raw(2 * SCALE))],
            },
            GovAction::SetAgentCapability { agent: ag(2), cap: C1, held: true },
            GovAction::SetCapacity { resource: RES_EXIT_WINDOW, capacity: Amount::from_raw(400 * SCALE) },
            GovAction::SetGroupMultiplier { ttype: TT, group: 3, mult: Amount::from_raw(2 * SCALE) },
            GovAction::SetMaxInfluenceShare { share: Amount::from_raw(SCALE / 2) },
            GovAction::RegisterAgent {
                agent: ag(5),
                groups: alloc::collections::BTreeSet::from([1, 2]),
                base_influence: Amount::from_raw(2 * SCALE),
            },
            GovAction::SetAgentActive { agent: ag(4), active: false },
            GovAction::SetCouncilSeats { seats: 5 },
            GovAction::InstallCouncil { seats: alloc::vec![ag(1), ag(2), ag(3)] },
            GovAction::Reboot,
        ]
    }

    #[test]
    fn stored_gov_action_round_trips_every_variant() {
        // Slice 33: encoding then decoding returns the exact
        // action for every variant, and the 15 kinds are distinct —
        // the later governance replay can trust stored approvals
        // to rebuild the action the seat approved.
        let actions = all_gov_actions();
        assert_eq!(actions.len(), 15);
        let mut kinds = alloc::collections::BTreeSet::new();
        for action in &actions {
            let stored = StoredGovAction::encode(action).expect("in-cap action must encode");
            assert!(kinds.insert(stored.kind), "kinds must be distinct");
            assert_eq!(stored.decode().expect("stored action must decode"), *action);
        }
        assert_eq!(kinds.len(), 15);
    }

    #[test]
    fn build_governance_approval_accepts_seat_and_matching_category() {
        // Slice 33: seat ag(1) approving a SetCapacity action on an
        // ADJUSTABLE record builds a record carrying the transition
        // id, the seat pubkey, and kind 7 — the exact action, stored.
        let state = sealed_state();
        let record = gov_record(T_GOV_ADJUSTABLE);
        let action = GovAction::SetCapacity {
            resource: RES_EXIT_WINDOW,
            capacity: Amount::from_raw(400 * SCALE),
        };
        let approval = build_governance_approval_record(
            &state, &record, Pubkey::new_from_array(ag(1)), &action, 9,
        )
        .expect("seat approval of a matching action must build");
        assert_eq!(approval.transition_id, [0xD1; 32]);
        assert_eq!(approval.seat, Pubkey::new_from_array(ag(1)));
        assert_eq!(approval.action.kind, 7);
        assert_eq!(approval.action.decode().expect("stored action must decode"), action);
        assert_eq!(approval.bump, 9);
    }

    #[test]
    fn build_governance_approval_refuses_non_gov_record() {
        // Approvals belong to governance records only: an ordinary
        // record refuses with the governance-type error.
        let state = sealed_state();
        let (id, ttype, parent_index, parent_hash, declared, obs, expires) = record_args();
        let ordinary = build_transition_record(
            &state, id, ttype, parent_index, parent_hash, declared, obs, expires,
            Pubkey::new_from_array(ag(1)), 1,
        )
        .expect("ordinary record must build");
        let action = GovAction::SetCapacity {
            resource: RES_EXIT_WINDOW,
            capacity: Amount::from_raw(400 * SCALE),
        };
        let err = build_governance_approval_record(
            &state, &ordinary, Pubkey::new_from_array(ag(1)), &action, 1,
        )
        .expect_err("ordinary record must refuse approval");
        assert!(format!("{:?}", err).contains("GovernanceTypeRequired"));
    }

    #[test]
    fn build_governance_approval_refuses_non_seat_agent() {
        // ag(4) is seeded and active but not a council seat: the
        // seat gate, not mere registration, is what approval needs.
        let state = sealed_state();
        let record = gov_record(T_GOV_ADJUSTABLE);
        let action = GovAction::SetCapacity {
            resource: RES_EXIT_WINDOW,
            capacity: Amount::from_raw(400 * SCALE),
        };
        let err = build_governance_approval_record(
            &state, &record, Pubkey::new_from_array(ag(4)), &action, 1,
        )
        .expect_err("non-seat agent must refuse approval");
        assert!(format!("{:?}", err).contains("NotCouncilSeat"));
    }

    #[test]
    fn build_governance_approval_refuses_unknown_approver() {
        let state = sealed_state();
        let record = gov_record(T_GOV_ADJUSTABLE);
        let action = GovAction::SetCapacity {
            resource: RES_EXIT_WINDOW,
            capacity: Amount::from_raw(400 * SCALE),
        };
        let err = build_governance_approval_record(
            &state, &record, Pubkey::new_from_array([0x99; 32]), &action, 1,
        )
        .expect_err("unknown approver must refuse approval");
        assert!(format!("{:?}", err).contains("NotCouncilSeat"));
    }

    #[test]
    fn build_governance_approval_refuses_unsealed() {
        let state = genesis_kernel_state().expect("genesis must construct");
        let record = gov_record(T_GOV_ADJUSTABLE);
        let action = GovAction::SetCapacity {
            resource: RES_EXIT_WINDOW,
            capacity: Amount::from_raw(400 * SCALE),
        };
        let err = build_governance_approval_record(
            &state, &record, Pubkey::new_from_array(ag(1)), &action, 1,
        )
        .expect_err("unsealed approval must refuse");
        assert!(format!("{:?}", err).contains("NotSealed"));
    }

    #[test]
    fn build_governance_approval_refuses_wrong_stage() {
        // Approvals are collected while the proposal is REGISTERED:
        // once it has advanced, the approval set is frozen.
        let state = sealed_state();
        let mut record = gov_record(T_GOV_ADJUSTABLE);
        record.stage = TransitionRecord::KERNEL_REGISTERED;
        let action = GovAction::SetCapacity {
            resource: RES_EXIT_WINDOW,
            capacity: Amount::from_raw(400 * SCALE),
        };
        let err = build_governance_approval_record(
            &state, &record, Pubkey::new_from_array(ag(1)), &action, 1,
        )
        .expect_err("advanced record must refuse approval");
        assert!(format!("{:?}", err).contains("WrongStage"));
    }

    #[test]
    fn build_governance_approval_refuses_category_mismatch() {
        // A SetCapacity action travels under ADJUSTABLE, never under
        // COUNCIL: the category gate refuses at approval time, so no
        // stored approval can carry an action its record cannot.
        let state = sealed_state();
        let record = gov_record(T_GOV_COUNCIL);
        let action = GovAction::SetCapacity {
            resource: RES_EXIT_WINDOW,
            capacity: Amount::from_raw(400 * SCALE),
        };
        let err = build_governance_approval_record(
            &state, &record, Pubkey::new_from_array(ag(1)), &action, 1,
        )
        .expect_err("category mismatch must refuse approval");
        assert!(format!("{:?}", err).contains("GovActionCategoryMismatch"));
    }

    #[test]
    fn stored_gov_action_encode_refuses_oversize_lists() {
        // Over-cap collections refuse rather than truncate: 17 ids
        // over the 16-set cap, 9 capability pairs over the 8-pair
        // cap, 16 council seats over the 15-seat cap.
        let big_set: alloc::collections::BTreeSet<[u8; 32]> =
            (0..17u8).map(|i| [i; 32]).collect();
        let err = StoredGovAction::encode(&GovAction::SetRequiredConstraints {
            ttype: TT,
            set: big_set,
        })
        .expect_err("17-id set must refuse encode");
        assert!(format!("{:?}", err).contains("GovActionTooLarge"));
        let big_reqs: alloc::vec::Vec<([u8; 32], Amount)> =
            (0..9u8).map(|i| ([i; 32], Amount::from_raw(SCALE))).collect();
        let err = StoredGovAction::encode(&GovAction::SetRequiredCapabilities {
            ttype: TT,
            reqs: big_reqs,
        })
        .expect_err("9-pair list must refuse encode");
        assert!(format!("{:?}", err).contains("GovActionTooLarge"));
        let big_council: alloc::vec::Vec<[u8; 32]> = (0..16u8).map(|i| [i; 32]).collect();
        let err = StoredGovAction::encode(&GovAction::InstallCouncil { seats: big_council })
            .expect_err("16-seat list must refuse encode");
        assert!(format!("{:?}", err).contains("GovActionTooLarge"));
    }

    #[test]
    fn stored_gov_action_decode_refuses_corrupt_values() {
        // An unknown kind and an over-cap count both refuse decode
        // instead of producing a wrong action.
        let stored = StoredGovAction { kind: 99, ..StoredGovAction::default() };
        let err = stored.decode().expect_err("unknown kind must refuse decode");
        assert!(format!("{:?}", err).contains("GovActionDecode"));
        let stored = StoredGovAction { kind: 3, set_count: 17, ..StoredGovAction::default() };
        let err = stored.decode().expect_err("over-cap count must refuse decode");
        assert!(format!("{:?}", err).contains("GovActionDecode"));
    }

    #[test]
    fn gov_approval_record_space_accounts_for_every_field() {
        // Discriminator 8 + transition 32 + seat 32 + StoredGovAction
        // 1524 (kind 1 + ttype 32 + agent 32 + role 1 + resource 2 +
        // group 2 + cap 32 + amount 8 + flag 1 + seats 4 + policy 61 +
        // set ids 512 + set count 1 + group ids 32 + group count 1 +
        // cap ids 256 + cap amounts 64 + cap count 1 + council seats
        // 480 + council count 1) + bump 1.
        assert_eq!(
            GovApprovalRecord::SPACE,
            8 + 32 + 32
                + (1 + 32 + 32 + 1 + 2 + 2 + 32 + 8 + 1 + 4 + 61 + 512 + 1 + 32 + 1 + 256 + 64 + 1
                    + 480
                    + 1)
                + 1
        );
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
        let life = replay_transition_from_records(&state, &mut record, &obs,
    &[]
)
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
        let err = replay_transition_from_records(&state, &mut record, &obs,
    &[]
)
            .expect_err("unsealed replay must refuse");
        assert!(format!("{:?}", err).contains("NotSealed"));
        assert_eq!(record.stage, TransitionRecord::REGISTERED, "a refused replay must not move the stage");
    }

    #[test]
    fn replay_is_once_only() {
        let (state, mut record, obs) = replay_pair();
        replay_transition_from_records(&state, &mut record, &obs,
    &[]
)
            .expect("first replay must succeed");
        let err = replay_transition_from_records(&state, &mut record, &obs,
    &[]
)
            .expect_err("second replay must refuse");
        assert!(format!("{:?}", err).contains("WrongStage"));
        assert_eq!(record.stage, TransitionRecord::KERNEL_REGISTERED);
    }

    #[test]
    fn replay_refuses_parent_mismatch() {
        let (state, mut record, obs) = replay_pair();
        record.parent_index = 1;
        let err = replay_transition_from_records(&state, &mut record, &obs,
    &[]
)
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
        let err = replay_transition_from_records(&state, &mut record, &other,
    &[]
)
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
        let err = replay_transition_from_records(&state, &mut record, &obs,
    &[]
)
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
        let err = replay_transition_from_records(&state, &mut record, &obs,
    &[]
)
            .expect_err("non-observer observation must refuse");
        assert!(format!("{:?}", err).contains("NotObserver"));
        assert_eq!(record.stage, TransitionRecord::REGISTERED);
    }

    #[test]
    fn replay_refuses_expired_transition_and_overconfident_observation() {
        let (state, mut record, obs) = replay_pair();
        record.expires_at = 100;
        let err = replay_transition_from_records(&state, &mut record, &obs,
    &[]
)
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
        let err = replay_transition_from_records(&state, &mut record, &obs,
    &[]
)
            .expect_err("confidence above SCALE must refuse in the kernel");
        assert!(format!("{:?}", err).contains("ReplayFailed"));
        assert_eq!(record.stage, TransitionRecord::REGISTERED);
    }

    #[test]
    fn replay_refuses_governance_and_unknown_types() {
        let (state, mut record, obs) = replay_pair();
        record.transition_type = T_GOV_COUNCIL;
        let err = replay_transition_from_records(&state, &mut record, &obs,
    &[]
)
            .expect_err("governance type must refuse in the kernel");
        assert!(format!("{:?}", err).contains("ReplayFailed"));
        assert_eq!(record.stage, TransitionRecord::REGISTERED);
        let (state, mut record, obs) = replay_pair();
        record.transition_type = [0x77; 32];
        let err = replay_transition_from_records(&state, &mut record, &obs,
    &[]
)
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
        let life = replay_transition_from_records(&state, &mut record, &obs,
    &[]
)
            .expect("post-genesis pair must replay at the advanced clock");
        assert_eq!(life, Life::Proposed);
        assert_eq!(record.stage, TransitionRecord::KERNEL_REGISTERED);
    }

    #[test]
    fn advance_past_expiry_kills_replay_through_the_kernel() {
        let (state, mut record, obs) = replay_pair();
        let mut advanced = state;
        advance_clock_on_state(&mut advanced, 9100).expect("advance must succeed");
        let err = replay_transition_from_records(&advanced, &mut record, &obs,
    &[]
)
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
        let life = replay_transition_from_records(&state, &mut record, &obs,
    &[]
)
            .expect("pair must replay after chained advances");
        assert_eq!(life, Life::Proposed);
        assert_eq!(record.stage, TransitionRecord::KERNEL_REGISTERED);
    }

    fn evaluated_pair() -> (KernelState, TransitionRecord, ObservationRecord) {
        let (state, mut record, obs) = replay_pair();
        replay_transition_from_records(&state, &mut record, &obs,
    &[]
)
            .expect("pair must replay before evaluate");
        assert_eq!(record.stage, TransitionRecord::KERNEL_REGISTERED);
        (state, record, obs)
    }

    #[test]
    fn evaluate_advances_kernel_registered_to_evaluated() {
        let (state, mut record, obs) = evaluated_pair();
        let life = evaluate_transition_from_records(&state, &mut record, &obs,
    &[]
)
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
        let err = evaluate_transition_from_records(&state, &mut record, &obs,
    &[]
)
            .expect_err("a merely-registered record has nothing to evaluate");
        assert!(format!("{:?}", err).contains("WrongStage"));
        assert_eq!(record.stage, TransitionRecord::REGISTERED);
    }

    #[test]
    fn evaluate_is_once_only() {
        let (state, mut record, obs) = evaluated_pair();
        evaluate_transition_from_records(&state, &mut record, &obs,
    &[]
)
            .expect("first evaluate must succeed");
        let err = evaluate_transition_from_records(&state, &mut record, &obs,
    &[]
)
            .expect_err("second evaluate must refuse");
        assert!(format!("{:?}", err).contains("WrongStage"));
        assert_eq!(record.stage, TransitionRecord::EVALUATED);
    }

    #[test]
    fn evaluate_refuses_unsealed() {
        let (mut state, mut record, obs) = evaluated_pair();
        state.sealed = false;
        let err = evaluate_transition_from_records(&state, &mut record, &obs,
    &[]
)
            .expect_err("unsealed evaluate must refuse");
        assert!(format!("{:?}", err).contains("NotSealed"));
        assert_eq!(record.stage, TransitionRecord::KERNEL_REGISTERED);
    }

    #[test]
    fn evaluate_refuses_parent_mismatch() {
        let (state, mut record, obs) = evaluated_pair();
        record.parent_index = 1;
        let err = evaluate_transition_from_records(&state, &mut record, &obs,
    &[]
)
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
        let err = evaluate_transition_from_records(&state, &mut record, &other,
    &[]
)
            .expect_err("observation id mismatch must refuse");
        assert!(format!("{:?}", err).contains("RecordMismatch"));
        assert_eq!(record.stage, TransitionRecord::KERNEL_REGISTERED);
    }

    #[test]
    fn evaluate_refuses_unknown_proposer() {
        let (state, mut record, obs) = evaluated_pair();
        record.proposer = Pubkey::new_from_array([0x99; 32]);
        let err = evaluate_transition_from_records(&state, &mut record, &obs,
    &[]
)
            .expect_err("unknown proposer must refuse");
        assert!(format!("{:?}", err).contains("UnknownProposer"));
        assert_eq!(record.stage, TransitionRecord::KERNEL_REGISTERED);
    }

    #[test]
    fn evaluate_at_advanced_clock() {
        let (state, mut record, obs) = evaluated_pair();
        let mut advanced = state;
        advance_clock_on_state(&mut advanced, 200).expect("advance must succeed");
        let life = evaluate_transition_from_records(&advanced, &mut record, &obs,
    &[]
)
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
        evaluate_transition_from_records(&state, &mut record, &obs,
    &[]
)
            .expect("pair must evaluate before marking eligible");
        assert_eq!(record.stage, TransitionRecord::EVALUATED);
        (state, record, obs)
    }

    #[test]
    fn mark_eligible_advances_evaluated_to_eligible() {
        let (mut state, mut record, obs) = eligible_pair();
        let life = mark_eligible_from_records(&mut state, &mut record, &obs,
    &[]
)
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
        let err = mark_eligible_from_records(&mut state, &mut record, &obs,
    &[]
)
            .expect_err("a merely kernel-registered record has no evaluation to make eligible");
        assert!(format!("{:?}", err).contains("WrongStage"));
        assert_eq!(record.stage, TransitionRecord::KERNEL_REGISTERED);
    }

    #[test]
    fn mark_eligible_is_once_only() {
        let (mut state, mut record, obs) = eligible_pair();
        mark_eligible_from_records(&mut state, &mut record, &obs,
    &[]
)
            .expect("first mark_eligible must succeed");
        let err = mark_eligible_from_records(&mut state, &mut record, &obs,
    &[]
)
            .expect_err("second mark_eligible must refuse");
        assert!(format!("{:?}", err).contains("WrongStage"));
        assert_eq!(record.stage, TransitionRecord::ELIGIBLE);
    }

    #[test]
    fn mark_eligible_refuses_unsealed() {
        let (mut state, mut record, obs) = eligible_pair();
        state.sealed = false;
        let err = mark_eligible_from_records(&mut state, &mut record, &obs,
    &[]
)
            .expect_err("unsealed mark_eligible must refuse");
        assert!(format!("{:?}", err).contains("NotSealed"));
        assert_eq!(record.stage, TransitionRecord::EVALUATED);
    }

    #[test]
    fn mark_eligible_refuses_parent_mismatch() {
        let (mut state, mut record, obs) = eligible_pair();
        record.parent_index = 1;
        let err = mark_eligible_from_records(&mut state, &mut record, &obs,
    &[]
)
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
        let err = mark_eligible_from_records(&mut state, &mut record, &other,
    &[]
)
            .expect_err("observation id mismatch must refuse");
        assert!(format!("{:?}", err).contains("RecordMismatch"));
        assert_eq!(record.stage, TransitionRecord::EVALUATED);
    }

    #[test]
    fn mark_eligible_refuses_unknown_proposer() {
        let (mut state, mut record, obs) = eligible_pair();
        record.proposer = Pubkey::new_from_array([0x99; 32]);
        let err = mark_eligible_from_records(&mut state, &mut record, &obs,
    &[]
)
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
        let err = mark_eligible_from_records(&mut state, &mut record, &obs,
    &[]
)
            .expect_err("non-observer observation must refuse");
        assert!(format!("{:?}", err).contains("NotObserver"));
        assert_eq!(record.stage, TransitionRecord::EVALUATED);
    }

    #[test]
    fn mark_eligible_at_advanced_clock() {
        let (state, mut record, obs) = eligible_pair();
        let mut advanced = state;
        advance_clock_on_state(&mut advanced, 200).expect("advance must succeed");
        let life = mark_eligible_from_records(&mut advanced, &mut record, &obs,
    &[]
)
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
        mark_eligible_from_records(&mut advanced, &mut record, &obs,
    &[]
)
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
        let err = mark_eligible_from_records(&mut advanced, &mut record, &obs,
    &[]
)
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
        let err = mark_eligible_from_records(&mut advanced, &mut record, &obs,
    &[]
)
            .expect_err("lapsed observation must refuse eligibility in the kernel");
        assert!(format!("{:?}", err).contains("EligibleFailed"));
        assert_eq!(record.stage, TransitionRecord::EVALUATED);
    }

    fn signal_pair() -> (KernelState, TransitionRecord, ObservationRecord) {
        let (mut state, mut record, obs) = eligible_pair();
        mark_eligible_from_records(&mut state, &mut record, &obs,
    &[]
)
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
            &[],
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
            &[],
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
            &[],
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
            &[],
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
            &[],
)
        .expect_err("unknown pubkey is no agent and must refuse in the kernel");
        assert!(format!("{:?}", err).contains("SignalFailed"));
        let err = submit_signal_from_records(
            &state, &mut record, &obs, &[], Pubkey::new_from_array(ag(2)),
            0, SCALE + 1, SCALE, true, 7,
            &[],
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
            &[],
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
            &[],
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
            &[],
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
            &[],
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
            &[],
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
            &[],
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
            &[],
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
            &[],
)
        .expect_err("signal at the deadline must refuse in the kernel");
        assert!(format!("{:?}", err).contains("SignalFailed"));
    }

    fn selected_pair() -> (KernelState, TransitionRecord, ObservationRecord, SignalRecord) {
        let (state, mut record, obs) = signal_pair();
        let (signal, _, _) = submit_signal_from_records(
            &state, &mut record, &obs, &[], Pubkey::new_from_array(ag(2)),
            0, SCALE, SCALE, true, 7,
            &[],
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
            &[],
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
        let life = select_candidate_from_records(&state, &mut record, &obs, &[first],
    &[]
)
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
        let err = select_candidate_from_records(&state, &mut record, &obs, &[first],
    &[]
)
            .expect_err("an open window must refuse selection");
        assert!(format!("{:?}", err).contains("SelectFailed"));
        assert_eq!(record.stage, TransitionRecord::ELIGIBLE);
    }

    #[test]
    fn select_refuses_wrong_stage() {
        let (state, mut record, obs) = evaluated_pair();
        let err = select_candidate_from_records(&state, &mut record, &obs, &[],
    &[]
)
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
        let err = select_candidate_from_records(&state, &mut record, &obs, &[first],
    &[]
)
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
        let err = select_candidate_from_records(&state, &mut record, &obs, &[first, first],
    &[]
)
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
            &[],
)
        .expect("support-0 signal must submit");
        let mut advanced = state;
        advance_clock_on_state(&mut advanced, 200).expect("advance must succeed");
        let err = select_candidate_from_records(&advanced, &mut record, &obs, &[signal],
    &[]
)
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
        let err = select_candidate_from_records(&state, &mut record, &obs, &[foreign],
    &[]
)
            .expect_err("a prior from another transition must refuse");
        assert!(format!("{:?}", err).contains("RecordMismatch"));
        assert_eq!(record.stage, TransitionRecord::ELIGIBLE);
    }

    #[test]
    fn select_is_once_only() {
        let (mut state, mut record, obs, first) = selected_pair();
        advance_clock_on_state(&mut state, 200).expect("advance must succeed");
        select_candidate_from_records(&state, &mut record, &obs, &[first],
    &[]
)
            .expect("first select must succeed");
        let err = select_candidate_from_records(&state, &mut record, &obs, &[first],
    &[]
)
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
            &[],
)
        .expect_err("same-round duplicate must refuse");
        assert!(format!("{:?}", err).contains("SignalFailed"));
        assert_eq!(record.signal_count, 1);
        let mut advanced = state;
        advance_clock_on_state(&mut advanced, 133).expect("advance must succeed");
        let (_, _, _) = submit_signal_from_records(
            &advanced, &mut record, &obs, &[first], Pubkey::new_from_array(ag(2)),
            1, SCALE, SCALE, true, 7,
            &[],
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
        replay_transition_from_records(&state, &mut record, &obs,
    &[]
)
            .expect("sibling replay must succeed");
        evaluate_transition_from_records(&state, &mut record, &obs,
    &[]
)
            .expect("sibling evaluate must succeed");
        mark_eligible_from_records(&mut state, &mut record, &obs,
    &[]
)
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
            &[],
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
            &[],
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
            &[],
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
            &[],
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
            &[],
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
            &[],
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
            &[],
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
            &[],
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
            &[],
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
            plan_count: 0,
            replans: 0,
            compatible: false,
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
            &[],
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
        replay_transition_from_records(&eval_state, &mut sib, &sib_obs,
    &[]
)
            .expect("sibling replay must succeed");
        evaluate_transition_from_records(&eval_state, &mut sib, &sib_obs,
    &[]
)
            .expect("sibling evaluate must succeed");
        assert_eq!(sib.stage, TransitionRecord::EVALUATED);
        let sibling = SiblingReplay {
            transition_record: &sib,
            observation_record: &sib_obs,
            prior_signals: &[],
        };
        let err = select_candidate_from_records_with_siblings(
            &state, &mut target, &target_obs, &[first], &[sibling],
            &[],
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
            &[],
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
            &[],
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
            &[],
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
            &[],
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
            &[],
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
            &[],
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
            &[],
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
            &[],
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
            &[],
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
            &[],
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
        mark_eligible_from_records(&mut state, &mut record, &obs,
    &[]
)
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
        replay_transition_from_records(&state, &mut second, &second_obs,
    &[]
)
            .expect("second replay must succeed");
        evaluate_transition_from_records(&state, &mut second, &second_obs,
    &[]
)
            .expect("second evaluate must succeed");
        mark_eligible_from_records(&mut state, &mut second, &second_obs,
    &[]
)
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
        mark_eligible_from_records(&mut state, &mut record, &obs,
    &[]
)
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
        let err = mark_eligible_from_records(&mut advanced, &mut record, &obs,
    &[]
)
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
        let err = select_candidate_from_records(&state, &mut record, &obs, &[first],
    &[]
)
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
            &[],
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
            &[],
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
        let life = select_candidate_from_records(&state, &mut record, &obs, &[first],
    &[]
)
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
        let life = open_planning_from_records(&state, &mut record, &obs, &[first], [0x77; 32],
    &[]
)
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
        let err = open_planning_from_records(&state, &mut record, &obs, &[first], ag(1),
    &[]
)
            .expect_err("an unselected record must refuse planning");
        assert!(format!("{:?}", err).contains("WrongStage"));
        assert_eq!(record.stage, TransitionRecord::ELIGIBLE);
        assert_eq!(record.planning_deadline, 0);
    }

    #[test]
    fn open_planning_is_once_only() {
        let (state, mut record, obs, first) = planning_pair();
        open_planning_from_records(&state, &mut record, &obs, &[first], ag(1),
    &[]
)
            .expect("first planning open must succeed");
        let err = open_planning_from_records(&state, &mut record, &obs, &[first], ag(1),
    &[]
)
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
        let err = open_planning_from_records(&state, &mut record, &obs, &[first], ag(1),
    &[]
)
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
        let err = open_planning_from_records(&state, &mut record, &obs, &[first], ag(1),
    &[]
)
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
        open_planning_from_records(&state, &mut record, &obs, &[first], ag(1),
    &[]
)
            .expect("planning at a later clock must succeed");
        assert_eq!(record.stage, TransitionRecord::PLANNING);
        assert_eq!(record.planning_deadline, 600);
    }

    #[test]
    fn open_planning_refuses_unsealed() {
        let (mut state, mut record, obs, first) = planning_pair();
        state.sealed = false;
        let err = open_planning_from_records(&state, &mut record, &obs, &[first], ag(1),
    &[]
)
            .expect_err("an unsealed state must refuse planning");
        assert!(format!("{:?}", err).contains("NotSealed"));
        assert_eq!(record.stage, TransitionRecord::SELECTED);
        assert_eq!(record.planning_deadline, 0);
    }

    #[test]
    fn open_planning_refuses_parent_mismatch() {
        let (mut state, mut record, obs, first) = planning_pair();
        state.head_index = 9;
        let err = open_planning_from_records(&state, &mut record, &obs, &[first], ag(1),
    &[]
)
            .expect_err("a record off the sealed head must refuse planning");
        assert!(format!("{:?}", err).contains("ParentMismatch"));
        assert_eq!(record.stage, TransitionRecord::SELECTED);
        assert_eq!(record.planning_deadline, 0);
    }

    fn blocked_pair() -> (KernelState, TransitionRecord, ObservationRecord, SignalRecord) {
        // Slice 22 fixture: a planning pair whose record stands at
        // BLOCKED with a prior episode's deadline of 250 — deliberately
        // not 300, so a successful re-plan proves the deadline RESET
        // to clock 200 + window 100, rather than leaving it alone. No slice can yet produce a Blocked
        // record — a future validate slice will — so the fixture
        // sets the label; the tests below prove the re-plan
        // authority gate and the cap, not the label itself (see
        // the slice-22 module note for the honesty bound).
        let (state, mut record, obs, first) = planning_pair();
        record.stage = TransitionRecord::BLOCKED;
        record.planning_deadline = 250;
        assert_eq!(record.replans, 0);
        (state, record, obs, first)
    }

    #[test]
    fn replan_by_proposer_opens_planning() {
        // The proposer (ag(1), also a seat) re-plans: stage
        // BLOCKED -> PLANNING, deadline reset to clock 200 + window
        // 100 = 300, replans 0 -> 1.
        let (state, mut record, obs, first) = blocked_pair();
        let life = open_planning_from_records(&state, &mut record, &obs, &[first], ag(1),
    &[]
)
            .expect("a proposer re-plan must open planning");
        assert_eq!(life, Life::Planning);
        assert_eq!(record.stage, TransitionRecord::PLANNING);
        assert_eq!(record.planning_deadline, 300);
        assert_eq!(record.replans, 1);
    }

    #[test]
    fn replan_by_seat_opens_planning() {
        // A seat who is NOT the proposer re-plans: the proposer is
        // moved to ag(4) (seeded, active, same group), so the
        // caller ag(2) qualifies on the seat ground alone.
        let (state, mut record, obs, first) = blocked_pair();
        record.proposer = Pubkey::new_from_array(ag(4));
        let life = open_planning_from_records(&state, &mut record, &obs, &[first], ag(2),
    &[]
)
            .expect("a seat re-plan must open planning");
        assert_eq!(life, Life::Planning);
        assert_eq!(record.stage, TransitionRecord::PLANNING);
        assert_eq!(record.replans, 1);
    }

    #[test]
    fn replan_by_non_seat_proposer_opens_planning() {
        // The proposer who holds no seat re-plans: ag(4) is a
        // seeded agent but not a council seat, so the caller
        // qualifies on the proposer ground alone.
        let (state, mut record, obs, first) = blocked_pair();
        record.proposer = Pubkey::new_from_array(ag(4));
        let life = open_planning_from_records(&state, &mut record, &obs, &[first], ag(4),
    &[]
)
            .expect("a non-seat proposer re-plan must open planning");
        assert_eq!(life, Life::Planning);
        assert_eq!(record.stage, TransitionRecord::PLANNING);
        assert_eq!(record.replans, 1);
    }

    #[test]
    fn replan_refuses_stranger() {
        // Neither proposer nor seat: ReplanRefused, and the record
        // stands exactly as it stood — stage, deadline, count.
        let (state, mut record, obs, first) = blocked_pair();
        let err = open_planning_from_records(&state, &mut record, &obs, &[first], [0x77; 32],
    &[]
)
            .expect_err("a stranger re-plan must refuse");
        assert!(format!("{:?}", err).contains("ReplanRefused"));
        assert_eq!(record.stage, TransitionRecord::BLOCKED);
        assert_eq!(record.planning_deadline, 250);
        assert_eq!(record.replans, 0);
    }

    #[test]
    fn replan_refuses_at_cap() {
        // MAX_REPLANS (3) already taken: even the proposer refuses,
        // and the count does not advance past the cap.
        let (state, mut record, obs, first) = blocked_pair();
        record.replans = 3;
        let err = open_planning_from_records(&state, &mut record, &obs, &[first], ag(1),
    &[]
)
            .expect_err("a capped re-plan must refuse");
        assert!(format!("{:?}", err).contains("ReplanRefused"));
        assert_eq!(record.stage, TransitionRecord::BLOCKED);
        assert_eq!(record.planning_deadline, 250);
        assert_eq!(record.replans, 3);
    }

    #[test]
    fn replan_then_open_refuses_second_time() {
        // A re-plan is once per Blocked episode: after success the
        // stage is PLANNING, so a second open refuses WrongStage
        // and the count stays at 1, not 2.
        let (state, mut record, obs, first) = blocked_pair();
        open_planning_from_records(&state, &mut record, &obs, &[first], ag(1),
    &[]
)
            .expect("first re-plan must succeed");
        let err = open_planning_from_records(&state, &mut record, &obs, &[first], ag(1),
    &[]
)
            .expect_err("a second planning open must refuse");
        assert!(format!("{:?}", err).contains("WrongStage"));
        assert_eq!(record.stage, TransitionRecord::PLANNING);
        assert_eq!(record.planning_deadline, 300);
        assert_eq!(record.replans, 1);
    }

    #[test]
    fn selected_open_leaves_replans_untouched() {
        // The Selected path is not a re-plan: replans stays 0.
        let (state, mut record, obs, first) = planning_pair();
        open_planning_from_records(&state, &mut record, &obs, &[first], ag(1),
    &[]
)
            .expect("a Selected open must succeed");
        assert_eq!(record.stage, TransitionRecord::PLANNING);
        assert_eq!(record.replans, 0);
    }

    fn planned_pair() -> (KernelState, TransitionRecord, ObservationRecord, SignalRecord) {
        // Planning opened at clock 200, so the stored deadline is
        // 300 and the derived open clock is 200: the replay clock
        // for select and open_planning in every plan test below.
        let (state, mut record, obs, first) = planning_pair();
        let life = open_planning_from_records(&state, &mut record, &obs, &[first], ag(1),
    &[]
)
            .expect("planning open must succeed");
        assert_eq!(life, Life::Planning);
        assert_eq!(record.stage, TransitionRecord::PLANNING);
        assert_eq!(record.planning_deadline, 300);
        assert_eq!(record.plan_count, 0);
        (state, record, obs, first)
    }

    fn two_claims() -> Vec<PlanClaimArg> {
        vec![
            PlanClaimArg { resource: RES_TREASURY_TRANCHE, amount_raw: 10 * SCALE },
            PlanClaimArg { resource: RES_LENDING_BOOK, amount_raw: 5 * SCALE },
        ]
    }

    #[test]
    fn plan_record_space_accounts_for_every_field() {
        // Discriminator 8 + transition 32 + agent 32 + count 1 +
        // resources 16 + amounts 64 + expires 8 + bump 1.
        assert_eq!(PlanRecord::SPACE, 8 + 32 + 32 + 1 + 16 + 64 + 8 + 1);
    }

    #[test]
    fn plan_record_borsh_round_trips() {
        use anchor_lang::{AccountDeserialize, AccountSerialize};
        let record = PlanRecord {
            transition_id: [0xD1; 32],
            agent: Pubkey::new_from_array(ag(2)),
            claim_count: 2,
            claim_resources: [RES_TREASURY_TRANCHE, RES_LENDING_BOOK, 0, 0, 0, 0, 0, 0],
            claim_amounts: [10 * SCALE, 5 * SCALE, 0, 0, 0, 0, 0, 0],
            expires_at: 400,
            bump: 7,
        };
        let mut bytes = Vec::new();
        record.try_serialize(&mut bytes).expect("serialize must succeed");
        assert_eq!(bytes.len(), PlanRecord::SPACE);
        let back = <PlanRecord as AccountDeserialize>::try_deserialize(&mut &bytes[..])
            .expect("deserialize must succeed");
        assert_eq!(back.transition_id, [0xD1; 32]);
        assert_eq!(back.agent.to_bytes(), ag(2));
        assert_eq!(back.claim_count, 2);
        assert_eq!(back.claim_resources[1], RES_LENDING_BOOK);
        assert_eq!(back.claim_amounts[0], 10 * SCALE);
        assert_eq!(back.expires_at, 400);
    }

    #[test]
    fn submit_plan_advances_planning_record() {
        // A two-resource plan from a seeded agent, expiring after
        // the deadline: the record stays PLANNING, the deadline
        // stands, and plan_count moves 0 -> 1 on success only.
        let (state, mut record, obs, first) = planned_pair();
        let plan = submit_plan_from_records(
            &state, &mut record, &obs, &[first], &[], &two_claims(), 400, ag(2), 7,
            &[],
)
        .expect("a seeded agent's plan inside the window must submit");
        assert_eq!(plan.transition_id, record.transition_id);
        assert_eq!(plan.agent.to_bytes(), ag(2));
        assert_eq!(plan.claim_count, 2);
        assert_eq!(plan.claim_resources[0], RES_TREASURY_TRANCHE);
        assert_eq!(plan.claim_amounts[0], 10 * SCALE);
        assert_eq!(plan.claim_resources[1], RES_LENDING_BOOK);
        assert_eq!(plan.claim_amounts[1], 5 * SCALE);
        assert_eq!(plan.expires_at, 400);
        assert_eq!(plan.bump, 7);
        assert_eq!(record.stage, TransitionRecord::PLANNING);
        assert_eq!(record.planning_deadline, 300);
        assert_eq!(record.plan_count, 1);
    }

    #[test]
    fn submit_plan_second_agent_replays_prior_plan() {
        // The second agent's submission replays the first plan at
        // the state's clock before its own: plan_count 1 -> 2, and
        // the first plan's claims are what the kernel judged, not
        // a fresh single-plan kernel.
        let (state, mut record, obs, first) = planned_pair();
        let prior = submit_plan_from_records(
            &state, &mut record, &obs, &[first], &[], &two_claims(), 400, ag(2), 7,
            &[],
)
        .expect("first plan must submit");
        assert_eq!(record.plan_count, 1);
        let second = submit_plan_from_records(
            &state,
            &mut record,
            &obs,
            &[first],
            &[prior],
            &[PlanClaimArg { resource: RES_EXIT_WINDOW, amount_raw: 7 * SCALE }],
            500,
            ag(3),
            8,
            &[],
)
        .expect("a second agent's plan must submit over the prior");
        assert_eq!(second.agent.to_bytes(), ag(3));
        assert_eq!(second.claim_count, 1);
        assert_eq!(second.claim_resources[0], RES_EXIT_WINDOW);
        assert_eq!(record.plan_count, 2);
        assert_eq!(record.stage, TransitionRecord::PLANNING);
    }

    #[test]
    fn submit_plan_refuses_wrong_stage() {
        // Still ELIGIBLE, never planned: the stage gate refuses
        // before any replay, and plan_count stays 0.
        let (state, mut record, obs, first) = selected_pair();
        let err = submit_plan_from_records(
            &state, &mut record, &obs, &[first], &[], &two_claims(), 400, ag(2), 7,
            &[],
)
        .expect_err("an unplanned record must refuse a plan");
        assert!(format!("{:?}", err).contains("WrongStage"));
        assert_eq!(record.plan_count, 0);
    }

    #[test]
    fn submit_plan_refuses_at_deadline() {
        // At clock 300 the window is closed: the stored-deadline
        // gate refuses, and the record is untouched.
        let (mut state, mut record, obs, first) = planned_pair();
        advance_clock_on_state(&mut state, 300).expect("advance to the deadline must succeed");
        let err = submit_plan_from_records(
            &state, &mut record, &obs, &[first], &[], &two_claims(), 400, ag(2), 7,
            &[],
)
        .expect_err("a plan at the deadline must refuse");
        assert!(format!("{:?}", err).contains("PlanFailed"));
        assert_eq!(record.stage, TransitionRecord::PLANNING);
        assert_eq!(record.plan_count, 0);
    }

    #[test]
    fn submit_plan_refuses_expiry_at_deadline() {
        // Expiry equal to the deadline is not after it: the
        // kernel's BadWindow rule, judged against the stored
        // deadline, refuses before any plan is recorded.
        let (state, mut record, obs, first) = planned_pair();
        let err = submit_plan_from_records(
            &state, &mut record, &obs, &[first], &[], &two_claims(), 300, ag(2), 7,
            &[],
)
        .expect_err("a plan expiring at the deadline must refuse");
        assert!(format!("{:?}", err).contains("PlanFailed"));
        assert_eq!(record.plan_count, 0);
    }

    #[test]
    fn submit_plan_refuses_unknown_agent() {
        // Unlike open_planning's anyone rule, submit_plan carries
        // the kernel's agent gate: a fresh id that is no seeded
        // agent refuses through the kernel, not by a program guess.
        let (state, mut record, obs, first) = planned_pair();
        let err = submit_plan_from_records(
            &state, &mut record, &obs, &[first], &[], &two_claims(), 400, [0x77; 32], 7,
            &[],
)
        .expect_err("an unknown agent's plan must refuse");
        assert!(format!("{:?}", err).contains("PlanFailed"));
        assert_eq!(record.plan_count, 0);
    }

    #[test]
    fn submit_plan_refuses_duplicate_agent() {
        // One plan per agent: the second submission from ag(2),
        // with its own first plan supplied as the prior set,
        // refuses at the kernel's Duplicate arm.
        let (state, mut record, obs, first) = planned_pair();
        let prior = submit_plan_from_records(
            &state, &mut record, &obs, &[first], &[], &two_claims(), 400, ag(2), 7,
            &[],
)
        .expect("first plan must submit");
        let err = submit_plan_from_records(
            &state, &mut record, &obs, &[first], &[prior], &two_claims(), 500, ag(2), 7,
            &[],
)
        .expect_err("a duplicate agent plan must refuse");
        assert!(format!("{:?}", err).contains("PlanFailed"));
        assert_eq!(record.plan_count, 1);
    }

    #[test]
    fn submit_plan_refuses_duplicate_resource() {
        // Two claims on the same resource would collapse silently
        // in a map insert, so the program refuses before any replay
        // and records nothing.
        let (state, mut record, obs, first) = planned_pair();
        let dup = vec![
            PlanClaimArg { resource: RES_TREASURY_TRANCHE, amount_raw: 10 * SCALE },
            PlanClaimArg { resource: RES_TREASURY_TRANCHE, amount_raw: 20 * SCALE },
        ];
        let err = submit_plan_from_records(
            &state, &mut record, &obs, &[first], &[], &dup, 400, ag(2), 7,
            &[],
)
        .expect_err("a duplicated resource must refuse");
        assert!(format!("{:?}", err).contains("PlanFailed"));
        assert_eq!(record.plan_count, 0);
    }

    #[test]
    fn submit_plan_refuses_incomplete_prior_plans() {
        // The record claims a prior plan that was never supplied:
        // the slice-21 anchor refuses before any replay.
        let (state, mut record, obs, first) = planned_pair();
        record.plan_count = 1;
        let err = submit_plan_from_records(
            &state, &mut record, &obs, &[first], &[], &two_claims(), 400, ag(2), 7,
            &[],
)
        .expect_err("a short prior plan set must refuse");
        assert!(format!("{:?}", err).contains("PlanFailed"));
        assert_eq!(record.plan_count, 1);
    }

    #[test]
    fn submit_plan_refuses_incomplete_prior_signals() {
        // The record claims a second signal that was never
        // supplied: the slice-16 anchor refuses before any replay,
        // exactly as in open_planning.
        let (state, mut record, obs, first) = planned_pair();
        record.signal_count = 2;
        let err = submit_plan_from_records(
            &state, &mut record, &obs, &[first], &[], &two_claims(), 400, ag(2), 7,
            &[],
)
        .expect_err("a short prior signal set must refuse a plan");
        assert!(format!("{:?}", err).contains("PlanFailed"));
        assert_eq!(record.plan_count, 0);
    }

    #[test]
    fn submit_plan_refuses_unsealed() {
        let (mut state, mut record, obs, first) = planned_pair();
        state.sealed = false;
        let err = submit_plan_from_records(
            &state, &mut record, &obs, &[first], &[], &two_claims(), 400, ag(2), 7,
            &[],
)
        .expect_err("an unsealed state must refuse a plan");
        assert!(format!("{:?}", err).contains("NotSealed"));
        assert_eq!(record.plan_count, 0);
    }

    fn one_plan_pair() -> (
        KernelState,
        TransitionRecord,
        ObservationRecord,
        SignalRecord,
        PlanRecord,
    ) {
        // Planning opened at clock 200, deadline 300, one fitting
        // two-resource plan from ag(2): the reconcile-success
        // fixture every slice-23 test below builds from.
        let (state, mut record, obs, first) = planned_pair();
        let plan = submit_plan_from_records(
            &state, &mut record, &obs, &[first], &[], &two_claims(), 400, ag(2), 7,
            &[],
)
        .expect("first plan must submit");
        assert_eq!(record.plan_count, 1);
        (state, record, obs, first, plan)
    }

    #[test]
    fn reconcile_advances_planning_to_reconciling_compatible() {
        // At the deadline exactly (clock 300) the window is closed:
        // the kernel reconciles, the stage advances to 7, and the
        // fitting plan set records compatible true.
        let (mut state, mut record, obs, first, plan) = one_plan_pair();
        advance_clock_on_state(&mut state, 300).expect("advance to the deadline must succeed");
        let life = reconcile_plans_from_records(&state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect("a kernel-proven plan set at the deadline must reconcile");
        assert_eq!(life, Life::Reconciling);
        assert_eq!(record.stage, TransitionRecord::RECONCILING);
        assert_eq!(record.stage, 7);
        assert!(record.compatible);
        assert_eq!(record.plan_count, 1);
        assert_eq!(record.planning_deadline, 300);
    }

    #[test]
    fn reconcile_empty_plan_set_is_incompatible_not_refused() {
        // No plans is never compatible — but it is a verdict, not
        // a refusal: the kernel still advances to Reconciling, and
        // the record says so, so validate judges the verdict
        // instead of the call failing to happen.
        let (mut state, mut record, obs, first) = planned_pair();
        advance_clock_on_state(&mut state, 300).expect("advance to the deadline must succeed");
        let life = reconcile_plans_from_records(&state, &mut record, &obs, &[first], &[],
    &[]
)
            .expect("an empty plan set must reconcile to incompatible, not refuse");
        assert_eq!(life, Life::Reconciling);
        assert_eq!(record.stage, TransitionRecord::RECONCILING);
        assert!(!record.compatible);
        assert_eq!(record.plan_count, 0);
    }

    #[test]
    fn reconcile_over_capacity_is_incompatible() {
        // Two plans at 600 x SCALE on the tranche stock (capacity
        // 1000 x SCALE) each submit cleanly — capacity is judged
        // at reconcile, never at submission — and their 1200 sum
        // reconciles to incompatible while still advancing the
        // stage, exactly as the kernel's fits_capacity judges it.
        let (mut state, mut record, obs, first) = planned_pair();
        let big = vec![PlanClaimArg { resource: RES_TREASURY_TRANCHE, amount_raw: 600 * SCALE }];
        let first_plan = submit_plan_from_records(
            &state, &mut record, &obs, &[first], &[], &big, 400, ag(2), 7,
            &[],
)
        .expect("first big plan must submit");
        let second_plan = submit_plan_from_records(
            &state, &mut record, &obs, &[first], std::slice::from_ref(&first_plan), &big, 400, ag(3), 8,
            &[],
)
        .expect("second big plan must submit");
        assert_eq!(record.plan_count, 2);
        advance_clock_on_state(&mut state, 300).expect("advance to the deadline must succeed");
        let life = reconcile_plans_from_records(
            &state,
            &mut record,
            &obs,
            &[first],
            &[first_plan, second_plan],
            &[],
)
        .expect("an over-capacity set must reconcile to incompatible, not refuse");
        assert_eq!(life, Life::Reconciling);
        assert_eq!(record.stage, TransitionRecord::RECONCILING);
        assert!(!record.compatible);
    }

    #[test]
    fn reconcile_refuses_before_deadline() {
        // At clock 299 the planning window is still open: the
        // kernel's PlanningWindowOpen refuses, and the record
        // stands PLANNING with compatible still false.
        let (mut state, mut record, obs, first, plan) = one_plan_pair();
        advance_clock_on_state(&mut state, 299).expect("advance must succeed");
        let err = reconcile_plans_from_records(&state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("an open window must refuse reconcile");
        assert!(format!("{:?}", err).contains("ReconcileFailed"));
        assert_eq!(record.stage, TransitionRecord::PLANNING);
        assert!(!record.compatible);
    }

    #[test]
    fn reconcile_refuses_wrong_stage() {
        // Still SELECTED, never planned: the stage gate refuses
        // before any replay, and nothing on the record moves.
        // (`planning_pair` is the fixture that actually stands at
        // SELECTED; `selected_pair` only holds a signal and is
        // still ELIGIBLE.)
        let (state, mut record, obs, first) = planning_pair();
        let err = reconcile_plans_from_records(&state, &mut record, &obs, &[first], &[],
    &[]
)
            .expect_err("an unplanned record must refuse reconcile");
        assert!(format!("{:?}", err).contains("WrongStage"));
        assert_eq!(record.stage, TransitionRecord::SELECTED);
        assert!(!record.compatible);
    }

    #[test]
    fn reconcile_is_once_only() {
        let (mut state, mut record, obs, first, plan) = one_plan_pair();
        advance_clock_on_state(&mut state, 300).expect("advance to the deadline must succeed");
        reconcile_plans_from_records(&state, &mut record, &obs, &[first], std::slice::from_ref(&plan),
    &[]
)
            .expect("first reconcile must succeed");
        let err = reconcile_plans_from_records(&state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("a second reconcile must refuse");
        assert!(format!("{:?}", err).contains("WrongStage"));
        assert_eq!(record.stage, TransitionRecord::RECONCILING);
        assert!(record.compatible);
    }

    #[test]
    fn reconcile_refuses_incomplete_prior_plans() {
        // The record claims a prior plan that was never supplied:
        // the slice-21 anchor refuses before any replay.
        let (mut state, mut record, obs, first, plan) = one_plan_pair();
        record.plan_count = 2;
        advance_clock_on_state(&mut state, 300).expect("advance to the deadline must succeed");
        let err = reconcile_plans_from_records(&state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("a short prior plan set must refuse reconcile");
        assert!(format!("{:?}", err).contains("ReconcileFailed"));
        assert_eq!(record.stage, TransitionRecord::PLANNING);
        assert!(!record.compatible);
        assert_eq!(record.plan_count, 2);
    }

    #[test]
    fn reconcile_refuses_incomplete_prior_signals() {
        // The record claims a second signal that was never
        // supplied: the slice-16 anchor refuses before any replay.
        let (mut state, mut record, obs, first, plan) = one_plan_pair();
        record.signal_count = 2;
        advance_clock_on_state(&mut state, 300).expect("advance to the deadline must succeed");
        let err = reconcile_plans_from_records(&state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("a short prior signal set must refuse reconcile");
        assert!(format!("{:?}", err).contains("ReconcileFailed"));
        assert_eq!(record.stage, TransitionRecord::PLANNING);
        assert!(!record.compatible);
    }

    #[test]
    fn reconcile_refuses_unreproducible_plan() {
        // A prior plan whose agent is no seeded agent cannot be
        // replayed: the kernel's agent gate refuses, so the stored
        // PLANNING label alone reconciles nothing, and the record
        // stands.
        let (mut state, mut record, obs, first, mut plan) = one_plan_pair();
        plan.agent = Pubkey::new_from_array([0x77; 32]);
        advance_clock_on_state(&mut state, 300).expect("advance to the deadline must succeed");
        let err = reconcile_plans_from_records(&state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("an unreproducible plan must refuse reconcile");
        assert!(format!("{:?}", err).contains("ReconcileFailed"));
        assert_eq!(record.stage, TransitionRecord::PLANNING);
        assert!(!record.compatible);
    }

    #[test]
    fn reconcile_refuses_unsealed() {
        let (mut state, mut record, obs, first, plan) = one_plan_pair();
        advance_clock_on_state(&mut state, 300).expect("advance to the deadline must succeed");
        state.sealed = false;
        let err = reconcile_plans_from_records(&state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("an unsealed state must refuse reconcile");
        assert!(format!("{:?}", err).contains("NotSealed"));
        assert_eq!(record.stage, TransitionRecord::PLANNING);
        assert!(!record.compatible);
    }

    #[test]
    fn reconcile_refuses_parent_mismatch() {
        let (mut state, mut record, obs, first, plan) = one_plan_pair();
        advance_clock_on_state(&mut state, 300).expect("advance to the deadline must succeed");
        state.head_index = 9;
        let err = reconcile_plans_from_records(&state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("a record off the sealed head must refuse reconcile");
        assert!(format!("{:?}", err).contains("ParentMismatch"));
        assert_eq!(record.stage, TransitionRecord::PLANNING);
        assert!(!record.compatible);
    }

    fn reconciling_pair() -> (
        KernelState,
        TransitionRecord,
        ObservationRecord,
        SignalRecord,
        PlanRecord,
    ) {
        // Reconciled at the deadline (clock 300) with one fitting
        // plan: the validate-success fixture every slice-24 test
        // below builds from.
        let (mut state, mut record, obs, first, plan) = one_plan_pair();
        advance_clock_on_state(&mut state, 300).expect("advance to the deadline must succeed");
        let life = reconcile_plans_from_records(&state, &mut record, &obs, &[first], std::slice::from_ref(&plan),
    &[]
)
            .expect("a fitting plan set at the deadline must reconcile");
        assert_eq!(life, Life::Reconciling);
        assert_eq!(record.stage, TransitionRecord::RECONCILING);
        assert!(record.compatible);
        (state, record, obs, first, plan)
    }

    fn empty_reconciling_pair() -> (KernelState, TransitionRecord, ObservationRecord, SignalRecord) {
        // Reconciled at the deadline with no plans: incompatible,
        // so validate blocks rather than validates.
        let (mut state, mut record, obs, first) = planned_pair();
        advance_clock_on_state(&mut state, 300).expect("advance to the deadline must succeed");
        let life = reconcile_plans_from_records(&state, &mut record, &obs, &[first], &[],
    &[]
)
            .expect("an empty plan set must reconcile to incompatible");
        assert_eq!(life, Life::Reconciling);
        assert_eq!(record.stage, TransitionRecord::RECONCILING);
        assert!(!record.compatible);
        (state, record, obs, first)
    }

    #[test]
    fn validate_advances_reconciling_to_validated() {
        // A kernel-proven chain with a fitting plan set and a
        // fresh observation at clock 300: the predicate passes,
        // the stage advances to 8, and the stored verdict stands.
        let (state, mut record, obs, first, plan) = reconciling_pair();
        let life = validate_transition_from_records(&state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect("a fitting reconciled chain must validate");
        assert_eq!(life, Life::Validated);
        assert_eq!(record.stage, TransitionRecord::VALIDATED);
        assert_eq!(record.stage, 8);
        assert!(record.compatible);
    }

    #[test]
    fn validate_empty_plan_set_blocks_not_refuses() {
        // No plans is never compatible — but blocking is a state,
        // not a refusal: the kernel advances to Blocked, and the
        // record says so, so the re-plan path (slice 22) opens
        // from it instead of the call failing to happen.
        let (state, mut record, obs, first) = empty_reconciling_pair();
        let life = validate_transition_from_records(&state, &mut record, &obs, &[first], &[],
    &[]
)
            .expect("an incompatible set must validate to Blocked, not refuse");
        assert_eq!(life, Life::Blocked);
        assert_eq!(record.stage, TransitionRecord::BLOCKED);
        assert_eq!(record.stage, 6);
        assert!(!record.compatible);
    }

    #[test]
    fn validate_over_capacity_blocks() {
        // The slice-23 over-capacity set reconciles to
        // incompatible, and validate judges that verdict: Blocked,
        // with the plan set untouched.
        let (mut state, mut record, obs, first) = planned_pair();
        let big = vec![PlanClaimArg { resource: RES_TREASURY_TRANCHE, amount_raw: 600 * SCALE }];
        let first_plan = submit_plan_from_records(
            &state, &mut record, &obs, &[first], &[], &big, 400, ag(2), 7,
            &[],
)
        .expect("first big plan must submit");
        let second_plan = submit_plan_from_records(
            &state, &mut record, &obs, &[first], std::slice::from_ref(&first_plan), &big, 400, ag(3), 8,
            &[],
)
        .expect("second big plan must submit");
        advance_clock_on_state(&mut state, 300).expect("advance to the deadline must succeed");
        let plans = [first_plan, second_plan];
        reconcile_plans_from_records(&state, &mut record, &obs, &[first], &plans,
    &[]
)
            .expect("an over-capacity set must reconcile to incompatible");
        assert!(!record.compatible);
        let life = validate_transition_from_records(&state, &mut record, &obs, &[first], &plans,
    &[]
)
            .expect("an incompatible set must validate to Blocked, not refuse");
        assert_eq!(life, Life::Blocked);
        assert_eq!(record.stage, TransitionRecord::BLOCKED);
        assert!(!record.compatible);
        assert_eq!(record.plan_count, 2);
    }

    #[test]
    fn validate_refuses_wrong_stage() {
        // Still PLANNING, never reconciled: the stage gate refuses
        // before any replay, and nothing on the record moves.
        let (state, mut record, obs, first, plan) = one_plan_pair();
        let err = validate_transition_from_records(&state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("an unreconciled record must refuse validate");
        assert!(format!("{:?}", err).contains("WrongStage"));
        assert_eq!(record.stage, TransitionRecord::PLANNING);
        assert!(!record.compatible);
    }

    #[test]
    fn validate_is_once_only_after_validated() {
        // A validated record is past validate: the stage gate
        // refuses a second call, and the VALIDATED stage stands.
        let (state, mut record, obs, first, plan) = reconciling_pair();
        validate_transition_from_records(&state, &mut record, &obs, &[first], std::slice::from_ref(&plan),
    &[]
)
            .expect("first validate must succeed");
        assert_eq!(record.stage, TransitionRecord::VALIDATED);
        let err = validate_transition_from_records(&state, &mut record, &obs, &[first], std::slice::from_ref(&plan),
    &[]
)
            .expect_err("a second validate must refuse");
        assert!(format!("{:?}", err).contains("WrongStage"));
        assert_eq!(record.stage, TransitionRecord::VALIDATED);
        assert!(record.compatible);
    }

    #[test]
    fn validate_blocked_record_refuses_still_blocked() {
        // A validate-produced BLOCKED record re-validated over
        // the same chain: nothing in the replay changed the
        // predicate inputs, so the kernel reports StillBlocked,
        // the call refuses, and the record stands BLOCKED.
        let (state, mut record, obs, first) = empty_reconciling_pair();
        validate_transition_from_records(&state, &mut record, &obs, &[first], &[],
    &[]
)
            .expect("first validate must block");
        assert_eq!(record.stage, TransitionRecord::BLOCKED);
        let err = validate_transition_from_records(&state, &mut record, &obs, &[first], &[],
    &[]
)
            .expect_err("a still-blocked record must refuse re-validate");
        assert!(format!("{:?}", err).contains("ValidateFailed"));
        assert_eq!(record.stage, TransitionRecord::BLOCKED);
        assert!(!record.compatible);
    }

    #[test]
    fn validate_refuses_stored_verdict_mismatch() {
        // The slice-23 promise, enforced: a stored verdict the
        // supplied plan set does not reproduce refuses before any
        // kernel validate runs — here a fitting set whose stored
        // verdict was flipped to false — and the record stands
        // RECONCILING, neither validated nor blocked on a lie.
        let (state, mut record, obs, first, plan) = reconciling_pair();
        record.compatible = false;
        let err = validate_transition_from_records(&state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("a mismatched stored verdict must refuse validate");
        assert!(format!("{:?}", err).contains("ValidateFailed"));
        assert_eq!(record.stage, TransitionRecord::RECONCILING);
        assert!(!record.compatible);
    }

    #[test]
    fn validate_stale_observation_blocks() {
        // At clock 9100 the observation (expiry 9100) is no longer
        // fresh: the predicate fails on freshness alone, so the
        // record blocks — the manifest (expiry 10_100) still
        // stands, so this is a verdict, not a refusal.
        let (mut state, mut record, obs, first, plan) = reconciling_pair();
        advance_clock_on_state(&mut state, 9100).expect("advance to the observation expiry must succeed");
        let life = validate_transition_from_records(&state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect("a stale observation must validate to Blocked, not refuse");
        assert_eq!(life, Life::Blocked);
        assert_eq!(record.stage, TransitionRecord::BLOCKED);
        assert!(record.compatible);
    }

    #[test]
    fn validate_refuses_incomplete_prior_plans() {
        // The record claims a prior plan that was never supplied:
        // the slice-21 anchor refuses before any replay.
        let (state, mut record, obs, first, plan) = reconciling_pair();
        record.plan_count = 2;
        let err = validate_transition_from_records(&state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("a short prior plan set must refuse validate");
        assert!(format!("{:?}", err).contains("ValidateFailed"));
        assert_eq!(record.stage, TransitionRecord::RECONCILING);
        assert!(record.compatible);
        assert_eq!(record.plan_count, 2);
    }

    #[test]
    fn validate_refuses_incomplete_prior_signals() {
        // The record claims a second signal that was never
        // supplied: the slice-16 anchor refuses before any replay.
        let (state, mut record, obs, first, plan) = reconciling_pair();
        record.signal_count = 2;
        let err = validate_transition_from_records(&state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("a short prior signal set must refuse validate");
        assert!(format!("{:?}", err).contains("ValidateFailed"));
        assert_eq!(record.stage, TransitionRecord::RECONCILING);
        assert!(record.compatible);
    }

    #[test]
    fn validate_refuses_unreproducible_plan() {
        // A prior plan whose agent is no seeded agent cannot be
        // replayed: the kernel's agent gate refuses, so the stored
        // RECONCILING label alone validates nothing, and the
        // record stands.
        let (state, mut record, obs, first, mut plan) = reconciling_pair();
        plan.agent = Pubkey::new_from_array([0x77; 32]);
        let err = validate_transition_from_records(&state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("an unreproducible plan must refuse validate");
        assert!(format!("{:?}", err).contains("ValidateFailed"));
        assert_eq!(record.stage, TransitionRecord::RECONCILING);
        assert!(record.compatible);
    }

    #[test]
    fn validate_refuses_unsealed() {
        let (mut state, mut record, obs, first, plan) = reconciling_pair();
        state.sealed = false;
        let err = validate_transition_from_records(&state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("an unsealed state must refuse validate");
        assert!(format!("{:?}", err).contains("NotSealed"));
        assert_eq!(record.stage, TransitionRecord::RECONCILING);
        assert!(record.compatible);
    }

    #[test]
    fn validate_refuses_parent_mismatch() {
        let (mut state, mut record, obs, first, plan) = reconciling_pair();
        state.head_index = 9;
        let err = validate_transition_from_records(&state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("a record off the sealed head must refuse validate");
        assert!(format!("{:?}", err).contains("ParentMismatch"));
        assert_eq!(record.stage, TransitionRecord::RECONCILING);
        assert!(record.compatible);
    }

    fn validated_pair() -> (
        KernelState,
        TransitionRecord,
        ObservationRecord,
        SignalRecord,
        PlanRecord,
    ) {
        // Validated at clock 300 with one fitting plan: the
        // declare-success fixture every slice-25 test builds from.
        let (state, mut record, obs, first, plan) = reconciling_pair();
        let life = validate_transition_from_records(&state, &mut record, &obs, &[first], std::slice::from_ref(&plan),
    &[]
)
            .expect("a fitting reconciled chain must validate");
        assert_eq!(life, Life::Validated);
        assert_eq!(record.stage, TransitionRecord::VALIDATED);
        assert!(record.compatible);
        (state, record, obs, first, plan)
    }

    #[test]
    fn declare_advances_validated_to_declared() {
        // A kernel-proven chain with a fitting plan set and a
        // fresh observation at clock 300: the full predicate
        // passes, the stage advances to 9, and — the slice's
        // head discipline — the sealed head does NOT move.
        let (state, mut record, obs, first, plan) = validated_pair();
        let head_index = state.head_index;
        let head_hash = state.head_hash;
        let life = declare_transition_from_records(&state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect("a fitting validated chain must declare");
        assert_eq!(life, Life::Committable);
        assert_eq!(record.stage, TransitionRecord::DECLARED);
        assert_eq!(record.stage, 9);
        assert!(record.compatible);
        assert_eq!(state.head_index, head_index);
        assert_eq!(state.head_hash, head_hash);
    }

    #[test]
    fn declare_is_once_only_after_declared() {
        // A declared record is past declare: the stage gate
        // refuses a second call, and the DECLARED stage stands.
        let (state, mut record, obs, first, plan) = validated_pair();
        declare_transition_from_records(&state, &mut record, &obs, &[first], std::slice::from_ref(&plan),
    &[]
)
            .expect("first declare must succeed");
        assert_eq!(record.stage, TransitionRecord::DECLARED);
        let err = declare_transition_from_records(&state, &mut record, &obs, &[first], std::slice::from_ref(&plan),
    &[]
)
            .expect_err("a second declare must refuse");
        assert!(format!("{:?}", err).contains("WrongStage"));
        assert_eq!(record.stage, TransitionRecord::DECLARED);
        assert!(record.compatible);
    }

    #[test]
    fn declare_refuses_wrong_stage() {
        // Still PLANNING, never reconciled or validated: the
        // stage gate refuses before any replay, and nothing on
        // the record moves.
        let (state, mut record, obs, first, plan) = one_plan_pair();
        let err = declare_transition_from_records(&state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("an unvalidated record must refuse declare");
        assert!(format!("{:?}", err).contains("WrongStage"));
        assert_eq!(record.stage, TransitionRecord::PLANNING);
        assert!(!record.compatible);
    }

    #[test]
    fn declare_refuses_blocked_record() {
        // A validate-produced BLOCKED record is not validated:
        // the stage gate refuses, and the record stands BLOCKED
        // for the re-plan path — declare never re-labels it.
        let (state, mut record, obs, first) = empty_reconciling_pair();
        validate_transition_from_records(&state, &mut record, &obs, &[first], &[],
    &[]
)
            .expect("first validate must block");
        assert_eq!(record.stage, TransitionRecord::BLOCKED);
        let err = declare_transition_from_records(&state, &mut record, &obs, &[first], &[],
    &[]
)
            .expect_err("a blocked record must refuse declare");
        assert!(format!("{:?}", err).contains("WrongStage"));
        assert_eq!(record.stage, TransitionRecord::BLOCKED);
        assert!(!record.compatible);
    }

    #[test]
    fn declare_refuses_expired_plan_predicate() {
        // The slice's design point, proven end to end: at clock
        // 400 the plan (expiry 400) has expired, so the full
        // predicate fails on the PLANS mask bit — while
        // validate's structural check still passes (fresh
        // observation, unexpired manifest, compatible set). The
        // kernel leaves its life Validated, and the program
        // mirrors it: the record stands VALIDATED, not blocked,
        // not declared.
        let (mut state, mut record, obs, first, plan) = validated_pair();
        advance_clock_on_state(&mut state, 400).expect("advance to the plan expiry must succeed");
        let err = declare_transition_from_records(&state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("an expired-plan predicate must refuse declare");
        assert!(format!("{:?}", err).contains("DeclareFailed"));
        assert_eq!(record.stage, TransitionRecord::VALIDATED);
        assert!(record.compatible);
    }

    #[test]
    fn declare_refuses_stale_observation() {
        // At clock 9100 the observation (expiry 9100) is no
        // longer fresh: the replay validate yields Blocked, so
        // declare refuses — and, crucially, does NOT re-label
        // the record BLOCKED. Declare is not a verdict writer;
        // the stored VALIDATED verdict stands for a later
        // declare or for the exception path to judge.
        let (mut state, mut record, obs, first, plan) = validated_pair();
        advance_clock_on_state(&mut state, 9100).expect("advance to the observation expiry must succeed");
        let err = declare_transition_from_records(&state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("a stale observation must refuse declare");
        assert!(format!("{:?}", err).contains("DeclareFailed"));
        assert_eq!(record.stage, TransitionRecord::VALIDATED);
        assert!(record.compatible);
    }

    #[test]
    fn declare_refuses_stored_verdict_mismatch() {
        // The slice-24 promise, carried into declare: a stored
        // verdict the supplied plan set does not reproduce
        // refuses before any kernel validate runs — here a
        // fitting set whose stored verdict was flipped to
        // false — and the record stands VALIDATED, neither
        // declared nor blocked on a lie.
        let (state, mut record, obs, first, plan) = validated_pair();
        record.compatible = false;
        let err = declare_transition_from_records(&state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("a mismatched stored verdict must refuse declare");
        assert!(format!("{:?}", err).contains("DeclareFailed"));
        assert_eq!(record.stage, TransitionRecord::VALIDATED);
        assert!(!record.compatible);
    }

    #[test]
    fn declare_refuses_incomplete_prior_plans() {
        // The record claims a prior plan that was never
        // supplied: the slice-21 anchor refuses before any
        // replay.
        let (state, mut record, obs, first, plan) = validated_pair();
        record.plan_count = 2;
        let err = declare_transition_from_records(&state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("a short prior plan set must refuse declare");
        assert!(format!("{:?}", err).contains("DeclareFailed"));
        assert_eq!(record.stage, TransitionRecord::VALIDATED);
        assert!(record.compatible);
        assert_eq!(record.plan_count, 2);
    }

    #[test]
    fn declare_refuses_incomplete_prior_signals() {
        // The record claims a second signal that was never
        // supplied: the slice-16 anchor refuses before any
        // replay.
        let (state, mut record, obs, first, plan) = validated_pair();
        record.signal_count = 2;
        let err = declare_transition_from_records(&state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("a short prior signal set must refuse declare");
        assert!(format!("{:?}", err).contains("DeclareFailed"));
        assert_eq!(record.stage, TransitionRecord::VALIDATED);
        assert!(record.compatible);
    }

    #[test]
    fn declare_refuses_unreproducible_plan() {
        // A prior plan whose agent is no seeded agent cannot be
        // replayed: the kernel's agent gate refuses, so the
        // stored VALIDATED label alone declares nothing, and
        // the record stands.
        let (state, mut record, obs, first, mut plan) = validated_pair();
        plan.agent = Pubkey::new_from_array([0x77; 32]);
        let err = declare_transition_from_records(&state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("an unreproducible plan must refuse declare");
        assert!(format!("{:?}", err).contains("DeclareFailed"));
        assert_eq!(record.stage, TransitionRecord::VALIDATED);
        assert!(record.compatible);
    }

    #[test]
    fn declare_refuses_unsealed() {
        let (mut state, mut record, obs, first, plan) = validated_pair();
        state.sealed = false;
        let err = declare_transition_from_records(&state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("an unsealed state must refuse declare");
        assert!(format!("{:?}", err).contains("NotSealed"));
        assert_eq!(record.stage, TransitionRecord::VALIDATED);
        assert!(record.compatible);
    }

    #[test]
    fn declare_refuses_parent_mismatch() {
        let (mut state, mut record, obs, first, plan) = validated_pair();
        state.head_index = 9;
        let err = declare_transition_from_records(&state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("a record off the sealed head must refuse declare");
        assert!(format!("{:?}", err).contains("ParentMismatch"));
        assert_eq!(record.stage, TransitionRecord::VALIDATED);
        assert!(record.compatible);
    }

    fn declared_pair() -> (
        KernelState,
        TransitionRecord,
        ObservationRecord,
        SignalRecord,
        PlanRecord,
    ) {
        // declare-success fixture every slice-26 test builds from.
        let (state, mut record, obs, first, plan) = validated_pair();
        let life = declare_transition_from_records(&state, &mut record, &obs, &[first], std::slice::from_ref(&plan),
    &[]
)
            .expect("a fitting validated chain must declare");
        assert_eq!(life, Life::Committable);
        assert_eq!(record.stage, TransitionRecord::DECLARED);
        (state, record, obs, first, plan)
    }

    #[test]
    fn commit_advances_declared_to_committed_and_moves_head() {
        // A kernel-proven chain with a fitting plan set and a
        // fresh observation at clock 300: the full predicate
        // passes a third time, the stage advances to 10, and —
        // the slice's head discipline, the mirror of declare's
        // — the sealed head DOES move, to exactly the record's
        // declared next hash at index one past the old head.
        // And the move is usable: a new record registers
        // against the new head.
        let (mut state, mut record, obs, first, plan) = declared_pair();
        let old_index = state.head_index;
        let next_hash = record.declared_next_hash;
        let life = commit_transition_from_records(&mut state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect("a fitting declared chain must commit");
        assert_eq!(life, Life::Committed);
        assert_eq!(record.stage, TransitionRecord::COMMITTED);
        assert_eq!(record.stage, 10);
        assert_eq!(state.head_index, old_index + 1);
        assert_eq!(state.head_hash, next_hash);
        let next = build_transition_record(
            &state,
            [0x55; 32],
            TT,
            state.head_index,
            state.head_hash,
            [0x66; 32],
            obs.observation_id,
            record.expires_at,
            record.proposer,
            0,
        )
        .expect("a record against the new head must build");
        assert_eq!(next.parent_index, state.head_index);
        assert_eq!(next.parent_hash, state.head_hash);
    }

    #[test]
    fn commit_is_once_only_after_committed() {
        // A committed record is past commit: the stage gate
        // refuses a second call, and the stage and the head
        // stand exactly where the first commit left them.
        let (mut state, mut record, obs, first, plan) = declared_pair();
        commit_transition_from_records(&mut state, &mut record, &obs, &[first], std::slice::from_ref(&plan),
    &[]
)
            .expect("first commit must succeed");
        assert_eq!(record.stage, TransitionRecord::COMMITTED);
        let head_index = state.head_index;
        let head_hash = state.head_hash;
        let err = commit_transition_from_records(&mut state, &mut record, &obs, &[first], std::slice::from_ref(&plan),
    &[]
)
            .expect_err("a second commit must refuse");
        assert!(format!("{:?}", err).contains("WrongStage"));
        assert_eq!(record.stage, TransitionRecord::COMMITTED);
        assert_eq!(state.head_index, head_index);
        assert_eq!(state.head_hash, head_hash);
    }

    #[test]
    fn commit_refuses_wrong_stage() {
        // Still VALIDATED, never declared: the stage gate
        // refuses before any replay, and nothing moves — the
        // record stands VALIDATED and the head stands.
        let (mut state, mut record, obs, first, plan) = validated_pair();
        let head_index = state.head_index;
        let head_hash = state.head_hash;
        let err = commit_transition_from_records(&mut state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("an undeclared record must refuse commit");
        assert!(format!("{:?}", err).contains("WrongStage"));
        assert_eq!(record.stage, TransitionRecord::VALIDATED);
        assert_eq!(state.head_index, head_index);
        assert_eq!(state.head_hash, head_hash);
    }

    #[test]
    fn commit_refuses_governance_type() {
        // The slice's gate, proven in isolation: a DECLARED
        // record whose type is a governance type refuses with
        // the governance error — not a predicate failure — and
        // the record stands DECLARED with the head unmoved, for
        // the slice-28 commit_governance helper to judge. (No
        // governance record reaches DECLARED through the
        // ordinary chain today, since kernel register refuses
        // governance types; the flipped type stands in for the
        // future governance-registration slice's output.)
        let (mut state, mut record, obs, first, plan) = declared_pair();
        record.transition_type = T_GOV_COUNCIL;
        let head_index = state.head_index;
        let head_hash = state.head_hash;
        let err = commit_transition_from_records(&mut state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("a governance record must refuse ordinary commit");
        assert!(format!("{:?}", err).contains("GovernanceCommitRequired"));
        assert_eq!(record.stage, TransitionRecord::DECLARED);
        assert_eq!(state.head_index, head_index);
        assert_eq!(state.head_hash, head_hash);
    }

    #[test]
    fn commit_refuses_expired_plan_predicate() {
        // At clock 400 the plan (expiry 400) has expired, so
        // the full predicate fails on the PLANS mask bit at
        // commit time — while the replay validate still
        // passes. The kernel leaves its life Committable, and
        // the program mirrors it: the record stands DECLARED
        // and the head stands, for a later commit or for the
        // exception path to judge.
        let (mut state, mut record, obs, first, plan) = declared_pair();
        advance_clock_on_state(&mut state, 400).expect("advance to the plan expiry must succeed");
        let head_index = state.head_index;
        let head_hash = state.head_hash;
        let err = commit_transition_from_records(&mut state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("an expired-plan predicate must refuse commit");
        assert!(format!("{:?}", err).contains("CommitFailed"));
        assert_eq!(record.stage, TransitionRecord::DECLARED);
        assert_eq!(state.head_index, head_index);
        assert_eq!(state.head_hash, head_hash);
    }

    #[test]
    fn commit_refuses_stale_observation() {
        // At clock 9100 the observation (expiry 9100) is no
        // longer fresh: the replay validate yields Blocked, so
        // commit refuses — and, exactly as declare is not a
        // verdict writer, commit is not one either: the record
        // stands DECLARED, not re-labeled BLOCKED, and the head
        // stands.
        let (mut state, mut record, obs, first, plan) = declared_pair();
        advance_clock_on_state(&mut state, 9100).expect("advance to the observation expiry must succeed");
        let head_index = state.head_index;
        let head_hash = state.head_hash;
        let err = commit_transition_from_records(&mut state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("a stale observation must refuse commit");
        assert!(format!("{:?}", err).contains("CommitFailed"));
        assert_eq!(record.stage, TransitionRecord::DECLARED);
        assert_eq!(state.head_index, head_index);
        assert_eq!(state.head_hash, head_hash);
    }

    #[test]
    fn commit_refuses_stored_verdict_mismatch() {
        // The slice-24 promise, carried into commit: a stored
        // verdict the supplied plan set does not reproduce
        // refuses before any kernel validate runs — here a
        // fitting set whose stored verdict was flipped to
        // false — and the record stands DECLARED with the head
        // unmoved, neither committed nor blocked on a lie.
        let (mut state, mut record, obs, first, plan) = declared_pair();
        record.compatible = false;
        let err = commit_transition_from_records(&mut state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("a mismatched stored verdict must refuse commit");
        assert!(format!("{:?}", err).contains("CommitFailed"));
        assert_eq!(record.stage, TransitionRecord::DECLARED);
        assert!(!record.compatible);
        assert_eq!(state.head_index, 0);
    }

    #[test]
    fn commit_refuses_incomplete_prior_plans() {
        // The record claims a prior plan that was never
        // supplied: the slice-21 anchor refuses before any
        // replay, and the head stands.
        let (mut state, mut record, obs, first, plan) = declared_pair();
        record.plan_count = 2;
        let err = commit_transition_from_records(&mut state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("a short prior plan set must refuse commit");
        assert!(format!("{:?}", err).contains("CommitFailed"));
        assert_eq!(record.stage, TransitionRecord::DECLARED);
        assert_eq!(record.plan_count, 2);
        assert_eq!(state.head_index, 0);
    }

    #[test]
    fn commit_refuses_incomplete_prior_signals() {
        // The record claims a second signal that was never
        // supplied: the slice-16 anchor refuses before any
        // replay, and the head stands.
        let (mut state, mut record, obs, first, plan) = declared_pair();
        record.signal_count = 2;
        let err = commit_transition_from_records(&mut state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("a short prior signal set must refuse commit");
        assert!(format!("{:?}", err).contains("CommitFailed"));
        assert_eq!(record.stage, TransitionRecord::DECLARED);
        assert_eq!(state.head_index, 0);
    }

    #[test]
    fn commit_refuses_unreproducible_plan() {
        // A prior plan whose agent is no seeded agent cannot be
        // replayed: the kernel's agent gate refuses, so the
        // stored DECLARED label alone commits nothing, and the
        // record and the head stand.
        let (mut state, mut record, obs, first, mut plan) = declared_pair();
        plan.agent = Pubkey::new_from_array([0x77; 32]);
        let err = commit_transition_from_records(&mut state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("an unreproducible plan must refuse commit");
        assert!(format!("{:?}", err).contains("CommitFailed"));
        assert_eq!(record.stage, TransitionRecord::DECLARED);
        assert_eq!(state.head_index, 0);
    }

    #[test]
    fn commit_refuses_unsealed() {
        let (mut state, mut record, obs, first, plan) = declared_pair();
        state.sealed = false;
        let err = commit_transition_from_records(&mut state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("an unsealed state must refuse commit");
        assert!(format!("{:?}", err).contains("NotSealed"));
        assert_eq!(record.stage, TransitionRecord::DECLARED);
        assert_eq!(state.head_index, 0);
    }

    #[test]
    fn commit_refuses_parent_mismatch() {
        // The head moved out from under the record (a sibling
        // committed first, in program terms): the parent gate
        // refuses, and commit moves the head no further.
        let (mut state, mut record, obs, first, plan) = declared_pair();
        state.head_index = 9;
        let err = commit_transition_from_records(&mut state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("a record off the sealed head must refuse commit");
        assert!(format!("{:?}", err).contains("ParentMismatch"));
        assert_eq!(record.stage, TransitionRecord::DECLARED);
        assert_eq!(state.head_index, 9);
    }

    #[test]
    fn commit_writes_ledger_totals_on_success() {
        // Slice 27's write, proven on the fixture's own plan set
        // (tranche 10, lending book 5, exit window untouched):
        // a successful commit persists exactly those per-resource
        // totals in the ledger, together with the stage advance
        // and the head move — the slice-26 honesty gap, closed
        // at the enforcement point.
        let (mut state, mut record, obs, first, plan) = declared_pair();
        assert_eq!(state.committed_claims, [0, 0, 0]);
        commit_transition_from_records(&mut state, &mut record, &obs, &[first], std::slice::from_ref(&plan),
    &[]
)
            .expect("a fitting declared chain must commit");
        assert_eq!(record.stage, TransitionRecord::COMMITTED);
        assert_eq!(state.committed_claims, [10 * SCALE, 5 * SCALE, 0]);
    }

    #[test]
    fn commit_refuses_when_ledger_plus_new_claims_exceeds_stock() {
        // The ledger gate, at the boundary that matters: prior
        // committed claims of 995 on the tranche stock (1000)
        // plus this transition's 10 exceed the sealed stock, so
        // commit refuses with `CommitFailed` — and, exactly as
        // with the head, a refused commit writes nothing: the
        // record stands `DECLARED`, the head stands, and the
        // ledger stands at the pre-seeded totals, unmoved.
        let (mut state, mut record, obs, first, plan) = declared_pair();
        state.committed_claims = [995 * SCALE, 0, 0];
        let err = commit_transition_from_records(&mut state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("a ledger-exhausting commit must refuse");
        assert!(format!("{:?}", err).contains("CommitFailed"));
        assert_eq!(record.stage, TransitionRecord::DECLARED);
        assert_eq!(state.head_index, 0);
        assert_eq!(state.committed_claims, [995 * SCALE, 0, 0]);
    }

    #[test]
    fn commit_succeeds_at_exact_ledger_capacity() {
        // The gate is a bound, not a hair trigger: prior totals
        // of 990 (tranche) and 495 (lending book) plus this
        // transition's 10 and 5 land exactly on the sealed
        // stocks (1000, 500), so commit succeeds and the ledger
        // records the full stocks — the next commit of any
        // positive claim on either resource must refuse.
        let (mut state, mut record, obs, first, plan) = declared_pair();
        state.committed_claims = [990 * SCALE, 495 * SCALE, 0];
        commit_transition_from_records(&mut state, &mut record, &obs, &[first], std::slice::from_ref(&plan),
    &[]
)
            .expect("an exactly-fitting ledger must commit");
        assert_eq!(record.stage, TransitionRecord::COMMITTED);
        assert_eq!(state.committed_claims, [1000 * SCALE, 500 * SCALE, 0]);
    }

    #[test]
    fn commit_refusal_leaves_preseeded_ledger_untouched() {
        // A refusal on a different gate must not partially
        // write the ledger either: with pre-seeded totals, a
        // stale-observation refusal (clock 9100) leaves the
        // ledger exactly as seeded, alongside the standing
        // `DECLARED` record and head.
        let (mut state, mut record, obs, first, plan) = declared_pair();
        state.committed_claims = [100 * SCALE, 50 * SCALE, 25 * SCALE];
        advance_clock_on_state(&mut state, 9100).expect("advance to the observation expiry must succeed");
        let err = commit_transition_from_records(&mut state, &mut record, &obs, &[first], &[plan],
    &[]
)
            .expect_err("a stale-observation commit must refuse");
        assert!(format!("{:?}", err).contains("CommitFailed"));
        assert_eq!(record.stage, TransitionRecord::DECLARED);
        assert_eq!(state.committed_claims, [100 * SCALE, 50 * SCALE, 25 * SCALE]);
    }

    fn gov_action() -> GovAction {
        // The bridge's governance shape: a capacity rule change,
        // exit-window stock 250 -> 400 in the sealed profile.
        GovAction::SetCapacity { resource: RES_EXIT_WINDOW, capacity: Amount::from_raw(400 * SCALE) }
    }

    fn gov_approvals() -> [[u8; 32]; 3] {
        [ag(1), ag(2), ag(3)]
    }

    fn gov_declared_pair() -> (
        KernelState,
        TransitionRecord,
        ObservationRecord,
        SignalRecord,
        PlanRecord,
    ) {
        // A DECLARED record with the bridge's governance shape:
        // planning opened at clock 200, deadline 300, one
        // qualifying signal, one empty-claims plan — a rule
        // change claims no stock. The chain is driven through
        // the ordinary helpers to DECLARED and then re-typed
        // to the action's category: the slice-26 flipped-type
        // pattern, and the label is never trusted — the
        // slice-28 helper re-proves the whole chain as
        // governance (approvals + register_governance +
        // evaluate + eligible + signals + select + planning +
        // reconcile + validate + declare + commit_governance)
        // from the stored records, the action, and the
        // approvals, so a label earned as ordinary proves
        // nothing the helper does not re-prove itself.
        let (mut state, mut record, obs, first) = planned_pair();
        let plan = submit_plan_from_records(
            &state, &mut record, &obs, &[first], &[], &[], 400, ag(2), 7,
            &[],
)
        .expect("an empty-claims governance plan must submit");
        assert_eq!(record.plan_count, 1);
        advance_clock_on_state(&mut state, 300).expect("advance to the deadline must succeed");
        let life = reconcile_plans_from_records(
            &state, &mut record, &obs, std::slice::from_ref(&first), std::slice::from_ref(&plan),
            &[],
)
        .expect("an empty-claims plan set must reconcile");
        assert_eq!(life, Life::Reconciling);
        assert!(record.compatible);
        let life = validate_transition_from_records(
            &state, &mut record, &obs, std::slice::from_ref(&first), std::slice::from_ref(&plan),
            &[],
)
        .expect("a compatible governance-shaped chain must validate");
        assert_eq!(life, Life::Validated);
        let life = declare_transition_from_records(
            &state, &mut record, &obs, std::slice::from_ref(&first), std::slice::from_ref(&plan),
            &[],
)
        .expect("a validated governance-shaped chain must declare");
        assert_eq!(life, Life::Committable);
        assert_eq!(record.stage, TransitionRecord::DECLARED);
        record.transition_type = T_GOV_ADJUSTABLE;
        (state, record, obs, first, plan)
    }

    #[test]
    fn gov_commit_advances_declared_to_committed_and_moves_head() {
        // Unanimous council approval of the exact action, a
        // fitting empty-claims plan set, and a fresh observation
        // at clock 300: the full governance predicate passes,
        // the stage advances to 10, and the sealed head moves
        // to exactly the record's declared next hash at index
        // one past the old head — the slice-26 head discipline,
        // on the governance path. Governance plans claim no
        // stock, so the ledger stands at zeros through the
        // commit; and a committed record is past commit.
        let (mut state, mut record, obs, first, plan) = gov_declared_pair();
        let action = gov_action();
        let approvals = gov_approvals();
        let old_index = state.head_index;
        let next_hash = record.declared_next_hash;
        let life = commit_governance_from_records(
            &mut state, &mut record, &obs, std::slice::from_ref(&first), std::slice::from_ref(&plan),
            &action, &approvals,
            &[],
)
        .expect("a unanimous governance chain must commit");
        assert_eq!(life, Life::Committed);
        assert_eq!(record.stage, TransitionRecord::COMMITTED);
        assert_eq!(state.head_index, old_index + 1);
        assert_eq!(state.head_hash, next_hash);
        assert_eq!(state.committed_claims, [0, 0, 0]);
        let err = commit_governance_from_records(
            &mut state, &mut record, &obs, std::slice::from_ref(&first), std::slice::from_ref(&plan),
            &action, &approvals,
            &[],
)
        .expect_err("a second governance commit must refuse");
        assert!(format!("{:?}", err).contains("WrongStage"));
        assert_eq!(record.stage, TransitionRecord::COMMITTED);
    }

    #[test]
    fn gov_commit_refuses_non_governance_type() {
        // The reverse of the ordinary helper's gate: an
        // ordinary DECLARED record refuses the governance path
        // with the distinct type error — not a predicate
        // failure — and stands DECLARED with the head unmoved.
        let (mut state, mut record, obs, first, plan) = declared_pair();
        let action = gov_action();
        let approvals = gov_approvals();
        let head_index = state.head_index;
        let head_hash = state.head_hash;
        let err = commit_governance_from_records(
            &mut state, &mut record, &obs, &[first], &[plan], &action, &approvals,
            &[],
)
        .expect_err("an ordinary record must refuse the governance path");
        assert!(format!("{:?}", err).contains("NotGovernanceTransition"));
        assert_eq!(record.stage, TransitionRecord::DECLARED);
        assert_eq!(state.head_index, head_index);
        assert_eq!(state.head_hash, head_hash);
    }

    #[test]
    fn gov_commit_refuses_non_unanimous_approvals() {
        // Two of three seats approve the exact action: the
        // kernel's unanimity gate refuses registration, the
        // record stands DECLARED, and the head stands. A seat
        // approving a different action is the same refusal —
        // to the gate, it is a seat that did not approve this
        // action — so the missing seat covers that shape too.
        let (mut state, mut record, obs, first, plan) = gov_declared_pair();
        let action = gov_action();
        let approvals = [ag(1), ag(2)];
        let head_index = state.head_index;
        let head_hash = state.head_hash;
        let err = commit_governance_from_records(
            &mut state, &mut record, &obs, &[first], &[plan], &action, &approvals,
            &[],
)
        .expect_err("a non-unanimous governance commit must refuse");
        assert!(format!("{:?}", err).contains("GovernanceCommitFailed"));
        assert_eq!(record.stage, TransitionRecord::DECLARED);
        assert_eq!(state.head_index, head_index);
        assert_eq!(state.head_hash, head_hash);
    }

    #[test]
    fn gov_commit_refuses_wrong_category() {
        // A governance-typed record whose type is not the
        // action's category (a council-typed record carrying
        // an adjustable SetCapacity action): kernel
        // register_governance refuses with WrongGovCategory,
        // and the record stands DECLARED with the head unmoved.
        let (mut state, mut record, obs, first, plan) = gov_declared_pair();
        record.transition_type = T_GOV_COUNCIL;
        let action = gov_action();
        let approvals = gov_approvals();
        let err = commit_governance_from_records(
            &mut state, &mut record, &obs, &[first], &[plan], &action, &approvals,
            &[],
)
        .expect_err("a wrong-category governance commit must refuse");
        assert!(format!("{:?}", err).contains("GovernanceCommitFailed"));
        assert_eq!(record.stage, TransitionRecord::DECLARED);
    }

    #[test]
    fn gov_commit_refuses_wrong_stage() {
        let (mut state, mut record, obs, first, plan) = gov_declared_pair();
        record.stage = TransitionRecord::VALIDATED;
        let action = gov_action();
        let approvals = gov_approvals();
        let err = commit_governance_from_records(
            &mut state, &mut record, &obs, &[first], &[plan], &action, &approvals,
            &[],
)
        .expect_err("a merely-validated record must refuse governance commit");
        assert!(format!("{:?}", err).contains("WrongStage"));
        assert_eq!(record.stage, TransitionRecord::VALIDATED);
    }

    #[test]
    fn gov_commit_refuses_expired_plan_predicate() {
        // At clock 400 the plan (expiry 400) has expired, so
        // the full predicate fails at declare time inside the
        // replay, and the record stands DECLARED — the mirror
        // of the ordinary commit's expired-plan refusal.
        let (mut state, mut record, obs, first, plan) = gov_declared_pair();
        advance_clock_on_state(&mut state, 400).expect("advance to the plan expiry must succeed");
        let action = gov_action();
        let approvals = gov_approvals();
        let err = commit_governance_from_records(
            &mut state, &mut record, &obs, &[first], &[plan], &action, &approvals,
            &[],
)
        .expect_err("an expired-plan governance commit must refuse");
        assert!(format!("{:?}", err).contains("GovernanceCommitFailed"));
        assert_eq!(record.stage, TransitionRecord::DECLARED);
    }

    #[test]
    fn gov_commit_refusal_leaves_preseeded_ledger_untouched() {
        // A refusal on a different gate must not partially
        // write the ledger either: with pre-seeded totals, a
        // stale-observation refusal (clock 9100) leaves the
        // ledger exactly as seeded, alongside the standing
        // `DECLARED` record and head.
        let (mut state, mut record, obs, first, plan) = gov_declared_pair();
        state.committed_claims = [100 * SCALE, 50 * SCALE, 25 * SCALE];
        advance_clock_on_state(&mut state, 9100).expect("advance to the observation expiry must succeed");
        let action = gov_action();
        let approvals = gov_approvals();
        let err = commit_governance_from_records(
            &mut state, &mut record, &obs, &[first], &[plan], &action, &approvals,
            &[],
)
        .expect_err("a stale-observation governance commit must refuse");
        assert!(format!("{:?}", err).contains("GovernanceCommitFailed"));
        assert_eq!(record.stage, TransitionRecord::DECLARED);
        assert_eq!(state.committed_claims, [100 * SCALE, 50 * SCALE, 25 * SCALE]);
    }

    #[test]
    fn gov_commit_success_leaves_preseeded_ledger_untouched() {
        // The success-side mirror: governance plans claim no
        // stock, so a commit over pre-seeded totals advances
        // the head and the stage and leaves the ledger exactly
        // as seeded — the gate ran, and found nothing to add.
        let (mut state, mut record, obs, first, plan) = gov_declared_pair();
        state.committed_claims = [100 * SCALE, 50 * SCALE, 25 * SCALE];
        let action = gov_action();
        let approvals = gov_approvals();
        let life = commit_governance_from_records(
            &mut state, &mut record, &obs, &[first], &[plan], &action, &approvals,
            &[],
)
        .expect("a unanimous governance chain over seeded totals must commit");
        assert_eq!(life, Life::Committed);
        assert_eq!(record.stage, TransitionRecord::COMMITTED);
        assert_eq!(state.committed_claims, [100 * SCALE, 50 * SCALE, 25 * SCALE]);
    }

    #[test]
    fn gov_commit_refuses_unsealed() {
        let (mut state, mut record, obs, first, plan) = gov_declared_pair();
        state.sealed = false;
        let action = gov_action();
        let approvals = gov_approvals();
        let err = commit_governance_from_records(
            &mut state, &mut record, &obs, &[first], &[plan], &action, &approvals,
            &[],
)
        .expect_err("an unsealed state must refuse governance commit");
        assert!(format!("{:?}", err).contains("NotSealed"));
        assert_eq!(record.stage, TransitionRecord::DECLARED);
    }

    #[test]
    fn gov_commit_refuses_parent_mismatch() {
        let (mut state, mut record, obs, first, plan) = gov_declared_pair();
        state.head_index = 9;
        let action = gov_action();
        let approvals = gov_approvals();
        let err = commit_governance_from_records(
            &mut state, &mut record, &obs, &[first], &[plan], &action, &approvals,
            &[],
)
        .expect_err("a record off the sealed head must refuse governance commit");
        assert!(format!("{:?}", err).contains("ParentMismatch"));
        assert_eq!(record.stage, TransitionRecord::DECLARED);
    }

    fn committed_pair() -> (
        KernelState,
        TransitionRecord,
        ObservationRecord,
        SignalRecord,
        PlanRecord,
    ) {
        // A DECLARED chain driven through the ordinary commit: the
        // first head move the slice-29 tests replay from. Clock 300,
        // head 1 at the record's declared next hash, ledger
        // tranche 10 / lending book 5.
        let (mut state, mut record, obs, first, plan) = declared_pair();
        let life = commit_transition_from_records(
            &mut state, &mut record, &obs, &[first], std::slice::from_ref(&plan),
            &[],
)
        .expect("a fitting declared chain must commit");
        assert_eq!(life, Life::Committed);
        assert_eq!(record.stage, TransitionRecord::COMMITTED);
        assert_eq!(state.head_index, 1);
        assert_eq!(state.head_hash, record.declared_next_hash);
        assert_eq!(state.committed_claims, [10 * SCALE, 5 * SCALE, 0]);
        (state, record, obs, first, plan)
    }

    #[test]
    fn at_head_returns_moved_head_and_second_registers() {
        // The slice's core proof: a rebuilt kernel advanced by one
        // prior commit stands at index 1 with the first commit's
        // debit already in its stocks (tranche 990, lending book
        // 495 of 1000/500), and a second transition whose parent
        // is that moved head registers in it — the exact step the
        // genesis-rebuilding helpers refuse. The same register
        // against a genesis kernel refuses, proving the head move
        // is what admits it, not a loosened gate.
        let (state, record, obs, first, plan) = committed_pair();
        let prior = PriorCommit {
            transition_record: &record,
            observation_record: &obs,
            prior_signals: std::slice::from_ref(&first),
            prior_plans: std::slice::from_ref(&plan),
        };
        let mut k = kernel_at_head_from_commits(&state, std::slice::from_ref(&prior))
            .expect("a proven prior commit must rebuild to the moved head");
        assert_eq!(k.declared_state().index, 1);
        assert_eq!(k.declared_state().hash, record.declared_next_hash);
        assert_eq!(
            k.capacity_of(RES_TREASURY_TRANCHE).map(|a| a.raw()),
            Some(990 * SCALE)
        );
        assert_eq!(
            k.capacity_of(RES_LENDING_BOOK).map(|a| a.raw()),
            Some(495 * SCALE)
        );
        let observation2 = Observation {
            id: [0xC1; 32],
            timestamp: 400,
            expiry: 9500,
            confidence: Amount::from_raw(900_000),
        };
        k.ingest_observation(&OBSERVER, observation2.clone(), 400)
            .expect("a fresh observation at the later clock must ingest");
        let manifest2 = Manifest {
            transition_type: TT,
            parent_index: 1,
            parent_hash: record.declared_next_hash,
            declared_next_hash: [0x66; 32],
            observation_id: [0xC1; 32],
            expires_at: 10400,
            relevant_groups: BTreeSet::from([1u16]),
            action: None,
        };
        k.register_transition(&ag(1), [0xD2; 32], manifest2, BTreeSet::new(), 400)
            .expect("a second transition against the moved head must register");
        assert_eq!(k.life_of(&[0xD2; 32]), Some(Life::Proposed));
        let mut g = sealed_launch_kernel_env().expect("a genesis kernel must build");
        g.ingest_observation(&OBSERVER, observation2, 400)
            .expect("the same observation must ingest in a genesis kernel");
        let manifest_genesis = Manifest {
            transition_type: TT,
            parent_index: 1,
            parent_hash: record.declared_next_hash,
            declared_next_hash: [0x66; 32],
            observation_id: [0xC1; 32],
            expires_at: 10400,
            relevant_groups: BTreeSet::from([1u16]),
            action: None,
        };
        assert!(
            g.register_transition(&ag(1), [0xD2; 32], manifest_genesis, BTreeSet::new(), 400)
                .is_err(),
            "a genesis kernel must refuse a parent it has not reached"
        );
    }

    #[test]
    fn at_head_empty_priors_returns_genesis_head() {
        // No history, genesis head: the empty chain is the genesis
        // kernel itself, at index 0 with the sealed stocks intact.
        let state = sealed_state();
        let k = kernel_at_head_from_commits(&state, &[])
            .expect("an empty chain at genesis must return the genesis kernel");
        assert_eq!(k.declared_state().index, 0);
        assert_eq!(k.declared_state().hash, [0; 32]);
        assert_eq!(
            k.capacity_of(RES_TREASURY_TRANCHE).map(|a| a.raw()),
            Some(1000 * SCALE)
        );
    }

    #[test]
    fn at_head_refuses_moved_head_with_no_history() {
        // The mirror of the genesis case: a moved head with no
        // supplied history is a missing chain, not a genesis
        // kernel — refusing here is what stops a caller from
        // replaying a successor against stocks that never saw
        // the first commit's debit.
        let (state, _, _, _, _) = committed_pair();
        let err = kernel_at_head_from_commits(&state, &[])
            .err()
            .expect("a moved head with no history must refuse");
        assert!(format!("{:?}", err).contains("ParentMismatch"));
    }

    #[test]
    fn at_head_refuses_non_committed_prior() {
        // A DECLARED record is not history yet: its commit has
        // not moved any head, so replaying it as a prior would
        // invent a head move the program never made.
        let (state, record, obs, first, plan) = declared_pair();
        let prior = PriorCommit {
            transition_record: &record,
            observation_record: &obs,
            prior_signals: std::slice::from_ref(&first),
            prior_plans: std::slice::from_ref(&plan),
        };
        let err = kernel_at_head_from_commits(&state, std::slice::from_ref(&prior))
            .err()
            .expect("a merely-declared prior must refuse");
        assert!(format!("{:?}", err).contains("WrongStage"));
    }

    #[test]
    fn at_head_refuses_broken_parent_link() {
        // A prior whose parent is not the running head breaks
        // the chain before any replay runs: the link, not the
        // predicate, is what refuses.
        let (state, mut record, obs, first, plan) = committed_pair();
        record.parent_hash = [0x99; 32];
        let prior = PriorCommit {
            transition_record: &record,
            observation_record: &obs,
            prior_signals: std::slice::from_ref(&first),
            prior_plans: std::slice::from_ref(&plan),
        };
        let err = kernel_at_head_from_commits(&state, std::slice::from_ref(&prior))
            .err()
            .expect("a prior off the running head must refuse");
        assert!(format!("{:?}", err).contains("ParentMismatch"));
    }

    #[test]
    fn at_head_refuses_unreproducible_prior_plan() {
        // A COMMITTED label with a plan the kernel cannot replay
        // (an agent outside the seeded set) proves nothing: the
        // history refuses, and no kernel at any head is returned.
        let (state, record, obs, first, plan) = committed_pair();
        let bad_plan = PlanRecord {
            agent: Pubkey::new_from_array([0x77; 32]),
            ..plan
        };
        let prior = PriorCommit {
            transition_record: &record,
            observation_record: &obs,
            prior_signals: std::slice::from_ref(&first),
            prior_plans: std::slice::from_ref(&bad_plan),
        };
        let err = kernel_at_head_from_commits(&state, std::slice::from_ref(&prior))
            .err()
            .expect("an unreproducible prior plan must refuse");
        assert!(format!("{:?}", err).contains("CommitFailed"));
    }

    #[test]
    fn at_head_refuses_governance_prior() {
        // A governance prior carries no action and no approvals
        // in its bundle, so its commit path cannot be driven from
        // here — refusing names the gap instead of silently
        // dropping a head move from the chain.
        let (state, mut record, obs, first, plan) = gov_declared_pair();
        record.stage = TransitionRecord::COMMITTED;
        let prior = PriorCommit {
            transition_record: &record,
            observation_record: &obs,
            prior_signals: std::slice::from_ref(&first),
            prior_plans: std::slice::from_ref(&plan),
        };
        let err = kernel_at_head_from_commits(&state, std::slice::from_ref(&prior))
            .err()
            .expect("a governance prior must refuse without its commit path");
        assert!(format!("{:?}", err).contains("GovernanceCommitRequired"));
    }

    #[test]
    fn at_head_refuses_future_deadline_prior() {
        // A prior whose planning deadline lies ahead of the state
        // clock is history from the future: its final steps
        // never ran at a clock the program has reached.
        let (state, mut record, obs, first, plan) = declared_pair();
        record.stage = TransitionRecord::COMMITTED;
        record.planning_deadline = 9999;
        let prior = PriorCommit {
            transition_record: &record,
            observation_record: &obs,
            prior_signals: std::slice::from_ref(&first),
            prior_plans: std::slice::from_ref(&plan),
        };
        let err = kernel_at_head_from_commits(&state, std::slice::from_ref(&prior))
            .err()
            .expect("a future-deadline prior must refuse");
        assert!(format!("{:?}", err).contains("CommitFailed"));
    }

    #[test]
    fn at_head_refuses_unsealed() {
        let (mut state, record, obs, first, plan) = committed_pair();
        state.sealed = false;
        let prior = PriorCommit {
            transition_record: &record,
            observation_record: &obs,
            prior_signals: std::slice::from_ref(&first),
            prior_plans: std::slice::from_ref(&plan),
        };
        let err = kernel_at_head_from_commits(&state, std::slice::from_ref(&prior))
            .err()
            .expect("an unsealed state must refuse");
        assert!(format!("{:?}", err).contains("NotSealed"));
    }

    #[test]
    fn at_head_replays_two_priors_and_debits_both() {
        // Two-commit sequencing, the gap slice 27 named: the
        // second prior is a re-based copy of the declared chain
        // with a strictly later window (eligible 400, consensus
        // 500, planning 600, plan expiry 700, observation
        // 400/9500) — later because the kernel clock never
        // regresses, so a successor's window must open after
        // its predecessor's deadline. Both commits land, the
        // head reaches index 2 at the second declared hash, and
        // the stocks carry both debits (tranche 980, lending
        // book 490): the second commit was judged against what
        // the first consumed, not against the sealed stocks.
        let (mut state, record, obs, first, plan) = committed_pair();
        let first_next = record.declared_next_hash;
        let prior1 = PriorCommit {
            transition_record: &record,
            observation_record: &obs,
            prior_signals: std::slice::from_ref(&first),
            prior_plans: std::slice::from_ref(&plan),
        };
        let (_, record2_declared, obs2_declared, signal2_declared, plan2_declared) =
            declared_pair();
        let record2 = TransitionRecord {
            transition_id: [0xD2; 32],
            parent_index: 1,
            parent_hash: first_next,
            declared_next_hash: [0xE2; 32],
            observation_id: [0xC1; 32],
            stage: TransitionRecord::COMMITTED,
            eligible_at: 400,
            consensus_deadline: 500,
            planning_deadline: 600,
            expires_at: 10400,
            ..record2_declared
        };
        let obs2 = ObservationRecord {
            observation_id: [0xC1; 32],
            timestamp: 400,
            expiry: 9500,
            ..obs2_declared
        };
        let signal2 = SignalRecord {
            transition_id: [0xD2; 32],
            submitted_at: 400,
            ..signal2_declared
        };
        let plan2 = PlanRecord {
            transition_id: [0xD2; 32],
            expires_at: 700,
            ..plan2_declared
        };
        let prior2 = PriorCommit {
            transition_record: &record2,
            observation_record: &obs2,
            prior_signals: std::slice::from_ref(&signal2),
            prior_plans: std::slice::from_ref(&plan2),
        };
        advance_clock_on_state(&mut state, 600).expect("advance to the second deadline must succeed");
        state.head_index = 2;
        state.head_hash = [0xE2; 32];
        let k = kernel_at_head_from_commits(&state, &[prior1, prior2])
            .expect("two proven priors must rebuild to index 2");
        assert_eq!(k.declared_state().index, 2);
        assert_eq!(k.declared_state().hash, [0xE2; 32]);
        assert_eq!(
            k.capacity_of(RES_TREASURY_TRANCHE).map(|a| a.raw()),
            Some(980 * SCALE)
        );
        assert_eq!(
            k.capacity_of(RES_LENDING_BOOK).map(|a| a.raw()),
            Some(490 * SCALE)
        );
    }

    #[test]
    fn at_head_refuses_chain_that_overshoots_program_head() {
        // The same two-prior chain against a program head of 1:
        // the chain lands at 2, the program stands at 1, and the
        // mismatch refuses — a chain longer than the head it is
        // offered against is not silently truncated to fit.
        let (mut state, record, obs, first, plan) = committed_pair();
        let first_next = record.declared_next_hash;
        let prior1 = PriorCommit {
            transition_record: &record,
            observation_record: &obs,
            prior_signals: std::slice::from_ref(&first),
            prior_plans: std::slice::from_ref(&plan),
        };
        let (_, record2_declared, obs2_declared, signal2_declared, plan2_declared) =
            declared_pair();
        let record2 = TransitionRecord {
            transition_id: [0xD2; 32],
            parent_index: 1,
            parent_hash: first_next,
            declared_next_hash: [0xE2; 32],
            observation_id: [0xC1; 32],
            stage: TransitionRecord::COMMITTED,
            eligible_at: 400,
            consensus_deadline: 500,
            planning_deadline: 600,
            expires_at: 10400,
            ..record2_declared
        };
        let obs2 = ObservationRecord {
            observation_id: [0xC1; 32],
            timestamp: 400,
            expiry: 9500,
            ..obs2_declared
        };
        let signal2 = SignalRecord {
            transition_id: [0xD2; 32],
            submitted_at: 400,
            ..signal2_declared
        };
        let plan2 = PlanRecord {
            transition_id: [0xD2; 32],
            expires_at: 700,
            ..plan2_declared
        };
        let prior2 = PriorCommit {
            transition_record: &record2,
            observation_record: &obs2,
            prior_signals: std::slice::from_ref(&signal2),
            prior_plans: std::slice::from_ref(&plan2),
        };
        advance_clock_on_state(&mut state, 600).expect("advance to the second deadline must succeed");
        let err = kernel_at_head_from_commits(&state, &[prior1, prior2])
            .err()
            .expect("a chain overshooting the program head must refuse");
        assert!(format!("{:?}", err).contains("ParentMismatch"));
    }

    // Slice 30 fixtures: a second transition at the moved head.
    // The prior is the committed_pair chain (head 1, planning
    // deadline 300, ledger tranche 10 / lending book 5); the
    // second window is strictly later (eligible 400, consensus
    // 500, planning 600, plan expiry 700, observation 400/9500)
    // because the kernel clock never regresses — a successor's
    // window must open after its predecessor's deadline.
    fn second_eligible_at_head(
        state: &mut KernelState,
        prior: &PriorCommit,
        id: [u8; 32],
        obs_id: [u8; 32],
        declared: [u8; 32],
    ) -> (TransitionRecord, ObservationRecord) {
        let clock = state.clock;
        let mut record = build_transition_record(
            state, id, TT, state.head_index, state.head_hash, declared, obs_id, 10_400,
            Pubkey::new_from_array(ag(1)), 7,
        )
        .expect("a record against the moved head must build");
        let obs = build_observation_record(
            state, obs_id, clock, 9500, 900_000, observer_pubkey(), 7,
        )
        .expect("an observation at the moved-head clock must build");
        replay_transition_from_records(state, &mut record, &obs, std::slice::from_ref(prior))
            .expect("a second transition must replay at the moved head");
        assert_eq!(record.stage, TransitionRecord::KERNEL_REGISTERED);
        evaluate_transition_from_records(state, &mut record, &obs, std::slice::from_ref(prior))
            .expect("a second transition must evaluate at the moved head");
        assert_eq!(record.stage, TransitionRecord::EVALUATED);
        mark_eligible_from_records(state, &mut record, &obs, std::slice::from_ref(prior))
            .expect("a second transition must mark eligible at the moved head");
        assert_eq!(record.stage, TransitionRecord::ELIGIBLE);
        assert_eq!(record.eligible_at, clock);
        assert_eq!(record.consensus_deadline, clock + 100);
        (record, obs)
    }

    fn second_signal_at_head(
        state: &KernelState,
        record: &mut TransitionRecord,
        obs: &ObservationRecord,
        prior: &PriorCommit,
        agent_n: u8,
        support_raw: u64,
    ) -> SignalRecord {
        let (signal, _, _) = submit_signal_from_records(
            state, record, obs, &[], Pubkey::new_from_array(ag(agent_n)),
            0, SCALE, support_raw, true, 7, std::slice::from_ref(prior),
        )
        .expect("a second transition's signal must submit at the moved head");
        assert_eq!(record.signal_count, 1);
        signal
    }

    #[test]
    fn head_threading_second_commit_succeeds_through_all_stages() {
        // End-to-end two-commit sequencing through the helpers
        // (not just the slice-29 constructor): every stage of a
        // second transition replays from the moved head, the
        // second commit judges its plan against the post-first
        // stocks (tranche 990, lending book 495) and the program
        // ledger (10/5 already committed), and the head moves
        // to index 2 at the second declared hash.
        let (mut state, prior_rec, prior_obs, prior_sig, prior_plan) = committed_pair();
        let prior = PriorCommit {
            transition_record: &prior_rec,
            observation_record: &prior_obs,
            prior_signals: std::slice::from_ref(&prior_sig),
            prior_plans: std::slice::from_ref(&prior_plan),
        };
        advance_clock_on_state(&mut state, 400).expect("advance past the first deadline must succeed");
        let (mut record, obs) =
            second_eligible_at_head(&mut state, &prior, [0xD2; 32], [0xC1; 32], [0xE2; 32]);
        let signal = second_signal_at_head(&state, &mut record, &obs, &prior, 2, SCALE);
        advance_clock_on_state(&mut state, 500).expect("advance to the consensus deadline must succeed");
        let life = select_candidate_from_records(
            &state, &mut record, &obs, std::slice::from_ref(&signal), std::slice::from_ref(&prior),
        )
        .expect("a closed window with a qualifying tally must select at the moved head");
        assert_eq!(life, Life::Selected);
        let life = open_planning_from_records(
            &state, &mut record, &obs, std::slice::from_ref(&signal), [0x77; 32],
            std::slice::from_ref(&prior),
        )
        .expect("a kernel-proven selection must open planning at the moved head");
        assert_eq!(life, Life::Planning);
        assert_eq!(record.planning_deadline, 600);
        let plan = submit_plan_from_records(
            &state, &mut record, &obs, std::slice::from_ref(&signal), &[], &two_claims(), 700,
            ag(2), 7, std::slice::from_ref(&prior),
        )
        .expect("a fitting plan must submit at the moved head");
        assert_eq!(record.plan_count, 1);
        advance_clock_on_state(&mut state, 600).expect("advance to the planning deadline must succeed");
        let life = reconcile_plans_from_records(
            &state, &mut record, &obs, std::slice::from_ref(&signal),
            std::slice::from_ref(&plan), std::slice::from_ref(&prior),
        )
        .expect("a fitting plan set must reconcile at the moved head");
        assert_eq!(life, Life::Reconciling);
        assert!(record.compatible);
        let life = validate_transition_from_records(
            &state, &mut record, &obs, std::slice::from_ref(&signal),
            std::slice::from_ref(&plan), std::slice::from_ref(&prior),
        )
        .expect("a fitting reconciled chain must validate at the moved head");
        assert_eq!(life, Life::Validated);
        let life = declare_transition_from_records(
            &state, &mut record, &obs, std::slice::from_ref(&signal),
            std::slice::from_ref(&plan), std::slice::from_ref(&prior),
        )
        .expect("a validated chain must declare at the moved head");
        assert_eq!(life, Life::Committable);
        let life = commit_transition_from_records(
            &mut state, &mut record, &obs, std::slice::from_ref(&signal),
            std::slice::from_ref(&plan), std::slice::from_ref(&prior),
        )
        .expect("a fitting declared chain must commit at the moved head");
        assert_eq!(life, Life::Committed);
        assert_eq!(record.stage, TransitionRecord::COMMITTED);
        assert_eq!(state.head_index, 2);
        assert_eq!(state.head_hash, [0xE2; 32]);
        assert_eq!(state.committed_claims, [20 * SCALE, 10 * SCALE, 0]);
        let prior2 = PriorCommit {
            transition_record: &record,
            observation_record: &obs,
            prior_signals: std::slice::from_ref(&signal),
            prior_plans: std::slice::from_ref(&plan),
        };
        let k = kernel_at_head_from_commits(&state, &[prior, prior2])
            .expect("both priors must rebuild to index 2");
        assert_eq!(k.declared_state().index, 2);
        assert_eq!(
            k.capacity_of(RES_TREASURY_TRANCHE).map(|a| a.raw()),
            Some(980 * SCALE)
        );
        assert_eq!(
            k.capacity_of(RES_LENDING_BOOK).map(|a| a.raw()),
            Some(490 * SCALE)
        );
    }

    #[test]
    fn head_threading_sibling_select_succeeds() {
        // The thirteenth helper at the moved head: target and
        // sibling both eligible at head 1 (census 2), target
        // confidence 1.0 against a 0-confidence sibling, both
        // windows 400/500 — the merged timeline replays from
        // the moved head and the target selects at 500.
        let (mut state, prior_rec, prior_obs, prior_sig, prior_plan) = committed_pair();
        let prior = PriorCommit {
            transition_record: &prior_rec,
            observation_record: &prior_obs,
            prior_signals: std::slice::from_ref(&prior_sig),
            prior_plans: std::slice::from_ref(&prior_plan),
        };
        advance_clock_on_state(&mut state, 400).expect("advance past the first deadline must succeed");
        let (mut target, target_obs) =
            second_eligible_at_head(&mut state, &prior, [0xD2; 32], [0xC1; 32], [0xE2; 32]);
        let (mut sib, sib_obs) =
            second_eligible_at_head(&mut state, &prior, [0xD3; 32], [0xC2; 32], [0xE3; 32]);
        assert_eq!(state.eligible_sibling_count, 2);
        let target_sig = second_signal_at_head(&state, &mut target, &target_obs, &prior, 2, SCALE);
        let sib_sig = second_signal_at_head(&state, &mut sib, &sib_obs, &prior, 2, 0);
        advance_clock_on_state(&mut state, 500).expect("advance to the consensus deadline must succeed");
        let sibling = SiblingReplay {
            transition_record: &sib,
            observation_record: &sib_obs,
            prior_signals: std::slice::from_ref(&sib_sig),
        };
        let life = select_candidate_from_records_with_siblings(
            &state, &mut target, &target_obs, std::slice::from_ref(&target_sig),
            std::slice::from_ref(&sibling), std::slice::from_ref(&prior),
        )
        .expect("target outranking its sibling must select at the moved head");
        assert_eq!(life, Life::Selected);
        assert_eq!(target.stage, TransitionRecord::SELECTED);
    }

    #[test]
    fn head_threading_governance_commit_succeeds() {
        // The governance path at the moved head: an ordinary
        // second chain to DECLARED with an empty-claims plan,
        // re-typed to the action's category (the slice-28
        // flipped-type pattern), then a unanimous governance
        // commit over the prior — head to 2, ledger untouched
        // (governance plans claim no stock).
        let (mut state, prior_rec, prior_obs, prior_sig, prior_plan) = committed_pair();
        let prior = PriorCommit {
            transition_record: &prior_rec,
            observation_record: &prior_obs,
            prior_signals: std::slice::from_ref(&prior_sig),
            prior_plans: std::slice::from_ref(&prior_plan),
        };
        advance_clock_on_state(&mut state, 400).expect("advance past the first deadline must succeed");
        let (mut record, obs) =
            second_eligible_at_head(&mut state, &prior, [0xD2; 32], [0xC1; 32], [0xE2; 32]);
        let signal = second_signal_at_head(&state, &mut record, &obs, &prior, 2, SCALE);
        advance_clock_on_state(&mut state, 500).expect("advance to the consensus deadline must succeed");
        select_candidate_from_records(
            &state, &mut record, &obs, std::slice::from_ref(&signal), std::slice::from_ref(&prior),
        )
        .expect("a closed window with a qualifying tally must select at the moved head");
        open_planning_from_records(
            &state, &mut record, &obs, std::slice::from_ref(&signal), [0x77; 32],
            std::slice::from_ref(&prior),
        )
        .expect("a kernel-proven selection must open planning at the moved head");
        let plan = submit_plan_from_records(
            &state, &mut record, &obs, std::slice::from_ref(&signal), &[], &[], 700,
            ag(2), 7, std::slice::from_ref(&prior),
        )
        .expect("an empty-claims governance plan must submit at the moved head");
        advance_clock_on_state(&mut state, 600).expect("advance to the planning deadline must succeed");
        reconcile_plans_from_records(
            &state, &mut record, &obs, std::slice::from_ref(&signal),
            std::slice::from_ref(&plan), std::slice::from_ref(&prior),
        )
        .expect("an empty-claims plan set must reconcile at the moved head");
        assert!(record.compatible);
        validate_transition_from_records(
            &state, &mut record, &obs, std::slice::from_ref(&signal),
            std::slice::from_ref(&plan), std::slice::from_ref(&prior),
        )
        .expect("a compatible governance-shaped chain must validate at the moved head");
        declare_transition_from_records(
            &state, &mut record, &obs, std::slice::from_ref(&signal),
            std::slice::from_ref(&plan), std::slice::from_ref(&prior),
        )
        .expect("a validated governance-shaped chain must declare at the moved head");
        record.transition_type = T_GOV_ADJUSTABLE;
        let action = gov_action();
        let approvals = gov_approvals();
        let life = commit_governance_from_records(
            &mut state, &mut record, &obs, std::slice::from_ref(&signal),
            std::slice::from_ref(&plan), &action, &approvals, std::slice::from_ref(&prior),
        )
        .expect("a unanimous governance chain must commit at the moved head");
        assert_eq!(life, Life::Committed);
        assert_eq!(record.stage, TransitionRecord::COMMITTED);
        assert_eq!(state.head_index, 2);
        assert_eq!(state.head_hash, [0xE2; 32]);
        assert_eq!(state.committed_claims, [10 * SCALE, 5 * SCALE, 0]);
    }

    #[test]
    fn head_threading_refuses_moved_head_without_history() {
        // Empty priors at a moved head are a missing chain, not
        // a genesis kernel: the helper refuses with
        // ParentMismatch and the record stands REGISTERED.
        let (mut state, _, _, _, _) = committed_pair();
        advance_clock_on_state(&mut state, 400).expect("advance past the first deadline must succeed");
        let mut record = build_transition_record(
            &state, [0xD2; 32], TT, state.head_index, state.head_hash, [0xE2; 32],
            [0xC1; 32], 10_400, Pubkey::new_from_array(ag(1)), 7,
        )
        .expect("a record against the moved head must build");
        let obs = build_observation_record(
            &state, [0xC1; 32], 400, 9500, 900_000, observer_pubkey(), 7,
        )
        .expect("an observation at the moved-head clock must build");
        let err = replay_transition_from_records(&state, &mut record, &obs, &[])
            .expect_err("a moved head with no history must refuse");
        assert!(format!("{:?}", err).contains("ParentMismatch"));
        assert_eq!(record.stage, TransitionRecord::REGISTERED);
    }

    #[test]
    fn head_threading_refuses_broken_prior_link() {
        // A prior whose parent is not the running head breaks
        // the chain before any replay runs: the link, not the
        // predicate, refuses, and the target stands REGISTERED.
        let (mut state, prior_rec, prior_obs, prior_sig, prior_plan) = committed_pair();
        let mut bad = prior_rec;
        bad.parent_hash = [0x99; 32];
        let bad_prior = PriorCommit {
            transition_record: &bad,
            observation_record: &prior_obs,
            prior_signals: std::slice::from_ref(&prior_sig),
            prior_plans: std::slice::from_ref(&prior_plan),
        };
        advance_clock_on_state(&mut state, 400).expect("advance past the first deadline must succeed");
        let mut record = build_transition_record(
            &state, [0xD2; 32], TT, state.head_index, state.head_hash, [0xE2; 32],
            [0xC1; 32], 10_400, Pubkey::new_from_array(ag(1)), 7,
        )
        .expect("a record against the moved head must build");
        let obs = build_observation_record(
            &state, [0xC1; 32], 400, 9500, 900_000, observer_pubkey(), 7,
        )
        .expect("an observation at the moved-head clock must build");
        let err = replay_transition_from_records(
            &state, &mut record, &obs, std::slice::from_ref(&bad_prior),
        )
        .expect_err("a prior off the running head must refuse");
        assert!(format!("{:?}", err).contains("ParentMismatch"));
        assert_eq!(record.stage, TransitionRecord::REGISTERED);
    }

    #[test]
    fn bundle_split_parses_one_bundle_and_tail() {
        // One committed bundle (transition, observation, its signal,
        // its plan) plus one trailing target signal: the bundle parses
        // to its own records and the tail carries exactly the extra
        // account, in order.
        let (_state, rec, obs, sig, plan) = committed_pair();
        let tail_sig = sig;
        let parsed = alloc::vec![
            ParsedAccount::Transition(rec),
            ParsedAccount::Observation(obs),
            ParsedAccount::Signal(sig),
            ParsedAccount::Plan(plan),
            ParsedAccount::Signal(tail_sig),
        ];
        let (bundles, tail) =
            split_prior_bundles(parsed, 1).expect("a well-linked bundle must parse");
        assert_eq!(bundles.len(), 1);
        assert_eq!(bundles[0].signals.len(), 1);
        assert_eq!(bundles[0].plans.len(), 1);
        assert_eq!(tail.len(), 1);
        assert!(matches!(tail[0], ParsedAccount::Signal(_)));
        let priors = prior_commits_of(&bundles);
        assert_eq!(priors.len(), 1);
        assert_eq!(priors[0].prior_signals.len(), 1);
        assert_eq!(priors[0].prior_plans.len(), 1);
    }

    #[test]
    fn bundle_split_zero_bundles_leaves_all_as_tail() {
        // Genesis instructions pass count 0: nothing is consumed and
        // the whole list is the target's own evidence.
        let (_state, rec, obs, sig, plan) = committed_pair();
        let parsed = alloc::vec![
            ParsedAccount::Transition(rec),
            ParsedAccount::Observation(obs),
            ParsedAccount::Signal(sig),
            ParsedAccount::Plan(plan),
        ];
        let (bundles, tail) =
            split_prior_bundles(parsed, 0).expect("zero bundles must parse");
        assert!(bundles.is_empty());
        assert_eq!(tail.len(), 4);
    }

    #[test]
    fn bundle_split_refuses_wrong_observation_link() {
        // The observation after the transition names another id: the
        // bundle mis-links and refuses before any replay runs.
        let (_state, rec, mut obs, sig, plan) = committed_pair();
        obs.observation_id = [0x99; 32];
        let parsed = alloc::vec![
            ParsedAccount::Transition(rec),
            ParsedAccount::Observation(obs),
            ParsedAccount::Signal(sig),
            ParsedAccount::Plan(plan),
        ];
        let err = split_prior_bundles(parsed, 1)
            .expect_err("a mis-linked observation must refuse");
        assert!(format!("{:?}", err).contains("RecordMismatch"));
    }

    #[test]
    fn bundle_split_refuses_signal_for_another_transition() {
        // A signal naming a different transition inside the bundle's
        // signal run: the count is right but the linkage is wrong.
        let (_state, rec, obs, mut sig, plan) = committed_pair();
        sig.transition_id = [0x99; 32];
        let parsed = alloc::vec![
            ParsedAccount::Transition(rec),
            ParsedAccount::Observation(obs),
            ParsedAccount::Signal(sig),
            ParsedAccount::Plan(plan),
        ];
        let err = split_prior_bundles(parsed, 1)
            .expect_err("a foreign signal must refuse");
        assert!(format!("{:?}", err).contains("RecordMismatch"));
    }

    #[test]
    fn bundle_split_refuses_plan_for_another_transition() {
        // Same shape on the plan run: a plan naming a different
        // transition refuses, even with the counts satisfied.
        let (_state, rec, obs, sig, mut plan) = committed_pair();
        plan.transition_id = [0x99; 32];
        let parsed = alloc::vec![
            ParsedAccount::Transition(rec),
            ParsedAccount::Observation(obs),
            ParsedAccount::Signal(sig),
            ParsedAccount::Plan(plan),
        ];
        let err = split_prior_bundles(parsed, 1)
            .expect_err("a foreign plan must refuse");
        assert!(format!("{:?}", err).contains("RecordMismatch"));
    }

    #[test]
    fn bundle_split_refuses_truncated_and_overcounted() {
        // Two refusals, one cause: the stored counts are not met by
        // the supplied accounts — a bundle missing its plan, and a
        // count of 2 bundles where only one exists.
        let (_state, rec, obs, sig, _plan) = committed_pair();
        let short = alloc::vec![
            ParsedAccount::Transition(rec),
            ParsedAccount::Observation(obs),
            ParsedAccount::Signal(sig),
        ];
        let err = split_prior_bundles(short, 1)
            .expect_err("a bundle short of its plan count must refuse");
        assert!(format!("{:?}", err).contains("RecordMismatch"));
        let (_state, rec, obs, sig, plan) = committed_pair();
        let one = alloc::vec![
            ParsedAccount::Transition(rec),
            ParsedAccount::Observation(obs),
            ParsedAccount::Signal(sig),
            ParsedAccount::Plan(plan),
        ];
        let err = split_prior_bundles(one, 2)
            .expect_err("a count past the list must refuse");
        assert!(format!("{:?}", err).contains("RecordMismatch"));
    }

    #[test]
    fn bundle_split_refuses_wrong_first_variant() {
        // A bundle must open with its transition record: an
        // observation first is a layout error, not a bundle.
        let (_state, rec, obs, sig, plan) = committed_pair();
        let parsed = alloc::vec![
            ParsedAccount::Observation(obs),
            ParsedAccount::Transition(rec),
            ParsedAccount::Signal(sig),
            ParsedAccount::Plan(plan),
        ];
        let err = split_prior_bundles(parsed, 1)
            .expect_err("a bundle not opening with its transition must refuse");
        assert!(format!("{:?}", err).contains("RecordMismatch"));
    }

}
