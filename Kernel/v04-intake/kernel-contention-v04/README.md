# kernel-contention-v04 — K-1 contention probes (staging only)

Migration step 3 preparation: what the delivered v0.4 crate actually does
when capacity claims exceed stock. Consumes the delivered v0.4 crate and the
step-2 launch profile by path; modifies neither; enters no repo.

## Results (4/4, clippy `-D warnings` clean)

- **k1a** — A capacity-blocked transition has NO in-place unblock:
  re-validation alone returns `StillBlocked` (compatibility is a stored
  reconcile-time flag), and re-planning cannot revise an existing planner's
  claim (`Duplicate`) — new plans only grow the aggregate. Exits are
  proposer withdrawal, expiry, or the re-plan loop ending still blocked.
  The claim carries only at the adapter level, as a fresh transition.
- **k1b** — A Blocked transition holds the `selected_for` slot for its
  parent key: a same-parent transition cannot select until the blocked
  claim is withdrawn, and only the proposer may withdraw.
- **k1c** — A governance top-up cannot select while a blocked claim holds
  the slot; after withdrawal + top-up commit, the old transition's mask
  carries LINEAGE, and a fresh transition against the new head carries
  the original claim in full. A withdrawn transition cannot also be
  marked conflicted (`BadLifecycle(Discarded)`).
- **k1d** — Aggregate over-claims block the whole transition (never a
  partial debit), and the re-plan loop is capped at 3 (`ReplanCap`).

## Standing

These probes do NOT rule K-1. The ruling questions are in
`hidden_files/K1_CONTENTION_DECISION_BRIEF.md`. Until the author rules,
carry-over is an adapter duty (withdraw + re-register against the new
head) because the delivered build forces that shape — not a builder
improvisation of kernel semantics.
