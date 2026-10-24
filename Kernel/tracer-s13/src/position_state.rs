//! Cross-window position state (W6.2, fifth pass).
//!
//! The stateless classifier (see [`crate::classify::position_close_candidate`]
//! — the arm itself is not public, but its limit is documented there) can
//! read a removal leg as a position coming OUT, but it cannot say whether
//! the position is fully closed or only reduced: both are removal legs to
//! one wallet. The distinction needs history the single window does not
//! contain — what the wallet previously ADDED to that pool. This module is
//! that history, kept deliberately small and deliberately fail-closed.
//!
//! ## What the ledger is
//!
//! A record of observed liquidity legs, keyed by `(wallet, pool, mint)`:
//! the wallet that holds the position, the pool account the legs move
//! through, and the mint moved. The caller feeds it legs as the indexer
//! observes them — adds as they open or grow a position, removes as they
//! shrink it — and asks, for a position, what its state is.
//!
//! ## The fail-closed rules (load-bearing, do not soften)
//!
//! * A removal with NO recorded add history reads [`PositionStatus::Unknown`],
//!   never "full close". A position opened before the observation window
//!   started is the common case, not an edge case, and the ledger must not
//!   guess that the observed removal happened to be all of it.
//! * A removal that EXCEEDS the recorded adds also reads `Unknown`: the add
//!   history is then provably incomplete (legs were missed, or the window
//!   started mid-position), so no full/partial claim is justified.
//! * A removal exactly equal to the recorded adds reads
//!   [`PositionStatus::Closed`] — the position is provably zero, but only
//!   against the recorded history, which the caller must have fed
//!   completely for the key. The status is a statement about the ledger,
//!   not about the chain.
//! * Zero-amount legs are ignored: they move nothing and must not create
//!   or close a position record.
//! * Amounts accumulate with saturating arithmetic: a corrupt or adversarial
//!   amount can pin a key at the ceiling, never panic the tracer.
//!
//! ## What the ledger is NOT
//!
//! It does not parse workflows, recognize instruction families, or label
//! anything. It is observation state, not inference. The classifier
//! consults it through [`crate::classify::classify_with_ledger`], which
//! annotates a `PositionClose` verdict's rationale with the ledger reading
//! and changes NEITHER the label NOR the confidence: the annotation refines
//! the sentence for that workflow, it does not upgrade the verdict's
//! strength. Full-vs-partial validation on real labeled mainnet routes is
//! still a W6.2 gate item — this module is the foundation that gate item
//! will be validated against, not the validation itself.

/// The state of one position key, as far as the recorded history shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionStatus {
    /// No complete add history for this key: either nothing was ever
    /// recorded, or removals exceed recorded adds (incomplete history).
    /// Full vs partial is NOT determinable — never a guessed close.
    Unknown,
    /// Recorded adds exceed recorded removes: the position is still open,
    /// with `remaining` base units unremoved according to the ledger.
    Open {
        /// Recorded adds minus recorded removes, in the mint's base units.
        remaining: i128,
    },
    /// Recorded removes exactly equal recorded adds (and adds are
    /// non-zero): the position is provably zero against the ledger.
    Closed,
}

/// The close reading a `PositionClose` verdict consults the ledger for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseReading {
    /// The ledger's recorded adds are fully removed: a full close.
    Full,
    /// The ledger's recorded adds are only partly removed: a reduction,
    /// with the position still open.
    Partial,
    /// The ledger cannot say: no add history, or incomplete history.
    Unknown,
}

/// One position's recorded history.
#[derive(Debug, Clone)]
struct PositionEntry {
    wallet: String,
    pool: String,
    mint: String,
    added: i128,
    removed: i128,
}

/// Cross-window ledger of observed liquidity legs.
///
/// Keyed by `(wallet, pool, mint)`. Feed legs in causal order as the
/// indexer observes them; query with [`PositionLedger::status`] or
/// [`PositionLedger::close_reading`]. There is no removal of entries: a
/// closed position stays `Closed` in the ledger, which is what lets a later
/// window distinguish "closed earlier" from "never seen".
#[derive(Debug, Clone, Default)]
pub struct PositionLedger {
    entries: Vec<PositionEntry>,
}

impl PositionLedger {
    /// An empty ledger: every key reads [`PositionStatus::Unknown`].
    pub fn new() -> Self {
        Self::default()
    }

    fn entry_mut(&mut self, wallet: &str, pool: &str, mint: &str) -> &mut PositionEntry {
        if let Some(i) = self
            .entries
            .iter()
            .position(|e| e.wallet == wallet && e.pool == pool && e.mint == mint)
        {
            &mut self.entries[i]
        } else {
            self.entries.push(PositionEntry {
                wallet: wallet.to_string(),
                pool: pool.to_string(),
                mint: mint.to_string(),
                added: 0,
                removed: 0,
            });
            self.entries.last_mut().expect("entry just pushed")
        }
    }

    fn entry(&self, wallet: &str, pool: &str, mint: &str) -> Option<&PositionEntry> {
        self.entries
            .iter()
            .find(|e| e.wallet == wallet && e.pool == pool && e.mint == mint)
    }

    /// Record an observed liquidity ADD leg: `wallet` put `amount` of
    /// `mint` into `pool`. Zero or negative amounts are ignored — a leg
    /// that moves nothing is not a position.
    pub fn record_add(&mut self, wallet: &str, pool: &str, mint: &str, amount: i128) {
        if amount <= 0 {
            return;
        }
        let entry = self.entry_mut(wallet, pool, mint);
        entry.added = entry.added.saturating_add(amount);
    }

    /// Record an observed liquidity REMOVE leg: `amount` of `mint` left
    /// `pool` for `wallet`. Zero or negative amounts are ignored. A remove
    /// with no prior add creates an entry with `added == 0`, which reads
    /// [`PositionStatus::Unknown`] — the fail-closed rule, not an error.
    pub fn record_remove(&mut self, wallet: &str, pool: &str, mint: &str, amount: i128) {
        if amount <= 0 {
            return;
        }
        let entry = self.entry_mut(wallet, pool, mint);
        entry.removed = entry.removed.saturating_add(amount);
    }

    /// The state of one position key against the recorded history.
    pub fn status(&self, wallet: &str, pool: &str, mint: &str) -> PositionStatus {
        let Some(entry) = self.entry(wallet, pool, mint) else {
            return PositionStatus::Unknown;
        };
        if entry.added == 0 || entry.removed > entry.added {
            // No add history, or provably incomplete add history: no claim.
            return PositionStatus::Unknown;
        }
        if entry.removed == entry.added {
            return PositionStatus::Closed;
        }
        PositionStatus::Open { remaining: entry.added - entry.removed }
    }

    /// The close reading for one position key: what a `PositionClose`
    /// verdict's rationale may honestly add from this ledger.
    pub fn close_reading(&self, wallet: &str, pool: &str, mint: &str) -> CloseReading {
        match self.status(wallet, pool, mint) {
            PositionStatus::Closed => CloseReading::Full,
            PositionStatus::Open { .. } => CloseReading::Partial,
            PositionStatus::Unknown => CloseReading::Unknown,
        }
    }
}
