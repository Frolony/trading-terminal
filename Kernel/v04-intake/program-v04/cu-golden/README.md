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

## Pin migration (2026-10-29, Carlos's migration-before-freeze ruling)

Program-v04 now builds and measures under **Agave 4.3.2 /
Anchor 1.2.2 / mollusk-svm 0.16.4** (branch `kernel-v04` only;
the main program still runs on 4.1.2 / 1.2.1 / 0.16.2 until its
own migration pass, and its OWN_IDL_DESIGN §2.2 figures are
therefore NOT updated by this pass — v04 figures live here,
not there). The four ruled conditions, as landed:

1. **Pinned lockfiles committed.** Program `Cargo.lock`
   (anchor-lang 1.2.2) and harness `Cargo.lock`
   (mollusk-svm 0.16.4, solana-program-runtime held at 4.3.2)
   are pushed with the manifests; the harness runs
   `cargo run --locked --offline`, so any fresh resolution
   fails the run instead of silently re-measuring under a
   different tree. Mollusk 0.16.4 builds ONLY on the pinned
   4.3.x lock — a fresh 4.4.0 lock fails (its `new_builtin`
   two-argument call was removed in 4.4.0) — so the pinned
   route (`cargo update -p mollusk-svm --precise 0.16.4` on
   the existing lock) is the only adoption route.
2. **Harness label fixed, stamp==resolved enforced.** The
   golden JSON now stamps `mollusk_svm 0.16.4` and
   `toolchain "Anchor 1.2.2 / Solana 4.3.2 / platform-tools
   v1.57"` from constants, and the harness reads its own
   `Cargo.lock` at startup and aborts if the resolved
   mollusk-svm version differs from the stamped version —
   the 0.16.2-stamp-under-0.16.4 gap cannot recur. The
   negative case was proven this pass: a scratch harness
   copy stamped 0.16.4 against a 0.16.2 lock aborted before
   any measurement was written.
3. **Frames re-measured under the new toolchain, not just
   CU.** `anchor build` (1.2.2 CLI, v1.57) overflow list at
   4,096: exactly one entry, the pre-existing BTreeMap-insert
   helper at 4,672 bytes — no instruction wrapper overflows.
   The 2,048-byte threshold build (`cargo build-sbf
   --sbf-stack-size 2048`, complete 47-function overflow
   list, same count as the slice-53 probe) lists exactly
   four `__global` wrappers — the slice-51 gov-action
   lifecycle wrappers (init 3,520; close 3,456; finalize
   3,456; append 3,584) — and NO approve, revise, register,
   commit, or sibling-select wrapper: each fits 2,048 bytes,
   so the with-margin bar still holds under 1.2.2/v1.57.
   (The one `commit_governance` hit in the threshold log is
   the `commit_governance_from_records` helper at 3,200
   bytes, not an instruction wrapper.)
4. **Pins + figures recorded together.** Pins in TOOLS.md
   (v04-line section) and here: new `.so` sha
   `3afca38f4adea2f6af776d302233fe30471ffcb437a8f2c7badb58780998699c`
   (canonical staging path; IDL byte-identical to the pinned
   build, sha `967c43ee…`, 38 instr / 7 accounts / 39 errors /
   23 events; host tests 392/392 under anchor-lang 1.2.2).
   Golden re-measured against that `.so` (17 measured,
   blocked `[]`): probe 7,705; initialize 17,772; seal 18,045;
   advance_clock 4,175; register_transition 11,415;
   ingest_observation 24,403; init_gov_action 8,447; append
   4,681 each; finalize 7,306; register_governance_transition
   26,120; approve 37,632; revise 32,973; abandoned second
   init 14,447 (the +6,000 observation, unchanged in shape);
   close 3,913; finalized-close refusal 5,332 (6034). Every
   value matches the Anchor 1.2.2 scratch assessment exactly.
   **This `.so` is NOT the golden freeze** — the freeze
   happens once, after the post-settle reclaim decision, per
   the ruled sequence; until then `3afca38f…` is the
   migrated-build sha, not a frozen sha.

## Slice-54 re-measurement (2026-10-29, same pins)

Slice 54 (the settle helper, no new instruction) changed
the program source, so the golden file was re-measured
against the new `.so` sha
`cb5bec94c3ae3c142a4df858939704721f3ea46b581d2c7f052bc633bd8f1fc1`
(host tests 402/402; IDL now 38 instr / 7 accounts /
41 errors / 24 events — the two new settle errors and the
`TransitionSettled` event). Drift from the migration
values is layout noise, not behavior: append
4,681 -> 4,682 (all four append entries), approve
37,632 -> 37,629, finalized-close refusal 5,332 -> 5,334;
the other fourteen values reproduce exactly, blocked `[]`
still. This `.so` is likewise NOT the golden freeze.

## Slice-55 re-measurement (2026-10-29, same pins)

