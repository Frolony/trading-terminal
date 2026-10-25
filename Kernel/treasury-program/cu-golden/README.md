# CU golden-test harness (W4.3)

**STATUS (2026-10-25): slice-6 EXIT-TRANSITION REFUSAL PROBE
MEASURED via the mollusk path below -- the queued whale-schedule
refusal is now proven, not assumed: on a Committed
`treasury.exit.v1` transition (transition 5, exit policy, parent =
chain head (4, [37;32])), `schedule_whale_exit` refuses
`InvalidScheduleAmount` (6065) at 16,017 CU, because FLAG = T/30
over the zero snapshot is zero and no schedule can be sized, with
state asserted unchanged afterwards (no schedule account created).
This is a refusal CU, NOT a success CU: schedule success still
needs a funded snapshot, and that funding question is still the
queue blocker, filed for Carlos (see Slice 5). Earlier measurements
stand: slice-5 refusals `borrow` = 11,948 CU (`BookCapExceeded`
6053), `exit` = 9,893 CU (`ExitExceedsLiquidity` 6060),
`settle_default` = 14,575 CU (`NoActiveLoan` 6055); `pay_tranche` =
11,703 CU (gate PASS), `settle` = 12,860 CU (gate PASS), `commit` =
17,633 CU (gate PASS), `initialize_treasury` = 27,485 CU,
`open_window` = 15,445 CU, `enroll` = 14,050 CU, all baseline
where noted. The validator path remains submission-only
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

