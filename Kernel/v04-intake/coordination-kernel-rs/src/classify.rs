//! The Stance x Contribution classifier. Informational: it builds each agent's
//! niche (specialty) and never enters the consensus arithmetic.
//!
//! The 48 cells are the ones in the uploaded fork, pinned by tests here and, through
//! `tests/parity.rs`, against the packed constants in the Solidity line.

use crate::num::Amount;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stance { Aligned = 0, Opposed = 1, Neutral = 2, Uncertain = 3 }

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Contribution { Substantive = 0, Procedural = 1, Informational = 2, Obstructive = 3, Abstaining = 4, Specialist = 5 }

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Sector {
    Aligned = 0, Misaligned = 1, Collaborative = 2, Divergent = 3, Adversarial = 4,
    Specialist = 5, Disassociated = 6, Abstaining = 7, Uncertain = 8,
}
pub const SECTORS: usize = 9;

use Sector::*;
/// Rows: Aligned, Opposed, Neutral, Uncertain. Columns: Substantive, Procedural,
/// Informational, Obstructive, Abstaining, Specialist.
const SECTOR_TABLE: [[Sector; 6]; 4] = [
    [Collaborative, Aligned,    Aligned,    Divergent,     Abstaining, Specialist],
    [Adversarial,   Misaligned, Misaligned, Adversarial,   Abstaining, Specialist],
    [Divergent,     Uncertain,  Uncertain,  Divergent,     Abstaining, Specialist],
    [Divergent,     Uncertain,  Uncertain,  Disassociated, Abstaining, Specialist],
];

/// Table weights in twentieths (20 = 1.0), exactly the Solidity packing.
const WEIGHT_TWENTIETHS: [[u64; 6]; 4] = [
    [20, 20, 20, 10, 0, 30],
    [20, 20, 20, 20, 0, 30],
    [10, 2, 2, 10, 0, 30],
    [10, 2, 2, 5, 0, 30],
];

/// Band edges are neutral: > 0.6 aligned, < 0.4 opposed. `None` is "no position".
pub fn classify_stance(support: Option<Amount>) -> Stance {
    match support {
        None => Stance::Uncertain,
        Some(s) if s.raw() > Amount::ONE.raw() * 6 / 10 => Stance::Aligned,
        Some(s) if s.raw() < Amount::ONE.raw() * 4 / 10 => Stance::Opposed,
        Some(_) => Stance::Neutral,
    }
}

pub fn sector_of(s: Stance, c: Contribution) -> Sector { SECTOR_TABLE[s as usize][c as usize] }

/// The strength a classified participation adds to the agent's niche score.
pub fn base_weight_of(s: Stance, c: Contribution) -> Amount {
    Amount::from_raw(WEIGHT_TWENTIETHS[s as usize][c as usize] * Amount::ONE.raw() / 20)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bands_and_edges() {
        assert_eq!(classify_stance(None), Stance::Uncertain);
        assert_eq!(classify_stance(Some(Amount::from_raw(600_001))), Stance::Aligned);
        assert_eq!(classify_stance(Some(Amount::from_raw(600_000))), Stance::Neutral);
        assert_eq!(classify_stance(Some(Amount::from_raw(400_000))), Stance::Neutral);
        assert_eq!(classify_stance(Some(Amount::from_raw(399_999))), Stance::Opposed);
    }
    #[test]
    fn spot_cells() {
        assert_eq!(sector_of(Stance::Aligned, Contribution::Substantive), Sector::Collaborative);
        assert_eq!(sector_of(Stance::Uncertain, Contribution::Obstructive), Sector::Disassociated);
        assert_eq!(base_weight_of(Stance::Aligned, Contribution::Specialist), Amount::from_raw(1_500_000));
        assert_eq!(base_weight_of(Stance::Neutral, Contribution::Procedural), Amount::from_raw(100_000));
        assert_eq!(base_weight_of(Stance::Uncertain, Contribution::Obstructive), Amount::from_raw(250_000));
    }
}
