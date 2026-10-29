# CU golden-test harness — program-v04 (mollusk path)

Measures real compute units per instruction for `program-v04` by
executing the real `.so` in-process (mollusk-svm 0.16.2) — not
estimates, not host-test timings. No validator, no devnet, no
mainnet, no real funds. The keys in the harness are throwaway
local-test keys.

## Slice 1 — genesis + governance registration (the cu-golden freeze, slice 1)

Chain as MEASURED: `probe_kernel`, `initialize_kernel`,
`seal_kernel`, `advance_clock` (100 -> 200),
`register_transition` (ordinary), `ingest_observation`
(observer `[0xA1;32]`, timestamp 100, expiry 10100, confidence
500,000), `register_governance_transition` (seat ag(1) =
`[1;32]`, `GOV_ADJUSTABLE`). Measured CU (mollusk-svm 0.16.2,
real `.so`, `.so` sha256 in the golden file):

| instruction | CU | result |
| --- | ---: | --- |
| probe_kernel | 9,795 | Success |
| initialize_kernel | 20,369 | Success |
| seal_kernel | 21,346 | Success |
| advance_clock (100 -> 200) | 4,317 | Success |
| register_transition | 11,872 | Success |
| ingest_observation | 28,013 | Success |
| register_governance_transition | 27,532 | Success |

State assertions held on every step (genesis unsealed/head
0/clock 0; sealed clock 100; clock 200; both records REGISTERED;
governance approval count 0).

BLOCKED, NOT MEASURED — `approve_governance` and
`revise_governance_approval` (slice-49 finding): under mollusk
both refuse with Anchor 3005 (`AccountNotEnoughKeys`,
attributed to `kernel_state`) before any handler gate runs,
despite correct account metas. Ten probes proved the failure
intrinsic to these two instructions under mollusk, not to
account state, meta order, payer identity, or data size
(reversed metas, minimal account vec, non-seat payer, fully
pre-initialized valid approval record, and a padded
`advance_clock` at 1516 bytes of data succeeding, all leave the
3005 unchanged; `replay_transition` on the same governance
record reaches its third account with 3012 as expected, and
truncated revise data gives the args-deserialize error 102
instead). The stack-frame execution check the freeze was queued
to carry (approve handler 5,760 bytes, revise 5,696, both over
the 4,096-byte limit) is therefore NOT closed by this slice: it
stays queued behind resolving the 3005. Both instructions are
recorded as `Blocked` in the golden file, never as fake
measurements.

DESIGN FINDING (independent of the mollusk 3005):
`approve`/`revise` carry the 1524-byte `StoredGovAction` by
value, so instruction data alone is 1564 bytes; with a 64-byte
signature that already exceeds Solana's 1232-byte transaction
size limit before any account key or blockhash, so neither
instruction could be submitted as a single real transaction as
currently encoded. The slice-33 by-value encoding choice also
explains the stack-frame overages. Needs a design decision
(compact action encoding or an account-carried action); not
redesigned in this slice.

All slice-1 values are BASELINE: no design estimate exists for
the v0.4 program (the design doc's §2.2 estimates are for the
v0.2 treasury program), so nothing here is gated against a band.

## Layout

- `mollusk/` — standalone Rust crate (its own `[workspace]`
  root, so the parent program-v04 workspace does not claim it).
  Loads `target/deploy/program_v04.so` (built by `anchor build`
  on the pinned toolchain: Anchor 1.2.1 / Solana 4.1.2 /
  platform-tools v1.54).
- `cu-golden-mollusk.json` — the golden file: measured CU per
  instruction, with the program `.so` hash and measurement
  environment it was measured against.
- `run-mollusk.sh` — runs the harness and writes the golden file.

## Setup

```sh
~/workspace/toolchains/anchor/bin/anchor build   # from program-v04/
./run-mollusk.sh   # from cu-golden/
```

## Later slices (not this slice)

The stage instructions join once their setup chains are
scripted: replay, evaluate, mark_eligible, submit_signal, select
(+ siblings), open_planning, submit_plan, reconcile, validate,
declare, commit, and the governance stage chain through
commit_governance. Nothing here claims those measurements.
