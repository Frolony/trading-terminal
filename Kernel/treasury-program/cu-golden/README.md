# CU golden-test harness (W4.3)

**STATUS (2026-10-25): slice-3 CU MEASURED via the mollusk path below
-- the second GATED measurement has landed: `settle` = 12,860 CU
against the design §2.2 8-12K EST, gate PASS (2x band 4,000-24,000;
measured sits 860 CU ABOVE the EST band's top -- a 1.07x overage, not
a design finding under the >2x rule, and the direction is the
unfavorable one by a whisker, so it is stated plainly rather than
rounded away). `record_evidence` = 12,413 CU baseline (no §2.2
estimate). The first GATED measurement stands: `commit` = 17,633 CU
against the design §2.2 25-40K EST, gate PASS (2x band 12,500-80,000;
measured sits BELOW the EST band -- the estimate was conservative,
which is the safe direction). The validator path remains
submission-only (see "Sandbox findings").** The validator path boots,
funds, encodes, and submits, but no transaction confirms on a local
validator in this sandbox. The mollusk path executes the chains
in-process and records real CU; it is the harness of record for
measured CU, and the validator path is retained only as a fallback
shape, not a measurement source.

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

## Next step (queue) — DONE, see Slice 2 below

Mollusk-svm in-process execution landed as the harness of record
(slice 1, then slice 2 with the first gated `commit` measurement).
Devnet measurement is NOT the fallback: W4.3 forbids devnet deployment
(W4.4 owns it).


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

## Next slices (pre-slice-2 note — superseded by Slice 2 below)

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

## Slice 2 (landed 2026-10-25)

The full governance lifecycle to a committed transition, in one
mollusk run: slice-1 chain, then proposer/evaluator/oracle agents,
`policy_transition` (design launch values for `treasury.window.v1`:
quorum 1.0, confidence 0.5, min-observation-confidence 0.5, no
objectives, max 3 rounds / 4 plans, one mandatory constraint),
`post_observation`, `register_transition`, `open_planning`,
`submit_plan` (claim 100 on stock 7), `submit_signal` (raw and support
1.0), `submit_constraint_report` (PASS), `reconcile`, `commit`.
Clock discipline: mollusk's clock starts at unix_timestamp 0 and warp
resets it, so the harness sets `mollusk.sysvars.clock` directly --
phase A at 1,000,000 with a 3,600s planning window (a harness
parameter, not a design value), phase B at the 1,003,600 deadline so
reconcile/commit judge planning closed. Post-commit assertions pass:
transition state COMMITTED, chain index 1 with the declared hash,
stock 7 debited 100 (1,000,000 -> 999,900).

| instruction | measured CU | result |
|---|---|---|
| `register_agent:proposer` | 9,927 | Success |
| `register_agent:evaluator` / `:oracle` | 12,927 each | Success |
| `grant_role:evaluator` / `:oracle` | 5,986 / 5,985 | Success |
| `policy_transition` | 10,698 | Success |
| `post_observation` | 10,791 | Success |
| `register_transition` | 14,150 | Success |
| `open_planning` | 10,393 | Success |
| `submit_plan` | 17,715 | Success |
| `submit_signal` | 20,144 | Success |
| `submit_constraint_report` | 7,706 | Success |
| `reconcile` | 8,652 | Success |
| **`commit`** | **17,633** | **Success, gate PASS** |

Gate reading, stated plainly: 17,633 CU is inside the 2x band
(12,500-80,000) but below the §2.2 EST band (25,000-40,000) -- the
low-end miss is 1.42x, so this is NOT a design finding under the >2x
rule, and the direction is favorable (the full predicate costs less
than budgeted, so the §2.2 batching headroom improves). The EST is a
range for a general transition; this measurement is the minimal chain
(1 plan, 1 signal, 1 constraint, 1 stock), so larger transitions are
measured next before any EST revision. Unexplained-but-deterministic
observation (setup only, ungated, no gate impact): the 3rd/4th
`register_agent` calls measure 12,927 CU vs 9,927 for the 1st/2nd, and
`grant_role` varies by 1-2 CU across agents -- identical across repeat
runs, cause not isolated, noted for a future run rather than
investigated here.

## Slice 3 (landed 2026-10-25)

Slice 2's chain, then `record_evidence` over the committed transition
(realized hash = declared hash [7;32], by the oracle) and `settle`
(exact: realized == declared, one attribution of +5 units to the plan
author, the proposer). Post-settle assertions pass: transition state
SETTLED, realized hash recorded on the record, proposer reputation
credited exactly +5,000,000 raw (SCALE 1e6). All slice-1/2 values
reproduced identically (commit 17,633 again).

| instruction | measured CU | result |
|---|---|---|
| `record_evidence` | 12,413 | Success, baseline (no §2.2 estimate) |
| **`settle`** | **12,860** | **Success, gate PASS** |

Gate reading, stated plainly: 12,860 CU is inside the 2x band
(4,000-24,000) but 860 CU above the §2.2 EST band's top (8,000-12,000)
-- a 1.07x overage, so this is NOT a design finding under the >2x
rule, and the EST is not revised on this one minimal-chain point
(1 plan, 1 attribution; the remaining-account classification and
reputation write-back are the plausible delta over the estimate, and
larger attributions are measured before any EST revision). The
direction is the unfavorable one by a whisker, and it is recorded as
such: the §2.2 batching headroom does not depend on settle (one
settle per transition, not per payout), so no batching claim moves.

## Next slices

The value-moving family (~7K EST each) joins once its setup chains
are scripted on top of slice 3 (treasury instructions ride committed
window transitions), then the W3.2 adversarial suite is re-expressed
against the program's interface with measured CU.
