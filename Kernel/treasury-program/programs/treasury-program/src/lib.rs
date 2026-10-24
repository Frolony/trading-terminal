//! treasury_program — the own treasury program, W4.3 scaffold (skeleton).
//!
//! Design: `files/OWN_IDL_DESIGN.md` (W4.1, validated and revised 2026-10-24;
//! W4.3 GO given by Carlos 2026-10-24 ~03:39 CST). Placement C: the kernel
//! crate compiles into the program (no_std SBF) and the full 13-bit
//! `commit_fail_mask` is computed on-chain; there is NO digest publisher —
//! the W3.3 mock's trust root does not survive into this program.
//!
//! # Scaffold status (this increment: kernel vendoring + predicate proof)
//!
//! One genesis instruction (`initialize_config`, design §4.1) and the
//! `Config` account (design §3) exist, and the pristine kernel crate is now
//! vendored in (`kernel_bridge`): placement C is wired, the launch policy
//! and config are named constants/functions, and host tests prove the full
//! lifecycle clears the 13-bit mask and commits — while an over-claim
//! fails COMPAT and commits nothing. **SBF status: GREEN (2026-10-24).**
//! D-12 closed this morning: the author's `map_or` revision landed
//! (`3ef5c412`), Carlos adopted Solana 2.2.20 and pushed the revert
//! (`f3086ac6`) restoring the original `is_none_or` text byte-for-byte,
//! and `anchor build` on platform-tools v1.48 / rustc 1.84.1 exits 0 with
//! the full predicate compiled in (see `treasury-program/README.md`).
//! The vendoring increment is PUSHED (remote `0df20ec7`, 2026-10-24 ~07:06 CST).
//! This increment adds the §4.1 admin-setup slice: `AgentRecord` plus
//! `register_agent` / `grant_role` / `revoke_role`. Admin setup comes
//! before the role-gated lifecycle instructions by necessity —
//! `post_observation` requires an Oracle, `submit_constraint_report` an
//! Evaluator, and no agent can hold a role until an AgentRecord exists.
//! Genesis-direct form only: the caller must be the `Config` authority.
//! After genesis, changes to what these calls control are executed as
//! committed `treasury.policy.v1` transitions (design §4.1), never by a
//! fresh authority signature alone — that execution path is a later
//! increment. This increment adds the first lifecycle instructions in
//! design order: `post_observation` (Oracle-only, Clock-judged freshness)
//! and `register_transition` (creates the TransitionRecord in Registered
//! state; the manifest's observation must already exist — the account
//! constraint enforces it; lineage is judged at commit, not here).
//! Constraint reports, plans, signals, and the remaining lifecycle
//! instructions land next, in design order, then §4.2 treasury, then
//! §4.3 distribution. This increment adds `open_planning` (design §4.1):
//! only the transition's recorded proposer or an Admin may open planning,
//! the state moves Registered → Planning, and the planning deadline is
//! `now + PLANNING_WINDOW` from the carried launch policy. Recording the
//! proposer required one honest change to the previous slice: the
//! TransitionRecord now carries the proposer's key (set from the payer at
//! `register_transition`), so its SPACE moves 74 → 106. No on-chain
//! account exists yet (no deployment in W4.3), so no migration is owed.
//! This increment adds `submit_plan` (design §4.1): Planning-only,
//! before the deadline, plan agent == caller, one plan per agent (the
//! PDA seeds enforce the duplicate rule — a second plan from the same
//! agent fails at account creation, no state changes), capacity
//! deliberately NOT checked here (the kernel checks it at
//! reconcile/commit, COMPAT — design §4.1: "there is no courtesy
//! check"). Plans live in standalone `PlanRecord` accounts (design §3
//! lists Plan separately from the TransitionRecord, unlike constraint
//! reports which live on it), and the TransitionRecord gains a
//! `plan_count` so the kernel's `TooManyPlans` rule (max 4 at launch)
//! is enforceable on-chain — SPACE 106 → 110. Honest gap, stated in
//! the handler docs: the kernel's eligibility rule also requires the
//! agent's groups to overlap the manifest's `relevant_groups`, but the
//! record does not carry `relevant_groups` yet (manifest completion
//! lands with its owning increment), so this slice enforces the
//! active half of eligibility and defers the group-overlap half — the
//! gap is documented, not papered over.
//! This increment adds `submit_signal` (design §4.1): Registered-or-
//! Planning state, support in range, eligible (active) agent, one signal
//! per agent per round (the SignalRecord PDA seeds enforce the duplicate
//! rule — a second signal from the same agent in the same round fails at
//! account creation). The kernel's monotone-revision rule is pure-helper
//! tested here but its on-chain wiring awaited `advance_round`: only
//! round 0 was reachable until rounds advanced, and the kernel's monotone
//! branch is vacuous in round 0 (it compares against a previous round
//! that does not exist yet). `advance_round` now snapshots the previous
//! round's weighted mean onto the TransitionRecord; the remaining
//! wiring — the agent's own previous-round support, read from that
//! round's SignalRecord — is a follow-up signal-side increment, and
//! until then commit replay enforces the check. The TransitionRecord
//! gains `round_count` (SPACE 110 → 114), starting at 1 — the kernel
//! starts every transition with one empty round.
//! This increment adds `submit_constraint_report` (design §4.1) and the
//! frozen constraint set it reports against: `register_transition` now
//! takes the frozen set (at most [`MAX_CONSTRAINTS`] distinct ids, each
//! starting Unavailable — the kernel freezes mandatory ∪ proposer
//! additions at registration), and an Evaluator reports Pass/Fail
//! against a member of that set while the transition is Registered or
//! Planning. A report against an id outside the frozen set is
//! `UnknownConstraint`, never a silent new constraint. Scaffold-form
//! note, stated plainly: the caller supplies the frozen set at
//! registration; when PolicyRecords land, registration will derive the
//! mandatory part from the transition type's policy and union the
//! proposer's additions, as the kernel does — the set's shape (frozen
//! at registration, reported against, never extended) does not change.
//! The TransitionRecord gains `constraints` (SPACE 114 → 382).
//! This increment adds `submit_objectives_check` (design §4.1): an
//! Evaluator reports the independent objectives verdict (a bool) while
//! the transition is Registered or Planning; the report overwrites the
//! TransitionRecord's `objectives_ok` flag (SPACE 382 → 383). The
//! OBJECTIVES bit at commit is `require_objectives && !objectives_ok` —
//! at launch the policy does not require objectives, so the bit is
//! clear regardless, but the flag is recorded now so a policy
//! transition that does require it is judged on a real report, never
//! on an assumed default.
//! This increment adds `advance_round` (design §4.1): permissionless,
//! like the kernel's (the kernel ignores the caller) — Registered or
//! Planning state, the current round non-empty, and room under
//! `max_rounds` (3 at launch), in that check order. The RoundEmpty
//! gate is faithful, not a signal-count proxy: `submit_signal` now
//! accumulates the current round's effective-weight total and weighted
//! support sum onto the TransitionRecord (FlatWeights: relevance and
//! group policy are 1.0, so effective weight is raw × the signer's
//! recorded influence, Amount arithmetic — the same computation the
//! kernel performs, from on-chain records, never a client total), and
//! a round whose total is zero — no signals, or only zero-weight
//! signals — cannot advance, exactly as the kernel's
//! `total == Amount::ZERO` test refuses. Advancing snapshots the
//! previous round's weighted mean onto the record
//! (`prev_round_mean`) and resets the current-round aggregates, so
//! the monotone-revision check can read its mean. The TransitionRecord
//! gains `current_round_total`, `current_round_weighted`, and
//! `prev_round_mean` (SPACE 383 → 407). One gap stays open, stated
//! plainly: the monotone check also needs the agent's own previous-round
//! support, which lives on that round's SignalRecord — `submit_signal`
//! does not yet take it as an account, so the check is enforced at
//! commit replay (the kernel's `submit_signal` during replay refuses
//! a violating sequence), not at signal time. That wiring is the next
//! signal-side increment, not this one.
//! This increment adds `reconcile` (design §4.1): permissionless, like
//! the kernel's (the kernel ignores its caller) — Planning state only,
//! and only once the planning deadline has passed (`PlanningStillOpen`
//! before it, at the deadline itself it passes, exactly as the kernel's
//! `now < planning_deadline` test). A false fit still advances the
//! state to Committable — the kernel's fail-closed shape: the COMPAT
//! bit then refuses every commit, and `commit` re-checks the fit
//! anyway. The fit rule itself is proven here as a pure helper
//! (aggregate claims per resource, checked addition, a missing stock
//! or an over-claim fails, overflow fails): the handler evaluates it
//! against the real stocks (this slice: `set_capacity` installs the
//! CapacityStock accounts, and the plans' claims are summed against
//! them through this same helper) — a plan-bearing transition whose
//! claims no passed stock covers reconciles false (fail-closed) and
//! a plan-free one reconciles true (the empty aggregate fits
//! vacuously; the PLANS bit will still refuse it at commit). The
//! flag's meaning does not change. The TransitionRecord gains
//! `reconciled` (SPACE 407 → 408).
//! This increment adds `commit` (design §4.1): permissionless, like the
//! kernel's (the predicate decides, not the caller) — Committable state
//! only (`BadState` otherwise), and the full 13-bit `commit_fail_mask`
//! computed on-chain by the pure `commit_fail_mask` helper below, bit for
//! bit as the kernel computes it (proven by a per-bit test matrix plus a
//! kernel-parity case). A non-zero mask refuses with `CommitFailed` and
//! changes nothing — no state move, no event, and (a failed transaction
//! rolls back, so) no chain advance; the mask itself is logged with
//! `msg!`, because Anchor error codes are unit variants and cannot carry
//! it — the detail is in the program logs and is recomputable from the
//! public account state, never hidden. On a clear mask the transition
//! moves to Committed and the `DeclaredChain` advances to the transition's
//! declared hash, exactly as the kernel's `commit` advances its declared
//! state. The handler reads what the mask needs from on-chain records:
//! constraint reports and the objectives flag from the TransitionRecord,
//! the last round's aggregates from the same record, the observation
//! account for FRESH, the Clock for planning/expiry, PlanRecord accounts
//! (passed as remaining accounts) for PLANS expiries and the COMPAT claim
//! aggregate, the plan authors' AgentRecords (CAPABILITY), the
//! CapacityStock accounts that follow them (COMPAT balances), and
//! the `DeclaredChain` for LINEAGE. Three completeness rules make
//! the remaining-accounts read sound: the caller must pass exactly
//! `plan_count` PlanRecords, each program-owned, each naming this
//! transition, each pubkey distinct — a missing, foreign, or duplicate
//! plan is `PlanMismatch`, never a silently partial mask; one
//! AgentRecord per distinct plan author, in any order — a missing,
//! foreign, or duplicate author is `CommitAccountsMismatch`; and the
//! stock segment after them, each program-owned CapacityStock at the
//! PDA of its stored resource id — anything else is `StockMismatch`.
//! The COMPAT aggregate is judged against those real stocks through
//! the same `fits_capacity` helper `reconcile` uses, and on a clear
//! mask the handler debits them atomically (see the handler docs).
//! The
//! EXCEPTIONS bit is fed by the transition's live `unresolved_blocking`
//! count (`raise_exception` / `resolve_exception`, this slice); the
//! CAPABILITY bit is clear because no transition type carries required
//! capabilities yet (`require_capability` lands with PolicyRecords) —
//! both bits are computed, not assumed away, and their inputs land with
//! their owning instructions. Recording lineage required one honest change to an
//! earlier slice: `register_transition` now takes the manifest's parent
//! index and parent hash and freezes them on the record — lineage inputs
//! are manifest inputs, frozen at registration, judged at commit — so
//! the TransitionRecord gains `parent_index` and `parent_hash` (SPACE
//! 408 → 448). No on-chain account exists yet (no deployment in W4.3),
//! so no migration is owed.
//! This increment adds `record_evidence` (design §4.1): the
//! EvidenceAttestor's write handle at the program layer — an active
//! Oracle records the realized hash for a Committed transition into a
//! one-time `EvidenceRecord` account (PDA seeds `["evidence",
//! transition_id]`, SPACE 49). The program cannot verify that the
//! realized outcome exists in the world; recording early records a hash
//! that settle will refuse to match, so a premature record harms only
//! the recorder's own transition. Settle (next increment) checks the
//! presented hash against this record before it ever calls the kernel's
//! settle — no record, or a mismatched hash, refuses and leaves the
//! transition Committed for a correct settlement to retry. Recording
//! evidence changes no transition state; the TransitionRecord's
//! realized hash lands with settle, and settle's event needs the commit
//! index — so the settle slice also records the commit index on the
//! TransitionRecord at commit time (a follow-up change to the commit
//! handler, owed then, not now).
//! This increment adds `settle` (design §4.1) and closes that
//! follow-up: `commit` now records the new chain index on the
//! TransitionRecord as `commit_index`, and settle records the realized
//! hash there too (SPACE 448 → 488), so settle's event can name the
//! commit it settles. Settle mirrors the kernel's `settle` in check
//! order — attestor gate first (active Oracle role AND the presented
//! hash equal to the independently recorded EvidenceRecord — the
//! program-layer EvidenceAttestor, a role + evidence check, not a
//! capability check), then the Committed state gate — then the
//! attribution rules: every attributed agent must be a plan
//! participant, deltas are summed per agent (order-independent),
//! capped at ±20 units, and an inexact settle (realized ≠ declared)
//! halves positive credit only; reputation is written to the attributed
//! agents' AgentRecords (FlatWeights: recorded, zero quorum influence).
//! A refused settle changes nothing and leaves the transition Committed
//! for a correct settlement to retry; a settled transition cannot
//! re-settle (its state is Settled, not Committed). No new account
//! type: settle reads the existing EvidenceRecord and writes the
//! existing AgentRecords (passed as remaining accounts, classified by
//! discriminator, with the plan set complete — the commit slice's
//! completeness discipline, applied here to both sets).
//! This increment adds `raise_exception` / `resolve_exception`
//! (design §4.1) and wires the EXCEPTIONS bit to live inputs:
//! exceptions are per-transition records on the TransitionRecord
//! (SPACE 488 → 768), raised only by an Evaluator while the transition
//! is Registered, Planning, or Committable — a blocking exception on a
//! Committable transition is a pre-commit veto, and only its resolution
//! lets the transition commit — with dispositions in the kernel's order
//! (Informational, Review, Block, Emergency; Block and Emergency count
//! toward `unresolved_blocking`). Resolve mirrors the kernel exactly,
//! including its two properties a stricter program would hide: it has
//! no state gate (resolving after commit cannot rewrite the mask already
//! decided, and resolving a rejected or expired transition is bookkeeping
//! only), and re-resolving an already-resolved exception is a no-op
//! success that neither double-decrements nor errors. `commit` now
//! reads the record's live `unresolved_blocking` instead of the
//! hard-coded 0 the earlier slices carried as an interim.
//! This increment adds `policy_transition` (design §4.1) and closes
//! the §4.1 lifecycle instruction set: the `PolicyRecord` account (PDA
//! seeds `["policy", transition_type]`) holds one transition type's
//! policy — quorum, confidence threshold, minimum observation
//! confidence, the objectives requirement, planning window, round and
//! plan caps, the mandatory constraint set, and the required
//! capabilities — installed by the `Config` authority in genesis-direct
//! form, like the other Admin setup calls. `register_transition` now
//! takes the transition's type and its PolicyRecord, and the frozen set
//! is derived as the kernel derives it: the type's mandatory
//! constraints ∪ the proposer's additions, deduped, all Unavailable —
//! a proposer can no longer register a transition that omits its type's
//! mandatory gate, and a mandatory id supplied twice is one entry, not
//! two. The TransitionRecord gains `transition_type` (SPACE 768 → 800)
//! so the record names the policy that governs it. Two honest
//! interims, stated plainly: the per-type numeric values are recorded
//! now, but the lifecycle handlers still read the launch constants
//! (`open_planning`'s deadline, `submit_plan`'s cap, `submit_signal`'s
//! round cap) — wiring them to read the type's PolicyRecord is the
//! next policy increment, and the installed launch values equal those
//! constants, so no behavior changes until then; and the CAPABILITY
//! bit's on-chain input still awaits `grant_capability` (per-agent
//! holdings) — the required capabilities are recorded per type now,
//! the holdings they are checked against land with that instruction.
//! Post-genesis policy changes remain what the design says they are:
//! the execution of a committed `treasury.policy.v1` transition, not a
//! fresh authority signature — the amendment execution is a later
//! increment, and this genesis-direct install does not pretend
//! otherwise.
//! This increment closes the first of those two interims: per-type
//! policy enforcement in the lifecycle handlers. `open_planning`,
//! `submit_plan`, `submit_signal`, and `advance_round` now take the
//! transition type's PolicyRecord (seeds-bound to the record's
//! `transition_type`, so a stranger's policy cannot be substituted)
//! and read their caps from it — the planning window, the plan cap,
//! and the round cap — instead of the launch constants. The installed
//! launch values equal those constants, so no behavior changes on any
//! transition registered under a launch policy; a type whose policy is
//! later amended is governed by its own record from then on. This
//! increment closes the second interim: `commit` now takes the
//! transition type's PolicyRecord (seeds-bound to the record's
//! `transition_type`, like the four lifecycle handlers) and reads its
//! mask inputs from it — quorum, confidence threshold, minimum
//! observation confidence, and the objectives requirement — instead
//! of the launch policy. The installed launch values equal those
//! constants, so no behavior changes on transitions under a launch
//! policy; a type whose policy is later amended commits under its own
//! record from then on. The remaining interim is the CAPABILITY bit's
//! holdings, which still await `grant_capability` (per-agent
//! holdings); `capability_covered` stays true until then.
//! This increment closes that last interim: `grant_capability` /
//! `revoke_capability` (design §4.1 Admin setup, genesis-direct form
//! like `grant_role`) record per-agent capability holdings on the
//! AgentRecord (SPACE 81 → 341), and `commit` now reads the CAPABILITY
//! bit from live inputs — the transition type's required capabilities
//! against the summed influence of the plan authors who hold each one,
//! exactly as the kernel's `capability_covered` computes it (checked
//! addition: an overflowing sum fails the requirement, never wraps).
//! `commit`'s remaining accounts are now the transition's plans
//! followed by one AgentRecord per distinct plan author, in any order;
//! a missing, foreign, or duplicate author record is
//! `CommitAccountsMismatch`, never a mask computed over partial
//! holdings. No new account type: holdings live on the AgentRecord,
//! the design §3 CapabilityRegistry in program form. (At that point
//! the §4.1 lifecycle was input-complete for every mask bit except
//! COMPAT's real stocks — the paragraph below closes that interim.)
//! This increment closes that last interim: `set_capacity` (design
//! §4.1 Admin setup, genesis-direct form like `policy_transition`)
//! installs a `CapacityStock` account per resource id (PDA seeds
//! `["stock", resource_id]`, SPACE 19), and `reconcile` and `commit`
//! now judge COMPAT against the real stocks, passed as remaining
//! accounts after the plans (reconcile) and after the plans and
//! authors (commit): program-owned, deserializing as CapacityStock,
//! distinct pubkeys, and the key must be the PDA of the stored
//! resource id — anything else is the new `StockMismatch`, never a
//! fit judged over partial stocks. A claimed resource with no stock
//! account passed still fails, fail-closed, exactly as the empty-map
//! evaluation failed it before — passing fewer stocks can only refuse
//! more, never admit a claim the stocks do not cover. On a clear mask
//! `commit` debits the stocks atomically (the kernel's commit debits
//! capacity): the pure `apply_debit` helper computes the post-debit
//! balances first and returns None on any failure, so a refused
//! commit changes no stock and a clear commit writes every debit or
//! none — the write-back follows settle's load → mutate → serialize
//! pattern, and the stock accounts are the only remaining accounts
//! `commit` writes. The §4.1 lifecycle is now input-complete for every
//! mask bit: every bit reads its governing on-chain record, and the
//! remaining §4.1 work is execution, not inputs — post-genesis policy
//! amendment through a committed `treasury.policy.v1` transition.
//! This increment closes that execution item: `apply_policy_amendment`
//! (design §4.1) overwrites a target type's PolicyRecord only from a
//! Settled transition whose type is the canonical
//! `POLICY_TRANSITION_TYPE` (SHA-256 of `treasury.policy.v1`), whose
//! declared hash equals the SHA-256 payload hash of the supplied new
//! values, and whose settle was exact (realized == declared) — the
//! amendment is bound to what the transition declared and settled,
//! not to caller-supplied values riding a convenient settled
//! transition. The executor is permissionless: the authority is the
//! settled transition, not the signer, exactly as `commit` and
//! `expire_transition` are permissionless. The one-time execution
//! flag (`policy_amendment_applied`, TransitionRecord SPACE 800 →
//! 801) refuses a second execution of the same transition — without
//! it, an old settled amendment replayed after a newer one would
//! roll the policy back. The genesis-direct `policy_transition`
//! install stays genesis-only in form; after genesis, this is the
//! only path that changes a PolicyRecord.
//! This increment adds `initialize_treasury` (design §4.2), the first
//! §4.2 instruction, in the transition-executed form the design row
//! requires — NOT a genesis-direct multisig call. The design row
//! names `treasury.init.v1` as the authorizing transition, and §10's
//! no-single-key-authority rule is the reason: the treasury's birth
//! state must itself be the execution of a Settled kernel transition,
//! not a fresh multisig signature acting alone (the genesis-direct
//! form was considered and rejected on exactly that ground). The
//! executor is permissionless and pays the rent for the three new
//! accounts; the authority is the settled transition, as with
//! `apply_policy_amendment`. Two deliberate differences from that
//! instruction, both from the init's parameter-free shape: (1) no
//! exact-settle requirement — the init's effect is fixed (zero
//! balances, unset threshold B), so there is no declared payload for
//! an inexact settle to diverge from; Settled + canonical type is
//! the whole binding. (2) no one-time execution flag — the Anchor
//! `init` constraint is the one-time guarantee: a second settled
//! init transition, or a replay of the first, fails at account
//! creation and changes no state. The three capacity stocks are NOT
//! created here: `set_capacity` (slice 18) is their installer, and a
//! second init path would collide with it; a window transition
//! cannot commit without its stocks anyway (COMPAT fails on a
//! missing stock, fail-closed), so no ordering between
//! `set_capacity` and this instruction is enforced. The stage flag
//! is NOT duplicated onto TreasuryState: it lives on Config (set at
//! genesis, flipped by `launch_stage2`), one source of truth.
//! The FeeVault binding is fail-closed and genesis-obligating: the
//! vault account created here must key-equal `Config.fee_vault`, so
//! the genesis authority must pass the `["fee-vault"]` PDA as
//! `fee_vault` at `initialize_config` — a wrong genesis value
//! refuses init visibly (`VaultMismatch`) rather than running with
//! two different "vaults", and there is no Config-update path to
//! paper over it. Threshold B (D-2) starts unset (0): until a
//! governed path sets it, `launch_stage2` can never commit, which
//! is the design's intended state, not a gap.
//! This increment adds `launch_stage2` (design §4.2), the second
//! §4.2 instruction, in the same transition-executed form as the
//! init: a Settled `treasury.launch.v1` transition authorizes the
//! flip of the stage flag on Config from Stage 1 to Stage 2 — never
//! a config check or a fresh authority signature. The B gate is
//! enforced by the launch transition (mandatory BACKING_EVIDENCED
//! constraint report feeding the CONSTRAINTS bit at commit; the
//! manifest's observation evidences the committed fiat total), not
//! recomputed by the handler, which has no fiat total, supply, or
//! price on-chain to compute from; no other instruction checks B ad
//! hoc. The handler's own gates are Settled + canonical type +
//! stage still Stage 1 + threshold B set (nonzero) — the last is
//! the D-2 intended state in executable form: B starts unset at
//! init and no governed path sets it yet, so until that path lands
//! a launch execution refuses with `ThresholdUnset` rather than
//! reading an unset threshold as "any backing passes". No
//! exact-settle requirement (the effect is a fixed flag flip) and no
//! execution flag (the flip itself is the one-time guarantee: a
//! second launch refuses because the stage is no longer Stage 1).
//! Nothing here is deployed: NO devnet deployment in W4.3, NO mainnet step
//! exists in W4. No instruction here moves value.
//!
//! The program id in `declare_id!` is a scaffold placeholder generated for
//! local build / IDL generation only; its keypair was never committed and
//! no deployment was made from it. W4.4 deployment uses a fresh keypair.
//!
//! # Canonical numbers (Carlos's stated assumptions — labeled, never promises)
//!
//! * `FEE_PPM = 7_300`: the 60 bps transaction fee and the 13 bps IDL fee
//!   ride the SAME flow (D-5 CLOSED 2026-10-23 ~16:00 CST; engine-sim T14).
//! * `LOAN_RATE_PPM = 120_000`: 12%, final (Carlos, 2026-10-22).
//! * `PENALTY_FLOOR_PPM = 120_000`: 12% extraction-penalty floor — a
//!   deterrent, not a revenue line (D-4 curve shape above the floor is open).
//! * `CASH_SLEEVE_PPM = 250_000`: 25% cash sleeve (50/25/25 portfolio,
//!   Carlos's 2026-10-22 revision). The lending book's maximum draw is
//!   exactly 1/3 of the sleeve at 60-day terms (math doc §4), so the max
//!   outstanding at 100% uptake is T/12, and movable liquidity — the cash
//!   sleeve NET of that maximum draw — is T/6 (D-10, refined ~16:09 CST).
//! * `LOAN_TERM_DAYS = 60`: loans are 60-day only at pilot (design §4.2);
//!   90-day terms need a term-aware cap (T/8) and Carlos's ruling (§9).
//! * `MAX_LOAN_TO_POSITION_PPM = 500_000`: up to $50 per $100 position.
//!
//! # Claims boundary (design §8, committed-inventory ONLY)
//!
//! This program governs committed inventory only. Secondary-market transfers
//! of the native token outside the program are not bound by rotational/exit
//! rules. The Token-2022 transfer-hook extension is a designed future phase
//! needing its own Carlos decision + legal review — NOT W4.3 scope.

use anchor_lang::prelude::*;

declare_id!("FPGSeLBWEiFKdTuqDbJeKhjXeah5H4c3rqsW4Tzn3RAS");

/// Bridge to the pristine kernel crate (design §1.2, §4.1).
/// See `kernel_bridge.rs` for the launch profile, the evidence-check
/// placement, and the replay-at-commit state discipline.
pub mod kernel_bridge;

/// Parts-per-million denominator for all rate constants.
pub const PPM: u128 = 1_000_000;

/// Combined fee stream, single flow (D-5): 60 bps + 13 bps = 73 bps.
pub const FEE_PPM: u64 = 7_300;

/// Loan rate, 12% — Carlos, final 2026-10-22.
pub const LOAN_RATE_PPM: u64 = 120_000;

/// Extraction-penalty floor, 12% — deterrent, not a revenue line.
pub const PENALTY_FLOOR_PPM: u64 = 120_000;

/// Cash sleeve, 25% of T (50/25/25 portfolio).
pub const CASH_SLEEVE_PPM: u64 = 250_000;

/// Maximum loan per unit of position: $50 per $100.
pub const MAX_LOAN_TO_POSITION_PPM: u64 = 500_000;

/// Loan term at pilot, days. 60-day only (design §4.2).
pub const LOAN_TERM_DAYS: u64 = 60;

/// Canonical transition-type id for `treasury.policy.v1` amendments:
/// SHA-256 of the bytes `treasury.policy.v1`. A policy transition
/// registered under any other type id is not an amendment source —
/// `apply_policy_amendment` refuses it — so the domain tag is
/// machine-checkable, not a naming convention in client code.
pub const POLICY_TRANSITION_TYPE: [u8; 32] = [
    112, 138, 102, 53, 84, 202, 22, 162, 127, 236, 43, 185, 126, 62, 225, 129,
    54, 155, 163, 94, 31, 38, 85, 207, 74, 251, 193, 139, 221, 27, 239, 186,
];

/// Stage flag values on [`Config`].
pub const STAGE_1: u8 = 1;
pub const STAGE_2: u8 = 2;

/// Movable liquidity at the reference sleeve: T/6, ratio-exact.
/// Movable = cash sleeve NET of the lending book's maximum draw
/// (1/3 of the sleeve at 60-day terms): 2/3 × 25% × T = T/6.
pub fn movable_liquidity(treasury: u128) -> u128 {
    treasury * (CASH_SLEEVE_PPM as u128) * 2 / PPM / 3
}

/// Maximum lending-book outstanding at 100% uptake: T/12.
pub fn max_outstanding(treasury: u128) -> u128 {
    treasury * (CASH_SLEEVE_PPM as u128) / PPM / 3
}

/// Canonical transition-type id for `treasury.init.v1`: SHA-256 of
/// the bytes `treasury.init.v1`. Only a Settled transition of this
/// type authorizes `initialize_treasury` — the domain tag is
/// machine-checkable, not a naming convention in client code, exactly
/// as `POLICY_TRANSITION_TYPE` is for amendments.
pub const TREASURY_INIT_TYPE: [u8; 32] = [
    116, 167, 130, 118, 19, 78, 9, 62, 70, 91, 124, 207, 146, 13, 252, 203,
    41, 105, 101, 71, 5, 249, 233, 78, 114, 250, 85, 94, 229, 225, 62, 47,
];

/// Canonical transition-type id for `treasury.launch.v1`: SHA-256 of
/// the bytes `treasury.launch.v1`. Only a Settled transition of this
/// type authorizes `launch_stage2` — the same machine-checkable
/// domain-tag discipline as the init and policy types. A settled
/// transition of any other type is not a launch source
/// (`LaunchMismatch`).
pub const TREASURY_LAUNCH_TYPE: [u8; 32] = [
    24, 150, 201, 135, 106, 198, 184, 96, 196, 151, 146, 230, 11, 122, 79, 108,
    12, 171, 180, 153, 187, 213, 179, 190, 241, 135, 160, 238, 225, 174, 59, 120,
];

/// Bonds sleeve, 50% of T (50/25/25 portfolio, Carlos's 2026-10-22
/// revision). Together with [`CASH_SLEEVE_PPM`] (25%) and
/// [`ARB_SLEEVE_PPM`] (25%) it sums to the whole treasury.
pub const BONDS_SLEEVE_PPM: u64 = 500_000;

/// Arbitrage-bot sleeve, 25% of T (50/25/25 portfolio).
pub const ARB_SLEEVE_PPM: u64 = 250_000;

/// The 50/25/25 sleeve split of a treasury total, as (cash, bonds,
/// arb) in the caller's units. Cash and arb take their floored
/// shares; bonds absorbs the remainder (`total - cash - arb`), so
/// the three sleeves sum to exactly `total` even for totals not
/// divisible by 4 — the remainder is never created or destroyed.
/// Window flows use this rule when they (re)populate sleeve
/// balances; `TreasuryState` stores the resulting ledger, it does
/// not recompute it. Computed in u128 so no u64 total can overflow
/// the intermediate products; each share is at most `total`, so the
/// narrowing casts cannot wrap.
pub fn split_sleeves(total: u64) -> (u64, u64, u64) {
    let t = total as u128;
    let cash = t * (CASH_SLEEVE_PPM as u128) / PPM;
    let arb = t * (ARB_SLEEVE_PPM as u128) / PPM;
    let bonds = t - cash - arb;
    (cash as u64, bonds as u64, arb as u64)
}

#[program]
pub mod treasury_program {
    use super::*;

    /// Genesis (design §4.1): one-time program configuration.
    ///
    /// This is the genesis exception to the "every state-changing
    /// instruction names its kernel transition" rule — no chain exists yet,
    /// so no kernel transition authorizes it. The `init` constraint makes
    /// it one-time: a second call fails and changes no state. Policy lives
    /// in `treasury.policy.v1` transitions, NOT in this account — `Config`
    /// holds only program parameters that are not policy (authority, fee
    /// vault) plus the stage flag (Stage 1 until a `treasury.launch.v1`
    /// transition settles, design §4.2).
    pub fn initialize_config(ctx: Context<InitializeConfig>, fee_vault: Pubkey) -> Result<()> {
        let config = &mut ctx.accounts.config;
        config.authority = ctx.accounts.authority.key();
        config.fee_vault = fee_vault;
        config.stage = STAGE_1;
        config.bump = ctx.bumps.config;
        // Genesis declared chain (design §3): index 0, zero hash —
        // the kernel's genesis `SystemState`. Created here, at
        // genesis, so `commit` never needs a create-on-first-use path
        // (and never needs the `init_if_needed` cargo feature): the
        // chain exists before any transition, and only `commit` on a
        // clear mask advances it.
        let chain = &mut ctx.accounts.declared_chain;
        chain.index = 0;
        chain.hash = [0; 32];
        chain.bump = ctx.bumps.declared_chain;
        Ok(())
    }

    /// Admin setup, genesis-direct form (design §4.1): register an agent.
    ///
    /// The caller must be the `Config` authority. The new record starts
    /// with no roles — roles arrive only via `grant_role` below, so no
    /// agent is born privileged. `influence_raw` / `reputation_raw` are
    /// the kernel's fixed-point raw values (SCALE 1_000_000); reputation
    /// is bounded 0–100 units, matching the launch config (kernel_bridge).
    /// Groups are the kernel's `GroupId` (u16) values, at most
    /// [`MAX_GROUPS`] — the account reserves exactly that many slots.
    pub fn register_agent(
        ctx: Context<RegisterAgent>,
        agent: Pubkey,
        groups: Vec<u16>,
        influence_raw: u64,
        reputation_raw: u64,
    ) -> Result<()> {
        require!(
            ctx.accounts.config.authority == ctx.accounts.authority.key(),
            TreasuryError::Unauthorized
        );
        require!(groups.len() <= MAX_GROUPS, TreasuryError::InvalidGroups);
        require!(
            reputation_raw <= REP_MAX_RAW,
            TreasuryError::InvalidReputation
        );
        let record = &mut ctx.accounts.agent_record;
        record.agent = agent;
        record.active = true;
        record.is_admin = false;
        record.is_oracle = false;
        record.is_evaluator = false;
        record.influence_raw = influence_raw;
        record.reputation_raw = reputation_raw;
        record.groups = groups;
        record.capabilities = Vec::new();
        record.bump = ctx.bumps.agent_record;
        Ok(())
    }

    /// Admin setup, genesis-direct form (design §4.1): grant one role.
    ///
    /// Role codes mirror the kernel's `Role` enum order: 0 = Admin,
    /// 1 = Oracle, 2 = Evaluator. Any other code is an error and changes
    /// nothing. The caller must be the `Config` authority.
    pub fn grant_role(ctx: Context<RoleChange>, agent: Pubkey, role: u8) -> Result<()> {
        require!(
            ctx.accounts.config.authority == ctx.accounts.authority.key(),
            TreasuryError::Unauthorized
        );
        // The PDA seeds already bind this account to `agent`; the check
        // below states that binding explicitly so a future seeds change
        // cannot silently decouple the record from its key.
        require!(
            ctx.accounts.agent_record.agent == agent,
            TreasuryError::AgentMismatch
        );
        ctx.accounts.agent_record.apply_role(role, true)
    }

    /// Admin setup, genesis-direct form (design §4.1): revoke one role.
    ///
    /// Same role codes and authority rule as `grant_role`. Revoking a
    /// role the agent does not hold is a no-op success, not an error —
    /// the end state (role absent) is what the caller asked for.
    pub fn revoke_role(ctx: Context<RoleChange>, agent: Pubkey, role: u8) -> Result<()> {
        require!(
            ctx.accounts.config.authority == ctx.accounts.authority.key(),
            TreasuryError::Unauthorized
        );
        require!(
            ctx.accounts.agent_record.agent == agent,
            TreasuryError::AgentMismatch
        );
        ctx.accounts.agent_record.apply_role(role, false)
    }

    /// Admin setup, genesis-direct form (design §4.1): grant one
    /// capability to an agent.
    ///
    /// The caller must be the `Config` authority, like `grant_role`.
    /// The kernel's `grant_capability` requires the agent to exist
    /// (UnknownAgent) and asks nothing about activity — the account
    /// constraint enforces existence here, and no active flag is
    /// checked, mirroring the kernel. Granting a capability the agent
    /// already holds is a no-op success (the kernel's set insert is
    /// idempotent); granting past [`MAX_CAPABILITIES`] distinct
    /// capabilities is `TooManyCapabilities`. After genesis, grants
    /// are the execution of a committed `treasury.policy.v1`
    /// transition — that execution path is a later increment, and
    /// this genesis-direct grant does not authorize itself
    /// post-genesis. The holdings recorded here are what `commit`'s
    /// CAPABILITY bit reads, summed over plan authors only.
    pub fn grant_capability(
        ctx: Context<RoleChange>,
        agent: Pubkey,
        capability: [u8; 32],
    ) -> Result<()> {
        require!(
            ctx.accounts.config.authority == ctx.accounts.authority.key(),
            TreasuryError::Unauthorized
        );
        require!(
            ctx.accounts.agent_record.agent == agent,
            TreasuryError::AgentMismatch
        );
        ctx.accounts.agent_record.grant_capability(capability)
    }

