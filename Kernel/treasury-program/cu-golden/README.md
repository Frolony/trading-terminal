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

**ADOPTION NOTE (2026-10-25, Anchor 1.2.1): the figures in the STATUS paragraph above are the
Anchor 0.31 dataset, now superseded. `cu-golden-mollusk.json` was regenerated under Anchor 1.2.1 /
Solana 4.1.2 (platform-tools v1.54) in the adoption pass: 70/70 entries, same result family,
every value shifted by new codegen. Current key figures: `commit` 17,876 CU, `settle` 12,845 CU,
`pay_tranche` 11,675 CU, `submit_signal` 16,463–29,963 CU, slice-5 refusals `borrow` 12,400 /
`exit` 10,427 / `settle_default` 14,938 CU, slice-6 `schedule_whale_exit` refusal 15,017 CU,
`enroll` 27,040 CU, `submit_plan` +6,251 CU at every call site. The design doc's §2.2 carries
the same figures; this README's STATUS paragraph is retained as the 0.31 record.**

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

## Slice 7: funded chain — deposit + borrow / exit / settle_default / schedule_whale_exit SUCCESS CU (2026-10-26)

The funding gap is closed by a REAL deposit, not a seeded
snapshot. The harness loads the public SPL Token program bytes
under the Token program id via mollusk's
`add_program_with_loader_and_elf`, so `deposit` executes the
genuine CPI chain: system create + InitializeAccount (owner =
the TreasuryState PDA) + TransferChecked. The fixture is NOT
committed to the repo (the GitHub snapshot tool accepts UTF-8
files only, and the ELF is binary — a 5-path push was refused
on exactly that file, nothing pushed): `run-mollusk.sh` fetches
it on demand from devnet with
`solana program dump TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA`
— a read of public program data, no deployment, no wallet, no
funds — and verifies sha256
`8190d3f7ceb6cb7a7a8d8924bff89f9f611e15ce1f806f2b6237f3311a98f697`
before measuring, refusing on mismatch; the same sha is
recorded in the golden file. A scratch build of
spl-token 4.0.0 from the registry was attempted first and
abandoned with evidence, not guessed away: under the adopted
Agave 4.x sbpf target (`target_os` unset) solana-program 1.18's
`cfg(not(target_os = "solana"))` ark deps leak into the SBF unit
graph and getrandom 0.3.4 has no sbpf backend (compile error);
the 2.2.20 cargo (1.84) cannot parse the edition2024 manifests
in the fresh lock. The devnet dump is the real program, not a
stub, so the deposit CU below is measured against real token
execution. (The scratch build directory is retained at
`hidden_files/spl-token-build/`, outside the repo tree.)

Funded-chain numbers (raw, ratio-exact): deposit 60,000,000
(6 decimals, source 100,000,000 -> vault 60,000,000, source
40,000,000, conservation asserted) -> T = cash = 60,000,000;
window 1 snapshot total 60,000,000, exit stock L = 10,000,000,
book cap 5,000,000, FLAG 2,000,000; holder window 1 position
30,000,000; pay_tranche:1 1,000,000 (repeat of the gated
instruction, 11,712 CU again); borrow 2,000,000 at clock
7,003,600 (maturity 12,187,600); exit 1,500,000; settle_default
at maturity; schedule 10,000,000 (tranche 2,000,000, count 5,
entry window 1, lock_until 13). The vault's rent-exact funding
is 2,039,280 lamports for 165 bytes, exactly the rent formula
— asserted by the vault's post-deposit lamports, not assumed.

| instruction | CU | result |
|---|---|---|
| `deposit` (60,000,000 raw, real SPL CPI chain) | 13,944 | Success, baseline (no §2.2 EST exists for deposit) |
| `borrow` (2,000,000 raw, funded window 1) | 12,158 | Success, gate PASS (1.74x the ~7K EST point; inside the 2x band) |
| `exit` (1,500,000 raw: 1,000,000 accrued + 500,000 excess, penalty 65,000, paid 1,435,000) | 10,607 | Success, gate PASS (1.52x; inside the 2x band) |
| `settle_default` (at maturity 12,187,600; position 27,500,000, skip flag set) | 14,512 | Success, **DESIGN FINDING** (2.07x the ~7K EST point; 512 CU over the 14,000 band top) |
| `schedule_whale_exit` (10,000,000 raw; tranche 2,000,000, count 5) | 15,841 | Success, **DESIGN FINDING** (2.26x the ~7K EST point) |

