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

- No transition replay on-chain, no accounts, no CU figures — the
  bridge crate (`program-bridge-v04`, slices 1–4) proves the replay
  shapes on the host; cu-golden measures them once instructions exist.
- No deployment: the program id in `Anchor.toml` is staging-only (the
  generating keypair was deleted after the pubkey was taken).

## Build

```sh
export RUSTUP_HOME=~/workspace/toolchains/rustup CARGO_HOME=~/workspace/toolchains/cargo
export PATH=~/workspace/toolchains/cargo/bin:~/workspace/toolchains/solana/bin:$PATH
~/workspace/toolchains/anchor/bin/anchor build   # from this directory
cargo test                                        # host tests, 4/4
```