    /// Admin setup, genesis-direct form (design §4.1): revoke one
    /// capability from an agent.
    ///
    /// Same authority rule as `grant_capability`. The kernel has no
    /// revoke-capability entry point — this mirrors `revoke_role`,
    /// the program's counterpart for roles: revoking a capability the
    /// agent does not hold is a no-op success, and a revoked
    /// capability stops counting toward the CAPABILITY bit at the
    /// next commit (a committed transition's mask is already
    /// decided and is never rewritten). This is the program-layer
    /// half of design §5's revocability: the `OPERATOR_SETTLE`
    /// capability is granted and revoked through these calls at
    /// genesis and through `treasury.policy.v1` after it.
    pub fn revoke_capability(
        ctx: Context<RoleChange>,
        agent: Pubkey,
        capability: [u8; 32],
    ) -> Result<()> {
        require!(
            ctx.accounts.config.authority == ctx.accounts.authority.key(),
            TreasuryError::Unauthorized
        );
        require!(
            ctx.accounts.agent_record.agent == agent,
            TreasuryError::AgentMismatch
        );
        ctx.accounts.agent_record.revoke_capability(capability);
        Ok(())
    }

    /// Admin setup, genesis-direct form (design §4.1): install one
    /// capacity stock.
    ///
    /// The caller must be the `Config` authority, like `grant_role`
    /// and `policy_transition`. The kernel's `set_capacity` takes a
    /// resource id and an amount and overwrites; the program's
    /// install is one-time per resource id (the PDA seeds bind one
    /// stock account per id, and a second install fails at account
    /// creation and changes no state — Duplicate in the design
    /// taxonomy), because the balance afterwards moves only by
    /// `commit`'s atomic debit (and, once it lands, by the window
    /// protocol's replenishment inside a committed flow). Post-genesis
    /// stock changes are the execution of a committed
    /// `treasury.policy.v1` transition — that execution path is a
    /// later increment, and this genesis-direct install does not
    /// authorize itself post-genesis. The installed balances are what
    /// `reconcile` and `commit` judge COMPAT against, and what a clear
    /// commit debits.
    pub fn set_capacity(
        ctx: Context<SetCapacity>,
        resource_id: u16,
        balance: u64,
    ) -> Result<()> {
        require!(
            ctx.accounts.config.authority == ctx.accounts.authority.key(),
            TreasuryError::Unauthorized
        );
        let stock = &mut ctx.accounts.stock;
        stock.resource_id = resource_id;
        stock.balance = balance;
        stock.bump = ctx.bumps.stock;
        emit!(CapacityStockSet {
            resource_id,
            balance,
            authority: ctx.accounts.authority.key(),
        });
        Ok(())
    }

    /// Governance lifecycle (design §4.1): post an observation.
    ///
    /// Feeds every transition: a transition's manifest names its
    /// observation, and the FRESH bit is judged through it at commit.
    /// Only an active Oracle may post (design §4.1: Oracle role, via the
    /// Attestor's `may_observe`). Freshness is judged against the Clock
    /// sysvar, never a client timestamp: a future-dated observation is
    /// rejected, and an expiry at or before the timestamp is a bad
    /// window. Confidence is recorded as recorded; the policy minimum
    /// (`min_obs_confidence`) is enforced at commit, not here. The
    /// `init` constraint makes the observation id one-time — a second
    /// post with the same id fails and changes no state (Duplicate in
    /// the design taxonomy).
    pub fn post_observation(
        ctx: Context<PostObservation>,
        observation_id: u64,
        timestamp: i64,
        expiry: i64,
        confidence: u64,
        content_hash: [u8; 32],
    ) -> Result<()> {
        let record = &ctx.accounts.agent_record;
        require!(
            record.agent == ctx.accounts.oracle.key(),
            TreasuryError::AgentMismatch
        );
        require!(
            record.active && record.is_oracle,
            TreasuryError::Unauthorized
        );
        let now = ctx.accounts.clock.unix_timestamp;
        validate_observation_window(timestamp, expiry, now)?;
        let observation = &mut ctx.accounts.observation;
        observation.observation_id = observation_id;
        observation.oracle = ctx.accounts.oracle.key();
        observation.timestamp = timestamp;
        observation.expiry = expiry;
        observation.confidence = confidence;
        observation.content_hash = content_hash;
        observation.bump = ctx.bumps.observation;
        Ok(())
    }

    /// Governance lifecycle (design §4.1): register a transition.
    ///
    /// Creates the TransitionRecord in Registered state and freezes the
    /// transition's manifest inputs (observation, expiry, declared hash,
    /// and — since the `commit` slice — the parent index and parent hash
    /// the LINEAGE bit will be judged against). The manifest's
    /// observation must already exist: the account constraint on
    /// `observation` enforces that (UnknownObservation in the design
    /// taxonomy — enforced by account existence, not a returned error).
    /// The manifest expiry must be in the future (BadWindow otherwise).
    /// Lineage is deliberately NOT checked here: it is judged at commit
    /// against the head at commit time (design §1.1, bit 10) — but the
    /// parent it is judged against is frozen here, at registration,
    /// because it is a manifest input, not a commit-time claim. The
    /// frozen constraint set, plans, rounds, and signals are stored on
    /// this record by later lifecycle instructions; this slice creates
    /// the record with the planning deadline unset (0) until
    /// `open_planning` sets it, and `objectives_ok` false until
    /// `submit_objectives_check` reports.
    /// The payer's key
    /// is recorded as the proposer: the kernel records the proposer at
    /// registration (`kernel.rs` `register_transition`), and
    /// `open_planning` below admits only that proposer or an Admin.
    /// The transition's type names its PolicyRecord (seeds-bound, so a
    /// record cannot be registered under a stranger's policy), and the
    /// frozen set is derived as the kernel derives it: the type's
    /// mandatory constraints ∪ the proposer's additions, deduped — the
    /// merged set must still fit [`MAX_CONSTRAINTS`], or registration
    /// refuses rather than silently dropping a mandatory gate.
    pub fn register_transition(
        ctx: Context<RegisterTransition>,
        transition_id: u64,
        observation_id: u64,
        transition_type: [u8; 32],
        expires_at: i64,
        declared_hash: [u8; 32],
        parent_index: u64,
        parent_hash: [u8; 32],
        constraint_ids: Vec<[u8; 32]>,
    ) -> Result<()> {
        let now = ctx.accounts.clock.unix_timestamp;
        validate_transition_expiry(expires_at, now)?;
        let frozen = merge_constraint_sets(
            &ctx.accounts.policy_record.mandatory_constraints,
            &constraint_ids,
        );
        validate_constraint_ids(&frozen)?;
        let record = &mut ctx.accounts.transition_record;
        record.transition_id = transition_id;
        record.transition_type = transition_type;
        record.state = STATE_REGISTERED;
        record.proposer = ctx.accounts.payer.key();
        record.observation_id = observation_id;
        record.expires_at = expires_at;
        record.planning_deadline = 0;
        record.plan_count = 0;
        record.round_count = 1;
        record.current_round_total = 0;
        record.current_round_weighted = 0;
        record.prev_round_mean = 0;
        record.objectives_ok = false;
        record.reconciled = false;
        record.constraints = frozen
            .iter()
            .map(|id| ConstraintEntry {
                id: *id,
                status: STATUS_UNAVAILABLE,
            })
            .collect();
        record.declared_hash = declared_hash;
        record.parent_index = parent_index;
        record.parent_hash = parent_hash;
        record.commit_index = 0;
        record.realized_hash = [0; 32];
        record.exceptions = Vec::new();
        record.unresolved_blocking = 0;
        record.policy_amendment_applied = false;
        record.bump = ctx.bumps.transition_record;
        Ok(())
    }

    /// Governance lifecycle (design §4.1): open planning for a transition.
    ///
    /// Mirrors the kernel's `open_planning` exactly: only the recorded
    /// proposer or an Admin may call it, the state must be Registered
    /// (the kernel's `set_state` allows Registered → Planning and
    /// nothing else into Planning), and the deadline is `now +
    /// planning_window` from the transition type's PolicyRecord
    /// (seeds-bound to the record's type; design §9 — the seconds
    /// value is a parameter, not a ruling). Time comes from the Clock sysvar, never
    /// a client timestamp. Program-layer note, stated plainly: the
    /// caller must present their AgentRecord, because Admin status lives
    /// only there — a proposer who has never been registered as an agent
    /// cannot open planning until registered. That is stricter than the
    /// kernel (which records any caller as proposer), and strictly
    /// fail-closed: it can only refuse, never admit an extra caller.
    pub fn open_planning(
        ctx: Context<OpenPlanning>,
        transition_id: u64,
    ) -> Result<()> {
        let _ = transition_id;
        let record = &ctx.accounts.agent_record;
        require!(
            record.agent == ctx.accounts.caller.key(),
            TreasuryError::AgentMismatch
        );
        let transition = &mut ctx.accounts.transition_record;
        can_open_planning(
            ctx.accounts.caller.key(),
            transition.proposer,
            record.is_admin,
            transition.state,
        )?;
        let now = ctx.accounts.clock.unix_timestamp;
        transition.planning_deadline =
            planning_deadline(now, ctx.accounts.policy_record.planning_window)?;
        transition.state = STATE_PLANNING;
        Ok(())
    }

    /// Governance lifecycle (design §4.1): submit a plan for a transition.
    ///
    /// Mirrors the kernel's `submit_plan` in check order: Planning state
    /// and before the deadline (`PlanningClosed` otherwise — the kernel
    /// returns `PlanningClosed` for both a non-Planning state and a
    /// lapsed deadline), the agent active (`NotEligible`), room under
    /// the transition type's PolicyRecord `max_plans` (`TooManyPlans`), and a plan
    /// expiry in the future (`BadWindow`). The kernel's plan-agent ==
    /// caller rule is enforced structurally, not by a returned error:
    /// the `PlanRecord` PDA seeds bind the plan to the signer, and the
    /// AgentRecord must match the signer (`AgentMismatch` otherwise).
    /// The duplicate rule is enforced one layer down:
    /// the `PlanRecord` PDA seeds bind it to (transition, caller), so
    /// a second plan from the same agent fails at account creation and
    /// changes no state (Duplicate in the design taxonomy, enforced by
    /// account existence, like the observation duplicate before it).
    /// Capacity is NOT checked here — the kernel checks aggregate
    /// claims against stocks at reconcile/commit (COMPAT); there is no
    /// courtesy check, so a plan that will not fit is still recorded
    /// and still fails closed later. Claims are stored as a
    /// `Vec<Claim>` (at most [`MAX_CLAIMS`] — an
    /// account-size bound, not a kernel rule; the launch surface is 3
    /// stocks plus allocation classes, so 16 carries headroom) with
    /// distinct resource ids enforced (`InvalidClaims` otherwise — the
    /// kernel's claims are a keyed map, so a duplicate key would be a
    /// silent double-count at reconcile). The event carries the
    /// claims hash (SHA-256 over the little-endian
    /// resource/amount encoding, see [`claims_hash`]), not the claims
    /// themselves. Eligibility gap, stated plainly: the kernel's
    /// `eligible` also requires the agent's groups to overlap the
    /// manifest's `relevant_groups`, but the TransitionRecord does not
    /// carry `relevant_groups` yet — that field lands with the manifest
    /// completion, and the group-overlap check lands with it. This
    /// slice enforces the active half of eligibility; it does not claim
    /// the full kernel rule.
    pub fn submit_plan(
        ctx: Context<SubmitPlan>,
        transition_id: u64,
        claims: Vec<Claim>,
        expires_at: i64,
    ) -> Result<()> {
        let _ = transition_id;
        let caller_key = ctx.accounts.caller.key();
        let record = &ctx.accounts.agent_record;
        require!(
            record.agent == caller_key,
            TreasuryError::AgentMismatch
        );
        validate_claims(&claims)?;
        let now = ctx.accounts.clock.unix_timestamp;
        let transition = &mut ctx.accounts.transition_record;
        can_submit_plan(
            record.active,
            transition.state,
            now,
            transition.planning_deadline,
            transition.plan_count,
            ctx.accounts.policy_record.max_plans,
            expires_at,
        )?;
        let hash = claims_hash(&claims);
        let plan = &mut ctx.accounts.plan_record;
        plan.transition_id = transition.transition_id;
        plan.agent = caller_key;
        plan.expires_at = expires_at;
        plan.claims = claims;
        plan.claims_hash = hash;
        plan.bump = ctx.bumps.plan_record;
        transition.plan_count = transition
            .plan_count
            .checked_add(1)
            .ok_or(TreasuryError::Overflow)?;
        emit!(PlanSubmitted {
            transition_id: transition.transition_id,
            agent: caller_key,
            claims_hash: hash,
        });
        Ok(())
    }

    /// Governance lifecycle (design §4.1): submit a signal for a transition.
    ///
    /// Mirrors the kernel's `submit_signal` in check order: support in
    /// range (`SupportOutOfRange` — support is a kernel `Amount`, so
    /// 1_000_000 raw is 1.0 and anything above it is out of range),
    /// Registered-or-Planning state (`BadState` otherwise — signals are
    /// accepted in both states, exactly as the kernel allows), the
    /// round index under the transition type's PolicyRecord `max_rounds`
    /// (`RoundsExhausted`), the round matching the transition's
    /// current round (`BadState` — a stale round index names a PDA
    /// that is not the live round), and the agent active (`NotEligible`;
    /// the group-overlap half of kernel eligibility awaits
    /// `relevant_groups` on the record, the same carried gap as plans).
    /// The signal agent == caller rule is structural: the
    /// `SignalRecord` PDA seeds bind the signal to the signer, and the
    /// AgentRecord must match the signer (`AgentMismatch` otherwise).
    /// The duplicate rule is structural too: the seeds bind one signal
    /// per (transition, round, agent), so a second signal in the same
    /// round fails at account creation and changes no state
    /// (`AlreadySignalled` in the design taxonomy, enforced by account
    /// existence, like the plan duplicate before it). The raw weight is
    /// stored as supplied; the effective weight (raw × relevance ×
    /// influence × group policy, FlatWeights at launch) is computed
    /// when the recorded signals are replayed through the kernel at
    /// commit — the program never trusts a client-computed total.
    /// Monotone revision, stated plainly: the kernel refuses a
    /// re-signal that moves farther from the previous round's weighted
    /// mean than the agent's previous signal did. The mean half of
    /// that check now exists on-chain: `advance_round` snapshots the
    /// previous round's weighted mean onto the TransitionRecord
    /// (`prev_round_mean`). The other half — the agent's own
    /// previous-round support, which lives on that round's
    /// SignalRecord — is not yet wired: this instruction does not take
    /// the previous SignalRecord as an account, so the check is
    /// enforced at commit replay (the kernel's `submit_signal` during
    /// replay refuses a violating sequence), not at signal time. The
    /// pure check is written and tested (see
    /// [`check_monotone_revision`]); its signal-time wiring is the
    /// follow-up signal-side increment. In round 0 the kernel's
    /// monotone branch is vacuous, as it has no previous round.
    pub fn submit_signal(
        ctx: Context<SubmitSignal>,
        transition_id: u64,
        round: u32,
        raw: u64,
        support: u64,
    ) -> Result<()> {
        let _ = transition_id;
        let caller_key = ctx.accounts.caller.key();
        let record = &ctx.accounts.agent_record;
        require!(
            record.agent == caller_key,
            TreasuryError::AgentMismatch
        );
        let transition = &mut ctx.accounts.transition_record;
        can_submit_signal(
            record.active,
            transition.state,
            support,
            round,
            ctx.accounts.policy_record.max_rounds,
            transition.round_count.checked_sub(1).unwrap_or(0),
        )?;
        // Round aggregates, maintained on-chain so `advance_round`
        // can judge RoundEmpty the way the kernel does (total == ZERO).
        // FlatWeights: relevance and group policy are 1.0, so the
        // effective weight is raw x the signer's recorded influence —
        // computed here from on-chain records, never trusted from the
        // client, and replayed identically at commit.
        let (eff, term) = signal_aggregates(raw, support, record.influence_raw)?;
        transition.current_round_total = transition
            .current_round_total
            .checked_add(eff)
            .ok_or(TreasuryError::Overflow)?;
        transition.current_round_weighted = transition
            .current_round_weighted
            .checked_add(term)
            .ok_or(TreasuryError::Overflow)?;
        let signal = &mut ctx.accounts.signal_record;
        signal.transition_id = transition.transition_id;
        signal.round = round;
        signal.agent = caller_key;
        signal.raw = raw;
        signal.support = support;
        signal.bump = ctx.bumps.signal_record;
        emit!(SignalSubmitted {
            transition_id: transition.transition_id,
            agent: caller_key,
            raw,
            support,
        });
        Ok(())
    }

    /// Governance lifecycle (design §4.1): submit a constraint report.
    ///
    /// Mirrors the kernel's `report_constraint` in check order:
    /// Evaluator role first (`Unauthorized` otherwise — the kernel's
    /// `need(caller, Role::Evaluator)`), then Registered-or-Planning
    /// state (`BadState` otherwise), then membership in the frozen set
    /// (`UnknownConstraint` — a report cannot invent a constraint the
    /// transition was not frozen with). The report overwrites the
    /// entry's status: an Evaluator may correct a report, and the
    /// CONSTRAINTS bit at commit judges the final set (every entry
    /// Pass, and the set non-empty — an empty frozen set can never
    /// commit, exactly as the kernel's mask computes it). The caller
    /// binding is structural, as with plans and signals: the
    /// AgentRecord must match the signer (`AgentMismatch` otherwise).
    /// No new account is created: reports live on the TransitionRecord
    /// as the frozen set (design §3), not as standalone accounts —
    /// so a report is one account write, and commit replay reads the
    /// set from the same record it replays everything else from.
    pub fn submit_constraint_report(
        ctx: Context<SubmitConstraintReport>,
        transition_id: u64,
        constraint_id: [u8; 32],
        status: u8,
    ) -> Result<()> {
        let _ = transition_id;
        let caller_key = ctx.accounts.caller.key();
        let record = &ctx.accounts.agent_record;
        require!(
            record.agent == caller_key,
            TreasuryError::AgentMismatch
        );
        let transition = &mut ctx.accounts.transition_record;
        can_submit_constraint_report(
            record.is_evaluator,
            transition.state,
            transition.constraints.iter().any(|e| e.id == constraint_id),
            status,
        )?;
        set_constraint_status(&mut transition.constraints, constraint_id, status)?;
        emit!(ConstraintReported {
            transition_id: transition.transition_id,
            evaluator: caller_key,
            constraint_id,
            status,
        });
        Ok(())
    }

    /// Governance lifecycle (design §4.1): submit an objectives check.
    ///
    /// Mirrors the kernel's `report_objectives` in check order:
    /// Evaluator role first (`Unauthorized` otherwise — the kernel's
    /// `need(caller, Role::Evaluator)`), then Registered-or-Planning
    /// state (`BadState` otherwise). The report is an independent
    /// Evaluator verdict — a bool on the TransitionRecord, never
    /// derived from plan fields (design §1.1, bit 2) — and it
    /// overwrites: a later report replaces the earlier verdict, and
    /// the OBJECTIVES bit at commit judges the final flag
    /// (`require_objectives && !objectives_ok`). At launch the policy
    /// does not require objectives, so the bit is clear with or
    /// without a report; recording the flag now is what keeps a future
    /// requiring policy honest. The caller binding is structural, as
    /// with constraint reports: the AgentRecord must match the signer
    /// (`AgentMismatch` otherwise). No new account is created: the flag
    /// lives on the TransitionRecord (design §3), so a check is one
    /// account write.
    pub fn submit_objectives_check(
        ctx: Context<SubmitObjectivesCheck>,
        transition_id: u64,
        ok: bool,
    ) -> Result<()> {
        let _ = transition_id;
        let caller_key = ctx.accounts.caller.key();
        let record = &ctx.accounts.agent_record;
        require!(
            record.agent == caller_key,
            TreasuryError::AgentMismatch
        );
        let transition = &mut ctx.accounts.transition_record;
        can_submit_objectives_check(record.is_evaluator, transition.state)?;
        transition.objectives_ok = ok;
        emit!(ObjectivesReported {
            transition_id: transition.transition_id,
            evaluator: caller_key,
            ok,
        });
        Ok(())
    }

    /// Governance lifecycle (design §4.1): advance to the next round.
    ///
    /// Mirrors the kernel's `advance_round` in check order: the
    /// transition must be Registered or Planning (`BadState`
    /// otherwise), the current round must be non-empty — its
    /// effective-weight total, maintained by `submit_signal`, must be
    /// non-zero (`RoundEmpty` otherwise; a round of only zero-weight
    /// signals is empty in the kernel's sense, and this gate refuses
    /// it exactly as the kernel does) — and room must remain under
    /// the transition type's PolicyRecord `max_rounds` (`RoundsExhausted`
    /// otherwise; the empty check precedes the exhaustion check, as
    /// in the kernel). The caller is permissionless, as in the kernel
    /// (the kernel ignores its caller): no AgentRecord is required,
    /// and revision happens between rounds, never within one. On
    /// success the previous round's weighted mean is snapshotted onto
    /// the record (`prev_round_mean`, for the monotone-revision check),
    /// the current-round aggregates reset to zero, and the round count
    /// increments — the new current round is `round_count - 1`.
    pub fn advance_round(
        ctx: Context<AdvanceRound>,
        transition_id: u64,
    ) -> Result<()> {
        let _ = transition_id;
        let transition = &mut ctx.accounts.transition_record;
        can_advance_round(
            transition.state,
            transition.current_round_total,
            transition.round_count,
            ctx.accounts.policy_record.max_rounds,
        )?;
        transition.prev_round_mean =
            round_mean(transition.current_round_weighted, transition.current_round_total)?;
        transition.current_round_total = 0;
        transition.current_round_weighted = 0;
        transition.round_count = transition
            .round_count
            .checked_add(1)
            .ok_or(TreasuryError::Overflow)?;
        emit!(RoundAdvanced {
            transition_id: transition.transition_id,
            round: transition.round_count - 1,
        });
        Ok(())
    }

    /// Governance lifecycle (design §4.1): reconcile a transition.
    ///
    /// Mirrors the kernel's `reconcile` in check order: the transition
    /// must be Planning (`BadState` otherwise — the kernel's
    /// `set_state` allows Planning → Committable and nothing else into
    /// Committable), and `now` must be at or past the planning
    /// deadline (`PlanningStillOpen` otherwise; at the deadline itself
    /// it passes, as the kernel's `now < planning_deadline` test).
    /// The caller is permissionless, as in the kernel (the kernel
    /// ignores its reconcile caller): no AgentRecord is required. On
    /// success the state moves to Committable and `reconciled` records
    /// the capacity fit — computed once, here. A false fit still
    /// advances the state: the COMPAT bit at commit (`reconciled &&
    /// fits`, re-checked) then refuses every commit of the transition,
    /// and its only exits are reject/expire. That is the kernel's
    /// fail-closed shape, not a half-state: a transition that cannot
    /// fit never commits, and no claim is partially applied.
    /// The fit is judged against the real stocks: the remaining
    /// accounts are the transition's plans first, then its
    /// CapacityStock accounts, in any order within each segment. The
    /// plan segment follows commit's completeness discipline exactly:
    /// exactly `plan_count` plans, each program-owned, each naming
    /// this transition, each pubkey distinct — anything else is
    /// `PlanMismatch`, never a fit judged over a partial claim set.
    /// Each stock account must be program-owned, deserialize as a
    /// CapacityStock, be listed once, and sit at the PDA of its stored
    /// resource id — anything else is `StockMismatch`, never a fit
    /// judged over partial stocks. A claimed resource with no stock
    /// account passed fails the fit, fail-closed: passing fewer stocks
    /// can only refuse more, never admit a claim the stocks do not
    /// cover. A plan-free transition fits vacuously (commit's PLANS
    /// bit refuses it anyway for having no plans). The fit is computed
    /// once, here, and re-checked at commit against the stocks passed
    /// there — the two calls may see different stock sets, and commit
    /// is the binding check: a transition reconciled true against
    /// ample stocks still refuses at commit if the stocks passed
    /// there do not cover the claims.
    pub fn reconcile(
        ctx: Context<Reconcile>,
        transition_id: u64,
    ) -> Result<()> {
        let _ = transition_id;
        let transition = &mut ctx.accounts.transition_record;
        let now = ctx.accounts.clock.unix_timestamp;
        can_reconcile(transition.state, now, transition.planning_deadline)?;
        let (claims, stocks) = load_plan_claims_and_stocks(
            ctx.remaining_accounts,
            transition.transition_id,
            transition.plan_count as usize,
        )?;
        let fits = fits_capacity(&claims, &stocks);
        transition.reconciled = fits;
        transition.state = STATE_COMMITTABLE;
        emit!(Reconciled {
            transition_id: transition.transition_id,
            fits,
        });
        Ok(())
    }

    /// Governance lifecycle (design §4.1): commit a transition.
    ///
    /// Permissionless, as in the kernel — the kernel ignores its commit
    /// caller, so no AgentRecord is required of the caller and the
    /// predicate, not the caller, decides. The transition must be
    /// Committable (`BadState` otherwise — the kernel's `set_state`
    /// allows Committable → Committed and nothing else into
    /// Committed). The full 13-bit mask is then computed by the pure
    /// `commit_fail_mask` helper from on-chain records only:
    /// constraint reports and the objectives flag on the
    /// TransitionRecord, the last round's aggregates on the same
    /// record, the observation account (FRESH), the Clock (planning
    /// deadline, manifest expiry), the PlanRecord accounts passed as
    /// remaining accounts (PLANS expiries, COMPAT aggregate), the
    /// plan authors' AgentRecords passed after them (CAPABILITY:
    /// holdings + influence, summed per requirement), and the
    /// `DeclaredChain` (LINEAGE). A non-zero mask refuses with
    /// `CommitFailed` and changes nothing — no state move, no event,
    /// no chain advance (a failed transaction rolls back in any case).
    /// The
    /// mask is logged with `msg!`: Anchor error codes are unit
    /// variants, so the code alone cannot carry it — the detail is in
    /// the program logs and recomputable from public account state.
    /// On a clear mask the transition moves to Committed and the chain
    /// advances to the transition's declared hash, exactly as the
    /// kernel's `commit` advances its declared state and debits
    /// capacity. The COMPAT check is the fit of the claim aggregate
    /// against the CapacityStock balances passed after the authors
    /// (see the `commit_fail_mask` docs), judged through
    /// `fits_capacity` — a claimed resource with no stock account
    /// passed fails, fail-closed — and the debit is the commit's
    /// write: on a clear mask only, the handler subtracts each
    /// aggregate from its stock (via `apply_debit`, computed whole
    /// before any write, so a failure writes nothing). The chain
    /// account is created at genesis by `initialize_config` (index 0,
    /// zero hash — the kernel's genesis declared state) and read here;
    /// a missing chain fails validation, fail-closed.
    /// Remaining-accounts completeness is enforced, not assumed:
    /// exactly `plan_count` plan accounts, each program-owned, each
    /// naming this transition, each pubkey distinct — anything else
    /// is `PlanMismatch`, never a partial mask — followed by one
    /// AgentRecord per distinct plan author, in any order, each
    /// program-owned and deserializing as an AgentRecord, each naming
    /// a plan author, each pubkey distinct — anything else is
    /// `CommitAccountsMismatch`, never a mask computed over partial
    /// holdings — followed by the stock segment, each program-owned
    /// CapacityStock at the PDA of its stored resource id, writable,
    /// distinct — anything else is `StockMismatch`, never a fit over
    /// partial stocks. The author records are read-only here; the
    /// stocks are the only remaining accounts a clear commit writes
    /// (the debit), and a refused commit writes nothing at all.
    /// The observation id is an
    /// instruction argument (the proven pattern of `register_transition`)
    /// and must match the record's, else `ObservationMismatch` — the
    /// FRESH bit is never judged through a stranger's observation.
    pub fn commit(
        ctx: Context<Commit>,
        transition_id: u64,
        observation_id: u64,
    ) -> Result<()> {
        let _ = transition_id;
        let transition = &ctx.accounts.transition_record;
        require!(
            transition.state == STATE_COMMITTABLE,
            TreasuryError::BadState
        );
        require!(
            observation_id == transition.observation_id,
            TreasuryError::ObservationMismatch
        );
        // Remaining accounts are this transition's plans first, then
        // one AgentRecord per distinct plan author: the plans are
        // exactly plan_count, program-owned, naming this transition,
        // pubkeys distinct. A caller who omits a plan would otherwise
        // get a mask computed over a partial claim set — a weaker
        // predicate wearing a complete mask's clothes — and a caller
        // who omits an author would get a CAPABILITY bit computed
        // over partial holdings, the same failure in the other
        // direction. Plans are positional (first), authors follow.
        let plan_count = transition.plan_count as usize;
        require!(
            ctx.remaining_accounts.len() >= plan_count,
            TreasuryError::PlanMismatch
        );
        let mut claims: Vec<(u16, u64)> = Vec::new();
        let mut min_plan_expiry: Option<i64> = None;
        let mut seen: Vec<Pubkey> = Vec::new();
        let mut authors: Vec<Pubkey> = Vec::new();
        for account in ctx.remaining_accounts.iter().take(plan_count) {
            require!(
                account.owner == &crate::ID,
                TreasuryError::PlanMismatch
            );
            require!(
                !seen.contains(account.key),
                TreasuryError::PlanMismatch
            );
            seen.push(*account.key);
            let data = account.data.borrow();
            let mut slice: &[u8] = &data;
            let plan = PlanRecord::try_deserialize(&mut slice)
                .map_err(|_| TreasuryError::PlanMismatch)?;
            require!(
                plan.transition_id == transition.transition_id,
                TreasuryError::PlanMismatch
            );
            if !authors.contains(&plan.agent) {
                authors.push(plan.agent);
            }
            for claim in plan.claims.iter() {
                claims.push((claim.resource_id, claim.amount));
            }
            min_plan_expiry = Some(match min_plan_expiry {
                Some(prev) => prev.min(plan.expires_at),
                None => plan.expires_at,
            });
        }
        // The author records must match the distinct-author set
        // exactly: count, membership, and distinctness. Fewer
        // remaining accounts than plans + authors fails here, not
        // at the plan loop above — the take() above reads only what
        // exists, so a short slice is a mismatch of the author set.
        // A long tail is not an extra account, never ignored: it is
        // the stock segment, parsed below.
        require!(
            ctx.remaining_accounts.len() >= plan_count + authors.len(),
            TreasuryError::CommitAccountsMismatch
        );
        let mut author_holdings: Vec<(u64, Vec<[u8; 32]>)> = Vec::new();
        let mut seen_authors: Vec<Pubkey> = Vec::new();
        for account in ctx
            .remaining_accounts
            .iter()
            .skip(plan_count)
            .take(authors.len())
        {
            require!(
                account.owner == &crate::ID,
                TreasuryError::CommitAccountsMismatch
            );
            require!(
                !seen_authors.contains(account.key),
                TreasuryError::CommitAccountsMismatch
            );
            seen_authors.push(*account.key);
            let data = account.data.borrow();
            let mut slice: &[u8] = &data;
            let record = AgentRecord::try_deserialize(&mut slice)
                .map_err(|_| TreasuryError::CommitAccountsMismatch)?;
            require!(
                authors.contains(&record.agent),
                TreasuryError::CommitAccountsMismatch
            );
            author_holdings.push((record.influence_raw, record.capabilities));
        }
        // The stock segment follows the authors: every account there
        // must be a program-owned CapacityStock at the PDA of its
        // stored resource id, listed once (see `load_stock_balances`).
        // Stock accounts must be writable — a clear mask debits them
        // below, and a read-only stock could not take the write, so
        // it is refused here rather than after the mask is decided.
        // The fit is judged against these real balances, the same
        // rule `reconcile` applied: a claimed resource with no stock
        // account passed fails, fail-closed.
        let stock_start = plan_count + authors.len();
        for account in ctx.remaining_accounts.iter().skip(stock_start) {
            require!(
                account.is_writable,
                TreasuryError::StockMismatch
            );
        }
        let stocks = load_stock_balances(&ctx.remaining_accounts[stock_start..])?;
        let fits = fits_capacity(&claims, &stocks);
        // The mask inputs come from the transition type's PolicyRecord,
        // not the launch policy: a type whose policy was amended commits
        // under its own record. The record is seeds-bound to the
        // transition's type, so a stranger's policy cannot be
        // substituted for the governing type's.
        let policy = &ctx.accounts.policy_record;
        // CAPABILITY from live inputs: the type's required
        // capabilities against the plan authors' holdings and
        // influence, exactly as the kernel computes it — never the
        // hard-coded true the earlier slices carried as an interim.
        let capability_covered =
            capability_covered(&policy.required_capabilities, &author_holdings);
        let now = ctx.accounts.clock.unix_timestamp;
        let constraint_statuses: Vec<u8> = transition
            .constraints
            .iter()
            .map(|entry| entry.status)
            .collect();
        let inputs = CommitInputs {
            constraint_statuses: &constraint_statuses,
            objectives_ok: transition.objectives_ok,
            require_objectives: policy.require_objectives,
            round_total: transition.current_round_total,
            round_weighted: transition.current_round_weighted,
            quorum_required: policy.quorum_required,
            confidence_threshold: policy.confidence_threshold,
            reconciled: transition.reconciled,
            fits,
            unresolved_blocking: transition.unresolved_blocking,
            obs_timestamp: ctx.accounts.observation.timestamp,
            obs_expiry: ctx.accounts.observation.expiry,
            obs_confidence: ctx.accounts.observation.confidence,
            min_obs_confidence: policy.min_obs_confidence,
            now,
            planning_deadline: transition.planning_deadline,
            expires_at: transition.expires_at,
            parent_index: transition.parent_index,
            parent_hash: transition.parent_hash,
            chain_index: ctx.accounts.declared_chain.index,
            chain_hash: ctx.accounts.declared_chain.hash,
            plan_count: transition.plan_count,
            min_plan_expiry,
            capability_covered,
        };
        let mask = commit_fail_mask(&inputs);
        if mask != 0 {
            msg!("commit refused: commit_fail_mask={:#06x}", mask);
            return Err(TreasuryError::CommitFailed.into());
        }
        // Clear mask: debit the stocks atomically, the kernel's commit
        // semantics. `apply_debit` computes the post-debit balances
        // first and returns None on any failure, so a failure here is
        // unreachable after a passing fit — but it is an error, never
        // a partial debit: nothing is written until every new balance
        // is computed. The write-back follows settle's load → mutate
        // → serialize pattern, one record at a time; a stock account
        // is identified by its stored resource id, which the PDA
        // check above bound to its key. Only the stocks move here;
        // the author records stay read-only, as before.
        let new_balances = apply_debit(&stocks, &claims)
            .ok_or(TreasuryError::Overflow)?;
        for account in ctx.remaining_accounts.iter().skip(stock_start) {
            let data = account
                .try_borrow_data()
                .map_err(|_| TreasuryError::StockMismatch)?;
            let mut slice: &[u8] = &data;
            let mut stock = CapacityStock::try_deserialize(&mut slice)
                .map_err(|_| TreasuryError::StockMismatch)?;
            if let Some((_, balance)) = new_balances
                .iter()
                .find(|(id, _)| *id == stock.resource_id)
            {
                stock.balance = *balance;
                drop(data);
                let mut data = account
                    .try_borrow_mut_data()
                    .map_err(|_| TreasuryError::StockMismatch)?;
                let mut dst: &mut [u8] = &mut data;
                stock
                    .try_serialize(&mut dst)
                    .map_err(|_| TreasuryError::StockMismatch)?;
            }
        }
        let transition = &mut ctx.accounts.transition_record;
        transition.state = STATE_COMMITTED;
        let chain = &mut ctx.accounts.declared_chain;
        chain.index = chain
            .index
            .checked_add(1)
            .ok_or(TreasuryError::Overflow)?;
        chain.hash = transition.declared_hash;
        // The settle slice's follow-up, closed here: the new chain
        // index is recorded on the transition as its commit index, so
        // settle's event can name the commit it settles (and so the
        // commit event's index and the settle event's index agree —
        // they are the same number, read from the same record).
        transition.commit_index = chain.index;
        // The bump stays as genesis set it: this account's seeds use
        // its stored bump (`bump = declared_chain.bump`), so it is not
        // in `ctx.bumps` and must not be re-derived here.
        emit!(Committed {
            transition_id: transition.transition_id,
            commit_index: chain.index,
            declared_hash: transition.declared_hash,
        });
        Ok(())
    }

