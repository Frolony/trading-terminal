#!/usr/bin/env bash
# Attempt-2 (enrollment slice) CU golden run: same mollusk harness
# as run-mollusk.sh, but writes the attempt-2 golden file alongside
# the frozen attempt-1 file (which is never overwritten).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
export SBF_OUT_DIR="${SBF_OUT_DIR:-$HERE/../target/deploy}"
export RUSTUP_HOME="${RUSTUP_HOME:-$HOME/workspace/toolchains/rustup}"
export CARGO_HOME="${CARGO_HOME:-$HOME/workspace/toolchains/cargo}"
export PATH="$CARGO_HOME/bin:$HOME/workspace/toolchains/solana-4.3.2-scratch/active_release/bin:$PATH"
# --bin is required since the enroll_attempt2 binary landed: without it
# cargo run refuses with "could not determine which binary to run"
# (found in the Phase-2 freeze pass, 2026-11-03).
cargo run --locked --offline --release --manifest-path "$HERE/mollusk/Cargo.toml" --bin cu-golden-mollusk-v04 -- "$HERE/cu-golden-mollusk-attempt2.json"
