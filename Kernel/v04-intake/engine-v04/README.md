# engine-v04 — migration step 4, increments 2–3

The v0.2 engine-sim aggregate + per-holder treasury engine, ported onto
the v0.4 kernel lifecycle through `engine-adapter-v04`. It consumes the
delivered v0.4 crate and the step-2 launch profile by path; it modifies
neither, and it never touches `Kernel/` in the repo. All v0.4 work stays
in `hidden_files/v04-intake/` until Carlos rules the branching question.

## Run

```sh
export RUSTUP_HOME=~/workspace/toolchains/rustup CARGO_HOME=~/workspace/toolchains/cargo
export PATH=~/workspace/toolchains/cargo/bin:$PATH
cargo test --offline        # from this directory
cargo clippy --offline --all-targets -- -D warnings
```

Baseline (increments 2–3): **9/9 theorem tests (T1–T9) + 6/6 adversarial
tests (T10–T14 + one v0.4 adapter anchor), clippy clean.**

## What is ported

- Ported verbatim from v0.2 (arithmetic, `Config`, cohorts, lending book,
  defaults, reinvestment, per-holder flooring): `src/lib.rs` (aggregate
  engine) and `src/per_holder.rs` (integer engine). The only edits are the
  governance-seam constants and a port note; no theorem value changed.
- Ported verbatim in increment 3: `src/adversarial.rs` + tests
  `tests/adversarial_v04.rs` (T10–T14 at the v0.2 values, unchanged; D-3
  priced at both bases with pre-default CLOSED and post-default retained
  as the rejected counterfactual). The attacks are pure-wealth models —
  they measure against the honest paths T7/T9 prove gated, and no attack
  flow bypasses governance. The one v0.4-new test is an adapter anchor,
  not a T-number: a $15M T13 whale tranche commits as a real governed
  flow through the step-4 adapter. The D-10 program flag (>= T/30) is
  NOT imported into T13's whale threshold (> movable liquidity).
- New, and the only v0.4-specific code: `src/governance.rs` — the v0.2
  `Governance` interface over the step-4 adapter. Before each non-zero
  flow it drives a unanimous-council `SetCapacity` governance transition
  setting the stock to `min(available, stock_cap)`, exactly the stock
  v0.2's direct admin call set; then the flow drives through the full
  v0.4 lifecycle. `settle()` is a documented no-op: the adapter already
  settled the flow (evidence-gated, against the declared hash — the
  exact-execution assumption v0.2 also carried) inside `authorize`.
- Step 4 in this crate is complete through increment 3 (engine + adversarial
  suite). Next: step 5 (program bridge) when the queue reaches it; nothing
  in this crate is pushed to the repo until Carlos rules the branching question.

## The two v0.4 differences a reader must not mistake for drift

1. **History length in gated runs is 48, not 24.** Each flow's governed
   stock refresh is itself a committed transition, so a 12-month gated
   run lands 24 flows + 24 refreshes in kernel history. The engine's
   `settled_transitions` is still 24 — it counts flows, not refreshes.
   T7/T9 assert both numbers, so the difference is pinned, not hidden.
2. **Micro-to-Amount conversion is exact in range.** Engine micro-dollars
   and kernel `Amount` share the 10^6 scale; `from_f64` rounds back to the
   same raw value for every amount here (<= 10^14 raw). T1–T9 assert the
   v0.2 values exactly, so a conversion drift would fail a test rather
   than be absorbed.

## Carried forward unchanged

- F1 (no `reputation_weighs` switch in the delivered build; influence
  follows reputation) and F2 (commit clone cost, a step-5 on-chain item).
- Ruling 2 (council discard of a Blocked transition) is author-side work,
  pending Jeffrey's build; when it lands it is a second exit for a blocked
  claim, not a change to the carry path in `engine-adapter-v04`.
- No declarative score anywhere: the engine carries no reinvestment
  score, no pro-rata scaling, no ranking beyond the kernel's own
  selection rule.