    /// Governance lifecycle (design §4.1): record evidence for a
    /// committed transition.
    ///
    /// The EvidenceAttestor's write handle, at the program layer (see
    /// `kernel_bridge`: the kernel-policy EvidenceAttestor is std-only
    /// and cannot be vendored into SBF, so the log lives in the account
    /// below, not in a hook). Only an active Oracle may record — the
    /// kernel's RoleAttestor rule (Oracle may observe/settle) plus the
    /// active flag, the same fail-closed tightening `post_observation`
    /// applies — and the transition must be Committed: the kernel's
    /// settle gate, applied one step early, so evidence cannot be
    /// recorded for a transition that has not committed, nor re-recorded
    /// after it settled (a Settled transition is not Committed). The
    /// record is one-time: the PDA seeds bind one EvidenceRecord per
    /// transition, so a second record fails at account creation and
    /// changes no state (Duplicate in the design taxonomy, enforced by
    /// account existence, like the observation duplicate before it).
    /// The program cannot verify that the realized outcome exists in
    /// the world — that is the recorder's attestation, and the design
    /// says so plainly: recording early records a hash that settle
    /// will then refuse to match, so a premature record harms only the
    /// recorder's own transition. Settle (next increment) checks the
    /// presented hash against this record before it ever calls the
    /// kernel's settle; no record, or a mismatched hash, refuses with
    /// Unauthorized and leaves the transition Committed for a correct
    /// settlement to retry. Recording evidence changes no transition
    /// state: the transition account is read, not written, and the
    /// EvidenceRecord is the only write.
    pub fn record_evidence(
        ctx: Context<RecordEvidence>,
        transition_id: u64,
        realized_hash: [u8; 32],
    ) -> Result<()> {
        let _ = transition_id;
        let caller_key = ctx.accounts.oracle.key();
        let record = &ctx.accounts.agent_record;
        require!(
            record.agent == caller_key,
            TreasuryError::AgentMismatch
        );
        let transition = &ctx.accounts.transition_record;
        can_record_evidence(record.active, record.is_oracle, transition.state)?;
        let evidence = &mut ctx.accounts.evidence_record;
        evidence.transition_id = transition.transition_id;
        evidence.realized_hash = realized_hash;
        evidence.bump = ctx.bumps.evidence_record;
        emit!(EvidenceRecorded {
            transition_id: transition.transition_id,
            realized_hash,
        });
        Ok(())
    }

    /// Governance lifecycle (design §4.1): settle a committed transition.
    ///
    /// Mirrors the kernel's `settle` in check order: the attestor gate
    /// first, then the state gate, then the attribution rules. The
    /// attestor gate at the program layer is a role + evidence check
    /// (the kernel-policy EvidenceAttestor is std-only and cannot be
    /// vendored into SBF, so the log lives in the EvidenceRecord
    /// account — see `kernel_bridge`): the caller must be an active
    /// Oracle, and the presented realized hash must equal the
    /// independently recorded EvidenceRecord. A missing EvidenceRecord
    /// fails account validation before this handler runs — still a
    /// refusal that changes no state; the `Unauthorized` path below
    /// covers a present-but-mismatched record. The state gate is the
    /// kernel's: only a Committed transition settles, so a settled
    /// transition cannot re-settle and a refused settle leaves the
    /// transition Committed for a correct settlement to retry.
    /// Attributions (kernel `settle`'s third check, after the state
    /// gate): every attributed agent must have an AgentRecord passed
    /// (`UnknownAgent` otherwise) and a plan for this transition
    /// (`NotParticipant` otherwise — the kernel's `plans.contains_key`
    /// test, judged over the complete plan set passed as remaining
    /// accounts, never a partial one). Deltas are summed per agent in
    /// first-appearance order (order-independent, as the kernel sums
    /// them), capped at ±20 units, and an inexact settle — realized
    /// hash ≠ declared hash — halves positive credit only, exactly as
    /// the kernel's settle does; reputation is clamped to 0–100 units.
    /// The attributed agents' AgentRecords are remaining accounts,
    /// classified by discriminator (AgentRecord vs PlanRecord): the
    /// agent set must match the attributed set exactly (one record per
    /// unique attributed agent, distinct pubkeys, writable — extras or
    /// duplicates are `SettleAccountsMismatch`), and the plan set must
    /// match the transition's plan count exactly (program-owned, naming
    /// this transition, distinct pubkeys — `PlanMismatch` otherwise,
    /// the commit slice's completeness rule). Remaining accounts are
    /// loaded and written by hand, as in `commit`: `Account::try_from`
    /// cannot satisfy the remaining-accounts lifetime here
    /// (AccountInfo is invariant over its lifetime, and the Context's
    /// lifetimes do not line up), so each record is deserialized from
    /// its data bytes (discriminator-checked, owner-checked
    /// explicitly — the manual load checks neither on its own) and
    /// the mutated record is serialized back explicitly — Anchor
    /// writes back Context accounts on exit but not remaining
    /// accounts, so a load without the explicit write-back would
    /// silently drop the reputation write, and this handler does not
    /// rely on it. Settle takes no Clock: the program
    /// keeps no kernel-time state, so there is no last-timestamp for
    /// the kernel's `tick` to check against — the time-judged mask
    /// bits were already decided at commit, and settle judges only
    /// attestation, state, and attribution. The event carries the
    /// commit index recorded at commit time, the realized hash, and
    /// `exact` (declared == realized).
    pub fn settle(
        ctx: Context<Settle>,
        transition_id: u64,
        realized_hash: [u8; 32],
        attributions: Vec<Attribution>,
    ) -> Result<()> {
        let _ = transition_id;
        let caller_key = ctx.accounts.oracle.key();
        let record = &ctx.accounts.agent_record;
        require!(
            record.agent == caller_key,
            TreasuryError::AgentMismatch
        );
        let transition = &ctx.accounts.transition_record;
        let evidence_matches =
            ctx.accounts.evidence_record.realized_hash == realized_hash;
        can_settle(
            record.active,
            record.is_oracle,
            evidence_matches,
            transition.state,
        )?;
        // Sum first: the kernel sums per agent (order-independent),
        // and the agent set below is the unique attributed set.
        let summed = sum_attributions(&attributions);
        // Classify the remaining accounts by discriminator. Reading
        // the discriminator is read-only; ownership is enforced by
        // the typed loads below, mapped to each set's error. Indices,
        // not references, are stored: `remaining_accounts` borrows
        // `ctx` with a shorter lifetime than the AccountInfos
        // themselves (AccountInfo is invariant over its lifetime),
        // so a stored `&AccountInfo` does not compile — each pass
        // below re-indexes the slice instead.
        let mut agent_idx: Vec<usize> = Vec::new();
        let mut plan_idx: Vec<usize> = Vec::new();
        for (i, info) in ctx.remaining_accounts.iter().enumerate() {
            let data = info.data.borrow();
            require!(
                data.len() >= 8,
                TreasuryError::SettleAccountsMismatch
            );
            let discriminator = &data[..8];
            if discriminator == AgentRecord::DISCRIMINATOR {
                agent_idx.push(i);
            } else if discriminator == PlanRecord::DISCRIMINATOR {
                plan_idx.push(i);
            } else {
                return Err(TreasuryError::SettleAccountsMismatch.into());
            }
        }
        // The plan set must be complete: exactly plan_count plans,
        // each program-owned, naming this transition, distinct
        // pubkeys — the commit slice's rule, so membership below is
        // judged over the whole set, never a convenient subset.
        require!(
            plan_idx.len() == transition.plan_count as usize,
            TreasuryError::PlanMismatch
        );
        let mut plan_agents: Vec<Pubkey> = Vec::new();
        let mut seen_plans: Vec<Pubkey> = Vec::new();
        for i in plan_idx.iter() {
            let info = &ctx.remaining_accounts[*i];
            require!(
                !seen_plans.contains(info.key),
                TreasuryError::PlanMismatch
            );
            seen_plans.push(*info.key);
            // Manual load (see handler docs): owner checked
            // explicitly, discriminator checked by try_deserialize.
            require!(
                info.owner == &crate::ID,
                TreasuryError::PlanMismatch
            );
            let data = info
                .try_borrow_data()
                .map_err(|_| TreasuryError::PlanMismatch)?;
            let mut slice: &[u8] = &data;
            let plan = PlanRecord::try_deserialize(&mut slice)
                .map_err(|_| TreasuryError::PlanMismatch)?;
            require!(
                plan.transition_id == transition.transition_id,
                TreasuryError::PlanMismatch
            );
            plan_agents.push(plan.agent);
        }
        // The agent set must match the attributed set exactly: one
        // record per unique attributed agent, distinct pubkeys,
        // writable (a non-writable record could not take the write).
        require!(
            agent_idx.len() == summed.len(),
            TreasuryError::SettleAccountsMismatch
        );
        let mut seen_agents: Vec<Pubkey> = Vec::new();
        for i in agent_idx.iter() {
            let info = &ctx.remaining_accounts[*i];
            require!(
                !seen_agents.contains(info.key),
                TreasuryError::SettleAccountsMismatch
            );
            seen_agents.push(*info.key);
            require!(
                info.owner == &crate::ID,
                TreasuryError::SettleAccountsMismatch
            );
            require!(
                info.is_writable,
                TreasuryError::SettleAccountsMismatch
            );
        }
        // Resolve each attributed agent to its record (UnknownAgent),
        // then to a plan (NotParticipant) — the kernel's order: agent
        // existence before plan membership.
        let mut writes: Vec<(Pubkey, u64)> = Vec::new();
        let exact = realized_hash == transition.declared_hash;
        for (agent, net) in summed.iter() {
            let mut found: Option<u64> = None;
            for i in agent_idx.iter() {
                let info = &ctx.remaining_accounts[*i];
                let data = info
                    .try_borrow_data()
                    .map_err(|_| TreasuryError::SettleAccountsMismatch)?;
                let mut slice: &[u8] = &data;
                let loaded = AgentRecord::try_deserialize(&mut slice)
                    .map_err(|_| TreasuryError::SettleAccountsMismatch)?;
                if loaded.agent == *agent {
                    found = Some(loaded.reputation_raw);
                    break;
                }
            }
            let old_rep = found.ok_or(TreasuryError::UnknownAgent)?;
            require!(
                plan_agents.contains(agent),
                TreasuryError::NotParticipant
            );
            writes.push((*agent, settle_attribution(old_rep, *net, exact)));
        }
        // All checks passed: write the reputations, then move state.
        // Each record is re-loaded here so the write path owns exactly
        // one record at a time (load → mutate → serialize back).
        // Settle never changes a record's shape (only reputation_raw,
        // a fixed-width field), so the serialized length is unchanged
        // and the write-back cannot overrun the account data.
        for (agent, new_rep) in writes.iter() {
            for i in agent_idx.iter() {
                let info = &ctx.remaining_accounts[*i];
                let data = info
                    .try_borrow_data()
                    .map_err(|_| TreasuryError::SettleAccountsMismatch)?;
                let mut slice: &[u8] = &data;
                let mut loaded = AgentRecord::try_deserialize(&mut slice)
                    .map_err(|_| TreasuryError::SettleAccountsMismatch)?;
                if loaded.agent == *agent {
                    loaded.reputation_raw = *new_rep;
                    drop(data);
                    let mut data = info
                        .try_borrow_mut_data()
                        .map_err(|_| TreasuryError::SettleAccountsMismatch)?;
                    let mut dst: &mut [u8] = &mut data;
                    loaded
                        .try_serialize(&mut dst)
                        .map_err(|_| TreasuryError::SettleAccountsMismatch)?;
                    break;
                }
            }
        }
        let transition = &mut ctx.accounts.transition_record;
        transition.realized_hash = realized_hash;
        transition.state = STATE_SETTLED;
        emit!(Settled {
            transition_id: transition.transition_id,
            commit_index: transition.commit_index,
            realized_hash,
            exact,
        });
        Ok(())
    }

    /// Governance lifecycle (design §4.1): raise an exception on a
    /// transition.
    ///
    /// Mirrors the kernel's `raise_exception` in check order:
    /// Evaluator role first (`Unauthorized` otherwise — the kernel's
    /// `need(caller, Role::Evaluator)`), then the state gate
    /// (`BadState` unless Registered, Planning, or Committable — the
    /// kernel's pre-commit states; a Committed transition cannot gain
    /// a new exception, and a Settled/Rejected/Expired one cannot
    /// either), then the disposition code (`InvalidDisposition`
    /// outside 0..=3, in the kernel's `Disposition` order:
    /// Informational, Review, Block, Emergency), then the duplicate
    /// rule (`DuplicateException` — the kernel's `Duplicate`, returned
    /// here rather than enforced by PDA creation, since exceptions
    /// live on the TransitionRecord, not in their own accounts), then
    /// the account-size bound (`TooManyExceptions` at 8 — the same
    /// bound shape as the frozen constraint set, and the only
    /// program-layer addition to the kernel's rule; the kernel's map
    /// is unbounded, an on-chain account is not). On success the
    /// entry is appended unresolved and, for a Block or Emergency
    /// disposition, `unresolved_blocking` increments (checked, never
    /// wrapping) — the count `commit` feeds to the EXCEPTIONS bit.
    /// Raising a blocking exception on a Committable transition is a
    /// pre-commit veto by construction: the transition cannot commit
    /// until the exception resolves. The caller binding is structural,
    /// as with constraint reports: the AgentRecord must match the
    /// signer (`AgentMismatch` otherwise). No Clock is taken: the
    /// kernel's `tick` has no program counterpart (the program keeps
    /// no kernel-time state, as with settle), and no exception rule
    /// is time-judged.
    pub fn raise_exception(
        ctx: Context<RaiseException>,
        transition_id: u64,
        exception_id: [u8; 32],
        disposition: u8,
    ) -> Result<()> {
        let _ = transition_id;
        let caller_key = ctx.accounts.caller.key();
        let record = &ctx.accounts.agent_record;
        require!(
            record.agent == caller_key,
            TreasuryError::AgentMismatch
        );
        let transition = &mut ctx.accounts.transition_record;
        can_raise_exception(
            record.is_evaluator,
            transition.state,
            disposition,
            transition.exceptions.iter().any(|e| e.id == exception_id),
            transition.exceptions.len(),
        )?;
        let blocking = is_blocking_disposition(disposition);
        transition.exceptions.push(ExceptionEntry {
            id: exception_id,
            disposition,
            resolved: false,
        });
        if blocking {
            transition.unresolved_blocking = transition
                .unresolved_blocking
                .checked_add(1)
                .ok_or(TreasuryError::Overflow)?;
        }
        emit!(ExceptionRaised {
            transition_id: transition.transition_id,
            evaluator: caller_key,
            exception_id,
            disposition,
            blocking,
        });
        Ok(())
    }

    /// Governance lifecycle (design §4.1): resolve an exception on a
    /// transition.
    ///
    /// Mirrors the kernel's `resolve_exception` exactly, including
    /// the two properties a stricter program would hide: there is NO
    /// state gate in the kernel — only the Evaluator role
    /// (`Unauthorized` otherwise) and the exception's existence
    /// (`UnknownException` otherwise) are checked — so this handler
    /// imposes none either. Resolving after commit cannot rewrite the
    /// mask already decided at commit (the EXCEPTIONS bit was judged
    /// then, from the count at that moment), and resolving on a
    /// Rejected or Expired transition changes a record nothing will
    /// ever commit — bookkeeping, not authority. Re-resolving an
    /// already-resolved exception is a no-op success, exactly as in
    /// the kernel: the entry stays resolved and the count does not
    /// decrement twice (a double decrement would undercount the
    /// remaining blockers and weaken the bit). A first resolution of
    /// a Block or Emergency exception decrements `unresolved_blocking`
    /// (checked, never wrapping below zero); Informational and Review
    /// exceptions never counted, so resolving them moves no count.
    /// The caller binding is structural, as with raise: the
    /// AgentRecord must match the signer.
    pub fn resolve_exception(
        ctx: Context<ResolveException>,
        transition_id: u64,
        exception_id: [u8; 32],
    ) -> Result<()> {
        let _ = transition_id;
        let caller_key = ctx.accounts.caller.key();
        let record = &ctx.accounts.agent_record;
        require!(
            record.agent == caller_key,
            TreasuryError::AgentMismatch
        );
        let transition = &mut ctx.accounts.transition_record;
        can_resolve_exception(
            record.is_evaluator,
            transition.exceptions.iter().any(|e| e.id == exception_id),
        )?;
        for entry in transition.exceptions.iter_mut() {
            if entry.id == exception_id && !entry.resolved {
                entry.resolved = true;
                if is_blocking_disposition(entry.disposition) {
                    transition.unresolved_blocking = transition
                        .unresolved_blocking
                        .checked_sub(1)
                        .ok_or(TreasuryError::Overflow)?;
                }
                break;
            }
        }
        emit!(ExceptionResolved {
            transition_id: transition.transition_id,
            evaluator: caller_key,
            exception_id,
        });
        Ok(())
    }

    /// Governance lifecycle (design §4.1): veto a pre-commit
    /// transition.
    ///
    /// Mirrors the kernel's `reject` exactly: the caller must hold
    /// the Evaluator role or the Admin role (`Unauthorized`
    /// otherwise — the kernel's `has_role` test, either role, not
    /// both), and the transition must be Registered, Planning, or
    /// Committable (`BadState` otherwise — the kernel's `set_state`
    /// gate, which admits only the pre-commit states to Rejected).
    /// Rejection is terminal and not idempotent: rejecting an
    /// already-Rejected (or Committed, Settled, Expired) transition
    /// refuses with `BadState`, it does not re-emit. The kernel's
    /// `tick(now)` has no program counterpart here — the program
    /// keeps no kernel-time state, as with exceptions — and reject
    /// judges no time rule, so no Clock is taken. The caller binding
    /// is structural, as with exceptions: the AgentRecord must match
    /// the signer (`AgentMismatch` otherwise). No active-flag check
    /// is added: the kernel's reject tests roles only, and this
    /// handler stays faithful to it (unlike `record_evidence` and
    /// `post_observation`, whose active tightenings are documented
    /// where they land).
    pub fn reject_transition(
        ctx: Context<RejectTransition>,
        transition_id: u64,
    ) -> Result<()> {
        let _ = transition_id;
        let caller_key = ctx.accounts.caller.key();
        let record = &ctx.accounts.agent_record;
        require!(
            record.agent == caller_key,
            TreasuryError::AgentMismatch
        );
        let transition = &mut ctx.accounts.transition_record;
        can_reject_transition(
            record.is_evaluator,
            record.is_admin,
            transition.state,
        )?;
        transition.state = STATE_REJECTED;
        emit!(TransitionRejected {
            transition_id: transition.transition_id,
            vetoer: caller_key,
        });
        Ok(())
    }

    /// Governance lifecycle (design §4.1): close a never-committed
    /// transition past its manifest expiry.
    ///
    /// Mirrors the kernel's `expire` exactly, including its two
    /// properties a stricter program would hide: the caller is
    /// permissionless (the kernel checks no role for expire — expiry
    /// is a fact about the Clock, not an authority's act), so no
    /// AgentRecord is required; and the check order is the kernel's —
    /// the expiry test first (`BadWindow` while `now` is before the
    /// manifest's `expires_at`), then the state gate (`BadState`
    /// unless Registered, Planning, or Committable). That order
    /// matters: expiring a Committed transition that has not yet
    /// expired reports the window, not the state — the caller learns
    /// the transition simply is not expirable yet, whatever its
    /// state. Expiry is terminal and not idempotent: expiring an
    /// already-Expired (or Rejected, Committed, Settled) transition
    /// past its expiry refuses with `BadState`. The kernel's
    /// `tick(now)` has no program counterpart — the Clock sysvar is
    /// monotonic on-chain, so there is no stored kernel clock to
    /// regress against — and `now` is the Clock sysvar, never a
    /// client timestamp.
    pub fn expire_transition(
        ctx: Context<ExpireTransition>,
        transition_id: u64,
    ) -> Result<()> {
        let _ = transition_id;
        let transition = &mut ctx.accounts.transition_record;
        can_expire_transition(
            transition.state,
            ctx.accounts.clock.unix_timestamp,
            transition.expires_at,
        )?;
        transition.state = STATE_EXPIRED;
        emit!(TransitionExpired {
            transition_id: transition.transition_id,
        });
        Ok(())
    }

    /// Admin setup, genesis-direct form (design §4.1): install one
    /// transition type's policy record.
    ///
    /// The caller must be the `Config` authority (the governance
    /// multisig at genesis), like `grant_role`. The record is one-time:
    /// installing a type twice fails at account creation and changes
    /// nothing. The policy values are validated by `validate_policy`
    /// against the kernel's own registration gates — a confidence
    /// threshold above 1.0 or a zero round/plan cap is `BadWindow`,
    /// exactly as the kernel refuses such a policy at registration.
    /// Quorum, minimum observation confidence, and planning window are
    /// deliberately NOT range-checked here: the kernel does not check
    /// them at registration, and an unsatisfiable value fails closed
    /// at commit (the mask bit stays set), never admits a transition
    /// that should not commit. Genesis-direct form only: after
    /// genesis, policy changes are the execution of a committed
    /// `treasury.policy.v1` transition (`apply_policy_amendment`),
    /// and this install does not authorize itself post-genesis. The
    /// `PolicyCommitted` / `PolicySettled` events of the design table
    /// are the lifecycle events of that policy transition itself
    /// (emitted by `commit` / `settle`); this install emits
    /// `PolicyInstalled` (type + authority), and the installed record
    /// itself is the payload.
    pub fn policy_transition(
        ctx: Context<PolicyTransition>,
        transition_type: [u8; 32],
        quorum_required: u64,
        confidence_threshold: u64,
        min_obs_confidence: u64,
        require_objectives: bool,
        planning_window: u64,
        max_rounds: u32,
        max_plans: u32,
        mandatory_constraints: Vec<[u8; 32]>,
        required_capabilities: Vec<CapabilityRequirement>,
    ) -> Result<()> {
        require!(
            ctx.accounts.config.authority == ctx.accounts.authority.key(),
            TreasuryError::Unauthorized
        );
        validate_policy(
            confidence_threshold,
            max_rounds,
            max_plans,
            &mandatory_constraints,
            &required_capabilities,
        )?;
        let record = &mut ctx.accounts.policy_record;
        record.transition_type = transition_type;
        record.quorum_required = quorum_required;
        record.confidence_threshold = confidence_threshold;
        record.min_obs_confidence = min_obs_confidence;
        record.require_objectives = require_objectives;
        record.planning_window = planning_window;
        record.max_rounds = max_rounds;
        record.max_plans = max_plans;
        record.mandatory_constraints = mandatory_constraints;
        record.required_capabilities = required_capabilities;
        record.bump = ctx.bumps.policy_record;
        emit!(PolicyInstalled {
            transition_type,
            authority: ctx.accounts.authority.key(),
        });
        Ok(())
    }

    /// Post-genesis policy execution (design §4.1): apply a committed
    /// `treasury.policy.v1` transition's amendment to a target type's
    /// PolicyRecord.
    ///
    /// This is the ONLY post-genesis path that changes a PolicyRecord:
    /// the genesis-direct `policy_transition` install cannot be called
    /// twice for one type (account creation fails), and no other
    /// instruction writes a PolicyRecord. The executor is
    /// permissionless — the authority is the settled transition, not
    /// the signer, exactly as `commit` and `expire_transition` are
    /// permissionless — and the executor pays no rent (no account is
    /// created here; the target record already exists).
    ///
    /// The binding is the payload hash, checked by
    /// `can_apply_policy_amendment` before anything is written:
    /// (1) the source transition is Settled, (2) it has not already
    /// been executed — a second execution refuses, because replaying
    /// an old amendment after a newer one would roll the policy
    /// back, (3) its type is the canonical `POLICY_TRANSITION_TYPE`,
    /// (4) the target record's stored type is the target type named
    /// here (the PDA seeds bind the account to that type, and the
    /// stored field is checked too), (5) the SHA-256 payload hash of
    /// the supplied new values equals the source's declared hash —
    /// the amendment is what the transition declared at registration,
    /// never values invented at execution time, and (6) the source's
    /// settle was exact (realized == declared) — an inexact settle
    /// means the executed outcome diverged from the declared one,
    /// so it authorizes nothing. Only then are the values validated
    /// by `validate_policy` (the genesis install's gates, unchanged)
    /// and written; the target record's bump is preserved, and the
    /// source transition's execution flag is set in the same
    /// instruction, so the write and the flag cannot diverge.
    /// Amending the policy type itself is allowed: the target type
    /// may be `POLICY_TRANSITION_TYPE`, governed by a policy
    /// transition like any other type.
    pub fn apply_policy_amendment(
        ctx: Context<ApplyPolicyAmendment>,
        transition_id: u64,
        target_type: [u8; 32],
        quorum_required: u64,
        confidence_threshold: u64,
        min_obs_confidence: u64,
        require_objectives: bool,
        planning_window: u64,
        max_rounds: u32,
        max_plans: u32,
        mandatory_constraints: Vec<[u8; 32]>,
        required_capabilities: Vec<CapabilityRequirement>,
    ) -> Result<()> {
        let _ = transition_id;
        let source = &ctx.accounts.transition_record;
        let source_state = source.state;
        let source_type = source.transition_type;
        let source_declared = source.declared_hash;
        let source_realized = source.realized_hash;
        let source_applied = source.policy_amendment_applied;
        let source_id = source.transition_id;
        let target_stored = ctx.accounts.policy_record.transition_type;
        let payload = policy_payload_hash(
            target_type,
            quorum_required,
            confidence_threshold,
            min_obs_confidence,
            require_objectives,
            planning_window,
            max_rounds,
            max_plans,
            &mandatory_constraints,
            &required_capabilities,
        );
        can_apply_policy_amendment(
            source_state,
            source_type,
            target_stored,
            target_type,
            payload,
            source_declared,
            source_realized,
            source_applied,
        )?;
        validate_policy(
            confidence_threshold,
            max_rounds,
            max_plans,
            &mandatory_constraints,
            &required_capabilities,
        )?;
        let record = &mut ctx.accounts.policy_record;
        record.transition_type = target_type;
        record.quorum_required = quorum_required;
        record.confidence_threshold = confidence_threshold;
        record.min_obs_confidence = min_obs_confidence;
        record.require_objectives = require_objectives;
        record.planning_window = planning_window;
        record.max_rounds = max_rounds;
        record.max_plans = max_plans;
        record.mandatory_constraints = mandatory_constraints;
        record.required_capabilities = required_capabilities;
        let transition = &mut ctx.accounts.transition_record;
        transition.policy_amendment_applied = true;
        emit!(PolicyAmended {
            transition_id: source_id,
            target_type,
            executor: ctx.accounts.executor.key(),
        });
        Ok(())
    }

    /// Treasury genesis (design §4.2): create the treasury's birth
    /// state from a Settled `treasury.init.v1` transition.
    ///
    /// This is NOT a genesis-direct multisig call, deliberately: the
    /// design row names `treasury.init.v1` as the authorizing
    /// transition, and §10's no-single-key-authority rule means the
    /// treasury's birth state must itself be a settled transition's
    /// execution. The executor is permissionless — the authority is
    /// the settled transition, not this key — and pays the rent for
    /// the three new accounts. The gate is
    /// [`can_initialize_treasury`]: Settled state, then the
    /// canonical init type, then the FeeVault binding (the created
    /// vault's key must equal `Config.fee_vault`, set at
    /// `initialize_config`). No exact-settle requirement: the
    /// init's effect is fixed, so there is no declared payload an
    /// inexact settle could diverge from — unlike
    /// `apply_policy_amendment`, where supplied values are bound to
    /// the declared hash. No execution flag on the transition: the
    /// Anchor `init` constraints make this one-time by
    /// construction, and a second attempt fails at account creation
    /// and changes no state.
    ///
    /// Initial values are all zero: total, sleeves, retained float,
    /// lending-book outstanding, fee-vault collections — and
    /// threshold B unset (0, D-2). Zero is the honest genesis state:
    /// the treasury holds nothing until window flows move value,
    /// and the launch gate cannot commit until a governed path sets
    /// B. The stage flag stays on Config (Stage 1 from
    /// `initialize_config`); this instruction does not duplicate it.
    /// The three capacity stocks are installed separately by
    /// `set_capacity` and are not touched here.
    pub fn initialize_treasury(
        ctx: Context<InitializeTreasury>,
        transition_id: u64,
    ) -> Result<()> {
        let source = &ctx.accounts.transition_record;
        can_initialize_treasury(
            source.state,
            source.transition_type,
            ctx.accounts.fee_vault.key() == ctx.accounts.config.fee_vault,
        )?;
        let state = &mut ctx.accounts.treasury_state;
        state.total_raw = 0;
        state.cash_raw = 0;
        state.bonds_raw = 0;
        state.arb_raw = 0;
        state.retained_float_raw = 0;
        state.backing_threshold_ppm = 0;
        state.bump = ctx.bumps.treasury_state;
        let book = &mut ctx.accounts.lending_book;
        book.outstanding_raw = 0;
        book.active_loans = 0;
        book.bump = ctx.bumps.lending_book;
        let vault = &mut ctx.accounts.fee_vault;
        vault.collected_raw = 0;
        vault.bump = ctx.bumps.fee_vault;
        emit!(TreasuryInitialized {
            transition_id,
            commit_index: source.commit_index,
            treasury_state: ctx.accounts.treasury_state.key(),
            lending_book: ctx.accounts.lending_book.key(),
            fee_vault: ctx.accounts.fee_vault.key(),
            executor: ctx.accounts.executor.key(),
        });
        Ok(())
    }

    /// Stage 2 launch (design §4.2): flip the stage flag on Config
    /// from Stage 1 to Stage 2, executed from a Settled
    /// `treasury.launch.v1` transition — never from a config check
    /// or a fresh authority signature.
    ///
    /// The Stage 2 gate is enforced by the launch transition, not by
    /// this handler recomputing B: the design row makes
    /// BACKING_EVIDENCED a mandatory constraint of the launch
    /// policy, so an Evaluator's report attesting B = treasury ÷
    /// (supply × price) meets the threshold in TreasuryState feeds
    /// the CONSTRAINTS bit at commit, and the manifest's observation
    /// evidences the committed fiat total. No fiat total, supply, or
    /// price exists on-chain for this handler to recompute the ratio
    /// from — the attestation IS the check, and it happens at commit
    /// time, before any settle. No other instruction checks B ad hoc.
    ///
    /// The gate [`can_launch_stage2`] enforces, in order: Settled
    /// state, the canonical launch type, the stage still being Stage
    /// 1, and threshold B set (nonzero). The threshold check is the
    /// D-2 intended state made executable: B starts unset (0) at
    /// `initialize_treasury`, and no governed path sets it yet — the
    /// only writer of the field is the init's zero — so until that
    /// path lands, a launch transition's execution refuses with
    /// `ThresholdUnset` even though its lifecycle settled. That is
    /// the design's intended state, not a gap in this instruction:
    /// an unset threshold must never read as "any backing passes".
    /// Deliberately no exact-settle requirement, as with
    /// `initialize_treasury`: the launch's effect is fixed (one flag
    /// flip), so no declared payload exists for an inexact settle to
    /// diverge from. Deliberately no one-time execution flag: the
    /// stage flip is the one-time guarantee — a second launch, or a
    /// replay of the first, refuses because the stage is no longer
    /// Stage 1 (`AlreadyLaunched`), and changes no state. The
    /// executor is permissionless and pays no rent: no account is
    /// created here, and the authority is the settled transition.
    /// The flag flip is the activation record later §4.2/§4.3 flows
    /// read; this instruction itself moves no value.
    pub fn launch_stage2(
        ctx: Context<LaunchStage2>,
        transition_id: u64,
    ) -> Result<()> {
        let source = &ctx.accounts.transition_record;
        can_launch_stage2(
            source.state,
            source.transition_type,
            ctx.accounts.config.stage,
            ctx.accounts.treasury_state.backing_threshold_ppm,
        )?;
        ctx.accounts.config.stage = STAGE_2;
        emit!(Stage2Launched {
            transition_id,
            commit_index: source.commit_index,
            executor: ctx.accounts.executor.key(),
        });
        Ok(())
    }
}

/// Who may open planning, as a pure check (design §4.1, kernel
/// `open_planning`): the recorded proposer, or any Admin. Any other
/// caller is unauthorized; a transition not in Registered state cannot
/// enter Planning (the kernel's `set_state` gate, mirrored here).
pub fn can_open_planning(
    caller: Pubkey,
    proposer: Pubkey,
    is_admin: bool,
    state: u8,
) -> Result<()> {
    require!(state == STATE_REGISTERED, TreasuryError::BadState);
    require!(
        caller == proposer || is_admin,
        TreasuryError::Unauthorized
    );
    Ok(())
}

/// The planning deadline for a planning window opened at `now`:
/// `now + planning_window`, checked — an overflow is an error, never a
/// wrapped deadline.
pub fn planning_deadline(now: i64, planning_window: u64) -> Result<i64> {
    now.checked_add(planning_window as i64)
        .ok_or(TreasuryError::Overflow.into())
}

/// Who may submit a plan, as a pure check (design §4.1, kernel
/// `submit_plan`, in the kernel's check order minus the caller
/// binding): the transition must be Planning and `now` must precede
/// the deadline (both failures are `PlanningClosed` in the kernel);
/// the agent must be active (the active half of the kernel's
/// eligibility — the group-overlap half awaits `relevant_groups` on
/// the record, see the handler docs); room must remain under
/// `max_plans`; the plan's expiry must be in the future. The
/// plan-agent == caller rule is NOT re-checked here: it is structural
/// — the `PlanRecord` PDA seeds bind the plan to the signer, and the
/// handler requires the AgentRecord to match the signer
/// (`AgentMismatch` otherwise) — so a seven-argument helper stays
/// within clippy's argument-count lint without duplicating a check
/// the account constraints already enforce.
pub fn can_submit_plan(
    agent_active: bool,
    state: u8,
    now: i64,
    planning_deadline: i64,
    plan_count: u32,
    max_plans: u32,
    plan_expires_at: i64,
) -> Result<()> {
    require!(
        state == STATE_PLANNING && now < planning_deadline,
        TreasuryError::PlanningClosed
    );
    require!(agent_active, TreasuryError::NotEligible);
    require!(plan_count < max_plans, TreasuryError::TooManyPlans);
    require!(plan_expires_at > now, TreasuryError::BadWindow);
    Ok(())
}

/// Support ceiling, kernel `Amount::ONE` in raw units (SCALE 1_000_000):
/// a signal's support is a fraction of the round's weight, so anything
/// above 1.0 is out of range by construction.
pub const SUPPORT_ONE_RAW: u64 = 1_000_000;

/// Who may submit a signal, as a pure check (design §4.1, kernel
/// `submit_signal`, in the kernel's check order minus the caller
/// binding and the duplicate rule, both structural — PDA seeds plus the
/// AgentRecord match, and one SignalRecord per (transition, round,
/// agent)): support in range first, then Registered-or-Planning state,
/// then the round index under `max_rounds` and equal to the
/// transition's current round (a stale round is a bad state, not a
/// signal for a round that is not live), then the agent active (the
/// active half of kernel eligibility — the group-overlap half awaits
/// `relevant_groups`, as with plans).
pub fn can_submit_signal(
    agent_active: bool,
    state: u8,
    support: u64,
    round: u32,
    max_rounds: u32,
    current_round: u32,
) -> Result<()> {
    require!(
        support <= SUPPORT_ONE_RAW,
        TreasuryError::SupportOutOfRange
    );
    require!(
        state == STATE_REGISTERED || state == STATE_PLANNING,
        TreasuryError::BadState
    );
    require!(round < max_rounds, TreasuryError::RoundsExhausted);
    require!(round == current_round, TreasuryError::BadState);
    require!(agent_active, TreasuryError::NotEligible);
    Ok(())
}

/// The kernel's monotone-revision rule for a re-signal (kernel
/// `submit_signal`): let `mean` be the previous round's weighted mean
/// support and `prev_support` the agent's previous-round signal. The
/// new support is admissible only if it is no farther from the mean
/// than the previous signal was: `|support - mean| <=
/// |prev_support - mean|`. A revision that moves away from the mean
/// (revision happens between rounds, never by jumping farther away)
/// is refused. Pure and tested now; the on-chain wiring lands with
/// `advance_round`, which snapshots the previous round's mean — until
/// then only round 0 is reachable and this branch is vacuous.
pub fn check_monotone_revision(
    prev_support: u64,
    prev_mean: u64,
    support: u64,
) -> Result<()> {
    require!(
        support.abs_diff(prev_mean) <= prev_support.abs_diff(prev_mean),
        TreasuryError::MonotoneRevision
    );
    Ok(())
}

