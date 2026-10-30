#!/usr/bin/env bash
# Deterministic too-small-frame failure test — program-v04 (slice 68, 2026-10-30).
#
# F6 condition (3): a too-small heap frame must fail DETERMINISTICALLY —
# the same instruction, the same error, the same CU, on every run — never
# a silent success and never a corrupted state. Two complementary modes:
#
#   cap   (default): build the program with the `small-heap` feature
#         (allocator capped at 32,768 bytes, the old default frame size)
#         and run the FULL cu-golden chain against it twice. Early
#         instructions whose cumulative heap use fits 32 KiB succeed
#         exactly as on the shipped build; the chain then fails with a
#         clean allocation error at the first stage that outgrows the
#         cap. This mode proves the failure point is a clean OOM at a
#         known stage.
#   frame: run the SHIPPED binary (no rebuild) under a 32 KiB requested
#         frame (CU_HEAP_SIZE=32768) twice. The top-down bump allocator's
#         first allocation lands outside the mapped region and faults.
#         This mode proves the shipped binary faults rather than
#         degrading when a client forgets request_heap_frame(256 * 1024).
#
# Each mode runs the chain TWICE and compares the failure signature
# (the harness's last `program_result=Err` line: label + error + CU).
# The script PASSES only if both runs fail (exit non-zero), the two
# signatures are byte-identical, and no golden JSON was written (the
# harness panics before its final write, so a written file would mean
# the chain unexpectedly succeeded). A success on either run FAILS
# the test — a too-small frame that succeeds is the finding, not a pass.
#
# The shipped .so is backed up before a cap build and restored
# afterwards (trap-guarded, sha-verified), exactly as in run-tripwire.sh,
# so a smallframe run can never silently leave the capped build in
# target/deploy. Frame mode builds nothing and restores nothing.
#
# Usage: SMALLFRAME_EVIDENCE_DIR=<dir for run logs> ./run-smallframe.sh [cap|frame]
#   Evidence defaults to ./ (the cu-golden directory). For run evidence
#   that should NOT be pushed, pass a directory outside this tree
#   (e.g. hidden_files/).
# No validator, no network, no mainnet, no real funds. The keys in
# the harness are throwaway local-test keys.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
PROG_DIR="$HERE/.."
MODE="${1:-cap}"
EVIDENCE_DIR="${SMALLFRAME_EVIDENCE_DIR:-$HERE}"
mkdir -p "$EVIDENCE_DIR"
export RUSTUP_HOME="${RUSTUP_HOME:-$HOME/workspace/toolchains/rustup}"
export CARGO_HOME="${CARGO_HOME:-$HOME/workspace/toolchains/cargo}"
# v04-line toolchain (Carlos's 2026-10-29 migration ruling): Agave
# 4.3.2 / Anchor 1.2.2 / mollusk 0.16.4, as in run-mollusk.sh.
export PATH="$CARGO_HOME/bin:$HOME/workspace/toolchains/solana-4.3.2-scratch/active_release/bin:$PATH"
ANCHOR="$HOME/workspace/toolchains/anchor-122-scratch/bin/anchor"

DEPLOY_SO="$PROG_DIR/target/deploy/program_v04.so"
SO_DIR="$PROG_DIR/target/deploy"

if [ "$MODE" = "cap" ]; then
    BACKUP="$(mktemp -d)/program_v04.shipped.so"
    cp "$DEPLOY_SO" "$BACKUP"
    SHIPPED_SHA="$(sha256sum "$BACKUP" | cut -d' ' -f1)"
    echo "smallframe(cap): shipped .so backed up (sha $SHIPPED_SHA)"
    restore() {
        cp "$BACKUP" "$DEPLOY_SO"
        local restored_sha
        restored_sha="$(sha256sum "$DEPLOY_SO" | cut -d' ' -f1)"
        if [ "$restored_sha" != "$SHIPPED_SHA" ]; then
            echo "smallframe(cap): FATAL — restored .so sha $restored_sha != shipped $SHIPPED_SHA" >&2
            exit 1
        fi
        echo "smallframe(cap): shipped .so restored (sha $restored_sha)"
    }
    trap restore EXIT
    cd "$PROG_DIR"
    # Observed 2026-10-30 (slice 68): a second consecutive variant build
    # with only the feature flag changed reused the shipped artifact
    # (capped .so sha == shipped sha, chain then "passed" against the
    # shipped binary) — cargo's fingerprint skipped the rebuild. Touch
    # the program source so the variant build always recompiles, and
    # assert below that the built .so actually differs from shipped:
    # a test that silently runs against the wrong binary is worse than
    # no test, because it reports a pass it never earned.
    touch "$PROG_DIR/programs/program-v04/src/lib.rs"
    "$ANCHOR" build -- --features small-heap > "$EVIDENCE_DIR/smallframe-cap-build.log" 2>&1
    SMALL_DIR="$(mktemp -d)"
    cp "$DEPLOY_SO" "$SMALL_DIR/program_v04.so"
    CAPPED_SHA="$(sha256sum "$SMALL_DIR/program_v04.so" | cut -d' ' -f1)"
    echo "smallframe(cap): capped .so built (sha $CAPPED_SHA)"
    if [ "$CAPPED_SHA" = "$SHIPPED_SHA" ]; then
        echo "smallframe(cap): FAIL — variant build reproduced the shipped .so; the small-heap feature did not take effect, refusing to run the chain against the shipped binary" >&2
        exit 1
    fi
    SO_DIR="$SMALL_DIR"
