# kernel-launch-v04 — migration step 2: the launch profile as configuration

A standalone crate (not a workspace member of the delivered crate) that builds
the treasury launch profile (W2.4) as configuration over Jeffrey Beitel's
v0.4 kernel crate, by path (`../coordination-kernel-rs`). It never edits the
v0.4 crate's source, and it never touches `Kernel/` in the repo: all v0.4
work stays in `hidden_files/v04-intake/` until Carlos rules the notes'
branching question (branch vs replace outright once step 1 passes).

## Run

```sh
export RUSTUP_HOME=~/workspace/toolchains/rustup CARGO_HOME=~/workspace/toolchains/cargo
export PATH=~/workspace/toolchains/cargo/bin:$PATH
cargo test --offline        # from this directory
cargo clippy --offline --all-targets -- -D warnings
```

Baseline (step 2 built, profile flipped pin-OFF 2026-10-26): **11/11 tests,
clippy clean.** The delivered crate's own suite stays independently green at
**39/39** in `../coordination-kernel-rs` (v0.4.1 delivery), and the step-1
port stays **19/19** in `../kernel-conformance-v04`.

## What step 2 builds

- **Genesis seeding** (`sealed_launch_kernel`): policies for the ordinary
  type and all four governance types, required constraints/objectives, six
  agents at base influence 2.0 (each exactly 1/6 of total base, so the
  seal's share check passes), the operator roles (Observer, Executor, two
  Settlers, OutcomeRecorder), and the three capacity stocks from the v0.2
  policy profile — `RES_TREASURY_TRANCHE` (0), `RES_LENDING_BOOK` (1),
  `RES_EXIT_WINDOW` (2) — then the council and the seal.
- **Seal gates**: the seal is refused until a full council, all four
  governance policies, and the share bound hold (each refusal names the
  missing piece); after the seal the genesis authority is retired
  (`GenesisIsSealed`) and rules change only through committed governance
  transitions (L9 proves a post-seal `SetCapacity` via unanimous governance).
- **Capacity stocks**: the three stocks are set and debited independently at
  commit (L4); an over-claim blocks at validation with the COMPAT bit and
  debits nothing (L5 — the fail-closed carry-over stance, K-1 still pending).
- **Evidence-bound settlement, as a composition** (`EvidenceLedger` +
  `SettlerGate`): settlers may attest only the recorded evidence hash. No
  evidence or a wrong hash is refused without consuming a kernel vote (the
  refused settler's later correct attestation still counts); the threshold
  of two distinct Settlers on the evidence hash settles exactly that hash.

## F1 closed, profile flipped pin-OFF — and the remaining author-side gap

- **F1 (reputation pin, roadmap Part B4) — CLOSED in the delivered v0.4.1
  build, profile flipped 2026-10-26**: `Config::reputation_weighs` now
  exists, and the launch profile's `config()` sets it `false` — reputation
  is recorded, influence stays at the administrator-set base. L11 proves
  the profile default is OFF (reputation 50 → 70, influence stays 2.0);
  L10 proves the explicit switch-OFF form; L8 keeps the switch-ON form
  pinned via an explicit configuration (reputation 50 → 70, influence
  1.0 → 1.4). Nothing in the author's crate was edited to achieve this:
  the flip is a configuration line in this crate only.
- **Evidence lookup**: the delivered `Env` trait has no evidence method and
  `attest_settlement` does not consult any record. L7 pins that behavior:
  attesting a non-evidence hash directly settles it. Until the author adds
  the `Env` evidence method consulted by `attest_settlement`, evidence
  binding lives in `SettlerGate` — a composition the deploying process runs
  around the kernel, not a property the kernel enforces. When the method
  lands, the gate's ledger becomes its source and the gate can retire.

## Fixture levels are not pilot parameters

The stock levels in the tests (1000 / 500 / 250) prove the stocks are set
and debited independently. Real stock sizing lives in the treasury
mathematics documents and is set by governance, never by this crate.