Slice 55 (the governance settle mirror, no new
instruction) changed the program source, so the golden file
was re-measured against the new `.so` sha
`a263117a671f1ff7923d13b35052364502b4840d2c9a6230993559150fb8d150`
(host tests 412/412; IDL now 38 instr / 7 accounts /
42 errors / 24 events — the one new
`GovernanceSettleFailed` error; no new instruction,
account, or event). Drift from the slice-54 values is
layout noise, not behavior: append 4,682 -> 4,683 (all
four append entries), approve 37,629 -> 37,628; the other
twelve values reproduce exactly, blocked `[]` still.
This `.so` is likewise NOT the golden freeze.

## Slice-56 re-measurement (2026-10-29, same pins)

Slice 56 (the `SettlementEvidence` account, the
`record_settlement_evidence` instruction, and the
`settle_transition` instruction) changed the program source,
so the golden file was re-measured against the new `.so` sha
`9bc4f6c25f01d5456cb1f0d0f3422ded0e67310e559296a6df57934a055e04af`
(host tests 418/418; IDL now 40 instr / 8 accounts /
42 errors / 25 events — two new instructions, the new
evidence account, and the `SettlementEvidenceRecorded`
event; no new error, the binding reuses `RecordMismatch`).
Drift from the slice-55 values is layout noise, not
behavior: probe 7,705 -> 7,707, register_transition
11,415 -> 11,417; the other fifteen values reproduce
exactly, blocked `[]` still. This `.so` is likewise NOT
the golden freeze.

## Slice-57 re-measurement (2026-10-29, same pins)

Slice 57 (the `settle_governance_transition` instruction
and its `from_stored` entry, no new account, error, or
event) changed the program source, so the golden file was
re-measured against the new `.so` sha
`3edd6f2bf608af13c9f27756a6309637c8b90abd8c67822aeae4af688ac89918`
(host tests 422/422; IDL now 41 instr / 8 accounts /
42 errors / 25 events — one new instruction; no new
account, error, or event). Drift from the slice-56 values
is layout noise, not behavior: probe 7,707 -> 7,705,
initialize 17,772 -> 17,776, register_transition 11,417
-> 11,415, ingest_observation 24,403 -> 24,401,
init_gov_action 8,447 -> 8,449, abandoned second init
14,447 -> 14,449; the other eleven values reproduce
exactly, blocked `[]` still. This `.so` is likewise NOT
the golden freeze.

## Slice-58 re-measurement (2026-10-29, same pins)

Slice 58 (the Rule-A close-refusal proof test + doc
comments only, no behavior change) changed the program
source, so the golden file was re-measured against the
new `.so` sha
`8e8d07d3d68449262c8e8d94ea8cb77c460f9c8fdd3b184d1501020d7d64cb7e`
(host tests 423/423; IDL unchanged at 41 instr /
8 accounts / 42 errors / 25 events). All seventeen values
reproduce exactly (finalized-refusal still 5,334),
blocked `[]` still — the `.so` hash moved on a
comment / test-only source change at identical size
(1,789,112 bytes), with zero CU drift. This `.so` is
likewise NOT the golden freeze.

## Slice-60 re-measurement (2026-10-29, same pins)

Slice 60 (the `record_outcome_governance` instruction,
governance path, + `record_outcome_governance_from_records`
helper + 10 host tests) changed the program source, so the
golden file was re-measured against the new `.so` sha
`d1d4fab7cc1afd92904e3f7c80d693fa4ff65db93529b974521d9f54df87003b`
(host tests 443/443; IDL now 43 instr / 8 accounts /
45 errors / 26 events — one new instruction, no new
account, error, or event). Zero drift from the slice-59
values: all seventeen measured CU values reproduce
exactly, blocked `[]` still. This `.so` is NOT the golden
freeze.

## Slice-66 — F6 CLOSED, full lifecycle chain measured (2026-10-30, same pins)

Slice 66 is the F6 direction-(c) fix, and it completes the
slice-65 chain. The program now carries a custom 256 KiB SBF
heap allocator (`PROGRAM_ALLOCATOR`, `custom-heap` default
feature — see the slice-66 comment at the allocator site in
`programs/program-v04/src/lib.rs`); direction (a), a larger
transaction heap frame against the default allocator, was
closed negatively the previous run because entrypoint 3.1.1
hardcodes its default allocator to 32 KiB. This harness
requests the matching 256 KiB heap frame for every
instruction (`mollusk.compute_budget.heap_size`), which is
the client contract, not a harness-only tweak: the bump
allocator allocates top-down from the end of its range, so
a smaller frame faults on the first allocation. Golden
re-freezes as slice 66 (`.so` sha `791a8be6…`, host tests
457/457, IDL shape unchanged 44 / 9 / 46): all 21 prior
values shift by exactly +56 CU, the heap-cost delta of the
256 KiB frame, and both refusals shift the same +56
(5636 / 5391) — no other drift, so the allocator change
costs nothing beyond the frame itself. The 16 new
measurements complete the lifecycle: `select_candidate`
**31,761 CU**, `open_planning` **32,344 CU**,
`submit_plan` **40,676 CU**, `reconcile_plans`
**38,181 CU**, `validate_transition` **38,409 CU**,
`declare_transition` **39,486 CU**, `commit_transition`
**44,854 CU** (head moves to 1 at the declared hash),
`record_settlement_evidence` **15,102 CU**,
`settle_transition` **46,565 CU**, `init_reputation`
for ag(2) **23,453 CU**, `record_outcome` **67,674 CU**
— the record reaches EVALUATED_OUTCOME (stage 12) with
the exact reputation tail ag(2) 50,000,000 -> 50,000,010
asserted. `blocked` is `[]` again. The governance outcome
tail (`record_outcome_governance`) is still unmeasured —
it needs a settled governance record, a later slice's
chain, not this one.

