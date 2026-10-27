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

## Build

```sh
export RUSTUP_HOME=~/workspace/toolchains/rustup CARGO_HOME=~/workspace/toolchains/cargo
export PATH=~/workspace/toolchains/cargo/bin:~/workspace/toolchains/solana/bin:$PATH
~/workspace/toolchains/anchor/bin/anchor build   # from this directory
cargo test                                        # host tests, 20/20
```
