# engine-adapter-v04 — migration step 4, increment 1

The engine-sim governance adapter rewritten over Jeffrey Beitel's v0.4
kernel lifecycle. It consumes the delivered v0.4 crate and the step-2
launch profile by path; it modifies neither, and it never touches
`Kernel/` in the repo. All v0.4 work stays in `hidden_files/v04-intake/`
until Carlos rules the notes' branching question.

## Run

```sh
export RUSTUP_HOME=~/workspace/toolchains/rustup CARGO_HOME=~/workspace/toolchains/cargo
export PATH=~/workspace/toolchains/cargo/bin:$PATH
cargo test --offline        # from this directory
cargo clippy --offline --all-targets -- -D warnings
```

Baseline (increment 1): **7/7 tests, clippy clean.**

## What it builds

- `EngineAdapter::authorize(resource, amount)` — one engine flow through
  the full lifecycle (register → evaluate → eligible → signals → select →
  planning → plan → reconcile → validate → declare → commit →
  start_execution → gated attestation → record_outcome). Returns `Some`
  iff the flow committed and settled; `None` iff the predicate refused
  (blocked, no money moves); zero amounts are no-ops that never reach
  the kernel. Per-flow observations are ingested at the drive's base
  time, so the schedule never depends on the genesis observation's
  expiry; one monotonic base per drive.
- `EngineAdapter::carry_pending()` — K-1 ruling 1 as code: a blocked
  claim is withdrawn and re-registered against the current head under a
  new id, re-earning consensus through the full drive. A claim that
  blocks again is re-recorded whole; nothing is partially debited
  (ruling 5), plans are never mutated (ruling 4: a smaller claim is a
  new transition), and carried claims compete under the existing
  selection rule (ruling 3: the carry drive is the same drive as any
  new flow, and re-registration resets eligibility time).
- Blocked-slot hygiene: a Blocked transition holds its parent's
  `selected_for` slot, so a blocked flow is withdrawn as soon as it is
  recorded as pending — otherwise later same-parent flows could never
  select. The claim survives in `pending()` for an explicit carry.
- `EngineAdapter::set_capacity_via_governance(resource, capacity)` — the
  v0.4 form of the v0.2 per-flow `set_capacity`: a unanimous-council
  governance transition through the full lifecycle. A driven governance
  act, not a shortcut around governance.
- Evidence-bound settlement via the step-2 `SettlerGate`: the realized
  hash is recorded first, then both Settlers attest through the gate.

## What it does not yet do

- The engine itself is not ported here, so T1–T9 are not yet re-run
  against this adapter — that is the next step-4 increment.
- F1 (no `reputation_weighs` switch in the delivered build) and F2 (the
  commit clone cost, a step-5 on-chain item) are carried forward
  unchanged. Ruling 2 (council discard of a Blocked transition) is
  author-side work, pending Jeffrey's build; when it lands it is a
  second exit for a blocked claim, not a change to this carry path.
- Stocks in the tests (tranche 1000, lending 500, exit 250) are the
  step-2 launch-profile fixtures — staging numbers, not pilot
  parameters. Stock sizing lives in the treasury mathematics documents
  and is set by governance, never by this crate.
