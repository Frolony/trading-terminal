//! # Treasury policy profile for the Coordination Kernel (roadmap W2.4)
//!
//! This crate configures Jeffrey Beitel's `coordination-kernel` for the retail
//! liquidity engine's treasury deployment. It does not modify the kernel; it
//! supplies the administrator hooks the kernel was designed to receive.
//!
//! ## The profile
//!
//! * **Attestation — evidence-bound, settle-after-realized.** The default
//!   `RoleAttestor` lets any Oracle-role caller settle with any realized hash
//!   it asserts. Our [`EvidenceAttestor`] is stricter: a settlement is admitted
//!   only if the presented realized hash equals the hash the attestor
//!   *independently recorded as evidence* for that transition — evidence that
//!   can only be recorded from observing the realized outcome, i.e. after it
//!   exists. No evidence on record, or a mismatched claim: denied, fail-closed,
//!   and the transition stays `Committed` for a correct settlement to retry.
//!   This is the mechanical form of the project's founding observation:
//!   attestation that precedes (or contradicts) the realized outcome is
//!   verification theater, and here it simply does not compile into a settle.
//!
//!   Observations stay role-based (Oracle may observe), matching the default.
//!
//! * **Weights — flat at launch.** [`FlatWeights`]: R = GPolicy = 1.0 and
//!   influence is the administrator-set profile value. Reputation is *recorded*
//!   from the first settlement (attribution runs as the kernel defines it) but
//!   has **zero influence** on quorum until governance — not a formula — decides
//!   otherwise. This is the anti-weight-gaming stance adopted 2026-10-22: no
//!   declarative quantity in this deployment gates anything.
//!
//! * **Capacity stocks** are named per resource (see constants below) so plan
//!   claims read as treasury language, not bare ids.
//!
//! * **Exceptions:** the EXCEPTIONS predicate bit is unused at pilot — no
//!   standing exception policy exists until governance creates one through the
//!   kernel's own lifecycle.
//!
//! The full prose profile, including the contention recommendation (W2.5,
//! awaiting the kernel author's ruling), lives in `KERNEL_POLICY_PROFILE.md`
//! at the package root.

use coordination_kernel::*;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// Capacity stock: treasury distribution tranches (monthly cohort payouts).
pub const RES_TREASURY_TRANCHE: ResourceId = 0;
/// Capacity stock: the secured lending book (rotational borrowing).
pub const RES_LENDING_BOOK: ResourceId = 1;
/// Capacity stock: rotational exit windows.
pub const RES_EXIT_WINDOW: ResourceId = 2;

/// An [`Attestor`] that settles only against independently recorded evidence.
///
/// The evidence log is the attestor's own record of realized outcomes, written
/// by the observing process *after* the outcome exists (`record_evidence`).
/// `may_settle` compares the settlement's claimed realized hash against that
/// record.
///
/// The kernel takes ownership of the attestor at construction and offers no
/// accessor, so the log lives behind an `Rc`: **clone the attestor before
/// handing one clone to the kernel** — every clone shares the same log, and
/// the clone you keep is the observing process's write handle. (Single-threaded
/// reference implementation, like the kernel itself; a production port would
/// put the log behind whatever shared state its runtime provides.)
#[derive(Debug, Default, Clone)]
pub struct EvidenceAttestor {
    evidence: Rc<RefCell<BTreeMap<TransitionId, Hash32>>>,
}

impl EvidenceAttestor {
    pub fn new() -> Self { Self::default() }

    /// Record an observed realized outcome for a transition. In deployment this
    /// is called by the attesting process once the outcome exists in the world
    /// (e.g. the settled transaction is final on-chain) — never before, and
    /// the record is what a later settlement is checked against.
    pub fn record_evidence(&self, id: TransitionId, realized: Hash32) {
        self.evidence.borrow_mut().insert(id, realized);
    }

    /// The recorded realized hash for a transition, if any.
    pub fn evidence_of(&self, id: &TransitionId) -> Option<Hash32> {
        self.evidence.borrow().get(id).copied()
    }
}

impl Attestor for EvidenceAttestor {
    fn may_observe(&self, caller: &CallerRoles, _who: &AgentId, _o: &Observation) -> bool {
        caller.has(Role::Oracle)
    }
    fn may_settle(
        &self,
        caller: &CallerRoles,
        _who: &AgentId,
        id: &TransitionId,
        realized: &Hash32,
        _attributions: &[(AgentId, Delta)],
    ) -> bool {
        // Fail closed: role AND evidence AND exact match. Anything less is the
        // rubber-stamp pattern this profile exists to refuse.
        caller.has(Role::Oracle) && self.evidence_of(id) == Some(*realized)
    }
}

/// The launch kernel type: evidence-bound attestation, flat weights.
pub type LaunchKernel = Kernel<EvidenceAttestor, FlatWeights>;

/// Construct the launch-profile kernel around a shared evidence attestor.
///
/// Pass `attestor.clone()` (or the original) — the kernel's copy and the copy
/// the caller keeps share one evidence log; see [`EvidenceAttestor`].
///
/// * `admin` — the founding administrator agent (holds the Admin role).
/// * `genesis` — the declared genesis system state.
/// * Reputation bounds 0..=100 with per-settlement attribution capped at 20 —
///   the same bounds the crate's own test profile uses; changing them is a
///   governance act, not a code edit someone slips in.
pub fn launch_kernel(admin: AgentId, genesis: SystemState, attestor: EvidenceAttestor) -> Result<LaunchKernel, KernelError> {
    Kernel::with_policies(
        admin,
        genesis,
        Config {
            rep_min: Amount::ZERO,
            rep_max: Amount::units(100)?,
            max_attribution: Amount::units(20)?,
        },
        attestor,
        FlatWeights,
    )
}
