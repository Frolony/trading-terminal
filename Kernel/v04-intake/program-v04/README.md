# program-v04 — migration step 5, slice 5: the program scaffold

The delivered v0.4.1 kernel crate compiled into an Anchor program under
the v04-line toolchain (Anchor 1.2.2 / Solana 4.3.2 / platform-tools
v1.57, migrated 2026-10-29 under Carlos's pin-migration ruling; the
1.2.1 / 4.1.2 / v1.54 pins are superseded for this program). One probe
instruction only (`probe_kernel`); the instruction families, account
inventory, and CU golden set are later step-5 slices.

## What it proves

- `anchor build` green under the pinned toolchain: the v0.4 crate's
  no_std + alloc build links into a program (`.so` + generated IDL).
- The program crate exercises the real kernel constructor on the host:
  4 host tests (genesis unsealed state; even/zero seats and zero epoch
  refused) — the constructor's own validation, not a stub.
- The probe config carries the step-2 launch pin (reputation influence
  OFF, 3 council seats) so the first on-chain kernel starts from the
  same policy the host bridge replays.

## What it does not prove (later slices)

- No transition replay on-chain, no CU figures — the bridge crate
  (`program-bridge-v04`, slices 1–4) proves the replay shapes on the
  host; cu-golden measures them once instructions exist.
- No deployment: the program id in `Anchor.toml` is staging-only (the
  generating keypair was deleted after the pubkey was taken).

## Slice 6 — the §4.1 genesis instruction + the first §3 account

- `initialize_kernel`: constructs the real probe-config kernel and
  persists its genesis state (unsealed, head 0, zero hash, clock 0)
  into the `KernelState` PDA (seeds `["kernel-state"]`, SPACE 58).
  One-time via the `init` constraint. The design's genesis row is a
  multisig-signed `initialize_config`; this slice's genesis authority
  is the initializing payer, and the multisig wiring is a later §4.1
  slice, not claimed here.
- `KernelState` carries the design's `DeclaredChain` fields
  (`head_index`/`head_hash`) alongside `sealed`/`clock`; no standalone
  `DeclaredChain` account is split out in this slice.
- Host tests 8/8 (4 probe + 4 slice-6: genesis state, SPACE accounting,
  Borsh round-trip at exactly SPACE bytes, and the seal refusal below).
- `anchor build` green under the pinned toolchain; IDL now 2
  instructions (`initialize_kernel`, `probe_kernel`), 1 account
  (`KernelState`), 2 errors (`KernelProbe` 6000, `KernelInit` 6001),
  1 event (`KernelInitialized`); `.so` 157,000 bytes.
- **No `seal_kernel` in this slice, by design:** the kernel's
  `seal_genesis` refuses until the launch genesis is complete (full
  council, four governance policies, influence-share bound), and those
  steps live in the std-only step-2 launch crate, which cannot compile
  to SBF as-is. An account-only seal flag would claim a seal the
  kernel never granted, so none is written; the seal instruction is
  slice 7's work, gated on a no_std port of the launch genesis steps.
  The host test pins the refusal (`GenesisNotReady` on the probe
  kernel), it is not faked.

## Slice 7 — the `seal_kernel` instruction (the slice-6 gate, closed)

- The no_std port of the step-2 launch genesis steps: same genesis
  authority GEN, same policy values, same six agents at base 2.0, same
  operator roles, same three capacity stocks, same 3-seat council —
  but with `alloc` collections and `Amount::from_raw` only (the
  step-2 crate's `from_f64` convenience is std-only and never used
  here), so the sequence compiles to SBF. Genesis steps run under
  `NoEnv`: they never consult the environment, so that is exact, not
  a shortcut.
- `seal_kernel`: runs the full sequence through the real kernel,
  seals it, and persists the sealed flag, the kernel clock (100), and
  the declared head (index 0, zero hash) into the `KernelState` PDA.
  Refused `AlreadySealed` (6003) if the account already records a
  seal. The kernel object itself is NOT persisted — the account
  records the seal outcome, and later slices' per-record accounts are
  what let instructions continue from a sealed kernel. Seal authority
  is the signing payer; the multisig wiring is a later §4.1 slice.
- Host tests 14/14 (8 prior + 6 slice-7: launch config pinned to the
  step-2 profile, sealed kernel at genesis head, seal refused without
  the council (`GenesisNotReady("COUNCIL")`), re-seal refused by the
  kernel itself (`NotGenesis` — the genesis authority is erased),
  account sealed by the instruction helper, second seal refused
  without moving the account).
- `anchor build` green under the pinned toolchain; IDL now 3
  instructions (`initialize_kernel`, `probe_kernel`, `seal_kernel`),
  1 account (`KernelState`), 4 errors (6000–6003), 2 events
  (`KernelInitialized`, `KernelSealed`); `.so` 284,984 bytes
  (sha256 `071ee2a6…`).

## Slice 8 — the `TransitionRecord` account + `register_transition`

- `TransitionRecord` (SPACE 218): the stored manifest — id, type,
  parent index/hash, declared next hash, observation id, expiry,
  proposer (signing payer), `stage` (0 = Registered), bump. PDA
  seeds `["transition", transition_id]`; a second record for the same
  id is refused by the `init` constraint.
- `register_transition`: gates on the sealed head — refuses an
  unsealed `KernelState` (`NotSealed` 6004) and any parent that is not
  exactly the sealed head (`ParentMismatch` 6005, index AND hash).
  It does NOT run kernel `register_transition`: that call additionally
  needs an ingested observation and a registered-agent proposer, and
  the pubkey-to-agent mapping for both is a later slice — so the record
  is a stored manifest awaiting replay, not a registered transition,
  and later slices advance `stage` from it.
- Host tests 20/20 (14 prior + 6 slice-8: SPACE accounting, Borsh
  round-trip at exactly SPACE bytes, unsealed refusal, index-mismatch
  refusal, hash-mismatch refusal, sealed-genesis-head acceptance with
  all fields pinned).
- `anchor build` green under the pinned toolchain; IDL now 4
  instructions (adds `register_transition`), 2 accounts (adds
  `TransitionRecord`), 6 errors (6000–6005), 3 events (adds
  `TransitionRegistered`); `.so` 304,704 bytes
  (sha256 `f28ca7ca…`).

## Slice 9 — the `ObservationRecord` account, `ingest_observation`, and the pubkey-to-agent mapping

- `ObservationRecord` (SPACE 97): the stored observation — id, timestamp,
  expiry, confidence in kernel raw units (SCALE = 1.0), observing signer,
  bump. PDA seeds `["observation", observation_id]`; a second record for
  the same id is refused by the `init` constraint.
- `ingest_observation`: gates on the sealed state (`NotSealed`), the
  signing payer's Observer role in the sealed launch profile
  (`NotObserver`), and the kernel's ingest window rules against
  `KernelState.clock` (`ObservationFromFuture` 6006, `ObservationBadWindow`
  6007). At the sealed genesis clock of 100 it admits the bridge's t=100
  observations; clock advancement is a later slice before post-genesis
  observations can ingest. Like slice 8 it stores the record — it does not
  run kernel `ingest_observation`, because the kernel object is not
  persisted; the later replay slice runs the kernel call from this record.
- The mapping rule is identity, not a registry: a signer's agent id IS the
  signer's pubkey bytes (`resolve_agent_id`), so the signature is the
  proof and no account grants an id the signer does not hold.
  `resolve_proposer` gates that identity against the sealed launch profile
  (registered AND active; `UnknownProposer` 6008): the Observer-role holder
  is role-bearing but not a registered agent, so it is refused as a proposer,
  and an unknown pubkey is refused outright. Both gates replay the sealed
  launch kernel, so they judge the pinned profile, not a caller-supplied
  roster; a governance-changed roster is a later slice's replay scope.
- Host tests 32/32 (20 prior + 12 slice-9: SPACE accounting, Borsh
  round-trip at exactly SPACE bytes, identity mapping, unsealed/non-observer/
  unknown/future/expired/overconfident refusals, observer acceptance at the
  genesis clock, proposer acceptance for seeded agents, proposer refusal for
  observer and unknown, and the combined proof that the resolved observer
  id ingests and the resolved proposer id registers on a real sealed kernel).
- `anchor build` green under the pinned toolchain; IDL now 5 instructions
  (adds `ingest_observation`), 3 accounts (adds `ObservationRecord`),
  10 errors (6000–6009), 4 events (adds `ObservationIngested`).

## Slice 10 — the first replay instruction, `replay_transition`

- `replay_transition`: runs kernel `ingest_observation` +
  `register_transition` from a stored `ObservationRecord` +
  `TransitionRecord` pair against a freshly rebuilt sealed launch
  kernel, and advances the transition record's `stage` from
  `REGISTERED` (0) to `KERNEL_REGISTERED` (1). Gates, in order:
  sealed state (`NotSealed`), once-only stage (`WrongStage` 6011),
  parent exactly the sealed head (`ParentMismatch`), observation
  linkage (`RecordMismatch` 6010), proposer resolved against the
  sealed profile (`UnknownProposer`), observer holding the Observer
  role there (`NotObserver`). A kernel refusal in either call maps
  to `ReplayFailed` (6012): governance and unknown types, an expired
  manifest, and an overconfident observation all fail through the
  kernel, not through a program-side guess.
- The manifest's `relevant_groups` are derived, not supplied: the
  proposer's own groups in the sealed profile (every seeded agent is
  in group 1, matching the bridge's ordinary record). That derivation
  is this slice's rule, recorded in the program docs, not a kernel
  rule — a transition needing groups beyond its proposer's must store
  those groups in the record in a later manifest-completion slice.
- Honesty bounds, stated not smoothed: replay creates no new
  authority — it executes steps the stored records already authorized
  (slice 8's register signature, slice 9's ingest signature) and the
  payer is a fee payer only. Replay is once-only per record because
  the rebuilt kernel is fresh each time and cannot refuse a duplicate
  itself — the stage byte is the duplicate guard. Replay proves
  registration only: the kernel is rebuilt at the genesis head, so a
  replayed transition is `Proposed` against head 0 and `KernelState`
  does not advance; sequencing past the genesis head, the
  evaluate/eligible/select/reconcile/validate/declare steps, CU
  measurement, and clock advancement are later slices. Replay runs at
  `KernelState.clock` (100 at genesis), so only genesis-clock pairs
  replay.
- Host tests 41/41 (32 prior + 9 slice-10: genesis-pair acceptance
  with stage advance and SPACE preserved, unsealed / second-replay /
  parent-mismatch / observation-mismatch / unknown-proposer /
  non-observer refusals, expired-transition and overconfident-
  observation kernel refusals, governance- and unknown-type kernel
  refusals — every refusal leaves the stage untouched).
- `anchor build` green under the pinned toolchain; IDL now 6
  instructions (adds `replay_transition`), 3 accounts (unchanged),
  13 errors (6000–6012), 5 events (adds `TransitionReplayed`); `.so`
  386,784 bytes (sha256 `ee10d4c7…`).

## Slice 11 — the clock instruction, `advance_clock`

