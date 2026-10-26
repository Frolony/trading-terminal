# program-bridge-v04 — migration step 5, slice 1

The program bridge over the v0.4 lifecycle: replay a transition from its
recorded inputs alone, through the added steps, on a fresh sealed launch
kernel. Consumes the delivered v0.4.1 crate and the step-2 launch profile
by path; modifies neither, and never touches `Kernel/` in the repo.

## What it proves (tests `tests/bridge_v04.rs`, 6/6)

- **B1** — A well-formed ordinary replay commits: head index 1, head hash
  equals the record's declared hash, and the fixture tranche stock debits
  exactly (1000 → 900 for a 100 claim).
- **B2** — An over-claim replay blocks at validate: no declare, no commit,
  stock and head unchanged.
- **B3** — A stale-parent record is refused at registration itself
  (`ParentNeDeclared`) — stronger than the declare-time LINEAGE refusal,
  which remains the backstop if a head moves mid-replay.
- **B4** — A governance replay applies its rule change: unanimous council
  approval + `register_governance_transition` + the same added steps +
  `commit_governance` moves the named stock (exit window 250 → 400) and
  leaves other stocks untouched.
- **B5** — The same record replayed twice on fresh kernels gives the same
  head, stocks, and trace: replay needs no live state.
- **B6** — A governance record without unanimity does not commit.

## The added steps, in order

`evaluate → eligible → select → reconcile → validate → declare`, as a
subsequence of every committing trace (`ADDED_STEPS`,
`trace_contains_in_order`). The v0.2 bridge's shorter replay
(register → signal → planning → reconcile → mask → commit) is the
contrast: step 5 does not skip the steps v0.4 added.

## Explicitly not this slice

- **No Anchor program change.** The program rewrite rides the branching
  question (branch vs replace outright), which is Carlos's and still gates
  any v0.4 repo entry. This crate proves the replay shape on the host.
- **No CU measurement.** F2's commit-clone cost is gone from the commit
  path as implemented in v0.4.1 (`commit_check`/`commit_apply`), but
  on-chain CU is unmeasured until the cu-golden step on the program
  rewrite. Nothing here assumes a CU figure.
- **No settlement change.** Settlement after commit stays with the step-2
  `SettlerGate` composition (evidence-bound, settle-after-realized).
- **Governance increments only.** Treasury instructions (§4.2/§4.3) and
  the program's account inventory are later step-5 slices, not this one.

## Slice 2 — treasury instructions + account inventory (`src/treasury.rs`, tests `tests/treasury_v04.rs`, 11/11)

- **Per-instruction records over the slice-1 replay:** `pay_tranche` claims
  `RES_TREASURY_TRANCHE`, `borrow` claims `RES_LENDING_BOOK`, `exit` and
  `execute_whale_tranche` claim `RES_EXIT_WINDOW`; `reinvest`,
  `settle_default`, `collect_fees`, and `open_window` claim no stock.
  Every value-moving instruction names exactly one stock — the §4
  authorization rule, tested, not prose.
- **Tests T1–T11:** tranche/borrow/exit debit exactly and leave the other
  stocks untouched; a whale tranche at the full exit stock commits and
  one unit over it blocks with nothing moved (the D-10 hard bound in
  stock form); reinvest / settle_default / collect_fees / open_window
  commit with all stocks unchanged and the head advanced;
  `settle_default` carries the D-3 pinned `PreDefault` base;
  `collect_fees` preserves its declared total; the §3 inventory
  (11 entries) has unique names, distinct stocks, and covers all three
  debited stocks; the window protocol order is open → pay → settle →
  borrow → exit → reinvest → collect_fees.
- **Honest limits, stated in the module docs:** host replays reuse the
  slice-1 fixture transition type (the on-chain types
  `treasury.window.v1` / `treasury.exit.v1` are documented, not yet
  enforced); the D-10 flag arithmetic (≥ T/30, 20% weekly blocks, 12-month
  lock) is program logic, not host-fixture logic — the host proves the
  stock bound; the penalty door is engine-priced (D-4: flat 13%, loan
  rate unchanged at 12%) and no penalty figure is asserted here.

## Slice 3 — ruling-2 council discard (`src/discard.rs`, tests `tests/discard_v04.rs`, 6/6)

- **Bridge replay of `discard_blocked`:** an over-claim record replayed
  to Blocked (1500 against the 1000 tranche stock), then discarded by
  replayed council votes, freeing the parent slot for its sibling — the
  K-1 ruling-2 shape the program rewrite will execute on-chain, so an
  abandoned blocked transition cannot freeze its parent until expiry.
- **Tests D1–D6:** a blocked replay carries the added steps to validate
  and no declare/commit step, and debits nothing; a non-seat vote is
  `NotCouncil`; one seat is not a strict majority (`Ok(false)`) and a
  double vote in one episode is `AlreadyVoted`, with the sibling's
  select still refused while the slot is held; a strict majority
  discards (`Discarded`) and the sibling selects with all stocks
  unchanged; discard refuses a Validated transition and an
  already-Discarded one (`BadLifecycle`); a sibling that blocks again
  on its own over-claim takes fresh votes (episode-scoped votes).
- **Fixture note:** the kernel clock is strictly monotonic per call, so
  the two-transition world staggers every ingest/register/evaluate/
  signal tick-by-tick; the shared added-steps schedule is preserved
  from the select step on. No CU is measured here — cu-golden still
  rides the program rewrite, and branch entry onto `kernel-v04` is the
  next queue item, not this slice.

## Also in this increment

Doc-only fix in `engine-adapter-v04/src/lib.rs`: the stale pre-v0.4.1 F1
wording ("no `reputation_weighs` switch in the delivered build") is
corrected to the closed form (switch exists in v0.4.1, profile pin-OFF in
step 2; F2's clone gone from the commit path, CU still unmeasured).
No behavior change; adapter suite still 7/7.
