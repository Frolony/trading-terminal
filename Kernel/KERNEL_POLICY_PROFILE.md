# Kernel Policy Profile — Treasury Deployment (W2.4, with W2.5 note)

Status: **implemented and test-verified** in `kernel-policy/` (this package). The prose here is the profile; the crate's tests are its executable form. Changing any line of this profile is a governance act — it goes through Carlos (and, where it touches kernel semantics, Jeffrey), never a quiet code edit.

## 1. Attestation: evidence-bound, settle-after-realized

- Observations are role-based: the Oracle role may post observations (unchanged from the kernel default).
- Settlement requires **both** the Oracle role **and** evidence: the attestor maintains its own log of realized outcomes, recorded by the observing process *after* the outcome exists. A settlement is admitted only if its claimed realized hash exactly equals the recorded evidence for that transition.
- No evidence, wrong hash, or wrong role → refused (`Unauthorized`), and the transition remains `Committed`, unharmed and retryable. Fail-closed, not fail-dead.
- Rationale (the project's founding observation, Carlos 2026-10-22): attestations whose timing cannot line up with the events they certify are verification theater. In this profile, an attestation that precedes the realized outcome cannot settle anything, because there is no evidence to match until the outcome exists — the ordering is enforced by the data dependency, not by trusting the attestor's clock.

Implementation note: the kernel owns the attestor after construction, so the evidence log sits behind a shared handle (`Rc<RefCell<…>>`); the deploying process keeps a clone as its write handle. A production port replaces that with its runtime's shared state; the policy is unchanged.

## 2. Weights: flat at launch

- `FlatWeights`: relevance and group policy are 1.0; influence is the administrator-set profile value.
- Reputation is **recorded from the first settlement** (attribution runs exactly as the kernel defines: summed per agent, capped, drift halves credit only) and has **zero influence** on quorum or eligibility.
- Rationale (Carlos's ruling, 2026-10-22): no declarative quantity gates anything in this deployment. If reputation is ever to influence weight, that change is a governed policy decision taken with attribution history in hand — the kernel's `WeightPolicy` hook is where it would land, and the crate's own test (`weight_policy_changes_quorum_outcome`) proves the hook can do it. Until then, the anti-gaming property is structural: there is no score to game.

## 3. Capacity stocks

Named resources (ids in `kernel-policy`): `RES_TREASURY_TRANCHE` (0) — monthly distribution tranches; `RES_LENDING_BOOK` (1) — the secured lending book; `RES_EXIT_WINDOW` (2) — rotational exit windows. Stock levels are set by the administrator through the kernel's governance entry points and debited atomically at commit, per kernel semantics. (The treasury mathematics that sizes these stocks lives in the engine design: `TREASURY_MATHEMATICAL_LOGIC.md` in the project files.)

## 4. Exceptions

Unused at pilot. The EXCEPTIONS predicate bit exists; no standing exception policy does. The first exception policy, if ever needed, is created through governance and recorded as a profile amendment.

## 5. Reputation bounds

0 to 100, per-settlement attribution capped at ±20 (the crate's own test profile values). Bounds are configuration, set at kernel construction; amending them follows the same governance path as any other line of this profile.

## 6. W2.5 — Contention semantics (OPEN — recommendation recorded, not implemented)

Question: when the plans in a round claim more of a capacity stock than remains, what should happen? The kernel today answers at the predicate level — COMPAT fails, nothing commits — which is safe but silent about *scheduling*: the proposals simply wait.

**Recommendation: fail-closed carry-over.** Over-subscribed rounds do not partially commit and do not scale claims down silently; uncommitted proposals carry to the next round (consistent with the kernel's existing "nothing commits" philosophy — the previous commit carries over when evidence or support is insufficient). Pro-rata scale-down is admissible only for resources that are divisible *and* where the proposal itself declares divisibility in advance.

Status: **awaiting the kernel author's ruling (roadmap item K-1)**, routed through Carlos. Nothing in `kernel-policy` implements contention behavior; this section records the question and the recommendation so the decision, when made, lands on a prepared page.
