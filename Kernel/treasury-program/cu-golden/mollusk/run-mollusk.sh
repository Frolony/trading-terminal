#!/usr/bin/env bash
# W4.3 CU golden-test harness (mollusk path). Executes the slice-2
# governance chain (slice-1 genesis/admin + policy/observation/
# transition/plan/signal/report/reconcile/commit) against the
# already-built program .so (build it first: `anchor build` from
# Kernel/treasury-program/) and writes ../cu-golden-mollusk.json. No validator, no network, no real funds.
# The keys below are throwaway local-test keys, never real secrets.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
export SBF_OUT_DIR="${SBF_OUT_DIR:-$HERE/../../target/deploy}"
cargo run --release --manifest-path "$HERE/Cargo.toml" -- "$HERE/../cu-golden-mollusk.json"