## Slice-67 — 75% heap tripwire test, SBF-side and wrapper-inclusive (2026-10-30, same pins)

Slice 67 adds the tripwire guard for F6 condition (1) without
changing program behavior. `programs/program-v04/src/lib.rs`
gains a cfg'd `PROGRAM_HEAP_LENGTH` and `Cargo.toml` a
`tripwire-heap` feature (never default, never shipped): with
it, `anchor build -- --features tripwire-heap` builds the same
program with the bump allocator capped at 196,608 bytes —
exactly 75% of the shipped 256 KiB cap. `run-tripwire.sh`
builds that variant, runs the FULL golden chain against it,
then restores the shipped `.so` (trap-guarded, sha-verified).
A chain that succeeds under the capped allocator has proven
every instruction's total SBF heap use under the tripwire,
because Anchor wrapper deserialize, handler replay, and
account serialize all allocate through the same bump
allocator — this closes the host-measurement wrapper gap
from the 03:01 stage-total run (which had to assume a
wrapper allowance) for the chains the golden currently
carries. Result this run: **tripwire NOT tripped** — all 37
instructions Success under the 196,608-byte cap (tripwire
`.so` sha `698a2ed7…`), all CU byte-identical to the shipped
golden, both refusals identical, shipped `.so` restored
(sha `449912de…`). Honest scope bound: the golden currently
carries the governance registration chain and the ordinary
life chain at the genesis head — it does NOT yet carry a
governance stage chain or a moved-head chain, so this test
guards those chains only once they land in the golden (the
governance outcome tail and second-commit chains are the
next queue items, and they will inherit the tripwire run
automatically because it re-runs the whole golden file).
The shipped golden re-freezes CU-identical under slice 67
(all 37 CU, both refusals unchanged; IDL shape unchanged
44 / 9 / 46 / 26); only the `.so` hash moves, with source
line numbers, which is why CU — not the hash — is the
slice-67 invariant. Run evidence:
`hidden_files/tripwire-golden-2026-10-30-0605.json`.

## Slice-68 — deterministic too-small-frame failure test, F6 condition (3) (2026-10-30, same pins)

Slice 68 closes F6 condition (3) with `run-smallframe.sh`, in two
complementary modes — a too-small frame must fail DETERMINISTICALLY
(same instruction, same error, same CU, twice), never silently
succeed and never corrupt state:

- **Cap mode** (`small-heap` feature, allocator capped at 32,768 bytes
  — the old default frame size — runtime frame still 256 KiB): the
  chain runs exactly as on the shipped build through 25 successes
  and the 2 expected refusals (genesis, governance registration,
  and the life chain through `submit_signal_life`), then fails at
  `select_candidate_life` with
  the program log `Error: memory allocation failed, out of memory`
  (mollusk `UnknownError(ProgramFailedToComplete)`, CU 28,033).
  Both runs identical, and every CU value identical to an earlier
  independent cap build — the failure point is a clean OOM at a
  known stage, and the harness's per-step account assertions rule
  out silent corruption behind it.
- **Frame mode** (shipped binary, `CU_HEAP_SIZE=32768` — the harness
  override added this slice; no rebuild): the chain fails on the
  FIRST instruction, `probe_kernel`, with `Access violation writing
  8 bytes at address 0x30003ffc8 (in unallocated region)` after
  only 199 CU consumed — the top-down bump allocator's first
  allocation landing outside the mapped 32 KiB region, exactly as
  the slice-66 contract comment states. Both runs identical
  (reported CU 1,400,000 is mollusk's fault-path accounting, not
  consumption), no golden file written, shipped `.so` untouched
  (sha `0125cf1e…` before and after).

