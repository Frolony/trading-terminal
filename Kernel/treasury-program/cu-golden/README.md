# CU golden-test harness (W4.3)

**STATUS (2026-10-25): slice-4 CU MEASURED via the mollusk path below
-- the third GATED measurement has landed: `pay_tranche` = 11,703 CU
against the design §2.2 ~7K EST, gate PASS (2x band 3,500-14,000;
measured sits 1.67x the EST point -- inside the band, so NOT a design
finding under the >2x rule, but the direction is unfavorable and the
§2.2 batching headroom recomputes to ~119 payouts/tx, see Slice 4).
`initialize_treasury` = 27,485 CU, `open_window` = 15,445 CU,
`enroll` = 14,050 CU, all baseline (no §2.2 estimate). Earlier gated
measurements stand: `settle` = 12,860 CU (gate PASS) and `commit` =
17,633 CU (gate PASS). The validator path remains submission-only
(see "Sandbox findings").** The validator path boots,
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

## Slice 4 (landed 2026-10-25)

The value-moving family's first measured member, on top of slice 3:
a full `treasury.init.v1` lifecycle (transition 2, parent = the
slice-2/3 chain head) through settle, `initialize_treasury` on it, a
settled `treasury.window.v1` lifecycle (transition 3) as the
window-open source, `open_window` (window 0), `enroll` (holder,
position 10,000,000 raw, cohort 0), an operator agent holding
`OPERATOR_SETTLE`, a window-flow lifecycle (transition 4) stopped at
commit, and `pay_tranche` (operator-settled, $6.26 raw — the operating
dividend figure as a harness amount, not a promise). Post-tranche
assertions pass: accrued dividend exactly 6,260,000, `tranche_paid`
set, transition 4 still COMMITTED, stock 999,600 after four 100
debits, chain index 4 with the flow transition's declared hash.

| instruction | measured CU | result |
|---|---|---|
| `initialize_treasury` | 27,485 | Success, baseline (no §2.2 estimate; three account creations) |
| `open_window` | 15,445 | Success, baseline (no §2.2 estimate) |
| `enroll` | 14,050 | Success, baseline (no §2.2 estimate) |
| **`pay_tranche`** | **11,703** | **Success, gate PASS** |

Gate reading, stated plainly: 11,703 CU is inside the 2x band
(3,500-14,000) but 1.67x the ~7K EST point — so under the >2x rule
this is NOT a design finding, and the EST is not revised on this one
minimal-chain point. The direction is unfavorable: the payout costs
more than budgeted. Consequence, not hidden: §2.2's batching
headroom ("~150+ payout instructions per tx" at ~7K) recomputes to
~119 `pay_tranche` instructions per 1.4M-CU transaction at the
measured 11,703 CU — the 150+ figure should be revisited in §2.2
before it is repeated as fact. All slice-1/2/3 values reproduced
identically in the slice-4 run (21/21), so the harness change below
cost nothing.

Two slice-4 harness lessons, both proven in scratch before landing:

1. **The fee vault must be its PDA.** Slice 1 passed an arbitrary
   pubkey ([2;32]) as `initialize_config`'s fee_vault, but
   `initialize_treasury` requires the fee-vault account's key to equal
   `config.fee_vault` AND to be the `[b"fee-vault"]` PDA (its `init`
   seeds) — no `initialize_treasury` could ever succeed on slice 1's
   config. Slice 4 derives the fee vault as the real PDA; slices 1-3
   reproduce exactly under the change.
2. **The canonical window type is the SHA id, not the literal
   string.** Slice 2's harness `type_window()` pads the literal
   "treasury.window.v1" with zeros; the program's
   `TREASURY_WINDOW_TYPE` is SHA-256 of that string. Commit never
   checks the type against the constant, so slice 2 passed — but
   `open_window` and `pay_tranche` do, and a literal-type transition
   fails them (`WindowMismatch`, then a seeds mismatch when the
   account and the argument disagree). Slice 4 uses the SHA id for
   every transition that must authorize a treasury instruction. This
   also means slice 2's transition 1, though Settled, cannot
   authorize treasury instructions — a harness-type gap, not a
   program bug, and it is why slice 4 runs its own settled window
   lifecycle instead of reusing transition 1.

**Not measured in slice 4, by design, not by omission:**
`launch_stage2` refuses `ThresholdUnset` (D-2: B is unset until
Carlos sets it, and an unset threshold must never read as a pass),
so no success chain exists for it yet. `borrow` is unmeasured
because its book cap is T/12 over the window's period-start
snapshot, and `initialize_treasury` funds the treasury at zero with
no deposit instruction yet on-chain — any positive borrow refuses
the cap. Both are next-slice work.

**Deterministic per-call-site variation (open question, not a
finding).** Every value in this file is deterministic across repeat
runs (the slice-4 run was executed twice with zero diffs), but
account-creating instructions vary by call site: e.g.
`submit_signal` measures 20,144 / 17,144 / 17,144 / 30,644 across
transitions 1-4, and `submit_plan` 17,715 / 22,215 / 17,715 /
20,715. Non-creating instructions are invariant across all four
lifecycles (`commit` 17,633, `settle` 12,860, `reconcile` 8,652,
`open_planning` 10,393, `submit_constraint_report` 7,706 every
time). The pattern points at the account-creation path (the only
thing that differs per created account is its PDA), but the
mechanism is unisolated and the magnitude (up to ~13.5K) is too
large for a bump search alone — a future run should isolate it
(same instruction, same args, different PDAs) before any
creating-instruction value is treated as a single stable number.
No gate depends on a creating instruction's value: the gated
`pay_tranche` is non-creating and stable at 11,703 across runs.

## Next slices

`borrow` (once a treasury-funding path exists), `settle_default` /
`exit` / whale tranches, then the W3.2 adversarial suite
re-expressed against the program's interface with measured CU.
