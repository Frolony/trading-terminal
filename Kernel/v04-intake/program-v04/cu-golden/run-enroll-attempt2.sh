#!/usr/bin/env bash
# Attempt-2 enrollment scenario: proves the enrollment slice's
# five properties against the SBF binary (see
# mollusk/src/bin/enroll_attempt2.rs). Writes its own report file;
# never touches the golden measurement files.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
export SBF_OUT_DIR="${SBF_OUT_DIR:-$HERE/../target/deploy}"
export RUSTUP_HOME="${RUSTUP_HOME:-$HOME/workspace/toolchains/rustup}"
export CARGO_HOME="${CARGO_HOME:-$HOME/workspace/toolchains/cargo}"
export PATH="$CARGO_HOME/bin:$HOME/workspace/toolchains/solana-4.3.2-scratch/active_release/bin:$PATH"
cargo run --locked --offline --release --manifest-path "$HERE/mollusk/Cargo.toml" --bin enroll_attempt2 -- "$HERE/cu-golden-enroll-attempt2.json"