Gate readings, stated plainly. The ~7K EST modeled a thin
bookkeeping instruction shaped like `pay_tranche`. `borrow`
and `exit` fit that shape and pass. `settle_default` and
`schedule_whale_exit` do not: the former touches five
accounts plus a Clock read and an event with authorization
context; the latter creates an account (system create CPI)
and writes nine schedule fields. Both findings are recorded
in the design doc's §2.2 in the same pass, not tuned away,
and neither moves the batch arithmetic: neither instruction
is per-payout batched (one default settlement per default,
one schedule per whale), so the ~119 payouts/tx figure
(pay_tranche-driven) stands. The two findings also close
the slice-5 refusal record honestly: the refusals were the
unfunded chain's truth, and the successes are the funded
chain's truth, both measured, neither averaged.

Slice 1–6 reproduction note: the golden file regenerated
against the post-deposit program .so (sha256 recorded in
the file; the shipped slice-6 golden was generated against
the pre-deposit .so). 69 of 73 slice 1–6 entries reproduce
identically, including every gated value (commit 17,959,
settle 12,965, pay_tranche 11,712); four entries shift by
exactly ±2 CU (submit_signal shadow/replay 12,401 -> 12,403,
borrow refusal 12,439 -> 12,437, settle_default refusal
14,975 -> 14,977) — the deposit build's codegen footprint
on the program binary, not a behavior change, and the
repeat run reproduces the new values exactly.

NOT measured here, by design, not by omission:
`execute_whale_tranche` success. Its gate needs an
execution window >= lock_until (13), so it waits for the
next slice's 12-window walk from window 1 (windows 2..13,
each with its own Settled window transition + open). The
schedule created here (entry 1, lock_until 13) is the walk's
input; the walk measures the tranche, it does not re-create
the schedule.

## Slice 8: 12-window walk -> execute_whale_tranche SUCCESS CU (2026-10-26)