/// Constraint status codes, in the kernel's `ConstraintStatus` enum
/// order (kernel.rs): Pass, Fail, Unavailable. A fresh frozen entry is
/// Unavailable; only an all-Pass, non-empty set clears the CONSTRAINTS
/// bit at commit.
pub const STATUS_PASS: u8 = 0;
pub const STATUS_FAIL: u8 = 1;
pub const STATUS_UNAVAILABLE: u8 = 2;

/// Who may submit a constraint report, as a pure check (design §4.1,
/// kernel `report_constraint`, in the kernel's check order minus the
/// caller binding, which is structural — PDA seeds plus the
/// AgentRecord match): the caller must hold the Evaluator role, the
/// transition must be Registered or Planning, the constraint must be a
/// member of the frozen set, and the status code must be a real status.
/// An unknown constraint is refused here, not at commit: the frozen set
/// is fixed at registration, so a report outside it has no entry to
/// write and must not be recorded anywhere.
pub fn can_submit_constraint_report(
    is_evaluator: bool,
    state: u8,
    constraint_known: bool,
    status: u8,
) -> Result<()> {
    require!(is_evaluator, TreasuryError::Unauthorized);
    require!(
        state == STATE_REGISTERED || state == STATE_PLANNING,
        TreasuryError::BadState
    );
    require!(
        constraint_known,
        TreasuryError::UnknownConstraint
    );
    require!(
        status <= STATUS_UNAVAILABLE,
        TreasuryError::InvalidConstraintStatus
    );
    Ok(())
}

/// Whether a disposition counts toward `unresolved_blocking` (kernel
/// `raise_exception`: `d >= Disposition::Block`). Informational (0)
/// and Review (1) never block; Block (2) and Emergency (3) do. The
/// caller must already have validated the code range — an out-of-range
/// code is not a disposition at all, and this function must not be
/// asked to classify it (the raise gate rejects it first).
pub fn is_blocking_disposition(disposition: u8) -> bool {
    disposition >= DISPOSITION_BLOCK
}

/// Who may raise an exception, as a pure check (design §4.1, kernel
/// `raise_exception`, in the kernel's check order minus the caller
/// binding, which is structural — PDA seeds plus the AgentRecord
/// match): the caller must hold the Evaluator role, the transition
/// must be Registered, Planning, or Committable (the kernel's
/// pre-commit states — a Committed transition cannot gain an
/// exception, since the mask was already judged), the disposition
/// code must be a kernel `Disposition` (0..=3), the id must not
/// already be raised on this transition, and room must remain under
/// `MAX_EXCEPTIONS` (the program-layer account bound; the kernel's
/// map is unbounded, but an on-chain record is not). The duplicate
/// check precedes the capacity check: an id already raised is refused
/// as a duplicate whatever the count, never as a capacity failure.
pub fn can_raise_exception(
    is_evaluator: bool,
    state: u8,
    disposition: u8,
    already_raised: bool,
    exception_count: usize,
) -> Result<()> {
    require!(is_evaluator, TreasuryError::Unauthorized);
    require!(
        state == STATE_REGISTERED || state == STATE_PLANNING || state == STATE_COMMITTABLE,
        TreasuryError::BadState
    );
    require!(
        disposition <= DISPOSITION_EMERGENCY,
        TreasuryError::InvalidDisposition
    );
    require!(!already_raised, TreasuryError::DuplicateException);
    require!(
        exception_count < MAX_EXCEPTIONS,
        TreasuryError::TooManyExceptions
    );
    Ok(())
}

/// Who may resolve an exception, as a pure check (design §4.1, kernel
/// `resolve_exception`): the caller must hold the Evaluator role,
/// and the id must have been raised on this transition. There is
/// deliberately NO state gate here — the kernel imposes none, and
/// this helper mirrors it: resolving after commit cannot rewrite a
/// decided mask, and resolving a rejected or expired transition is
/// bookkeeping only. Whether the entry is already resolved is not a
/// gate either — re-resolution is a no-op success in the kernel, and
/// the handler keeps it one by only decrementing on the first
/// resolution.
pub fn can_resolve_exception(is_evaluator: bool, exception_known: bool) -> Result<()> {
    require!(is_evaluator, TreasuryError::Unauthorized);
    require!(exception_known, TreasuryError::UnknownException);
    Ok(())
}

/// Who may veto a transition, as a pure check (design §4.1, kernel
/// `reject`, in the kernel's check order minus the caller binding,
/// which is structural — PDA seeds plus the AgentRecord match): the
/// caller must hold the Evaluator role or the Admin role, and the
/// transition must be Registered, Planning, or Committable (the
/// kernel's pre-commit states — a Committed transition cannot be
/// vetoed back, and a terminal one cannot be vetoed twice). The role
/// test precedes the state test, as in the kernel: a non-vetoer
/// learns nothing about the state from the error.
pub fn can_reject_transition(is_evaluator: bool, is_admin: bool, state: u8) -> Result<()> {
    require!(
        is_evaluator || is_admin,
        TreasuryError::Unauthorized
    );
    require!(
        state == STATE_REGISTERED || state == STATE_PLANNING || state == STATE_COMMITTABLE,
        TreasuryError::BadState
    );
    Ok(())
}

/// When a transition may expire, as a pure check (design §4.1,
/// kernel `expire`, in the kernel's check order): the manifest must
/// already be expired (`now` at or past `expires_at` — `BadWindow`
/// before it), and the transition must be Registered, Planning, or
/// Committable (`BadState` otherwise — a Committed transition never
/// expires, and a terminal one never expires twice). There is
/// deliberately NO caller argument here — the kernel checks no role
/// for expire, so this helper takes none, and a caller cannot smuggle
/// a role gate in later.
pub fn can_expire_transition(state: u8, now: i64, expires_at: i64) -> Result<()> {
    require!(now >= expires_at, TreasuryError::BadWindow);
    require!(
        state == STATE_REGISTERED || state == STATE_PLANNING || state == STATE_COMMITTABLE,
        TreasuryError::BadState
    );
    Ok(())
}

/// Who may submit an objectives check, as a pure check (design §4.1,
/// kernel `report_objectives`, in the kernel's check order minus the
/// caller binding, which is structural — PDA seeds plus the
/// AgentRecord match): the caller must hold the Evaluator role, and
/// the transition must be Registered or Planning. The verdict itself
/// is a bool and needs no validation — any bool is a real verdict, and
/// a later report overwrites it, so the OBJECTIVES bit judges the
/// final flag at commit.
pub fn can_submit_objectives_check(is_evaluator: bool, state: u8) -> Result<()> {
    require!(is_evaluator, TreasuryError::Unauthorized);
    require!(
        state == STATE_REGISTERED || state == STATE_PLANNING,
        TreasuryError::BadState
    );
    Ok(())
}

/// Who may advance a round, as a pure check (design §4.1, kernel
/// `advance_round`, in the kernel's check order): the transition must
/// be Registered or Planning, the current round must be non-empty
/// (its effective-weight total non-zero — the kernel's
/// `total == Amount::ZERO` test, not a signal count), and room must
/// remain under `max_rounds`. The empty check precedes the exhaustion
/// check, as in the kernel: an empty final round reports RoundEmpty,
/// not RoundsExhausted. The caller is permissionless (the kernel
/// ignores its caller), so no caller argument appears here.
pub fn can_advance_round(
    state: u8,
    current_round_total: u64,
    round_count: u32,
    max_rounds: u32,
) -> Result<()> {
    require!(
        state == STATE_REGISTERED || state == STATE_PLANNING,
        TreasuryError::BadState
    );
    require!(current_round_total != 0, TreasuryError::RoundEmpty);
    require!(round_count < max_rounds, TreasuryError::RoundsExhausted);
    Ok(())
}

/// Who may reconcile a transition, as a pure check (design §4.1,
/// kernel `reconcile`, in the kernel's check order): the transition
/// must be Planning, and `now` must be at or past the planning
/// deadline. The caller is permissionless (the kernel ignores its
/// reconcile caller), so no caller argument appears here. The fit
/// itself is not a gate here: a false fit still advances the state
/// (see [`fits_capacity`]), and the COMPAT bit refuses the commit.
pub fn can_reconcile(state: u8, now: i64, planning_deadline: i64) -> Result<()> {
    require!(state == STATE_PLANNING, TreasuryError::BadState);
    require!(
        now >= planning_deadline,
        TreasuryError::PlanningStillOpen
    );
    Ok(())
}

/// The kernel's capacity-fit rule (kernel `fits_capacity`): aggregate
/// every plan's claims per resource (checked addition — an overflow
/// fails the fit, never wraps), then require every aggregate to name
/// an existing stock holding at least the aggregate. A claimed
/// resource with no stock fails, exactly as the kernel's
/// `is_some_and` test fails it; an empty claim set fits vacuously.
/// Claims and stocks are `(resource_id, amount)` pairs; duplicate
/// resource ids in either list are summed (claims) or take the larger
/// (stocks) — callers pass distinct keys, and the aggregation is
/// order-independent either way.
pub fn fits_capacity(claims: &[(u16, u64)], stocks: &[(u16, u64)]) -> bool {
    let mut agg: Vec<(u16, u64)> = Vec::new();
    for (resource, amount) in claims {
        let mut found = false;
        for entry in agg.iter_mut() {
            if entry.0 == *resource {
                match entry.1.checked_add(*amount) {
                    Some(sum) => entry.1 = sum,
                    None => return false,
                }
                found = true;
                break;
            }
        }
        if !found {
            agg.push((*resource, *amount));
        }
    }
    agg.iter().all(|(resource, total)| {
        stocks
            .iter()
            .filter(|(id, _)| id == resource)
            .map(|(_, cap)| *cap)
            .max()
            .is_some_and(|cap| *total <= cap)
    })
}

/// Load a CapacityStock segment of remaining accounts (design §4.1):
/// every account must be program-owned, deserialize as a
/// CapacityStock, be listed once, and sit at the PDA of its stored
/// resource id (`["stock", resource_id]`) — anything else is
/// `StockMismatch`. The PDA check is the stock analogue of the plan
/// segment's transition-naming check: a stock account names its
/// resource by its key, and the key is program-derived, so a caller
/// cannot present a stranger's balance as a stock. Two accounts with
/// the same resource id would share one PDA key, so the distinctness
/// check also guarantees distinct resource ids in the returned list.
pub fn load_stock_balances(
    accounts: &[AccountInfo],
) -> Result<Vec<(u16, u64)>> {
    let mut stocks: Vec<(u16, u64)> = Vec::new();
    let mut seen: Vec<Pubkey> = Vec::new();
    for account in accounts.iter() {
        require!(
            account.owner == &crate::ID,
            TreasuryError::StockMismatch
        );
        require!(
            !seen.contains(account.key),
            TreasuryError::StockMismatch
        );
        seen.push(*account.key);
        let data = account.data.borrow();
        let mut slice: &[u8] = &data;
        let stock = CapacityStock::try_deserialize(&mut slice)
            .map_err(|_| TreasuryError::StockMismatch)?;
        let (expected, _) = Pubkey::find_program_address(
            &[b"stock", &stock.resource_id.to_le_bytes()],
            &crate::ID,
        );
        require!(
            expected == *account.key,
            TreasuryError::StockMismatch
        );
        stocks.push((stock.resource_id, stock.balance));
    }
    Ok(stocks)
}

/// Load a reconcile plan segment plus its stock segment (design
/// §4.1): the first `plan_count` accounts are plans — program-owned,
/// naming this transition, distinct pubkeys, else `PlanMismatch` —
/// and the rest are stocks, via `load_stock_balances`. Returns the
/// plans' claims (in plan order, duplicates kept for aggregation by
/// `fits_capacity`) and the stock balances. `commit` parses its
/// longer plans + authors + stocks layout inline, because the author
/// segment sits between the plans and the stocks; the plan and stock
/// rules it applies are these same two.
pub fn load_plan_claims_and_stocks(
    accounts: &[AccountInfo],
    transition_id: u64,
    plan_count: usize,
) -> Result<(Vec<(u16, u64)>, Vec<(u16, u64)>)> {
    require!(
        accounts.len() >= plan_count,
        TreasuryError::PlanMismatch
    );
    let mut claims: Vec<(u16, u64)> = Vec::new();
    let mut seen: Vec<Pubkey> = Vec::new();
    for account in accounts.iter().take(plan_count) {
        require!(
            account.owner == &crate::ID,
            TreasuryError::PlanMismatch
        );
        require!(
            !seen.contains(account.key),
            TreasuryError::PlanMismatch
        );
        seen.push(*account.key);
        let data = account.data.borrow();
        let mut slice: &[u8] = &data;
        let plan = PlanRecord::try_deserialize(&mut slice)
            .map_err(|_| TreasuryError::PlanMismatch)?;
        require!(
            plan.transition_id == transition_id,
            TreasuryError::PlanMismatch
        );
        for claim in plan.claims.iter() {
            claims.push((claim.resource_id, claim.amount));
        }
    }
    let stocks = load_stock_balances(&accounts[plan_count..])?;
    Ok((claims, stocks))
}

/// The kernel's commit debit, as a pure function (design §4.1,
/// kernel `commit` debits capacity): aggregate the claims per
/// resource (checked addition — an overflow fails, never wraps),
/// then subtract each aggregate from its stock. Returns None — and
/// changes nothing, since the input is borrowed — when a claimed
/// resource has no stock, when any aggregate exceeds its stock, or
/// when an aggregate overflows. Callers pass distinct resource ids
/// (the PDA check in `load_stock_balances` enforces it), so the
/// first-match lookup below is unambiguous.
pub fn apply_debit(
    stocks: &[(u16, u64)],
    claims: &[(u16, u64)],
) -> Option<Vec<(u16, u64)>> {
    let mut agg: Vec<(u16, u64)> = Vec::new();
    for (resource, amount) in claims {
        let mut found = false;
        for entry in agg.iter_mut() {
            if entry.0 == *resource {
                entry.1 = entry.1.checked_add(*amount)?;
                found = true;
                break;
            }
        }
        if !found {
            agg.push((*resource, *amount));
        }
    }
    let mut next = stocks.to_vec();
    for (resource, total) in agg {
        let entry = next.iter_mut().find(|(id, _)| *id == resource)?;
        if entry.1 < total {
            return None;
        }
        entry.1 -= total;
    }
    Some(next)
}

/// The commit gate, as a pure check (design §4.1, kernel `commit`):
/// only a Committable transition may commit — the kernel's `set_state`
/// allows Committable → Committed and nothing else into Committed.
/// The mask itself is judged separately, by `commit_fail_mask` below;
/// a Committable transition with a non-zero mask still refuses, with
/// `CommitFailed`, and changes nothing.
pub fn can_commit(state: u8) -> Result<()> {
    require!(state == STATE_COMMITTABLE, TreasuryError::BadState);
    Ok(())
}

/// Who may record evidence, as a pure check (design §4.1, the kernel's
/// `settle` attestor gate applied at the program layer, one step early):
/// the caller must be an active Oracle — the kernel's RoleAttestor
/// allows Oracle to observe/settle, and the active flag is the same
/// fail-closed tightening `post_observation` applies — and the
/// transition must be Committed, the kernel's settle gate applied here
/// so evidence cannot be recorded for a transition that has not
/// committed, nor re-recorded after it settled. The realized hash
/// itself is a value, not a claim the program can verify: any 32 bytes
/// record, and a hash that does not match the realized outcome simply
/// fails settle later. The caller binding is structural (AgentRecord
/// match in the handler), so no caller argument appears here. Check
/// order, as in the kernel's `settle`: the attestor gate first, then
/// the state gate.
pub fn can_record_evidence(agent_active: bool, is_oracle: bool, state: u8) -> Result<()> {
    require!(agent_active && is_oracle, TreasuryError::Unauthorized);
    require!(state == STATE_COMMITTED, TreasuryError::BadState);
    Ok(())
}

/// Who may settle a transition, as a pure check (design §4.1, the
/// kernel's `settle` in check order): the attestor gate first — the
/// caller must be an active Oracle (the kernel's RoleAttestor rule,
/// plus the active flag, the same fail-closed tightening
/// `post_observation` applies) AND the presented realized hash must
/// equal the independently recorded EvidenceRecord (the program-layer
/// EvidenceAttestor — a role + evidence check, not a capability
/// check) — then the state gate: only a Committed transition settles.
/// A missing EvidenceRecord never reaches this helper (account
/// validation fails first); a present-but-mismatched record arrives
/// here as `evidence_matches = false`. The caller binding is
/// structural (AgentRecord match in the handler), so no caller
/// argument appears here. Check order, as in the kernel's `settle`:
/// the attestor gate before the state gate, and the role half of the
/// attestor gate before the evidence half (a non-Oracle cannot learn
/// from the error whether the hash matched).
pub fn can_settle(
    agent_active: bool,
    is_oracle: bool,
    evidence_matches: bool,
    state: u8,
) -> Result<()> {
    require!(agent_active && is_oracle, TreasuryError::Unauthorized);
    require!(evidence_matches, TreasuryError::Unauthorized);
    require!(state == STATE_COMMITTED, TreasuryError::BadState);
    Ok(())
}

/// Per-settlement attribution cap in kernel fixed-point raw units: 20
/// units at SCALE 1_000_000 (kernel launch config `max_attribution`,
/// kernel_bridge). The kernel clamps each agent's summed delta to
/// ±this before applying it.
pub const MAX_ATTRIBUTION_RAW: i128 = 20_000_000;

/// Sum settle attributions per agent (kernel `settle`'s net map):
/// deltas for the same agent add (order-independent — the sum is the
/// same whatever order the caller lists them in), in i128 so no sum of
/// i64 deltas can overflow. Agents appear in first-appearance order;
/// the handler resolves each unique agent to exactly one AgentRecord.
pub fn sum_attributions(attributions: &[Attribution]) -> Vec<(Pubkey, i128)> {
    let mut summed: Vec<(Pubkey, i128)> = Vec::new();
    for attribution in attributions {
        let mut found = false;
        for entry in summed.iter_mut() {
            if entry.0 == attribution.agent {
                entry.1 += attribution.delta as i128;
                found = true;
                break;
            }
        }
        if !found {
            summed.push((attribution.agent, attribution.delta as i128));
        }
    }
    summed
}

/// Apply one agent's summed settle delta to its reputation (kernel
/// `settle`'s attribution rule, exactly): clamp the sum to
/// ±[`MAX_ATTRIBUTION_RAW`], halve positive credit on an inexact
/// settle (realized ≠ declared — negative deltas are never halved,
/// the kernel halves only positive credit), then add to the current
/// reputation with saturation and clamp to the 0–100 unit band
/// (kernel launch config `rep_min`/`rep_max`, raw 0/100_000_000 —
/// the same band `register_agent` enforces). Under FlatWeights the
/// result is recorded influence, not quorum power: reputation moves
/// nothing until a weight policy says otherwise.
pub fn settle_attribution(reputation_raw: u64, net: i128, exact: bool) -> u64 {
    let mut n = net.clamp(-MAX_ATTRIBUTION_RAW, MAX_ATTRIBUTION_RAW);
    if !exact && n > 0 {
        n /= 2;
    }
    let value = (reputation_raw as i128).saturating_add(n);
    value.clamp(0, REP_MAX_RAW as i128) as u64
}

/// The 13 `commit_fail_mask` bit values (kernel `kernel.rs:20-34`,
/// design §1.1). Local constants, not re-exported from the kernel
/// crate, so the program's mask computation reads standalone — and a
/// test below pins them equal to the kernel's `FailMask` values, so
/// any drift between the two fails loudly instead of silently.
pub const MASK_CONSTRAINTS: u32 = 1;
pub const MASK_OBJECTIVES: u32 = 2;
pub const MASK_QUORUM: u32 = 4;
pub const MASK_CONFIDENCE: u32 = 8;
pub const MASK_COMPAT: u32 = 16;
pub const MASK_EXCEPTIONS: u32 = 32;
pub const MASK_FRESH: u32 = 64;
pub const MASK_PLANNING_OPEN: u32 = 128;
pub const MASK_EXPIRED: u32 = 256;
pub const MASK_LINEAGE: u32 = 512;
pub const MASK_PLANS: u32 = 1024;
pub const MASK_CAPABILITY: u32 = 2048;
pub const MASK_DIALECT: u32 = 4096;

/// Every input the 13-bit predicate reads, as values — no accounts,
/// no Clock, no caller. The `commit` handler assembles this struct
/// from on-chain records; the helper judges it. Keeping the predicate
/// pure is what makes the per-bit test matrix possible, and what keeps
/// the on-chain predicate exactly the kernel's predicate: the kernel's
/// `commit_fail_mask` reads the same fields in the same order, and the
/// bit comments below name the kernel term each line mirrors.
pub struct CommitInputs<'a> {
    /// Final statuses of the frozen constraint set (kernel
    /// `constraints` values; empty set is infeasible).
    pub constraint_statuses: &'a [u8],
    /// The independent objectives verdict (kernel `objectives_ok`).
    pub objectives_ok: bool,
    /// Whether the transition type's policy requires objectives.
    pub require_objectives: bool,
    /// Last round's effective-weight total (kernel round `total`).
    pub round_total: u64,
    /// Last round's weighted-support sum (kernel round `weighted`).
    pub round_weighted: u64,
    /// The transition type's policy quorum (PolicyRecord
    /// `quorum_required`, kernel `quorum_required`, raw).
    pub quorum_required: u64,
    /// The transition type's policy confidence threshold (PolicyRecord,
    /// raw).
    pub confidence_threshold: u64,
    /// The `reconcile` verdict (kernel `reconciled`).
    pub reconciled: bool,
    /// The re-checked capacity fit (kernel `fits_capacity`, judged
    /// by the caller over the stock set that exists today).
    pub fits: bool,
    /// Unresolved blocking exceptions on this transition (kernel
    /// `unresolved_blocking`; the TransitionRecord's live count,
    /// maintained by `raise_exception` / `resolve_exception`).
    pub unresolved_blocking: u32,
    /// Observation timestamp (kernel observation `timestamp`).
    pub obs_timestamp: i64,
    /// Observation expiry (kernel observation `expiry`).
    pub obs_expiry: i64,
    /// Observation confidence (raw).
    pub obs_confidence: u64,
    /// The transition type's policy minimum observation confidence
    /// (PolicyRecord, raw).
    pub min_obs_confidence: u64,
    /// On-chain time, from the Clock sysvar (kernel `now`).
    pub now: i64,
    /// Planning deadline set by `open_planning`.
    pub planning_deadline: i64,
    /// Manifest expiry.
    pub expires_at: i64,
    /// Manifest parent index (frozen at registration).
    pub parent_index: u64,
    /// Manifest parent hash (frozen at registration).
    pub parent_hash: [u8; 32],
    /// Declared chain head index at commit time.
    pub chain_index: u64,
    /// Declared chain head hash at commit time.
    pub chain_hash: [u8; 32],
    /// Plans submitted (kernel `plans.len()`).
    pub plan_count: u32,
    /// Earliest plan expiry, if any plan exists (kernel
    /// `min(expires_at)`; `None` when no plan exists).
    pub min_plan_expiry: Option<i64>,
    /// Whether every capability required of the transition type is
    /// covered by plan authors' influence (kernel
    /// `capability_covered`, computed by the helper below over the
    /// commit's author records — never assumed).
    pub capability_covered: bool,
}

/// The kernel's CAPABILITY coverage test (`capability_covered` in
/// `kernel.rs`), over program records: every required capability must
/// be covered by the summed influence of the plan authors holding it.
/// Each author counts once (the caller passes one record per distinct
/// author — the kernel iterates `plans.keys()`, which are distinct by
/// construction); influence held by a non-author counts for nothing,
/// exactly as the kernel ignores it. Checked addition throughout: an
/// overflowing sum fails the requirement (the kernel's `Err → false`),
/// never wraps into coverage. No requirements means covered, as in
/// the kernel (no `required_caps` entry for the type).
pub fn capability_covered(
    required: &[CapabilityRequirement],
    authors: &[(u64, Vec<[u8; 32]>)],
) -> bool {
    required.iter().all(|req| {
        let mut acc: u64 = 0;
        for (influence, capabilities) in authors {
            if capabilities.contains(&req.id) {
                match acc.checked_add(*influence) {
                    Some(sum) => acc = sum,
                    None => return false,
                }
            }
        }
        acc >= req.min_weight
    })
}

/// The kernel's one predicate, computed on-chain (kernel
/// `commit_fail_mask`, design §1.1 bit by bit). Mask 0 ⟺ commit.
/// DIALECT is computed and always clear, exactly as the kernel's
/// `dialect_deps_ok` stub always passes — present in the mask, never
/// set at launch, never smuggled into a version check. CONFIDENCE with
/// a zero round total is 0 (the kernel's `confidence_of` returns zero
/// for an empty round), so an empty round fails CONFIDENCE and QUORUM
/// together, never divides by zero. The caller supplies `fits`
/// pre-computed by `fits_capacity` over the stocks that exist today;
/// the COMPAT bit itself is `reconciled && fits`, re-checked here, so
/// a stale `reconciled` value can never commit on its own.
pub fn commit_fail_mask(inputs: &CommitInputs) -> u32 {
    let mut mask = 0u32;
    // CONSTRAINTS: empty set is infeasible; Unavailable is not Pass.
    if inputs.constraint_statuses.is_empty()
        || inputs
            .constraint_statuses
            .iter()
            .any(|status| *status != STATUS_PASS)
    {
        mask |= MASK_CONSTRAINTS;
    }
    // OBJECTIVES: enforced only when the policy requires objectives.
    if inputs.require_objectives && !inputs.objectives_ok {
        mask |= MASK_OBJECTIVES;
    }
    // QUORUM: last round's total against the required quorum.
    if inputs.round_total < inputs.quorum_required {
        mask |= MASK_QUORUM;
    }
    // CONFIDENCE: last round's weighted mean against the threshold;
    // a zero total has no mean, so its confidence is zero.
    let confidence = if inputs.round_total == 0 {
        0u64
    } else {
        let mean = (inputs.round_weighted as u128) * (SUPPORT_ONE_RAW as u128)
            / (inputs.round_total as u128);
        u64::try_from(mean).unwrap_or(u64::MAX)
    };
    if confidence < inputs.confidence_threshold {
        mask |= MASK_CONFIDENCE;
    }
    // COMPAT: reconciled AND the fit, re-checked — never either alone.
    if !(inputs.reconciled && inputs.fits) {
        mask |= MASK_COMPAT;
    }
    // EXCEPTIONS: per-transition, blocking dispositions only; the
    // caller passes the transition's live unresolved-blocking count
    // — the bit is computed, not skipped.
    if inputs.unresolved_blocking != 0 {
        mask |= MASK_EXCEPTIONS;
    }
    // FRESH: observation timestamp ≤ now < expiry, confidence ≥ min.
    if inputs.now < inputs.obs_timestamp
        || inputs.now >= inputs.obs_expiry
        || inputs.obs_confidence < inputs.min_obs_confidence
    {
        mask |= MASK_FRESH;
    }
    // PLANNING_OPEN: planning must have closed (deadline reached).
    if inputs.now < inputs.planning_deadline {
        mask |= MASK_PLANNING_OPEN;
    }
    // EXPIRED: the manifest must still be live.
    if inputs.now >= inputs.expires_at {
        mask |= MASK_EXPIRED;
    }
    // LINEAGE: parent must equal the declared chain head right now —
    // judged at commit time, so a race loser re-registers (K-1).
    if inputs.parent_index != inputs.chain_index
        || inputs.parent_hash != inputs.chain_hash
    {
        mask |= MASK_LINEAGE;
    }
    // PLANS: at least one plan, every plan unexpired. No plans, or no
    // expiry to judge (a count/expiry mismatch the handler must never
    // produce), fails — a planless transition carries no claims and
    // can never commit.
    if inputs.plan_count == 0
        || inputs
            .min_plan_expiry
            .is_none_or(|expiry| inputs.now >= expiry)
    {
        mask |= MASK_PLANS;
    }
    // CAPABILITY: covered by plan authors' influence, or no
    // requirement exists (the kernel's `capability_covered` returns
    // true when nothing is required of the type).
    if !inputs.capability_covered {
        mask |= MASK_CAPABILITY;
    }
    // DIALECT: the kernel's Phase-7 stub always passes. Computed here
    // as always-clear, deliberately — no version check is smuggled
    // into a bit the kernel leaves open.
    mask
}

/// A round's weighted mean support, as the kernel computes it at
/// `submit_signal` time for the previous round: `weighted * ONE /
/// total`, in u128, floored — the kernel's
/// `checked_mul_div(Amount::ONE, total)`. A zero total has no mean
/// and is an overflow error; `advance_round` gates on non-zero total
/// before calling this, so the error is unreachable through the
/// handler and exists only to keep the helper total.
pub fn round_mean(weighted: u64, total: u64) -> Result<u64> {
    if total == 0 {
        return Err(TreasuryError::Overflow.into());
    }
    let mean = (weighted as u128) * (SUPPORT_ONE_RAW as u128) / (total as u128);
    u64::try_from(mean).map_err(|_| TreasuryError::Overflow.into())
}

/// One signal's contribution to its round's aggregates, in kernel
/// `Amount` arithmetic (SCALE 1_000_000, u128 intermediates, floored).
/// Under FlatWeights, relevance and group policy are 1.0, so the
/// effective weight is raw x influence / SCALE, and the weighted term
/// is effective weight x support / SCALE — exactly the kernel's
/// `submit_signal` computation (`eff` then `term`). An intermediate
/// that does not fit u64 is an overflow, never a wrap, matching the
/// kernel's checked `Amount` operations.
pub fn signal_aggregates(raw: u64, support: u64, influence_raw: u64) -> Result<(u64, u64)> {
    let eff = (raw as u128) * (influence_raw as u128) / (SUPPORT_ONE_RAW as u128);
    let term = eff * (support as u128) / (SUPPORT_ONE_RAW as u128);
    let eff = u64::try_from(eff).map_err(|_| TreasuryError::Overflow)?;
    let term = u64::try_from(term).map_err(|_| TreasuryError::Overflow)?;
    Ok((eff, term))
}

/// Write one report into a frozen set: the entry must exist (an unknown
/// id is an error, never an append — the set is frozen), and the status
/// must be a real status code. Re-reporting overwrites: the CONSTRAINTS
/// bit judges the final set, so a corrected report replaces, not
/// appends.
pub fn set_constraint_status(
    constraints: &mut [ConstraintEntry],
    constraint_id: [u8; 32],
    status: u8,
) -> Result<()> {
    require!(
        status <= STATUS_UNAVAILABLE,
        TreasuryError::InvalidConstraintStatus
    );
    let entry = constraints
        .iter_mut()
        .find(|e| e.id == constraint_id)
        .ok_or(TreasuryError::UnknownConstraint)?;
    entry.status = status;
    Ok(())
}

/// A frozen constraint set must fit the account and the kernel's set
/// shape: at most [`MAX_CONSTRAINTS`] entries, no id twice (a duplicate
/// id would be one constraint with two statuses at commit replay).
pub fn validate_constraint_ids(ids: &[[u8; 32]]) -> Result<()> {
    require!(ids.len() <= MAX_CONSTRAINTS, TreasuryError::InvalidConstraints);
    for (i, id) in ids.iter().enumerate() {
        for other in &ids[..i] {
            require!(id != other, TreasuryError::InvalidConstraints);
        }
    }
    Ok(())
}

/// The kernel's frozen-set derivation (design §4.1, kernel
/// `register_transition`): the type's mandatory constraints ∪ the
/// proposer's additions, in that order, deduped — a mandatory id the
/// proposer also supplies is one entry, never two, and the mandatory
/// order is preserved so the set is deterministic from its inputs.
/// The caller still validates the merged set's shape (at most
/// [`MAX_CONSTRAINTS`]); merging alone cannot shrink a too-large union.
pub fn merge_constraint_sets(
    mandatory: &[[u8; 32]],
    additions: &[[u8; 32]],
) -> Vec<[u8; 32]> {
    let mut merged: Vec<[u8; 32]> = mandatory.to_vec();
    for id in additions {
        if !merged.contains(id) {
            merged.push(*id);
        }
    }
    merged
}

/// A transition type's policy, validated as the kernel validates it at
/// registration (kernel `register_transition`): a confidence
/// threshold above 1.0, or a zero round or plan cap, is `BadWindow`.
/// Quorum, minimum observation confidence, and planning window are
/// deliberately NOT range-checked — the kernel does not check them at
/// registration, and an unsatisfiable value fails closed at commit
/// (its mask bit stays set), never admits a transition that should not
/// commit. The mandatory set must fit the frozen-set shape, and the
/// required capabilities must fit their bound with no id twice (a
/// duplicate requirement would double-count one capability's cover).
pub fn validate_policy(
    confidence_threshold: u64,
    max_rounds: u32,
    max_plans: u32,
    mandatory: &[[u8; 32]],
    required_capabilities: &[CapabilityRequirement],
) -> Result<()> {
    require!(
        confidence_threshold <= SUPPORT_ONE_RAW,
        TreasuryError::BadWindow
    );
    require!(max_rounds > 0, TreasuryError::BadWindow);
    require!(max_plans > 0, TreasuryError::BadWindow);
    validate_constraint_ids(mandatory)?;
    require!(
        required_capabilities.len() <= MAX_REQUIRED_CAPS,
        TreasuryError::InvalidCapabilities
    );
    for (i, cap) in required_capabilities.iter().enumerate() {
        for other in &required_capabilities[..i] {
            require!(cap.id != other.id, TreasuryError::InvalidCapabilities);
        }
    }
    Ok(())
}

/// The payload hash a `treasury.policy.v1` transition declares for its
/// amendment (design §4.1, `apply_policy_amendment`): SHA-256 over
/// the target type followed by every new policy value, in argument
/// order, little-endian, with the two variable-length sets prefixed
/// by their counts — the same commitment shape as `claims_hash`
/// (fixed-width fields in order, counts before variable data), so the
/// hash is a function of exactly the values applied and nothing else.
/// A proposer registers the policy transition with this hash as its
/// declared hash; at execution the handler recomputes it from the
/// supplied values and refuses any mismatch.
#[allow(clippy::too_many_arguments)]
pub fn policy_payload_hash(
    target_type: [u8; 32],
    quorum_required: u64,
    confidence_threshold: u64,
    min_obs_confidence: u64,
    require_objectives: bool,
    planning_window: u64,
    max_rounds: u32,
    max_plans: u32,
    mandatory_constraints: &[[u8; 32]],
    required_capabilities: &[CapabilityRequirement],
) -> [u8; 32] {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&target_type);
    bytes.extend_from_slice(&quorum_required.to_le_bytes());
    bytes.extend_from_slice(&confidence_threshold.to_le_bytes());
    bytes.extend_from_slice(&min_obs_confidence.to_le_bytes());
    bytes.push(u8::from(require_objectives));
    bytes.extend_from_slice(&planning_window.to_le_bytes());
    bytes.extend_from_slice(&max_rounds.to_le_bytes());
    bytes.extend_from_slice(&max_plans.to_le_bytes());
    bytes.extend_from_slice(&(mandatory_constraints.len() as u32).to_le_bytes());
    for id in mandatory_constraints {
        bytes.extend_from_slice(id);
    }
    bytes.extend_from_slice(&(required_capabilities.len() as u32).to_le_bytes());
    for cap in required_capabilities {
        bytes.extend_from_slice(&cap.id);
        bytes.extend_from_slice(&cap.min_weight.to_le_bytes());
    }
    anchor_lang::solana_program::hash::hash(&bytes).to_bytes()
}

/// The `apply_policy_amendment` gate, as a pure check (design §4.1):
/// Settled state first, then the one-time execution flag, then the
/// canonical policy-transition type, then the target record's stored
/// type, then the payload hash against the declared hash, then the
/// exact-settle condition (realized == declared). Any failure is a
/// refusal to write: the order puts the cheapest, most general
/// failures first and the binding checks last, so no partial
/// amendment can ever be constructed from a subset of these tests.
#[allow(clippy::too_many_arguments)]
pub fn can_apply_policy_amendment(
    state: u8,
    source_type: [u8; 32],
    target_stored_type: [u8; 32],
    target_type: [u8; 32],
    payload_hash: [u8; 32],
    declared_hash: [u8; 32],
    realized_hash: [u8; 32],
    already_applied: bool,
) -> Result<()> {
    require!(state == STATE_SETTLED, TreasuryError::BadState);
    require!(!already_applied, TreasuryError::BadState);
    require!(
        source_type == POLICY_TRANSITION_TYPE,
        TreasuryError::AmendmentMismatch
    );
    require!(
        target_stored_type == target_type,
        TreasuryError::AmendmentMismatch
    );
    require!(
        payload_hash == declared_hash,
        TreasuryError::AmendmentMismatch
    );
    require!(
        realized_hash == declared_hash,
        TreasuryError::AmendmentMismatch
    );
    Ok(())
}