elif [ "$MODE" = "frame" ]; then
    echo "smallframe(frame): shipped .so used as-is (sha $(sha256sum "$DEPLOY_SO" | cut -d' ' -f1))"
else
    echo "smallframe: unknown mode '$MODE' (want cap or frame)" >&2
    exit 2
fi

run_once() {
    local n="$1" log="$2" out="$3"
    local rc=0
    # Stale-output guard (fixed 2026-10-30): a previous run's out file
    # left in a reused evidence dir must not count as "written by this
    # run". Remove it first, so the post-run existence check below
    # proves this run wrote it — without this, the 2026-10-30 cap
    # re-run aborted on a slice-68-era out file even though the run
    # itself failed cleanly with an allocation error.
    rm -f "$out"
    if [ "$MODE" = "frame" ]; then
        CU_HEAP_SIZE=32768 SBF_OUT_DIR="$SO_DIR" cargo run --locked --offline --release \
            --manifest-path "$HERE/mollusk/Cargo.toml" -- "$out" > "$log" 2>&1 || rc=$?
    else
        SBF_OUT_DIR="$SO_DIR" cargo run --locked --offline --release \
            --manifest-path "$HERE/mollusk/Cargo.toml" -- "$out" > "$log" 2>&1 || rc=$?
    fi
    echo "smallframe($MODE): run $n exit code $rc (log $log)"
    if [ "$rc" -eq 0 ]; then
        echo "smallframe($MODE): FAIL — run $n SUCCEEDED under a too-small frame; a success here is the finding, not a pass" >&2
        exit 1
    fi
    if [ -e "$out" ]; then
        echo "smallframe($MODE): FAIL — run $n wrote a golden file despite failing; the harness must panic before its final write" >&2
        exit 1
    fi
}

sig_of() {
    # Last harness line reporting a non-Success result: label + error + CU.
    # (Mollusk prints `program_result=Success` on success and
    # `program_result=UnknownError(ProgramFailedToComplete)` on an SBF
    # panic such as an allocation failure — there is no `Err(...)` form.)
    grep ": program_result=" "$1" | grep -v ": program_result=Success" | tail -n 1
}

run_once 1 "$EVIDENCE_DIR/smallframe-$MODE-run1.log" "$EVIDENCE_DIR/smallframe-$MODE-out1.json"
run_once 2 "$EVIDENCE_DIR/smallframe-$MODE-run2.log" "$EVIDENCE_DIR/smallframe-$MODE-out2.json"

SIG1="$(sig_of "$EVIDENCE_DIR/smallframe-$MODE-run1.log")"
SIG2="$(sig_of "$EVIDENCE_DIR/smallframe-$MODE-run2.log")"
echo "smallframe($MODE): run 1 failure signature: $SIG1"
echo "smallframe($MODE): run 2 failure signature: $SIG2"
if [ -z "$SIG1" ] || [ "$SIG1" != "$SIG2" ]; then
    echo "smallframe($MODE): FAIL — failure signatures differ or are empty; the failure is NOT deterministic" >&2
    exit 1
fi
if [ "$MODE" = "cap" ]; then
    # Cap mode must fail by allocation exhaustion, not by any other panic.
    grep -q "memory allocation failed" "$EVIDENCE_DIR/smallframe-$MODE-run1.log" || { echo "smallframe(cap): FAIL — run 1 failure is not an allocation failure" >&2; exit 1; }
    grep -q "memory allocation failed" "$EVIDENCE_DIR/smallframe-$MODE-run2.log" || { echo "smallframe(cap): FAIL — run 2 failure is not an allocation failure" >&2; exit 1; }
fi
echo "smallframe($MODE): PASS — deterministic failure, identical signature on both runs"