**Deterministic per-call-site variation — ISOLATED 2026-10-25
(submit_signal optimization pass, step 1).** Every value in this
file is deterministic across repeat runs, but account-creating
instructions vary by call site — and the mechanism is now proven,
not guessed: it is Anchor's on-chain PDA bump search. For an
`init` account Anchor calls `find_program_address` on-chain
(anchor-derive-accounts: "`init` uses `find_program_address` to
calculate the pda"), which hashes seed-by-seed from bump 255 down
until the hash falls off the ed25519 curve. Each attempt costs
**1,500 CU** (one SHA-256 + off-curve check via syscall), and the
golden file fits `CU = C0 + 1,500 x attempts` with ZERO residual
across 7 instruction families and 25 call sites. The bump for each
created PDA was recomputed independently of the harness with
`solana-pubkey` 2.1.21 `find_program_address` (scratch tool at
`hidden_files/pda-check/`, program id
`FPGSeLBWEiFKdTuqDbJeKhjXeah5H4c3rqsW4Tzn3RAS`):

| instruction | call sites (CU) | attempts | fit |
|---|---|---|---|
| `submit_signal` | 20,144 / 17,144 / 17,144 / 30,644 / 18,644 (transitions 1-5) | 3 / 1 / 1 / 10 / 2 | C0 15,644 + 1,500 x attempts, exact |
| `submit_plan` | 17,715 / 22,215 / 17,715 / 20,715 / 22,215 | 1 / 4 / 1 / 3 / 4 | C0 16,215 + 1,500 x attempts, exact |
| `register_agent` | 9,927 / 9,927 / 12,927 / 12,927 / 9,927 (agent / proposer / evaluator / oracle / operator) | 1 / 1 / 3 / 3 / 1 | C0 8,427 + 1,500 x attempts, exact |
| `register_transition` | 14,150 / 14,150 / 20,150 / 15,650 / 14,150 | 1 / 1 / 5 / 2 / 1 | C0 12,650 + 1,500 x attempts, exact |
| `policy_transition` | 10,698 / 12,198 / 12,198 / 10,698 (window-lit / init / window-sha / exit) | 1 / 2 / 2 / 1 | C0 9,198 + 1,500 x attempts, exact |
| `post_observation` | 10,791 / 18,291 / 10,791 / 12,291 / 12,291 | 1 / 6 / 1 / 2 / 2 | C0 9,291 + 1,500 x attempts, exact |
| `record_evidence` | 12,413 / 16,913 / 19,913 (transitions 1-3) | 1 / 4 / 6 | C0 10,913 + 1,500 x attempts, exact |

Two consequences, stated plainly. First, a creating instruction
has no single stable CU — only a per-PDA value, and the golden
file's values are correct as the PDAs they were measured on; any
future citation must name the PDA (or the attempts count) with
the figure. Second, the other cost drivers named for this pass
(SignalRecord creation per call, TransitionRecord
deserialize/re-serialize, PolicyRecord/AgentRecord traffic) are
NOT the variance mechanism — they live inside the constant C0,
which is identical across call sites. What step 2 of the pass can
remove is only the search: an explicit-bump `init` (bump supplied
as an instruction argument, verified by a single
`create_program_address` instead of the search loop) would
collapse every creating instruction to at most its attempts=1
value — e.g. `submit_signal` at 17,144 CU on every call site —
without changing signal semantics (the SignalRecord, its seeds,
and the one-signal-per-agent-per-round rule are untouched). That
is the queued step-2 change, and its floor is measured, not
assumed, after it lands. Non-creating instructions stay invariant
(`commit` 17,633, `settle` 12,860, `reconcile` 8,652,
`open_planning` 10,393, `submit_constraint_report` 7,706), and no
gate depends on a creating instruction's value: the gated
`pay_tranche` is non-creating and stable at 11,703 across runs.
An early offline bump computation that disagreed with Rust was
discarded — the Rust `find_program_address` values above are the
ones that fit, and the Python attempt is recorded here only so it
is not re-derived.

## Slice 5: blocked-family refusal probes (2026-10-25)

| instruction | CU | result |
|---|---|---|
| `borrow` (1,000,000 raw, window 0, zero snapshot) | 11,948 | Refused: `BookCapExceeded` (6053) |
| `exit` (1,000,000 raw, window 0, zero snapshot) | 9,893 | Refused: `ExitExceedsLiquidity` (6060) |
| `settle_default` (holder with no loan) | 14,575 | Refused: `NoActiveLoan` (6055) |

Why these are refusals, not failures of the harness: each probe
rides a Committed window flow transition (transition 4) and passes
every gate before the funding-dependent one — the borrow is within
the per-holder cap (50% of the 10,000,000 position), the exit is
within accrued dividend and position, the settlement caller holds
`OPERATOR_SETTLE` — and each then meets exactly the gate that a
zero snapshot forces. Post-refusal assertions prove nothing moved:
position 10,000,000, accrued dividend 6,260,000, loan 0, book
outstanding 0, transition 4 still COMMITTED. All 57 slice-1..4
values reproduced identically in the slice-5 run.

**The funding gap, stated as a finding, not a footnote.**
`initialize_treasury` zeroes every total, `enroll` writes only the
HolderRecord and the window roster hash, `open_window` snapshots
whatever totals exist — and no instruction anywhere in the program
or the design writes a nonzero total into TreasuryState. The
treasury's T is bookkeeping that nothing funds. Until a funding
path exists, `borrow`, `exit`, `schedule_whale_exit` (zero FLAG
sizes no schedule), and everything downstream of a loan
(`settle_default`) have no success chain at all — only the
refusals measured here. A harness-seeded funded snapshot was
considered and refused: it would measure a chain no real execution
can reach. What the funding path IS — a deposit instruction with a
token vault, a governed bookkeeping set via policy transition, or
enrollment aggregation into TreasuryState — is a design decision
for Carlos, not builder improvisation, and it is now the W4.3
queue blocker.

## Slice 6: exit lifecycle + schedule_whale_exit zero-FLAG refusal (2026-10-25)

The queued refusal did not need the funding decision after all: it
needs a Committed `treasury.exit.v1` transition, not a funded
snapshot, so this slice builds one (transition 5: exit policy
installed, observation 5, plan/signal/constraint, reconcile, commit
— parent = chain head (4, [37;32]); chain index 5, hash [47;32],
stock 999,500 after the fifth 100 debit) and probes the schedule
on it. The probe passes every gate before the FLAG check — state
COMMITTED, exit type, window binding (holder record window 0),
positive total 1,000,000 raw within the 10,000,000 position — and
then meets exactly the gate a zero snapshot forces: FLAG = T/30
over snapshot 0 is zero (lib.rs:4739, the `flag != 0` require).

| instruction | CU | result |
|---|---|---|
| `schedule_whale_exit` (1,000,000 raw, transition 5, window 0, zero snapshot) | 16,017 | Refused: `InvalidScheduleAmount` (6065) |
| `commit:5` (exit lifecycle, repeat of the gated instruction) | 17,633 | Success (identical to the gated `commit` — same minimal chain) |

Post-refusal assertions prove nothing moved: position 10,000,000,
accrued dividend 6,260,000, loan 0, transition 5 still COMMITTED,
chain index still 5, and the pre-seeded schedule slot is still
empty — the refused instruction created no schedule account. All 60
slice-1..5 values reproduced identically in the slice-6 run. This
is a refusal CU, NOT a success CU, and it changes nothing about
the funding blocker: schedule success, borrow/exit/settle_default
success, and the W3.2 re-expression still wait on Carlos's
funding-path decision.

## Next slices

Optimization pass step 2 (queued, NOT blocked by the funding
decision): explicit-bump `init` on the creating instructions,
re-measured against the attempts=1 baselines above
(`submit_signal` 17,144, `submit_plan` 17,715,
`register_agent` 9,927, `register_transition` 14,150,
`policy_transition` 10,698, `post_observation` 10,791,
`record_evidence` 12,413) — the search loop is the only part
removed; signal/plan/record semantics are unchanged. Then, once
Carlos rules the funding path: `borrow` / `exit` /
`settle_default` / `schedule_whale_exit` / whale-tranche SUCCESS
CU, then the W3.2 adversarial suite re-expressed against the
program's interface with measured CU.