/// The `initialize_treasury` gate, as a pure check (design §4.2):
/// Settled state first, then the canonical `treasury.init.v1`
/// type, then the FeeVault binding (the created vault's key equals
/// `Config.fee_vault`). Order mirrors
/// [`can_apply_policy_amendment`]: the general failure first, the
/// bindings last. Deliberately no exact-settle check: the init's
/// effect is parameter-free, so no declared payload exists for an
/// inexact settle to diverge from — the binding is Settled + type
/// + vault, and re-execution is refused by the `init` constraints,
/// not by a flag.
pub fn can_initialize_treasury(
    state: u8,
    source_type: [u8; 32],
    vault_matches: bool,
) -> Result<()> {
    require!(state == STATE_SETTLED, TreasuryError::BadState);
    require!(
        source_type == TREASURY_INIT_TYPE,
        TreasuryError::InitMismatch
    );
    require!(vault_matches, TreasuryError::VaultMismatch);
    Ok(())
}

/// The `launch_stage2` gate, as a pure check (design §4.2):
/// Settled state first, then the canonical `treasury.launch.v1`
/// type, then the stage still being Stage 1, then threshold B set.
/// Order mirrors [`can_initialize_treasury`]: the general failure
/// first, the bindings last. The threshold check is the D-2 gate in
/// executable form — an unset (0) threshold refuses, so an unset B
/// can never read as "any backing passes". Deliberately no
/// exact-settle check, as with the init: the launch's effect is a
/// fixed flag flip, so no declared payload exists for an inexact
/// settle to diverge from. The BACKING_EVIDENCED attestation itself
/// is judged at commit time through the launch policy's mandatory
/// constraints (the CONSTRAINTS bit), not here.
pub fn can_launch_stage2(
    state: u8,
    source_type: [u8; 32],
    stage: u8,
    backing_threshold_ppm: u64,
) -> Result<()> {
    require!(state == STATE_SETTLED, TreasuryError::BadState);
    require!(
        source_type == TREASURY_LAUNCH_TYPE,
        TreasuryError::LaunchMismatch
    );
    require!(stage == STAGE_1, TreasuryError::AlreadyLaunched);
    require!(
        backing_threshold_ppm != 0,
        TreasuryError::ThresholdUnset
    );
    Ok(())
}

/// Event for [`treasury_program::submit_constraint_report`] (design
/// §4.1): the transition, the reporting Evaluator, and the report as
/// submitted — the authorization context the design requires on every
/// event. The CONSTRAINTS bit is judged at commit over the final set,
/// never from this event alone.
#[event]
pub struct ConstraintReported {
    pub transition_id: u64,
    pub evaluator: Pubkey,
    pub constraint_id: [u8; 32],
    pub status: u8,
}

/// Event for [`treasury_program::submit_objectives_check`] (design
/// §4.1): the transition, the reporting Evaluator, and the verdict as
/// submitted — the authorization context the design requires on every
/// event. The OBJECTIVES bit is judged at commit over the final flag
/// (and only when the transition type's policy requires objectives),
/// never from this event alone.
#[event]
pub struct ObjectivesReported {
    pub transition_id: u64,
    pub evaluator: Pubkey,
    pub ok: bool,
}

/// Event for [`treasury_program::advance_round`] (design §4.1, §6):
/// the transition and the new current round index — the round the
/// next signal belongs to. The previous round's weighted mean, which
/// the monotone-revision check reads, is snapshotted on the
/// TransitionRecord by the same instruction, not carried in the event.
#[event]
pub struct RoundAdvanced {
    pub transition_id: u64,
    pub round: u32,
}

/// Event for [`treasury_program::reconcile`] (design §4.1): the
/// transition and the fit as computed — a false fit still advances
/// the state to Committable, and the COMPAT bit at commit is what
/// refuses it. The event is the receipt that reconcile ran, not a
/// promise that the transition can commit.
#[event]
pub struct Reconciled {
    pub transition_id: u64,
    pub fits: bool,
}

/// Event for [`treasury_program::commit`] (design §4.1, §6): the
/// transition, the commit index the chain advanced to, and the
/// declared hash that is now the chain head — the authorization
/// context the design requires on every event. A refused commit emits
/// nothing: the mask is in the program logs, and the absence of this
/// event is the receipt that nothing committed.
#[event]
pub struct Committed {
    pub transition_id: u64,
    pub commit_index: u64,
    pub declared_hash: [u8; 32],
}

/// Event for [`treasury_program::record_evidence`] (design §4.1, §6):
/// the transition and the realized hash as recorded — the receipt
/// `settle` will be checked against. The event is the record's public
/// face, not a second copy of it: settle reads the EvidenceRecord
/// account, never this event.
#[event]
pub struct EvidenceRecorded {
    pub transition_id: u64,
    pub realized_hash: [u8; 32],
}

/// Event for [`treasury_program::settle`] (design §4.1, §6): the
/// transition, the commit index recorded at commit time (the same
/// number the `Committed` event carried), the realized hash as
/// settled, and `exact` (declared == realized). An inexact settle is
/// still a settle — the event says so plainly instead of hiding the
/// drift — and the halved positive credit is the kernel's consequence,
/// recorded on the attributed agents' records.
#[event]
pub struct Settled {
    pub transition_id: u64,
    pub commit_index: u64,
    pub realized_hash: [u8; 32],
    pub exact: bool,
}

/// Event for [`treasury_program::raise_exception`] (design §4.1): the
/// transition, the raising Evaluator, the exception id, its
/// disposition, and whether it blocks (Block/Emergency) — the fact
/// `commit` will judge through `unresolved_blocking`.
#[event]
pub struct ExceptionRaised {
    pub transition_id: u64,
    pub evaluator: Pubkey,
    pub exception_id: [u8; 32],
    pub disposition: u8,
    pub blocking: bool,
}

/// Event for [`treasury_program::resolve_exception`] (design §4.1):
/// the transition, the resolving Evaluator, and the exception id.
/// A re-resolution emits the same event again — the event records
/// the call, not a state change, since re-resolution is a no-op.
#[event]
pub struct ExceptionResolved {
    pub transition_id: u64,
    pub evaluator: Pubkey,
    pub exception_id: [u8; 32],
}

/// Event for [`treasury_program::reject_transition`] (design §4.1,
/// §6): the transition and the vetoing agent — the vetoer is the
/// authorization context for a terminal close, as the Evaluator is
/// for an exception. A rejected chain closes honestly instead of
/// dangling.
#[event]
pub struct TransitionRejected {
    pub transition_id: u64,
    pub vetoer: Pubkey,
}

/// Event for [`treasury_program::expire_transition`] (design §4.1,
/// §6): the transition only. Expiry is permissionless — no agent's
/// authority closes the chain, the Clock does — so there is no
/// vetoer to name, and inventing one would misstate the authority.
#[event]
pub struct TransitionExpired {
    pub transition_id: u64,
}

/// Event for [`treasury_program::policy_transition`] (design §4.1):
/// the installed type and the installing authority — the
/// authorization context for a genesis policy install. The installed
/// record itself is the payload; the `PolicyCommitted` /
/// `PolicySettled` events of the design table belong to the later
/// `treasury.policy.v1` amendment execution, not to this install.
#[event]
pub struct PolicyInstalled {
    pub transition_type: [u8; 32],
    pub authority: Pubkey,
}

/// Event for [`treasury_program::apply_policy_amendment`] (design
/// §4.1): the settled policy transition, the amended target type,
/// and the permissionless executor — the authorization context for
/// a post-genesis policy change. The design table's
/// `PolicyCommitted` / `PolicySettled` are the policy transition's
/// own lifecycle events (emitted by `commit` / `settle`); this event
/// is the execution receipt, distinct from both.
#[event]
pub struct PolicyAmended {
    pub transition_id: u64,
    pub target_type: [u8; 32],
    pub executor: Pubkey,
}

/// Event for [`treasury_program::initialize_treasury`] (design
/// §4.2, §6): the authorizing transition (id + commit index — the
/// authorization context) and the three created accounts. The
/// genesis exception does not apply here: unlike
/// `initialize_config`, a chain already exists, so this event names
/// its transition like every other non-genesis event.
#[event]
pub struct TreasuryInitialized {
    pub transition_id: u64,
    pub commit_index: u64,
    pub treasury_state: Pubkey,
    pub lending_book: Pubkey,
    pub fee_vault: Pubkey,
    pub executor: Pubkey,
}

/// Event for [`treasury_program::launch_stage2`] (design §4.2,
/// §6): the authorizing launch transition (id + commit index —
/// the authorization context) and the permissionless executor.
/// The stage flag itself is on Config; the event is the execution
/// receipt, distinct from the launch transition's own lifecycle
/// events (emitted by `commit` / `settle`).
#[event]
pub struct Stage2Launched {
    pub transition_id: u64,
    pub commit_index: u64,
    pub executor: Pubkey,
}

/// Capacity stock installed (design §4.1): the `set_capacity` event,
/// carrying the installed balance so the stock's genesis is on the
/// receipt chain, like the policy install above it.
#[event]
pub struct CapacityStockSet {
    pub resource_id: u16,
    pub balance: u64,
    pub authority: Pubkey,
}

/// One settle attribution: an agent and its reputation delta (kernel
/// `Delta`, i64, in fixed-point raw units). The kernel sums deltas
/// per agent, caps at ±20 units, and halves positive credit on an
/// inexact settle — the program's `settle_attribution` helper is that
/// rule, and this struct is its input shape, not a stored account.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attribution {
    /// The attributed agent's key — must name a plan participant.
    pub agent: Pubkey,
    /// The delta, kernel fixed-point raw (SCALE 1_000_000).
    pub delta: i64,
}

/// Event for [`treasury_program::submit_signal`] (design §4.1): the
/// transition, the signalling agent, and the signal as submitted — the
/// authorization context the design requires on every event. The
/// effective weight is computed at commit replay, never trusted from
/// the client, so it is not in the event.
#[event]
pub struct SignalSubmitted {
    pub transition_id: u64,
    pub agent: Pubkey,
    pub raw: u64,
    pub support: u64,
}

/// A plan's claims must fit the account and the kernel's keyed-map
/// shape: at most [`MAX_CLAIMS`] entries, no resource id twice (a
/// duplicate key would double-count at reconcile).
pub fn validate_claims(claims: &[Claim]) -> Result<()> {
    require!(claims.len() <= MAX_CLAIMS, TreasuryError::InvalidClaims);
    for (i, claim) in claims.iter().enumerate() {
        for other in &claims[..i] {
            require!(
                claim.resource_id != other.resource_id,
                TreasuryError::InvalidClaims
            );
        }
    }
    Ok(())
}

/// The claims hash carried by the `PlanSubmitted` event: SHA-256 over
/// the concatenated little-endian (resource_id, amount) pairs, in the
/// order submitted. The encoding is the commitment — reconcile and
/// commit re-derive it from the stored claims, so a stored plan whose
/// hash does not match its claims is evidence of tampering, not a
/// second copy of the data.
pub fn claims_hash(claims: &[Claim]) -> [u8; 32] {
    let mut bytes = Vec::new();
    for claim in claims {
        bytes.extend_from_slice(&claim.resource_id.to_le_bytes());
        bytes.extend_from_slice(&claim.amount.to_le_bytes());
    }
    anchor_lang::solana_program::hash::hash(&bytes).to_bytes()
}

/// Event for [`treasury_program::submit_plan`] (design §4.1): the
/// transition, the submitting agent, and the claims hash — the
/// authorization context the design requires on every event.
#[event]
pub struct PlanSubmitted {
    pub transition_id: u64,
    pub agent: Pubkey,
    pub claims_hash: [u8; 32],
}

/// TransitionRecord states (design §3): the kernel's seven, exactly, in
/// the kernel's order — Registered, Planning, Committable, Committed,
/// Settled, Rejected, Expired.
pub const STATE_REGISTERED: u8 = 0;
pub const STATE_PLANNING: u8 = 1;
pub const STATE_COMMITTABLE: u8 = 2;
pub const STATE_COMMITTED: u8 = 3;
pub const STATE_SETTLED: u8 = 4;
pub const STATE_REJECTED: u8 = 5;
pub const STATE_EXPIRED: u8 = 6;

/// Validate an observation's window against on-chain time (design §4.1).
///
/// Future-dated observations are verification theater, not data: the
/// timestamp must not exceed `now`. An expiry at or before the
/// timestamp is not a window at all.
pub fn validate_observation_window(timestamp: i64, expiry: i64, now: i64) -> Result<()> {
    require!(
        timestamp <= now,
        TreasuryError::ObservationFromFuture
    );
    require!(expiry > timestamp, TreasuryError::BadWindow);
    Ok(())
}

/// Validate a transition manifest's expiry against on-chain time
/// (design §4.1): a manifest already expired at registration is a bad
/// window and creates nothing.
pub fn validate_transition_expiry(expires_at: i64, now: i64) -> Result<()> {
    require!(expires_at > now, TreasuryError::BadWindow);
    Ok(())
}

/// Genesis accounts for [`treasury_program::initialize_config`].
#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    /// Program configuration PDA, seeds `["config"]`. One-time init.
    #[account(
        init,
        payer = authority,
        space = Config::SPACE,
        seeds = [b"config"],
        bump
    )]
    pub config: Account<'info, Config>,
    /// Declared chain head PDA, seeds `["chain"]`. One-time init at
    /// genesis (index 0, zero hash); only `commit` advances it, and
    /// only on a clear mask.
    #[account(
        init,
        payer = authority,
        space = DeclaredChain::SPACE,
        seeds = [b"chain"],
        bump
    )]
    pub declared_chain: Account<'info, DeclaredChain>,
    /// Genesis authority (the governance multisig at genesis). Pays rent.
    #[account(mut)]
    pub authority: Signer<'info>,
    /// Rent sysvar, required by the `init` constraint.
    pub system_program: Program<'info, System>,
}

/// Program parameters that are NOT policy (design §3).
///
/// Policy (quorum, confidence thresholds, planning windows, mandatory
/// constraints, required capabilities) lives in PolicyRecords and is amended
/// only via `treasury.policy.v1` transitions — never by editing this account.
#[account]
pub struct Config {
    /// Genesis authority (governance multisig).
    pub authority: Pubkey,
    /// Collection account for the 73 bps single fee flow (D-5).
    pub fee_vault: Pubkey,
    /// Stage flag: [`STAGE_1`] until `treasury.launch.v1` settles.
    pub stage: u8,
    /// PDA bump for the `["config"]` seed.
    pub bump: u8,
}

impl Config {
    /// 8 (discriminator) + 32 + 32 + 1 + 1.
    pub const SPACE: usize = 8 + 32 + 32 + 1 + 1;
}

/// Maximum groups per agent (design §3: groups are the kernel's u16
/// `GroupId`). The account reserves exactly this many `Vec` slots so
/// `SPACE` is a fixed upper bound, not a function of the caller's input.
pub const MAX_GROUPS: usize = 8;

/// Maximum capability holdings per agent (design §3
/// CapabilityRegistry: the kernel's `CapabilityId` set per agent).
/// Mirrors the groups bound — the kernel's set is unbounded, but an
/// on-chain record is not; raising it is a SPACE change, not a
/// policy change.
pub const MAX_CAPABILITIES: usize = 8;

/// Reputation ceiling in kernel fixed-point raw units: 100 units at
/// SCALE 1_000_000, matching the launch config (kernel_bridge).
pub const REP_MAX_RAW: u64 = 100_000_000;

/// Genesis accounts for [`treasury_program::register_agent`].
#[derive(Accounts)]
#[instruction(agent: Pubkey)]
pub struct RegisterAgent<'info> {
    /// Program configuration PDA — read to check the authority.
    #[account(seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, Config>,
    /// Agent record PDA, seeds `["agent", agent]`. One-time init.
    #[account(
        init,
        payer = authority,
        space = AgentRecord::SPACE,
        seeds = [b"agent", agent.as_ref()],
        bump
    )]
    pub agent_record: Account<'info, AgentRecord>,
    /// Genesis authority (the governance multisig at genesis). Pays rent.
    #[account(mut)]
    pub authority: Signer<'info>,
    /// Rent sysvar, required by the `init` constraint.
    pub system_program: Program<'info, System>,
}

/// Accounts for [`treasury_program::grant_role`] and
/// [`treasury_program::revoke_role`].
#[derive(Accounts)]
#[instruction(agent: Pubkey)]
pub struct RoleChange<'info> {
    /// Program configuration PDA — read to check the authority.
    #[account(seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, Config>,
    /// The agent's existing record (must already exist).
    #[account(mut, seeds = [b"agent", agent.as_ref()], bump = agent_record.bump)]
    pub agent_record: Account<'info, AgentRecord>,
    /// Genesis authority (the governance multisig at genesis).
    pub authority: Signer<'info>,
}

/// One registered agent (design §3 `AgentRecord`).
///
/// Mirrors the kernel's `AgentProfile` plus the role set the kernel
/// keeps separately: the kernel's three roles (Admin, Oracle, Evaluator)
/// are the only roles — "Operator" is not a role, it is an agent class
/// holding capabilities (design §5). Influence and reputation are the
/// kernel's fixed-point raw values (SCALE 1_000_000); under FlatWeights
/// at launch, reputation is recorded and influence on quorum is zero
/// by policy, not by this account's contents.
#[account]
pub struct AgentRecord {
    /// The agent's key — the kernel's `AgentId` ([u8; 32]).
    pub agent: Pubkey,
    /// Whether the agent is active (kernel `AgentProfile.active`).
    pub active: bool,
    /// Role flags, kernel `Role` order: Admin / Oracle / Evaluator.
    pub is_admin: bool,
    pub is_oracle: bool,
    pub is_evaluator: bool,
    /// Influence, kernel fixed-point raw (kernel `Amount`).
    pub influence_raw: u64,
    /// Reputation, kernel fixed-point raw, bounded 0–100 units.
    pub reputation_raw: u64,
    /// Group memberships (kernel `GroupId`, u16), at most [`MAX_GROUPS`].
    pub groups: Vec<u16>,
    /// Capability holdings (kernel `CapabilityId`, [u8; 32]), at most
    /// [`MAX_CAPABILITIES`] — the design §3 CapabilityRegistry, stored
    /// on the agent's own record. The CAPABILITY bit at commit sums
    /// the influence of plan authors holding each required
    /// capability, so these holdings are the bit's on-chain input.
    pub capabilities: Vec<[u8; 32]>,
    /// PDA bump for the `["agent", agent]` seeds.
    pub bump: u8,
}

impl AgentRecord {
    /// 8 (discriminator) + 32 + 1 + 1 + 1 + 1 + 8 + 8 + (4 + 8×2)
    /// + (4 + 8×32) + 1.
    pub const SPACE: usize = 8 + 32 + 4 + 8 + 8 + 20 + 260 + 1;

    /// Set or clear one role flag. Role codes: 0 = Admin, 1 = Oracle,
    /// 2 = Evaluator — the kernel's `Role` enum order. Any other code is
    /// an error and changes nothing.
    pub fn apply_role(&mut self, role: u8, grant: bool) -> Result<()> {
        match role {
            0 => self.is_admin = grant,
            1 => self.is_oracle = grant,
            2 => self.is_evaluator = grant,
            _ => return Err(TreasuryError::InvalidRole.into()),
        }
        Ok(())
    }

    /// Grant one capability (kernel `grant_capability`'s set insert).
    /// Idempotent: granting a capability already held is a no-op
    /// success and does not consume a slot. A ninth distinct
    /// capability is `TooManyCapabilities` — the kernel's set is
    /// unbounded, but an on-chain record is not, and the bound is a
    /// SPACE change, not a policy change.
    pub fn grant_capability(&mut self, capability: [u8; 32]) -> Result<()> {
        if self.capabilities.contains(&capability) {
            return Ok(());
        }
        require!(
            self.capabilities.len() < MAX_CAPABILITIES,
            TreasuryError::TooManyCapabilities
        );
        self.capabilities.push(capability);
        Ok(())
    }

    /// Revoke one capability. No-op success when the agent does not
    /// hold it — the end state (capability absent) is what the
    /// caller asked for, mirroring `apply_role`'s revoke shape.
    pub fn revoke_capability(&mut self, capability: [u8; 32]) {
        self.capabilities.retain(|held| *held != capability);
    }
}

/// Accounts for [`treasury_program::post_observation`].
#[derive(Accounts)]
#[instruction(observation_id: u64)]
pub struct PostObservation<'info> {
    /// The posting Oracle's agent record (must exist, be active, and
    /// hold the Oracle role). Seeds bind it to the signer.
    #[account(mut, seeds = [b"agent", oracle.key().as_ref()], bump = agent_record.bump)]
    pub agent_record: Account<'info, AgentRecord>,
    /// The posting Oracle (signer; pays rent for the observation).
    #[account(mut)]
    pub oracle: Signer<'info>,
    /// Observation PDA, seeds `["observation", observation_id]`.
    /// One-time init: a duplicate id fails and changes no state.
    #[account(
        init,
        payer = oracle,
        space = Observation::SPACE,
        seeds = [b"observation", observation_id.to_le_bytes().as_ref()],
        bump
    )]
    pub observation: Account<'info, Observation>,
    /// Rent sysvar, required by the `init` constraint.
    pub system_program: Program<'info, System>,
    /// Clock sysvar: freshness is judged on-chain, never from a client
    /// timestamp (design §4.1, the FRESH bit's anchor).
    pub clock: Sysvar<'info, Clock>,
}

/// One Oracle-posted observation (design §3 `Observation`).
///
/// The FRESH bit reads through this account at commit: `timestamp ≤ now
/// < expiry` and `confidence ≥ Policy.min_obs_confidence`. The content
/// hash is the observation's content binding; the content itself lives
/// off-chain and is attested by the hash.
#[account]
pub struct Observation {
    /// Caller-chosen id (PDA seed component).
    pub observation_id: u64,
    /// The posting Oracle's key.
    pub oracle: Pubkey,
    /// Observation timestamp (must be ≤ Clock time at posting).
    pub timestamp: i64,
    /// Expiry: the observation is fresh only while `now < expiry`.
    pub expiry: i64,
    /// Confidence, kernel fixed-point raw (SCALE 1_000_000).
    pub confidence: u64,
    /// Content hash binding the off-chain observation content.
    pub content_hash: [u8; 32],
    /// PDA bump for the `["observation", id]` seeds.
    pub bump: u8,
}

impl Observation {
    /// 8 (discriminator) + 8 + 32 + 8 + 8 + 8 + 32 + 1.
    pub const SPACE: usize = 8 + 8 + 32 + 8 + 8 + 8 + 32 + 1;
}

/// Accounts for [`treasury_program::register_transition`].
#[derive(Accounts)]
#[instruction(transition_id: u64, observation_id: u64, transition_type: [u8; 32])]
pub struct RegisterTransition<'info> {
    /// The manifest's observation — must already exist (design §4.1:
    /// UnknownObservation if it does not; enforced by this account
    /// constraint, not by a returned error).
    #[account(seeds = [b"observation", observation_id.to_le_bytes().as_ref()], bump = observation.bump)]
    pub observation: Account<'info, Observation>,
    /// The transition type's policy — must already exist (installed by
    /// `policy_transition` at genesis). The seeds bind it to the
    /// registered type, so the mandatory set it contributes cannot be
    /// swapped for another type's policy at registration time.
    #[account(seeds = [b"policy", transition_type.as_ref()], bump = policy_record.bump)]
    pub policy_record: Account<'info, PolicyRecord>,
    /// TransitionRecord PDA, seeds `["transition", transition_id]`.
    /// One-time init: a duplicate id fails and changes no state.
    #[account(
        init,
        payer = payer,
        space = TransitionRecord::SPACE,
        seeds = [b"transition", transition_id.to_le_bytes().as_ref()],
        bump
    )]
    pub transition_record: Account<'info, TransitionRecord>,
    /// Caller (pays rent). Registration itself is permissionless in the
    /// design table — no role gates who may create a transition; the
    /// predicate, not the caller, decides at commit.
    #[account(mut)]
    pub payer: Signer<'info>,
    /// Rent sysvar, required by the `init` constraint.
    pub system_program: Program<'info, System>,
    /// Clock sysvar: the manifest expiry is judged on-chain.
    pub clock: Sysvar<'info, Clock>,
}

/// Accounts for [`treasury_program::open_planning`].
#[derive(Accounts)]
#[instruction(transition_id: u64)]
pub struct OpenPlanning<'info> {
    /// The caller's agent record (must exist and be bound to the
    /// signer). Admin status is read from here: only the proposer or
    /// an Admin may open planning (design §4.1).
    #[account(seeds = [b"agent", caller.key().as_ref()], bump = agent_record.bump)]
    pub agent_record: Account<'info, AgentRecord>,
    /// The transition record to move Registered → Planning.
    #[account(mut, seeds = [b"transition", transition_id.to_le_bytes().as_ref()], bump = transition_record.bump)]
    pub transition_record: Account<'info, TransitionRecord>,
    /// The transition type's policy — seeds-bound to the record's
    /// `transition_type`, so the planning window read here is the
    /// governing type's own, never a stranger's policy. A missing
    /// policy fails validation, fail-closed (registration already
    /// requires it, so every live transition has one).
    #[account(seeds = [b"policy", transition_record.transition_type.as_ref()], bump = policy_record.bump)]
    pub policy_record: Account<'info, PolicyRecord>,
    /// The caller (proposer or Admin). Pays no rent here.
    pub caller: Signer<'info>,
    /// Clock sysvar: the planning deadline is judged on-chain.
    pub clock: Sysvar<'info, Clock>,
}

/// Maximum constraints in a frozen set (design §3: the frozen set is
/// mandatory ∪ proposer additions, fixed at registration). An
/// account-size bound, not a kernel rule — the launch policy types
/// carry at most 4 mandatory constraints (alloc), so 8 carries
/// headroom for proposer additions, and raising the bound is a SPACE
/// change, not a policy change.
pub const MAX_CONSTRAINTS: usize = 8;

/// Maximum required capabilities on one transition type's policy
/// (design §4.1: the kernel's `required_caps` per type). An
/// account-size bound, not a kernel rule — the kernel's list is
/// unbounded, but an on-chain record is not; 8 mirrors the frozen-set
/// bound, and raising it is a SPACE change, not a policy change.
pub const MAX_REQUIRED_CAPS: usize = 8;

/// One entry of a transition's frozen constraint set: the kernel's
/// `ConstraintId` ([u8; 32]) plus its current status, one of the
/// `STATUS_*` codes. Every entry starts Unavailable at registration;
/// `submit_constraint_report` overwrites the status, never the set.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConstraintEntry {
    /// The constraint's id (kernel `ConstraintId`).
    pub id: [u8; 32],
    /// Current status: Pass (0), Fail (1), or Unavailable (2).
    pub status: u8,
}

/// One required capability on a transition type's policy: the kernel's
/// `required_caps` entry (a `CapabilityId` plus the minimum summed
/// influence its plan authors must hold). The CAPABILITY bit at commit
/// is the coverage test over these entries; the holdings they are
/// checked against live on AgentRecords and land with
/// `grant_capability` — this struct records the requirement, not the
/// holding.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CapabilityRequirement {
    /// The capability's id (kernel `CapabilityId`).
    pub id: [u8; 32],
    /// Minimum summed influence of plan authors holding it, raw
    /// fixed-point units (kernel `Amount`).
    pub min_weight: u64,
}

/// Maximum exceptions on one transition (design §3: exceptions are
/// per-transition records on the TransitionRecord). An account-size
/// bound, not a kernel rule — the kernel's map is unbounded, but an
/// on-chain record is not; 8 mirrors the frozen-set bound, and
/// raising it is a SPACE change, not a policy change.
pub const MAX_EXCEPTIONS: usize = 8;

/// Disposition codes, in the kernel's `Disposition` order
/// (Informational, Review, Block, Emergency). Only Block and
/// Emergency count toward `unresolved_blocking` (kernel
/// `raise_exception`: `d >= Disposition::Block`).
pub const DISPOSITION_INFORMATIONAL: u8 = 0;
pub const DISPOSITION_REVIEW: u8 = 1;
pub const DISPOSITION_BLOCK: u8 = 2;
pub const DISPOSITION_EMERGENCY: u8 = 3;

/// One entry of a transition's per-transition exception set: the
/// kernel's `ExceptionId` ([u8; 32]) plus its disposition and whether
/// it has resolved. Raised by `raise_exception` (Evaluator only,
/// pre-commit states), resolved by `resolve_exception` (Evaluator
/// only, any state — the kernel imposes no state gate on resolve);
/// only unresolved Block/Emergency entries count toward the
/// record's `unresolved_blocking`, which `commit` feeds to the
/// EXCEPTIONS bit.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExceptionEntry {
    /// The exception's id (kernel `ExceptionId`).
    pub id: [u8; 32],
    /// Disposition: one of the `DISPOSITION_*` codes.
    pub disposition: u8,
    /// Whether `resolve_exception` has resolved this entry.
    pub resolved: bool,
}

/// One kernel transition's on-chain record (design §3 `TransitionRecord`,
/// first fields).
///
/// Per-transition kernel state lives here, serialized (design §1.2:
/// load → call → store). This slice carries the manifest inputs, the
/// recorded proposer, the state, the plan count, the round count, the
/// current-round aggregates, the previous-round mean, the frozen
/// constraint set, the objectives flag, the reconciled flag, and —
/// as of this slice — the per-transition exception set with its
/// unresolved-blocking count, which `commit` feeds to the EXCEPTIONS
/// bit; each field lands with the instruction that writes it, not
/// before.
#[account]
pub struct TransitionRecord {
    /// Caller-chosen id (PDA seed component).
    pub transition_id: u64,
    /// The transition's type (kernel `TransitionType`, [u8; 32]) —
    /// the PolicyRecord whose mandatory set and caps govern it.
    /// Frozen at registration, like the rest of the manifest.
    pub transition_type: [u8; 32],
    /// One of the `STATE_*` constants (kernel's seven states).
    pub state: u8,
    /// The registering payer's key — the kernel's recorded proposer.
    /// `open_planning` admits only this key or an Admin.
    pub proposer: Pubkey,
    /// The manifest's observation id (FRESH reads through it).
    pub observation_id: u64,
    /// Manifest expiry (past it, the transition can only expire).
    pub expires_at: i64,
    /// Planning deadline, set by `open_planning`; 0 until then.
    pub planning_deadline: i64,
    /// Plans submitted so far, counted by `submit_plan` so the
    /// kernel's `TooManyPlans` rule is enforceable on-chain.
    pub plan_count: u32,
    /// Rounds so far, starting at 1 — the kernel starts every
    /// transition with one empty round, and `advance_round` adds one.
    /// `submit_signal` signals the current round, `round_count - 1`.
    pub round_count: u32,
    /// Effective-weight total of the current round, maintained by
    /// `submit_signal` (FlatWeights: raw x signer influence). The
    /// kernel's RoundEmpty test is `total == Amount::ZERO`, and this
    /// is the field that test reads on-chain.
    pub current_round_total: u64,
    /// Weighted-support sum of the current round, maintained by
    /// `submit_signal` (effective weight x support, Amount arithmetic).
    /// `advance_round` derives the previous round's mean from this
    /// and the total, then resets both to zero for the new round.
    pub current_round_weighted: u64,
    /// Weighted mean support of the most recently closed round,
    /// snapshotted by `advance_round` (weighted x ONE / total). Zero
    /// until the first advance — the monotone-revision check reads
    /// this field, and round 0 has no previous round to read.
    pub prev_round_mean: u64,
    /// The frozen constraint set (design §3): derived at
    /// `register_transition` as the type's PolicyRecord mandatory set
    /// ∪ the proposer's additions, deduped, all Unavailable; reported
    /// against by `submit_constraint_report`, never extended after
    /// registration. Each entry is 32 (id) + 1 (status) bytes.
    pub constraints: Vec<ConstraintEntry>,
    /// The independent objectives verdict (kernel `objectives_ok`):
    /// false at registration, overwritten by each
    /// `submit_objectives_check` report. The OBJECTIVES bit at commit
    /// is `require_objectives && !objectives_ok` — at launch the
    /// policy does not require objectives, so the bit is clear
    /// regardless, but the flag is recorded, not assumed.
    pub objectives_ok: bool,
    /// The capacity-fit verdict from `reconcile` (kernel `reconciled`):
    /// false at registration, set once by `reconcile` when the
    /// transition moves Planning → Committable. A false value still
    /// advances the state — the COMPAT bit at commit is
    /// `reconciled && fits`, re-checked, so a false verdict can never
    /// commit. It is a verdict, not a gate that blocks the state move.
    pub reconciled: bool,
    /// The transition's declared next hash (commit/commitment anchor).
    pub declared_hash: [u8; 32],
    /// Manifest parent index, frozen at registration — the LINEAGE
    /// bit is judged against the declared chain head at commit time,
    /// and this is the parent it must equal.
    pub parent_index: u64,
    /// Manifest parent hash, frozen at registration with the index.
    pub parent_hash: [u8; 32],
    /// The chain index this transition committed at, recorded by
    /// `commit` (the settle slice's follow-up, closed with `settle`):
    /// 0 until commit, then the new chain index — the same number the
    /// `Committed` event carried, so settle's event can name it.
    pub commit_index: u64,
    /// The realized hash, recorded by `settle` (zero until then): the
    /// hash the transition settled against, equal to the
    /// EvidenceRecord's hash by the settle gate.
    pub realized_hash: [u8; 32],
    /// The per-transition exception set (design §3): raised by
    /// `raise_exception`, resolved by `resolve_exception`, never
    /// extended after the transition leaves its pre-commit states.
    /// Each entry is 32 (id) + 1 (disposition) + 1 (resolved) bytes.
    pub exceptions: Vec<ExceptionEntry>,
    /// Unresolved blocking exceptions on this transition (kernel
    /// `unresolved_blocking`): Block/Emergency entries raised and not
    /// yet resolved. `commit` feeds this count to the EXCEPTIONS bit;
    /// a non-zero count refuses every commit until resolved.
    pub unresolved_blocking: u32,
    /// Program-layer execution marker, not kernel state:
    /// `apply_policy_amendment` sets this once it applies a Settled
    /// policy transition's amendment, and refuses a second execution.
    /// False at registration; meaningful only on a Settled transition
    /// of the canonical policy type, and never set on any other
    /// transition.
    pub policy_amendment_applied: bool,
    /// PDA bump for the `["transition", id]` seeds.
    pub bump: u8,
}

impl TransitionRecord {
    /// 8 (discriminator) + 8 + 32 + 1 + 32 + 8 + 8 + 8 + 4 + 4
    /// + 8 + 8 + 8 + (4 + 8 × 33) + 1 + 1 + 32 + 8 + 32 + 8 + 32
    /// + (4 + 8 × 34) + 4 + 1 + 1 = 801.
    pub const SPACE: usize = 8 + 8 + 32 + 1 + 32 + 8 + 8 + 8 + 4 + 4 + 8 + 8 + 8 + 268 + 1 + 1 + 32 + 8 + 32 + 8 + 32 + 276 + 4 + 1 + 1;
}

/// One transition type's policy (design §3 `PolicyRecord`, §4.1
/// launch policy table): the kernel `Policy` for a type, plus the
/// mandatory constraint set and required capabilities the kernel keeps
/// per type (`mandatory`, `required_caps`). Installed by
/// `policy_transition` in genesis-direct form; after genesis, changes
/// are the execution of a committed `treasury.policy.v1` transition,
/// never a fresh authority signature. The launch table's numeric values
/// are identical across types at genesis (quorum 1.0, confidence 0.5,
/// minimum observation confidence 0.5, objectives not required, 3
/// rounds, 4 plans); the types differ in their mandatory sets and
/// required capabilities, which is where the per-type policy lives.
#[account]
pub struct PolicyRecord {
    /// The transition type this record governs (kernel
    /// `TransitionType`, [u8; 32]; PDA seed component).
    pub transition_type: [u8; 32],
    /// Quorum: the last round's effective-weight total must reach this
    /// (kernel `quorum_required`, raw fixed-point units).
    pub quorum_required: u64,
    /// Confidence threshold for the last round's mean support (kernel
    /// `confidence_threshold`, raw units).
    pub confidence_threshold: u64,
    /// Minimum observation confidence for the FRESH bit (kernel
    /// `min_obs_confidence`, raw units).
    pub min_obs_confidence: u64,
    /// Whether the transition type requires an objectives report (kernel
    /// `require_objectives`).
    pub require_objectives: bool,
    /// Planning window in seconds (kernel `planning_window`; the
    /// engine-sim value carried at genesis, a parameter not a ruling).
    pub planning_window: u64,
    /// Maximum rounds (kernel `max_rounds`).
    pub max_rounds: u32,
    /// Maximum plans (kernel `max_plans`).
    pub max_plans: u32,
    /// Mandatory constraint ids for the type (kernel `mandatory`):
    /// every transition of this type freezes these into its set at
    /// registration, all Unavailable. At most [`MAX_CONSTRAINTS`].
    pub mandatory_constraints: Vec<[u8; 32]>,
    /// Required capabilities for the type (kernel `required_caps`):
    /// each entry's minimum summed plan-author influence. At most
    /// [`MAX_REQUIRED_CAPS`].
    pub required_capabilities: Vec<CapabilityRequirement>,
    /// PDA bump for the `["policy", transition_type]` seeds.
    pub bump: u8,
}