- `advance_clock`: moves `KernelState.clock` forward to `new_clock`
  via the pure helper `advance_clock_on_state`. Gates, in order:
  sealed state (`NotSealed`); forward-only movement
  (`ClockRegression` 6013 on a strictly lower value — an equal value
  is an accepted no-op, mirroring the kernel's own private `tick`).
  The head (`head_index`/`head_hash`) and the seal flag are never
  touched: clock advancement is not chain advancement, and only a
  later commit replay moves the head.
- This closes the slice-9/10 limitation: `KernelState.clock` is the
  `now` later instructions pass to the freshly rebuilt kernel, so a
  clock at 200 admits observations timestamped after genesis (150
  ingests, 250 replays after chained advances), and a clock advanced
  past a window lapses it — a pair whose observation expired at 9100
  fails replay through the kernel (`ReplayFailed`) once the clock
  reaches 9100, with the stage untouched.
- Honesty bounds, stated not smoothed: authority in this slice is
  the signing payer, the same explicitly recorded gap as slices 6/7
  (multisig/role wiring is a later §4.1 slice, not claimed here).
  Advancing the clock by itself moves no money and expires nothing
  on-chain — expiry bites only when a later instruction replays
  against the new clock. No CU measurement; that rides the
  cu-golden freeze. The further transition-replay families
  (evaluate/eligible/select/reconcile/validate/declare, then commit
  replay moving the head) are later slices.
- Host tests 49/49 (41 prior + 8 slice-11: forward advance with head
  preserved, unsealed / regression refusals with the clock untouched,
  equal-value no-op, later-observation unblocked, post-genesis replay
  unblocked, expiry-lapsed replay refused through the kernel, chained
  advances then replay — every refusal leaves the clock untouched).
- `anchor build` green under the pinned toolchain; IDL now 7
  instructions (adds `advance_clock`), 3 accounts (unchanged),
  14 errors (6000–6013), 6 events (adds `ClockAdvanced`).

## Slice 12 — the evaluate replay instruction, `evaluate_transition`

- `evaluate_transition`: takes a `KERNEL_REGISTERED` record pair and
  runs kernel `evaluate_transition` on it — after replaying the
  slice-10 ingest + register sequence on a freshly rebuilt sealed
  launch kernel, because the kernel object is not persisted and
  evaluation needs a `Proposed` transition to judge. Only
  `Life::Evaluated` advances the stage, to `EVALUATED` (2); a kernel
  refusal, and a non-Evaluated life, both map to `EvaluateFailed`
  (6014) with the stage untouched — the stage records only a proven
  evaluation, and a verdict-bearing `Rejected` stage is a later
  slice's work, not claimed here.
- The kernel here runs under `ProgramEnv`, the program-side port of
  the step-2 launch env (C1 = Pass, O1 = true, everything else the
  trait defaults, so an unlisted constraint still answers
  Unavailable). This is the slice's load-bearing point, proven by
  test rather than assumed: under the delivered crate's default
  `NoEnv`, C1 answers Unavailable and `evaluate_transition` returns
  Ok without moving the life — a NoEnv evaluate could never advance
  a stage. The env answers are the administrator's answers baked
  into the program, never taken from an instruction argument.
- Gates, in order: sealed state (`NotSealed`); stage exactly
  `KERNEL_REGISTERED` (`WrongStage` — a merely-registered record has
  nothing to evaluate, an evaluated record evaluates once only);
  parent exactly the sealed head (`ParentMismatch`); observation
  linkage (`RecordMismatch`); proposer/observer gates against the
  sealed profile (`UnknownProposer`/`NotObserver`); the relevant
  groups are slice 10's proposer-groups derivation, reused here.
- Host tests 59/59 (49 prior + 10 slice-12: evaluate acceptance with
  stage advance and SPACE preserved, registered-stage and
  once-only refusals, unsealed / parent-mismatch /
  observation-mismatch / unknown-proposer refusals, evaluation at an
  advanced clock (200), the `ProgramEnv` answer table, and the NoEnv
  stays-`Proposed` premise test — every refusal leaves the stage
  untouched).
- `anchor build` green under the pinned toolchain; IDL now 8
  instructions (adds `evaluate_transition`), 3 accounts (unchanged),
  15 errors (6000–6014), 7 events (adds `TransitionEvaluated`); `.so`
  440,328 bytes (sha256 `f7840903…`).
- Honesty bounds, stated not smoothed: eligible/select/reconcile/
  validate/declare, commit replay moving the head, and CU measurement
  are later slices. Authority is the signing payer, the same recorded
  gap as slices 6/7.

## Slice 13 — the eligible replay instruction, `mark_eligible`

- `mark_eligible`: takes an `EVALUATED` record pair and runs kernel
  `mark_eligible` on it — after replaying the ingest + register +
  evaluate sequence on a freshly rebuilt sealed launch kernel under
  `ProgramEnv`, because the kernel object is not persisted and
  eligibility needs an `Evaluated` life whose `constraints_ok` /
  `objectives_ok` flags evaluate recorded. Only `Life::Eligible`
  advances the stage, to `ELIGIBLE` (3); a kernel refusal, and a
  non-Eligible life, both map to `EligibleFailed` (6015) with the
  stage untouched.
- The feasibility gate is the kernel's: `constraints_ok` must be true
  (evaluate under `ProgramEnv` records it, since C1 passes there),
  and the objectives gate binds only when the policy requires
  objectives — the launch policy's `require_objectives` is false, so
  that check is inert for this profile and is not claimed as a gate.
  Observation freshness and manifest expiry are re-checked by the
  kernel at this step, so a window lapsed since evaluation fails
  here rather than being carried forward.
- Gates, in order: sealed state (`NotSealed`); stage exactly
  `EVALUATED` (`WrongStage` — a merely kernel-registered record has
  no evaluation to make eligible, an eligible record marks once
  only); parent exactly the sealed head (`ParentMismatch`);
  observation linkage (`RecordMismatch`); proposer/observer gates
  against the sealed profile (`UnknownProposer`/`NotObserver`); the
  relevant groups are slice 10's proposer-groups derivation, reused
  here.
- Host tests 69/69 (59 prior + 10 slice-13: eligible acceptance with
  stage advance and SPACE preserved, kernel-registered-stage and
  once-only refusals, unsealed / parent-mismatch /
  observation-mismatch / unknown-proposer / non-observer refusals,
  eligibility at an advanced clock (200), and a lapsed observation
  (expiry 9100) refused at the advanced clock — every refusal leaves
  the stage untouched).
- `anchor build` green under the pinned toolchain; IDL now 9
  instructions (adds `mark_eligible`), 3 accounts (unchanged),
  16 errors (6000–6015), 8 events (adds `TransitionMarkedEligible`); `.so`
  491,800 bytes (sha256 `80cbb40e…`).
- Honesty bounds, stated not smoothed: the sibling field
  `mark_eligible` joins lives only inside the freshly rebuilt kernel
  and is not persisted — the program records the stage only, not
  `eligible_at` / `consensus_deadline` — so a later slice must persist
  those fields before `select_candidate` can be proven against a real
  field. Select/reconcile/validate/declare, commit replay moving the
  head, and CU measurement are later slices. Authority is the signing
  payer, the same recorded gap as slices 6/7.

## Slice 14 — the eligibility-window persistence slice 13 named

- `TransitionRecord` gains `eligible_at` / `consensus_deadline`
  (SPACE 218 → 234); `mark_eligible_from_records` writes them on
  success only — the state's clock, and that clock plus the sealed
  launch policy's `consensus_window` (100) — so a later select slice
  reads a real field rather than a freshly rebuilt one. No new
  instruction, no new error, no stage-gate change: the `mark_eligible`
  instruction's meaning grows by exactly this persistence.
- The values are derived from the same inputs the kernel's own
  `mark_eligible` used: the delivered crate exposes no `eligible_at`
  accessor, so the program derives rather than reads, and the
  derivation is proven by test at two clocks (100 → 100/200,
  200 → 200/300), not assumed. A refusal leaves both fields at 0, so
  0 means "eligibility never proven" and can never be mistaken for a
  window.
- Host tests 71/71 (69 prior + 2 slice-14: window persistence at the
  advanced clock, and a refusal leaving both fields at 0; the slice-13
  acceptance test now also asserts 100/200, and the borsh round-trip
  now carries a 200/300 window).
- `anchor build` green under the pinned toolchain; IDL counts
  unchanged (9 instructions, 3 accounts, 16 errors 6000–6015,
  8 events) with the `TransitionRecord` type gaining the two `u64`
  fields; `.so` 492,368 bytes (sha256 `35750964…`).
- Deliberately NOT slice 14: a `select_candidate` instruction — the
  signal tally it needs has no persisted record yet, so a select this
  slice could only refuse, the no-op pattern slice 12 rejected. Select
  stays a later slice, now unblocked on the window field and still
  blocked on signal persistence.

## Slice 15 — the signal persistence slice 14 named

- The fourth §3 account, `SignalRecord` (SPACE 106): one record per
  (transition, agent, round) — PDA seeds `["signal", transition_id,
  agent, round_le]` — carrying raw, support (0 when no position),
  the `has_support` flag, and `submitted_at`, the state's clock at
  submission. A new round is a new record, never an overwrite, so the
  record set is the audit trail the select replay walks; the latest
  record per agent is the one the kernel counts.
- The `submit_signal` instruction: the round is an instruction
  argument the helper re-derives from the stored window and refuses
  on mismatch, never trusts. The helper replays the full chain before
  the new signal — ingest + register + evaluate + mark_eligible at
  the stored `eligible_at` (so the rebuilt kernel's window IS the
  stored window), then every supplied prior signal at its own
  `submitted_at`, then the new signal at the state's clock — and
  returns the kernel's tally (total weight, confidence) alongside the
  record, plus a `SignalSubmitted` event carrying them. Prior signals
  arrive as remaining accounts on-chain. Signals do not advance the
  transition stage: it stays `ELIGIBLE`, and the window is untouched.
- Gates, in order: sealed state (`NotSealed`); stage exactly
  `ELIGIBLE` (`WrongStage`); parent exactly the sealed head
  (`ParentMismatch`); observation linkage (`RecordMismatch`); a
  proven window (nonzero `eligible_at`, deadline after it) and a
  round argument equal to the derived round (`SignalFailed`); every
  prior belonging to this transition with a stored round matching its
  own `submitted_at` (`RecordMismatch`); proposer/observer gates
  against the sealed profile (`UnknownProposer`/`NotObserver`); the
  relevant groups are slice 10's proposer-groups derivation, reused
  here. Every kernel refusal in the replay maps to `SignalFailed`:
  same-round duplicate, revision away from the last round's mean,
  signal at or past the deadline, unknown/inactive agent, over-range
  raw or support.
- Host tests 86/86 (71 prior + 15 slice-15: round-slice derivation,
  SPACE accounting, borsh round-trip, first-signal acceptance with
  the kernel tally (weight 2.0, confidence 1.0), no-position signal
  at zero weight with support normalization, wrong-stage /
  observation-mismatch / unknown-agent / over-range /
  wrong-round-arg / foreign-prior refusals, same-round duplicate
  refused, later-round same-support revision accepted,
  away-from-mean revision refused, two-agent tally accumulation
  (weight 4.0, confidence exactly 0.5), signal at the deadline
  refused).
- `anchor build` green under the pinned toolchain; IDL now 10
  instructions (adds `submit_signal`), 4 accounts (adds
  `SignalRecord`), 17 errors (6000–6016, adds `SignalFailed`),
  9 events (adds `SignalSubmitted`); `.so` 614,584 bytes (sha256
  `59891a10…`).
- Deliberately NOT slice 15: a `select_candidate` instruction, and
  the completeness anchor select needs — `submit_signal` replays the
  priors the caller supplies, so a record that could only pass by
  omitting a prior can never survive the select replay, which replays
  the full set, but proving a supplied set IS the full set needs a
  count anchored on the transition record (or an equivalent), and
  that is the select slice's work, named here rather than claimed
  here. Select/reconcile/validate/declare, commit replay moving the
  head, and CU measurement are later slices.

## Slice 16 — the select instruction slice 15 named

- The completeness anchor: `signal_count` on `TransitionRecord`
  (SPACE 234 → 242), written by `submit_signal` on success only —
  one per created record, revisions included — so on-chain the count
  and the record set cannot diverge: one instruction creates both or
  neither, and a refusal leaves the count untouched. Slice 15's "the
  transition record is not mutated" is amended here, not contradicted
  silently: signals still do not advance the stage and still do not
  touch the window; only the count moves.
- The `select_candidate` instruction: prior signals arrive as
  remaining accounts and must number exactly `signal_count`, or the
  instruction refuses before any replay; a set that omits a real
  record can only reach the count by duplicating one, and a
  duplicated (agent, round) is refused explicitly before the replay
  and again by the kernel's `AlreadySignalledThisRound` on replay.
  The helper replays the full chain — ingest + register + evaluate
  + mark_eligible at the stored `eligible_at`, then every supplied
  prior at its own `submitted_at` — and runs kernel
  `select_candidate` at the state's clock, so window closure and the
  quorum/confidence gates are the kernel's. Only a `Life::Selected`
  life advances the stage, to `SELECTED` (4). The payer is a fee payer
  only: selection is permissionless once the window has closed.
- Honesty bound, stated not smoothed: the rebuilt kernel holds only
  this transition, so this slice proves own qualification and window
  closure, NOT the kernel's sibling comparison — a higher-confidence
  sibling that exists on-chain but is not replayed here cannot be
  judged, and `OutrankedBySibling` / `SiblingsStillOpen` /
  `ParentAlreadyHasASelectedTransition` are unreachable here.
  Sibling replay is the next slice's work.
- Host tests 95/95 (86 prior + 9 slice-16: selection advancing an
  eligible record to stage 4, refusal before the deadline, wrong-stage
  refusal, incomplete-set refusal (count 2, supplied 1), duplicate-set
  refusal (count 2, supplied one record twice), unqualified-tally
  refusal (confidence 0 below the 0.5 threshold), foreign-prior
  refusal, once-only select, and the count rule itself — +1 per
  created record, unchanged by a refused signal).
- `anchor build` green under the pinned toolchain; IDL now 11
  instructions (adds `select_candidate`), 4 accounts (unchanged),
  18 errors (6000–6017, adds `SelectFailed`), 10 events (adds
  `TransitionSelected`); `.so` 667,352 bytes (sha256
  `74047e44…`); `TransitionRecord` fields in order with
  `signal_count` before `bump`.
- Deliberately NOT slice 16: sibling replay (the select
  completeness bound above covers this transition's signals, not its
  siblings), reconcile/validate/declare, commit replay moving the
  head, and CU measurement — all later slices.

## Slice 17 — sibling replay for select

- The `select_candidate_with_siblings` instruction: remaining
  accounts carry the sibling evidence, one account per stored record,
  classified by discriminator — sibling `TransitionRecord`s, their
  `ObservationRecord`s, and `SignalRecord`s (target priors and
  sibling priors alike, told apart by each signal's
  `transition_id`). A signal for no supplied transition, a sibling
  without exactly one matching observation, an observation matching
  no sibling, a duplicated sibling id, or an unclassifiable
  remaining account refuses with `RecordMismatch` before any replay.
- The helper replays every supplied transition into one rebuilt
  kernel as ONE merged timeline — eligibility chains and signals
  interleaved in ascending time order, chains before signals at an
  equal time — because two phases would regress the kernel clock
  whenever a transition's signal predates another transition's
  eligibility, which is the normal interleaved case. The
  mark-eligible order is the kernel's siblings-vec order, and
  `eligible_at` IS that order; on an equal `eligible_at`, siblings
  replay before the target, so an equal-confidence tie at equal
  eligibility refuses the target (the fail-closed direction).
- The kernel's sibling arms now fire: `SiblingsStillOpen` for a
  sibling whose window is still open, `OutrankedBySibling` for a
  qualifying sibling with higher confidence or an earlier
  equal-confidence eligibility, and an expired sibling is skipped
  exactly as the kernel skips it. Each sibling's supplied signal set
  must number exactly its own `signal_count`; a sibling at any stage
  but `ELIGIBLE` refuses.
- Host tests 108/108 (95 prior + 13 slice-17: target wins over a
  weaker sibling, outranked by a 1.0-confidence sibling over a 0.5
  target, open-sibling refusal, tie with the earlier sibling
  winning, tie with the earlier target winning, equal-eligibility
  tie refusing the target, expired sibling ignored, incomplete
  sibling set, non-sibling parent, wrong-stage sibling, duplicate
  sibling, mismatched sibling observation, and empty field reducing
  to plain select).
- `anchor build` green under the pinned toolchain; IDL now 12
  instructions (adds `select_candidate_with_siblings`), 4 accounts
  (unchanged), 18 errors (unchanged), 10 events (unchanged); `.so`
  710,656 bytes (sha256 `45dcfdea…`).
- Deliberately NOT slice 17: the already-selected sibling replay
  (a `SELECTED` sibling's own select replay — the
  `ParentAlreadyHasASelected` arm), the sibling-set completeness
  anchor (no count anchor for siblings exists on `KernelState`, so
  an omitted blocking sibling is still a caller omission),
  reconcile/validate/declare, commit replay moving the head, and CU
  measurement — all later slices.

## Slice 18 — already-selected sibling replay for select

- Slice 17's "a sibling at any stage but `ELIGIBLE` refuses" is
  amended: a sibling at stage `SELECTED` (4) is now admitted, and
  every other non-`ELIGIBLE` stage still refuses. The stage label
  is NOT trusted: after the merged timeline, the helper replays
  the selected sibling's own select at the state's clock and
  verifies its life becomes `Selected` before running the
  target's select — call order, not a stored selection timestamp
  (no such field exists), is what makes the sibling "already"
  selected. Only then can the kernel's
  `ParentAlreadyHasASelectedTransition` arm fire on the target.
- A `SELECTED` sibling that cannot be proven selected refuses the
  whole helper: unqualified tally, an outranking target, or a
  window not yet closed all leave the target `ELIGIBLE`. At most
  one `SELECTED` sibling may be supplied — a parent can have only
  one selected transition — so two refuse at the gate, before any
  replay. The per-sibling `signal_count` anchor still applies to
  selected siblings.
- Host tests 115/115 (108 prior + 7 slice-18: selected sibling
  blocks the target, unqualified selected sibling refuses,
  outranked selected sibling refuses, selected sibling with an
  open window refuses, two selected siblings refuse, mixed
  selected-plus-eligible field blocks the target, and incomplete
  selected sibling signal set refuses).
- `anchor build` green under the pinned toolchain; IDL unchanged
  (12 instructions, 4 accounts, 18 errors, 10 events — no new
  instruction, the slice-17 instruction's meaning grows); `.so`
  712,144 bytes (sha256 `30b54f87…`).
- Deliberately NOT slice 18: the sibling-set completeness anchor
  (still no count anchor for siblings on `KernelState`),
  later-stage siblings (planning onward, stages 5+, do not exist
  in the program yet, so their `ParentAlreadyHasASelected` lives
  are not replayable here), reconcile/validate/declare, commit
  replay moving the head, and CU measurement — all later slices.

## Slice 19 — sibling-set completeness anchor for select

- `KernelState` gains the census slice 17 named and slice 18 left
  open: `eligible_sibling_parent_index` / `eligible_sibling_parent_hash`
  plus `eligible_sibling_count` (SPACE 58 -> 106). `mark_eligible`
  writes it on success only — the same instruction advances the
  stage and the census or neither, so count and eligible set cannot
  diverge, exactly as slice 16's `signal_count` is written by
  `submit_signal`. A transition becoming eligible at the census
  parent increments the count; the first eligible transition at a
  new parent resets the census to that parent with count 1. The
  `mark_eligible` instruction's `kernel_state` account is now `mut`;
  the IDL gains no instruction (12 instructions, 4 accounts,
  18 errors, 10 events, unchanged), only that account's writability.
- Both select helpers gate on the census before any replay. The
  plain select requires count exactly 1 at the target's parent —
  otherwise a second eligible transition exists and the plain path,
  which rebuilds a kernel holding only the target, would silently
  bypass the comparison. The sibling select requires supplied
  siblings + target to equal the count at the target's parent, so
  an omitted eligible competitor refuses instead of passing
  unjudged. The census parent must equal the target's parent in
  both: a count recorded at another parent authorizes nothing, and
  the next `mark_eligible` at the new parent resets it lazily, so
  the future commit-replay slice that moves the head need not
  reset it and must not trust it across the move either. The
  census counts every transition that ever became eligible at this
  parent — selected or still eligible, expired or not — because
  the kernel's sibling loop judges only `Eligible` siblings at the
  parent (plus the `selected_for` record, itself formerly
  eligible), and an omitted expired sibling is indistinguishable
  on-chain from an omitted live one.
- Host tests 122/122 (115 prior + 7 slice-19: genesis census empty
  at the genesis head, census advances 1 -> 2 across two
  eligibilities, census resets at a new parent, refused
  eligibility leaves the census, plain select refuses when the
  census reports another eligible transition, sibling select
  refuses an omitted sibling, sibling select refuses a census at
  another parent). All 20 prior sibling tests now set the census
  to their supplied field size, so their kernel arms are still
  what is proven, not the gate.
- `anchor build` green under the pinned toolchain; `.so`
  721,744 bytes (sha256 `751d44b2…`).
- Deliberately NOT slice 19: later-stage siblings (planning
  onward — the with-siblings gate still admits only `ELIGIBLE` /
  `SELECTED`, so a sibling advanced past selection cannot be
  supplied, though that select must fail anyway since
  `selected_for` already names it), reconcile/validate/declare,
  commit replay moving the head, and CU measurement — later slices.

## Slice 20 — open planning (`SELECTED` -> `PLANNING`)

- The `open_planning` instruction: `TransitionRecord` gains
  `planning_deadline` (SPACE 242 -> 250) and the `PLANNING` stage
  value 5. The helper re-proves the whole selection chain in a
  rebuilt kernel (chain + priors + select at the state's clock,
  with the prior set required to number exactly `signal_count`)
  and only then runs kernel `open_planning` at the state's
  clock — a stored `SELECTED` label a replay cannot reproduce
  never opens planning. The persisted deadline is the state's
  clock plus the launch policy's planning window (100): the
  sealed launch kernel installs `launch_policy()` for every
  type, so this is the kernel's own policy value. The caller is
  the signing payer's agent id, and the kernel's Selected-path
  rule is "anyone" — the tests prove a non-proposer, non-agent
  payer opens planning. A `Blocked` record refuses here: the
  kernel's proposer-or-seat re-plan path is later work, not
  opened by this instruction. No plan is submitted here.
- Host tests 130/130 (122 prior + 8 slice-20: selected ->
  planning with deadline 300 at clock 200, wrong-stage refusal,
  once-only, incomplete prior set refusal, selection re-proven
  at the clock (a SELECTED label with the clock moved back
  before the deadline refuses), later-clock deadline 600 at
  clock 500, unsealed refusal, parent-mismatch refusal).
  `anchor build` green under the pinned toolchain; the IDL gains
  the 13th instruction, the 19th error (`PlanningFailed`), and
  the 11th event (`TransitionPlanningOpened`) — 13 instructions,
  4 accounts, 19 errors, 11 events; `.so` 752,464 bytes (sha256
  `8b11c147…`).
- Deliberately NOT slice 20: `submit_plan` (its own record
  account, next slice), the Blocked -> Planning re-plan path,
  reconcile/validate/declare, commit replay moving the head,
  and CU measurement — later slices.

## Slice 21 — submit plan (`PLANNING` gains a per-agent plan)

- The `submit_plan` instruction: a `PlanRecord` PDA per
  (transition, agent) — a second plan from the same agent fails
  at account creation, and the kernel's `Duplicate` arm is the
  second guard in replay. The transition stage stays `PLANNING`
  and `plan_count` increments on success only: the completeness
  anchor the later reconcile slice checks its supplied plan set
  against, exactly as `signal_count` anchors select.
  `TransitionRecord` SPACE grows 250 -> 258 for the new count.
- Two replay clocks, not one, and the distinction is the design:
  select and open_planning replay at the *derived open clock*
  (`planning_deadline - planning_window`), because that is the
  clock the stored deadline was set from — replaying them at the
  state's clock would silently move the deadline a plan is judged
  against — while prior plans and the new plan run at the state's
  clock, gated strictly inside the stored window. The new plan's
  expiry is judged against the *stored* deadline, not a replayed
  one. The plan's agent is the signing payer's agent id, and
  unlike open_planning's anyone rule, the kernel's agent gate
  applies here: an unknown or inactive agent, or one outside the
  proposer's groups, refuses through the kernel. Claims are a
  bounded multi-resource map (`MAX_PLAN_CLAIMS` = 8, parallel
  fixed arrays, SPACE 162): a longer list refuses rather than
  truncating, and a duplicated resource refuses rather than
  collapsing silently the way a map insert would. The
  instruction's remaining accounts split deterministically (the
  first `signal_count` are signals, the rest plans).
- Host tests 143/143 (130 prior + 13 slice-21: record space and
  borsh round-trip, a two-resource plan from a seeded agent with
  stage/deadline/count asserted, a second agent's plan replaying
  the first, wrong-stage refusal, at-deadline refusal,
  expiry-at-deadline refusal, unknown-agent refusal, duplicate
  agent refusal, duplicate resource refusal, incomplete prior
  plan set refusal, incomplete prior signal set refusal, and
  unsealed refusal).
- `anchor build` green under the pinned toolchain; the IDL gains
  the 14th instruction (`submit_plan`, with the `PlanClaimArg`
  claim type), the 5th account (`PlanRecord`), the 20th error
  (`PlanFailed`), and the 12th event (`PlanSubmitted`) — 14
  instructions, 5 accounts, 20 errors, 12 events; `.so`
  829,456 bytes (sha256 `b6c88fa2…`).
- Deliberately NOT slice 21: the Blocked -> Planning re-plan
  path, reconcile/validate/declare, commit replay moving the
  head, and CU measurement — later slices.

## Slice 22 — Blocked re-plan (`BLOCKED` -> `PLANNING`)

- The `open_planning` instruction now admits a `BLOCKED` record
  (stage 6) as well as `SELECTED`, on the kernel's re-plan rule
  enforced by the helper from the stored record, in the kernel's
  own order: caller is the record's proposer or a seat of the
  sealed launch council (seats 1–3, pinned genesis), and the
  stored replan count is below the kernel's `MAX_REPLANS` (3,
  imported from the kernel crate, never re-stated). A stranger
  or a capped record refuses with the new `ReplanRefused` error
  and nothing moves. On success the stage advances to
  `PLANNING`, the deadline resets to `clock + planning_window`,
  and `replans` increments by exactly one; the Selected path
  leaves `replans` untouched, and a re-plan never clears
  `plan_count` or the stored plan set, because the kernel
  carries its plan map across a re-plan unchanged.
  `TransitionRecord` SPACE grows 258 -> 262 for the new
  `replans: u32` field.
