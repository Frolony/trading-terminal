# Coordination Kernel — Verified Package (W2)

**Repository home:** `Kernel/` in `Frolony/trading-terminal`. This directory is the canonical workspace for the Coordination Kernel, its conformance suite, the treasury policy profile, and the engine simulator as those workstreams land.

**Author of the kernel: Jeffrey Zoltan Beitel.** The kernel crate in this package is his work, included **byte-identical** to the `coordination-kernel-v0.2-2.zip` he provided (crate version 0.2.0, the "hooks" revision). Everything else in this package is additive work by Carlos's agent (Cheesehead), built *around* the kernel without modifying it — the kernel's own design intends exactly this: its administrator hooks are the seam, and this package is what plugs into them.

## Contents

| Path | What it is |
|---|---|
| `kernel/` | The kernel crate, pristine. Verify: every source file compares byte-identical against the archived original (`src/kernel.rs`, `src/lib.rs`, `src/num.rs`, `tests/kernel_tests.rs`, `reference/CoordinationKernel.sol`, `Cargo.toml`, `README.md`). |
| `kernel-conformance/` | **W2.3** — our conformance suite: 7 property tests of *our* requirements against the crate (commit ⟺ mask-clear under randomized defect injection; capacity conservation across chained commits; settle discipline; monotonic time; attribution vs an independent re-implementation, permutation-invariant; lifecycle legality under a 2,000-op randomized fuzz × 3 seeds; lineage chain integrity). Deterministic xorshift RNG, zero external dependencies. |
| `kernel-policy/` | **W2.4** — the treasury deployment's policy profile: an evidence-bound `Attestor` (settlement admitted only when the claimed realized hash matches independently recorded evidence — settle-after-realized, fail-closed), flat launch weights (reputation recorded, zero influence), named capacity stocks, and `launch_kernel()` constructor. Its tests are the executable spec of the profile. |
| `KERNEL_VERIFICATION_REPORT.md` | W2.2 + W2.3/W2.4 results: every claim, executed, with verdicts. |
| `KERNEL_POLICY_PROFILE.md` | The W2.4 profile in prose, incl. the W2.5 contention recommendation (open — awaiting the author's ruling). |

## Run it

```sh
cargo test --workspace            # 55 tests: 42 kernel + 7 conformance + 6 policy
cargo clippy --workspace --all-targets -- -D warnings   # clean
cargo build -p coordination-kernel --no-default-features  # no_std + alloc
```

Verified 2026-10-22 with rustc/cargo/clippy 1.99.1. Toolchain versions and full outputs: `KERNEL_VERIFICATION_REPORT.md`.

## The one-sentence state of things

The kernel does everything its author reported (42/42 tests, clippy-clean, no_std, Solidity bit-parity — all reproduced), it additionally satisfies every requirement *we* wrote down for the treasury deployment, and the one design question that remains open — contention semantics when capacity claims exceed stock — is documented in the policy profile as an open item for the author, deliberately unimplemented rather than silently decided.
