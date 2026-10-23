//! # Coordination Kernel (Rust reference)
//!
//! Mirrors `CoordinationKernel.sol`. One canonical predicate, [`Kernel::commit_fail_mask`],
//! returns a bitset of failed terms using the same `F_*` values as the Solidity line.
//! `commit()` is `mask == 0`; nothing else decides.
//!
//! * The kernel owns every input to the predicate. Callers never assert quorum, constraints,
//!   exceptions or plans; they submit signals/plans/reports through guarded entry points.
//! * Quantities are non-negative [`Amount`]s with checked u128 math; nothing saturates silently.
//! * Time is monotonic: every entry point takes `now` and rejects regression.
//! * Declared and realized state are separate: commit advances the declared chain, settlement
//!   records the realized hash and applies attribution.
//! * `std` is a feature only for float conveniences; the kernel builds `no_std + alloc`.

#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

mod kernel;
mod num;

pub use kernel::*;
pub use num::*;
