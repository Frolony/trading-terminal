#!/usr/bin/env bash
# W4.3 CU golden-test harness (mollusk path). Executes the slice-15
# chain (slices 1-14 as before, plus the last §4.2 row
# `collect_fees`: the operator collects 73,000 (73 bps on a
# 10,000,000 flow, operator-reported) in window 14 on Committed
# window flow transition 7, a further 27,000 accumulates in the
# same window (window and vault both 100,000), a zero collection
# refuses InvalidCollectAmount and an agent without OPERATOR_SETTLE
# refuses Unauthorized; gated against design §2.2 ~7K CU, 2x band
# 3,500-14,000 — the 14,223 CU success figure is a marginal design
# finding, 223 CU over the band top, recorded not tuned away)
# against the already-built program .so
# (build it first: `anchor build` from Kernel/treasury-program/) and
# writes ../cu-golden-mollusk.json. No validator, no network, no real funds.
# The keys below are throwaway local-test keys, never real secrets.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
export SBF_OUT_DIR="${SBF_OUT_DIR:-$HERE/../../target/deploy}"
export SPL_TOKEN_SO_PATH="${SPL_TOKEN_SO_PATH:-$HERE/../spl-token.so}"
# Slice-7 fixture: the real SPL Token program bytes. The GitHub
# snapshot tool accepts UTF-8 files only, so the binary fixture
# is NOT committed to the repo — it is fetched on demand from
# public devnet program data and verified by sha256 before use.
# (The dump is a read of public program bytes, not a deployment.)
if [ ! -f "$SPL_TOKEN_SO_PATH" ]; then
  echo "spl-token fixture missing at $SPL_TOKEN_SO_PATH — fetching public program bytes from devnet…"
  if ! command -v solana >/dev/null 2>&1; then
    echo "ERROR: solana CLI not found; fetch the fixture manually:" >&2
    echo "  solana program dump TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA $SPL_TOKEN_SO_PATH --url https://api.devnet.solana.com" >&2
    exit 1
  fi
  solana program dump TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA "$SPL_TOKEN_SO_PATH" --url https://api.devnet.solana.com
fi
EXPECTED_SPL_SHA="8190d3f7ceb6cb7a7a8d8924bff89f9f611e15ce1f806f2b6237f3311a98f697"
ACTUAL_SPL_SHA="$(sha256sum "$SPL_TOKEN_SO_PATH" | cut -d' ' -f1)"
if [ "$ACTUAL_SPL_SHA" != "$EXPECTED_SPL_SHA" ]; then
  echo "ERROR: spl-token fixture sha mismatch (got $ACTUAL_SPL_SHA, want $EXPECTED_SPL_SHA) — refusing to measure against unknown token bytes." >&2
  rm -f "$SPL_TOKEN_SO_PATH"
  exit 1
fi
cargo run --release --manifest-path "$HERE/Cargo.toml" -- "$HERE/../cu-golden-mollusk.json"
