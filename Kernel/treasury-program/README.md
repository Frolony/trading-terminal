# treasury-program — own treasury program (W4.3 scaffold)

**Status: KERNEL VENDORED, host-proven; SBF-GREEN on the adopted toolchain (2026-10-24).**
Anchor project at `Kernel/treasury-program/` — its own Anchor project, NOT
a member of the `Kernel/` Cargo workspace. The pristine kernel crate is now
a path dependency (`programs/treasury-program/src/kernel_bridge.rs`):
launch policy + config as named items, host tests proving the full
lifecycle clears the 13-bit mask and commits while an over-claim fails
COMPAT and commits nothing (program tests 9/9).

**D-12, closed 2026-10-24 — read before building.** The SBF blocker is
gone, by both routes in sequence: (a) the kernel's author revised the two
`Option::is_none_or` call sites (`kernel/src/kernel.rs:401`, `:537`) to the
1.79-compatible `map_or(true, f)` form (remote `3ef5c412`, 06:21 CST,
exactly those two lines); then Carlos adopted Solana 2.2.20 (below) and
pushed the revert (remote `f3086ac6`, 06:38 CST), restoring the author's
original `is_none_or` text byte-for-byte (remote blob SHA matches the
archived pristine crate). Final state: original kernel text + Solana
2.2.20, `anchor build` exit 0, host clippy clean again (the `map_or` form
had tripped host clippy 1.99's `unnecessary_map_or` — the revert removes
that friction too). The vendoring increment is PUSHED (remote
`0df20ec7`, 2026-10-24 ~07:06 CST). The §4.1 admin-setup slice landed
(remote `1a5f53b7`, 2026-10-24 ~07:21 CST): `AgentRecord` +
`register_agent` / `grant_role` / `revoke_role` (genesis-direct form).
This increment adds the first lifecycle instructions in design order:
`post_observation` (Oracle-only, Clock-judged) + `register_transition`
(TransitionRecord in Registered state; observation must already exist;
lineage judged at commit, not here) + `open_planning` (proposer-or-Admin
only, Registered → Planning, deadline `now + PLANNING_WINDOW`; the
TransitionRecord now carries the recorded proposer key, SPACE 74 → 106)
+ `submit_plan` (Planning-only, before the deadline, plan agent ==
caller, one plan per agent via PDA seeds, capacity NOT checked here —
COMPAT at reconcile/commit; standalone PlanRecord accounts, SPACE 253;
TransitionRecord gains `plan_count`, SPACE 106 → 110, so TooManyPlans
is enforceable on-chain). Carried gap, stated in the code: the kernel's
group-overlap half of plan eligibility awaits `relevant_groups` on the
record (manifest completion); this slice enforces the active half.
+ `submit_signal` (Registered-or-Planning, support ≤ 1.0, eligible
agent, one signal per agent per round via PDA seeds, round matched to
the transition's current round; TransitionRecord gains `round_count`,
SPACE 110 → 114, starting at 1 — the kernel starts every transition
with one empty round; the monotone-revision check is pure-helper
tested now and wires on-chain with `advance_round`, which snapshots
the previous round's mean — until then only round 0 is reachable,
where the kernel's monotone branch is vacuous).
+ `submit_constraint_report` (Evaluator-only, Registered-or-Planning,
report against a member of the frozen set — an outside id is
`UnknownConstraint`, never a new constraint; reports live on the
TransitionRecord, not standalone accounts, and overwrite the entry's
status). `register_transition` now takes the frozen set (≤ 8 distinct
ids, all Unavailable at registration; TransitionRecord SPACE 114 →
382). Scaffold-form note: the caller supplies the frozen set at
registration; when PolicyRecords land, registration derives the
mandatory part from the transition type's policy and unions the
proposer's additions, as the kernel does.
+ `submit_objectives_check` (Evaluator-only, Registered-or-Planning,
independent bool verdict, never derived from plan fields; overwrites
the TransitionRecord's `objectives_ok` flag, false at registration;
TransitionRecord SPACE 382 → 383). The OBJECTIVES bit at commit is
`require_objectives && !objectives_ok` — at launch the policy does not
require objectives, so the bit is clear regardless, but the flag is
recorded now so a requiring policy is judged on a real report.
+ `advance_round` (permissionless, as in the kernel — the kernel
ignores its caller; Registered-or-Planning, current round non-empty,
room under `max_rounds` 3, in that check order; new error
`RoundEmpty`, IDL errors 19 → 20). The RoundEmpty gate is faithful,
not a signal-count proxy: `submit_signal` now maintains the current
round's effective-weight total and weighted sum on the
TransitionRecord (FlatWeights: raw × signer influence, Amount
arithmetic, computed from on-chain records), and a zero total — no
signals, or only zero-weight signals — cannot advance, exactly as
the kernel's `total == Amount::ZERO` test refuses. Advancing
snapshots the previous round's weighted mean (`prev_round_mean`)
and resets the aggregates; TransitionRecord SPACE 383 → 407.
Program host tests 27 → 29. Carried gap, stated in the code: the
monotone-revision check also needs the agent's own previous-round
support (that round's SignalRecord), which `submit_signal` does not
yet take as an account — until that wiring lands, commit replay
enforces the check (the kernel's `submit_signal` during replay
refuses a violating sequence).
+ `reconcile` (permissionless, as in the kernel — the kernel
ignores its caller; Planning-only, and only at or past the planning
deadline, `PlanningStillOpen` before it; a false fit still advances
the state to Committable — the COMPAT bit at commit, re-checked,
then refuses every commit; new error `PlanningStillOpen`, IDL
errors 20 → 21). The fit rule is proven as a pure helper (aggregate
claims per resource, checked addition, missing stock or over-claim
fails, overflow fails); the handler evaluates it against the stock
set that exists today, which is empty — no CapacityStock accounts
exist yet — so a plan-bearing transition reconciles false
(fail-closed) and a plan-free one reconciles true (empty aggregate
fits vacuously; commit's PLANS bit refuses it anyway). When stocks
land, the handler sums the transition's PlanRecord claims against
them through the same helper. TransitionRecord gains `reconciled`,
SPACE 407 → 408. Program host tests 29 → 31.
+ `commit` (permissionless, as in the kernel — the predicate decides,
not the caller; Committable-only, `BadState` otherwise; the full
13-bit `commit_fail_mask` computed on-chain by a pure helper, proven
bit by bit plus a kernel-parity case; a non-zero mask refuses with
`CommitFailed`, changes nothing, emits no event, and logs the mask with
`msg!` — Anchor error codes are unit variants, so the code alone cannot
carry it). On a clear mask the transition moves to Committed and the
`DeclaredChain` advances to the declared hash. The handler reads the
mask's inputs from on-chain records only: constraint reports and the
objectives flag on the TransitionRecord, the last round's aggregates on
the same record, the observation account (FRESH), the Clock, PlanRecord
accounts passed as remaining accounts (PLANS expiries, COMPAT
aggregate), and the `DeclaredChain` (LINEAGE, created at genesis by
`initialize_config` at index 0 / zero hash — no create-on-first-use
path, and no `init_if_needed` cargo feature). Remaining-accounts
completeness is enforced: exactly `plan_count` accounts, each
program-owned, each naming this transition, each pubkey distinct —
anything else is `PlanMismatch`, never a partial mask. Honest interims,
stated in the code: CapacityStock accounts do not exist yet, so COMPAT
is judged against the empty stock set (a plan with claims fails,
fail-closed; an empty-claim plan fits vacuously and can commit);
EXCEPTIONS is fed by the transition's live `unresolved_blocking`
count, and CAPABILITY is clear because no transition type carries
requirements yet — both bits are computed, and their inputs land with
their owning instructions. Recording lineage required one honest change to an earlier
slice: `register_transition` now takes the manifest's parent index and
parent hash and freezes them on the record (SPACE 408 → 448). New errors
`CommitFailed`, `ObservationMismatch`, `PlanMismatch`, IDL errors
21 → 24, instructions 13 → 14, accounts 6 → 7. Program host tests
31 → 34.
**This increment adds `reject_transition` / `expire_transition`
(design §4.1)** and closes the §4.1 lifecycle: both are terminal,
neither takes a new account, and neither moves SPACE. `reject` is
Evaluator-or-Admin from Registered, Planning, or Committable only —
the kernel's role test, then its `set_state` gate — and is not
idempotent (re-vetoing a terminal transition refuses with
`BadState`). `expire` is permissionless, as in the kernel (no
AgentRecord, expiry is a Clock fact, not an authority's act), and
keeps the kernel's check order: `BadWindow` while `now` precedes the
manifest's `expires_at`, then `BadState` for non-pre-commit states —
so an unexpired Committed transition reports the window, not the
state. No new errors (both reuse `Unauthorized` / `BadState` /
`BadWindow`); IDL: instructions 18 → 20 (incl. both terminal
instructions), accounts 8, errors 31. Program host tests 41 → 43.
**This increment adds `policy_transition` (design §4.1)** and closes
the §4.1 lifecycle instruction set: the `PolicyRecord` account (PDA
seeds `["policy", transition_type]`, SPACE 666) holds one transition
type's policy — quorum, confidence threshold, minimum observation
confidence, objectives requirement, planning window, round and plan
caps, the mandatory constraint set, and the required capabilities —
installed by the `Config` authority in genesis-direct form, like the
other Admin setup calls (one-time: installing a type twice fails at
account creation). `register_transition` now takes the transition's
type and its PolicyRecord, and the frozen set is derived as the kernel
derives it: the type's mandatory constraints ∪ the proposer's
additions, deduped, all Unavailable — a proposer can no longer omit a
mandatory gate, and the merged set must still fit 8 entries.
TransitionRecord gains `transition_type` (SPACE 768 → 800). New error
`InvalidCapabilities`; IDL: instructions 20 → 21 (incl.
`policy_transition`), accounts 8 → 9 (incl. `PolicyRecord`), errors
31 → 32. Program host tests 43 → 47. Honest interims, stated in code:
per-type numeric values are recorded now but lifecycle handlers still
read the launch constants (installed launch values equal them, so no
behavior change until the wiring increment); the CAPABILITY bit's
on-chain input still awaits `grant_capability` (per-agent holdings);
post-genesis policy changes remain the execution of a committed
`treasury.policy.v1` transition, not a fresh authority signature —
the amendment execution is a later increment. Next: per-type policy
enforcement in handlers, `grant_capability` (AgentRecord holdings),
`set_capacity` (CapacityStock accounts, closing the COMPAT interim),
then §4.2 treasury.

**This increment wires per-type policy enforcement into the lifecycle
handlers (design §4.1)** and closes the first of the interims above:
`open_planning`, `submit_plan`, `submit_signal`, and `advance_round`
now take the transition type's PolicyRecord (seeds-bound to the
TransitionRecord's `transition_type`, so a stranger's policy cannot be
substituted) and read their caps from it — planning window, plan cap,
round cap — instead of the launch constants. Installed launch values
equal those constants, so no behavior changes on transitions under a
launch policy. IDL: instructions 21 (unchanged count — the four
instructions gain the `policy_record` account), accounts 9, errors 32.
Program host tests 47 → 48 (per-type caps govern the pure gates at
non-launch values). Remaining interims: `commit`'s mask inputs (quorum,
confidence, minimum observation confidence, objectives requirement)
still read the launch policy — the mask-input wiring is the next
policy increment; the CAPABILITY bit's holdings still await
`grant_capability`; post-genesis policy changes remain the execution
of a committed `treasury.policy.v1` transition. Next:
`grant_capability`, `set_capacity`, then §4.2 treasury.

**This increment wires the `commit` mask inputs to the type's
PolicyRecord (design §4.1)** and closes the second interim above:
`commit` now takes the transition type's PolicyRecord (seeds-bound to
the TransitionRecord's `transition_type`) and reads its quorum,
confidence threshold, minimum observation confidence, and objectives
requirement from it instead of the launch policy. Installed launch
values equal those constants, so no behavior changes on transitions
under a launch policy. IDL: instructions 21 (unchanged count — `commit`
gains the `policy_record` account), accounts 9, errors 32. Program host
tests 48 → 49 (the mask follows the record's values at non-launch
settings and clears at launch settings). Remaining interim: the
CAPABILITY bit's holdings still await `grant_capability`; post-genesis
policy changes remain the execution of a committed `treasury.policy.v1`
transition. Next: `grant_capability`, `set_capacity`, then §4.2
treasury.

**This increment lands `grant_capability` / `revoke_capability`
(design §4.1)** and closes the last CAPABILITY interim: per-agent
capability holdings live on the AgentRecord (the design §3
CapabilityRegistry in program form, SPACE 81 → 341, at most 8
holdings — the kernel's set is unbounded, the on-chain record is
not), granted genesis-direct by the `Config` authority like
`grant_role` (duplicate grant a no-op success, a ninth distinct
grant `TooManyCapabilities`; revoke a no-op when unheld, mirroring
the role-revoke shape; the kernel asks nothing about activity, and
neither does this). `commit` now reads the CAPABILITY bit from live
inputs: its remaining accounts are the transition's plans followed by
one AgentRecord per distinct plan author, and the bit is the kernel's
`capability_covered` over the type's required capabilities against
the authors' summed influence (checked addition — an overflowing sum
fails the requirement, never wraps; influence held by a non-author
counts for nothing). A missing, foreign, or duplicate author record
is the new `CommitAccountsMismatch`, never a mask over partial
holdings. IDL: instructions 21 → 23 (incl. both capability
instructions), accounts 9 (no new account type), errors 32 → 34
(`TooManyCapabilities`, `CommitAccountsMismatch`). Program host
tests 49 → 51 (grant/revoke set semantics; the coverage helper
matrix incl. the overflow-fails case). (At that point the §4.1
lifecycle was input-complete for every mask bit except COMPAT's
real stocks — the paragraph below closes that interim.)

**This increment lands `set_capacity` (design §4.1)** and closes
the COMPAT interim: the `CapacityStock` account (PDA seeds
`["stock", resource_id]`, SPACE 19) is installed genesis-direct by
the `Config` authority like `policy_transition` (one-time per
resource id; the balance moves afterwards only by `commit`'s
atomic debit and by committed-flow replenishment). `reconcile`
judges its fit against the real stocks (remaining accounts: plans
then stocks, `PlanMismatch` / the new `StockMismatch` on any
mismatch — a claimed resource with no stock account passed still
fails, fail-closed) and `commit` does the same against the stocks
passed after the plans and authors (writable, so a clear mask can
debit them), debiting atomically on a clear mask only via the pure
`apply_debit` helper (computed whole before any write; a refused
commit writes nothing). IDL: instructions 23 → 24 (incl.
`set_capacity`), accounts 9 → 10 (`CapacityStock`), errors
34 → 35 (`StockMismatch`). Program host tests 51 → 53
(`CapacityStock` SPACE; the debit matrix incl. exact-fit,
over-claim-fails-whole, missing-stock-fails, and overflow-fails).
The §4.1 lifecycle is now input-complete for every mask bit: every
bit reads its governing on-chain record. Remaining: post-genesis
policy execution (amendment through a committed
`treasury.policy.v1` transition), then §4.2 treasury.
+ `apply_policy_amendment` (design §4.1, post-genesis policy
execution — the only post-genesis path that changes a PolicyRecord):
permissionless execution from a Settled transition whose type is the
canonical `POLICY_TRANSITION_TYPE` (SHA-256 of `treasury.policy.v1`),
whose declared hash equals the SHA-256 payload hash of the supplied
new values, and whose settle was exact (realized == declared); a
one-time execution flag on the TransitionRecord (SPACE 800 → 801)
refuses replay, so an old amendment cannot roll back a newer one.
IDL: instructions 24 → 25 (incl. `apply_policy_amendment`), accounts
10 (no new account type), errors 35 → 36 (`AmendmentMismatch`).
Program host tests 53 → 55 (the payload-hash binding matrix and the
amendment gate matrix incl. replay-refused and inexact-settle
refused). The §4.1 lifecycle instruction set is now complete in
code — governance inputs, lifecycle, and post-genesis execution.
Next: §4.2 treasury.
**This increment adds `initialize_treasury` (design §4.2)** — the
first §4.2 instruction — in the transition-executed form the
design row requires, not a genesis-direct multisig call (§10's
no-single-key-authority rule: the treasury's birth state must
itself be a Settled transition's execution). Permissionless
executor pays rent for the three new accounts: `TreasuryState`
(PDA `["treasury"]`, SPACE 57: total, 50/25/25 sleeves, retained
float, threshold B), `LendingBook` (PDA `["lending-book"]`, SPACE
21: outstanding + active-loan count), `FeeVault` (PDA
`["fee-vault"]`, SPACE 17: collected total). All values start at
zero and threshold B starts unset (D-2) — the launch gate cannot
commit until a governed path sets it, which is the intended state.
The gate is Settled + canonical `TREASURY_INIT_TYPE` (SHA-256 of
`treasury.init.v1`) + the FeeVault key equaling `Config.fee_vault`
(a genesis obligation: the genesis authority must pass the
`["fee-vault"]` PDA at `initialize_config`, or init refuses with
`VaultMismatch` rather than running with two "vaults"). No
exact-settle requirement (the init's effect is parameter-free, so
no declared payload exists to diverge from) and no execution flag
(the `init` constraints are the one-time guarantee). The stage
flag stays on Config only; the three capacity stocks stay
`set_capacity`'s job and are not touched here. IDL: instructions
25 → 26 (incl. `initialize_treasury`), accounts 10 → 13
(`TreasuryState`, `LendingBook`, `FeeVault`), errors 36 → 38
(`InitMismatch`, `VaultMismatch`). Program host tests 55 → 59
(type id recomputed from the string, the init gate matrix, the
sleeve-split sum identity, the three SPACE checks). Next, in
design order: `launch_stage2` / `open_window`.

**Design:** `files/OWN_IDL_DESIGN.md` (W4.1, validated and revised
2026-10-24; W4.3 GO given by Carlos 2026-10-24 ~03:39 CST). Placement C: the
kernel crate compiled into the program, the full 13-bit `commit_fail_mask`
computed on-chain, NO digest publisher.

**Scope (§8, committed-inventory ONLY):** the Token-2022 transfer-hook
extension is NOT W4.3 scope (future phase, its own Carlos decision + legal
review). Planning-window durations are a genesis parameter (engine-sim-proven
policy values carried as named config constants — no ruling invented). Loans
are 60-day only at pilot (§4.2).

**No deployment in W4.3.** Deployment, the swarm against the real program,
and tracer dogfood are W4.4, separately sequenced. No mainnet step exists
in W4. The program id in `declare_id!` is a scaffold placeholder for local
build / IDL generation only; its keypair was never committed. Known
scaffold item: `anchor build` generated a `target/deploy/` keypair whose
pubkey does NOT match this `declare_id!` (build output, not committed) —
do not deploy from it. W4.4 generates a fresh deployment keypair and
updates `declare_id!` + `Anchor.toml` in one pass, as one change.

**Canonical IDL:** generated by `anchor build` — hand-written IDLs drift, the
program is the source of truth. The scaffold's golden tests must emit
measured CU per instruction, and any §2.2 design estimate missing by more
than 2× is a design finding — reported, never tuned away silently. CU
measurement lands with the instruction families (local-validator harness);
this skeleton's tests are pure-constant checks only.

**Toolchain:** Anchor 0.31.0, Solana 2.2.20 (platform-tools v1.48 /
rustc 1.84.1; 2.1.21 kept at `~/workspace/toolchains/solana-2.1.21/` for
rollback), rustc 1.99.1 host (see `~/TOOLS.md`).
The SBF pin set (incl. the vendored `patches/cpufeatures`) is the proven
W3.3 mock set. This project is self-contained: the patch path is relative,
so it builds without the W3.3 scratch directories.

**Build:** from this directory, with the toolchain env exported,

```sh
anchor build
```

The generated IDL lands at `target/idl/treasury_program.json` (build output,
not committed — regenerate it; never hand-edit it).

**Increment order (design order):** skeleton (this) → §4.1 governance
(kernel vendoring + lifecycle instructions + launch policy table) → §4.2
treasury (pinned window protocol) → §4.3 distribution → W3.2 adversarial
suite re-expressed against the program's interface (local validator).

**This increment adds `record_evidence` (design §4.1):** the
EvidenceAttestor's write handle at the program layer — an active Oracle
records the realized hash for a Committed transition into a one-time
`EvidenceRecord` account (PDA seeds `["evidence", transition_id]`,
SPACE 49). The program cannot verify that the realized outcome exists
in the world; recording early records a hash that settle will refuse to
match. Settle (next) checks the presented hash against this record
before it ever calls the kernel's settle — no record, or a mismatched
hash, refuses and leaves the transition Committed for a correct
settlement to retry. Recording evidence changes no transition state.
Program host tests 34 → 36.
**This increment adds `settle` (design §4.1)** and closes the
follow-up the `record_evidence` slice owed: `commit` now records the
new chain index on the TransitionRecord as `commit_index`, and settle
records the realized hash there too (SPACE 448 → 488), so settle's
event can name the commit it settles. Settle mirrors the kernel's
`settle` in check order — attestor gate first (active Oracle role AND
the presented hash equal to the independently recorded EvidenceRecord),
then the Committed state gate, then attribution rules: every attributed
agent must be a plan participant, deltas summed per agent, capped at
±20 units, inexact settle halves positive credit only; reputation is
written to the attributed agents' AgentRecords (FlatWeights: recorded,
zero quorum influence). A refused settle changes nothing and leaves
the transition Committed for a correct settlement to retry; a settled
transition cannot re-settle. Remaining accounts are loaded and written
by hand (discriminator-classified, owner-checked): `Account::try_from`
cannot satisfy the remaining-accounts lifetime, and Anchor writes back
Context accounts on exit but not remaining accounts. No new account
type. IDL: instructions 15 → 16 (incl. `settle`), accounts 8,
errors 24 → 27 (`UnknownAgent`, `NotParticipant`,
`SettleAccountsMismatch`). Program host tests 36 → 39.
**This increment adds `raise_exception` / `resolve_exception`
(design §4.1)** and wires the EXCEPTIONS bit to live inputs:
exceptions are per-transition records on the TransitionRecord
(SPACE 488 → 768: the set, 4 + 8 × 34 bytes, plus the
`unresolved_blocking` count), raised only by an Evaluator while the
transition is Registered, Planning, or Committable, with dispositions
in the kernel's order (Informational, Review, Block, Emergency; Block
and Emergency count toward `unresolved_blocking`). Resolve mirrors
the kernel exactly: no state gate (resolving after commit cannot
rewrite the decided mask) and re-resolution is a no-op success that
never double-decrements. `commit` now reads the record's live count
instead of the hard-coded 0 the earlier slices carried as an interim.
IDL: instructions 16 → 18 (incl. both exception instructions),
accounts 8 (no new account type), errors 27 → 31
(`DuplicateException`, `UnknownException`, `InvalidDisposition`,
`TooManyExceptions`). Program host tests 39 → 41.
**This increment adds `launch_stage2` (design §4.2)** — the second
§4.2 instruction, in the same transition-executed form as the init:
a Settled `treasury.launch.v1` transition authorizes flipping the
stage flag on Config from Stage 1 to Stage 2, never a config check
or a fresh authority signature. The B gate is enforced by the launch
transition (mandatory BACKING_EVIDENCED constraint report feeding the
CONSTRAINTS bit at commit; the manifest's observation evidences the
committed fiat total), not recomputed by the handler, which has no
fiat total, supply, or price on-chain to compute from; no other
instruction checks B ad hoc. The handler's own gates are Settled +
canonical type (`TREASURY_LAUNCH_TYPE` = SHA-256 of
`treasury.launch.v1`, recomputed from the string in the host tests)
+ stage still Stage 1 + threshold B set (nonzero) — the last is the
D-2 intended state in executable form: B starts unset at init and no
governed path sets it yet, so until that path lands a launch
execution refuses with `ThresholdUnset` rather than reading an unset
threshold as "any backing passes". No exact-settle requirement (the
effect is a fixed flag flip) and no execution flag (the flip itself
is the one-time guarantee: a second launch refuses with
`AlreadyLaunched`). The executor is permissionless and pays no rent.
IDL: instructions 26 → 27 (incl. `launch_stage2`), accounts 13 (no
new account type), errors 38 → 41 (`LaunchMismatch`,
`AlreadyLaunched`, `ThresholdUnset`). Program host tests 59 → 61.
**This increment adds `open_window` (design §4.2)** — the third
§4.2 instruction, in the same transition-executed form: a Settled
`treasury.window.v1` transition authorizes opening the NEXT window
(the window opens for the next cohort, never mid-window, D-11), and
the window's own flow transitions are registered after open and ride
the window, per the pinned window protocol's order of operations.
Window ids run in sequence with no gaps or repeats, enforced by a
counter on TreasuryState (`window_count`, advanced here and nowhere
else; TreasuryState SPACE 57 → 65, init zeroes it) plus the Anchor
`init` constraint on the WindowState PDA. The handler snapshots the
period-start ledger (T, the three sleeves, the retained float, the
lending book's outstanding) into the new WindowState account and
sizes the window's exit stock fresh from the snapshot as movable
liquidity (T/6, D-10) — that fresh sizing is the protocol's stock
replenishment; the kernel capacity stocks are governed by
`set_capacity` / commit debit and are not rewritten here. The roster
hash starts zero (no roster exists at open; the writer lands with
`enroll`). No exact-settle requirement (the effect is a fixed
snapshot + counter advance). The executor is permissionless and pays
the WindowState rent (operator-paid from the 73 bps fee flow,
design §2.3). The source transition is recorded on the WindowState
with its commit index as the authorization context, and the
`WindowOpened` event carries the same context plus the exit stock.
IDL: instructions 27 → 28 (incl. `open_window`), accounts 13 → 14
(`WindowState`), errors 41 → 43 (`WindowMismatch`,
`WindowNotSequential`). Program host tests 61 → 64 (window type id
recomputed from the string, the open gate matrix, the exit-stock
identity). Next, in design order: `enroll`, then the window's
execution family (`pay_tranche` first, per the pinned protocol).