- Honesty bound, stated not smoothed: the replay kernel reaches
  `Selected`, so the kernel call itself takes its anyone path —
  the re-plan authority and cap live in the helper, not in the
  replay, because no slice can yet replay a transition INTO
  `Blocked` (reconcile/validate are later slices). The stored
  `BLOCKED` label is trusted this slice in exactly one narrow
  sense, and it is safe to trust that far: no instruction writes
  `BLOCKED` yet, so no live record can carry it, and the label
  becomes program-written — hence re-provable — the day validate
  lands. Until then the re-plan tests set the label as a fixture
  standing in for validate's future output. Selection, the
  proposer, and the seat roster are all re-proven in every call.
- Host tests 150/150 (143 prior + 7 slice-22: proposer re-plan,
  seat-only re-plan, non-seat proposer re-plan, stranger
  refusal, cap refusal, second-open refusal, Selected path
  leaving `replans` untouched — the fixture deadline is 250,
  not 300, so success proves the deadline reset).
- `anchor build` green under the pinned toolchain; the IDL gains
  the 21st error (`ReplanRefused`) and the `TransitionRecord`
  type gains the `replans: u32` field — 14 instructions, 5
  accounts, 21 errors, 12 events; `.so` 831,808 bytes (sha256
  `34bdb513…`).
- Deliberately NOT slice 22: reconcile/validate/declare,
  commit replay moving the head, and CU measurement — later
  slices.

## Slice 23 — reconcile plans (`PLANNING` -> `RECONCILING`)

- The `reconcile_plans` instruction re-proves the whole chain in
  a rebuilt kernel (chain + priors + select + open_planning +
  every prior plan) and only then runs kernel `reconcile_plans`
  at the state's clock, advancing a `PLANNING` record to
  `RECONCILING` (stage 7, a new program stage value) and
  persisting the kernel's compatibility verdict on the record
  (`compatible: bool`, SPACE 262 -> 263) and on the new
  `TransitionReconciled` event. The payer is a fee payer only:
  kernel `reconcile_plans` takes no caller, so no proposer, seat,
  or agent gate applies.
