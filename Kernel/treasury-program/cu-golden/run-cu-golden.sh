#!/usr/bin/env bash
# CU golden-test harness runner, slice 1.
# Starts a local test validator with the built treasury program, runs
# cu_golden.py against it over pure JSON-RPC (no websockets), records
# measured CU into cu-golden.json, then stops the validator and removes
# the scratch ledger. Local validator only -- no devnet, no mainnet.
#
# Usage (from this directory):
#   CU_GOLDEN_VENV=/path/to/venv ./run-cu-golden.sh
# The venv must have the `solana` package installed:
#   python3 -m venv venv && venv/bin/pip install solana
# Env overrides: PROGRAM_SO, IDL_JSON, RPC_PORT (default 8899).
set -euo pipefail
cd "$(dirname "$0")"

export PATH="$HOME/workspace/toolchains/solana/bin:$PATH"
PROGRAM_ID="FPGSeLBWEiFKdTuqDbJeKhjXeah5H4c3rqsW4Tzn3RAS"
# Repo layout: this script lives at Kernel/treasury-program/cu-golden/,
# so the built program is ../target/deploy/treasury_program.so.
PROGRAM_SO="${PROGRAM_SO:-../target/deploy/treasury_program.so}"
IDL_JSON="${IDL_JSON:-$(dirname "$PROGRAM_SO")/../idl/treasury_program.json}"
VENV="${CU_GOLDEN_VENV:-./venv}"
RPC_PORT="${RPC_PORT:-8899}"
RPC_URL="http://127.0.0.1:${RPC_PORT}"

[ -x "$VENV/bin/python" ] || { echo "venv python not found at $VENV (set CU_GOLDEN_VENV)"; echo "create it: python3 -m venv venv && venv/bin/pip install solana"; exit 1; }
[ -f "$PROGRAM_SO" ] || { echo "program .so not found: $PROGRAM_SO"; exit 1; }
[ -f "$IDL_JSON" ] || { echo "IDL not found: $IDL_JSON"; exit 1; }

# Health check helper: the RPC server requires a JSON content type;
# a bare urllib POST without it is rejected, which once masqueraded as
# a validator that never became healthy.
health_ok() {
  python3 -c "
import urllib.request, json, sys
req = urllib.request.Request('$RPC_URL',
  data=json.dumps({'jsonrpc':'2.0','id':1,'method':'getHealth'}).encode(),
  headers={'Content-Type': 'application/json'})
try:
    body = urllib.request.urlopen(req, timeout=3).read().decode()
except Exception:
    sys.exit(1)
sys.exit(0 if '\"ok\"' in body else 1)
"
}

# Refuse to measure against a foreign validator already on the port.
if health_ok; then
  echo "refusing: something already answers getHealth on $RPC_URL"
  exit 1
fi

LEDGER="./.ledger-tmp"
rm -rf "$LEDGER"
solana-test-validator --ledger "$LEDGER" --reset \
  --bind-address 127.0.0.1 \
  --bpf-program "$PROGRAM_ID" "$PROGRAM_SO" \
  --rpc-port "$RPC_PORT" > validator.log 2>&1 &
VALIDATOR_PID=$!
trap 'kill $VALIDATOR_PID 2>/dev/null || true; rm -rf "$LEDGER"' EXIT

# Wait for RPC health (pure HTTP polling, no websockets).
for _ in $(seq 1 120); do
  if health_ok; then break; fi
  sleep 1
done
health_ok || { echo "validator did not become healthy; see validator.log"; exit 1; }

SO_SHA="$(sha256sum "$PROGRAM_SO" | cut -d' ' -f1)"
export CU_GOLDEN_RPC="$RPC_URL"
# Payer is the validator's genesis faucet keypair: the sandbox faucet's
# requestAirdrop path fails with TransactionDataTooLarge, so the
# harness signs as the faucet account itself (local ledger only).
"$VENV/bin/python" cu_golden.py "$IDL_JSON" "$SO_SHA" cu-golden.json \
  "$LEDGER/faucet-keypair.json"
echo "--- golden ---"
cat cu-golden.json
