//! Entrance/exit boundary judgment: the second half of the S13 blob-rendering rule.
//!
//! Carlos's direction (2026-10-24 ~02:26, filed as the §6 S13 addendum in
//! the master plan): a new blob spawns only at a genuine entrance (value
//! arriving from a persistent, unrelated source) or exit (value leaving the
//! cluster for good). A dispersal slice graduates to its own exit blob only
//! when the recipient itself crosses a boundary — "an exit is declared when
//! money leaves the story, not when it moves."
//!
//! [`exit_declared`] is that judgment as a pure function of the recipient's
//! observed onward activity. It decides nothing from a single hop: a slice
//! that merely moves on — to another single-use intermediate, or to another
//! wallet inside the cluster — is still in the story. Only a crossing
//! graduates it, and the crossing kinds are exactly the rule's list:
//!
//! * **Burn** of the received mint — the value is destroyed, not moved.
//! * **Asset change** — the received mint is swapped for another; the story
//!   of the first asset ends here (the swap's output leg is a new blob's
//!   entrance, judged separately).
//! * **Persistent destination** — an onward transfer to a wallet outside the
//!   cluster that has its own history beyond this flow. Persistence is
//!   caller-supplied evidence (`destination_persistent`), never inferred
//!   from one hop: a destination seen exactly once is a relay hop, not a
//!   home.
//! * **Off-cluster deposit** — an explicit deposit leg out of the cluster
//!   (exchange deposit, vault, bridge out), which ends the story even
//!   before the destination's own history is known.
//!
//! Fail-closed, in both directions: no onward activity means no exit (the
//! slice stays unclassified-as-exit, never a guessed exit), and a crossing
//! is reported with its kind so the renderer can say *how* the money left.
//! [`entrance_declared`] is the mirror half: value arriving from a
//! persistent source outside the cluster is a genuine entrance; value from
//! a single-use source, or from inside the cluster, is a continuation of
//! the existing story, not a new blob.
//!
//! This module judges boundaries only; it labels nothing (see
//! [`crate::classify`]) and it does not collapse anything (see
//! [`crate::blob`]). Real-route validation is still the W6.2 gate item —
//! the fixtures in `tests/boundary.rs` are synthetic shapes, not mainnet
//! claims.

/// One observed onward hop by the recipient of a dispersal slice.
#[derive(Debug, Clone)]
pub struct OnwardHop {
    /// Destination wallet of the onward hop (empty for a burn).
    pub to: String,
    /// Mint moved by the onward hop.
    pub mint: String,
    /// What the onward hop does.
    pub kind: OnwardKind,
    /// Caller-supplied: the destination has history beyond this flow
    /// (repeated appearances, its own unrelated activity). False for a
    /// single-use intermediate. Ignored for burns.
    pub destination_persistent: bool,
    /// Caller-supplied: the destination belongs to the cluster under study.
    pub destination_in_cluster: bool,
}

/// The shape of one onward hop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnwardKind {
    /// Plain transfer of the same mint onward.
    Transfer,
    /// The received mint is burned.
    Burn,
    /// The received mint is swapped for another mint.
    AssetChange,
    /// An explicit deposit leg out of the cluster.
    OffClusterDeposit,
}

/// How value left the story, when it did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossingKind {
    /// The received mint was burned.
    Burn,
    /// The received mint was swapped for another asset.
    AssetChange,
    /// The value arrived at a persistent wallet outside the cluster.
    PersistentDestination,
    /// An explicit deposit leg left the cluster.
    OffClusterDeposit,
}

/// The boundary verdict for one dispersal slice.
#[derive(Debug, Clone)]
pub enum BoundaryDecision {
    /// The slice graduated to its own exit blob: the money left the story
    /// by this crossing, at this onward hop index.
    Exit {
        kind: CrossingKind,
        hop_index: usize,
    },
    /// No crossing observed: the slice is still in the story. The reason
    /// names what was observed instead of a crossing.
    StillInStory {
        reason: String,
    },
}

/// Judge whether a dispersal slice's recipient crossed an exit boundary.
///
/// `received_mint` is the mint the slice delivered. Onward hops are scanned
/// in causal order and the FIRST crossing wins — later hops belong to the
/// exit blob's own story, not to this judgment. A hop that merely moves the
/// value on (same mint, inside the cluster, or to a single-use destination)
/// is skipped, not judged: the recipient may still cross later.
pub fn exit_declared(received_mint: &str, onward: &[OnwardHop]) -> BoundaryDecision {
    if onward.is_empty() {
        return BoundaryDecision::StillInStory {
            reason: "no onward activity observed; an exit is declared when money leaves the story, not when it moves".to_string(),
        };
    }
    for (i, hop) in onward.iter().enumerate() {
        match hop.kind {
            OnwardKind::Burn if hop.mint == received_mint => {
                return BoundaryDecision::Exit { kind: CrossingKind::Burn, hop_index: i };
            }
            OnwardKind::AssetChange if hop.mint != received_mint => {
                return BoundaryDecision::Exit { kind: CrossingKind::AssetChange, hop_index: i };
            }
            OnwardKind::OffClusterDeposit if !hop.destination_in_cluster => {
                return BoundaryDecision::Exit { kind: CrossingKind::OffClusterDeposit, hop_index: i };
            }
            OnwardKind::Transfer
                if !hop.destination_in_cluster && hop.destination_persistent =>
            {
                return BoundaryDecision::Exit {
                    kind: CrossingKind::PersistentDestination,
                    hop_index: i,
                };
            }
            _ => {
                // A move, not a crossing: keep scanning.
            }
        }
    }
    BoundaryDecision::StillInStory {
        reason: "onward hops move the value on but none crosses a boundary (no burn, asset change, persistent outside destination, or off-cluster deposit)".to_string(),
    }
}

/// The source of an inbound slice, for the entrance half of the rule.
#[derive(Debug, Clone, Copy)]
pub struct SourceInfo {
    /// The source has history beyond this flow (repeated appearances, its
    /// own unrelated activity). Caller-supplied evidence, never inferred
    /// from one hop.
    pub persistent: bool,
    /// The source belongs to the cluster under study.
    pub in_cluster: bool,
}

/// Judge whether an inbound slice is a genuine entrance: value arriving
/// from a persistent, unrelated source. A single-use source is a relay hop
/// continuing an existing story; an in-cluster source is the cluster paying
/// itself. Both are continuations, not entrances.
pub fn entrance_declared(source: &SourceInfo) -> bool {
    source.persistent && !source.in_cluster
}
