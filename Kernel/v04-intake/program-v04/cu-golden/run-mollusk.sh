#!/usr/bin/env bash
# V0.4 program CU golden-test harness (mollusk path), slice 1.
# Executes the slice-1 chain (genesis + governance registration)
# against the already-built program .so (build it first:
# `anchor build` from program-v04/) and writes
# ../cu-golden-mollusk.json. No validator, no network, no real funds.
# approve_governance / revise_governance_approval are recorded as
# BLOCKED in the golden file (slice-49 finding), not executed.
# The keys in the harness are throwaway local-test keys.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
export SBF_OUT_DIR="${SBF_OUT_DIR:-$HERE/../target/deploy}"
export RUSTUP_HOME="${RUSTUP_HOME:-$HOME/workspace/toolchains/rustup}"
export CARGO_HOME="${CARGO_HOME:-$HOME/workspace/toolchains/cargo}"
export PATH="$CARGO_HOME/bin:$HOME/workspace/toolchains/solana/bin:$PATH"
cargo run --locked --offline --release --manifest-path "$HERE/mollusk/Cargo.toml" -- "$HERE/cu-golden-mollusk.json"
