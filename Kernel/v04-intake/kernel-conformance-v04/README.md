# kernel-conformance-v04 — W2.3 conformance port to kernel v0.4 (migration step 1, slice 1)

A standalone crate (not a workspace member of the delivered crate) that depends
on the delivered v0.4 crate by path (`../coordination-kernel-rs`) and tests it
against OUR requirements. It never edits the v0.4 crate's source, and it never
touches `Kernel/` in the repo: all v0.4 work stays in `hidden_files/v04-intake/`
until Carlos rules the notes' branching question (branch vs replace outright
once step 1 passes).

## Run

```sh
export RUSTUP_HOME=~/workspace/toolchains/rustup CARGO_HOME=~/workspace/toolchains/cargo
export PATH=~/workspace/toolchains/cargo/bin:$PATH
cargo test --offline        # from this directory
cargo clippy --offline --all-targets -- -D warnings
```

Baseline (step 1 complete): **18/18 tests, clippy clean.** The delivered crate's
own suite stays independently green at **33/33** (2 classifier + 25 kernel + 6
parity) in `../coordination-kernel-rs`.

## What is ported

The seven v0.2 properties, adapted to the v0.4 lifecycle (which front-loads
several predicate terms into earlier gates, so each term is asserted at its
natural gate rather than forced to the commit stage):

- P1 — nothing commits unless the full predicate is clear; refusals quote
  exactly the predicate's mask and leave state/capacity untouched
  (declare-stage and commit-stage variants, 240 randomized defect iterations)
- P2 — capacity is a conserved stock (chained commits + over-claim gate)
- P3 — settle discipline: Executor start, threshold-2 Settler attestation,
  OutcomeRecorder outcome, exactly once
- P4 — monotonic time at every mutating entry point
- P5 — attribution vs an independent hand computation + permutation invariance
- P6 — lifecycle edges under a 3-seed × 2000-step randomized fuzz
- P7 — chain advance by committed manifests only; stale-parent LINEAGE

Plus the migration's new properties, all seven:

- G1 — governance unanimity binds the exact (id, action) pair; a committed
  governance transition applies exactly its action
- G2 — the 1/6 influence cap at seal, and tightening under an existing
  holder is refused
- G3 — the position gate never weights a signal or a commit
- G4 — defection cooldown: a DefectionConfirmed resolution blocks candidacy
  through epochs 2–4 (defection in epoch 2) and candidacy returns in epoch 5
- G5 — failed governance commits leave no trace (capacity, declared state,
  history, events unchanged; repeatable failure; proposer still Committable)
- G6 — stale dialect: a competing change's commit moves the head, the
  dependent transition's mask then carries DIALECT, and it can never commit
- G7 — post-seal registration cap: a 3.0 newcomer (1/5 of base) is refused
  at commit with no trace; a 2.0 newcomer (1/7) applies exactly

## F1 note (roadmap Part B4)

`Config::reputation_weighs` does NOT exist in the delivered v0.4 build —
influence follows reputation via `influence_at()`. The fixture therefore pins
influence arithmetically: agents register at the reputation midpoint (50 of
0..100) with base influence 2.0, so `influence_at()` is exactly 1.0 and signal
weights equal raw signal weights. The launch profile's reputation pin remains
author-side work (request the switch or a newer build via the roadmap).

## Step 1 complete — and one finding about the delivered build

Migration step 1 is done: the seven v0.2 properties plus the seven new
properties above all pass against the delivered v0.4 crate as-is. Step 1's
gate for the branching question (branch vs replace outright) is therefore
met from the conformance side; the branching decision itself remains
Carlos's, and no v0.4 code has entered the repo.

Finding (G6): the notes' "stale dialect between declare and commit" sequence
cannot occur in the delivered build — only one transition per parent may be
selected at a time, so a Committable sibling blocks every other sibling's
selection. Staleness instead strands the dependent transition at or before
selection, where the predicate still catches it (DIALECT bit). The port
asserts that reachable behavior; it does not pretend the unreachable
sequence was tested. (Related: the notes' `commit_check` does not exist in
the delivered build — F2 — the mask gate at commit is the equivalent.)