impl PolicyRecord {
    /// 8 (discriminator) + 32 + 8 + 8 + 8 + 1 + 8 + 4 + 4
    /// + (4 + 8 × 32) + (4 + 8 × (32 + 8)) + 1 = 666.
    pub const SPACE: usize = 8 + 32 + 8 + 8 + 8 + 1 + 8 + 4 + 4 + 260 + 324 + 1;
}

/// Maximum claims per plan (design §3: a plan is a capacity-claims
/// map). An account-size bound, not a kernel rule — the kernel's map
/// is unbounded, but an on-chain account is not; the launch surface
/// (3 stocks plus allocation classes) fits 16 with headroom, and
/// raising the bound is a SPACE change, not a policy change.
pub const MAX_CLAIMS: usize = 16;

/// Accounts for [`treasury_program::submit_plan`].
#[derive(Accounts)]
#[instruction(transition_id: u64)]
pub struct SubmitPlan<'info> {
    /// The submitting agent's record (must exist and be bound to the
    /// signer; active flag is the eligibility half enforced here).
    #[account(seeds = [b"agent", caller.key().as_ref()], bump = agent_record.bump)]
    pub agent_record: Account<'info, AgentRecord>,
    /// The transition record to plan against (must be Planning).
    #[account(mut, seeds = [b"transition", transition_id.to_le_bytes().as_ref()], bump = transition_record.bump)]
    pub transition_record: Account<'info, TransitionRecord>,
    /// The transition type's policy — seeds-bound to the record's
    /// `transition_type`; its `max_plans` is the cap this instruction
    /// enforces, never a stranger's policy and never a constant.
    #[account(seeds = [b"policy", transition_record.transition_type.as_ref()], bump = policy_record.bump)]
    pub policy_record: Account<'info, PolicyRecord>,
    /// PlanRecord PDA, seeds `["plan", transition_id, caller]`.
    /// One-time init: a second plan from the same agent fails and
    /// changes no state (Duplicate in the design taxonomy).
    #[account(
        init,
        payer = caller,
        space = PlanRecord::SPACE,
        seeds = [b"plan", transition_id.to_le_bytes().as_ref(), caller.key().as_ref()],
        bump
    )]
    pub plan_record: Account<'info, PlanRecord>,
    /// The submitting agent (signer; pays rent for the plan record).
    #[account(mut)]
    pub caller: Signer<'info>,
    /// Rent sysvar, required by the `init` constraint.
    pub system_program: Program<'info, System>,
    /// Clock sysvar: Planning state and the deadline are judged
    /// on-chain, never from a client timestamp.
    pub clock: Sysvar<'info, Clock>,
}

/// One capacity claim inside a plan: the kernel's claims map is
/// `ResourceId → Amount`, and Anchor's IDL cannot express a tuple
/// type — so the map entry is a named struct, not `(u16, u64)`.
/// Borsh layout is identical (2 + 8 = 10 bytes), and the struct is
/// what the generated IDL carries as a defined type.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Claim {
    /// The capacity stock claimed (launch stocks 0/1/2; allocation
    /// class stocks are ids ≥ 3, design §4.1).
    pub resource_id: u16,
    /// The claimed amount, kernel fixed-point raw (SCALE 1_000_000).
    pub amount: u64,
}

/// One agent's plan for a transition (design §3 `Plan`).
///
/// Capacity claims `{resource_id: amount}` plus expiry, stored so
/// `reconcile` can sum them against the capacity stocks (COMPAT) and
/// `commit` can debit them atomically. The claims hash is the event's
/// commitment over the stored claims (see [`claims_hash`]).
/// Claims are [`Claim`] structs, not tuples: Anchor's IDL cannot
/// express a tuple type, and the first `anchor build` of this slice
/// failed on exactly that (fixed by the struct, same byte layout).
#[account]
pub struct PlanRecord {
    /// The transition this plan belongs to (PDA seed component).
    pub transition_id: u64,
    /// The submitting agent's key (PDA seed component).
    pub agent: Pubkey,
    /// Plan expiry: the kernel refuses a plan expired at submission,
    /// and reconcile/commit judge the plan against its expiry.
    pub expires_at: i64,
    /// Capacity claims, at most [`MAX_CLAIMS`], distinct resource ids.
    pub claims: Vec<Claim>,
    /// SHA-256 over the claims encoding (see [`claims_hash`]).
    pub claims_hash: [u8; 32],
    /// PDA bump for the `["plan", transition_id, agent]` seeds.
    pub bump: u8,
}

impl PlanRecord {
    /// 8 (discriminator) + 8 + 32 + 8 + (4 + 16×10) + 32 + 1.
    pub const SPACE: usize = 8 + 8 + 32 + 8 + 164 + 32 + 1;
}

/// One capacity stock (design §3 `CapacityStock`, program-owned).
///
/// The kernel's capacity map made persistent, one account per
/// resource id: the launch stocks (0 = treasury tranche, 1 = lending
/// book, 2 = exit window) and allocation class stocks (ids ≥ 3).
/// `reconcile` and `commit` read these balances for the COMPAT bit,
/// and a clear `commit` debits them atomically; replenishment
/// happens only inside a committed window/policy flow (design §4.2),
/// never by editing this account ad hoc. The PDA seeds bind one
/// account per resource id, so two accounts can never claim the same
/// stock — `load_stock_balances` relies on that for distinctness.
#[account]
pub struct CapacityStock {
    /// The capacity stock claimed (launch stocks 0/1/2; allocation
    /// class stocks are ids ≥ 3, design §4.1).
    pub resource_id: u16,
    /// Current balance, kernel fixed-point raw (SCALE 1_000_000).
    pub balance: u64,
    /// PDA bump for the `["stock", resource_id]` seeds.
    pub bump: u8,
}

impl CapacityStock {
    /// 8 (discriminator) + 2 + 8 + 1.
    pub const SPACE: usize = 8 + 2 + 8 + 1;
}

/// Treasury state (design §3, §4.2) — the treasury's ledger, created
/// by `initialize_treasury` at zero and moved only by window flows.
/// Sleeve balances are the stored ledger, not recomputed views: window
/// flows populate them with [`split_sleeves`]. The stage flag is
/// deliberately NOT here — it lives on Config, one source of truth.
/// Threshold B is 0 (unset) at init: until a governed path sets it,
/// `launch_stage2` can never commit (D-2, intended state).
#[account]
pub struct TreasuryState {
    /// Total treasury value T, kernel fixed-point raw.
    pub total_raw: u64,
    /// Cash sleeve balance (25% of T at the 50/25/25 split).
    pub cash_raw: u64,
    /// Bonds sleeve balance (50% of T).
    pub bonds_raw: u64,
    /// Arbitrage-bot sleeve balance (25% of T).
    pub arb_raw: u64,
    /// Retained-income float (D-1, timing retention): the unpaid
    /// balance of the annual dividend obligation, earmarked for the
    /// year's remaining tranches, never free surplus.
    pub retained_float_raw: u64,
    /// Stage 2 backing threshold B in parts-per-million of
    /// (supply × price); 0 means unset (D-2 open).
    pub backing_threshold_ppm: u64,
    /// PDA bump for the `["treasury"]` seeds.
    pub bump: u8,
}

impl TreasuryState {
    /// 8 (discriminator) + 6 × 8 + 1.
    pub const SPACE: usize = 8 + 8 + 8 + 8 + 8 + 8 + 8 + 1;
}

/// Lending book (design §3, §4.2) — aggregate only. Individual loans
/// live on HolderRecords (principal + maturity); this account tracks
/// the book total the T/12 cap is judged against (60-day terms at
/// pilot, so outstanding spans 2 cohorts).
#[account]
pub struct LendingBook {
    /// Total outstanding principal, kernel fixed-point raw.
    pub outstanding_raw: u64,
    /// Number of active loans (bookkeeping; the cap judges the total,
    /// not the count).
    pub active_loans: u32,
    /// PDA bump for the `["lending-book"]` seeds.
    pub bump: u8,
}

impl LendingBook {
    /// 8 (discriminator) + 8 + 4 + 1.
    pub const SPACE: usize = 8 + 8 + 4 + 1;
}

/// Fee vault (design §3, §4.2) — the 73 bps single flow's collection
/// ledger (D-5). This is a program-owned data account, not a token
/// account: `collect_fees` reconciles its collected total against
/// the window transition's declared total, and a shortfall is an
/// inexact settle, visible, never a silent gap. Its key must equal
/// `Config.fee_vault` (see `initialize_treasury`).
#[account]
pub struct FeeVault {
    /// Total fees collected, kernel fixed-point raw.
    pub collected_raw: u64,
    /// PDA bump for the `["fee-vault"]` seeds.
    pub bump: u8,
}

impl FeeVault {
    /// 8 (discriminator) + 8 + 1.
    pub const SPACE: usize = 8 + 8 + 1;
}

/// Accounts for [`treasury_program::submit_signal`].
#[derive(Accounts)]
#[instruction(transition_id: u64, round: u32)]
pub struct SubmitSignal<'info> {
    /// The signalling agent's record (must exist and be bound to the
    /// signer; active flag is the eligibility half enforced here).
    #[account(seeds = [b"agent", caller.key().as_ref()], bump = agent_record.bump)]
    pub agent_record: Account<'info, AgentRecord>,
    /// The transition record to signal against (Registered or Planning).
    #[account(mut, seeds = [b"transition", transition_id.to_le_bytes().as_ref()], bump = transition_record.bump)]
    pub transition_record: Account<'info, TransitionRecord>,
    /// The transition type's policy — seeds-bound to the record's
    /// `transition_type`; its `max_rounds` is the cap this instruction
    /// enforces, never a stranger's policy and never a constant.
    #[account(seeds = [b"policy", transition_record.transition_type.as_ref()], bump = policy_record.bump)]
    pub policy_record: Account<'info, PolicyRecord>,
    /// SignalRecord PDA, seeds `["signal", transition_id, round, caller]`.
    /// One-time init: a second signal from the same agent in the same
    /// round fails and changes no state (AlreadySignalled in the
    /// design taxonomy).
    #[account(
        init,
        payer = caller,
        space = SignalRecord::SPACE,
        seeds = [b"signal", transition_id.to_le_bytes().as_ref(), round.to_le_bytes().as_ref(), caller.key().as_ref()],
        bump
    )]
    pub signal_record: Account<'info, SignalRecord>,
    /// The signalling agent (signer; pays rent for the signal record).
    #[account(mut)]
    pub caller: Signer<'info>,
    /// Rent sysvar, required by the `init` constraint.
    pub system_program: Program<'info, System>,
}

/// Accounts for [`treasury_program::advance_round`].
#[derive(Accounts)]
#[instruction(transition_id: u64)]
pub struct AdvanceRound<'info> {
    /// The transition record to advance (must be Registered or
    /// Planning, current round non-empty, room under `max_rounds`).
    /// No new account: the round count, the snapshotted previous-round
    /// mean, and the reset current-round aggregates all live on this
    /// record (design §3).
    #[account(mut, seeds = [b"transition", transition_id.to_le_bytes().as_ref()], bump = transition_record.bump)]
    pub transition_record: Account<'info, TransitionRecord>,
    /// The transition type's policy — seeds-bound to the record's
    /// `transition_type`; its `max_rounds` is the cap this instruction
    /// enforces, never a stranger's policy and never a constant.
    #[account(seeds = [b"policy", transition_record.transition_type.as_ref()], bump = policy_record.bump)]
    pub policy_record: Account<'info, PolicyRecord>,
    /// The caller (signer; pays no rent here). Permissionless, as in
    /// the kernel — the kernel ignores its advance_round caller, and
    /// no AgentRecord is required, so revision timing cannot be gated
    /// by a role the kernel does not gate it by.
    pub caller: Signer<'info>,
}

/// Accounts for [`treasury_program::reconcile`].
#[derive(Accounts)]
#[instruction(transition_id: u64)]
pub struct Reconcile<'info> {
    /// The transition record to move Planning → Committable.
    /// No new account: the state and the `reconciled` flag both live
    /// on this record (design §3).
    #[account(mut, seeds = [b"transition", transition_id.to_le_bytes().as_ref()], bump = transition_record.bump)]
    pub transition_record: Account<'info, TransitionRecord>,
    /// The caller (signer; pays no rent here). Permissionless, as in
    /// the kernel — the kernel ignores its reconcile caller, so no
    /// AgentRecord is required and revision timing cannot be gated
    /// by a role the kernel does not gate it by.
    pub caller: Signer<'info>,
    /// Clock sysvar: the planning deadline is judged on-chain.
    pub clock: Sysvar<'info, Clock>,
}

/// Accounts for [`treasury_program::commit`].
#[derive(Accounts)]
#[instruction(transition_id: u64, observation_id: u64)]
pub struct Commit<'info> {
    /// The transition record to commit (must be Committable).
    #[account(mut, seeds = [b"transition", transition_id.to_le_bytes().as_ref()], bump = transition_record.bump)]
    pub transition_record: Account<'info, TransitionRecord>,
    /// The transition type's policy — seeds-bound to the record's
    /// `transition_type`; its quorum, confidence threshold, minimum
    /// observation confidence, and objectives requirement are the
    /// mask inputs `commit` judges, never a stranger's policy and
    /// never a constant.
    #[account(seeds = [b"policy", transition_record.transition_type.as_ref()], bump = policy_record.bump)]
    pub policy_record: Account<'info, PolicyRecord>,
    /// The manifest's observation — the FRESH bit reads through it.
    /// The instruction argument must match the record's observation
    /// id (checked in the handler); a stranger's observation is never
    /// judged. Seeds bind it to that id, as at registration.
    #[account(seeds = [b"observation", observation_id.to_le_bytes().as_ref()], bump = observation.bump)]
    pub observation: Account<'info, Observation>,
    /// The declared chain head (design §3): the LINEAGE bit is judged
    /// against it, and a clear mask advances it. Created at genesis
    /// by `initialize_config` (index 0, zero hash — the kernel's
    /// genesis declared state); a missing chain account fails
    /// validation, fail-closed. Only `commit` mutates it, and only on
    /// a clear mask.
    #[account(mut, seeds = [b"chain"], bump = declared_chain.bump)]
    pub declared_chain: Account<'info, DeclaredChain>,
    /// The caller (signer; pays no rent here). Permissionless, as in
    /// the kernel — the kernel ignores its commit caller, and no
    /// AgentRecord is required.
    pub caller: Signer<'info>,
    /// Clock sysvar: planning deadline and manifest expiry are judged
    /// on-chain, and `now` feeds the whole mask.
    pub clock: Sysvar<'info, Clock>,
}

/// The declared chain head (design §3 `SystemState`, program-owned).
///
/// The kernel's declared state made persistent: the index and hash the
/// LINEAGE bit is judged against at commit time, advanced by exactly
/// one commit at a time (commits serialize on this account's write
/// lock, as the design states plainly). Genesis is index 0 with the
/// zero hash — the kernel's genesis `SystemState` — and the first
/// commit against it must carry that parent, or fail LINEAGE.
#[account]
pub struct DeclaredChain {
    /// Commits landed so far (genesis 0).
    pub index: u64,
    /// Head hash (genesis zero hash).
    pub hash: [u8; 32],
    /// PDA bump for the `["chain"]` seeds.
    pub bump: u8,
}

impl DeclaredChain {
    /// 8 (discriminator) + 8 + 32 + 1.
    pub const SPACE: usize = 8 + 8 + 32 + 1;
}

/// Accounts for [`treasury_program::record_evidence`].
#[derive(Accounts)]
#[instruction(transition_id: u64)]
pub struct RecordEvidence<'info> {
    /// The recording Oracle's agent record (must exist, be active, and
    /// hold the Oracle role). Seeds bind it to the signer.
    #[account(seeds = [b"agent", oracle.key().as_ref()], bump = agent_record.bump)]
    pub agent_record: Account<'info, AgentRecord>,
    /// The transition record to observe (must be Committed). Read,
    /// not written: recording evidence changes no transition state —
    /// the EvidenceRecord below is the only write.
    #[account(seeds = [b"transition", transition_id.to_le_bytes().as_ref()], bump = transition_record.bump)]
    pub transition_record: Account<'info, TransitionRecord>,
    /// EvidenceRecord PDA, seeds `["evidence", transition_id]`.
    /// One-time init: a second record for the same transition fails
    /// and changes no state (Duplicate in the design taxonomy).
    #[account(
        init,
        payer = oracle,
        space = EvidenceRecord::SPACE,
        seeds = [b"evidence", transition_id.to_le_bytes().as_ref()],
        bump
    )]
    pub evidence_record: Account<'info, EvidenceRecord>,
    /// The recording Oracle (signer; pays rent for the evidence record).
    #[account(mut)]
    pub oracle: Signer<'info>,
    /// Rent sysvar, required by the `init` constraint.
    pub system_program: Program<'info, System>,
}

/// One committed transition's evidence record (design §3
/// `EvidenceRecord`).
///
/// The EvidenceAttestor's on-chain form: `transition_id` → realized
/// hash, written by `record_evidence` (Oracle role) only after the
/// realized outcome exists. `settle` refuses unless the presented hash
/// equals this record — no record, no settle. The record is what makes
/// settle-after-realized mechanical at the program layer: the kernel
/// itself cannot hold the log (its EvidenceAttestor is std-only), so
/// the log lives here, in an account, instead of in a hook.
#[account]
pub struct EvidenceRecord {
    /// The transition this evidence observes (PDA seed component).
    pub transition_id: u64,
    /// The realized hash the outcome produced.
    pub realized_hash: [u8; 32],
    /// PDA bump for the `["evidence", transition_id]` seeds.
    pub bump: u8,
}

impl EvidenceRecord {
    /// 8 (discriminator) + 8 + 32 + 1.
    pub const SPACE: usize = 8 + 8 + 32 + 1;
}

/// Accounts for [`treasury_program::settle`].
#[derive(Accounts)]
#[instruction(transition_id: u64)]
pub struct Settle<'info> {
    /// The settling Oracle's agent record (must exist, be active, and
    /// hold the Oracle role). Seeds bind it to the signer.
    #[account(seeds = [b"agent", oracle.key().as_ref()], bump = agent_record.bump)]
    pub agent_record: Account<'info, AgentRecord>,
    /// The transition record to settle (must be Committed). Written:
    /// state moves to Settled and the realized hash is recorded here.
    #[account(mut, seeds = [b"transition", transition_id.to_le_bytes().as_ref()], bump = transition_record.bump)]
    pub transition_record: Account<'info, TransitionRecord>,
    /// The independently recorded evidence (must exist — a missing
    /// record fails validation before the handler runs, still a
    /// refusal that changes no state). The presented hash must equal
    /// this record's hash; the record is read, never written, here.
    #[account(seeds = [b"evidence", transition_id.to_le_bytes().as_ref()], bump = evidence_record.bump)]
    pub evidence_record: Account<'info, EvidenceRecord>,
    /// The settling Oracle (signer; pays no rent here — the
    /// attributed agents' records already exist).
    pub oracle: Signer<'info>,
}

/// Accounts for [`treasury_program::submit_objectives_check`].
#[derive(Accounts)]
#[instruction(transition_id: u64)]
pub struct SubmitObjectivesCheck<'info> {
    /// The reporting agent's record (must exist and be bound to the
    /// signer; the Evaluator role is the gate enforced here).
    #[account(seeds = [b"agent", caller.key().as_ref()], bump = agent_record.bump)]
    pub agent_record: Account<'info, AgentRecord>,
    /// The transition record to check against (Registered or
    /// Planning). No new account: the verdict overwrites the flag on
    /// this record in place (design §3).
    #[account(mut, seeds = [b"transition", transition_id.to_le_bytes().as_ref()], bump = transition_record.bump)]
    pub transition_record: Account<'info, TransitionRecord>,
    /// The reporting Evaluator (signer; pays no rent here).
    pub caller: Signer<'info>,
}

/// Accounts for [`treasury_program::raise_exception`].
#[derive(Accounts)]
#[instruction(transition_id: u64)]
pub struct RaiseException<'info> {
    /// The raising agent's record (must exist and be bound to the
    /// signer; the Evaluator role is the gate enforced here).
    #[account(seeds = [b"agent", caller.key().as_ref()], bump = agent_record.bump)]
    pub agent_record: Account<'info, AgentRecord>,
    /// The transition record to raise against (Registered, Planning,
    /// or Committable). No new account: the exception mutates this
    /// record's per-transition set in place (design §3).
    #[account(mut, seeds = [b"transition", transition_id.to_le_bytes().as_ref()], bump = transition_record.bump)]
    pub transition_record: Account<'info, TransitionRecord>,
    /// The raising Evaluator (signer; pays no rent here).
    pub caller: Signer<'info>,
}

/// Accounts for [`treasury_program::resolve_exception`].
#[derive(Accounts)]
#[instruction(transition_id: u64)]
pub struct ResolveException<'info> {
    /// The resolving agent's record (must exist and be bound to the
    /// signer; the Evaluator role is the gate enforced here).
    #[account(seeds = [b"agent", caller.key().as_ref()], bump = agent_record.bump)]
    pub agent_record: Account<'info, AgentRecord>,
    /// The transition record to resolve against. No state gate is
    /// imposed here (the kernel imposes none on resolve) — the
    /// record is written in place, its exception entry marked
    /// resolved and its blocking count decremented on first
    /// resolution only.
    #[account(mut, seeds = [b"transition", transition_id.to_le_bytes().as_ref()], bump = transition_record.bump)]
    pub transition_record: Account<'info, TransitionRecord>,
    /// The resolving Evaluator (signer; pays no rent here).
    pub caller: Signer<'info>,
}

/// Accounts for [`treasury_program::reject_transition`].
#[derive(Accounts)]
#[instruction(transition_id: u64)]
pub struct RejectTransition<'info> {
    /// The vetoing agent's record (must exist and be bound to the
    /// signer; the Evaluator-or-Admin gate is enforced here).
    #[account(seeds = [b"agent", caller.key().as_ref()], bump = agent_record.bump)]
    pub agent_record: Account<'info, AgentRecord>,
    /// The transition record to veto (Registered, Planning, or
    /// Committable). Written in place to Rejected; no new account.
    #[account(mut, seeds = [b"transition", transition_id.to_le_bytes().as_ref()], bump = transition_record.bump)]
    pub transition_record: Account<'info, TransitionRecord>,
    /// The vetoing agent (signer; pays no rent here).
    pub caller: Signer<'info>,
}

/// Accounts for [`treasury_program::expire_transition`].
#[derive(Accounts)]
#[instruction(transition_id: u64)]
pub struct ExpireTransition<'info> {
    /// The transition record to expire (Registered, Planning, or
    /// Committable, past its manifest expiry). Written in place to
    /// Expired; no new account.
    #[account(mut, seeds = [b"transition", transition_id.to_le_bytes().as_ref()], bump = transition_record.bump)]
    pub transition_record: Account<'info, TransitionRecord>,
    /// The caller (signer; pays no rent here). Permissionless, as in
    /// the kernel — the kernel checks no role for expire, so no
    /// AgentRecord is required.
    pub caller: Signer<'info>,
    /// Clock sysvar: expiry is judged on-chain, never from a client
    /// timestamp (design §4.1, the EXPIRED bit's anchor).
    pub clock: Sysvar<'info, Clock>,
}

/// Accounts for [`treasury_program::policy_transition`].
#[derive(Accounts)]
#[instruction(transition_type: [u8; 32])]
pub struct PolicyTransition<'info> {
    /// Program configuration PDA — read to check the authority.
    #[account(seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, Config>,
    /// Policy record PDA, seeds `["policy", transition_type]`.
    /// One-time init: installing a type twice fails at account
    /// creation and changes no state.
    #[account(
        init,
        payer = authority,
        space = PolicyRecord::SPACE,
        seeds = [b"policy", transition_type.as_ref()],
        bump
    )]
    pub policy_record: Account<'info, PolicyRecord>,
    /// Genesis authority (the governance multisig at genesis).
    #[account(mut)]
    pub authority: Signer<'info>,
    /// Rent sysvar, required by the `init` constraint.
    pub system_program: Program<'info, System>,
}

/// Accounts for [`treasury_program::apply_policy_amendment`].
#[derive(Accounts)]
#[instruction(transition_id: u64, target_type: [u8; 32])]
pub struct ApplyPolicyAmendment<'info> {
    /// The settled policy transition authorizing the amendment
    /// (mut only for the one-time execution flag; its state, type,
    /// and hashes are read, never rewritten, by this instruction).
    #[account(mut, seeds = [b"transition", transition_id.to_le_bytes().as_ref()], bump = transition_record.bump)]
    pub transition_record: Account<'info, TransitionRecord>,
    /// The target type's policy record (mut: its values are
    /// overwritten by the amendment; its PDA seeds bind it to the
    /// target type, and its stored type is checked too).
    #[account(mut, seeds = [b"policy", target_type.as_ref()], bump = policy_record.bump)]
    pub policy_record: Account<'info, PolicyRecord>,
    /// Permissionless executor (signer; pays no rent — no account is
    /// created here). The authority is the settled transition, not
    /// this key.
    pub executor: Signer<'info>,
}

/// Accounts for [`treasury_program::initialize_treasury`].
#[derive(Accounts)]
#[instruction(transition_id: u64)]
pub struct InitializeTreasury<'info> {
    /// Program configuration PDA — read for the FeeVault binding
    /// (`fee_vault` must equal the created vault's key).
    #[account(seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, Config>,
    /// The Settled `treasury.init.v1` transition authorizing the
    /// init (read-only: no flag is written — the `init` constraints
    /// below are the one-time guarantee).
    #[account(seeds = [b"transition", transition_id.to_le_bytes().as_ref()], bump = transition_record.bump)]
    pub transition_record: Account<'info, TransitionRecord>,
    /// Treasury state PDA, seeds `["treasury"]`. One-time init.
    #[account(
        init,
        payer = executor,
        space = TreasuryState::SPACE,
        seeds = [b"treasury"],
        bump
    )]
    pub treasury_state: Account<'info, TreasuryState>,
    /// Lending book PDA, seeds `["lending-book"]`. One-time init.
    #[account(
        init,
        payer = executor,
        space = LendingBook::SPACE,
        seeds = [b"lending-book"],
        bump
    )]
    pub lending_book: Account<'info, LendingBook>,
    /// Fee vault PDA, seeds `["fee-vault"]`. One-time init; the
    /// handler requires its key to equal `config.fee_vault`.
    #[account(
        init,
        payer = executor,
        space = FeeVault::SPACE,
        seeds = [b"fee-vault"],
        bump
    )]
    pub fee_vault: Account<'info, FeeVault>,
    /// Permissionless executor (signer; pays rent for the three new
    /// accounts). The authority is the settled transition, not this
    /// key.
    #[account(mut)]
    pub executor: Signer<'info>,
    /// Rent sysvar, required by the `init` constraints.
    pub system_program: Program<'info, System>,
}

/// Accounts for [`treasury_program::launch_stage2`].
#[derive(Accounts)]
#[instruction(transition_id: u64)]
pub struct LaunchStage2<'info> {
    /// Program configuration PDA — mutated: the stage flag flips
    /// Stage 1 → Stage 2 here and nowhere else (TreasuryState does
    /// not duplicate it; no other instruction writes it).
    #[account(mut, seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, Config>,
    /// Treasury state PDA — read for the backing threshold B
    /// (D-2): launch refuses while it is unset (0). The account
    /// must already exist, so `initialize_treasury` necessarily ran
    /// first; there is no launch before a treasury.
    #[account(seeds = [b"treasury"], bump = treasury_state.bump)]
    pub treasury_state: Account<'info, TreasuryState>,
    /// The Settled `treasury.launch.v1` transition authorizing the
    /// launch (read-only: no flag is written — the stage flip is
    /// the one-time guarantee).
    #[account(seeds = [b"transition", transition_id.to_le_bytes().as_ref()], bump = transition_record.bump)]
    pub transition_record: Account<'info, TransitionRecord>,
    /// Permissionless executor (signer; pays no rent — no account is
    /// created here). The authority is the settled transition, not
    /// this key.
    pub executor: Signer<'info>,
}

/// Accounts for [`treasury_program::set_capacity`].
#[derive(Accounts)]
#[instruction(resource_id: u16)]
pub struct SetCapacity<'info> {
    /// Program configuration PDA — read to check the authority.
    #[account(seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, Config>,
    /// Capacity stock PDA, seeds `["stock", resource_id]`.
    /// One-time init: installing a resource twice fails at account
    /// creation and changes no state (Duplicate in the design
    /// taxonomy); the balance moves afterwards only by `commit`'s
    /// atomic debit and by committed-flow replenishment.
    #[account(
        init,
        payer = authority,
        space = CapacityStock::SPACE,
        seeds = [b"stock", resource_id.to_le_bytes().as_ref()],
        bump
    )]
    pub stock: Account<'info, CapacityStock>,
    /// Genesis authority (the governance multisig at genesis).
    #[account(mut)]
    pub authority: Signer<'info>,
    /// Rent sysvar, required by the `init` constraint.
    pub system_program: Program<'info, System>,
}

/// Accounts for [`treasury_program::submit_constraint_report`].
#[derive(Accounts)]
#[instruction(transition_id: u64)]
pub struct SubmitConstraintReport<'info> {
    /// The reporting agent's record (must exist and be bound to the
    /// signer; the Evaluator role is the gate enforced here).
    #[account(seeds = [b"agent", caller.key().as_ref()], bump = agent_record.bump)]
    pub agent_record: Account<'info, AgentRecord>,
    /// The transition record to report against (Registered or
    /// Planning). No new account: the report mutates this record's
    /// frozen constraint set in place (design §3).
    #[account(mut, seeds = [b"transition", transition_id.to_le_bytes().as_ref()], bump = transition_record.bump)]
    pub transition_record: Account<'info, TransitionRecord>,
    /// The reporting Evaluator (signer; pays no rent here).
    pub caller: Signer<'info>,
}

/// One agent's signal in one round of a transition (design §3
/// `Round / Signal`, per-agent leg).
///
/// The kernel's round aggregates (total, weighted) are computed at
/// commit replay from these records — they are not stored here, so no
/// client-supplied total can be trusted. The monotone-revision mean
/// likewise comes from replay, not from this record.
#[account]
pub struct SignalRecord {
    /// The transition this signal belongs to (PDA seed component).
    pub transition_id: u64,
    /// The round signalled (PDA seed component).
    pub round: u32,
    /// The signalling agent's key (PDA seed component).
    pub agent: Pubkey,
    /// The raw weight supplied (kernel `Amount` raw, SCALE 1_000_000).
    pub raw: u64,
    /// Support, kernel `Amount` raw, at most [`SUPPORT_ONE_RAW`].
    pub support: u64,
    /// PDA bump for the `["signal", transition_id, round, agent]` seeds.
    pub bump: u8,
}

impl SignalRecord {
    /// 8 (discriminator) + 8 + 4 + 32 + 8 + 8 + 1.
    pub const SPACE: usize = 8 + 8 + 4 + 32 + 8 + 8 + 1;
}