Stale-build guard (found this slice, applied to both scripts): a
second consecutive variant build with only the feature flag changed
reused the shipped artifact (capped `.so` sha == shipped sha, chain
then "passed" against the shipped binary). Both `run-smallframe.sh`
and `run-tripwire.sh` now touch `programs/program-v04/src/lib.rs`
before the variant build and fail fast if the built `.so` still
equals the shipped one — a test that silently runs against the
wrong binary reports a pass it never earned. Slice 67's tripwire
result is unaffected by this finding: its build followed a source
change (so it recompiled) and its tripwire `.so` sha differed from
shipped as required. Honest process note: this slice's
guard-forced cap build was interrupted in anchor's host phase by a
killed background session, so the cap evidence comes from that
build's completed SBF artifact (sha `23331d17…`, tested directly
twice) plus the earlier independent cap build (sha `6bed7008…`) —
all CU values identical across both artifacts and duplicate runs.
The committed script performs build → sha-differ assert → two runs
→ trap-guarded restore as one flow; frame mode ran end-to-end
through it this slice. The shipped golden re-freezes CU-identical
under slice 68 (all 37 CU, both refusals unchanged; IDL shape
unchanged 44 / 9 / 46 / 26); only the `.so` hash moves, with source
line numbers. With conditions (3) and (4) closed here (TOOLS.md
heap-frame contract updated in the same pass) and conditions (1)
and (2) already closed, the slice-66 allocator choice and its
test guards are final; the governance outcome tail and
second-commit chains inherit both guards automatically when they
land in the golden, because both scripts re-run the whole file.
Run evidence: `hidden_files/smallframe-cap2-run1.log`,
`hidden_files/smallframe-cap2-run2.log`,
`hidden_files/smallframe-frame-run1.log`,
`hidden_files/smallframe-frame-run2.log`,
`hidden_files/smallframe-frame-script.log`.

## Slice-69 — governance outcome tail + second-commit-at-moved-head chain (2026-10-30, same pins)

Slice 69 lands both remaining queue items in one chain, with no
program change (harness only, `.so` sha unchanged `0125cf1e…`,
host tests 457/457): the golden grows from 37 to **63 measured
+ 2 refusals**, all 37 prior CU values reproducing exactly.

Why a second governance chain, not the slice-1 one: the slice-1
governance record was registered at the genesis parent (0, zero
hash), and commit requires parent == current head — after the
life chain moved the head to 1, that record can never commit.
GOV2 registers at the moved head (1, DECLARED_HASH) instead and
runs replay through outcome while the life record still stands
COMMITTED: the prior-bundle driver requires the prior record at
exactly COMMITTED, so the whole gov2 chain sits between
`commit_transition_life` and `settle_transition_life`, carrying
the life bundle (life record, life observation, life signal, life
plan, in bundle order) as its one prior on every stage
instruction. Unanimity needs all three council seats
(ag(1)/ag(2)/ag(3)), each signing its own approval record; the
plan is the canonical empty-claims governance shape (a rule
change claims no stock); the outcome attributes ag(2), the plan
agent, +10 exact. Clocks: mark_eligible at 400 (window 400/500),
select/open at 500 (deadline 600), reconcile through outcome at
600. `init_reputation_life` moved to just after `commit_life`
(same label, same 23,453 CU) so the gov2 outcome can attribute
ag(2); the life outcome then credits ag(2) a second +10
(50,000,010 -> 50,000,020). The life settle's head assertion
moves with the chain: settle moves neither head nor ledger, but
the head is now index 2 at the gov2 declared hash, not index 1.

