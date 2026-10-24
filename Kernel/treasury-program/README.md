# treasury-program — own treasury program (W4.3 scaffold)

**Status: KERNEL VENDORED, host-proven; SBF-GREEN on the adopted toolchain (2026-10-24).**
Anchor project at `Kernel/treasury-program/` — its own Anchor project, NOT
a member of the `Kernel/` Cargo workspace. The pristine kernel crate is now
a path dependency (`programs/treasury-program/src/kernel_bridge.rs`):
launch policy + config as named items, host tests proving the full
lifecycle clears the 13-bit mask and commits while an over-claim fails
COMPAT and commits nothing (program tests 9/9).

**D-12, closed 2026-10-24 — read before building.** The SBF blocker is
gone, by both routes in sequence: (a) the kernel's author revised the two
`Option::is_none_or` call sites (`kernel/src/kernel.rs:401`, `:537`) to the
1.79-compatible `map_or(true, f)` form (remote `3ef5c412`, 06:21 CST,
exactly those two lines); then Carlos adopted Solana 2.2.20 (below) and
pushed the revert (remote `f3086ac6`, 06:38 CST), restoring the author's
original `is_none_or` text byte-for-byte (remote blob SHA matches the
archived pristine crate). Final state: original kernel text + Solana
2.2.20, `anchor build` exit 0, host clippy clean again (the `map_or` form
had tripped host clippy 1.99's `unnecessary_map_or` — the revert removes
that friction too). The vendoring increment is PUSHED (remote
`0df20ec7`, 2026-10-24 ~07:06 CST). The §4.1 admin-setup slice landed
(remote `1a5f53b7`, 2026-10-24 ~07:21 CST): `AgentRecord` +
`register_agent` / `grant_role` / `revoke_role` (genesis-direct form).
This increment adds the first lifecycle instructions in design order:
`post_observation` (Oracle-only, Clock-judged) + `register_transition`
(TransitionRecord in Registered state; observation must already exist;
lineage judged at commit, not here) + `open_planning` (proposer-or-Admin
only, Registered → Planning, deadline `now + PLANNING_WINDOW`; the
TransitionRecord now carries the recorded proposer key, SPACE 74 → 106)
+ `submit_plan` (Planning-only, before the deadline, plan agent ==
caller, one plan per agent via PDA seeds, capacity NOT checked here —
COMPAT at reconcile/commit; standalone PlanRecord accounts, SPACE 253;
TransitionRecord gains `plan_count`, SPACE 106 → 110, so TooManyPlans
is enforceable on-chain). Carried gap, stated in the code: the kernel's
group-overlap half of plan eligibility awaits `relevant_groups` on the
record (manifest completion); this slice enforces the active half.
+ `submit_signal` (Registered-or-Planning, support ≤ 1.0, eligible
agent, one signal per agent per round via PDA seeds, round matched to
the transition's current round; TransitionRecord gains `round_count`,
SPACE 110 → 114, starting at 1 — the kernel starts every transition
with one empty round; the monotone-revision check is pure-helper
tested now and wires on-chain with `advance_round`, which snapshots
the previous round's mean — until then only round 0 is reachable,
where the kernel's monotone branch is vacuous).
+ `submit_constraint_report` (Evaluator-only, Registered-or-Planning,
report against a member of the frozen set — an outside id is
`UnknownConstraint`, never a new constraint; reports live on the
TransitionRecord, not standalone accounts, and overwrite the entry's
status). `register_transition` now takes the frozen set (≤ 8 distinct
ids, all Unavailable at registration; TransitionRecord SPACE 114 →
382). Scaffold-form note: the caller supplies the frozen set at
registration; when PolicyRecords land, registration derives the
mandatory part from the transition type's policy and unions the
proposer's additions, as the kernel does.
+ `submit_objectives_check` (Evaluator-only, Registered-or-Planning,
independent bool verdict, never derived from plan fields; overwrites
the TransitionRecord's `objectives_ok` flag, false at registration;
TransitionRecord SPACE 382 → 383). The OBJECTIVES bit at commit is
`require_objectives && !objectives_ok` — at launch the policy does not
require objectives, so the bit is clear regardless, but the flag is
recorded now so a requiring policy is judged on a real report.
`advance_round` and the remaining lifecycle instructions are next.

**Design:** `files/OWN_IDL_DESIGN.md` (W4.1, validated and revised
2026-10-24; W4.3 GO given by Carlos 2026-10-24 ~03:39 CST). Placement C: the
kernel crate compiled into the program, the full 13-bit `commit_fail_mask`
computed on-chain, NO digest publisher.

**Scope (§8, committed-inventory ONLY):** the Token-2022 transfer-hook
extension is NOT W4.3 scope (future phase, its own Carlos decision + legal
review). Planning-window durations are a genesis parameter (engine-sim-proven
policy values carried as named config constants — no ruling invented). Loans
are 60-day only at pilot (§4.2).

**No deployment in W4.3.** Deployment, the swarm against the real program,
and tracer dogfood are W4.4, separately sequenced. No mainnet step exists
in W4. The program id in `declare_id!` is a scaffold placeholder for local
build / IDL generation only; its keypair was never committed. Known
scaffold item: `anchor build` generated a `target/deploy/` keypair whose
pubkey does NOT match this `declare_id!` (build output, not committed) —
do not deploy from it. W4.4 generates a fresh deployment keypair and
updates `declare_id!` + `Anchor.toml` in one pass, as one change.

**Canonical IDL:** generated by `anchor build` — hand-written IDLs drift, the
program is the source of truth. The scaffold's golden tests must emit
measured CU per instruction, and any §2.2 design estimate missing by more
than 2× is a design finding — reported, never tuned away silently. CU
measurement lands with the instruction families (local-validator harness);
this skeleton's tests are pure-constant checks only.

**Toolchain:** Anchor 0.31.0, Solana 2.2.20 (platform-tools v1.48 /
rustc 1.84.1; 2.1.21 kept at `~/workspace/toolchains/solana-2.1.21/` for
rollback), rustc 1.99.1 host (see `~/TOOLS.md`).
The SBF pin set (incl. the vendored `patches/cpufeatures`) is the proven
W3.3 mock set. This project is self-contained: the patch path is relative,
so it builds without the W3.3 scratch directories.

**Build:** from this directory, with the toolchain env exported,

```sh
anchor build
```

The generated IDL lands at `target/idl/treasury_program.json` (build output,
not committed — regenerate it; never hand-edit it).

**Increment order (design order):** skeleton (this) → §4.1 governance
(kernel vendoring + lifecycle instructions + launch policy table) → §4.2
treasury (pinned window protocol) → §4.3 distribution → W3.2 adversarial
suite re-expressed against the program's interface (local validator).