/// Program errors (design §4 taxonomy, first entries).
#[error_code]
pub enum TreasuryError {
    /// The caller is not the `Config` authority.
    #[msg("Caller is not the program authority.")]
    Unauthorized,
    /// The record's agent key does not match the instruction's agent.
    #[msg("Agent record does not match the instruction agent.")]
    AgentMismatch,
    /// Role code outside 0 (Admin), 1 (Oracle), 2 (Evaluator).
    #[msg("Unknown role code: 0 = Admin, 1 = Oracle, 2 = Evaluator.")]
    InvalidRole,
    /// More than MAX_GROUPS groups supplied at registration.
    #[msg("Too many groups: at most 8 per agent.")]
    InvalidGroups,
    /// Reputation outside 0–100 units.
    #[msg("Reputation out of bounds: 0 to 100 units.")]
    InvalidReputation,
    /// Observation timestamp is after on-chain time — future-dated.
    #[msg("Observation timestamp is in the future.")]
    ObservationFromFuture,
    /// The window is not a window: expiry at/before timestamp, or a
    /// manifest already expired at registration.
    #[msg("Bad window: expiry must be after the timestamp / in the future.")]
    BadWindow,
    /// The transition is not in the state this instruction requires
    /// (e.g. opening planning on a non-Registered transition).
    #[msg("Transition is not in the required state.")]
    BadState,
    /// Checked arithmetic overflowed (e.g. a planning deadline beyond
    /// the timestamp range). An overflow is an error, never a wrap.
    #[msg("Arithmetic overflow.")]
    Overflow,
    /// Planning is not open: the transition is not Planning, or `now`
    /// has reached the planning deadline (kernel `PlanningClosed`).
    #[msg("Planning is closed: transition not Planning or deadline reached.")]
    PlanningClosed,
    /// The agent is not eligible to plan (e.g. inactive). The
    /// group-overlap half of kernel eligibility awaits `relevant_groups`
    /// on the record (see `submit_plan` docs).
    #[msg("Agent is not eligible to submit a plan.")]
    NotEligible,
    /// The transition already holds `max_plans` plans (4 at launch).
    #[msg("Too many plans for this transition.")]
    TooManyPlans,
    /// Claims break the account/keyed-map shape: more than MAX_CLAIMS
    /// entries, or a resource id claimed twice.
    #[msg("Invalid claims: too many, or a resource claimed twice.")]
    InvalidClaims,
    /// Signal support above kernel `Amount::ONE` (1_000_000 raw).
    #[msg("Signal support out of range: at most 1.0.")]
    SupportOutOfRange,
    /// The round index is at or beyond `max_rounds` (3 at launch).
    #[msg("Rounds exhausted: round index at or beyond max_rounds.")]
    RoundsExhausted,
    /// A re-signal moved farther from the previous round's weighted
    /// mean than the agent's previous signal — revision must move
    /// toward the mean, never away from it.
    #[msg("Monotone revision violated: re-signal moved away from the previous round's mean.")]
    MonotoneRevision,
    /// The constraint id is not a member of the transition's frozen
    /// set — a report cannot invent a constraint (design §4.1).
    #[msg("Unknown constraint: not a member of the transition's frozen set.")]
    UnknownConstraint,
    /// The status code is not a kernel `ConstraintStatus` (0 = Pass,
    /// 1 = Fail, 2 = Unavailable).
    #[msg("Invalid constraint status: 0 = Pass, 1 = Fail, 2 = Unavailable.")]
    InvalidConstraintStatus,
    /// The frozen set breaks its shape: more than MAX_CONSTRAINTS
    /// entries, or a constraint id supplied twice.
    #[msg("Invalid constraint set: too many, or a constraint id twice.")]
    InvalidConstraints,
    /// The current round is empty: its effective-weight total is zero
    /// — no signals, or only zero-weight signals. Advancing an empty
    /// round would create a round with nothing to revise from.
    #[msg("Current round is empty: effective-weight total is zero.")]
    RoundEmpty,
    /// Planning is still open: `now` has not reached the planning
    /// deadline, so the transition cannot be reconciled yet (kernel
    /// `PlanningStillOpen`). At the deadline itself it passes.
    #[msg("Planning is still open: now has not reached the planning deadline.")]
    PlanningStillOpen,
    /// The commit predicate refused: the 13-bit mask is non-zero.
    /// Anchor error codes are unit variants, so the mask itself is not
    /// carried here — it is logged with `msg!` by the `commit`
    /// handler and is recomputable from the public account state.
    /// A refused commit changes no state and emits no event.
    #[msg("Commit refused: commit_fail_mask is non-zero (see program logs).")]
    CommitFailed,
    /// The observation id passed to `commit` does not match the
    /// transition record's manifest observation — the FRESH bit is
    /// never judged through a stranger's observation.
    #[msg("Observation id does not match the transition's manifest observation.")]
    ObservationMismatch,
    /// A remaining-accounts plan does not belong to this commit: the
    /// count differs from `plan_count`, an account is not
    /// program-owned, names another transition, or is listed twice.
    /// The mask is never computed over a partial plan set.
    #[msg("Remaining plan accounts do not match this transition's plan set.")]
    PlanMismatch,
    /// A settle attribution names an agent with no AgentRecord passed
    /// among the remaining accounts — the kernel's `UnknownAgent`.
    /// The attribution names nobody the program can write to.
    #[msg("Settle attribution names an agent with no agent record passed.")]
    UnknownAgent,
    /// A settle attribution names an agent with no plan for this
    /// transition — the kernel's `NotParticipant`. Only plan authors
    /// earn reputation from a settlement.
    #[msg("Settle attribution names an agent with no plan for this transition.")]
    NotParticipant,
    /// A settle remaining account does not belong to this settle: an
    /// agent record that is not program-owned, not writable, listed
    /// twice, or outside the attributed set, or an account whose
    /// discriminator is neither AgentRecord nor PlanRecord. The
    /// agent set must match the attributed set exactly.
    #[msg("Remaining settle accounts do not match this settle's agent set.")]
    SettleAccountsMismatch,
    /// The exception id is already raised on this transition — the
    /// kernel's `Duplicate`. A raised exception is never raised
    /// twice; resolve it, or raise a different id.
    #[msg("Exception already raised on this transition.")]
    DuplicateException,
    /// The exception id was never raised on this transition — the
    /// kernel's `UnknownException`. Only a raised exception can
    /// resolve.
    #[msg("Unknown exception: not raised on this transition.")]
    UnknownException,
    /// The disposition code is not a kernel `Disposition` (0 =
    /// Informational, 1 = Review, 2 = Block, 3 = Emergency).
    #[msg("Invalid disposition: 0 = Informational, 1 = Review, 2 = Block, 3 = Emergency.")]
    InvalidDisposition,
    /// The transition already holds `MAX_EXCEPTIONS` exceptions (8).
    #[msg("Too many exceptions: at most 8 per transition.")]
    TooManyExceptions,
    /// The required-capability set breaks its shape: more than
    /// MAX_REQUIRED_CAPS entries, or a capability id twice.
    #[msg("Invalid required capabilities: too many, or a capability id twice.")]
    InvalidCapabilities,
    /// The agent already holds `MAX_CAPABILITIES` distinct
    /// capabilities (8) — a ninth distinct grant is refused, while a
    /// duplicate grant stays a no-op success.
    #[msg("Too many capabilities: at most 8 per agent.")]
    TooManyCapabilities,
    /// A commit remaining account does not belong to this commit's
    /// author set: the author-record count differs from the distinct
    /// plan-author count, an account is not program-owned, does not
    /// deserialize as an AgentRecord, names a non-author, or is
    /// listed twice. The CAPABILITY bit is never computed over a
    /// partial author set.
    #[msg("Remaining author accounts do not match this commit's plan-author set.")]
    CommitAccountsMismatch,
    /// A remaining-accounts stock does not belong to this
    /// reconcile/commit's stock set: an account is not program-owned,
    /// does not deserialize as a CapacityStock, is listed twice, is
    /// not writable at commit (a clear mask must debit it), or does
    /// not sit at the PDA of its stored resource id. The COMPAT bit
    /// is never judged over partial stocks.
    #[msg("Remaining stock accounts do not match this transition's stock set.")]
    StockMismatch,
    /// A policy amendment does not match its authorizing transition:
    /// the source transition's type is not the canonical
    /// `treasury.policy.v1` type, the target record's stored type is
    /// not the target type, the payload hash does not equal the
    /// declared hash, or the settle was not exact (realized differs
    /// from declared). The amendment is never applied on a partial
    /// binding.
    #[msg("Policy amendment does not match its settled policy transition.")]
    AmendmentMismatch,
    /// A treasury init does not match its authorizing transition:
    /// the source transition's type is not the canonical
    /// `treasury.init.v1` type. The init is never executed on a
    /// transition of another type.
    #[msg("Treasury init does not match a settled treasury.init.v1 transition.")]
    InitMismatch,
    /// The FeeVault account created at treasury init does not
    /// key-equal `Config.fee_vault`. The genesis authority must
    /// pass the `["fee-vault"]` PDA as `fee_vault` at
    /// `initialize_config`; a wrong genesis value refuses init
    /// visibly instead of running with two different "vaults".
    #[msg("FeeVault account does not match the Config fee_vault.")]
    VaultMismatch,
    /// A stage 2 launch does not match its authorizing transition:
    /// the source transition's type is not the canonical
    /// `treasury.launch.v1` type. The launch is never executed on a
    /// transition of another type.
    #[msg("Stage 2 launch does not match a settled treasury.launch.v1 transition.")]
    LaunchMismatch,
    /// The stage flag is no longer Stage 1: Stage 2 has already
    /// launched. A second launch, or a replay of the first, changes
    /// no state.
    #[msg("Stage 2 is already launched: the stage flag is no longer Stage 1.")]
    AlreadyLaunched,
    /// The Stage 2 backing threshold B is unset (0, D-2 open): no
    /// governed path has set it yet, so the launch cannot complete.
    /// An unset threshold must never read as "any backing passes".
    #[msg("Stage 2 backing threshold B is unset (D-2 open): no governed path has set it yet.")]
    ThresholdUnset,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fee_stream_is_the_single_combined_flow() {
        // D-5: 60 bps transaction + 13 bps IDL on the SAME flow.
        assert_eq!(FEE_PPM, 6_000 + 1_300);
    }

    #[test]
    fn movable_liquidity_is_one_sixth_of_treasury() {
        // D-10, ratio-exact at the 25% sleeve: 2/3 x 25% = 1/6.
        assert_eq!(movable_liquidity(100_000_000), 16_666_666);
        assert_eq!(max_outstanding(100_000_000), 8_333_333);
        assert_eq!(movable_liquidity(100_000_000), 2 * max_outstanding(100_000_000));
    }

    #[test]
    fn config_space_matches_fields() {
        assert_eq!(Config::SPACE, 74);
    }

    #[test]
    fn agent_record_space_matches_fields() {
        assert_eq!(AgentRecord::SPACE, 341);
        assert_eq!(MAX_GROUPS, 8);
        assert_eq!(MAX_CAPABILITIES, 8);
        assert_eq!(REP_MAX_RAW, 100_000_000);
    }

    #[test]
    fn role_codes_follow_kernel_role_order() {
        // Kernel Role enum order: Admin, Oracle, Evaluator.
        let mut record = AgentRecord {
            agent: Pubkey::default(),
            active: true,
            is_admin: false,
            is_oracle: false,
            is_evaluator: false,
            influence_raw: 1_000_000,
            reputation_raw: 50_000_000,
            groups: vec![1],
            capabilities: vec![],
            bump: 0,
        };
        record.apply_role(1, true).expect("oracle grant lands");
        assert!(record.is_oracle);
        assert!(!record.is_admin && !record.is_evaluator);
        record.apply_role(0, true).expect("admin grant lands");
        record.apply_role(1, false).expect("oracle revoke lands");
        assert!(record.is_admin && !record.is_oracle);
        // Revoking an unheld role is a no-op success.
        record.apply_role(2, false).expect("unheld revoke is fine");
        assert!(!record.is_evaluator);
        // An unknown code changes nothing and errors.
        assert!(record.apply_role(3, true).is_err());
        assert!(record.is_admin && !record.is_oracle && !record.is_evaluator);
    }

    #[test]
    fn capability_grant_and_revoke_follow_the_kernel_set() {
        // Kernel `grant_capability` is a set insert: idempotent, and
        // the program bound is the only addition (8 distinct).
        let mut record = AgentRecord {
            agent: Pubkey::default(),
            active: true,
            is_admin: false,
            is_oracle: false,
            is_evaluator: false,
            influence_raw: 1_000_000,
            reputation_raw: 50_000_000,
            groups: vec![],
            capabilities: vec![],
            bump: 0,
        };
        let cap_a = [1u8; 32];
        record.grant_capability(cap_a).expect("first grant lands");
        assert_eq!(record.capabilities.len(), 1);
        // A duplicate grant is a no-op success, not a second entry.
        record.grant_capability(cap_a).expect("duplicate grant is fine");
        assert_eq!(record.capabilities.len(), 1);
        // Fill to the bound, then a ninth distinct grant refuses —
        // but a duplicate at a full set still succeeds.
        for i in 2u8..=8 {
            record
                .grant_capability([i; 32])
                .expect("grant under the bound lands");
        }
        assert_eq!(record.capabilities.len(), 8);
        assert!(record.grant_capability([9u8; 32]).is_err());
        record.grant_capability(cap_a).expect("duplicate at full set is fine");
        assert_eq!(record.capabilities.len(), 8);
        // Revoke removes, and revoking an unheld capability is a
        // no-op success, mirroring the role-revoke shape.
        record.revoke_capability(cap_a);
        assert!(!record.capabilities.contains(&cap_a));
        assert_eq!(record.capabilities.len(), 7);
        record.revoke_capability(cap_a);
        assert_eq!(record.capabilities.len(), 7);
    }

    #[test]
    fn capability_covered_sums_plan_authors_only() {
        // Kernel `capability_covered`: summed influence of plan
        // authors holding the capability, per requirement, all
        // requirements covered — checked addition, overflow fails.
        let cap_a = [1u8; 32];
        let cap_b = [2u8; 32];
        let req = |id: [u8; 32], min: u64| CapabilityRequirement {
            id,
            min_weight: min,
        };
        // No requirements: covered, even with no authors.
        assert!(capability_covered(&[], &[]));
        // Two authors holding A sum past the minimum.
        let authors: Vec<(u64, Vec<[u8; 32]>)> = vec![
            (1_000_000, vec![cap_a]),
            (600_000, vec![cap_a]),
        ];
        assert!(capability_covered(&[req(cap_a, 1_500_000)], &authors));
        // Exactly the minimum still covers (boundary, not strict).
        assert!(capability_covered(&[req(cap_a, 1_600_000)], &authors));
        // One above the minimum fails: influence held by an author
        // without the capability counts for nothing.
        let authors: Vec<(u64, Vec<[u8; 32]>)> = vec![
            (1_000_000, vec![cap_a]),
            (600_000, vec![cap_b]),
        ];
        assert!(!capability_covered(&[req(cap_a, 1_500_000)], &authors));
        // All requirements must cover: A covered, B not.
        let authors: Vec<(u64, Vec<[u8; 32]>)> =
            vec![(2_000_000, vec![cap_a])];
        assert!(!capability_covered(
            &[req(cap_a, 1_000_000), req(cap_b, 1)],
            &authors
        ));
        // An overflowing sum fails the requirement, never wraps
        // into coverage — the kernel's `Err → false`.
        let authors: Vec<(u64, Vec<[u8; 32]>)> =
            vec![(u64::MAX, vec![cap_a]), (1, vec![cap_a])];
        assert!(!capability_covered(&[req(cap_a, 1)], &authors));
    }

    #[test]
    fn observation_and_transition_spaces_match_fields() {
        assert_eq!(Observation::SPACE, 105);
        // 407: the proposer key (32) joined with open_planning, the
        // plan count (4) with submit_plan, the round count (4) with
        // submit_signal, the frozen constraint set (4 + 8 x 33)
        // with submit_constraint_report, the objectives flag (1)
        // with submit_objectives_check, and the round aggregates
        // (8 + 8) plus the previous-round mean (8) with advance_round,
        // the reconciled flag (1) with reconcile, and the lineage
        // inputs (8 + 32) with commit, and the commit index (8) plus
        // the realized hash (32) with settle, and the exception set
        // (4 + 8 x 34) plus the unresolved-blocking count (4) with
        // raise_exception / resolve_exception, and the transition
        // type (32) with policy_transition, and the one-time
        // policy-amendment execution flag (1) with
        // apply_policy_amendment.
        assert_eq!(TransitionRecord::SPACE, 801);
        assert_eq!(DeclaredChain::SPACE, 49);
        assert_eq!(MAX_CONSTRAINTS, 8);
        assert_eq!(MAX_EXCEPTIONS, 8);
        assert_eq!(STATUS_PASS, 0);
        assert_eq!(STATUS_FAIL, 1);
        assert_eq!(STATUS_UNAVAILABLE, 2);
        assert_eq!(DISPOSITION_INFORMATIONAL, 0);
        assert_eq!(DISPOSITION_REVIEW, 1);
        assert_eq!(DISPOSITION_BLOCK, 2);
        assert_eq!(DISPOSITION_EMERGENCY, 3);
    }

    #[test]
    fn raise_exception_gate_matrix() {
        // Kernel check order: Evaluator, pre-commit state, valid
        // disposition, not already raised, room under MAX_EXCEPTIONS.
        let base = |evaluator: bool, state: u8, disp: u8, raised: bool, count: usize| {
            can_raise_exception(evaluator, state, disp, raised, count)
        };
        // Clean cases pass in all three pre-commit states, for
        // blocking and non-blocking dispositions alike.
        assert!(base(true, STATE_REGISTERED, DISPOSITION_INFORMATIONAL, false, 0).is_ok());
        assert!(base(true, STATE_PLANNING, DISPOSITION_BLOCK, false, 3).is_ok());
        assert!(base(true, STATE_COMMITTABLE, DISPOSITION_EMERGENCY, false, 7).is_ok());
        // A non-Evaluator is refused first, whatever else holds.
        assert!(base(false, STATE_REGISTERED, DISPOSITION_BLOCK, false, 0).is_err());
        // Committed, Settled, Rejected, Expired cannot gain an exception.
        assert!(base(true, STATE_COMMITTED, DISPOSITION_BLOCK, false, 0).is_err());
        assert!(base(true, STATE_SETTLED, DISPOSITION_BLOCK, false, 0).is_err());
        assert!(base(true, STATE_REJECTED, DISPOSITION_BLOCK, false, 0).is_err());
        assert!(base(true, STATE_EXPIRED, DISPOSITION_BLOCK, false, 0).is_err());
        // A code outside 0..=3 is not a disposition.
        assert!(base(true, STATE_REGISTERED, 4, false, 0).is_err());
        // An id already raised is a duplicate, never a new entry.
        assert!(base(true, STATE_REGISTERED, DISPOSITION_BLOCK, true, 0).is_err());
        // A full set refuses a ninth exception.
        assert!(base(true, STATE_REGISTERED, DISPOSITION_BLOCK, false, 8).is_err());
        // The blocking table follows the kernel's `d >= Block`.
        assert!(!is_blocking_disposition(DISPOSITION_INFORMATIONAL));
        assert!(!is_blocking_disposition(DISPOSITION_REVIEW));
        assert!(is_blocking_disposition(DISPOSITION_BLOCK));
        assert!(is_blocking_disposition(DISPOSITION_EMERGENCY));
    }

    #[test]
    fn resolve_exception_gate_matrix() {
        // Kernel resolve: Evaluator + existence, and deliberately
        // no state gate — the helper takes no state argument, so a
        // caller cannot smuggle one in later.
        assert!(can_resolve_exception(true, true).is_ok());
        // A non-Evaluator is refused first.
        assert!(can_resolve_exception(false, true).is_err());
        // An id never raised is unknown, never resolved.
        assert!(can_resolve_exception(true, false).is_err());
    }

    #[test]
    fn reject_transition_gate_matrix() {
        // Kernel check order: Evaluator-or-Admin, then pre-commit
        // state. Caller binding is structural (AgentRecord match in
        // the handler), so the helper takes no caller argument.
        let base =
            |evaluator: bool, admin: bool, state: u8| can_reject_transition(evaluator, admin, state);
        // Clean cases pass in all three pre-commit states, for
        // either vetoing role.
        assert!(base(true, false, STATE_REGISTERED).is_ok());
        assert!(base(false, true, STATE_PLANNING).is_ok());
        assert!(base(true, true, STATE_COMMITTABLE).is_ok());
        // A caller holding neither role is refused first, whatever
        // the state.
        assert!(base(false, false, STATE_REGISTERED).is_err());
        assert!(base(false, false, STATE_COMMITTED).is_err());
        // Committed, Settled, Rejected, Expired cannot be vetoed —
        // rejection is terminal, not idempotent.
        assert!(base(true, false, STATE_COMMITTED).is_err());
        assert!(base(false, true, STATE_SETTLED).is_err());
        assert!(base(true, false, STATE_REJECTED).is_err());
        assert!(base(false, true, STATE_EXPIRED).is_err());
    }

    #[test]
    fn expire_transition_gate_matrix() {
        // Kernel check order: expiry first, then pre-commit state.
        // No caller argument: expire is permissionless in the kernel.
        let base =
            |state: u8, now: i64, expires_at: i64| can_expire_transition(state, now, expires_at);
        // Clean cases pass in all three pre-commit states, at and
        // past expiry (the boundary itself expires: `now >=
        // expires_at`, as in the kernel's `now < expires_at` refuse).
        assert!(base(STATE_REGISTERED, 100, 100).is_ok());
        assert!(base(STATE_PLANNING, 101, 100).is_ok());
        assert!(base(STATE_COMMITTABLE, 200, 100).is_ok());
        // Not yet expired refuses with the window, whatever the
        // state — even a Committed transition reports BadWindow
        // before BadState, as in the kernel.
        assert!(base(STATE_REGISTERED, 99, 100).is_err());
        assert!(base(STATE_COMMITTED, 99, 100).is_err());
        // Past expiry, a terminal or committed state still refuses —
        // expiry never lands twice and never un-commits.
        assert!(base(STATE_COMMITTED, 100, 100).is_err());
        assert!(base(STATE_SETTLED, 200, 100).is_err());
        assert!(base(STATE_REJECTED, 200, 100).is_err());
        assert!(base(STATE_EXPIRED, 200, 100).is_err());
    }

    #[test]
    fn submit_constraint_report_gate_matrix() {
        // Caller binding is structural (AgentRecord match in the
        // handler), so the helper takes no caller argument.
        let base = |evaluator: bool, state: u8, known: bool, status: u8| {
            can_submit_constraint_report(evaluator, state, known, status)
        };
        // The clean cases pass: Evaluator, Registered or Planning,
        // known constraint, real status.
        assert!(base(true, STATE_REGISTERED, true, STATUS_PASS).is_ok());
        assert!(base(true, STATE_PLANNING, true, STATUS_FAIL).is_ok());
        // A non-Evaluator is refused first, whatever else holds.
        assert!(base(false, STATE_REGISTERED, true, STATUS_PASS).is_err());
        // Wrong state refuses (Committed, Settled, Rejected, Expired).
        assert!(base(true, STATE_COMMITTED, true, STATUS_PASS).is_err());
        assert!(base(true, STATE_SETTLED, true, STATUS_PASS).is_err());
        // An id outside the frozen set is unknown, never recorded.
        assert!(base(true, STATE_REGISTERED, false, STATUS_PASS).is_err());
        // A status code outside 0..=2 is not a status.
        assert!(base(true, STATE_REGISTERED, true, 3).is_err());
    }

    #[test]
    fn submit_objectives_check_gate_matrix() {
        // Caller binding is structural (AgentRecord match in the
        // handler), so the helper takes no caller argument. The
        // verdict itself is a bool and needs no validation — any bool
        // is a real verdict, and a later report overwrites it.
        let base =
            |evaluator: bool, state: u8| can_submit_objectives_check(evaluator, state);
        // The clean cases pass: Evaluator, Registered or Planning.
        assert!(base(true, STATE_REGISTERED).is_ok());
        assert!(base(true, STATE_PLANNING).is_ok());
        // A non-Evaluator is refused first, whatever else holds.
        assert!(base(false, STATE_REGISTERED).is_err());
        assert!(base(false, STATE_PLANNING).is_err());
        // Wrong state refuses (Committed, Settled, Rejected, Expired).
        assert!(base(true, STATE_COMMITTED).is_err());
        assert!(base(true, STATE_SETTLED).is_err());
        assert!(base(true, STATE_REJECTED).is_err());
        assert!(base(true, STATE_EXPIRED).is_err());
    }

    #[test]
    fn constraint_set_validation_and_reporting() {
        // Empty and distinct sets pass; the bound is 8.
        assert!(validate_constraint_ids(&[]).is_ok());
        assert!(validate_constraint_ids(&[[1u8; 32], [2u8; 32]]).is_ok());
        assert!(validate_constraint_ids(&[[1u8; 32], [1u8; 32]]).is_err());
        let too_many: Vec<[u8; 32]> = (0..9).map(|b| [b; 32]).collect();
        assert!(validate_constraint_ids(&too_many).is_err());
        // Reporting overwrites the entry's status in place.
        let mut set = vec![
            ConstraintEntry { id: [1u8; 32], status: STATUS_UNAVAILABLE },
            ConstraintEntry { id: [2u8; 32], status: STATUS_UNAVAILABLE },
        ];
        set_constraint_status(&mut set, [1u8; 32], STATUS_PASS).expect("known id reports");
        assert_eq!(set[0].status, STATUS_PASS);
        assert_eq!(set[1].status, STATUS_UNAVAILABLE);
        // Re-reporting corrects; the set never grows.
        set_constraint_status(&mut set, [1u8; 32], STATUS_FAIL).expect("re-report lands");
        assert_eq!(set[0].status, STATUS_FAIL);
        assert_eq!(set.len(), 2);
        // An unknown id reports nothing and changes nothing.
        assert!(set_constraint_status(&mut set, [9u8; 32], STATUS_PASS).is_err());
        assert_eq!(set[0].status, STATUS_FAIL);
        // A bad status code is refused before any write.
        assert!(set_constraint_status(&mut set, [1u8; 32], 3).is_err());
        assert_eq!(set[0].status, STATUS_FAIL);
    }

    #[test]
    fn signal_record_space_matches_fields() {
        // 8 + 8 + 4 + 32 + 8 + 8 + 1 = 69.
        assert_eq!(SignalRecord::SPACE, 69);
        assert_eq!(SUPPORT_ONE_RAW, 1_000_000);
    }

    #[test]
    fn submit_signal_gate_matrix() {
        // Caller binding and the duplicate rule are structural (PDA
        // seeds + AgentRecord match; one SignalRecord per
        // (transition, round, agent)), so the helper takes no caller
        // argument. max_rounds 3, current round 0 at launch.
        let base = |state: u8, support: u64, round: u32, active: bool| {
            can_submit_signal(active, state, support, round, 3, 0)
        };
        // The clean cases pass: Registered or Planning, round 0.
        assert!(base(STATE_REGISTERED, 500_000, 0, true).is_ok());
        assert!(base(STATE_PLANNING, 1_000_000, 0, true).is_ok());
        // Support above 1.0 is out of range, checked first.
        assert!(base(STATE_REGISTERED, 1_000_001, 0, true).is_err());
        // Wrong state refuses (Committed, Settled, Rejected, Expired).
        assert!(base(STATE_COMMITTED, 500_000, 0, true).is_err());
        assert!(base(STATE_SETTLED, 500_000, 0, true).is_err());
        // Round at max_rounds is exhausted; a stale round is bad state.
        assert!(base(STATE_PLANNING, 500_000, 3, true).is_err());
        assert!(base(STATE_PLANNING, 500_000, 1, true).is_err());
        // Inactive agents are not eligible.
        assert!(base(STATE_PLANNING, 500_000, 0, false).is_err());
    }

    #[test]
    fn monotone_revision_gate() {
        // Mean 600_000, previous signal 500_000 (|diff| 100_000).
        // Moving toward the mean passes; holding passes; moving away fails.
        assert!(check_monotone_revision(500_000, 600_000, 600_000).is_ok());
        assert!(check_monotone_revision(500_000, 600_000, 550_000).is_ok());
        assert!(check_monotone_revision(500_000, 600_000, 500_000).is_ok());
        assert!(check_monotone_revision(500_000, 600_000, 499_999).is_err());
        assert!(check_monotone_revision(500_000, 600_000, 700_001).is_err());
        // Exactly as far as before is admissible (no farther).
        assert!(check_monotone_revision(500_000, 600_000, 700_000).is_ok());
    }

    #[test]
    fn advance_round_gate_matrix() {
        // Caller binding is permissionless (the kernel ignores its
        // advance_round caller), so the helper takes no caller
        // argument. max_rounds 3 at launch.
        let base = |state: u8, total: u64, count: u32| {
            can_advance_round(state, total, count, 3)
        };
        // The clean cases pass: Registered or Planning, non-empty
        // round, room under max_rounds.
        assert!(base(STATE_REGISTERED, 1, 1).is_ok());
        assert!(base(STATE_PLANNING, 500_000, 2).is_ok());
        // Wrong state refuses (Committable, Committed, Settled,
        // Rejected, Expired) — checked before emptiness.
        assert!(base(STATE_COMMITTABLE, 500_000, 1).is_err());
        assert!(base(STATE_COMMITTED, 500_000, 1).is_err());
        assert!(base(STATE_SETTLED, 500_000, 1).is_err());
        // An empty round refuses: no signals, or only zero-weight
        // signals — both are a zero total, and the kernel's test is
        // total == ZERO, not a signal count.
        assert!(base(STATE_REGISTERED, 0, 1).is_err());
        assert!(base(STATE_PLANNING, 0, 2).is_err());
        // At max_rounds a non-empty round is exhausted.
        assert!(base(STATE_PLANNING, 500_000, 3).is_err());
        // Check order, as in the kernel: an empty round at max_rounds
        // reports RoundEmpty, not RoundsExhausted.
        let empty_at_max = base(STATE_PLANNING, 0, 3);
        assert!(empty_at_max.is_err());
        assert_eq!(
            empty_at_max.unwrap_err(),
            anchor_lang::error::Error::from(TreasuryError::RoundEmpty)
                .into()
        );
    }

    #[test]
    fn round_aggregates_and_mean_follow_kernel_amount_math() {
        // FlatWeights: relevance and group policy are 1.0, so eff is
        // raw x influence / SCALE, and term is eff x support / SCALE.
        // Full influence (1.0): eff == raw.
        assert_eq!(
            signal_aggregates(1_000_000, 600_000, 1_000_000).unwrap(),
            (1_000_000, 600_000)
        );
        // Half influence (0.5): raw 1.0 -> eff 0.5; support 0.6 ->
        // term 0.3.
        assert_eq!(
            signal_aggregates(1_000_000, 600_000, 500_000).unwrap(),
            (500_000, 300_000)
        );
        // Zero raw or zero influence contributes nothing — the round
        // stays empty in the kernel's sense.
        assert_eq!(
            signal_aggregates(0, 600_000, 1_000_000).unwrap(),
            (0, 0)
        );
        assert_eq!(
            signal_aggregates(1_000_000, 600_000, 0).unwrap(),
            (0, 0)
        );
        // The mean is weighted x ONE / total, floored, as the kernel
        // computes it for the previous round.
        assert_eq!(round_mean(600_000, 1_000_000).unwrap(), 600_000);
        assert_eq!(round_mean(300_000, 500_000).unwrap(), 600_000);
        assert_eq!(round_mean(1, 3).unwrap(), 333_333);
        // A zero total has no mean.
        assert!(round_mean(0, 0).is_err());
        assert!(round_mean(600_000, 0).is_err());
    }

    #[test]
    fn plan_record_space_matches_fields() {
        // 8 + 8 + 32 + 8 + (4 + 16 x 10) + 32 + 1 = 253.
        assert_eq!(PlanRecord::SPACE, 253);
        assert_eq!(MAX_CLAIMS, 16);
    }

    #[test]
    fn transition_states_follow_kernel_order() {
        // Design §3: the kernel's seven states, exactly, in order.
        assert_eq!(STATE_REGISTERED, 0);
        assert_eq!(STATE_PLANNING, 1);
        assert_eq!(STATE_COMMITTABLE, 2);
        assert_eq!(STATE_COMMITTED, 3);
        assert_eq!(STATE_SETTLED, 4);
        assert_eq!(STATE_REJECTED, 5);
        assert_eq!(STATE_EXPIRED, 6);
    }

    #[test]
    fn observation_window_validation() {
        // Valid window passes.
        assert!(validate_observation_window(100, 200, 150).is_ok());
        // Future-dated observation is rejected (verification theater).
        assert!(validate_observation_window(200, 300, 150).is_err());
        // Expiry at the timestamp is not a window.
        assert!(validate_observation_window(100, 100, 150).is_err());
        // Expiry before the timestamp is not a window.
        assert!(validate_observation_window(100, 90, 150).is_err());
        // Timestamp exactly at now is not future-dated.
        assert!(validate_observation_window(150, 200, 150).is_ok());
    }

    #[test]
    fn transition_expiry_validation() {
        assert!(validate_transition_expiry(200, 150).is_ok());
        // Already expired at registration creates nothing.
        assert!(validate_transition_expiry(150, 150).is_err());
        assert!(validate_transition_expiry(100, 150).is_err());
    }

    #[test]
    fn fresh_transition_record_starts_registered() {
        let proposer = Pubkey::new_from_array([1u8; 32]);
        let record = TransitionRecord {
            transition_id: 7,
            transition_type: [4u8; 32],
            state: STATE_REGISTERED,
            proposer,
            observation_id: 3,
            expires_at: 1_000,
            planning_deadline: 0,
            plan_count: 0,
            round_count: 1,
            current_round_total: 0,
            current_round_weighted: 0,
            prev_round_mean: 0,
            constraints: vec![ConstraintEntry {
                id: [5u8; 32],
                status: STATUS_UNAVAILABLE,
            }],
            objectives_ok: false,
            reconciled: false,
            declared_hash: [9u8; 32],
            parent_index: 0,
            parent_hash: [0; 32],
            commit_index: 0,
            realized_hash: [0; 32],
            exceptions: vec![],
            unresolved_blocking: 0,
            policy_amendment_applied: false,
            bump: 255,
        };
        assert_eq!(record.state, STATE_REGISTERED);
        assert!(!record.policy_amendment_applied);
        assert_eq!(record.proposer, proposer);
        assert_eq!(record.planning_deadline, 0);
        assert_eq!(record.plan_count, 0);
        assert_eq!(record.round_count, 1);
        assert_eq!(record.current_round_total, 0);
        assert_eq!(record.current_round_weighted, 0);
        assert_eq!(record.prev_round_mean, 0);
        assert!(!record.objectives_ok);
        assert!(!record.reconciled);
        assert_eq!(record.constraints.len(), 1);
        assert_eq!(record.constraints[0].status, STATUS_UNAVAILABLE);
        assert_eq!(record.observation_id, 3);
        assert_eq!(record.commit_index, 0);
        assert_eq!(record.realized_hash, [0; 32]);
    }

    #[test]
    fn open_planning_admits_proposer_or_admin_only() {
        let proposer = Pubkey::new_from_array([1u8; 32]);
        let admin = Pubkey::new_from_array([2u8; 32]);
        let stranger = Pubkey::new_from_array([3u8; 32]);
        // The proposer opens their own transition.
        assert!(can_open_planning(proposer, proposer, false, STATE_REGISTERED).is_ok());
        // An Admin opens anyone's transition.
        assert!(can_open_planning(admin, proposer, true, STATE_REGISTERED).is_ok());
        // A non-proposer non-Admin is refused.
        assert!(can_open_planning(stranger, proposer, false, STATE_REGISTERED).is_err());
        // Even the proposer cannot re-open a transition already Planning.
        assert!(can_open_planning(proposer, proposer, false, STATE_PLANNING).is_err());
        // Even an Admin cannot open a Committed transition.
        assert!(can_open_planning(admin, proposer, true, STATE_COMMITTED).is_err());
    }

    #[test]
    fn submit_plan_gate_matrix() {
        // Caller binding is structural (PDA seeds + AgentRecord match
        // in the handler), so the helper takes no caller argument.
        let base = |state: u8,
                    now: i64,
                    deadline: i64,
                    active: bool,
                    count: u32,
                    expires: i64| {
            can_submit_plan(active, state, now, deadline, count, 4, expires)
        };
        // The clean case passes.
        assert!(base(STATE_PLANNING, 100, 200, true, 0, 300).is_ok());
        // Room under the cap passes; at the cap it does not.
        assert!(base(STATE_PLANNING, 100, 200, true, 3, 300).is_ok());
        assert!(base(STATE_PLANNING, 100, 200, true, 4, 300).is_err());
        // Planning not open: wrong state, or the deadline reached.
        assert!(base(STATE_REGISTERED, 100, 200, true, 0, 300).is_err());
        assert!(base(STATE_PLANNING, 200, 200, true, 0, 300).is_err());
        assert!(base(STATE_PLANNING, 201, 200, true, 0, 300).is_err());
        // Inactive agents are not eligible.
        assert!(base(STATE_PLANNING, 100, 200, false, 0, 300).is_err());
        // Plan expiry at or before now is a bad window.
        assert!(base(STATE_PLANNING, 100, 200, true, 0, 100).is_err());
        assert!(base(STATE_PLANNING, 100, 200, true, 0, 99).is_err());
    }

    #[test]
    fn claims_validation_and_hash() {
        let claim = |resource_id: u16, amount: u64| Claim { resource_id, amount };
        // Empty and distinct claims pass.
        assert!(validate_claims(&[]).is_ok());
        assert!(validate_claims(&[claim(0, 100), claim(1, 200), claim(2, 300)]).is_ok());
        // A resource claimed twice is rejected, not double-counted.
        assert!(validate_claims(&[claim(1, 100), claim(1, 200)]).is_err());
        // Over the account bound is rejected.
        let too_many: Vec<Claim> = (0..17).map(|r| claim(r, 1)).collect();
        assert!(validate_claims(&too_many).is_err());
        // The hash is deterministic, order-sensitive, and content-bound.
        let claims = vec![claim(0, 100), claim(2, 300)];
        assert_eq!(claims_hash(&claims), claims_hash(&claims));
        assert_ne!(claims_hash(&claims), claims_hash(&[claim(0, 100)]));
        assert_ne!(claims_hash(&claims), claims_hash(&[claim(2, 300), claim(0, 100)]));
        // SHA-256 of the empty encoding is the well-known empty hash.
        assert_eq!(
            claims_hash(&[]),
            [
                0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8,
                0x99, 0x6f, 0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c,
                0xa4, 0x95, 0x99, 0x1b, 0x78, 0x52, 0xb8, 0x55,
            ]
        );
    }

    #[test]
    fn planning_deadline_is_now_plus_window() {
        assert_eq!(
            planning_deadline(1_000, kernel_bridge::PLANNING_WINDOW).unwrap(),
            1_000 + kernel_bridge::PLANNING_WINDOW as i64
        );
        // Overflow is an error, never a wrapped deadline.
        assert!(planning_deadline(i64::MAX, kernel_bridge::PLANNING_WINDOW).is_err());
    }

    #[test]
    fn reconcile_gate_matrix() {
        // Caller binding is permissionless (the kernel ignores its
        // reconcile caller), so the helper takes no caller argument.
        let base = |state: u8, now: i64, deadline: i64| {
            can_reconcile(state, now, deadline)
        };
        // The clean cases pass: Planning, at or past the deadline.
        assert!(base(STATE_PLANNING, 200, 200).is_ok());
        assert!(base(STATE_PLANNING, 201, 200).is_ok());
        // Wrong state refuses, even past the deadline — only Planning
        // moves to Committable (the kernel's set_state gate).
        assert!(base(STATE_REGISTERED, 201, 200).is_err());
        assert!(base(STATE_COMMITTABLE, 201, 200).is_err());
        assert!(base(STATE_COMMITTED, 201, 200).is_err());
        assert!(base(STATE_SETTLED, 201, 200).is_err());
        // Before the deadline refuses, in Planning or not — checked
        // after the state, as in the kernel.
        assert!(base(STATE_PLANNING, 199, 200).is_err());
        // Check order, as in the kernel: a non-Planning transition
        // before its deadline reports BadState, not PlanningStillOpen.
        let wrong_state_early = base(STATE_REGISTERED, 199, 200);
        assert!(wrong_state_early.is_err());
        assert_eq!(
            wrong_state_early.unwrap_err(),
            anchor_lang::error::Error::from(TreasuryError::BadState).into()
        );
        let still_open = base(STATE_PLANNING, 199, 200);
        assert_eq!(
            still_open.unwrap_err(),
            anchor_lang::error::Error::from(TreasuryError::PlanningStillOpen)
                .into()
        );
    }

    #[test]
    fn capacity_fit_follows_kernel_rules() {
        // Empty claims fit vacuously, even with no stocks at all —
        // the kernel's all() over an empty aggregate is true.
        assert!(fits_capacity(&[], &[]));
        assert!(fits_capacity(&[], &[(0, 1_000)]));
        // Exact fit passes; one unit over fails.
        assert!(fits_capacity(&[(0, 1_000)], &[(0, 1_000)]));
        assert!(!fits_capacity(&[(0, 1_001)], &[(0, 1_000)]));
        // A claimed resource with no stock fails — the kernel's
        // is_some_and test — even when other stocks are ample.
        assert!(!fits_capacity(&[(1, 1)], &[(0, 1_000_000)]));
        // Aggregates sum across plans: two claims on one stock that
        // fit singly but not together fail.
        assert!(fits_capacity(&[(0, 600), (0, 400)], &[(0, 1_000)]));
        assert!(!fits_capacity(&[(0, 600), (0, 401)], &[(0, 1_000)]));
        // Multiple resources are judged independently.
        assert!(fits_capacity(
            &[(0, 100), (1, 200)],
            &[(0, 100), (1, 200)]
        ));
        assert!(!fits_capacity(
            &[(0, 100), (1, 201)],
            &[(0, 100), (1, 200)]
        ));
        // An overflowing aggregate fails the fit, never wraps into
        // a small number that would fit.
        assert!(!fits_capacity(
            &[(0, u64::MAX), (0, 1)],
            &[(0, u64::MAX)]
        ));
        // The empty-stock-map evaluation the handlers performed
        // before stocks landed: any claim fails, no claims fit —
        // fail-closed, and still the rule today for a claimed
        // resource with no stock account passed.
        assert!(!fits_capacity(&[(0, 1)], &[]));
    }

