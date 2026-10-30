#!/usr/bin/env bash
# 75% heap tripwire test — program-v04 (slice 67, 2026-10-30).
#
# Builds the program with the `tripwire-heap` feature (allocator capped
# at 196,608 bytes = 75% of the shipped 256 KiB cap, see lib.rs
# PROGRAM_HEAP_LENGTH) and runs the FULL cu-golden chain against that
# build. Success of every instruction under the capped allocator is
# the SBF-side, wrapper-inclusive tripwire proof: Anchor wrapper
# deserialize, handler replay, and account serialize all allocate
# through the same bump allocator, so no host-side wrapper allowance
# is needed. An OOM on any instruction trips the guard — the harness
# fails on that instruction and names the stage.
#
# The shipped .so is backed up before the tripwire build and restored
# afterwards (trap-guarded); the script verifies the restored .so's
# sha256 equals the backup's and fails otherwise, so a tripwire run
# can never silently leave the capped build in target/deploy.
#
# Usage: TRIPWIRE_OUT=<path for the tripwire JSON> ./run-tripwire.sh
#   TRIPWIRE_OUT defaults to ./cu-golden-tripwire.json. For run
#   evidence that should NOT be pushed, pass a path outside this
#   tree (e.g. hidden_files/tripwire-golden-<date>.json).
# No validator, no network, no mainnet, no real funds. The keys in
# the harness are throwaway local-test keys.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
PROG_DIR="$HERE/.."
OUT="${TRIPWIRE_OUT:-$HERE/cu-golden-tripwire.json}"
export RUSTUP_HOME="${RUSTUP_HOME:-$HOME/workspace/toolchains/rustup}"
export CARGO_HOME="${CARGO_HOME:-$HOME/workspace/toolchains/cargo}"
# v04-line toolchain (Carlos's 2026-10-29 migration ruling): Agave
# 4.3.2 / Anchor 1.2.2 / mollusk 0.16.4, as in run-mollusk.sh.
export PATH="$CARGO_HOME/bin:$HOME/workspace/toolchains/solana-4.3.2-scratch/active_release/bin:$PATH"
ANCHOR="$HOME/workspace/toolchains/anchor-122-scratch/bin/anchor"

DEPLOY_SO="$PROG_DIR/target/deploy/program_v04.so"
BACKUP="$(mktemp -d)/program_v04.shipped.so"
cp "$DEPLOY_SO" "$BACKUP"
SHIPPED_SHA="$(sha256sum "$BACKUP" | cut -d' ' -f1)"
echo "tripwire: shipped .so backed up (sha $SHIPPED_SHA)"

restore() {
    cp "$BACKUP" "$DEPLOY_SO"
    local restored_sha
    restored_sha="$(sha256sum "$DEPLOY_SO" | cut -d' ' -f1)"
    if [ "$restored_sha" != "$SHIPPED_SHA" ]; then
        echo "tripwire: FATAL — restored .so sha $restored_sha != shipped $SHIPPED_SHA" >&2
        exit 1
    fi
    echo "tripwire: shipped .so restored (sha $restored_sha)"
}
trap restore EXIT

cd "$PROG_DIR"
"$ANCHOR" build -- --features tripwire-heap > "$HERE/tripwire-build.log" 2>&1
TRIPWIRE_DIR="$(mktemp -d)"
cp "$DEPLOY_SO" "$TRIPWIRE_DIR/program_v04.so"
TRIPWIRE_SHA="$(sha256sum "$TRIPWIRE_DIR/program_v04.so" | cut -d' ' -f1)"
echo "tripwire: capped .so built (sha $TRIPWIRE_SHA)"

SBF_OUT_DIR="$TRIPWIRE_DIR" cargo run --locked --offline --release \
    --manifest-path "$HERE/mollusk/Cargo.toml" -- "$OUT" 2>&1 | tee "$HERE/tripwire-run.log"
echo "tripwire: chain complete, result JSON at $OUT"
