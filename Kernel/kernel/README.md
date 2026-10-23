# Coordination Kernel (Rust reference)

Reference implementation of the five-organ coordination kernel. It mirrors
`CoordinationKernel.sol`: one canonical predicate, `Kernel::commit_fail_mask`, returns a
bitset of failed terms with the **same `F_*` values** as Solidity. `commit()` succeeds iff the
mask is zero. `bit_parity_with_solidity_source` parses `reference/CoordinationKernel.sol` and compares its `F_*` literals to the Rust constants (mutation-checked: changing one Rust bit fails it).

| Bit | Term | Bit | Term |
|----:|------|----:|------|
| 1 | CONSTRAINTS | 256 | EXPIRED |
| 2 | OBJECTIVES | 512 | LINEAGE |
| 4 | QUORUM | 1024 | PLANS |
| 8 | CONFIDENCE | 2048 | CAPABILITY |
| 16 | COMPAT | 4096 | DIALECT (hook; Phase 7) |
| 32 | EXCEPTIONS | 64 | FRESH |
| 128 | PLANNING_OPEN | | |

## Properties
- **Kernel-owned inputs.** Quorum, constraints, exceptions, plans and observation are state, reached only through role-guarded entry points (Admin / Oracle / Evaluator). Fields are private.
- **Safe numbers.** `Amount` is a non-negative u64 fixed-point (1e6); multiplication runs in u128 and errors on overflow. No saturating math, no f64 in kernel logic (`from_f64` is `std`-only, for tests/tools).
- **Feasibility is strict.** An empty constraint set fails; `Unavailable` is not `Pass`; no plans fails both COMPAT and PLANS.
- **Guarded lifecycle.** Registered → Planning → Committable → Committed → Settled (or Rejected/Expired). Nothing regresses.
- **Declared vs realized.** `commit` advances the declared chain; `settle` records the realized hash and applies attribution.
- **Attribution.** Summed per agent (order-independent), capped by `max_attribution`, drift halves only positive credit, penalties apply in full.
- **Monotonic time.** Every entry point takes `now`; regression is an error.
- **Capacity is a consumed stock**, debited at commit.

## Administrator hooks (deliberately left to the swarm administrator)
- `Attestor`: who may observe and settle, and what counts as proof. Default `RoleAttestor` = Oracle role.
- `WeightPolicy`: relevance R, group policy G and the influence term I in W_eff = raw·R·I·G. Default `FlatWeights` = flat 1.0 and admin-set influence; override `influence()` to let reputation govern.
- Construct with `Kernel::with_policies(...)`; `Kernel::new(...)` uses the defaults.
- "Nothing commits" is intentional: a failed transition leaves the declared chain unchanged, so the previous commit carries over, and the next round is a new transition on the same parent.

## Decisions taken as defaults (change if you disagree)
capacity = consumed stock; declared state advances the chain; drift halves credit only.

## Not yet implemented
Dialect registry (F_DIALECT is a hook that always passes), relevance functions (R and GPolicy are flat 1.0, as in Solidity `_eligible`), collapse freeze, Context(T) versioning.

## Build
`cargo test` · `cargo build --no-default-features` (no_std + alloc)