    #[test]
    fn capacity_stock_space_matches_fields() {
        // 8 (discriminator) + 2 + 8 + 1.
        assert_eq!(CapacityStock::SPACE, 19);
    }

    #[test]
    fn commit_debit_follows_kernel_rules() {
        // Empty claims leave every balance untouched.
        assert_eq!(
            apply_debit(&[(0, 1_000), (1, 500)], &[]),
            Some(vec![(0, 1_000), (1, 500)])
        );
        // Partial debits, per resource, independently.
        assert_eq!(
            apply_debit(&[(0, 1_000), (1, 500)], &[(0, 400), (1, 500)]),
            Some(vec![(0, 600), (1, 0)])
        );
        // Claims on one stock sum across plans before debiting.
        assert_eq!(
            apply_debit(&[(0, 1_000)], &[(0, 600), (0, 400)]),
            Some(vec![(0, 0)])
        );
        // Exact-fit debit empties the stock without going negative.
        assert_eq!(
            apply_debit(&[(2, 250)], &[(2, 250)]),
            Some(vec![(2, 0)])
        );
        // Over-claim fails whole, and the input is untouched — the
        // helper borrows, so a None carries no partial mutation.
        let stocks = [(0, 1_000)];
        assert_eq!(apply_debit(&stocks, &[(0, 1_001)]), None);
        assert_eq!(stocks, [(0, 1_000)]);
        // A claimed resource with no stock fails, even when other
        // stocks are ample — the same fail-closed rule as the fit.
        assert_eq!(apply_debit(&[(0, 1_000)], &[(1, 1)]), None);
        // An overflowing aggregate fails, never wraps into a debit
        // that fits.
        assert_eq!(
            apply_debit(&[(0, u64::MAX)], &[(0, u64::MAX), (0, 1)]),
            None
        );
        // Untouched stocks keep their balances, in order.
        assert_eq!(
            apply_debit(&[(0, 100), (1, 200), (2, 300)], &[(1, 50)]),
            Some(vec![(0, 100), (1, 150), (2, 300)])
        );
    }

    #[test]
    fn commit_gate_matrix() {
        // Only Committable commits — the kernel's set_state gate.
        assert!(can_commit(STATE_COMMITTABLE).is_ok());
        assert!(can_commit(STATE_REGISTERED).is_err());
        assert!(can_commit(STATE_PLANNING).is_err());
        assert!(can_commit(STATE_COMMITTED).is_err());
        assert!(can_commit(STATE_SETTLED).is_err());
        assert!(can_commit(STATE_REJECTED).is_err());
        assert!(can_commit(STATE_EXPIRED).is_err());
    }

    #[test]
    fn record_evidence_gate_matrix() {
        // Caller binding is structural (AgentRecord match in the
        // handler), so the helper takes no caller argument. The
        // realized hash itself is a value, not a claim the program
        // can verify — any 32 bytes record; a hash that does not
        // match the realized outcome fails settle later, not here.
        let base = |active: bool, oracle: bool, state: u8| {
            can_record_evidence(active, oracle, state)
        };
        // The clean case passes: active Oracle, Committed transition.
        assert!(base(true, true, STATE_COMMITTED).is_ok());
        // A non-Oracle is refused first, whatever else holds.
        assert!(base(true, false, STATE_COMMITTED).is_err());
        // An inactive Oracle is refused — the same fail-closed
        // tightening post_observation applies.
        assert!(base(false, true, STATE_COMMITTED).is_err());
        // Wrong state refuses: evidence is for committed transitions
        // only — not Registered, Planning, Committable, Settled,
        // Rejected, or Expired. The kernel's settle gate, applied one
        // step early.
        assert!(base(true, true, STATE_REGISTERED).is_err());
        assert!(base(true, true, STATE_PLANNING).is_err());
        assert!(base(true, true, STATE_COMMITTABLE).is_err());
        assert!(base(true, true, STATE_SETTLED).is_err());
        assert!(base(true, true, STATE_REJECTED).is_err());
        assert!(base(true, true, STATE_EXPIRED).is_err());
        // Check order, as in the kernel's settle: the attestor gate
        // before the state gate.
        let role_first = base(false, false, STATE_REGISTERED);
        assert_eq!(
            role_first.unwrap_err(),
            anchor_lang::error::Error::from(TreasuryError::Unauthorized).into()
        );
    }

    #[test]
    fn evidence_record_space_matches_fields() {
        // 8 + 8 + 32 + 1 = 49.
        assert_eq!(EvidenceRecord::SPACE, 49);
    }

    #[test]
    fn settle_gate_matrix() {
        // Caller binding is structural (AgentRecord match in the
        // handler), so the helper takes no caller argument. Check
        // order, as in the kernel's settle: the attestor gate (role,
        // then evidence) before the state gate.
        let base = |active: bool, oracle: bool, evidence: bool, state: u8| {
            can_settle(active, oracle, evidence, state)
        };
        // The clean case passes: active Oracle, matching evidence,
        // Committed transition.
        assert!(base(true, true, true, STATE_COMMITTED).is_ok());
        // A non-Oracle is refused first, whatever else holds.
        assert!(base(true, false, true, STATE_COMMITTED).is_err());
        assert!(base(true, false, false, STATE_COMMITTED).is_err());
        // An inactive Oracle is refused — the same fail-closed
        // tightening post_observation and record_evidence apply.
        assert!(base(false, true, true, STATE_COMMITTED).is_err());
        // A mismatched hash is refused: the presented hash must equal
        // the independently recorded EvidenceRecord.
        assert!(base(true, true, false, STATE_COMMITTED).is_err());
        // Wrong state refuses: Registered, Planning, Committable,
        // Settled (no re-settle), Rejected, Expired.
        assert!(base(true, true, true, STATE_REGISTERED).is_err());
        assert!(base(true, true, true, STATE_PLANNING).is_err());
        assert!(base(true, true, true, STATE_COMMITTABLE).is_err());
        assert!(base(true, true, true, STATE_SETTLED).is_err());
        assert!(base(true, true, true, STATE_REJECTED).is_err());
        assert!(base(true, true, true, STATE_EXPIRED).is_err());
        // Check order: role before evidence, evidence before state.
        let role_first = base(false, false, false, STATE_REGISTERED);
        assert_eq!(
            role_first.unwrap_err(),
            anchor_lang::error::Error::from(TreasuryError::Unauthorized).into()
        );
        let evidence_before_state = base(true, true, false, STATE_SETTLED);
        assert_eq!(
            evidence_before_state.unwrap_err(),
            anchor_lang::error::Error::from(TreasuryError::Unauthorized).into()
        );
        let state_last = base(true, true, true, STATE_SETTLED);
        assert_eq!(
            state_last.unwrap_err(),
            anchor_lang::error::Error::from(TreasuryError::BadState).into()
        );
    }

    #[test]
    fn settle_attributions_sum_per_agent_in_order() {
        // The kernel sums per agent, order-independent: the same
        // agents in a different listing order sum the same, and the
        // handler's unique-agent set is the summed set.
        let a = Pubkey::new_from_array([1u8; 32]);
        let b = Pubkey::new_from_array([2u8; 32]);
        let list = vec![
            Attribution { agent: a, delta: 5 },
            Attribution { agent: b, delta: -3 },
            Attribution { agent: a, delta: 7 },
            Attribution { agent: a, delta: -2 },
        ];
        assert_eq!(
            sum_attributions(&list),
            vec![(a, 10), (b, -3)]
        );
        // Reordered, the same sums (first-appearance order changes,
        // the per-agent totals do not).
        let reordered = vec![
            Attribution { agent: b, delta: -3 },
            Attribution { agent: a, delta: -2 },
            Attribution { agent: a, delta: 7 },
            Attribution { agent: a, delta: 5 },
        ];
        let summed = sum_attributions(&reordered);
        assert_eq!(summed.len(), 2);
        assert_eq!(
            summed.iter().find(|(agent, _)| *agent == a).unwrap().1,
            10
        );
        assert_eq!(
            summed.iter().find(|(agent, _)| *agent == b).unwrap().1,
            -3
        );
        // Empty attributions sum to empty: a settle with no
        // attributions still flips state and records the hash.
        assert!(sum_attributions(&[]).is_empty());
        // i64 extremes cannot overflow the i128 sum.
        assert_eq!(
            sum_attributions(&[
                Attribution { agent: a, delta: i64::MAX },
                Attribution { agent: a, delta: i64::MAX },
            ]),
            vec![(a, 2 * i64::MAX as i128)]
        );
    }

    #[test]
    fn settle_attribution_follows_kernel_rule() {
        // Exact settle, inside the cap: the delta applies in full.
        assert_eq!(settle_attribution(50_000_000, 5_000_000, true), 55_000_000);
        assert_eq!(settle_attribution(50_000_000, -5_000_000, true), 45_000_000);
        // The cap clamps both directions: ±20 units raw.
        assert_eq!(
            settle_attribution(50_000_000, 100_000_000, true),
            70_000_000
        );
        assert_eq!(
            settle_attribution(50_000_000, -100_000_000, true),
            30_000_000
        );
        assert_eq!(MAX_ATTRIBUTION_RAW, 20_000_000);
        // Inexact settle halves positive credit only: 5 -> 2
        // (integer division, as the kernel's n /= 2), -5 untouched.
        assert_eq!(settle_attribution(50_000_000, 5, false), 50_000_002);
        assert_eq!(settle_attribution(50_000_000, -5, false), 49_999_995);
        // Capped-then-halved: a 100-unit positive claim caps to 20
        // and halves to 10 on an inexact settle.
        assert_eq!(
            settle_attribution(50_000_000, 100_000_000, false),
            60_000_000
        );
        // The 0–100 unit band clamps both ends: reputation never
        // leaves it, whatever the delta.
        assert_eq!(
            settle_attribution(99_000_000, 20_000_000, true),
            100_000_000
        );
        assert_eq!(settle_attribution(1_000_000, -20_000_000, true), 0);
        assert_eq!(settle_attribution(0, -1, false), 0);
    }

    #[test]
    fn commit_mask_bit_values_match_kernel() {
        // The program's local constants must equal the kernel crate's
        // FailMask values, or the on-chain predicate has drifted from
        // the crate it vendors — a loud failure, never a silent one.
        assert_eq!(MASK_CONSTRAINTS, coordination_kernel::FailMask::CONSTRAINTS);
        assert_eq!(MASK_OBJECTIVES, coordination_kernel::FailMask::OBJECTIVES);
        assert_eq!(MASK_QUORUM, coordination_kernel::FailMask::QUORUM);
        assert_eq!(MASK_CONFIDENCE, coordination_kernel::FailMask::CONFIDENCE);
        assert_eq!(MASK_COMPAT, coordination_kernel::FailMask::COMPAT);
        assert_eq!(MASK_EXCEPTIONS, coordination_kernel::FailMask::EXCEPTIONS);
        assert_eq!(MASK_FRESH, coordination_kernel::FailMask::FRESH);
        assert_eq!(
            MASK_PLANNING_OPEN,
            coordination_kernel::FailMask::PLANNING_OPEN
        );
        assert_eq!(MASK_EXPIRED, coordination_kernel::FailMask::EXPIRED);
        assert_eq!(MASK_LINEAGE, coordination_kernel::FailMask::LINEAGE);
        assert_eq!(MASK_PLANS, coordination_kernel::FailMask::PLANS);
        assert_eq!(MASK_CAPABILITY, coordination_kernel::FailMask::CAPABILITY);
        assert_eq!(MASK_DIALECT, coordination_kernel::FailMask::DIALECT);
    }

    /// The all-clear input set, mirroring the kernel_bridge
    /// well-formed lifecycle: one Pass constraint, objectives not
    /// required, round total 1.0 against a 1.0 quorum, weighted mean
    /// 0.9 against a 0.5 threshold, reconciled with a fitting (empty)
    /// claim set, no blocking exceptions, fresh 0.9-confidence
    /// observation, planning closed, manifest live, genesis lineage,
    /// one unexpired plan, capabilities covered. The closure mutates
    /// one field per case; the mask is computed inside, so the
    /// borrowed statuses slice never outlives this function.
    fn mask_with<F>(mut flip: F) -> u32
    where
        F: FnMut(&mut CommitInputs<'_>),
    {
        let statuses = [STATUS_PASS];
        let mut inputs = CommitInputs {
            constraint_statuses: &statuses,
            objectives_ok: false,
            require_objectives: false,
            round_total: 1_000_000,
            round_weighted: 900_000,
            quorum_required: 1_000_000,
            confidence_threshold: 500_000,
            reconciled: true,
            fits: true,
            unresolved_blocking: 0,
            obs_timestamp: 1_100,
            obs_expiry: 10_000,
            obs_confidence: 900_000,
            min_obs_confidence: 500_000,
            now: 1_150,
            planning_deadline: 1_113,
            expires_at: 11_000,
            parent_index: 0,
            parent_hash: [0; 32],
            chain_index: 0,
            chain_hash: [0; 32],
            plan_count: 1,
            min_plan_expiry: Some(6_100),
            capability_covered: true,
        };
        flip(&mut inputs);
        commit_fail_mask(&inputs)
    }

    #[test]
    fn commit_mask_bit_matrix() {
        // Each flip sets exactly its own bit and no other — the
        // per-bit proof that the on-chain predicate is the kernel's.
        // Multi-bit cases name every bit they set; a case that sets
        // an unnamed bit fails here instead of passing quietly.
        assert_eq!(mask_with(|_| {}), 0);
        // CONSTRAINTS: a Fail report, an Unavailable report, an empty set.
        assert_eq!(
            mask_with(|i| i.constraint_statuses = &[STATUS_FAIL]),
            MASK_CONSTRAINTS
        );
        assert_eq!(
            mask_with(|i| i.constraint_statuses = &[STATUS_UNAVAILABLE]),
            MASK_CONSTRAINTS
        );
        assert_eq!(
            mask_with(|i| i.constraint_statuses = &[]),
            MASK_CONSTRAINTS
        );
        // OBJECTIVES: required and unreported.
        assert_eq!(
            mask_with(|i| {
                i.require_objectives = true;
                i.objectives_ok = false;
            }),
            MASK_OBJECTIVES
        );
        // A required-but-reported objectives check clears it.
        assert_eq!(
            mask_with(|i| {
                i.require_objectives = true;
                i.objectives_ok = true;
            }),
            0
        );
        // QUORUM: total below required. Confidence stays clear: the
        // mean over the smaller total is still above threshold.
        assert_eq!(mask_with(|i| i.round_total = 999_999), MASK_QUORUM);
        // CONFIDENCE: mean below threshold, total still at quorum.
        assert_eq!(
            mask_with(|i| i.round_weighted = 499_999),
            MASK_CONFIDENCE
        );
        // Empty round fails QUORUM and CONFIDENCE together, never
        // divides by zero.
        assert_eq!(
            mask_with(|i| {
                i.round_total = 0;
                i.round_weighted = 0;
            }),
            MASK_QUORUM | MASK_CONFIDENCE
        );
        // COMPAT: unreconciled, or reconciled-but-not-fitting.
        assert_eq!(mask_with(|i| i.reconciled = false), MASK_COMPAT);
        assert_eq!(mask_with(|i| i.fits = false), MASK_COMPAT);
        // EXCEPTIONS: one unresolved blocking exception.
        assert_eq!(
            mask_with(|i| i.unresolved_blocking = 1),
            MASK_EXCEPTIONS
        );
        // FRESH: future-dated, expired, low-confidence observation.
        assert_eq!(mask_with(|i| i.obs_timestamp = 1_151), MASK_FRESH);
        assert_eq!(mask_with(|i| i.obs_expiry = 1_150), MASK_FRESH);
        assert_eq!(mask_with(|i| i.obs_confidence = 499_999), MASK_FRESH);
        // PLANNING_OPEN: now before the deadline. FRESH stays clear
        // (now is still inside the observation window).
        assert_eq!(mask_with(|i| i.now = 1_112), MASK_PLANNING_OPEN);
        // EXPIRED: now at the manifest expiry. FRESH sets too (the
        // observation expired at 10_000) and PLANS sets too (the
        // earliest plan expired at 6_100) — all three named.
        assert_eq!(
            mask_with(|i| i.now = 11_000),
            MASK_EXPIRED | MASK_FRESH | MASK_PLANS
        );
        // LINEAGE: wrong index, wrong hash.
        assert_eq!(mask_with(|i| i.chain_index = 1), MASK_LINEAGE);
        assert_eq!(mask_with(|i| i.chain_hash = [1; 32]), MASK_LINEAGE);
        // PLANS: no plans, or the earliest plan expired.
        assert_eq!(
            mask_with(|i| {
                i.plan_count = 0;
                i.min_plan_expiry = None;
            }),
            MASK_PLANS
        );
        assert_eq!(
            mask_with(|i| i.min_plan_expiry = Some(1_150)),
            MASK_PLANS
        );
        // CAPABILITY: an uncovered requirement.
        assert_eq!(
            mask_with(|i| i.capability_covered = false),
            MASK_CAPABILITY
        );
        // Everything bad at once: all bits except DIALECT, which the
        // kernel's stub leaves always clear.
        let worst = mask_with(|i| {
            i.constraint_statuses = &[];
            i.require_objectives = true;
            i.objectives_ok = false;
            i.round_total = 0;
            i.round_weighted = 0;
            i.reconciled = false;
            i.fits = false;
            i.unresolved_blocking = 3;
            i.obs_timestamp = 9_999;
            i.obs_confidence = 0;
            i.planning_deadline = 9_999;
            i.expires_at = 1_000;
            i.chain_index = 7;
            i.plan_count = 0;
            i.min_plan_expiry = None;
            i.capability_covered = false;
        });
        assert_eq!(
            worst,
            MASK_CONSTRAINTS
                | MASK_OBJECTIVES
                | MASK_QUORUM
                | MASK_CONFIDENCE
                | MASK_COMPAT
                | MASK_EXCEPTIONS
                | MASK_FRESH
                | MASK_PLANNING_OPEN
                | MASK_EXPIRED
                | MASK_LINEAGE
                | MASK_PLANS
                | MASK_CAPABILITY
        );
        assert_eq!(worst & MASK_DIALECT, 0);
    }

    #[test]
    fn policy_record_space_matches_fields() {
        // 8 (discriminator) + 32 (type) + 8 + 8 + 8 (quorum,
        // confidence, min-obs) + 1 (objectives) + 8 (window) + 4 + 4
        // (caps) + (4 + 8 x 32) mandatory + (4 + 8 x (32 + 8))
        // required caps + 1 (bump) = 666.
        assert_eq!(PolicyRecord::SPACE, 666);
        assert_eq!(MAX_REQUIRED_CAPS, 8);
    }

    #[test]
    fn merged_constraint_set_is_mandatory_union_proposer_deduped() {
        let mandatory = [[1; 32], [2; 32]];
        let additions = [[2; 32], [3; 32]];
        let merged = merge_constraint_sets(&mandatory, &additions);
        // Mandatory order preserved, the duplicate appears once, the
        // new addition appends — never a second entry for one id.
        assert_eq!(merged, vec![[1; 32], [2; 32], [3; 32]]);
        // Empty sides degenerate cleanly both ways.
        assert_eq!(merge_constraint_sets(&mandatory, &[]), mandatory);
        assert_eq!(
            merge_constraint_sets(&[], &additions),
            vec![[2; 32], [3; 32]]
        );
        assert!(merge_constraint_sets(&[], &[]).is_empty());
    }

    #[test]
    fn validate_policy_mirrors_kernel_registration_gates() {
        let mandatory = [[9; 32]];
        let caps = [CapabilityRequirement {
            id: [7; 32],
            min_weight: 500_000,
        }];
        // The launch values install cleanly.
        assert!(validate_policy(500_000, 3, 4, &mandatory, &caps).is_ok());
        // The kernel's own gates: confidence above 1.0, zero rounds,
        // zero plans — each is BadWindow, never a silent install.
        assert!(validate_policy(1_000_001, 3, 4, &mandatory, &caps).is_err());
        assert!(validate_policy(500_000, 0, 4, &mandatory, &caps).is_err());
        assert!(validate_policy(500_000, 3, 0, &mandatory, &caps).is_err());
        // A duplicate mandatory id breaks the frozen-set shape.
        assert!(validate_policy(500_000, 3, 4, &[[9; 32], [9; 32]], &caps).is_err());
    }

    #[test]
    fn validate_policy_rejects_bad_capability_sets() {
        let mandatory = [[9; 32]];
        // A duplicate capability id would double-count one cover.
        let dup = [
            CapabilityRequirement {
                id: [7; 32],
                min_weight: 500_000,
            },
            CapabilityRequirement {
                id: [7; 32],
                min_weight: 250_000,
            },
        ];
        assert!(validate_policy(500_000, 3, 4, &mandatory, &dup).is_err());
        // Nine requirements break the account-size bound.
        let many: Vec<CapabilityRequirement> = (0..9)
            .map(|i| CapabilityRequirement {
                id: [i as u8; 32],
                min_weight: 0,
            })
            .collect();
        assert!(validate_policy(500_000, 3, 4, &mandatory, &many).is_err());
    }

    #[test]
    fn policy_payload_hash_binds_exactly_the_applied_values() {
        let target = [3u8; 32];
        let mandatory = [[9u8; 32]];
        let caps = [CapabilityRequirement {
            id: [7; 32],
            min_weight: 500_000,
        }];
        let base = policy_payload_hash(
            target, 1_000_000, 500_000, 500_000, false, 10, 3, 4, &mandatory,
            &caps,
        );
        // Deterministic: the same values hash the same.
        assert_eq!(
            base,
            policy_payload_hash(
                target, 1_000_000, 500_000, 500_000, false, 10, 3, 4,
                &mandatory, &caps,
            )
        );
        // Every field is load-bearing: changing any one changes the
        // hash, so no supplied value can drift from the declared one.
        assert_ne!(
            base,
            policy_payload_hash(
                target, 999_999, 500_000, 500_000, false, 10, 3, 4,
                &mandatory, &caps,
            )
        );
        assert_ne!(
            base,
            policy_payload_hash(
                target, 1_000_000, 500_000, 500_000, true, 10, 3, 4,
                &mandatory, &caps,
            )
        );
        assert_ne!(
            base,
            policy_payload_hash(
                [4u8; 32], 1_000_000, 500_000, 500_000, false, 10, 3, 4,
                &mandatory, &caps,
            )
        );
        assert_ne!(
            base,
            policy_payload_hash(
                target, 1_000_000, 500_000, 500_000, false, 10, 3, 4,
                &[[8u8; 32]], &caps,
            )
        );
        assert_ne!(
            base,
            policy_payload_hash(
                target, 1_000_000, 500_000, 500_000, false, 10, 3, 4,
                &mandatory,
                &[CapabilityRequirement {
                    id: [7; 32],
                    min_weight: 250_000,
                }],
            )
        );
        // The canonical policy type id is the SHA-256 of its name, not
        // an arbitrary constant: recompute it here from the string.
        assert_eq!(
            POLICY_TRANSITION_TYPE,
            anchor_lang::solana_program::hash::hash(b"treasury.policy.v1")
                .to_bytes()
        );
    }

    #[test]
    fn policy_amendment_gate_matrix() {
        let target = [3u8; 32];
        let payload = [11u8; 32];
        let ok = |state: u8,
                  source_type: [u8; 32],
                  stored: [u8; 32],
                  declared: [u8; 32],
                  realized: [u8; 32],
                  applied: bool| {
            can_apply_policy_amendment(
                state,
                source_type,
                stored,
                target,
                payload,
                declared,
                realized,
                applied,
            )
        };
        // The happy path: Settled, canonical policy type, stored type
        // matches, payload == declared == realized, never applied.
        assert!(ok(
            STATE_SETTLED,
            POLICY_TRANSITION_TYPE,
            target,
            payload,
            payload,
            false
        )
        .is_ok());
        // Not settled (Committed, or any pre-settle state): BadState.
        assert!(ok(
            STATE_COMMITTED,
            POLICY_TRANSITION_TYPE,
            target,
            payload,
            payload,
            false
        )
        .is_err());
        // Already executed once: BadState — replay is refused even
        // when every other check would pass, so an old amendment
        // cannot roll back a newer one.
        assert!(ok(
            STATE_SETTLED,
            POLICY_TRANSITION_TYPE,
            target,
            payload,
            payload,
            true
        )
        .is_err());
        // Wrong source type: a settled window transition is not a
        // policy amendment, however clean its settle.
        assert!(ok(
            STATE_SETTLED,
            [9u8; 32],
            target,
            payload,
            payload,
            false
        )
        .is_err());
        // Target record names another type than the amendment targets.
        assert!(ok(
            STATE_SETTLED,
            POLICY_TRANSITION_TYPE,
            [4u8; 32],
            payload,
            payload,
            false
        )
        .is_err());
        // Payload hash differs from the declared hash: values
        // invented at execution time, not declared at registration.
        assert!(ok(
            STATE_SETTLED,
            POLICY_TRANSITION_TYPE,
            target,
            [12u8; 32],
            [12u8; 32],
            false
        )
        .is_err());
        // Inexact settle: realized differs from declared, so the
        // settled outcome is not the declared amendment.
        assert!(ok(
            STATE_SETTLED,
            POLICY_TRANSITION_TYPE,
            target,
            payload,
            [13u8; 32],
            false
        )
        .is_err());
    }

    #[test]
    fn per_type_policy_values_govern_the_lifecycle_gates() {
        // A PolicyRecord whose caps differ from the launch constants:
        // the lifecycle gates must follow the record, not the launch
        // values. The handlers pass exactly these fields to the pure
        // gates (and the IDL carries the policy_record account on all
        // four instructions), so this test plus the IDL check is the
        // wiring evidence for the slice.
        let policy = PolicyRecord {
            transition_type: [3; 32],
            quorum_required: 1_000_000,
            confidence_threshold: 500_000,
            min_obs_confidence: 500_000,
            require_objectives: false,
            planning_window: 999,
            max_rounds: 1,
            max_plans: 2,
            mandatory_constraints: vec![],
            required_capabilities: vec![],
            bump: 0,
        };
        assert!(validate_policy(
            policy.confidence_threshold,
            policy.max_rounds,
            policy.max_plans,
            &policy.mandatory_constraints,
            &policy.required_capabilities,
        )
        .is_ok());
        // The deadline is now + the record's window, not the launch one.
        assert_eq!(planning_deadline(1_000, policy.planning_window).unwrap(), 1_999);
        assert_ne!(
            policy.planning_window,
            kernel_bridge::PLANNING_WINDOW,
            "test fixture must differ from the launch window or it proves nothing"
        );
        // Plan cap: 2 plans already submitted refuses at max 2,
        // admits at the launch max 4.
        assert!(can_submit_plan(
            true,
            STATE_PLANNING,
            1_000,
            1_999,
            2,
            policy.max_plans,
            2_000,
        )
        .is_err());
        assert!(can_submit_plan(
            true,
            STATE_PLANNING,
            1_000,
            1_999,
            2,
            kernel_bridge::launch_policy().max_plans,
            2_000,
        )
        .is_ok());
        // Round cap: round 1 is exhausted at max 1, live at max 3.
        assert!(can_submit_signal(
            true,
            STATE_PLANNING,
            500_000,
            1,
            policy.max_rounds,
            1,
        )
        .is_err());
        assert!(can_submit_signal(
            true,
            STATE_PLANNING,
            500_000,
            1,
            kernel_bridge::launch_policy().max_rounds,
            1,
        )
        .is_ok());
        assert!(can_advance_round(STATE_PLANNING, 1, 1, policy.max_rounds).is_err());
        assert!(can_advance_round(
            STATE_PLANNING,
            1,
            1,
            kernel_bridge::launch_policy().max_rounds,
        )
        .is_ok());
    }

    #[test]
    fn commit_mask_inputs_follow_the_type_policy_record() {
        // A PolicyRecord whose mask inputs differ from the launch
        // constants: the commit mask must follow the record, not the
        // launch values. The handler passes exactly these four fields
        // into `CommitInputs` (and the IDL carries the policy_record
        // account on `commit`), so this test plus the IDL check is the
        // wiring evidence for the slice; the handler itself is not
        // unit-testable on host (Anchor context), and this test does
        // not pretend otherwise. Every other bit is clear in both
        // runs, so the two masks differ only by the four policy bits.
        let policy = PolicyRecord {
            transition_type: [3; 32],
            quorum_required: 2_000_000,
            confidence_threshold: 900_000,
            min_obs_confidence: 900_000,
            require_objectives: true,
            planning_window: 999,
            max_rounds: 1,
            max_plans: 2,
            mandatory_constraints: vec![],
            required_capabilities: vec![],
            bump: 0,
        };
        let launch = kernel_bridge::launch_policy();
        assert_ne!(policy.quorum_required, launch.quorum_required.raw());
        assert_ne!(
            policy.confidence_threshold,
            launch.confidence_threshold.raw()
        );
        assert_ne!(
            policy.min_obs_confidence,
            launch.min_obs_confidence.raw()
        );
        assert_ne!(policy.require_objectives, launch.require_objectives);
        // Round: total 1.0, weighted support 0.5 — clears the launch
        // quorum (1.0) and confidence (0.5), fails the record's (2.0,
        // 0.9). Observation confidence 0.5 clears the launch minimum
        // (0.5), fails the record's (0.9). Objectives unreported:
        // clear when not required (launch), set when required (record).
        let mask_for = |require_objectives: bool,
                        quorum_required: u64,
                        confidence_threshold: u64,
                        min_obs_confidence: u64| {
            let statuses = [STATUS_PASS];
            commit_fail_mask(&CommitInputs {
                constraint_statuses: &statuses,
                objectives_ok: false,
                require_objectives,
                round_total: 1_000_000,
                round_weighted: 500_000,
                quorum_required,
                confidence_threshold,
                reconciled: true,
                fits: true,
                unresolved_blocking: 0,
                obs_timestamp: 900,
                obs_expiry: 2_000,
                obs_confidence: 500_000,
                min_obs_confidence,
                now: 1_000,
                planning_deadline: 900,
                expires_at: 2_000,
                parent_index: 0,
                parent_hash: [0; 32],
                chain_index: 0,
                chain_hash: [0; 32],
                plan_count: 1,
                min_plan_expiry: Some(2_000),
                capability_covered: true,
            })
        };
        assert_eq!(
            mask_for(
                policy.require_objectives,
                policy.quorum_required,
                policy.confidence_threshold,
                policy.min_obs_confidence,
            ),
            MASK_OBJECTIVES | MASK_QUORUM | MASK_CONFIDENCE | MASK_FRESH
        );
        assert_eq!(
            mask_for(
                launch.require_objectives,
                launch.quorum_required.raw(),
                launch.confidence_threshold.raw(),
                launch.min_obs_confidence.raw(),
            ),
            0
        );
    }

    #[test]
    fn treasury_init_type_id_is_sha256_of_domain_tag() {
        // The canonical type id is recomputed from the string, not
        // trusted as a literal — the same discipline as the policy
        // type id. (Cross-checked: this hash function reproduces the
        // existing POLICY_TRANSITION_TYPE bytes for its own tag.)
        let computed =
            anchor_lang::solana_program::hash::hash(b"treasury.init.v1").to_bytes();
        assert_eq!(TREASURY_INIT_TYPE, computed);
        assert_ne!(TREASURY_INIT_TYPE, POLICY_TRANSITION_TYPE);
    }

    #[test]
    fn treasury_init_gate_matrix() {
        // Settled + canonical type + vault bound: passes.
        can_initialize_treasury(STATE_SETTLED, TREASURY_INIT_TYPE, true)
            .expect("settled init transition authorizes init");
        // Any non-Settled state refuses first, whatever the rest is.
        assert!(can_initialize_treasury(STATE_COMMITTED, TREASURY_INIT_TYPE, true).is_err());
        assert!(can_initialize_treasury(STATE_PLANNING, TREASURY_INIT_TYPE, true).is_err());
        // A settled transition of another type is not an init source.
        assert!(can_initialize_treasury(STATE_SETTLED, POLICY_TRANSITION_TYPE, true).is_err());
        assert!(can_initialize_treasury(STATE_SETTLED, [0; 32], true).is_err());
        // A vault key that does not match Config.fee_vault refuses.
        assert!(can_initialize_treasury(STATE_SETTLED, TREASURY_INIT_TYPE, false).is_err());
    }

    #[test]
    fn treasury_launch_type_id_is_sha256_of_domain_tag() {
        // The canonical type id is recomputed from the string, not
        // trusted as a literal — the same discipline as the init and
        // policy type ids. (Cross-checked: this hash function
        // reproduces the existing POLICY_TRANSITION_TYPE and
        // TREASURY_INIT_TYPE bytes for their own tags.)
        let computed =
            anchor_lang::solana_program::hash::hash(b"treasury.launch.v1").to_bytes();
        assert_eq!(TREASURY_LAUNCH_TYPE, computed);
        assert_ne!(TREASURY_LAUNCH_TYPE, TREASURY_INIT_TYPE);
        assert_ne!(TREASURY_LAUNCH_TYPE, POLICY_TRANSITION_TYPE);
    }

    #[test]
    fn treasury_launch_gate_matrix() {
        // Settled + canonical type + Stage 1 + threshold set: passes.
        // (The threshold value itself is not judged here — any
        // nonzero B passes this gate; whether B is met is the
        // BACKING_EVIDENCED attestation judged at commit.)
        can_launch_stage2(STATE_SETTLED, TREASURY_LAUNCH_TYPE, STAGE_1, 500_000)
            .expect("settled launch transition authorizes launch");
        // Any non-Settled state refuses first, whatever the rest is.
        assert!(can_launch_stage2(STATE_COMMITTED, TREASURY_LAUNCH_TYPE, STAGE_1, 500_000).is_err());
        assert!(can_launch_stage2(STATE_PLANNING, TREASURY_LAUNCH_TYPE, STAGE_1, 500_000).is_err());
        // A settled transition of another type is not a launch source.
        assert!(can_launch_stage2(STATE_SETTLED, TREASURY_INIT_TYPE, STAGE_1, 500_000).is_err());
        assert!(can_launch_stage2(STATE_SETTLED, POLICY_TRANSITION_TYPE, STAGE_1, 500_000).is_err());
        assert!(can_launch_stage2(STATE_SETTLED, [0; 32], STAGE_1, 500_000).is_err());
        // A stage that is no longer Stage 1 refuses: the flip is the
        // one-time guarantee, so a second launch changes nothing.
        assert!(can_launch_stage2(STATE_SETTLED, TREASURY_LAUNCH_TYPE, STAGE_2, 500_000).is_err());
        // An unset threshold (0, D-2 open) refuses: an unset B must
        // never read as "any backing passes".
        assert!(can_launch_stage2(STATE_SETTLED, TREASURY_LAUNCH_TYPE, STAGE_1, 0).is_err());
    }

    #[test]
    fn treasury_sleeve_split_sums_exactly() {
        // 50/25/25 at the $100M reference scale (raw units).
        assert_eq!(split_sleeves(100_000_000), (25_000_000, 50_000_000, 25_000_000));
        // Zero splits to zero.
        assert_eq!(split_sleeves(0), (0, 0, 0));
        // Odd total: floored cash/arb, bonds absorbs the remainder,
        // and the sleeves sum to exactly the total — nothing created
        // or destroyed by rounding.
        let (cash, bonds, arb) = split_sleeves(101);
        assert_eq!((cash, arb), (25, 25));
        assert_eq!(bonds, 51);
        assert_eq!(cash + bonds + arb, 101);
        // The sleeve constants sum to the whole treasury.
        assert_eq!(
            CASH_SLEEVE_PPM + BONDS_SLEEVE_PPM + ARB_SLEEVE_PPM,
            1_000_000
        );
    }

    #[test]
    fn treasury_account_spaces_match_fields() {
        assert_eq!(TreasuryState::SPACE, 57);
        assert_eq!(LendingBook::SPACE, 21);
        assert_eq!(FeeVault::SPACE, 17);
    }
}
