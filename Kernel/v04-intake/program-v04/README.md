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

## Build

```sh
export RUSTUP_HOME=~/workspace/toolchains/rustup CARGO_HOME=~/workspace/toolchains/cargo
export PATH=~/workspace/toolchains/cargo/bin:~/workspace/toolchains/solana/bin:$PATH
~/workspace/toolchains/anchor/bin/anchor build   # from this directory
cargo test                                        # host tests, 69/69
```
