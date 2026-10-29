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

## Slice 50 — the 3005 ROOT CAUSE, proven (2026-10-29)

The slice-49 3005 is NOT a mollusk artifact and NOT an Anchor
bug: it is the program's own oversized BPF stack frame. The SBF
linker reports the `__global` wrapper frames at approve 5,760
bytes and revise 5,696 bytes, both over the 4,096-byte BPF stack
limit (build warnings the build still exits 0 on). A scratch
variant program proved the link by controlled experiment, all
against the same binary, same accounts, same data: a probe
instruction with fields byte-identical to
`ReviseGovernanceApproval` and the same `StoredGovAction` arg
but a trivial body loads accounts fine (fails later, 3012, on
the empty approval record, as it should); the same probe with
revise's exact body overflows identically (5,696 bytes) and
3005s identically (same CU, 5,384–5,386). The only variable is
the body, i.e. the frame size. Anchor-debug confirms the failure
sits inside the first field's `try_accounts` with an empty
slice: the wrapper's accounts slice is corrupted by the frame
overflow (BPF stack overflow is undefined behavior by the
linker's own warning; here it surfaces deterministically as a
zero-length slice, hence 3005 at `kernel_state`). Shape, data
size, meta order, payer identity, and account state were all
separately exonerated: `submit_signal` (same `init` +
payer-key-seed shape) and a 1,568-byte `submit_plan` load fine.

Consequences: (a) approve/revise cannot execute on-chain as
built — not just under mollusk — until their frames come under
4,096 bytes; (b) the tx-size design finding above stands
independently (1564 bytes of instruction data still exceeds a
real transaction), so the pending design decision (compact
action encoding or an account-carried action) is now the fix for
BOTH findings, and any fix must also be frame-measured, not
just tx-measured; (c) FORWARD WARNING: the sibling-select
wrappers `select_candidate_governance_with_siblings` and
`select_candidate_mixed_with_siblings` also overflow (4,224
bytes each) and are predicted to fail the same way when the
golden chain reaches them — the `try_accounts`-frame overflows
also reported (e.g. `ApproveGovernance::try_accounts` 4,480)
are NOT the discriminator (a fitting-wrapper probe whose
`try_accounts` overflows at 4,544 still loads), only the
`__global` wrapper frame decides it. The two instructions stay
`Blocked` in the golden file until a frame-fitting fix lands
and is re-measured; no fake measurements.

All slice-1 values are BASELINE: no design estimate exists for
the v0.4 program (the design doc's §2.2 estimates are for the
v0.2 treasury program), so nothing here is gated against a band.

## Slice 52/53 follow-up — the account-carried approval chain, MEASURED (2026-10-29)

The slice-49/50 block is CLOSED by runtime proof, not just by
frame measurement. The harness chain now runs, against the real
`.so` (sha256 `cb2bf957…`, pinned-toolchain `anchor build`,
exit 0), the full account-carried sequence the program slices
51–53 built: `init_gov_action` (writer seat1, expected hash =
SHA-256 of the serialized action), three `append_gov_action_chunk`
calls (512 + 512 + 500 = 1,524 bytes, sequential-prefix rule),
`finalize_gov_action` (on-chain hash, write-once seal), the
slice-52 `register_governance_transition` (finalized action
account riding between record and payer), `approve_governance`,
and `revise_governance_approval`. The action is a hand-serialized
`GrantRole` (Guardian) to `[0x99;32]` in the fixed
`StoredGovAction` Borsh layout; its category is `GOV_ADJUSTABLE`,
matching the record type. State assertions held on every new
step (finalize stores the expected hash with 1,524 bytes written
and sealed; approval count 0 → 1 on approve, still 1 after
revise; the approval record binds the finalized action hash
through both approve and revise — revise re-binds the same
sealed action, the only action this transition id can carry).

Measured CU (mollusk-svm 0.16.2, same `.so` as the re-measured
slice-1 chain — the slice-1 values below shifted slightly from
the stale golden file because that file predated the slice-52
registration gate and the current `.so`):

| instruction | CU | result |
| --- | ---: | --- |
| probe_kernel | 7,705 | Success |
| initialize_kernel | 17,756 | Success |
| seal_kernel | 18,028 | Success |
| advance_clock (100 -> 200) | 4,158 | Success |
| register_transition | 11,395 | Success |
| ingest_observation | 24,383 | Success |
| init_gov_action | 8,427 | Success |
| append_gov_action_chunk (each of 3) | 4,660 | Success |
| finalize_gov_action | 7,285 | Success |
| register_governance_transition | 26,100 | Success |
| approve_governance | 37,577 | Success |
| revise_governance_approval | 32,917 | Success |

The golden file's `blocked` list is now empty and its
`design_finding` is replaced by `design_finding_resolved`: the
by-value 1,564-byte instruction-data finding is gone (the
largest new instruction is a 512-byte chunk), and the wrappers
that 3005'd at 5,760/5,696 bytes now execute. Remaining
carried debts are unchanged: genuine Agave 4.3.2 / Anchor 1.2.2
builds, cu-golden for the slice-51 instructions beyond this
chain's init/append/finalize (close is unmeasured), the
post-settle reclaim decision, and the golden-`.so` sha debt —
this run's `.so` (`cb2bf957…`) is the freshly built current
source, not the stale slice-49 golden sha.

## Slice 3 — close_gov_action MEASURED (2026-10-29)