New measurements (CU): init_gov_action 11,505; append chunks
4,740 each; finalize 7,364; register_governance_transition
30,674; approve seat1 36,189, seat2 45,400, seat3 36,419;
ingest_observation 22,959; replay_governance 66,559;
evaluate_governance 66,936; mark_eligible_governance 69,065;
submit_signal_governance 78,445; select_candidate_governance
73,636; open_planning_governance 74,264;
submit_plan_governance 82,265; reconcile 78,960; validate
79,233; declare 79,912; commit_governance 87,074 (head moves to
index 2 at the gov2 declared hash); record_settlement_evidence
13,602; settle_governance_transition 90,398;
record_outcome_governance 111,307 (gov2 record to
EVALUATED_OUTCOME, ag(2) 50,000,000 -> 50,000,010). Two variance
notes, both the slice-6 PDA bump-search mechanism, not
behavior: seat2's approval costs ~9k more than the other seats'
(the approval PDA's bump search length differs per seat), and
the gov2 evidence costs 1,500 less than the life evidence (one
fewer bump attempt on its PDA). The tripwire and smallframe
scripts re-run this whole 63-step file by construction, and
both guards now cover the gov2 chain by execution, not just
by construction.
Guard status (2026-10-30, BUILD run scheduled 09:55 CST):
the guard re-runs + deploy `.so` restore LANDED this run.
The sandbox write refusal had cleared (small writes succeed
again in both `/tmp` and the workspace), but the two
`/tmp` shipped-`.so` backups from the 08:46 run were gone
(`/tmp` is ephemeral — a backup kept only there is not a
backup; a workspace copy now lives at
`hidden_files/program-v04-rebuilt-2026-10-30.so`). The
deploy `.so` was therefore REBUILT from the unchanged
program source (`lib.rs` blob `2f9ead51…`, identical to the
slice-69 push): `anchor build` exit 0, and the rebuilt `.so`
hashes EXACTLY to the golden's `so_sha256`
(`0125cf1e…`) — the SBF build is byte-deterministic here,
so this is a true restore, not a rebuild-and-hope. The
golden chain re-run against it is byte-identical to the
frozen golden (65/65). Tripwire (75% heap cap, capped `.so`
sha `05854b37…`): full chain succeeds, measured + refusals
identical to the golden (only `so_sha256` differs, as
designed). Smallframe cap (32 KiB allocator): deterministic
failure at `select_candidate_life`, cu 28,033, identical on
both runs. Smallframe frame (32 KiB requested frame on the
shipped binary): deterministic failure at the first
instruction `probe_kernel`, cu 1,400,000, identical on both
runs. After every guard the shipped `.so` was restored
sha-verified (`0125cf1e…`). Queue after this slice: W4.4
devnet deploy (go stands, devnet/test assets only) — the
guard precondition on the 63-step golden is now met.

Adoption re-freeze (2026-10-30, BUILD run scheduled 11:27
CST): the v0.4.2 adoption + devnet identity, staged since
the 09:55 run's blocker note, are now verified and frozen.
Kernel 0.4.1 -> 0.4.2 is allocation-only (Box<Rec>
transitions, Cow reason; staging crate byte-identical to
the pristine v042 intake, 40/40 tests incl. the new
alloc_profile test) and `declare_id!` moves from the
retired staging-only Fzxn… id (keypair deleted) to the
fresh devnet-only program id 2CQ58… (keypair at staging
`deploy-keys/program-v04-devnet-keypair.json`, never
pushed; payer keypair alongside it, pubkey FGWF…;
Anchor.toml documents both, staging-local only). The
deploy `.so` for this binary (sha `b47ea2fa…`) postdates
every source edit (lib.rs 16:51, kernel 17:10, build
finished 17:30 UTC) with no program source newer than it,
and the full chain passes on it with identical outcomes
(63 Success + 2 refusals, all asserts incl. the
50,000,020 outcome credit green; program host tests
457/457). 49 of 65 CU values move vs the slice-69 golden
— expected under the slice-6 PDA bump-search mechanism:
on-chain PDA derivation cost depends on the program id,
so CU is not comparable across program ids. This is an
INTERIM re-freeze (new baseline for the devnet-id
binary), not the merge-gate final freeze. The
tripwire/smallframe guards proven above were run on the
prior binary and are PENDING re-run on this binary —
queue item before the final freeze, after the
0/1/2/3/5-prior fixture and chain-length enforcement.
The prior golden is preserved at
`hidden_files/cu-golden-slice69-fzxn.json`. Queue after
this re-freeze: 0/1/2/3/5-prior chain-length fixture,
then chain-length enforcement, then the single final
freeze (merge-gate item 4); W4.4 devnet deploy rides the
final binary, not this interim one.

## Slice-65 PARTIAL — full lifecycle chain stops at select (F6, 2026-10-30, same pins)

Slice 65 extends the slice-64 chain past replay at clock 200
and measures three more stage instructions in-process:
`evaluate_transition` **25,872 CU** (stage 2),
`mark_eligible` **28,179 CU** (stage 3, window 200/300), and
`submit_signal` **35,311 CU** (round-0 signal by ag(2), raw and
support SCALE, signal_count 1) — all Success, all prior
measurements reproducing exactly. The chain then STOPS:
`select_candidate` panics in-process at 27,977 CU with
"Error: memory allocation failed, out of memory" — finding
F6 (roadmap Part B4): the F4 mechanism generalizes, because
select's replay (ingest + register + evaluate + mark_eligible
+ signal replay + select) is longer than replay's, and the SBF
heap does not reclaim dropped memory within an instruction.
Host counting-allocator measurement proves the shape: select
replay peaks at 27,837 bytes live (fits 32 KiB) but allocates
31,237 bytes total, max single allocation 10,048 bytes — the
cumulative total plus Anchor/SBF wrapper overhead is what
overflows. Host select succeeds (life Selected, stage 4), so
this is a heap-budget finding, not a gate or kernel failure.
Per this harness's invariant (a golden file is never written
from a failed chain), the golden file stays slice 64 and the
slice-65 figures live in the run log
`hidden_files/kernel-build-run-2026-10-30-0152.md` until the
F6 fix lets the chain run to completion. The slice-65 harness
code (evaluate through record_outcome, scripted and asserted)
stays in `mollusk/src/main.rs` in staging for the next run
to resume from; the program `.so` is unchanged since slice 64
(sha `97190530…`, host tests 457/457).

## Slice-64 F4 fix + first in-process replay (2026-10-30, same pins)

