# program-v04 — migration step 5, slice 5: the program scaffold

The delivered v0.4.1 kernel crate compiled into an Anchor program under
the pinned toolchain (Anchor 1.2.1 / Solana 4.1.2 / platform-tools
v1.54). One probe instruction only (`probe_kernel`); the instruction
families, account inventory, and CU golden set are later step-5 slices.

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
- Deliberately NOT slice 24: declare, commit replay moving the
  head, and CU measurement — later slices.

## Build

```sh
export RUSTUP_HOME=~/workspace/toolchains/rustup CARGO_HOME=~/workspace/toolchains/cargo
export PATH=~/workspace/toolchains/cargo/bin:~/workspace/toolchains/solana/bin:$PATH
~/workspace/toolchains/anchor/bin/anchor build   # from this directory
cargo test                                        # host tests, 174/174
```