The last unmeasured slice-51 instruction is now measured, against
the same `.so` (`cb2bf957…`, reproduced byte-identical from a
fresh pinned-toolchain build this run) with all slice-2 values
reproduced exactly. The chain extends past revise with a second,
abandoned action account (transition id `[0xD3;32]`, writer seat1):
`init_gov_action` + one 512-byte chunk, never finalized, then
`close_gov_action` — plus the lifetime gate, a close attempt on
the finalized GOV account.

| instruction | CU | result |
| --- | ---: | --- |
| init_gov_action (abandoned account) | 14,427 | Success |
| append_gov_action_chunk (abandoned, @0) | 4,660 | Success |
| close_gov_action | 3,903 | Success |
| close_gov_action (finalized account) | 5,328 | Refused — GovActionAlreadyFinalized (6034) |

State assertions held: the abandoned account starts unfinalized
with exactly one chunk written; close returns its full rent to
the writer (writer lamports rise by exactly the account's
pre-close lamports) and empties the account; the refused close
leaves the finalized account sealed with its hash untouched —
the finalized-stays-open lifetime rule is now runtime proven,
not just host-tested. Refusal CU lives in the golden file's new
`refusals` section, never as a success measurement.

OPEN OBSERVATION (not a gate — both values are baselines): the
second `init_gov_action` in the same chain costs 14,427 CU
against 8,427 CU for the first, a deterministic +6,000 delta
reproduced across two runs, with no identified cause yet (same
account shape, same payer, same 3-account instruction). Flagged
here rather than smoothed over; a future slice may isolate it.

With this slice, every slice-51 carrier instruction is measured
(init / append / finalize / close) and the slice-51 CU set is
closed. Remaining carried debts: genuine Agave 4.3.2 /
Anchor 1.2.2 builds, and the post-settle reclaim decision (close
after settle is still unbuilt — this slice proves only the
abandoned-account reclaim and the finalized refusal). The
golden-`.so` sha debt below is CLOSED by the freeze section
that follows.

## Golden `.so` freeze (2026-10-29)

> **SUPERSEDED 2026-10-29 by Carlos's pin-migration ruling
> (~11:45 CST): migrate to Agave 4.3.2 / Anchor 1.2.2 /
> mollusk 0.16.4 BEFORE the golden freeze, and freeze once
> under the new pins. The sha below is the OLD-pin
> (1.2.1 / 4.1.2 / 0.16.2) freeze only — do not cite it as
> the golden after the migration pass; the migration pass
> re-freezes under the new pins with the four ruled
> conditions (pinned lockfile + CI fails on fresh
> resolution; harness label fixed with a stamp==resolved
> check; frames re-measured under the new toolchain; pins +
> figures recorded together in TOOLS.md + OWN_IDL_DESIGN
> §2.2). Sequence as ruled: migrate + re-measure frames
> → post-settle reclaim decision → golden freeze once.**

**FROZEN (old pins) — program_v04 `.so` sha256
`cb2bf9577169737befb4c68ce12aa0e11d8978a6d4fdb8d6d375f1d22662547d`.**
This is the binary the slice-2/slice-3 golden CU values were
measured against, now frozen rather than merely fresh:

- Source: the slice-53 program source (pushed as `dde06b67`
  on branch `kernel-v04`); this run verified the staging
  `lib.rs` byte-identical to the branch-staging copy.
- Reproducibility proof, this run: a forced rebuild
  (`touch` on `lib.rs`, pinned-toolchain `anchor build`,
  Anchor 1.2.1 / Solana 4.1.2 / platform-tools v1.54) produced
  a byte-identical `.so` — same sha, same 1,663,024 bytes.
  The known pinned overflow-list item (the pre-existing
  BTreeMap-insert frame, 4,672 bytes) still reports during the
  build and does not change the output.
- Measurement proof, this run: the mollusk harness re-run
  against the rebuilt `.so` reproduced all 17 measured CU
  values, the finalized-close refusal (5,328 CU, 6034), and
  the empty `blocked` list exactly; the regenerated golden
  JSON is text-identical to the slice-3 file.
- The binary itself is NOT committed: the snapshot push path
  accepts UTF-8 files only, so the freeze is this sha plus
  the reproducibility proof above. Reproduction is
  path-sensitive, stated plainly: the proof above rebuilds
  from the same staging directory, and oversight's scratch
  copy of identical source built a different sha
  (`7c9ab3fb…`, roadmap 10:2x entry — attributed there to
  embedded build-directory paths, source diff clean, CU
  reproduction exact). So the frozen sha is
  pinned-toolchain-plus-canonical-path, not toolchain alone:
  a mismatch from a same-path rebuild is a stop; a mismatch
  from a different build directory is a procedure difference
  to re-check from the canonical path first, not automatic
  proof of source drift.
- Freeze rule going forward: before trusting any CU run,
  check `sha256sum target/deploy/program_v04.so` against the
  frozen sha above. A source change requires an explicit
  re-freeze (new sha + full re-measurement), never silent
  drift — the golden file's `so_sha256` field is the check,
  and a mismatch is a stop, not a rounding error.
- Carried caveat, not smoothed: slice 51 recorded a golden
  sha (`1de0c0a2…`) that two pinned-toolchain rebuilds did
  NOT reproduce (`407907c8…` twice). That debt belonged to
  the slice-51 build context and source state; for the
  slice-53 source the pinned rebuild reproduces, as proven
  above. If a future rebuild ever fails to reproduce the
  frozen sha, that is a new finding to record, not a value
  to overwrite quietly.

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
