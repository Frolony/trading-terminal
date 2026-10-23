# Kernel Verification Report — W2.2

**Executed 2026-10-22 ~21:5x CST** (W2 of `files/IMPLEMENTATION_ROADMAP.md`). Purpose: close verification debt **V-1** — every behavioral claim about the Coordination Kernel crate previously rested on the author's report and our static reading. This report replaces both with executed results.

**Environment:** rustc / cargo / clippy **1.99.1** (58cf8f9c3, 2026-10-13), installed via rustup under `~/workspace/toolchains/` (`RUSTUP_HOME=~/workspace/toolchains/rustup`, `CARGO_HOME=~/workspace/toolchains/cargo`) so the toolchain survives VM replacement. Builds ran on pristine copies of the archived packages at `~/workspace/toolchains/kernel-verify/{base,hooks}` — the archives in `hidden_files/coordination-kernel/` were not modified.

**Packages:** base = `extracted/ck_rs` (from `coordination-kernel-v0.2.zip`; note the crate's internal version is **0.1.0** — the "v0.2" names the design iteration), hooks = `v2-hooks/ck_rs` (from `coordination-kernel-v0.2-2.zip`; crate version **0.2.0**).

## Results — every author claim reproduced

| Claim | Base (0.1.0) | Hooks (0.2.0) |
|---|---|---|
| Test suite passes | ✅ **36 passed, 0 failed** (`cargo test`) | ✅ **42 passed, 0 failed** (`cargo test`) |
| Clippy clean | ✅ zero warnings — verified at the strict setting (`cargo clippy --all-targets -- -D warnings`) | ✅ zero warnings, same strict setting |
| no_std build works | ✅ `cargo build --no-default-features` (no_std + alloc) compiles | ✅ same |
| Zero dependencies | ✅ `Cargo.toml` dependency section empty; builds with no network fetches | ✅ same |
| Solidity bit-parity | n/a (twin not in this package) | ✅ `bit_parity_with_solidity_source` — **ok** (parses `reference/CoordinationKernel.sol` and checks the F_* constants against the Rust predicate) |
| Hooks behave as documented | n/a | ✅ included in the 42: `weight_policy_changes_quorum_outcome` (a `WeightPolicy` override flips a quorum result through `influence()`), `reject_is_reachable_and_terminal` (the `reject()` veto reaches the terminal Rejected state), `only_proposer_or_admin_starts_planning` (proposer-gated `open_planning`) |

Full test inventory (hooks package, all ok): predicate/feasibility (empty constraints infeasible, no-plans incompatible, bare transition cannot commit, planning-open blocks early commit), quorum/claims (negative claims unrepresentable), attribution (order-independent, capped, unknown agent errors, drift halves only credit and never softens a penalty), declared/realized separation, observation binding/freshness/confidence, monotonic time (cannot regress), capacity consumption, role enforcement, stale-parent rejection after the chain moves, bounds unforgeable, plus the three hooks tests and the parity test above. The base package runs the same suite minus the hooks/parity/reject tests (36).

## Verdict

**V-1 CLOSED — reproduced in full.** The crate does what its author reported, on a current toolchain, under stricter-than-claimed lint settings. The static analysis in `COORDINATION_KERNEL_ANALYSIS.md` (predicate semantics, commit/settle separation, attribution rules) is now backed by executed behavior for everything the test suite covers.

**What this does NOT establish** (kept honest): the tests are the author's — they verify the crate against its own specification, not against *our* requirements. Our-requirements verification is W2.3 (additive property tests in a dependent crate: mask-zero ⟺ commit, capacity never over-committed under adversarial sequences, settle-only-after-commit, attestation-after-realized policy seam). Nothing here tests the Solidity twin's *behavior* — only that its F_* constants parse-match the Rust ones. And no contention semantics exist to test yet (K-1, awaiting Jeffrey's ruling via Carlos).

---

## Addendum — W2.3 conformance + W2.4 policy profile (executed 2026-10-22 ~23:5x CST)

At Carlos's direction ("reconstitute anything, edit, implement or adjust anything as needed"), the roadmap's next two W2 steps were implemented the same evening, as **separate crates that depend on the kernel without modifying it** (the kernel directory in the package compares byte-identical against the archived original on every source file). Same toolchain (rustc 1.99.1).

**W2.3 — `kernel-conformance`: 7/7 property tests pass.** Our requirements, not the author's: (P1) commit ⟺ Committable ∧ mask == 0 across 300 randomized defect-injected scenarios (constraint states, quorum/confidence manipulation, over-capacity plans, blocking exceptions, plan/manifest expiry), with refused commits quoting exactly the predicate's mask and leaving state and capacity untouched; (P2) capacity conserved exactly across chained commits (60 scenarios), over-claims refused with the COMPAT bit; (P3) settle only after commit, only via the attestor, exactly once, with the realized hash recorded and the exact flag truthful; (P4) time regression rejected at every mutating entry point, clock unmoved by rejected calls; (P5) attribution matches an independent re-implementation of the spec math and is permutation-invariant (120 randomized cases × 5 orderings, including cap-busting magnitudes); (P6) lifecycle legality under a 2,000-operation randomized fuzz × 3 seeds — no illegal edge ever taken, terminal states absorbing; (P7) the declared chain advances only by committed manifests, in order, and a stale-parent transition carries the LINEAGE bit and can never commit.

**W2.4 — `kernel-policy`: 6/6 profile tests pass.** The evidence-bound attestor behaves as specified: settlement without recorded evidence is refused (role alone insufficient) and retryable; settlement against mismatched evidence is refused; matching evidence + Oracle role settles, with attribution applied; an honestly-reported drifted outcome settles as inexact with credit halved; observations remain role-gated; and reputation demonstrably has zero influence on quorum under the launch profile (reputation 0 and 100 produce identical quorum outcomes). The prose profile, including the open W2.5 contention recommendation (fail-closed carry-over; **not implemented — awaiting the author's ruling, item K-1**), is `KERNEL_POLICY_PROFILE.md` in the package.

**Workspace totals: 55/55 tests pass (42 kernel + 7 conformance + 6 policy); clippy clean across the workspace at `--all-targets -- -D warnings`; kernel no_std build unaffected.**
