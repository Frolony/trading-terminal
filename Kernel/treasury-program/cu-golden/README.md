# CU golden-test harness (W4.3)

**STATUS (2026-10-25): slice-1 CU MEASURED via the mollusk path below
-- the validator path remains submission-only (see "Sandbox
findings").** The validator path boots, funds, encodes, and submits,
but no transaction confirms on a local validator in this sandbox. The
mollusk path executes the same slice-1 chain in-process and records
real CU; it is now the harness of record for measured CU, and the
validator path is retained only as a fallback shape, not a measurement
source.

## Sandbox findings (2026-10-25, proven, not guessed)

- Validator boots and answers RPC: `getHealth` ok, `getVersion`
  2.2.20, `getBalance` works, slots advance in RPC context.
- `requestAirdrop` fails with `TransactionDataTooLarge(30061)` in the
  validator log -- the sandbox faucet path is unusable. Workaround
  (landed in the harness): pay as the validator's own genesis faucet
  keypair (`<ledger>/faucet-keypair.json`, 1,000,000 SOL at genesis).
- Preflight simulation mis-reports `BlockhashNotFound` for a blockhash
  that `isBlockhashValid` confirms and that the transaction itself
  carries byte-for-byte. Workaround: `skipPreflight: true`; the landed
  meta check (`meta.err is None`) stays the ground truth.
- With preflight skipped the transaction is accepted but NEVER
  confirms (60s+ wait, status stays null). A plain `solana transfer`
  via the CLI hangs the same way and the recipient balance stays 0.
  Root cause class: the sandbox denies UDP (validator log: gossip
  `SendPktsError(PermissionDenied)`), so validator-internal transaction
  ingress stalls. This is environmental, not a harness bug: the same
  code path is submission-correct.
- `anchor build` + generated IDL are unaffected; host tests 79/79.

## Next step (queue)

Mollusk-svm in-process execution (`mollusk-svm`, crates.io reachable,
0.16.2 at last check): load `target/deploy/treasury_program.so`,
execute the slice-1 instructions, read CU from the result -- no
validator, no faucet, no UDP. Compatibility of the harness crate's
dependency versions with the program's Anchor 0.31 / Solana 2.1 pins
is that run's verification item. Devnet measurement is NOT the
fallback: W4.3 forbids devnet deployment (W4.4 owns it).


Measures real compute units per instruction for the treasury program by
executing it on a local test validator and reading `computeUnitsConsumed`
from each landed transaction's meta -- not estimates, not host-test
timings. This is the harness the design's §2.2 >2x-miss gate runs on:
any §2.2 estimate that misses its measured value by more than 2x is a
design finding, reported, never tuned away silently.

## Layout

- `cu_golden.py` -- the runner: builds Anchor-shaped instructions
  (8-byte `sha256("global:<name>")` discriminator + borsh args), sends
  them over pure JSON-RPC (no websockets, per the W3.3 lesson), polls
  confirmation, and writes `cu-golden.json`.
- `run-cu-golden.sh` -- validator lifecycle: starts
  `solana-test-validator` with the built program, waits for RPC health
  over HTTP polling, runs the harness, stops the validator, removes
  the scratch ledger. Local validator only. No devnet, no mainnet, no
  real funds -- the authority is the validator's own genesis faucet
  keypair (read from the scratch ledger), because this sandbox's
  faucet `requestAirdrop` path fails with `TransactionDataTooLarge`
  (validator log, 2026-10-25) -- the faucet account is a throwaway
  local-test key, never a real secret.
- `cu-golden.json` -- the golden file: measured CU per instruction,
  with the program .so hash and Solana version it was measured against.

## Setup

```sh
python3 -m venv venv && venv/bin/pip install solana
anchor build   # from Kernel/treasury-program/
CU_GOLDEN_VENV=./venv ./run-cu-golden.sh
```

## Slice 1 (landed)

Genesis/admin instructions only -- the ones that need no kernel
transition: `initialize_config`, `register_agent`, `grant_role`,
`set_capacity`, `grant_capability`. These have no §2.2 estimate, so
their values are recorded as baseline, not gated.

## Next slices

Transition-bound instructions join in design order once their setup
chains are scripted (policy install, observation, plan, reconcile,
commit): `commit` against the §2.2 25-40K EST is the first gated
measurement, then `settle` (8-12K EST) and the value-moving family
(~7K EST each). The W3.2 adversarial suite is re-expressed against
the program's interface on top of this harness.

## Mollusk path (measured CU -- harness of record since 2026-10-25)

`mollusk/` is a standalone Rust crate (its own `[workspace]` root, so
the parent treasury-program workspace does not claim it). It loads
`target/deploy/treasury_program.so` (built by `anchor build` on the
adopted Solana 2.2.20 toolchain) into mollusk-svm 0.16.2's in-process
SVM and executes the slice-1 chain, threading resulting accounts
between instructions. The system program account is passed explicitly
(mollusk does not supply it as a fallback for instruction accounts).
Every instruction must succeed or the run aborts without writing a
golden file. Run it with:

```sh
anchor build   # from Kernel/treasury-program/
./mollusk/run-mollusk.sh   # writes cu-golden-mollusk.json
```

Measured slice 1 (2026-10-25, program .so sha256 recorded in
`cu-golden-mollusk.json`; repeat runs byte-identical, i.e. the
numbers are deterministic, not sampled):

| instruction | measured CU | result |
|---|---|---|
| `initialize_config` | 19,109 | Success |
| `register_agent` | 9,927 | Success |
| `grant_role` | 5,984 | Success |
| `set_capacity` | 9,486 | Success |
| `grant_capability` | 6,172 | Success |

Slice-1 instructions have no §2.2 estimate, so these are baseline,
NOT gated. Compatibility note (this run's verification item): the
program .so is built against Solana 2.1 program crates (Anchor 0.31
pins) while mollusk-svm 0.16.2 runs the Agave 4.x SVM line -- the
combination executes cleanly, including the Anchor `init` CPI to the
system program, and the Config account lands at exactly 74 bytes with
the authority stored correctly. Caveat, stated plainly: in-process
mollusk CU and validator-landed CU may differ slightly (feature set,
sysvar defaults); the golden file records the measurement environment
(mollusk version + .so hash) so any such difference is visible rather
than hidden. Devnet measurement is NOT a fallback: W4.3 forbids devnet
deployment (W4.4 owns it).
