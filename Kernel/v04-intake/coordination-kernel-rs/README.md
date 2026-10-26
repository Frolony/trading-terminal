# coordination-kernel (Rust reference, v0.4)

The Rust mirror of the amalgamated Solidity kernel (`contracts/` in the Solidity folder; copies of the files that matter are in `reference/`). `no_std + alloc`, no dependencies.

## What is the same, and how that is enforced

`tests/parity.rs` reads the Solidity sources and fails if any of these drift:

* the 13 fail bits (`F_CONSTRAINTS` … `F_DIALECT`) and their values;
* the order in which the predicate tests its terms, and that no term exists on one side only;
* all 48 Stance × Contribution cells (sector and weight) of the classifier;
* the lifecycle numbering;
* the governance constants (3 epochs sitting, 3 epochs of position, defection cooldown, caps);
* the nine genesis notations.

## What is decided where

* **Rules change only through a governance transition** that passes the whole predicate, after every sitting council seat approved that exact (id, action). The genesis authority exists only until `seal_genesis`, which is refused until a full council, a policy for each governance type and a valid influence share exist.
* **Position gates council candidacy and nothing else.** The amount is never stored and never weights a signal. The tests assert a holder of 1 and a holder of u128::MAX carry the same weight.
* **Group policy is the highest-matching multiplier**, not the sum.
* **No single agent can carry more than `max_influence_share` (default 1/6) of base influence**, enforced at seal, at `RegisterAgent`, and when the cap is tightened.
* **Attestor logic is in the kernel**: `settlement_threshold` distinct Settlers agreeing on one realized hash. The v0.3 `Attestor` and `WeightPolicy` traits are gone; the administrator now owns only the `Env` hooks (constraints, objectives, relevance, locked holding), and the default `NoEnv` answers UNAVAILABLE to everything, so nothing commits until evaluators are wired.
* **Governance commits are atomic without copying the kernel**: the commit and its action are both validated first (`commit_check`, `check_action`) and applied only if both would succeed, so a committed governance transition always means an applied one. The apply step cannot fail, which is what lets this run in a fixed-memory runtime.
* **Abandoned blocked transitions can be discarded.** A Blocked transition holds its parent's selection slot. `discard_blocked` lets a strict majority of all council seats discard it (Blocked only, one vote per seat per blocking episode), which frees the slot. It adjudicates that the transition is abandoned; it is not a rule change.
* **Reputation switch**: `Config::reputation_weighs`. `true` is Solidity parity (influence scales with reputation). `false` is the treasury launch profile: reputation is recorded from the first outcome and has zero influence; influence is the administrator-set base.

## Differences from Solidity, on purpose

* K (plan compatibility) is the built-in capacity fit and is re-read against live capacity at commit; Solidity delegates K to a pluggable engine.
* There is no module routing, no versioned evaluator registries and no Merkle trust root; those are EVM concerns.
* Attributions are supplied by an `OutcomeRecorder` role (summed per agent, capped, positive credit halved on drift); Solidity derives them from registered attribution rules.
* Fixed-point is 6 decimals (`Amount`), not 18; the predicate is identical, the arithmetic is checked u128.
* A locked-holding source is the `Env::locked_holding` hook rather than a contract address.

## Tests

`cargo test` runs the unit tests, 30 behavioural tests and the 7 parity tests. `cargo build --no-default-features` builds the `no_std` kernel.