Slice 64 is the F4 fix slice (roadmap Part B4): the stage
helpers no longer build two kernels per instruction. The new
`resolve_proposer_on_kernel` helper judges the proposer on
the single rebuilt head kernel every `*_from_records` helper
and chain helper already builds — 27 sites rewired
(24 stage helpers + `drive_prior_commit_to_commit` +
both `replay_*_transition_chain` helpers) — instead of
`resolve_proposer` building a second sealed kernel whose
memory SBF heap accounting never reclaims within the
instruction. The old sealed-genesis helper is kept for
callers that build no other kernel (host probe tests), and
a new host test guards that the on-kernel form judges
exactly as the sealed form on a genesis kernel. Host tests
457/457 (456 prior + the new guard), `anchor build` exit 0,
`.so` sha
`97190530dca4a1885cc8a3ca80d686285f1962ac3b312df8aa5aa88d0bcbc76e`.
The slice-63 chain now continues past ingest:
`replay_transition` for the lifecycle pair — **25,435 CU**,
Success, record advanced to KERNEL_REGISTERED — the first
stage instruction ever to execute in-process. All prior
measured CU reproduce EXACTLY, both refusals unchanged,
`blocked` is `[]` again. Semantic note, stated plainly:
proposer resolution now judges the rebuilt head roster, not
sealed genesis — a governance-registered proposer resolves
where sealed-genesis resolution refused it, with the kernel's
own registration gate, judging the same rebuilt kernel, as
the backstop. (The `open_planning` replan path still builds
a second kernel for its `is_seated` check — a residual
double-build in that path only, named here, not fixed in
this slice.) The full lifecycle chain (evaluate through
settle to an outcome with a reputation tail) is the next
slice's work. Clippy note: program `--all-targets` clippy
under host clippy 1.99 fails on pre-existing lints
(Anchor-derive `anchor-debug` cfg values, arity/complex-type
at untouched fns) — none introduced by this slice; the
slice's own doc-list lints were fixed in-slice.

## Slice-63 partial lifecycle chain + F4 block (2026-10-29, same pins)