The slice-7 schedule (entry window 1, lock_until 13, tranche
2,000,000, count 5) executes here, on its own terms. Windows
2..13 each open on their OWN Settled `treasury.window.v1`
transition (transitions 9..20, parented in sequence on the
previous transition's declared hash, transition 9 on head 8),
because `open_window` requires window_id == window_count and
each open consumes exactly one count slot — no transition is
reused, and the carried source-transition-consumption item is
satisfied by construction on this chain (one transition, one
window). Every window opens on the same funded snapshot
(total 60,000,000, exit stock 10,000,000, FLAG 2,000,000):
no further deposit lands in this chain. The walk's 132
setup entries (12 lifecycles through settle + 12 opens)
reproduce deterministically; the walk `open_window` values
vary by window id (14,984–19,484 CU) in 1,500 CU steps — the
known per-PDA bump-search cost (see the §2.2 finding), not a
behavior difference, and these are setup values, ungated.

| instruction | CU | result |
|---|---|---|
| `execute_whale_tranche` (tranche 1 of 5, 2,000,000 raw; Committed exit transition 8, creation window 1, execution window 13 >= lock_until 13, execution window 13 snapshot 60,000,000 / FLAG 2,000,000, position 27,500,000, loan 0) | 16,360 | Success, **DESIGN FINDING** (2.34x the ~7K EST point; 2,360 CU over the 14,000 band top) |

Post-execution state asserted: schedule tranches_executed 1,
last_block_time 31,003,600 (the walk's closing clock — the
weekly-cadence baseline for tranche 2, not measured here);
position 25,500,000; window 13 exit stock 8,000,000; window 1
exit stock unchanged at 8,500,000 (execution debits the
execution window only, as the D-10 cross-window design
requires). Golden file 240 entries: all 107 slice-7 entries
reproduce exactly (program .so sha unchanged), plus the 132
walk setup entries and this measurement. The finding joins
`settle_default` and `schedule_whale_exit` as the third
~7K-EST miss above the band, and like them it does not move
the batch arithmetic: one tranche execution is not
per-payout batched.

## Slice 9: W3.2 re-expression, part 1 — tranche-2 weekly cadence (2026-10-26)

The D-10 revised contract pays a whale schedule in weekly blocks:
tranche 2 may execute only at `now >= last_block_time + WEEK_SECS`
(604,800s), the gate inclusive. Three measurements on the slice-8
chain, all on execution window 13 (13 >= lock_until 13; window-13
exit stock 8,000,000 after tranche 1 still covers a 2,000,000 block)
under the same Committed exit transition 8:

| instruction | CU | result |
|---|---|---|
| `execute_whale_tranche:too_soon` (tranche 2 at the tranche-1 clock, 31,003,600 == last_block_time, a full week short) | 15,647 | Refused, `TrancheTooSoon` (6069) |
| `execute_whale_tranche:too_soon_boundary_minus_1` (tranche 2 at 31,608,399 = last_block_time + WEEK_SECS − 1) | 15,647 | Refused, `TrancheTooSoon` (6069) |
| `execute_whale_tranche:tranche2` (tranche 2 of 5, 2,000,000 raw, at exactly 31,608,400) | 16,369 | Success, **DESIGN FINDING** (2.34x the ~7K EST point; 2,369 CU over the 14,000 band top) |

Post-refusal state asserted unchanged (schedule still 1 block executed,
last_block_time still 31,003,600, position 25,500,000, window-13 stock
8,000,000); post-success state asserted: tranches_executed 2,
last_block_time 31,608,400, position 23,500,000, window-13 exit stock
6,000,000, window-1 stock untouched at 8,500,000 (execution debits the
execution window only). The two refusals cost identically (15,647 CU):
the clock value cannot move CU through this gate — only the branch
outcome can, and both refusals take the same branch. The success costs
9 CU above tranche 1 (16,360 CU): the cadence check itself, now
actually evaluated (tranche 1, with tranches_executed 0, skips it).
The finding joins tranche 1, `settle_default`, and
`schedule_whale_exit` as the same class of ~7K-EST miss above the band,
and like them it does not move the batch arithmetic: one tranche
execution is not per-payout batched. Golden file 243 entries: all 240
slice-8 entries reproduce exactly (program .so sha unchanged).

## Slice 10: W3.2 re-expression, part 2 — blind-ape bulk exit vs the flat 13% door (2026-10-26)

Engine-sim T11 extracts the whole position in one go above the
per-period allowance and prices the flat 13% penalty on the excess.
On the program that whole-position order cannot even be placed: the
D-10 hard bound (no single order above one period's movable
liquidity L = T/6, judged on the window's period-start snapshot)
refuses it before any position or stock check. Three measurements
on the slice-9 chain (funded window 1: snapshot 60,000,000,
L = 10,000,000; holder window 1: position 23,500,000 after tranche 2,
accrued dividend 0, loan 0; window-1 exit stock 8,500,000 after the
slice-7 exit; Committed window-flow transition 7 still authorizes
exits; clock 31,608,400, which no exit gate reads):

| instruction | CU | result |
|---|---|---|
| `exit:bulk_over_L` (23,500,000 raw, the ape's whole-position order) | 10,463 | Refused, `ExitExceedsLiquidity` (6060) |
| `exit:bulk_max` (8,500,000 raw, the entire remaining window-1 stock, below L) | 10,606 | Success, gate PASS (1.51x the ~7K EST point; inside the 2x band) |
| `exit:stock_exhausted` (a further 1,000,000 raw on window 1) | 10,472 | Refused, `ExitStockExhausted` (6063) |

Post-refusal state asserted unchanged in both refusal cases; after
the success: accrued still 0, position 23,500,000 -> 15,000,000,
window-1 stock 8,500,000 -> 0, window-13 stock untouched at
6,000,000, schedule untouched (2 blocks executed, last_block_time
31,608,400). The bulk exit's arithmetic, stated plainly: accrued 0,
so the full 8,500,000 is excess at the flat 13% door — penalty
1,105,000, paid 7,395,000. The ape's ledger on this record after the
bulk exit is position 15,000,000 + paid 7,395,000 = 22,395,000
against 23,500,000 before it: 1,105,000 worse at once, before any
future penalty-free allowances abandoned by rushing. The window's
aggregate exits now total exactly L (slice-7 1,500,000 + slice-10
8,500,000 = 10,000,000): repeated sub-L exits sum against the stock,
so no further exit in window 1 is possible on this chain, and the
schedule's remaining tranches (3 x 2,000,000) still fit the
15,000,000 position. Gate reading: `exit:bulk_max` at 10,606 CU
repeats the slice-7 exit's gate PASS at 10,607 CU (1 CU apart, same
instruction, same account set, different holder state — accrued 0
here vs 1,000,000 there, loan 0 vs 2,000,000; the 1 CU delta is
recorded, not explained away, and it moves no gate). No §2.2
revision: the exit family already carries the 10,607 figure, and
10,606 confirms it rather than changing it. As in every slice,
`exit` here is bookkeeping, not token movement. Golden file 246
entries: all 243 slice-9 entries reproduce exactly (program .so
sha unchanged).

## Slice 11: W3.2 re-expression, part 3 — default-as-exit (T10) on the funded chain (2026-10-26)

Engine-sim T10: the attacker borrows the max at every window and
defaults (interest forgiven, principal settles against the position,
next dividend forfeited), ending strictly worse than the scheduled
path because the forfeited dividends are the gap. This slice
separates T10's two halves on the program instead of asserting the
engine's conclusion onto it. Twin holders in the currently open
window 13 (window_count 14, cohort 13 % 12 = 1, snapshot 60,000,000,
exit stock 6,000,000 after tranche 2, Committed window-flow
transition 7 still authorizing executions), each enrolled at
position 10,000,000:

| instruction | CU | result |
|---|---|---|
| `enroll:13D` / `enroll:13S` (twin enrolls, window 13) | 30,040 / 27,040 | Success, setup (ungated; the 3,000 CU delta is two extra PDA bump-search attempts at the known 1,500 CU/attempt cost, not a behavior difference) |
| `borrow:over_cap` (5,000,001 raw, twin D) | 12,406 | Refused, `LoanExceedsPosition` (6052) |
| `borrow:11max` (5,000,000 raw, twin D) | 12,159 | Success, gate PASS (1 CU above slice-7 `borrow` 12,158 — the same 1-CU repeat class as slice 10's exit) |
| `settle_default:pre_maturity` (twin D, clock 31,608,400 < maturity 36,792,400) | 14,962 | Refused, `LoanNotMatured` (6056) |
| `settle_default:11` (twin D, at maturity 36,792,400) | 14,513 | Success, **same DESIGN FINDING as slice-7 `settle_default` 14,512** (1 CU above it, 513 CU over the 14,000 band top — the finding is confirmed, not new) |
| `pay_tranche:11skip_probe` (twin D, AFTER the default set skip_next_dividend) | 11,711 | **Success — THE PROBE.** The skip flag does not gate `pay_tranche`: `can_pay_tranche` takes no skip argument (state, type, cohort, standing authorization, operator, tranche-paid, amount), and the `settle_default` handler itself carries the gap in its docs (the skipped slot is a FUTURE window's separate HolderRecord; honoring the skip at that window's `pay_tranche` is still owed) |
| `exit:11D_par` (twin D, 1,000,000 raw) | 10,607 | Success, gate PASS (penalty 0, position stays 5,000,000) |
| `pay_tranche:11S` / `exit:11S_par` (twin S, scheduled path, 1,000,000 raw) | 11,712 / 10,608 | Success, gate PASS (penalty 0, position stays 10,000,000) |

The one-cycle ledgers come out EQUAL on the program today:
defaulter 5,000,000 loan cash + 5,000,000 position + 1,000,000 paid
tranche = 11,000,000; scheduled 1,000,000 paid + 10,000,000
position = 11,000,000. That equality is the finding, stated, not a
pass: the mechanics half of T10 (max-loan boundary, maturity gate,
position settlement on the pre-default base, skip flag set) is
measured and green, but the forfeiture half that makes engine T10
strictly worse is unenforced on the program — the carried
skip-enforcement item must land before T10 can be claimed as
re-expressed on the program. No §2.2 revision: `borrow`,
`settle_default`, `pay_tranche`, and `exit` repeats all confirm
their existing §2.2 figures within 1 CU (the slice-10 treatment).
Window 13 exit stock ends 4,000,000 (6,000,000 − twin D 1,000,000 −
twin S 1,000,000); window 1 stock stays 0; the schedule is untouched
(2 blocks, last_block_time 31,608,400); clock ends 36,792,400, which
later slices must account for. Golden file 256 entries: all 246
slice-10 entries reproduce exactly (program .so sha unchanged).
As in every slice, `borrow` / `exit` / `settle_default` here are
bookkeeping, not token movement.

## Slice 12: W3.2 re-expression, part 4 — cohort-timing at window boundaries (D-11) (2026-10-26)

Engine-sim cohort-timing games ask whether a holder can slip into
a window that has closed, reserve a window that has not opened,
enter twice, or enter with nothing locked. The program answers
all four at the enroll gate (`can_enroll`: window_id must equal
window_count − 1, position positive) plus the PDA `init` (one
record per holder per window). Measured on the slice-11 chain
(window_count 14, current window 13, clock 36,792,400, window 13
snapshot 60,000,000 / stock 4,000,000, Committed window-flow
transition 7 still authorizing executions), then the boundary is
crossed for real — Settled transition 21 opens window 14 — and
both sides of the crossing are measured:

| instruction | CU | result |
|---|---|---|
| `enroll:12past` (past window 12, window_count 14) | 14,388 | Refused, `NotCurrentWindow` (6043) — a closed pool admits no newcomer |
| `enroll:13zero` (current window 13, position 0) | 12,868 | Refused, `InvalidPosition` (6044) — a zero lock is not an entry |
| `enroll:13double` (slice-11 twin D again in window 13) | 8,619 | Refused at the account layer before the handler: the Anchor `init`'s system-program create_account CPI fails with custom 0x0 on the already-existing holder record — the counted-once mechanism, not a gate bypass |
| `enroll:13current` (holder 25, window 13, 5,000,000) | 27,040 | Success, setup — cohort 1 (13 % 12), standing authorization true; the live side of the boundary before it is crossed |
| `open_window:14` (Settled transition 21) | 16,484 | Success, setup — window_count 14 → 15, snapshot 60,000,000, exit stock 10,000,000; the 10 transition-21 lifecycle entries ride the slice-8 shape |
| `enroll:13closed` (window 13 AFTER window 14 opened) | 14,388 | Refused, `NotCurrentWindow` (6043) — the same window that accepted `enroll:13current` now refuses; the boundary is the counter, not the clock |
| `enroll:14current` (holder 27, window 14, 5,000,000) | 27,040 | Success, setup — cohort 2 (14 % 12), standing authorization true; next-window-only entry in its positive form |
| `enroll:15future` (future window 15, window_count 15) | 3,500 | Refused at the account layer: no window 15 account exists yet, so Anchor fails with `AccountNotInitialized` (3012) on `window_state` before `can_enroll` runs — reserving a not-yet-opened pool is impossible by construction, not merely by policy |
| `pay_tranche:14cohort` (holder 27, window 14, 1,000,000) | 11,712 | Success, gate PASS — holder cohort 2 equals window 14's rotation slot (14 % 12), so the new cohort's slot pays in its own window |

Final state: window 13 snapshot 60,000,000 / stock 4,000,000
unchanged after its enroll (enroll debits no stock); window 14
snapshot 60,000,000 / stock 10,000,000 unchanged by its enroll
and payout; window 1 stock stays 0; the whale schedule is
untouched (2 blocks, last_block_time 31,608,400); clock ends
37,003,600 (transition 21 lifecycle close), which later slices
must account for. Golden file 275 entries: all 256 slice-11
entries reproduce exactly (program .so sha unchanged,
`82759c51…`). No §2.2 revision: the one gated figure
(`pay_tranche` 11,712) confirms the family's existing figure
within 1 CU. As in every slice, `enroll` / `pay_tranche` here
are bookkeeping, not token movement. A window-13 cohort holder
cannot collect a window-14 slot by the same construction: its
record is PDA-bound to window 13, so no cross-window payout
address exists to attempt.

## Slice 13: W3.2 re-expression, part 5 — fee-flow edge cases under the single 73 bps stream (D-5) (2026-10-26)

The design (§4.2) says the 73 bps fee rides the venue flows in the compose layer and
`collect_fees` reconciles the window's collected total against the declared total; it does
NOT say any current instruction skims a fee. The program matches that negative half
exactly: `FEE_PPM = 7_300` is the only fee constant, no value-moving handler adds a fee
term to any amount, and the only FeeVault write anywhere is `initialize_treasury` zeroing
`collected_raw`. This slice proves the negative half on the slice-12 chain (window_count
15, current window 14, clock 37,003,600, window 14 snapshot 60,000,000 / stock 10,000,000,
Committed window-flow transition 7 still authorizing executions) instead of asserting it:

| instruction | CU | result |
|---|---|---|
| `deposit:13fee_exact` (second real deposit, 10,000,000 raw) | 9,649 | Success, baseline — TreasuryState total and cash 60,000,000 → 70,000,000 by exactly the transferred amount (a funding-time 73 bps skim would credit 9,927,000), vault token amount 60,000,000 → 70,000,000, source 40,000,000 → 30,000,000, FeeVault `collected_raw` still 0; window 14 snapshot and stock UNCHANGED by the mid-window deposit (caps read the period-start snapshot, never the live total); lower than slice-7 deposit 13,944 because the vault already exists, so no vault-create CPI runs |
| `exit:13accrued_par` (holder 27, 1,000,000, all accrued) | 10,608 | Success, gate PASS — accrued consumed, position stays 5,000,000, stock 10,000,000 → 9,000,000, FeeVault still 0; an all-accrued exit takes neither penalty nor fee (1 CU above the exit family's 10,607 — the same 1-CU repeat class as slices 10–12) |
| `exit:13excess_penalty` (holder 27, 1,000,000, all excess) | 10,607 | Success, gate PASS — penalty exactly the flat 13% (130,000; paid 870,000 by `can_exit`, which computes `(excess * PENALTY_PPM / PPM, amount - penalty)` with no `FEE_PPM` term, so no second skim exists to take), position 5,000,000 → 4,000,000, stock 9,000,000 → 8,000,000, FeeVault still 0 |

Stated, not passed off: the POSITIVE half of the fee flow is unbuilt. FeeVault
`collected_raw` is still 0 after every value move in this slice because `collect_fees`
(the remaining §4.2 row) does not exist yet — so the 73 bps income identity has no
on-chain realization, and the fee-flow class is only half re-expressed after this slice:
no double skim is possible today, and no fee income is collected either. That gap rides
with the `collect_fees` row, not hidden behind these passing probes. Final state: T and
cash 70,000,000, window_count 15, window 14 snapshot 60,000,000 / stock 8,000,000, window
13 stock 4,000,000, window 1 stock 0, whale schedule untouched (2 blocks,
last_block_time 31,608,400), clock 37,003,600. Golden file 278 entries: all 275 slice-12
entries reproduce exactly (program .so sha unchanged, `82759c51…`). No §2.2 revision:
both gated exits confirm the exit family's figure within 1 CU; deposit has no §2.2 EST.
As in every slice, `exit` here is bookkeeping, not token movement; `deposit` moves real
tokens via the SPL CPI.

## Slice 14: the remaining §4.2 row `reinvest` (design §4.2, engine-sim T8) (2026-10-26)

The first program change since the golden campaign's funded chain froze:
`reinvest` lands as a holder-signed instruction gated by a Committed
`treasury.window.v1` transition (IDL 37 → 38 instructions, 85 → 88 errors
— the three new errors appended at the end of the enum, 6085
`ReinvestMismatch` / 6086 `InvalidReinvestAmount` / 6087
`ReinvestExceedsAccrued`, so no existing error code moves — accounts 16,
program host tests 82 → 83 with the new `reinvest_gate_matrix`, new
`Reinvested` event). The reading implemented, stated plainly because the
design row is one line ("claims no cash; increases the position"): the
reinvested amount comes out of the holder's accrued unpaid dividend on
THIS record and into the same record's locked position — accrued falls
by exactly the amount, position rises by exactly the amount, nothing else
moves. A partial reinvest is allowed; an amount above the accrued
dividend refuses, never a partial reinvest of a larger request. No exit
stock is debited (the stock is the period budget for cash exits, and a
reinvest pays no cash), no penalty is taken (the 13% door prices cash
extraction above the allowance), and no TreasuryState total moves
(`pay_tranche` already accounted the dividend; reinvesting re-labels it
as position, it does not create value). Reinvesting a BORROWED amount
(Carlos's 2026-10-22 window-rule phrasing) is NOT this instruction: a
loan's principal is cash disbursed from the sleeve, and folding it back
into the locked position would tangle the loan's settlement against
that same position — that reading needs its own ruling and its own
instruction, and is not claimed here.

Measured on the slice-13 chain (holder 27's tranche is spent, so fresh
holder 29 enrolls in the still-current window 14, collects a 1,000,000
tranche on Committed window-flow transition 7, and reinvests it in two
parts):

| instruction | CU | result |
|---|---|---|
| `enroll:14holder29` (5,000,000, cohort 2) | 28,542 | Success, setup |
| `pay_tranche:14holder29` (1,000,000) | 11,712 | Success, gate PASS — exactly the slice-12 figure |
| `reinvest:14partial` (400,000 of 1,000,000 accrued) | 9,310 | Success, gate PASS — accrued 1,000,000 -> 600,000, position 5,000,000 -> 5,400,000, window 14 exit stock UNCHANGED at 8,000,000, TreasuryState UNCHANGED |
| `reinvest:14over_accrued` (600,001 of 600,000 accrued) | 9,979 | Refused `ReinvestExceedsAccrued` (6087), state unchanged, NOT a success CU |
| `reinvest:14rest` (600,000 of 600,000 accrued) | 9,310 | Success, gate PASS — accrued -> 0, position -> 6,000,000, stock still 8,000,000 |
| `reinvest:14zero` (0) | 9,944 | Refused `InvalidReinvestAmount` (6086), NOT a success CU |

Re-baseline, stated not hidden: adding an instruction re-measures slices
1–13 against the new .so (sha `e0f40adf…`, was `82759c51…`), and 112 of
the 278 prior entries shift by −4…+6 CU (code-layout jitter from the
larger binary) — with ZERO gate flips and no design-finding status
change. The §2.2 canonical figures are updated to the slice-14
measurements in the same pass (commit 17,959 → 17,963, borrow 12,158 →
12,162, schedule_whale_exit 15,841 → 15,839; pay_tranche 11,712, exit
10,607, settle_default 14,512, execute_whale_tranche 16,360 unchanged),
per Carlos's measured-figures directive. Final state: T and cash
70,000,000, window_count 15, window 14 stock 8,000,000, window 13 stock
4,000,000, window 1 stock 0, whale schedule untouched (2 blocks,
last_block_time 31,608,400), clock 37,003,600, FeeVault collected_raw
still 0 (collect_fees still unbuilt; reinvest touches no fees). Golden
file 284 entries. As in every slice, `reinvest` here is bookkeeping,
not token movement.

## Slice 15: the last §4.2 row `collect_fees` (design §4.2, D-5) (2026-10-26)

The §4.2 table is now complete: every row has an instruction. `collect_fees`
lands as an operator-settled instruction gated by a Committed
`treasury.window.v1` transition with the `OPERATOR_SETTLE` capability
(IDL 38 → 39 instructions, 88 → 91 errors — the three new errors appended
at the end of the enum, 6088 `CollectMismatch` / 6089
`CollectVaultMismatch` / 6090 `InvalidCollectAmount`, so no existing error
code moves — accounts 16, program host tests 83 → 84 with the new
`collect_fees_gate_matrix`, new `FeesCollected` event). The reading
implemented, stated plainly because the design row is one paragraph with
a gap in it: the operator reports the window's collected 73 bps total, the
FeeVault key must equal `Config.fee_vault`, and the amount accrues twice
— once on the window's new `fees_collected_raw` field (WindowState SPACE
121 → 129, appended after `bump` so every pre-existing field keeps its byte
offset) and once on the vault's `collected_raw` (the sum over windows). A
zero collection refuses; repeated collections in one window accumulate
(fees arrive through a window, not once); an agent without
`OPERATOR_SETTLE` refuses. The vault-mismatch case cannot be probed in
the harness — the vault account is PDA-bound, so a foreign vault fails at
the account layer — and is covered by the host gate matrix instead.

Measured on the slice-14 chain (window 14, Committed window-flow
transition 7, operator [7;32]):

| instruction | CU | result |
|---|---|---|
| `collect_fees:15first` (73,000 — 73 bps on a 10,000,000 flow, operator-reported) | 14,223 | Success, DESIGN FINDING — 2.03× the ~7K family EST point, 223 CU over the 14,000 band top: the instruction touches six accounts (operator, operator record, transition, config, window, vault) plus an authorization-context event, more than the thin shape the EST modeled; in the settle_default class, not a new shape. Window fees 0 → 73,000, vault 0 → 73,000, window 14 exit stock UNCHANGED at 8,000,000, TreasuryState UNCHANGED at 70,000,000 |
| `collect_fees:15second` (27,000 more in the same window) | 14,223 | Success, same finding — window and vault both 100,000, so the vault total is the sum over windows and the window figure is its own accounting |
| `collect_fees:15zero` (0) | 14,655 | Refused `InvalidCollectAmount` (6090), state unchanged, NOT a success CU |
| `collect_fees:15unauthorized` (agent [3;32], no `OPERATOR_SETTLE`) | 14,605 | Refused `Unauthorized` (6000), state unchanged, NOT a success CU |

Stated, not passed off: this is the accounting half, bookkeeping not token
movement, as in the sibling slices — and the design row's "reconciles the
vault against the declared total" is NOT implemented, because the
TransitionRecord carries only a `declared_hash`, not a declared fee total
to reconcile against. The event carries the transition id, commit index,
and declared hash — the authorization context settle's `exact` check
attests — but a caller-supplied amount that disagrees with the off-chain
declared schedule is not refused by this instruction. That numeric
reconciliation is a carried item (a declared fee schedule on the
transition, or an evidence record binding the reported total), not a
claim made here. The fee flow now has its positive half as on-chain
accounting; the 73 bps income identity's realization is the ledger total,
not yet a token settlement.

Re-baseline, stated not hidden: adding an instruction re-measures slices
1–14 against the new .so (sha `e139e22a…`, was `e0f40adf…`), and 123 of
the 284 prior entries shift — all small (mostly +2…+14 CU code-layout
jitter, up to +59 CU on window-touching instructions: `open_window`,
`enroll`, and `exit` each gain ~56–59 CU from the 8-byte larger
WindowState) — with ZERO gate flips and no design-finding status change
except the new collect_fees finding itself. The §2.2 canonical figures
are updated to the slice-15 measurements in the same pass (pay_tranche
11,712 → 11,722, borrow 12,162 → 12,174, exit 10,607 → 10,665,
settle_default 14,512 → 14,524, schedule_whale_exit 15,839 → 15,851,
execute_whale_tranche 16,360 → 16,414, tranche 2 16,369 → 16,423,
reinvest 9,310 → 9,322; commit 17,963 and settle 12,965 unchanged),
per Carlos's measured-figures directive. Final state: T and cash
70,000,000, window_count 15, window 14 stock 8,000,000 and window 14 fees
100,000, FeeVault collected_raw 100,000, window 13 stock 4,000,000,
window 1 stock 0, whale schedule untouched (2 blocks,
last_block_time 31,608,400), clock 37,003,600. Golden file 288 entries.
As in every slice, `collect_fees` here is bookkeeping, not token movement.

## Next slices

Queue head, continued: the W3.2 adversarial suite re-expressed against
the program's interface with measured CU per instruction is now complete
for the §4.2 rows — next the carried Part B4 items in roadmap order
(including the skip-enforcement item slice 11 confirmed open, the
open_window source-transition consumption fix, and the declared-fee-schedule
reconciliation this slice carries), then the remaining queue items
(deposit↔enroll binding, accepted-mints registry, native-SOL deposits).

Optimization pass step 2 (ATTEMPTED 2026-10-25, BLOCKED, REVERTED —
NOT blocked by the funding decision, blocked by the toolchain):
explicit-bump `init` was implemented across all 14 creating
instructions / 19 init accounts (bump args appended last in each
signature, handlers storing the arg instead of `ctx.bumps`, both
harnesses passing the off-chain-computed bump) — and pinned Anchor
0.31 rejects it at parse time: anchor-syn `constraints.rs`
unconditionally errors `bump targets should not be provided with
init` for any bump target on an `init` account (99 build errors, no
IDL produced). The anchor-derive-accounts doc example showing
`bump = bump` with `init` is stale for this version; the parser is
authoritative. The change was fully reverted: all three touched
files byte-identical to the delivered slice-6 zip, workspace
125/125, host tests 79/79, `anchor build` exit 0, IDL 36/17/78,
mollusk run reproduces all 70 golden values identically. The
per-PDA search cost (1,500 CU/attempt; attempts=1 baselines
`submit_signal` 17,144, `submit_plan` 17,715,
`register_agent` 9,927, `register_transition` 14,150,
`policy_transition` 10,698, `post_observation` 10,791,
`record_evidence` 12,413) therefore stands. What unblocks step 2 is
Carlos's call, not builder improvisation: (a) accept the search
cost, (b) an Anchor upgrade, or (c) hand-rolled account creation
via system-program CPI (drops Anchor's `init` checks — needs its
own design review). (The funding-path sentence that followed here is
superseded: the funding path LANDED 2026-10-26 as the deposit
instruction + token vault, and slice 7 above measures the
borrow / exit / settle_default / schedule_whale_exit SUCCESS
CU on it; only the execute_whale_tranche success — the
12-window walk — and the W3.2 re-expression remain queued.)