- Replay clocks, and the slice's design point: select and
  open_planning replay at the derived open clock
  (`planning_deadline - planning_window`), as in slice 21, and
  prior plans replay at that same open clock — because a
  `PlanRecord` carries no submission time, no replay can run a
  plan at its true submission clock, and the open clock is the
  one window clock every stored plan is guaranteed to satisfy
  (inside the window, expiry after the deadline — the only clock
  rules kernel `submit_plan` applies besides manifest expiry,
  which the state's clock already proves). Reconcile itself runs
  at the state's clock, which must sit at or after the stored
  deadline; before it the kernel's `PlanningWindowOpen` refuses.
- The verdict is a state, not a refusal: an empty plan set
  reconciles to incompatible (the kernel never calls a plan-less
  transition compatible), and an over-capacity set reconciles to
  incompatible while still advancing the stage — only a window
  still open, an unreproducible chain, or an incomplete prior set
  refuses, with the record standing exactly as it stood. The
  stored verdict is the exact mirror of the kernel's
  `fits_capacity` over the supplied plan set (same sums, same
  checked-add, same sealed capacity stocks), so the later
  validate slice can cross-check it against its own replay.
- Host tests 161/161 (150 prior + 11 slice-23: compatible
  advance at the deadline exactly, empty-set incompatibility,
  over-capacity incompatibility (2 x 600 against a 1000 stock),
  before-deadline refusal, wrong-stage refusal, once-only,
  incomplete prior plans, incomplete prior signals,
  unreproducible-plan refusal, unsealed refusal, parent
  mismatch). Clippy back to the pre-existing profile (anchor
  macro `unexpected cfg` noise + the slice-8 helper's
  `type_complexity` note); zero new warnings.
- `anchor build` green under the pinned toolchain; the IDL gains
  the `reconcile_plans` instruction, the 22nd error
  (`ReconcileFailed`), the 13th event (`TransitionReconciled`),
  and the `compatible: bool` field on `TransitionRecord` — 15
  instructions, 5 accounts, 22 errors, 13 events; `.so` 875,088
  bytes (sha256 `c7c0a274…`).
- Deliberately NOT slice 23: validate/declare, commit replay
  moving the head, and CU measurement — later slices.

## Slice 24 — validate (`RECONCILING` -> `VALIDATED` / `BLOCKED`)

- The `validate_transition` instruction re-proves the whole chain
  in a rebuilt kernel (chain + priors + select + open_planning +
  every prior plan + reconcile at the state's clock) and only then
  runs kernel `validate_transition` at the state's clock,
  advancing a `RECONCILING` record to `VALIDATED` (stage 8, a new
  program stage value) when the kernel's predicate passes, or to
  `BLOCKED` (stage 6) when it fails — the first program-written
  `BLOCKED` label, which closes the slice-22 honesty bound and
  opens the slice-22 re-plan path for real. The payer is a fee
  payer only: kernel `validate_transition` takes no caller, so no
  proposer, seat, or agent gate applies.
- The slice's design point, and the slice-23 promise kept: the
  stored `compatible` verdict is cross-checked, not trusted —
  slice 23's exact mirror is recomputed over the supplied plan
  set, and a stored verdict the set does not reproduce refuses
  before any kernel validate runs. A still-blocked `BLOCKED`
  record refuses and stands, exactly as the kernel's
  `StillBlocked` arm dictates — enforced from the stored stage,
  because the replay kernel reaches `Reconciling` and never
  carries the record's `Blocked` life into the kernel call (the
  slice-22 pattern: a rule the replay cannot enforce is enforced
  by the helper from the stored record). The replay carries no
  exceptions, so `unresolved_blocking` is always zero here:
  program validate judges the stored chain, not off-program
  exception state, and the exception instructions that would clear
  a block are a later slice's work.
- Host tests 174/174 (161 prior + 13 slice-24: validated advance,
  empty-set block, over-capacity block (2 x 600 against a 1000
  stock), wrong-stage refusal, once-only after validated,
  still-blocked refusal, stored-verdict-mismatch refusal,
  stale-observation block (clock 9100, observation expiry),
  incomplete prior plans, incomplete prior signals,
  unreproducible-plan refusal, unsealed refusal, parent
  mismatch). One test-authoring correction, in the tests only:
  `PlanRecord` is not `Copy`, so repeated prior sets use
  `std::slice::from_ref` (the slice-23 pattern). One real finding,
  fixed in the slice code, not the test: the first version let a
  re-validate of a `BLOCKED` record succeed as a no-op `Blocked`
  advance, because the fresh replay kernel always reaches
  `Reconciling` — the helper now enforces `StillBlocked` from the
  stored stage. Clippy back to the pre-existing profile (anchor
  macro `unexpected cfg` noise + the slice-8 helper's
  `type_complexity` note); zero new warnings.
- `anchor build` green under the pinned toolchain (from-source
  binary by absolute path; the AVM shim on PATH refuses with a
  not-installed error and is not the invocation route); the IDL
  gains the `validate_transition` instruction, the 23rd error
  (`ValidateFailed`), and the 14th event (`TransitionValidated`)
  — 16 instructions, 5 accounts, 23 errors, 14 events; `.so`
  912,672 bytes (sha256 `6fffcfd5…`).
- Deliberately NOT slice 24: commit replay moving the head, and
  CU measurement — later slices.

## Slice 25 — declare (`VALIDATED` -> `DECLARED`)

- The `declare_transition` instruction re-proves the whole chain
  in a rebuilt kernel (chain + priors + select + open_planning +
  every prior plan + reconcile + validate at the state's clock)
  and only then runs kernel `declare_commit` at the state's
  clock, advancing a `VALIDATED` record to `DECLARED` (stage 9,
  a new program stage value; kernel `Committable` is 8) when the
  full 13-bit predicate passes. The payer is a fee payer only:
  kernel `declare_commit` is permissionless — the predicate is
  the gate, not a person.
- The slice's design point: declare is NOT a verdict writer.
  The kernel's `declare_commit` never moves `Validated` ->
  `Blocked` (only the exception path does), so a replay
  validate that yields `Blocked` (a predicate that no longer
  holds at this clock, e.g. a stale observation) or a full
  predicate that fails at declare time refuses with
  `DeclareFailed` and the record stands `VALIDATED` — exactly as
  the kernel leaves its life untouched. The stored `compatible`
  verdict is cross-checked, not trusted, exactly as in slice 24.
  The sealed head does NOT move: declare marks a transition
  committable; commit replay (a later slice) moves the head.
- Host tests 186/186 (174 prior + 12 slice-25: declared advance
  with the head asserted unmoved, once-only after declared,
  wrong-stage refusal, blocked-record refusal, expired-plan
  predicate refusal (clock 400, plan expiry — the `PLANS` mask
  bit validate's structural check does not judge),
  stale-observation refusal (clock 9100, record stands
  `VALIDATED`, not re-labeled `BLOCKED`),
  stored-verdict-mismatch refusal, incomplete prior plans,
  incomplete prior signals, unreproducible-plan refusal,
  unsealed refusal, parent mismatch). Clippy back to the
  pre-existing profile (anchor macro `unexpected cfg` noise +
  the slice-8 helper's `type_complexity` note); zero new
  warnings.
- `anchor build` green under the pinned toolchain (from-source
  binary by absolute path; the AVM shim on PATH refuses with a
  not-installed error and is not the invocation route); the IDL
  gains the `declare_transition` instruction, the 24th error
  (`DeclareFailed`), and the 15th event (`TransitionDeclared`)
  — 17 instructions, 5 accounts, 24 errors, 15 events; `.so`
  958,024 bytes (sha256 `ddc5238b…`).
- Deliberately NOT slice 25: CU measurement — later slices.
  (Commit replay moving the head is slice 26, below.)

## Slice 26 — commit (`DECLARED` -> `COMMITTED`, the head moves)

- The `commit_transition` instruction re-proves the whole chain
  in a rebuilt kernel (chain + priors + select + open_planning +
  every prior plan + reconcile + validate + declare at the
  state's clock) and only then runs kernel `commit` at the
  state's clock, advancing a `DECLARED` record to `COMMITTED`
  (stage 10, a new program stage value; kernel `Committed` is
  9, program `DECLARED` is 9) when the full 13-bit predicate
  passes a third time. On success the kernel's returned
  `SystemState` is written into `KernelState` — head index one
  past the old head, head hash exactly the record's declared
  next hash, both asserted before any write — so the head move
  is real and usable: a new record registers against the new
  head. The payer is a fee payer only: kernel `commit` is
  permissionless — the predicate is the gate, not a person.
- The slice's design points: commit is NOT a verdict writer
  either — a predicate that fails at commit time (a plan expiry
  reached since declare, a stale observation) refuses with
  `CommitFailed` and the record stands `DECLARED` with the head
  unmoved, exactly as the kernel leaves its life `Committable`.
  Governance-typed records refuse at the gate with
  `GovernanceCommitRequired` (their path is `commit_governance`,
  a later slice); the gate is defense in depth today, stated
  plainly — no governance record reaches `DECLARED` through the
  ordinary chain, since kernel `register_transition` refuses
  governance types outright. Honesty bound: the capacity debit
  and dialect effects of `commit_apply` run in the rebuilt
  kernel, which is discarded — the program persists the head
  move, not the post-commit capacity stocks — so until a
  committed-claims ledger lands (a later slice), each commit is
  judged against the sealed launch stocks, not against
  post-commit stocks.
- Host tests 198/198 (186 prior + 12 slice-26: committed advance
  with the head asserted moved to the declared next hash and a
  new record registering against it, once-only after committed,
  wrong-stage refusal, governance-type refusal, expired-plan
  predicate refusal (clock 400, plan expiry — the `PLANS` mask
  bit), stale-observation refusal (clock 9100, record stands
  `DECLARED`, not re-labeled `BLOCKED`),
  stored-verdict-mismatch refusal, incomplete prior plans,
  incomplete prior signals, unreproducible-plan refusal,
  unsealed refusal, parent mismatch). Clippy back to the
  pre-existing profile (anchor macro `unexpected cfg` noise +
  the slice-8 helper's `type_complexity` note); zero new
  warnings — a doc-wrap fix (a `+ register` line start parsed
  as a rustdoc list marker) was needed to keep it there.
- `anchor build` green under the pinned toolchain (from-source
  binary by absolute path); the IDL gains the
  `commit_transition` instruction, the 25th/26th errors
  (`CommitFailed` 6024, `GovernanceCommitRequired` 6025), and
  the 16th event (`TransitionCommitted`) — 18 instructions, 5
  accounts, 26 errors, 16 events; `.so` 1,012,952 bytes
  (sha256 `7ba84086…`).
- Deliberately NOT slice 26: the committed-claims ledger, the
  `commit_governance` path, and CU measurement — later slices.

## Slice 27 — the committed-claims ledger (commit judges remaining capacity)

- `KernelState` gains `committed_claims`: cumulative committed
  claims per launch resource (treasury tranche, lending book,
  exit window, in that order), in kernel `Amount` raw units —
  `KernelState::SPACE` grows 106 -> 130. `commit_transition`
  now checks each commit's per-resource claim sums (the same
  sums the slice-23 mirror recomputes from the prior plans)
  against the sealed stocks minus prior committed claims,
  after the replayed declare succeeds and before kernel
  `commit` runs: a commit that would exceed any stock refuses
  with `CommitFailed`, and a refused commit writes nothing —
  the record stands `DECLARED`, the head stands, and the
  ledger stands at its prior totals. On success the new totals
  are written together with the stage advance and the head
  move, so a later commit is judged against what prior commits
  already consumed. The ledger is totals, not per-commit
  entries: it enforces the bound across commits without
  attributing a past debit to a transition. Declaration is
  still not a reservation — reconcile, validate, and declare
  judge a transition's own plan set against the sealed stocks,
  so a transition can declare and then refuse at commit when
  the ledger is exhausted.
- Host tests 202/202 (198 prior + 4 slice-27: ledger totals
  written on success, refusal when ledger plus new claims
  exceed the tranche stock with ledger untouched, success at
  exact ledger capacity (1000/500), refusal on another gate
  leaving pre-seeded totals untouched). Clippy back to the
  pre-existing profile (anchor macro `unexpected cfg` noise +
  the slice-8 helper's `type_complexity` note); zero new
  warnings.
- `anchor build` green under the pinned toolchain; IDL counts
  unchanged (18 instructions, 5 accounts, 26 errors, 16
  events — the ledger is a field on `KernelState`, not a new
  instruction or account); `.so` 1,016,240 bytes
  (sha256 `f3a57d2f…`).
- Honesty bound, recorded not built: the rebuilt kernel starts
  from the sealed genesis head, so a second transition whose
  parent is the moved head still cannot replay its chain
  through the current helpers (kernel `register_transition`
  refuses a parent that is not the rebuilt kernel's declared
  head) — end-to-end two-commit sequencing needs a
  replay-from-head slice. The ledger gate is therefore tested
  at its enforcement point with pre-seeded totals standing in
  for prior commits; the gate itself is complete and will
  bind successive commits once replay-from-head lands.
- Deliberately NOT slice 27: the `commit_governance` path
  and CU measurement — later slices. Replay-from-head is
  slice 29, below; the current helpers still rebuild from
  genesis until it is threaded through them.

## Slice 28 — the governance commit path (`commit_governance_from_records`)

- The `commit_governance_from_records` helper mirrors the
  slice-26 ordinary commit helper step for step, for a
  governance-typed record: `DECLARED` (9) -> `COMMITTED`
  (10), with the sealed head moving to the kernel's
  returned `SystemState` (index one past the old head, hash
  exactly the declared next hash, both asserted before any
  write). The governance differences: council approvals
  precede registration (each listed seat approves the exact
  action at the stored `eligible_at`; the kernel's unanimity
  gate judges whether every sitting seat did — a seat
  approving a different action is, to that gate, a seat that
  did not approve this one), registration runs through
  kernel `register_governance_transition`, and the commit
  itself runs kernel `commit_governance`, which checks the
  action and applies it atomically with the commit. A
  non-governance record refuses at the gate with the new
  `NotGovernanceTransition` error (the mirror of the
  ordinary helper's `GovernanceCommitRequired`); other
  governance-commit failures refuse with the new
  `GovernanceCommitFailed` error, the record standing
  `DECLARED` with the head unmoved.
- Helper + host tests only, deliberately: `TransitionRecord`
  carries no action, and no program instruction yet
  produces a `DECLARED` governance record (kernel
  `register_transition` refuses governance types), so the
  action and the approvals are helper arguments and the
  tests prove the path with hand-crafted `DECLARED`
  governance records whose labels the helper never trusts
  (it re-proves the whole governance chain from the stored
  records, the action, and the approvals). The
  governance-registration instruction, the approval
  persistence, and the instruction calling this helper are
  later slices. Honesty bound, stated not smoothed: the
  kernel's action application runs in the rebuilt kernel,
  which is discarded — the program persists the stage
  advance and the head move, not the applied rule, so a
  committed `SetCapacity` does not yet change the stocks
  later sealed replays judge against; governance-state
  persistence is the slice that closes that gap. The slice-27
  ledger gate runs over the supplied plan sums exactly as
  in slice 27 (governance plans claim no stock in the
  bridge shape, so it is a no-op there — but a
  claims-carrying governance plan would be ledger-bound
  identically, not passed through unchecked).
- Host tests 212/212 (202 prior + 10 slice-28: unanimous
  commit advancing the head with the ledger at zeros and
  once-only after committed, non-governance type,
  non-unanimous approvals, wrong category, wrong stage,
  expired-plan predicate, stale-observation refusal leaving
  a pre-seeded ledger untouched, success over pre-seeded
  totals leaving the ledger untouched, unsealed, parent
  mismatch). Clippy back to the pre-existing profile
  (anchor macro `unexpected cfg` noise + the slice-8
  helper's `type_complexity` note); zero new warnings.
- `anchor build` green under the pinned toolchain; IDL
  counts move only on errors (18 instructions, 5 accounts,
  28 errors incl. `NotGovernanceTransition` and
  `GovernanceCommitFailed`, 16 events — no new instruction
  or account this slice, as designed); `.so` 1,025,352
  bytes (sha256 `a50da0bd…`).
- Deliberately NOT slice 28: the governance-registration
  instruction, approval persistence, governance-state
  persistence, and CU measurement — later slices.
  Replay-from-head is slice 29, below.

## Slice 29 — replay-from-head (`kernel_at_head_from_commits`)

- A rebuilt sealed launch kernel advanced to the program's
  current head by replaying a supplied chain of prior
  `COMMITTED` bundles (`PriorCommit`: transition record,
  observation record, signal records, plan records), each
  driven through its full ordinary commit chain and checked
  link by link against the running head: a prior whose
  parent is not the running head, whose label is not
  `COMMITTED`, whose plan set does not reproduce its stored
  `compatible` verdict, or whose commit does not land
  exactly one past its parent at its declared next hash
  refuses, and a refusal returns nothing — a partial history
  is never a head. An empty prior set returns the genesis
  kernel and passes only when the program's head is genesis;
  a chain that stops short of, or overshoots, the program's
  head refuses with `ParentMismatch`.
- The clock rule, stated not smoothed: a prior is historical
  reproduction, so its final steps (reconcile, validate,
  declare, commit) run at its stored `planning_deadline` —
  the clock its declare/commit originally proved at — not at
  the current state clock, because replaying an old commit at
  today's clock would re-judge it under conditions that no
  longer hold (a plan whose expiry has passed since) and
  brick every chain older than one plan expiry. A prior
  whose deadline lies ahead of the state clock refuses as
  history from the future. Because the kernel clock never
  regresses, a valid successor chain is time-ordered: each
  prior's window must open after the previous prior's
  deadline, and a target replayed from this head must open
  after the last prior's deadline — a target created before
  the last commit is a sibling, not a successor. Governance
  priors refuse with `GovernanceCommitRequired`: a bundle
  carries no action and no approvals, so skipping one would
  silently drop a head move. Capacity is judged against the
  kernel's current stocks, which already carry earlier
  priors' debits — the second commit in the two-prior test
  is judged against tranche 990 / lending book 495, not the
  sealed 1000 / 500. The slice-27 program ledger is untouched
  by this helper (it reconstructs kernel state, not program
  state); governance-state persistence still awaits its
  slice, as in slice 28.
- Host tests 223/223 (212 prior + 11 slice-29: moved head
  with a second transition registering in it — and refusing
  in a genesis kernel — genesis head from empty priors,
  moved head with no history, non-committed prior, broken
  parent link, unreproducible prior plan, governance prior,
  future-deadline prior, unsealed, two priors debiting both
  stocks to index 2, chain overshooting the program head).
  Clippy back to the pre-existing profile (anchor macro
  `unexpected cfg` noise + the slice-8 test helper's
  `type_complexity` warning); zero new warnings — one
  doc-wrap fix was needed to keep it there (a doc line
  starting with `+` parsed as a rustdoc list marker, the
  slice-26 trap). No new instruction, account, or error this
  slice, as designed: IDL counts unchanged (18 instructions,
  5 accounts, 28 errors, 16 events); `.so` 1,025,352 bytes
  (sha256 `ae325496…`, coincidentally the slice-28 byte size
  with different content).
- Deliberately NOT slice 29: threading this head through
  the thirteen stage helpers (slice 30, below) and the
  commit instructions (the next slice), the
  governance-registration instruction,
  approval persistence, governance-state persistence, and
  CU measurement — later slices.

## Slice 30 — head threading through the stage helpers

- All thirteen stage helpers now take
  `prior_commits: &[PriorCommit]` and build their kernel
  with slice 29's `kernel_at_head_from_commits` instead of
  the sealed genesis kernel: replay, evaluate,
  mark_eligible, submit_signal, select_candidate,
  select_candidate_with_siblings, open_planning,
  submit_plan, reconcile_plans, validate_transition,
  declare_transition, commit_transition, and
  commit_governance. Empty priors are exactly the genesis
  case, so every pre-existing test and every instruction
  (which still passes `&[]`) behaves as before at genesis;
  a moved head with empty priors refuses with
  `ParentMismatch` — a missing chain is never silently
  replayed against genesis stocks. The replay helper moves
  from the `NoEnv` genesis kernel to the `ProgramEnv` head
  kernel: ingest and register never consult the
  environment, so genesis behaviour is identical, and only
  a `ProgramEnv` kernel can drive a prior's evaluate step.
  The successor clock rule is slice 29's, unchanged: a
  target's window must open after the last prior's
  planning deadline, or the kernel's tick refuses the
  regression. The slice-27 ledger gate in the commit
  helpers is unchanged, as a cross-check on the kernel's
  own post-prior stocks.
- Host tests 228/228 (223 prior + 5 slice-30: a second
  commit driven through all thirteen helpers' ordinary
  path to index 2 with the head-kernel stocks at tranche
  980 / lending book 490 and the program ledger at
  20/10, a sibling select at the moved head, a governance
  commit at the moved head with the ledger untouched, a
  moved head with no history refusing `ParentMismatch`,
  and a broken prior link refusing `ParentMismatch`).
  Clippy back to the pre-existing profile (anchor macro
  `unexpected cfg` noise + the slice-8 test helper's
  `type_complexity` warning); zero new warnings. No new
  instruction, account, or error this slice, as designed:
  IDL counts unchanged (18 instructions, 5 accounts, 28
  errors, 16 events); `.so` 1,047,136 bytes
  (sha256 `7c3a7d29…`).
- Deliberately NOT slice 30: threading prior bundles
  through the instruction remaining accounts (the
  instructions still pass `&[]`, so on-chain instructions
  still refuse at a moved head), the
  governance-registration instruction, approval
  persistence, governance-state persistence, and CU
  measurement — later slices.

## Slice 31 — prior bundles through the instruction remaining accounts

- All twelve stage instructions gain a `prior_bundle_count`
  argument, and their remaining accounts begin with exactly
  that many prior COMMITTED bundles in head order, followed
  by the target's own evidence exactly as before: replay,
  evaluate, and mark_eligible refuse any account past the
  bundles; submit_signal, select, open_planning, and the
  plan-family instructions (submit_plan, reconcile,
  validate, declare, commit) parse their own signals/plans
  from the tail; select_with_siblings classifies its sibling
  field from the tail. A bundle is self-delimiting — its
  transition record, then the observation it names, then
  exactly its stored `signal_count` signals, then exactly
  its stored `plan_count` plans — so no new argument carries
  a bundle length and no caller count can shorten a chain.
  The parser (`split_prior_bundles`, over accounts classified
  by discriminator) checks every linkage at parse time and
  refuses with `RecordMismatch` on a mis-linked, truncated,
  or over-counted bundle before any kernel replay runs; the
  stage gate stays in slice 29's driver. On-chain
  instructions can now drive a second commit at a moved
  head; the commit_governance helper still has no
  instruction, so it takes no bundles here.
- Host tests 235/235 (228 prior + 7 slice-31 parser tests:
  one bundle plus tail, zero bundles leaving all as tail,
  wrong observation link, foreign signal, foreign plan,
  truncated and over-counted bundles, wrong first variant).
  Clippy back to the pre-existing profile (anchor macro
  `unexpected cfg` noise + the slice-8 test helper's
  `type_complexity` warning); zero new warnings — the one
  new warning this slice introduced (submit_signal now
  carries 8 arguments) is allowed at the instruction, as
  register_transition already was. No new instruction,
  account, or error this slice, as designed: IDL counts
  unchanged (18 instructions, 5 accounts, 28 errors, 16
  events), with `prior_bundle_count` added to the twelve
  stage instructions' arguments; `.so` 1,052,336 bytes
  (sha256 `e163f93d…`).
- Deliberately NOT slice 31: the
  governance-registration instruction, approval
  persistence, governance-state persistence, and CU
  measurement — later slices.

## Slice 32 — the governance-registration instruction (`register_governance_transition`)

- The governance mirror of slice 8's `register_transition`:
  it creates a `TransitionRecord` at stage `REGISTERED` for a
  governance-typed transition, gated on the sealed head
  (parent exactly the `KernelState` head) plus two governance
  gates: the type must be a governance type
  (`GovernanceTypeRequired` otherwise), and the signing payer
  must be a seated council member of the sealed launch
  profile (`NotCouncilSeat` otherwise) — the program-side
  mirror of kernel `register_governance_transition`'s
  `NotCouncil` gate, judged against the pinned profile, not a
  caller-supplied roster. The ordinary builder refuses
  governance types in this slice
  (`GovernanceRegisterRequired`), so the two registration
  paths are disjoint exactly as the kernel's pair is. A new
  `GovernanceTransitionRegistered` event carries the id, the
  parent index, and the seat proposer.
- Honesty bound, stated not smoothed: this slice stores the
  governance manifest only. `TransitionRecord` still carries
  no action, and no approval is persisted here — the action
  encoding and the approval accounts are the
  approval-persistence slice's work — so a registered
  governance record still cannot advance (ordinary replay
  refuses governance types; the governance replay does not
  exist yet). Slice 31's "its instruction arrives with
  governance-registration" is read here as the governance
  family, not this slice: the `commit_governance` instruction
  still awaits stored action and approvals.
- Host tests 242/242 (235 prior + 7 slice-32: seat acceptance
  at the sealed head, non-gov-type refusal, non-seat-agent
  refusal, unknown-proposer refusal, unsealed refusal, parent
  mismatch, ordinary-builder gov-type refusal). Clippy back
  to the pre-existing profile (anchor macro `unexpected cfg`
  noise + the slice-8 test helper's `type_complexity`
  warning); zero new warnings. IDL counts move by design
  (19 instructions, 5 accounts, 31 errors, 17 events — no new
  account this slice, as designed); `.so` 1,069,456 bytes
  (sha256 `c6b7c24a…`).
- Deliberately NOT slice 32: approval persistence, the
  governance replay and `commit_governance` instruction,
  governance-state persistence, and CU measurement — later
  slices.

## Slice 33 — governance approval persistence (`approve_governance`)

- One `GovApprovalRecord` per (transition, seat), PDA seeds
  `["gov-approval", transition_id, seat]`, storing the seat's
  approval as a bounded `StoredGovAction` — a fixed-size,
  lossless encoding of every kernel `GovAction` variant (kinds
  0–14 in kernel declaration order; full `Policy` for
  `SetPolicy`; fixed arrays + counts for the set, group,
  capability-pair, and council-seat lists), so the later
  governance replay rebuilds the exact approved action from
  chain state instead of trusting a caller-supplied argument.
  Gates mirror the kernel plus the registration
  preconditions: sealed state, governance-typed `REGISTERED`
  record, seated council signer in the pinned profile
  (`NotCouncilSeat` otherwise), and action category equal to
  the record's type (`GovActionCategoryMismatch` otherwise —
  the kernel's `WrongGovCategory` gate, moved to approval
  time). Caps, decided here for Carlos's audit: id sets 16
  (kernel `MAX_GROUPS` pattern), capability pairs 8
  (`MAX_PLAN_CLAIMS` precedent), council seats 15 (kernel
  `MAX_SEATS`); over-cap refuses with `GovActionTooLarge`,
  never truncates. A new `GovernanceApproved` event carries
  the id, the seat, and the action kind.
- Honesty bounds, stated not smoothed: approval records are
  `init`-only, so a seat's first approval stands — the kernel
  permits re-approval (map insert overwrites) before
  registration, and the program does not yet mirror that; a
  seat that approved the wrong action cannot re-approve this
  transition id, and the proposal then fails unanimity
  (fail-closed; a fresh id is the remedy). A revise-approval
  instruction is a later slice's decision. The slice-28
  `commit_governance_from_records` helper still takes its
  action and approvals as arguments: consuming stored
  approvals at commit time rides the governance-replay slice.
- Host tests 253/253 (242 prior + 11 slice-33: every-variant
  round-trip with distinct kinds, seat acceptance with
  matching category, non-gov-record refusal, non-seat-agent
  refusal, unknown-approver refusal, unsealed refusal,
  wrong-stage refusal, category-mismatch refusal,
  over-cap encode refusals, corrupt-decode refusals, space
  accounting). Clippy back to the pre-existing profile
  (anchor macro `unexpected cfg` noise + the slice-8 test
  helper's `type_complexity` warning); zero new warnings. IDL
  counts move by design (20 instructions, 6 accounts, 34
  errors, 18 events — +1 instruction, +1 account, +3 errors,
  +1 event); `.so` 1,143,992 bytes (sha256 `bb790cc8…`).
- Deliberately NOT slice 33: the governance replay and
  `commit_governance` instruction, governance-state
  persistence, a revise-approval instruction, and CU
  measurement — later slices.

## Slice 34 — governance-state persistence, helper level (`kernel_at_head_from_entries`)

- A committed governance bundle is now replayable as a prior:
  `GovPriorCommit` carries the transition, observation, signal,
  and plan records plus the stored council approval records, and
  `kernel_at_head_from_entries` replays a mixed chain of
  ordinary and governance priors in head order, each through its
  own commit path, under the slice-29 discipline (per-entry
  parent linkage, refusal returns nothing, empty chain passes
  only at genesis, short/overshooting chains refuse with
  `ParentMismatch`). The action is decoded from the stored
  approvals alone — every approval must name the transition,
  decode, and agree on one action, no seat twice — and the
  kernel's own unanimity gate at
  `register_governance_transition` is the final judge. The
  slice-28 honesty bound closes where replays are built: a
  committed `SetCapacity` (exit-window 250 -> 400 in the test
  bundle) returns a rebuilt kernel whose stock IS 400, so later
  entries are judged against the committed rule, not genesis.
  The prior driver judges proposer and observer gates against
  the RUNNING kernel, not the genesis-pinned profile, so a
  roster-changing prior is visible to later entries.
- Host tests 259/259 (253 prior + 6 slice-34: applied
  capacity on the rebuilt kernel with untouched sibling
  stocks and an unwritten prior record, mixed-action refusal,
  missing- and duplicate-seat refusals, foreign-approval
  linkage and corrupt-action decode refusals, wrong-stage and
  unsealed refusals, mixed-chain head discipline). Clippy back
  to the pre-existing profile (anchor macro `unexpected cfg`
  noise + the slice-8 test helper's `type_complexity`
  warning); zero new warnings. No new instruction, account, or
  error this slice, as designed: IDL counts unchanged (20
  instructions, 6 accounts, 34 errors, 18 events); `.so`
  1,143,960 bytes (helper code only, not yet reachable from
  any instruction — the size barely moves until threading).
- Deliberately NOT slice 34: threading the mixed chain
  through the thirteen stage helpers and the instruction
  remaining accounts (helpers and instructions still pass
  ordinary-only priors, so on-chain instructions still refuse
  past a governance commit), the governance replay and
  `commit_governance` instruction, a revise-approval
  instruction, and CU measurement — later slices.

## Slice 35 — threading the mixed prior chain through helpers and instructions

- The thirteen stage helpers now take `prior_entries:
  &[PriorEntry]` and build their kernel with
  `kernel_at_head_from_entries` (empty entries = the genesis
  case; an incomplete chain at a moved head still refuses with
  `ParentMismatch`), and the twelve stage instructions parse
  their remaining accounts through `split_prior_bundles` +
  `prior_entries_of` — so on-chain instructions now replay past
  a governance commit instead of refusing at it. The delimiting
  rule extends the slice-16/21 anchor pattern:
  `TransitionRecord` gains `approval_count` (SPACE 263 ->
  271), written by `approve_governance` on success only via
  the pure `note_governance_approval` helper, so a governance
  bundle is self-delimiting by its stored signal, plan, and
  approval counts; a bundle short of its approval count, or
  carrying a foreign approval, refuses at parse time with
  `RecordMismatch`. The classifier gains a boxed fifth variant,
  `GovApproval` (boxing keeps clippy's large-variant lint
  quiet: the approval record is ~1600 bytes against ~264 for
  the next-largest variant); the sibling instruction's tail
  match refuses the new variant, as it already refuses `Plan`.
  Design decisions for Carlos's audit: a count field, not a
  scan-until-next-transition rule (the anchor catches an
  omitted approval at parse time, where the other anchors
  live, rather than leaving it to the kernel's unanimity gate),
  and the count on the transition record itself, preserving
  the one-instruction-one-anchor invariant.
- Host tests 263/263 (259 prior + 4 slice-35: the approval-count
  anchor increment and overflow refusal, mixed ordinary +
  governance bundle parsing with entry-kind split, short and
  foreign approval parse refusals, and the end-to-end mixed
  chain — ordinary commit to head 1, governance commit to head
  2, target replayed from the mixed head, and the same target
  with the governance prior omitted refusing with
  `ParentMismatch`). Clippy back to the pre-existing profile
  (anchor macro `unexpected cfg` noise + the slice-8 test
  helper's `type_complexity` warning); zero new warnings — the
  large-variant warning the unboxed variant raised was fixed
  by boxing, not suppressed. No new instruction, account, or
  error this slice, as designed: IDL counts unchanged (20
  instructions, 6 accounts, 34 errors, 18 events); `.so`
  1,211,832 bytes (sha256 `7eb6114b…`, re-measured this run — the
  interrupted slice-35 draft's recorded `c7db9a1a…` did not reproduce
  from the current tree; a forced rebuild reproduces `7eb6114b…`
  exactly, size and IDL counts unchanged).
- Deliberately NOT slice 35: the governance replay and
  `commit_governance` instruction, a revise-approval
  instruction, and CU measurement — later slices.

## Slice 36 — the governance replay at commit time + the `commit_governance` instruction

- The slice-28 whole-chain governance re-proof, now driven
  from stored records alone: `commit_governance_from_stored`
  re-checks the approval count against the record, decodes
  the action and seats through the shared
  `gov_action_and_seats` (which the slice-34 prior driver
  now also uses — one decode rule for history and target),
  and calls the slice-28 helper. The decoder's gates:
  every approval names the transition (`RecordMismatch`),
  every stored action decodes (`GovActionDecode` for a
  corrupt record), every approval agrees on one action and
  no seat approves twice (`GovernanceCommitFailed`), empty
  set refuses; unanimity stays the kernel's gate at
  `register_governance_transition`, not a program count.
- The `commit_governance` instruction mirrors
  `commit_transition`: prior bundles first, then the tail
  split by the stored counts through the pure
  `split_gov_commit_tail` (`signal_count` signals,
  `plan_count` plans, `approval_count` approvals), refusing
  with `RecordMismatch` before any replay on a short, past,
  or mis-typed run. It takes NO action argument — a
  caller-supplied action could re-target a unanimous
  approval set at an action the council never approved.
  On success the record advances `DECLARED` -> `COMMITTED`,
  the sealed head moves exactly as in slice 26, and the
  existing `TransitionCommitted` event fires.
- Host tests 268/268 (263 prior + 5 slice-36: the stored
  commit success with head move and second-commit refusal,
  the count-mismatch anchor refusal, the foreign /
  mixed-action / duplicate-seat decoder refusals, the
  corrupt-decode and non-unanimous refusals, and the tail
  split success plus four parse refusals). Clippy back to
  the pre-existing profile (anchor macro `unexpected cfg`
  noise + the slice-8 test helper's `type_complexity`
  warning); zero new warnings. IDL counts move by design
  (21 instructions, 6 accounts, 34 errors, 18 events — +1
  instruction only, the existing event reused); `.so`
  1,255,808 bytes (sha256 `096055ec…`).
- Deliberately NOT slice 36: the governance stage chain
  (replay_governance through declare_governance — a
  governance record still cannot reach `DECLARED` through
  instructions, so this instruction completes the commit
  half, not the chain), a revise-approval instruction (the
  init-only divergence stands, fail-closed),
  governance-state persistence beyond the slice-34 head
  replay, and CU measurement — later slices.

## Slice 37 — the governance stage chain, first three stages

- `replay_governance`, `evaluate_governance`, and
  `mark_eligible_governance`: each mirrors its ordinary
  slice (10 / 12 / 13) with one substitution at the head of
  the prefix — the shared slice-36 decoder derives the
  action and seats from the supplied stored approvals, then
  every stored seat's `approve_governance` runs and kernel
  `register_governance_transition` replaces
  `register_transition` at the state's clock. The staged
  replay's approval time is a proof, not history: the
  slice-28 commit re-proof re-runs approvals at the stored
  `eligible_at`, so replay at the current clock proves
  registrability and the commit proves the stored chain.
  Remaining accounts parse as in slice 36's commit, minus
  the signal/plan tail: prior bundles first, then the tail
  through the pure `split_gov_stage_tail` — every account a
  `GovApprovalRecord` numbering exactly `approval_count`,
  else `RecordMismatch` before any replay. The
  mark-eligible helper advances the slice-19 sibling census
  exactly as the ordinary one does, so a governance
  transition that becomes eligible counts as a competitor at
  its parent. No new account or error; the three
  instructions reuse the existing stage events.
- Host tests 273/273 (268 prior + 5 slice-37: the replay
  success with once-only refusal, the non-unanimous and
  ordinary-type replay refusals, the evaluate chain with
  once-only and wrong-stage refusals, the eligible chain
  with the persisted window (`eligible_at` 100, deadline
  200) and census advance to 1 plus once-only refusal, and
  the stage-tail split success plus short / past / mis-typed
  refusals). Clippy back to the pre-existing profile
  (anchor macro `unexpected cfg` noise + the slice-8 test
  helper's `type_complexity` warning); zero new warnings.
  IDL counts move by design (24 instructions, 6 accounts,
  34 errors, 18 events — +3 instructions only); `.so`
  1,287,336 bytes (sha256 `06043724…`).
- Deliberately NOT slice 37: the governance signal, select,
  planning, reconcile, validate, and declare stages — a
  governance record still cannot reach `DECLARED` through
  instructions alone; a revise-approval instruction; and CU
  measurement — later slices.

## Slice 38 — the governance signal instruction

- `submit_signal_governance` + `submit_signal_governance_from_records`:
  the slice-15 signal on the governance path. The helper mirrors
  the ordinary signal with one substitution in the prefix: the
  slice-37 governance prefix (every stored seat's
  `approve_governance`, ingest, governance registration,
  evaluate, mark_eligible) runs at the stored `eligible_at`, so
  the rebuilt kernel's window IS the stored window, then every
  supplied prior signal replays at its own `submitted_at`,
  then the new signal at the state's clock. The kernel's own
  signal gates refuse duplicates, away-from-mean revisions,
  deadline signals, unknown agents, and over-range values.
  Signals do not advance the stage and do not touch the window;
  only `signal_count` increments, the anchor the governance
  select slice will check its prior set against. The staged
  prefix's approval time is a proof, not history, as in slice
  37: the slice-28 commit re-proof re-runs approvals at the
  stored `eligible_at`.
- The instruction's remaining accounts parse as in slice 31
  (prior COMMITTED bundles first), then the tail splits
  through the new pure `split_gov_signal_tail` — first
  exactly `approval_count` `GovApprovalRecord`s, then this
  transition's prior `SignalRecord`s — refusing a short
  approval run, a signal inside the approval run, or an
  approval inside the signal run with `RecordMismatch`
  before any replay. The payer is fee payer and signal agent
  in one, as in slice 15; the emitted event is the existing
  `SignalSubmitted`. No new account or error.
- Host tests 277/277 (273 prior + 4 slice-38: the first-signal
  success with the kernel tally (weight 2.0, confidence 1.0),
  stage and window untouched, count 1; the prior replay with
  a same-round duplicate refused and the count left at 1; the
  wrong-stage / ordinary-type / non-unanimous refusals; and
  the tail-split success plus short / mixed-run refusals).
  Clippy back to the pre-existing profile (anchor macro
  `unexpected cfg` noise + the slice-8 test helper's
  `type_complexity` warning); zero warnings naming any
  slice-38 symbol. IDL counts move by design (25
  instructions, 6 accounts, 34 errors, 18 events — +1
  instruction only, including `submit_signal_governance`);
  `.so` 1,332,168 bytes (sha256 `d470713d…`).
- Deliberately NOT slice 38: the governance select,
  planning, reconcile, validate, and declare stages — a
  governance record still cannot reach `DECLARED` through
  instructions alone; a revise-approval instruction; and CU
  measurement — later slices.

## Slice 39 — the governance select instruction

- `select_candidate_governance` +
  `select_candidate_governance_from_records`: the slice-16
  select on the governance path. The helper mirrors the
  ordinary select with one substitution in the prefix: the
  slice-37 governance prefix (every stored seat's
  `approve_governance`, ingest, governance registration,
  evaluate, mark_eligible) runs at the stored `eligible_at`,
  so the rebuilt kernel's window IS the stored window, then
  every supplied prior signal replays at its own
  `submitted_at`, then kernel `select_candidate` at the
  state's clock. Only a `Life::Selected` life advances the
  stage, to `SELECTED` (4). Gates, in order: sealed state;
  stage exactly `ELIGIBLE`; parent exactly the sealed head;
  observation linkage; a governance type
  (`NotGovernanceTransition` otherwise); the
  approval-count anchor; a proven window; the completeness
  anchor — the supplied prior set must number exactly
  `signal_count`, every prior belonging to this transition
  with a stored round matching its own `submitted_at`, no
  duplicated (agent, round); and the slice-19 census gate —
  the `KernelState` sibling census at the target's parent
  must read exactly 1, so a competitor cannot be bypassed
  through this plain path. A kernel refusal in any call
  maps to `SelectFailed` (still-open window, unmet quorum,
  confidence below threshold, expired manifest,
  non-unanimous approval set, deadline signal in the prior
  replay). The instruction's remaining accounts parse as
  in slice 38 (prior COMMITTED bundles first, then the
  tail through the reused `split_gov_signal_tail`); the
  payer is a fee payer only, as in slice 16; the emitted
  event is the existing `TransitionSelected`. No new
  account or error.
- Host tests 283/283 (277 prior + 6 slice-39: the
  deadline select success (stage 4, count unchanged); the
  before-deadline refusal; the wrong-stage /
  ordinary-type / non-unanimous refusals; the incomplete
  and duplicated prior-set refusals; the unqualified-tally
  and competitor-census refusals; and once-only).
  Clippy back to the pre-existing profile (anchor macro
  `unexpected cfg` noise + the slice-8 test helper's
  `type_complexity` warning); zero warnings naming any
  slice-39 symbol and none pointing at the new accounts
  struct. IDL counts move by design (26 instructions, 6
  accounts, 34 errors, 18 events — +1 instruction only,
  including `select_candidate_governance`); `.so`
  1,358,392 bytes (sha256 `9309c84d…`). The build output
  also carries two stack-frame messages naming
  `approve_governance` only — that instruction is untouched
  by this slice, and `anchor build` exits 0, so they are
  recorded here as pre-existing build output, not a
  slice-39 regression.
- Deliberately NOT slice 39: the sibling-aware
  governance select, the governance planning, reconcile,
  validate, and declare stages — a governance record
  still cannot reach `DECLARED` through instructions
  alone; a revise-approval instruction; and CU
  measurement — later slices.

## Slice 40 — the sibling-aware governance select instruction

- `select_candidate_governance_with_siblings` +
  `select_candidate_governance_from_records_with_siblings`
  + `GovSiblingReplay` + `replay_governance_transition_chain`:
  the slice-17/19 select on the governance path. The
  helper mirrors the ordinary with-siblings select with a
  governance prefix per transition: every stored seat's
  `approve_governance`, ingest, governance registration,
  evaluate, mark_eligible, all at the stored `eligible_at`,
  then every supplied signal at its own `submitted_at` in
  one merged timeline (chains before signals at an equal
  time, siblings before the target on an equal
  `eligible_at` — the fail-closed tie rule), then kernel
  `select_candidate` on the target at the state's clock,
  so the kernel's `SiblingsStillOpen` /
  `OutrankedBySibling` arms can now fire on the governance
  path. Only a `Life::Selected` life advances the stage,
  to `SELECTED` (4). Gates, in order: the slice-39 target
  gates (sealed state; stage exactly `ELIGIBLE`; parent
  exactly the sealed head; observation linkage; a
  governance type; the approval-count anchor; a proven
  window; the completeness anchor — the supplied prior set
  numbering exactly `signal_count`, every prior belonging
  to the target with a stored round matching its own
  `submitted_at`, no duplicated (agent, round)); then per
  sibling: an id distinct from the target and every other
  sibling; stage exactly `ELIGIBLE`; parent exactly the
  target's parent; observation linkage; a governance type;
  its approval set numbering exactly its own
  `approval_count`; a proven window; its signal set
  numbering exactly its own `signal_count`, with the same
  ownership/round/duplicate rules; then the slice-19
  census gate — supplied siblings + target equal to the
  `KernelState` sibling census at the target's parent.
  The instruction's tail groups by transition id, as in
  the ordinary with-siblings instruction: sibling
  transitions and observations, approvals (the target's
  and each sibling's, told apart by `transition_id`),
  and signals likewise; a plan, an approval or signal
  naming no supplied transition, a sibling without
  exactly one matching observation, or an unmatched
  observation refuses with `RecordMismatch` before any
  replay. No new account or error; the payer is a fee
  payer only; the emitted event is the existing
  `TransitionSelected`.
- Host tests 294/294 (283 prior + 11 slice-40: the
  target-wins-over-weaker-sibling success; the outranked
  and open-sibling refusals; the earlier-target tie win
  and the equal-eligibility tie refusal; the omitted
  sibling (census 3, one supplied) refusal; the
  ordinary-typed sibling refusal; the selected-stage
  sibling gate refusal; the incomplete sibling-signal
  refusal; the duplicate-sibling refusal; and the empty
  field at census 1 selecting like the plain path).
  Clippy back to the pre-existing profile (anchor macro
  `unexpected cfg` noise + the slice-8 test helper's
  `type_complexity` warning); zero warnings naming any
  slice-40 symbol and none pointing at the new accounts
  struct. IDL counts move by design (27 instructions, 6
  accounts, 34 errors, 18 events — +1 instruction only,
  including `select_candidate_governance_with_siblings`);
  `.so` 1,399,616 bytes (sha256 `6ff4de8b…`).
- Deliberately NOT slice 40: the already-selected
  governance sibling re-proof (a `SELECTED` sibling
  refuses at this slice's gate — the correct outcome by
  the gate, not yet by the kernel's selected arm); the
  mixed-field select (an ordinary competitor at the same
  parent is fail-closed by the census gate, not judged —
  the kernel's sibling loop is type-blind, but neither
  single-type replay path can replay the other type);
  the governance planning, reconcile, validate, and
  declare stages — a governance record still cannot reach
  `DECLARED` through instructions alone; a
  revise-approval instruction; and CU measurement —
  later slices.

## Slice 41 — the already-selected governance sibling re-proof

- `select_candidate_governance_from_records_with_siblings`
  (helper only — no new instruction, account, or error): the
  slice-18 re-proof on the governance path. The sibling gate
  now admits stage `SELECTED` (4) as well as `ELIGIBLE` (3);
  every other non-`ELIGIBLE` stage still refuses, and at most
  one `SELECTED` sibling is admitted — a parent can have only
  one selected transition, so two supplied `SELECTED` siblings
  refuse at the gate, before any replay. After the merged
  timeline, kernel `select_candidate` runs on the supplied
  `SELECTED` sibling at the state's clock and its life must
  become `Selected`: the stage label is re-proven, never
  trusted. Only then does kernel `select_candidate` run on
  the target, where it meets the kernel's
  `ParentAlreadyHasASelectedTransition` arm. A `SELECTED`
  sibling whose own select cannot be proven — unqualified
  tally, an outranking target, a window not yet closed —
  refuses the whole helper with `SelectFailed`, and the
  target stays `ELIGIBLE`: a stored stage that history
  cannot reproduce is an inconsistent record, not a block.
  Only a `Life::Selected` life advances the target's stage,
  to `SELECTED` (4); sibling records are read, never written.
  IDL counts unchanged by design (27 instructions, 6
  accounts, 34 errors, 18 events); `.so` 1,401,472 bytes
  (sha256 `e778fa6f…`).
- Host tests 300/300 (294 prior − 1 replaced + 7 slice-41:
  the slice-40 selected-stage gate-refusal test is replaced
  by the blocks-target test, its gate premise amended as
  slice 18 amended slice 17's — a proven selected sibling
  now blocks through the kernel's arm, not the gate; plus
  the unqualified, outranked, and open-window re-proof
  refusals; the two-selected-siblings gate refusal; the
  selected-plus-eligible mixed field blocking the target;
  and the incomplete selected-sibling signal-set anchor
  refusal). Clippy back to the pre-existing profile (anchor
  macro `unexpected cfg` noise + the slice-8 test helper's
  `type_complexity` warning, line shifted by this slice's
  inserts); zero warnings naming any slice-41 symbol.
- Deliberately NOT slice 41: the mixed-field select (an
  ordinary competitor at the same parent is still
  fail-closed by the census gate, not judged — neither
  single-type replay path can replay the other type); the
  governance planning, reconcile, validate, and declare
  stages — a governance record still cannot reach
  `DECLARED` through instructions alone; a revise-approval
  instruction; and CU measurement — later slices.

## Slice 42 — the mixed-field select instruction

- `select_candidate_mixed_with_siblings` +
  `select_candidate_mixed_from_records_with_siblings` +
  `MixedSiblingReplay`: the slice-17/19/40/41 select with
  the type dispatch the single-type paths lack. The kernel's
  sibling loop is type-blind, but ordinary registration
  refuses a governance type and the governance prefix
  refuses an ordinary type, so a mixed field refused at a
  type gate or at the census gate, never judged. This slice
  dispatches each member's eligibility chain on its own
  type (governance prefix for governance members, ordinary
  registration for ordinary members) into one rebuilt
  kernel, so the kernel's `SiblingsStillOpen` /
  `OutrankedBySibling` / `ParentAlreadyHasASelectedTransition`
  arms judge the whole field at once, across types,
  including the sibling-first tie rule on an equal
  `eligible_at`. The approval anchor dispatches too: a
  governance member carries exactly its own
  `approval_count` approvals, an ordinary member carries
  none (a non-empty set on an ordinary member refuses
  with `RecordMismatch`). The slice-41 `SELECTED`
  re-proof runs across types, and the slice-19 census gate
  still applies — an omitted competitor of either type
  refuses before any replay. The single-type instructions
  keep their contracts (their helpers still refuse
  cross-type members); the mixed instruction is the path
  for fields containing both types, and no mixed-only gate
  is imposed — a census-complete single-type field judges
  correctly here too, but callers that know their field
  is single-type use the specific instruction. No new
  account, error, or event; the emitted event is the
  existing `TransitionSelected`.
- Host tests 310/310 (300 prior + 10 slice-42: the
  governance target winning over a weaker ordinary
  sibling and the reverse; the outranked refusal in both
  directions; the cross-type `SELECTED` sibling blocking
  the target in both directions; approvals on an
  ordinary sibling refusing with `RecordMismatch`; a
  governance sibling without approvals refusing the same
  way; an open ordinary sibling window refusing a
  governance target; and the equal-eligibility tie
  refusing across types). Clippy back to the
  pre-existing profile (anchor macro `unexpected cfg`
  noise + the slice-8 test helper's `type_complexity`
  warning, line shifted by this slice's inserts); zero
  warnings naming any slice-42 symbol and none pointing
  at the new accounts struct. IDL counts move by design
  (28 instructions, 6 accounts, 34 errors, 18 events — +1
  instruction only, including
  `select_candidate_mixed_with_siblings`); `.so`
  1,436,464 bytes (sha256 `69bb01ba…`).
- Deliberately NOT slice 42: the governance planning,
  reconcile, validate, and declare stages — a governance
  record still cannot reach `DECLARED` through
  instructions alone; a revise-approval instruction; and
  CU measurement — later slices.

## Slice 43 — the governance open-planning instruction

- `open_planning_governance` +
  `open_planning_governance_from_records` +
  `OpenPlanningGovernance`: the slice-20/22 open planning on
  the governance path. Gates, in order: sealed state; stage
  exactly `SELECTED` or `BLOCKED` (the re-plan path: a
  `BLOCKED` governance record re-plans only for its proposer
  or a seat of the sealed launch council, and fewer than
  `MAX_REPLANS` re-plans taken); parent exactly the sealed
  head; observation linkage; a governance type
  (`NotGovernanceTransition` otherwise — an ordinary record
  opens planning through the ordinary helper, never here);
  the approval-count anchor; a proven window; the
  completeness anchor (supplied priors numbering exactly
  `signal_count`, every prior belonging to the transition
  with a stored round matching its own `submitted_at`, no
  duplicated (agent, round)). Replay runs the slice-37
  governance prefix (approvals + ingest + governance
  registration) at the stored `eligible_at`, then evaluate
  and mark_eligible at the same clock, then each prior
  signal at its `submitted_at` in order, then select and
  open_planning at the state's clock; only a
  `Life::Planning` life advances the stage, and the stored
  deadline is `clock + planning_window` — a re-plan resets
  it, exactly as the ordinary path does. The instruction's
  remaining accounts parse as in slice 39: prior COMMITTED
  bundles first, then the governance tail (approvals, then
  prior signals) split by `split_gov_signal_tail`. No new
  account, error, or event; the emitted event is the
  existing `TransitionPlanningOpened`.
- Governance difference found by a failing test, not by
  design review: the kernel's
  `register_governance_transition` refuses a non-seat
  caller with `NotCouncil`, so a governance record's
  proposer is a seat by construction — the ordinary
  seat-ground fixture (proposer moved to a non-seat agent)
  cannot exist on this path and refuses in replay. The
  seat-ground test therefore keeps the proposer a seat and
  varies the caller instead (seat 2, not the proposer),
  proving the seat arm distinctly from the proposer arm.
  The helper is unchanged by this: the re-plan gate is the
  same proposer-or-seat rule as the ordinary helper.
- The `BLOCKED` fixture carries the slice-22 honesty note:
  no slice can yet produce a Blocked governance record — a
  future governance validate slice will — so the fixture
  sets the label and the tests prove the re-plan authority
  gate and the cap, not the label itself.
- Host tests 324/324 (310 prior + 14 slice-43: the Selected
  open with the anyone caller and deadline 300; the
  wrong-stage, once-only, incomplete-prior-set, clock-moved
  -back re-proof, later-clock deadline, ordinary-type,
  non-unanimous-approval, unsealed, and parent-mismatch
  refusals; the proposer re-plan, the non-proposer-seat
  re-plan, the stranger refusal, the at-cap refusal, and
  the Selected open leaving `replans` untouched). All 14
  passed except the seat-ground test's first form, which
  failed on the `NotCouncil` rule above and was corrected
  to the faithful form rather than weakened. Clippy back
  to the pre-existing profile (anchor macro `unexpected
  cfg` noise + the slice-8 test helper's `type_complexity`
  warning, line shifted by this slice's inserts); zero
  warnings naming any slice-43 symbol and none pointing at
  the new accounts struct. IDL counts move by design
  (29 instructions, 6 accounts, 34 errors, 18 events — +1
  instruction only, including
  `open_planning_governance`); `.so` 1,466,024 bytes
  (sha256 `55b7055b…`).
- Deliberately NOT slice 43: the governance submit-plan,
  reconcile, validate, and declare stages — a governance
  record still cannot reach `DECLARED` through
  instructions alone; a revise-approval instruction; and
  CU measurement — later slices.

## Slice 44 — the governance submit-plan instruction

- `submit_plan_governance` +
  `submit_plan_governance_from_records` +
  `SubmitPlanGovernance` + `split_gov_plan_tail`: the
  slice-21 submit plan on the governance path. Gates, in
  order: sealed state; stage exactly `PLANNING`; parent
  exactly the sealed head; observation linkage; a
  governance type (`NotGovernanceTransition` otherwise —
  an ordinary record submits a plan through the ordinary
  helper, never here); the approval-count anchor; a proven
  planning window (a nonzero stored deadline, the state's
  clock strictly inside it); the new plan's expiry strictly
  after the stored deadline; the prior-signal anchor
  (supplied signals numbering exactly `signal_count`,
  every prior belonging to the transition with a stored
  round matching its own `submitted_at`, no duplicated
  (agent, round)); the prior-plan anchor (supplied plans
  numbering exactly `plan_count`, every prior belonging
  to the transition, no duplicated resource inside a
  prior's claims); the new claim list bounded (8) and
  duplicate-free. Replay runs the slice-37 governance
  prefix (approvals + ingest + governance registration)
  at the stored `eligible_at`, then evaluate and
  mark_eligible at the same clock, then each prior signal
  at its `submitted_at` in order, then select and
  open_planning at the derived open clock (so the
  replayed deadline equals the stored deadline, by
  construction), then each prior plan and the new plan at
  the state's clock. Only a `Life::Planning` life at each
  stage advances anything, and the stage stays `PLANNING`
  throughout — `plan_count` increments on success only.
  The instruction's remaining accounts parse as in slice
  31's submit plan, with the governance tail first:
  prior COMMITTED bundles first, then approvals, then
  prior signals, then prior plans, split by the new
  `split_gov_plan_tail`. No new account, error, or event;
  the emitted event is the existing `PlanSubmitted`.
- No governance difference surfaced by a failing test
  this slice: the first plan from a seeded agent, the
  second agent replaying the prior plan, and all eleven
  refusals (wrong stage, at-deadline, expiry-at-deadline,
  unknown agent, duplicate agent, duplicate resource,
  incomplete prior plans, incomplete prior signals,
  unsealed, ordinary type, non-unanimous approvals)
  passed in their first faithful form — the kernel's
  agent gate and unanimity gate behave on the governance
  path exactly as on the ordinary one.
- Host tests 337/337 (324 prior + 13 slice-44). Clippy
  back to the pre-existing profile (anchor macro
  `unexpected cfg` noise + the slice-8 test helper's
  `type_complexity` warning at lib.rs:11055, the
  `record_args` helper — line shifted by this slice's
  inserts); zero warnings naming any slice-44 symbol and
  none pointing at the new accounts struct. IDL counts
  move by design (30 instructions, 6 accounts, 34 errors,
  18 events — +1 instruction only, including
  `submit_plan_governance`); `.so` 1,517,376 bytes
  (sha256 `8450b572…`).
- Deliberately NOT slice 44: the governance reconcile,
  validate, and declare stages — a governance record
  still cannot reach `DECLARED` through instructions
  alone; a revise-approval instruction; and CU
  measurement — later slices.

## Slice 45 — the governance reconcile instruction

- `reconcile_plans_governance` +
  `reconcile_plans_governance_from_records` +
  `ReconcilePlansGovernance`: the slice-23 reconcile on
  the governance path. Gates, in order: sealed state;
  stage exactly `PLANNING`; parent exactly the sealed
  head; observation linkage; a governance type
  (`NotGovernanceTransition` otherwise — an ordinary
  record reconciles through the ordinary helper, never
  here); the approval-count anchor; a proven planning
  window (a nonzero stored deadline, with a derivable
  open clock and the state's clock at or after the
  deadline — before the deadline the window is still open
  and the kernel refuses, so no legitimate reconcile is
  lost); the prior-signal anchor (supplied signals
  numbering exactly `signal_count`, every prior belonging
  to the transition with a stored round matching its own
  `submitted_at`, no duplicated (agent, round)); the
  prior-plan anchor (supplied plans numbering exactly
  `plan_count`, every prior belonging to the transition,
  no duplicated resource inside a prior's claims). Replay
  runs the slice-37 governance prefix (approvals + ingest
  + governance registration) at the stored `eligible_at`,
  then evaluate and mark_eligible at the same clock, then
  each prior signal at its `submitted_at` in order, then
  select and open_planning at the derived open clock (so
  the replayed deadline equals the stored deadline, by
  construction), then each prior plan at the open clock,
  then reconcile at the state's clock. The stage advances
  to `RECONCILING` and the kernel's compatibility verdict
  is persisted, exactly as on the ordinary path — an
  empty or over-capacity set advances with
  `compatible: false`, not a refusal. The instruction's
  remaining accounts parse as in slice 31's reconcile,
  with the governance tail: prior COMMITTED bundles
  first, then approvals, then prior signals, then prior
  plans, split by `split_gov_plan_tail`. The payer is a
  fee payer only: kernel `reconcile_plans` takes no
  caller, so no proposer, seat, or agent gate applies
  here. No new account, error, or event; the emitted
  event is the existing `TransitionReconciled`.
- No governance difference surfaced by a failing test
  this slice: the compatible advance, the empty-set and
  over-capacity incompatibilities, and all seven refusals
  (before-deadline, wrong stage, once-only, incomplete
  prior plans, ordinary type, non-unanimous approvals,
  unsealed / parent mismatch) passed in their first
  faithful form — the kernel's capacity verdict and
  unanimity gate behave on the governance path exactly as
  on the ordinary one.
- Host tests 347/347 (337 prior + 10 slice-45). Clippy
  back to the pre-existing profile (anchor macro
  `unexpected cfg` noise + the slice-8 test helper's
  `type_complexity` warning at lib.rs:11400, the
  `record_args` helper — line shifted by this slice's
  inserts); zero warnings naming any slice-45 symbol and
  none pointing at the new accounts struct. IDL counts
  move by design (31 instructions, 6 accounts, 34 errors,
  18 events — +1 instruction only, including
  `reconcile_plans_governance`); `.so` 1,550,168 bytes
  (sha256 `8be06f9d…`).
- Deliberately NOT slice 45: the governance validate and
  declare stages — a governance record still cannot reach
  `DECLARED` through instructions alone; a revise-approval
  instruction; and CU measurement — later slices.

## Slice 46 — the governance validate instruction

- `validate_transition_governance` +
  `validate_transition_governance_from_records` +
  `ValidateTransitionGovernance`: the slice-24 validate on
  the governance path. Gates, in order: sealed state;
  stage exactly `RECONCILING` or `BLOCKED`; parent exactly
  the sealed head; observation linkage; a governance type
  (`NotGovernanceTransition` otherwise — an ordinary
  record validates through the ordinary helper, never
  here); the approval-count anchor; a proven planning
  window (a nonzero stored deadline, with a derivable
  open clock); the prior-signal anchor (supplied signals
  numbering exactly `signal_count`, every prior belonging
  to the transition with a stored round matching its own
  `submitted_at`, no duplicated (agent, round)); the
  prior-plan anchor (supplied plans numbering exactly
  `plan_count`, every prior belonging to the transition,
  no duplicated resource inside a prior's claims). Replay
  runs the slice-37 governance prefix (approvals + ingest
  + governance registration) at the stored `eligible_at`,
  then evaluate and mark_eligible at the same clock, then
  each prior signal at its `submitted_at` in order, then
  select and open_planning at the derived open clock (so
  the replayed deadline equals the stored deadline, by
  construction), then each prior plan at the open clock,
  then reconcile at the state's clock, then the stored
  `compatible` verdict cross-checked against slice 23's
  exact mirror recomputed over the supplied plan set (a
  stored verdict the set does not reproduce refuses
  before any kernel validate runs), then kernel
  `validate_transition` at the state's clock. The stage
  advances to `VALIDATED` on a passing predicate, to
  `BLOCKED` on a failing predicate entered from
  `RECONCILING`, and a still-blocked `BLOCKED` record
  refuses and stands — exactly as on the ordinary path.
  The instruction's remaining accounts parse as in slice
  31's reconcile, with the governance tail: prior
  COMMITTED bundles first, then approvals, then prior
  signals, then prior plans, split by
  `split_gov_plan_tail`. The payer is a fee payer only:
  kernel `validate_transition` takes no caller, so no
  proposer, seat, or agent gate applies here. No new
  account, error, or event; the emitted event is the
  existing `TransitionValidated`.
- No governance difference surfaced by a failing test
  this slice: the validated advance, the empty-set and
  over-capacity blocks, the stale-observation block, and
  all ten refusal tests (wrong stage, once-only, still
  blocked, stored-verdict mismatch, incomplete prior
  plans, incomplete prior signals, unreproducible plan,
  ordinary type, non-unanimous approvals, unsealed /
  parent mismatch) passed in their first faithful form —
  apart from one test-compile move error in the new
  fixture (a `PlanRecord` moved into a prior-plan slice,
  then returned), fixed by borrowing through
  `from_ref`, as the ordinary tests do — no test
  weakened, no helper changed. The kernel's predicate,
  capacity verdict, and unanimity gate behave on the
  governance path exactly as on the ordinary one.
- Host tests 361/361 (347 prior + 14 slice-46). Clippy
  back to the pre-existing profile (anchor macro
  `unexpected cfg` noise + the slice-8 test helper's
  `type_complexity` warning at lib.rs:11772, the
  `record_args` helper — line shifted by this slice's
  inserts); zero warnings naming any slice-46 symbol and
  none pointing at the new accounts struct. IDL counts
  move by design (32 instructions, 6 accounts, 34 errors,
  18 events — +1 instruction only, including
  `validate_transition_governance`); `.so` 1,584,080 bytes
  (sha256 `3e97266c…`).
- Deliberately NOT slice 46: the governance declare stage
  — a governance record still cannot reach `DECLARED`
  through instructions alone; a revise-approval
  instruction; and CU measurement — later slices.

## Slice 47 — the governance declare instruction

- `declare_transition_governance` +
  `declare_transition_governance_from_records` +
  `DeclareTransitionGovernance`: the slice-25 declare on
  the governance path. Gates, in order: sealed state;
  stage exactly `VALIDATED`; parent exactly the sealed
  head; observation linkage; a governance type
  (`NotGovernanceTransition` otherwise — an ordinary
  record declares through the ordinary helper, never
  here); the approval-count anchor; a proven planning
  window (a nonzero stored deadline, with a derivable
  open clock); the prior-signal anchor (supplied signals
  numbering exactly `signal_count`, every prior belonging
  to the transition with a stored round matching its own
  `submitted_at`, no duplicated (agent, round)); the
  prior-plan anchor (supplied plans numbering exactly
  `plan_count`, every prior belonging to the transition,
  no duplicated resource inside a prior's claims). Replay
  runs the slice-37 governance prefix (approvals + ingest
  + governance registration) at the stored `eligible_at`,
  then evaluate and mark_eligible at the same clock, then
  each prior signal at its `submitted_at` in order, then
  select and open_planning at the derived open clock (so
  the replayed deadline equals the stored deadline, by
  construction), then each prior plan at the open clock,
  then reconcile at the state's clock, then the stored
  `compatible` verdict cross-checked against slice 23's
  exact mirror recomputed over the supplied plan set (a
  mismatch refuses before any kernel validate runs), then
  kernel `validate_transition` at the state's clock, which
  must yield `Validated` — a `Blocked` verdict refuses
  and the record stands `VALIDATED`, because declare is
  not a verdict writer — and only then kernel
  `declare_commit` at the state's clock, which re-runs the
  full 13-bit predicate: a plan expiry reached since
  validate (the `PLANS` bit) refuses here, the one case
  validate cannot see. The stage advances to `DECLARED`
  on a passing predicate; the sealed head does not move.
  The instruction's remaining accounts parse as in slice
  31's reconcile, with the governance tail: prior
  COMMITTED bundles first, then approvals, then prior
  signals, then prior plans, split by
  `split_gov_plan_tail`. The payer is a fee payer only:
  kernel `declare_commit` takes no caller, so no proposer,
  seat, or agent gate applies. No new account, error, or
  event; the emitted event is the existing
  `TransitionDeclared`. The slice-36 `commit_governance`
  honesty bound is updated in the same pass: the
  governance stage chain is now complete through declare,
  so a governance record reaches `DECLARED` through
  instructions alone and `commit_governance`'s on-chain
  reach is a fully instruction-driven record.
- No governance difference surfaced by a failing test
  this slice: the declared advance, the expired-plan and
  stale-observation refusals, the stored-verdict
  mismatch, and all ten refusal tests passed in their
  first faithful form — the kernel's predicate, capacity
  verdict, and unanimity gate behave on the governance
  path exactly as on the ordinary one.
- Host tests 374/374 (361 prior + 13 slice-47). Clippy
  back to the pre-existing profile (anchor macro
  `unexpected cfg` noise + the slice-8 test helper's
  `type_complexity` warning at lib.rs:12148, the
  `record_args` helper — line shifted by this slice's
  inserts); zero warnings naming any slice-47 symbol and
  none pointing at the new accounts struct. IDL counts
  move by design (33 instructions, 6 accounts, 34 errors,
  18 events — +1 instruction only, including
  `declare_transition_governance`); `.so` 1,618,680 bytes
  (sha256 `4393a4b9…`). The build log carries four
  pre-existing stack-frame lines naming approve/select
  instructions, none naming any slice-47 symbol; the exit
  code is 0 and both artifacts were regenerated.
- Deliberately NOT slice 47: the governance stage chain
  is complete — what remains on the branch is the
  revise-approval decision, then the cu-golden freeze,
  then the Agave 4.3.2 + Anchor next-stable scratch
  assessments; and CU measurement — later slices.

## Slice 48 — the revise-approval decision, decided and built

- Decision, made under Carlos's 2026-10-27 delegation rule
  and surfaced here for his audit or override: BUILD the
  revise instruction; do NOT keep the init-only divergence
  permanently. The reasoning: kernel-first fidelity decides
  it. Kernel `approve_governance` is a map insert that
  overwrites, with no stage gate beyond tick, not-paused,
  and seated council — so a seat that approved the wrong
  action can re-approve before registration, and the
  program bridge that claims to mirror the kernel must
  not refuse what the kernel permits. The fail-closed
  alternative (keep init-only, fresh transition id as the
  only remedy) is safe but is a permanent fidelity gap,
  not a security property: a revised approval is still
  gated, still seat-self-only, and still judged by the
  kernel's unanimity gate at replay. Nothing in the
  predicate, the design corpus, or the project goals
  requires the program to be stricter than the kernel
  here, so the decision is the builder's to make and
  record, not Carlos's to bottleneck on. Hard gates are
  untouched: this is a test-branch program instruction,
  no wallet, transaction, or deployment rides it.
- `revise_governance_approval` +
  `revise_governance_approval_record` +
  `ReviseGovernanceApproval`: the seat's own stored
  approval, overwritten in place. Gates, in order: the
  transition record's id matches the instruction id; the
  stored approval belongs to this transition and to the
  signing seat (`RecordMismatch` otherwise — the PDA
  seeds `[\"gov-approval\", transition_id, seat]` already
  bind a record to its seat, and the helper refuses a
  mismatched record outright rather than overwriting a
  foreign seat's approval); then exactly the
  first-approval gates via
  `build_governance_approval_record` (sealed state,
  governance-typed `REGISTERED` record, seated council
  signer, action category matching the record's type).
  Only the stored action is written back; the bump is
  preserved. The approval count is deliberately NOT
  incremented — the instruction does not call
  `note_governance_approval` — because the bundle
  already counts this seat once, and a second count
  would corrupt the slice-35 self-delimiting bundle
  parse. A revision after the record leaves `REGISTERED`
  is refused (`WrongStage`), exactly as a first approval
  is: once replay runs, the approval set is frozen and
  the kernel's registration has consumed it. A revised
  set that is no longer unanimous fails at governance
  replay, fail-closed, exactly as a non-unanimous first
  set does. The instruction does not run kernel
  `approve_governance` itself; replay runs the kernel
  call from the stored records, as with first approvals.
  New event `GovernanceApprovalRevised` (id, seat, action
  kind) so the event log distinguishes a revision from a
  first approval instead of conflating them under
  `GovernanceApproved`. The slice-33 honesty bound is
  closed in the same pass: approval records are still
  `init`-only as accounts, but a seat's first approval
  no longer stands unchangeably — it is revised through
  this instruction, and the fresh-id remedy is retired
  to a fallback, not the mechanism.
- Host tests 383/383 (374 prior + 9 slice-48: the stored
  action overwritten with seat, transition, and bump
  preserved; a kind change within the same category; a
  mixed stored set (400 vs 500) restored to unanimity
  through the stored-set decoder; the approval count
  untouched by a revision; and the foreign-transition,
  other-seat, wrong-stage, category-mismatch, and
  unsealed refusals, each leaving the stored action
  unchanged). Clippy back to the pre-existing profile
  (anchor macro `unexpected cfg` noise + the slice-8
  test helper's `type_complexity` warning, the
  `record_args` helper); zero warnings naming any
  slice-48 symbol and none pointing at the new accounts
  struct. IDL counts move by design (34 instructions, 6
  accounts, 34 errors, 19 events — +1 instruction and +1
  event, including `revise_governance_approval` and
  `GovernanceApprovalRevised`); `.so` 1,630,384 bytes
  (sha256 `1de0c0a9…`, working-tree build — the
  slices-45/46 oversight pattern of unreproducible `.so`
  sha prefixes applies, so this figure is a tree
  measurement, not a reproducibility claim).
- Stack-frame finding, recorded not smoothed: the build
  log now carries six stack-frame lines (was four at
  slice 47). The two new lines name this slice's handler
  (estimated frame 5,696 bytes, exceeding the 4,096-byte
  limit by 1,600) and its accounts struct (4,544 bytes).
  The class is pre-existing, not new: the slice-33
  `approve_governance` handler itself estimates 5,760
  bytes (exceeding by 1,664) — larger than revise — and
  the sibling-select handlers exceed by 128. `anchor
  build` still exits 0 and regenerates both artifacts,
  as it did for those slices. But a frame over the
  limit is a runtime risk, not just a log line, so this
  class is now a queued item ahead of the cu-golden
  freeze: mollusk execution must prove the
  approve/revise handlers actually run on-chain before
  any W4.4 deployment relies on them, and a frame-size
  refactor (avoiding by-value `StoredGovAction` and
  by-value record returns on the approval path) is the
  fix if execution proves the risk real. Slice 48 does
  not smuggle that refactor in: it ships in parity with
  approve and names the debt.
- Deliberately NOT slice 48: CU measurement, the
  stack-frame refactor above, and the Agave 4.3.2 +
  Anchor next-stable scratch assessments — the queue
  after this slice is the cu-golden freeze (with the
  stack-frame execution check riding it), then the
  toolchain scratch assessments.

## Slice 49 — cu-golden freeze, slice 1 (2026-10-29)

New: `cu-golden/` — a mollusk-svm 0.16.2 harness executing the
real `.so` in-process (no validator, no devnet, no mainnet).
Locked dependencies: the lockfile is copied from the main
treasury program's proven cu-golden lock (fresh resolution
picked solana crates newer than mollusk-svm 0.16.2 compiles
against — 18 compile errors — so the lock is load-bearing, not
decorative). Run from `cu-golden/` with `./run-mollusk.sh`
after `anchor build`; the golden file is
`cu-golden/cu-golden-mollusk.json`.

Measured (BASELINE — no design estimate exists for the v0.4
program, so nothing is gated): probe_kernel 9,795 CU;
initialize_kernel 20,369; seal_kernel 21,346; advance_clock
(100 -> 200) 4,317; register_transition 11,872;
ingest_observation 28,013; register_governance_transition
27,532. State assertions held on every step (genesis
unsealed/head 0/clock 0; sealed clock 100; clock 200; both
records REGISTERED; governance approval count 0). Anchor build
exit 0, IDL unchanged 34 instructions / 6 accounts / 34 errors /
19 events, `.so` 1,630,384 bytes (same size as slice 48).

BLOCKED, NOT MEASURED — `approve_governance` and
`revise_governance_approval`: under mollusk both refuse with
Anchor 3005 (`AccountNotEnoughKeys`, attributed to
`kernel_state`) before any handler gate runs, despite correct
account metas. Ten probes proved the failure intrinsic to these
two instructions under mollusk, not to account state, meta
order, payer identity, or data size: reversed metas, a minimal
account vec, a non-seat payer, a fully pre-initialized valid
approval record, and a padded `advance_clock` (1516 bytes of
data, Success) all leave the 3005 unchanged, while
`replay_transition` on the same governance record reaches its
third account (3012) as expected and truncated revise data
gives the args-deserialize error (102) instead. An Anchor-debug
scratch build confirmed the failure sits inside
`Account::<KernelState>::try_accounts` (only the kernel_state
name logs before the failure; no transition_record line). The
stack-frame execution check is therefore NOT closed by this
slice — it stays queued behind resolving the 3005, and both
instructions are recorded as `Blocked` in the golden file,
never as fake measurements.

DESIGN FINDING (independent of the mollusk 3005):
`approve`/`revise` carry the 1524-byte `StoredGovAction` by
value, so instruction data alone is 1564 bytes; with a 64-byte
signature that already exceeds Solana's 1232-byte transaction
size limit before any account key or blockhash, so neither
instruction could be submitted as a single real transaction as
currently encoded. The slice-33 by-value encoding choice also
explains the stack-frame overages (approve 5,760, revise 5,696,
both over the 4,096-byte limit). Needs a design decision
(compact action encoding or an account-carried action); not
redesigned in this slice.

Deliberately NOT slice 49: the 3005 resolution, the
stack-frame execution check, the later cu-golden slices
(stage instructions through commit_governance), and the Agave
4.3.2 + Anchor 1.2.2 scratch assessments — the queue after
this slice is the 3005 resolution + stack-frame check, then
the toolchain scratch assessments.

## Slice 50 — the 3005 root cause, proven (2026-10-29, docs only; no program change)

The slice-49 3005 is the program's own oversized BPF stack
frame, not a mollusk artifact and not an Anchor bug. The SBF
linker reports the `__global` wrapper frames at approve 5,760
bytes and revise 5,696 bytes, both over the 4,096-byte limit
(build warnings the build still exits 0 on). A scratch variant
program proved it by controlled experiment (same binary, same
accounts, same data): a probe with fields byte-identical to
`ReviseGovernanceApproval` and the same `StoredGovAction` arg
but a trivial body loads accounts fine (3012 on the empty
approval record, as it should); the same probe with revise's
exact body overflows identically (5,696 bytes) and 3005s
identically. Anchor-debug confirms the failure inside the
first field's `try_accounts` with an empty slice. Shape, data
size, meta order, payer identity, and account state were all
separately exonerated. Consequences: approve/revise cannot
execute on-chain as built (not just under mollusk) until their
frames come under 4,096 bytes; the tx-size design finding
stands independently, so the pending design decision (compact
action encoding or an account-carried action) is now the fix
for BOTH findings and any fix must be frame-measured as well
as tx-measured. FORWARD WARNING: the sibling-select wrappers
(`select_candidate_governance_with_siblings`,
`select_candidate_mixed_with_siblings`, 4,224 bytes each) also
overflow and are predicted to fail the same way when the
golden chain reaches them. The `try_accounts`-frame overflows
also reported are NOT the discriminator (a fitting-wrapper
probe whose `try_accounts` overflows at 4,544 still loads).
Both instructions stay `Blocked` in the golden file until a
frame-fitting fix lands and is re-measured. Full evidence:
run log `hidden_files/kernel-build-run-2026-10-29-0228.md`;
scratch probes were run under
`hidden_files/v04-3005-scratch/` (scratch only — the variant
sources are not canonical and must never be measured against
or pushed).

Deliberately NOT slice 50: the frame-fitting fix itself (a
design decision for Carlos, folded into the pending encoding
decision), the later cu-golden slices, and the Agave 4.3.2 +
Anchor 1.2.2 scratch assessments.

## Slice 51 — the account-carried action, carrier only (2026-10-29)

Carlos's encoding decision, refined the same morning
(2026-10-29 ~05:19 CST): the action is account-carried, written
in CHUNKED APPEND + FINALIZE on the current legacy/v0 stack (a
later v1 single write may replace the chunks without changing
the account layout or the hash rule). This slice builds the
carrier; the approval rewiring is slice 52.

- `GovActionAccount` (SPACE 1664): one per
  `(transition, writer)`, PDA seeds
  `["gov-action", transition_id, writer]`. Fields: transition
  id, writer, `expected_hash` (recorded at init, before any
  byte), `action_hash` (written by finalize alone, zero until
  then), `written_len`, `finalized`, the 1,524-byte serialized
  action buffer, bump. The action travels as raw bytes, never
  as a by-value instruction argument — that is the fix for
  both the 1,564-byte transaction finding and the 5,760-byte
  wrapper frame.
- Four instructions: `init_gov_action` (writer = signing
  payer; records the expected hash), `append_gov_action_chunk`
  (offset + up to 512 bytes; a chunk instruction is ~550 bytes
  of instruction data, inside the legacy 1,232-byte cap),
  `finalize_gov_action` (refuses incomplete buffers and
  undecodable buffers; computes the hash from the accumulated
  bytes with the `sol_sha256` syscall on SBF — never from a
  caller-supplied hash — and succeeds only if it equals the
  expected hash), `close_gov_action` (rent reclaim for a
  half-written account; writer-only, refused once finalized).
- Squatting, closed by both defenses together: the seed
  includes the writer AND writes require the writer's
  signature, so no other party can pre-create the account for
  a transition id; and init records the expected hash, so a
  writer cannot init one action and finalize another.
- Sequential-prefix rule: a chunk's offset must equal
  `written_len` exactly — no gaps, no overwrites. A gapped
  buffer can never finalize as a different action, and a
  poisoned chunk cannot be repaired in place (fail-closed;
  remedy is a fresh transition id, as with a wrong first
  approval before slice 48).
- Frame result (pinned toolchain, this slice's build): the
  four new `__global` wrappers report NO overflow — the
  overflow list is exactly the baseline's (approve/revise
  5,760 each, sibling selects 4,224 each, `try_accounts`
  4,480 each, BTreeMap-insert 4,672). Approve/revise still
  overflow in this slice, as designed: they still take the
  action by value, and their rewiring is slice 52.
- Baseline correction, found while measuring: the canonical
  revise wrapper is 5,760 bytes, not the 5,696 slice 50
  reported — 5,696 was the scratch probe variant's frame, and
  two independent pinned-toolchain builds of the canonical
  program (this slice's baseline and the Agave assessment's
  scratch build) both report 5,760. The acceptance bar is
  unchanged, the number is corrected.
- Agave 4.3.2 correction, found in the same pass: the
  assessment's scratch build log shows toolchain
  `1.89.0-sbpf-solana-v1.54` — its build ran on the pinned
  toolchain, not v1.57, and its `.so` is byte-identical to
  this slice's pinned build. Its CU deltas (-4% to -21%) are
  therefore NOT Agave 4.3.2 effects and are withdrawn pending
  a genuine v1.57 build (the build must be forced onto the
  4.3.2 install's `cargo-build-sbf` and the toolchain line
  confirmed in the log). The golden `.so` sha (1de0c0a2…)
  likewise does not reproduce from current source + pinned
  toolchain (twice 407907c8…); CU golden values stay valid
  as measurements of their `.so`, but the sha is an open
  verification debt, not a reproducibility claim.
- Host tests 388/388 (383 prior + 5 slice-51: serialized
  length + SPACE, chunk-sequence finalize, gap/overwrite/
  oversize/empty refusals, incomplete/hash-mismatch/foreign-
  writer refusals, close gate + post-finalize refusals).
  `anchor build` green under the pinned toolchain; IDL now
  38 instructions / 7 accounts / 38 errors / 23 events;
  `.so` sha256 `58e601a6…`.
- Approval-record shrink audit (the refined ruling's
  confirm step): exactly ONE production function decodes an
  action from approval records — `gov_action_and_seats`
  (the shared decoder, called from ~14 helpers: commit,
  stage chain, prior drivers, sibling helpers) — plus the
  approve/revise bodies' `action.kind` event reads. Under
  the shrink, all of them must read the action from the
  `GovActionAccount` instead; `GovPriorCommit` bundles
  (which carry approval-record slices) must additionally
  carry the action account. That rework is slice 52's scope,
  not smuggled into this carrier slice.

Deliberately NOT slice 51: rewiring approve/revise to read
the action account (their frames still overflow), the
registration/commit hash checks (registration refuses an
unfinalized account; commit takes the action account in),
the sibling-select wrapper fix (still 4,224 each), the
post-settle reclaim decision for finalized accounts, the
cu-golden measurements for the four new instructions, and
the genuine Agave 4.3.2 / Anchor 1.2.2 builds.

## Slice 52 — approval rewiring + registration/commit hash checks (2026-10-29)

- `approve_governance` / `revise_governance_approval` take no
  action argument any more: the 1,524-byte by-value argument
  is gone from the instruction data, and the action is read
  from the finalized `GovActionAccount` (gate:
  `check_gov_action_for_approval` — account belongs to the
  transition, writer is the record's proposer, finalized,
  and the stored bytes hash to the finalized `action_hash`).
  The approving seat is not the writer, so both instructions
  take an explicit `writer` account (seed component only,
  `/// CHECK:`-documented) and the action account is
  PDA-bound to it.
- `register_governance_transition` takes the action account
  (seeded by the registering proposer) and refuses an
  unfinalized or foreign-written account before any record
  exists (`check_gov_action_for_registration`) — the
  slice-51 proposer binding, now enforced.
- `commit_governance` takes the action account in (the
  lifetime rule's commit half) and proves the binding
  against the approved set itself
  (`check_gov_action_for_commit`): the action decoded from
  the approval records (unanimity re-checked by the shared
  decoder) is re-encoded to canonical stored form, hashed,
  and compared to the finalized hash — a substituted
  action refuses with `GovActionHashMismatch` before any
  replay runs. New error `GovActionNotFinalized` (6038).
- Frame result (pinned toolchain): the rewiring alone made
  the wrappers WORSE — approve 5,760 → 7,552, revise
  5,760 → 7,488 — because the new 1,664-byte account
  lands in the wrapper frame while the removed argument
  saved nothing there. The fix is boxing (Anchor
  `Box<Account<…>>`, no IDL/seeds/semantics change):
  the gov-action account is boxed in all four
  instructions and the approval record is boxed in
  approve/revise. After boxing, the overflow list is back
  to the pre-slice-52 three: BTreeMap-insert 4,672 and
  the two sibling-select wrappers at 4,224 each — the
  approve, revise, register, and commit `__global`
  wrappers (and their `try_accounts`) report no overflow.
  Exact fitting sizes are not emitted by the toolchain,
  so the margin is recorded as "fits 4,096, margin
  unquantified" — the ruled with-margin bar is NOT yet
  met on measurement alone (see below).
- Host tests 390/390 (388 prior + 2 slice-52: approval /
  registration gates incl. unfinalized, foreign-writer,
  and foreign-transition refusals; commit gate incl. the
  substituted-action hash mismatch). Plain clippy exit 0
  (only the pre-existing 43 cfg warnings + the old
  `record_args` type-complexity warning; `-D warnings`
  was already failing on those before this slice).
  `anchor build` green under the pinned toolchain; IDL
  now 38 instructions / 7 accounts / 39 errors / 23
  events; `.so` sha256 `741230a2…`.
- Frame-margin probe (scratch copy, threshold build):
  MEASURED after the slice-52 push (run-log addendum):
  rebuilding the pushed tree with `--sbf-stack-size 2048`
  leaves the approve, revise, register-governance, and
  commit-governance `__global` wrappers OUT of the
  overflow list — each fits 2,048 bytes, i.e. at least
  2,048 bytes of margin under the 4,096-byte limit, which
  meets the ruled with-margin bar on the wrapper
  measure. Toolchain correction, stated not smoothed:
  the probe's build log stamps
  `1.89.0-sbpf-solana-v1.54` — the Agave 4.3.2
  `cargo-build-sbf` resolved to the pinned v1.54 SBF
  toolchain, the same phenomenon slice 51 caught in the
  4.3.2 assessment, so this is a lowered-threshold
  measurement on the pinned compiler, not a v1.57
  measurement. A mollusk execution of approve/revise
  (Blocked since slice 49) remains the runtime proof and
  is still open; the frame bar itself is now met.

Deliberately NOT slice 52: the approval-record shrink to
the 32-byte hash (records still store the full
`StoredGovAction`, so the ~14 replay helpers are
untouched — the shrink, with `GovPriorCommit` bundles
carrying the action account, rides slice 53); the
sibling-select wrapper fix (mixed + governance-with-
siblings still 4,224 each; the ordinary with-siblings
wrapper never overflowed); the post-settle reclaim
decision for finalized accounts; cu-golden measurements
for the slice-51 instructions; the genuine Agave 4.3.2 /
Anchor 1.2.2 builds.

## Slice 53 — the approval-record shrink (2026-10-29)

- `GovApprovalRecord.action` (the full 1,524-byte
  `StoredGovAction`) is replaced by `action_hash:
  [u8; 32]`; SPACE 1,597 → 105. The record no longer
  carries action bytes at all, so no replay path can
  decode the action from an approval record any more —
  the slice-51 fix-run requirement, confirmed: every
  path that used to decode from records now reads the
  action from the finalized `GovActionAccount`.
- `gov_action_hash` (new): SHA-256 of the canonical
  stored serialization — the same bytes finalize hashes
  in the `GovActionAccount`, so an approval record's hash
  binds the seat to exactly the finalized action.
  `gov_action_from_account` (new): requires finalized,
  requires the stored bytes to hash to the finalized
  `action_hash`, then decodes.
- The shared decoder changes shape:
  `gov_action_and_seats(action, records, tid)` no longer
  returns the action — it verifies the seats against a
  supplied action: every record names the transition,
  seats are unique, every record's `action_hash` equals
  `gov_action_hash(action)`, and the set is non-empty.
  A tampered hash refuses with `GovActionHashMismatch`;
  there is no longer a corrupt-decode path from approval
  records (`GovActionDecode` survives only for corrupt
  stored-action bytes inside an account buffer).
- Threading: the eleven governance `*_from_records`
  helpers plus `commit_governance_from_stored` take
  `action: &GovAction` (after `approval_records`);
  `GovPriorCommit` gains `action: Option<&GovAction>`;
  `OwnedPriorBundle` gains `action: Option<GovAction>`
  (decoded at parse time, with the exactly-one action
  account + hash checks in `split_prior_bundles`);
  `replay_governance_transition_chain` takes the action;
  the stage / signal / plan splits return the decoded
  action from the trailing finalized `GovActionAccount`
  (stage: approvals then exactly one action account;
  signal: approvals, signals, action last; plan:
  approvals, signals, plans until the trailing action
  account); sibling handlers collect `GovAction`
  accounts and resolve them by (transition id,
  proposer) with an exactly-one requirement, and the
  sibling replay structs carry the action owned
  (`GovSiblingReplay.action`, `MixedSiblingReplay.action`).
  Approve/revise event kind now comes from
  `StoredGovAction::encode(&decoded)?.kind`.
- Frame result (pinned toolchain): the overflow list is
  now EXACTLY ONE function — the pre-existing
  BTreeMap-insert at 4,672 bytes, not an instruction
  wrapper. Approve, revise, register-governance, commit-
  governance, AND both sibling-select wrappers now fit
  at 4,096: the shrink fixed the sibling wrappers as
  predicted (the 1,597-byte record copies were the frame
  driver; sibling wrappers previously overflowed by only
  128 bytes). The record shrink is the fix; the slice-52
  boxing stays as side relief.
- Frame-margin probe: MEASURED 2026-10-29 ~07:3x CST
  (BUILD run scheduled 07:28 CST), closing the slice-53
  measurement debt. Scratch copy of the pushed tree
  (lib.rs sha256 `03dd5686…`, byte-identical to staging)
  built with `cargo build-sbf --sbf-stack-size 2048`
  under the Agave 4.3.2 scratch toolchain
  (cargo-build-sbf 4.4.0 / platform-tools v1.57 /
  rustc 1.95.0 — the pinned v1.54 toolchain does not
  support `--sbf-stack-size`, so the threshold route
  cannot run on it; frame sizes vary only by tens of
  bytes across these compilers, far below the measured
  margin). The 2,048-byte overflow list is COMPLETE
  (47 functions, including the BTreeMap-insert at
  4,672 bytes — the completeness marker the 07:12
  oversight probe found missing): its `__global`
  entries are exactly the four slice-51 gov-action
  lifecycle wrappers (init 3,520; close 3,456;
  finalize 3,456; append 3,584 — all fit at 4,096, as
  before). Approve, revise, register-governance,
  commit-governance, AND both sibling-select
  `__global` wrappers are ABSENT from the list: each
  fits 2,048 bytes, i.e. at least 2,048 bytes of margin
  under the 4,096-byte limit. The ruled with-margin
  bar (Carlos 2026-10-29 ~05:19 CST) is therefore MET
  for slice 53 on the wrapper measure. Helper-function
  frames between 2,048 and 4,096 bytes (e.g.
  `gov_action_hash` 3,136) are not the entry-path
  gate per the slice-50 methodology and fit at 4,096.
  The pinned-toolchain fit-at-4,096 result (overflow
  list exactly BTreeMap-insert) stands alongside this
  threshold measurement; the two together close the
  slice-53 margin question.
- Host tests 392/392 (390 prior + 2 slice-53: the
  approval hash rule equals the finalize hash rule, and
  a hash-tampered approval set refuses stage replay with
  `GovActionHashMismatch` before any kernel call).
  Corrupt-action tests were redesigned, not smoothed:
  hash-tamper cases now expect `GovActionHashMismatch`.
  Coverage gap, noted not hidden: no dedicated
  `split_gov_plan_tail` test existed and none was added
  — plan tails are covered only via the
  submit_plan/reconcile/validate/declare helper tests.
  Clippy could not complete this session (the same
  sandbox I/O failure; the lib pass reported only the
  pre-existing custom-heap cfg warning and no new lint
  before the driver output parse failed on EPERM) —
  re-run clippy in a healthy session before treating
  the lint bar as re-confirmed. Delivered v0.4.1 crate
  re-run unmodified and green, 39/39. IDL unchanged:
  38 instructions / 7 accounts / 39 errors / 23 events
  (Anchor 1.2.1 IDL accounts carry only names +
  discriminators, and neither changed). Pinned
  `anchor build` green; `.so` sha256 `e2b474e7…`.
  Doc comments were swept in the same pass: handler,
  helper, split, and decoder docs that still said the
  action is "decoded from stored approvals alone" now
  say the action comes from the finalized account and
  the approvals are hash-verified against it; the
  historical slice notes (slices 34/36/37) are left as
  written — they describe what was true then.

Deliberately NOT slice 53: the 2,048-byte margin probe
(blocked environmentally, rides the next healthy
session); the mollusk execution of approve/revise
(Blocked since slice 49, still the runtime proof);
the genuine Agave 4.3.2 / Anchor 1.2.2 builds;
cu-golden measurements for the slice-51 instructions;
the post-settle reclaim decision for finalized
accounts; the golden `.so` sha debt (slice-49
`1de0c0a2…` still not reproducible from current source
+ pinned toolchain).

## Slice 54 — the settle tail at helper level, ordinary path (2026-10-29)

- `settle_transition_from_records`: `COMMITTED` (10) ->
  `SETTLED` (11, a new program stage value matching the
  kernel's `Life::Settled`), the first half of the bridge
  slice-4 tail on-chain. The helper re-proves the commit
  in a rebuilt kernel — ingest + register + evaluate +
  mark_eligible at the stored `eligible_at`, signals at
  their `submitted_at`, select + open_planning + plans at
  the derived open clock, reconcile + validate + declare +
  commit at the stored planning deadline (the slice-29
  clock rule for historical reproduction) — and only then
  runs the tail at the state's clock: `start_execution`
  by the launch Executor, two `attest_settlement` calls
  by the launch Settlers on the supplied realized hash
  (threshold 2, so the first must return false and the
  second true), and the `Settled` life assertion. On
  success only the record's stage advances: the sealed
  head and the slice-27 ledger are untouched, because
  settle closes a commit that already moved both.
- Gates, in order: sealed state (`NotSealed`); stage
  exactly `COMMITTED` (`WrongStage` — a settled record
  settles once only); observation linkage
  (`RecordMismatch`); an ordinary type
  (`GovernanceSettleRequired` otherwise — the governance
  settle mirror is the next slice); the signal/plan
  count anchors and ownership/round rules; proposer /
  observer gates against the running kernel
  (`UnknownProposer` / `NotObserver`); the stored
  `compatible` verdict cross-checked against the
  recomputed mirror, exactly as in the commit path.
- The parent-head build, stated not smoothed: the earlier
  chain replays against a parent state — the program
  state with its head fields set to the target's parent —
  so `kernel_at_head_from_entries`' own landing check
  becomes the parent check (a chain overshooting the
  parent, or stopping short of it, refuses with
  `ParentMismatch` before any target replay). The
  re-commit must land exactly one past the parent at the
  declared next hash, or the stored `COMMITTED` label is
  unproven. The live head is NOT required to equal the
  declared hash: a successor may already have committed
  past this transition, and requiring head equality would
  brick settlement of every non-head commit.
- The slice-27 ledger gate is deliberately NOT re-applied:
  the ledger already contains this transition's claims
  (written by the commit instruction), so re-applying it
  would double-count and refuse every legitimate settle.
  The kernel's own commit still judges the claims against
  the sealed stocks in the rebuilt kernel.
- Honesty bounds, stated not smoothed: the realized hash
  is a helper argument and both attestations name it —
  the kernel's per-hash vote counting plus the threshold
  is what is proven here. The step-2 `SettlerGate`
  ledger binding (settlers attest ONLY a hash an
  observing process recorded) is NOT enforced at helper
  level: no on-chain evidence record exists yet, so that
  binding rides the instruction slice, which will source
  the hash from stored evidence instead of a caller
  argument. Outcome recording (`Settled` ->
  `EvaluatedOutcome`, attribution) is the next slice after
  the governance settle mirror, not this one: exactness
  (realized == declared) therefore does not change this
  slice's outcome — an inexact settle still settles, and
  the halved-credit rule bites at outcome time.
- Host tests 402/402 (392 prior + 10 slice-54: exact
  settle with head and ledger untouched, inexact settle,
  once-only, wrong-stage, governance-type,
  past-execution-deadline (clock 1301 against the
  300 + 1000 window), regressed-clock, unsealed,
  overshooting-prior-chain, and settle of a second
  commit at a moved head from its prior chain with head
  2 and ledger 20/10 untouched). Clippy back to the
  pre-existing profile (anchor macro `unexpected cfg`
  noise + the slice-8 helper's `type_complexity` note +
  the gov-helper `too_many_arguments` notes); zero new
  warnings — one doc-wrap fix was needed to keep it
  there (a doc line starting with `+` parsed as a
  rustdoc list marker, the slice-26 trap).
  `anchor build` green under the pinned toolchain
  (Anchor 1.2.2 / Agave 4.3.2 / platform-tools v1.57);
  IDL counts move by design (38 instructions, 7 accounts,
  41 errors incl. `SettleFailed` and
  `GovernanceSettleRequired`, 24 events incl.
  `TransitionSettled` — no new instruction or account,
  as designed); `.so` 1,658,984 bytes (sha256
  `cb5bec94…`). Cu-golden re-measured against the new
  `.so` (17 measured, blocked `[]`): append 4,682 (was
  4,681), approve 37,629 (was 37,632), finalized-close
  refusal 5,334 (was 5,332), the other fourteen values
  exact — layout noise from the source change, not a
  behavior change; the golden JSON + cu-golden README
  carry the new values in the same pass.
- Deliberately NOT slice 54: the `settle_transition`
  instruction (remaining-account parse + the evidence
  record the SettlerGate binding needs); the governance
  settle mirror; outcome recording; and the golden `.so`
  freeze, which still waits on the post-settle reclaim
  ruling — this slice supplies the `SETTLED` stage that
  ruling's gate will check.

## Build

```sh
export RUSTUP_HOME=~/workspace/toolchains/rustup CARGO_HOME=~/workspace/toolchains/cargo
export PATH=~/workspace/toolchains/cargo/bin:~/workspace/toolchains/solana/bin:$PATH
~/workspace/toolchains/anchor/bin/anchor build   # from this directory
cargo test                                        # host tests, 392/392
```