Slice 63 adds a NEW ordinary transition's register + ingest
to the harness — the slice-1 ordinary record cannot replay
(it names observation `[0xC0;32]`, never ingested):
`register_transition` for TT `[9;32]` with proposer ag(1)
— **9,915 CU** — and `ingest_observation` for
`[0xC2;32]` (timestamp 200, expiry 9100, confidence
900,000) by the launch observer — **22,903 CU**. All 18
prior measured values reproduce EXACTLY, both refusals
unchanged (5580 / 5335), `.so` sha unchanged
`1518d41e154ac52d3b919901112c55812d800276b6d710b5f857f0661b5a802d`
(host tests 456/456; IDL 44 / 9 / 46 / 26, unchanged).
The chain stops there: `replay_transition` is BLOCKED
(roadmap F4) — it panics in-process with the program log
"Error: memory allocation failed, out of memory" at
~36,717 CU, reproduced in a minimal probe at the default
32 KiB heap AND at mollusk's maximum configurable 256 KiB
heap (512 KiB is rejected by the VM as "invalid heap
size"). A panic is not a refusal, so it lives under
`blocked`, never under `refusals` or `measured`. No stage
instruction has executed in-process before: every prior
slice measured only genesis / registration / approval /
close chains, none of which rebuild the sealed kernel the
way the stage helpers do. The full lifecycle chain (replay
through settle to an outcome with a reputation tail) rides
the F4 fix slice.

## Slice-62 golden freeze (2026-10-29, same pins)

Slice 62 adds the reputation-init chain to the harness and
freezes the golden for the current `.so` — the program
source is unchanged since slice 61, so the freeze is a
reproduction proof, not a re-measurement after a change.
Chain addition, run right after `seal_kernel`:
`init_reputation` for seat1 (ag(1) = `[1;32]`, a
sealed-profile agent) — **23,387 CU**, record built at the
genesis midpoint (50,000,000 raw) with its PDA bump — and a
second init on the same account, which refuses (5,580 CU,
surfaced under mollusk as Custom(0)) and leaves the genesis
value untouched, proving the record one-time. All 17 prior
measured values reproduce EXACTLY, the finalized-close
refusal is unchanged (5335 / 6034), blocked `[]` still,
`.so` sha unchanged
`1518d41e154ac52d3b919901112c55812d800276b6d710b5f857f0661b5a802d`
(host tests 456/456; IDL 44 / 9 / 46 / 26, unchanged).
Scope stated plainly: this freeze does NOT cover
`record_outcome` / `record_outcome_governance` with
reputation tails — those need a SETTLED transition, which
needs the full lifecycle chain (replay through settle)
driven in mollusk, and that chain is the next slice's
work, not something this freeze claims.

## Slice-61 re-measurement (2026-10-29, same pins)

Slice 61 (the reputation-persistence slice: `ReputationRecord`
account + `init_reputation` instruction + reputation tails on
both outcome instructions + 13 host tests) changed the program
source, so the golden file was re-measured against the new `.so`
sha
`1518d41e154ac52d3b919901112c55812d800276b6d710b5f857f0661b5a802d`
(host tests 456/456; IDL now 44 instr / 9 accounts /
46 errors / 26 events — one new instruction, one new account,
one new error, events unchanged). Drift from the slice-60
values is layout noise, not behavior: initialize 17776 ->
17772, seal 18045 -> 18047, ingest 24401 -> 24403, init gov
action 8447 -> 8449, finalize 7306 -> 7308, register
governance 26120 -> 26118, approve 37629 -> 37633,
abandoned second init 14447 -> 14449, close 3913 -> 3915;
the other eight values reproduce exactly, the finalized-close
refusal is unchanged (5335 / 6034), blocked `[]` still. CU
for the new `init_reputation` instruction and for outcome
instructions with reputation tails is not yet in the golden
set — it rides the golden-freeze slice. This `.so` is
likewise NOT the golden freeze.

## Slice-59 re-measurement (2026-10-29, same pins)

Slice 59 (the `record_outcome` instruction, ordinary
path, + `record_outcome_from_records` helper + 10 host
tests + the Rule-A wording correction in the harness
refusal note) changed the program source, so the golden
file was re-measured against the new `.so` sha
`9cff80abfe892dcd68f6b3e5b90103863b4196b685d3ef028ae11c5949840238`
(host tests 433/433; IDL now 42 instr / 8 accounts /
45 errors / 26 events — one new instruction, three new
errors, one new event; no new account). Drift from the
slice-58 values is layout noise, not behavior: append
chunk 4683 -> 4684, approve 37628 -> 37629, init
8449 -> 8447, abandoned second init 14449 -> 14447,
finalized-close refusal 5334 -> 5335; the other twelve
values reproduce exactly, blocked `[]` still. The
refusal note now reads "never closes (Rule A)" (it said
"stays open until the transition settles", contradicting
the slice-58 ruling); the CU is unchanged by the wording.
This `.so` is likewise NOT the golden freeze.

## Layout

- `mollusk/` — standalone Rust crate (its own `[workspace]`
  root, so the parent program-v04 workspace does not claim it).
  Loads `target/deploy/program_v04.so` (built by `anchor build`
  on the v04-line toolchain: Anchor 1.2.2 / Solana 4.3.2 /
  platform-tools v1.57 — migrated 2026-10-29 under Carlos's
  pin-migration ruling; the 1.2.1 / 4.1.2 / v1.54 pins are
  superseded for this program).
- `cu-golden-mollusk.json` — the golden file: measured CU per
  instruction, with the program `.so` hash and measurement
  environment it was measured against.
- `run-mollusk.sh` — runs the harness and writes the golden file.

## Setup

```sh
~/workspace/toolchains/anchor-122-scratch/bin/anchor build   # from program-v04/
./run-mollusk.sh   # from cu-golden/ (pins the Agave 4.3.2 bin dir on PATH)
```

## Later slices (not this slice)

The stage instructions join once the F4 fix lands (slice 63
proved `replay_transition` panics out-of-memory
in-process; the rest of the chain — evaluate,
mark_eligible, submit_signal, select (+ siblings),
open_planning, submit_plan, reconcile, validate, declare,
commit, and the governance stage chain through
commit_governance — cannot be measured until replay
executes). Nothing here claims those measurements.

## Chain-length fixture, heap leg (2026-10-30, merge-gate item 5, part 1 of 2)

Host-side fixture in `programs/program-v04/src/lib.rs` (tests):
`chain_length_outcome_heap_vs_priors` drives N claims-carrying
ordinary bundles to COMMITTED at successive heads (the proven
moved-head pattern: eligible at the current clock, window +100,
planning deadline +200, plan expiry deadline +100, next bundle
eligible at the previous planning deadline +100), then a
claims-carrying governance tail (two-claims plan) through
commit/settle, and measures `record_outcome_governance`'s host
heap with the counting allocator (`alloc_meter`, cfg(test)-only)
armed around exactly that one helper call. Run single-threaded
(`--test-threads=1`): the meter is a process-global allocator.
The measured chain itself is governance, as the gate requires.

Measured `record_outcome_governance` host heap (cumulative /
live peak, bytes; with_wrapper = cumulative + the gate's 16 KiB
wrapper allowance):

| priors | cumulative | allocs | live peak | with_wrapper | % of 192 KiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| 0 | 39,797 | 112 | 31,624 | 56,181 | 28.58 |
| 1 | 46,721 | 143 | 35,856 | 63,105 | 32.10 |
| 2 | 54,413 | 175 | 40,472 | 70,797 | 36.01 |
| 3 | 61,337 | 206 | 44,704 | 77,721 | 39.53 |
| 5 | 75,761 | 269 | 53,456 | 92,145 | 46.87 |

Least-squares fit on the first three points (0/1/2), cumulative:
slope 7,308 B/prior, intercept 39,669 — predicts N=3 within
+0.42% and N=5 within +0.59% (76,209 predicted vs 75,761
measured). Peak fit: slope 4,424 B/prior, predicts N=5 within
+0.42%. The gate's predict-5 property holds on the heap leg.
Heap-limit leg (cumulative + 16 KiB vs 196,608): Nmax = 19
(fit with_wrapper 194,905 at 19; 202,213 at 20). The peak leg
alone would allow 33, so cumulative binds. NO program constant
is set from this leg alone: the CU leg and the frame leg are
still pending, and the constant is ~half of the SMALLEST of
the three limits — setting it now would prejudge the CU leg.

Note on the F6 figures: this fixture's 0-prior outcome
(39,797) sits ~17.6 KB under F6's genesis outcome row
(57,383), and the 1-prior row ~15.9 KB under F6's moved-head
row (62,579) — a systematic version delta (v0.4.1 -> v0.4.2
allocation-only changes: Box<Rec> transitions, Cow reason),
not a fixture disagreement: both fixtures' slopes are linear
and this fixture's own predict-5 check passes. The F6 rows
are superseded by this fixture for the v0.4.2 line.

`.so` hash note (same pass): adding the test-only meter +
fixture shifts source line numbers, and this project's SBF
binary embeds panic line numbers, so the rebuilt `.so` hashes
`f4e543ee…` instead of the adoption build's `b47ea2fa…` at
the identical size (1,860,248 bytes) — the slice-58 precedent
(test-only change moves the hash, CU unchanged). The golden
chain re-run against the new binary reproduces all 63 CU
values, both refusals, and the empty blocked list exactly, so
the golden file's `so_sha256` is re-frozen to `f4e543ee…`
with no CU change. This is NOT the merge-gate final freeze:
that waits on the CU leg, the limit derivation, the program
constant, enforcement, and the tripwire/smallframe guard
re-runs on the final binary.

## Chain-length CU leg + frame leg (merge-gate item 5, parts 2–3)

In-process fixture in `chainlen/` (own crate, own lockfile — the
lock is the golden harness's lock with only the root package
renamed: a fresh offline resolution pulls conflicting
`solana-instruction-error` 2.x/3.x into mollusk-svm 0.16.4's
graph and fails to compile, so the proven lock is copied, not
re-resolved). Run: `cu-golden/run-chainlen.sh`, output
`cu-golden/chainlen-cu.json`. For each N in 0/1/2/3/5, in a
FRESH mollusk instance: genesis + seal, then N ordinary
bundles driven to COMMITTED at successive heads (each bundle's
own 200-tick window, two-claims plan 10 + 5 SCALE, same shape
as the golden harness's life chain), then a governance tail
(GrantRole action, unanimous council, empty-claims plan)
registered at the moved head with prior_bundle_count = N,
driven through commit, settlement evidence, settle, and
`record_outcome_governance`. The measured value is the
outcome instruction's CU at that prior count, against the
shipped binary (`so_sha256 f4e543ee…`, the heap-leg binary).

Measured `record_outcome_governance` CU (reproduced IDENTICALLY
on a second run):

| priors | CU |
| --- | ---: |
| 0 | 88,279 |
| 1 | 110,451 |
| 2 | 132,991 |
| 3 | 156,313 |
| 5 | 205,288 |

Cross-validation: the 1-prior row reproduces the golden
chain's `record_outcome_governance_gov2` EXACTLY (110,451) —
same binary, same chain shape, independent driver. Per-prior
increments grow: 22,172 → 22,540 → 23,322 → 24,488/step
(3→5 averaged) — the CU curve is mildly CONVEX, not linear.
Least-squares fit on the first three points (0/1/2): slope
22,356 CU/prior, intercept 88,218 — predicts N=3 within
+0.66% but N=5 within +2.65% (UNDER-predicts). The gate's
predict-5 property holds tightly on the heap leg (+0.59%)
and only loosely here: a linear CU extrapolation understates
large-N cost, so the limit derivation must NOT use the naive
linear fit — it must bound with the growing marginal cost
(last-increment extrapolation: Nmax 53 at a 1.4M CU limit,
vs 58 on the linear fit). Either way CU does NOT bind: both
figures sit far above the heap leg's Nmax 19, so the smallest
limit remains the heap leg until the frame leg is weighed.

Frame leg: SBF stack frames are compile-time constants — they
cannot vary with prior count, so the frame limit is
N-independent by construction and cannot bind the
chain-length limit at any N. Verified on the shipped binary
this pass: a forced `anchor build` (v04 pins, source touched
for mtime only, `.so` hash unchanged at `f4e543ee…`) exits 0
with ZERO linker frame-overflow lines — no function frame
exceeds 4,096 bytes. (The 4,672-byte BTreeMap-insert overflow
in `tripwire-build.log` belongs to the `tripwire-heap`
feature build, whose allocator change alters codegen; it is
not present in the shipped build's link.) The gate adoption's
3,648-byte max figure is not re-derived by this build's
output — the linker reports only overflows, never fitting
sizes — but the pass/fail question (any frame > 4,096?) is
answered directly: none. NO program constant is set from
these legs alone: the limit derivation (smallest of
heap-19 / CU-53-or-58 / frame-unbounded, constant at ~half)
is the next queue item, followed by enforcement + the final
golden freeze.
