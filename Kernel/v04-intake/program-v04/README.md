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

## Build

```sh
export RUSTUP_HOME=~/workspace/toolchains/rustup CARGO_HOME=~/workspace/toolchains/cargo
export PATH=~/workspace/toolchains/cargo/bin:~/workspace/toolchains/solana/bin:$PATH
~/workspace/toolchains/anchor/bin/anchor build   # from this directory
cargo test                                        # host tests, 4/4
```
