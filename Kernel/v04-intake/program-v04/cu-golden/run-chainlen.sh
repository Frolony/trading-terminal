#!/usr/bin/env bash
# V0.4 program chain-length CU fixture (mollusk path), merge-gate item 5 CU leg.
# Drives 0/1/2/3/5-prior chains in-process against the already-built
# program .so (build it first: `anchor build` from program-v04/) and
# writes ../chainlen-cu.json. No validator, no network, no real funds.
# The keys in the harness are throwaway local-test keys.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
export SBF_OUT_DIR="${SBF_OUT_DIR:-$HERE/../target/deploy}"
export RUSTUP_HOME="${RUSTUP_HOME:-$HOME/workspace/toolchains/rustup}"
export CARGO_HOME="${CARGO_HOME:-$HOME/workspace/toolchains/cargo}"
# v04-line toolchain (Carlos's 2026-10-29 migration ruling): Agave 4.3.2 /
# Anchor 1.2.2 / mollusk 0.16.4.
export PATH="$CARGO_HOME/bin:$HOME/workspace/toolchains/solana-4.3.2-scratch/active_release/bin:$PATH"
cargo run --locked --offline --release --manifest-path "$HERE/chainlen/Cargo.toml" -- "$HERE/chainlen-cu.json"
