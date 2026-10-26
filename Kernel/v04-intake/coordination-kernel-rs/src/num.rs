//! Non-negative fixed-point arithmetic. Every operation is checked; nothing saturates silently.

use crate::KernelError;

/// Fixed-point scale (6 decimals): `Amount::raw() == value * SCALE`.
pub const SCALE: u64 = 1_000_000;

/// Signed reputation adjustment, same scale as `Amount`.
pub type Delta = i64;

/// A non-negative fixed-point quantity (weight, claim, capacity, quorum, influence,
/// reputation, confidence). Negative values are unrepresentable.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Amount(u64);

impl Amount {
    pub const ZERO: Amount = Amount(0);
    /// 1.0
    pub const ONE: Amount = Amount(SCALE);

    #[inline]
    pub const fn from_raw(raw: u64) -> Self {
        Amount(raw)
    }
    #[inline]
    pub const fn raw(self) -> u64 {
        self.0
    }
    /// Whole units, e.g. `Amount::units(5)` == 5.0.
    pub fn units(n: u64) -> Result<Self, KernelError> {
        n.checked_mul(SCALE).map(Amount).ok_or(KernelError::Overflow)
    }
    pub fn checked_add(self, o: Amount) -> Result<Amount, KernelError> {
        self.0.checked_add(o.0).map(Amount).ok_or(KernelError::Overflow)
    }
    pub fn checked_sub(self, o: Amount) -> Result<Amount, KernelError> {
        self.0.checked_sub(o.0).map(Amount).ok_or(KernelError::Overflow)
    }
    /// `self * o / SCALE`, computed in u128, floored, overflow-checked.
    pub fn checked_mul(self, o: Amount) -> Result<Amount, KernelError> {
        let p = (self.0 as u128) * (o.0 as u128) / (SCALE as u128);
        u64::try_from(p).map(Amount).map_err(|_| KernelError::Overflow)
    }
    /// `self * num / den`, computed in u128, floored. `den == 0` is an error.
    pub fn checked_mul_div(self, num: Amount, den: Amount) -> Result<Amount, KernelError> {
        if den.0 == 0 {
            return Err(KernelError::Overflow);
        }
        let p = (self.0 as u128) * (num.0 as u128) / (den.0 as u128);
        u64::try_from(p).map(Amount).map_err(|_| KernelError::Overflow)
    }
    /// Clamp into `[lo, hi]`; requires `lo <= hi` (checked by `Config::new`).
    pub fn clamp_to(self, lo: Amount, hi: Amount) -> Amount {
        if self < lo {
            lo
        } else if self > hi {
            hi
        } else {
            self
        }
    }
    /// Apply a signed delta, clamping the result into `[lo, hi]`. Never overflows.
    pub fn apply_delta(self, d: i128, lo: Amount, hi: Amount) -> Amount {
        let v = (self.0 as i128).saturating_add(d);
        let v = v.clamp(lo.0 as i128, hi.0 as i128);
        Amount(v as u64)
    }

    /// Convenience for tests, logs and tooling. Never used by kernel logic.
    #[cfg(feature = "std")]
    pub fn from_f64(x: f64) -> Option<Amount> {
        if !x.is_finite() || x < 0.0 {
            return None;
        }
        let v = (x * SCALE as f64).round();
        if v >= u64::MAX as f64 {
            return None;
        }
        Some(Amount(v as u64))
    }
    #[cfg(feature = "std")]
    pub fn to_f64(self) -> f64 {
        self.0 as f64 / SCALE as f64
    }
}
