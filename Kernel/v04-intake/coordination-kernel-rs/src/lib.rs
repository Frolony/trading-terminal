//! # Coordination Kernel (Rust reference, v0.4)
//!
//! Mirrors the amalgamated Solidity kernel (`reference/`). One canonical predicate,
//! [`Kernel::commit_fail_mask`], returns a bitset of failed terms with the same `F_*`
//! values as Solidity. A commit succeeds iff the mask is zero; nothing else decides.
//!
//! * The kernel owns every input to the predicate. Callers never assert quorum, constraints,
//!   exceptions or plans; they submit signals and plans through guarded entry points.
//! * Rules change only through governance transitions that pass the whole predicate
//!   (after a council that agreed unanimously). The genesis authority exists only until the seal.
//! * Position gates council candidacy and nothing else: it never weights a signal or a commit.
//! * Quantities are non-negative [`Amount`]s with checked u128 math; nothing saturates silently.
//! * Time is monotonic: every entry point takes `now` and rejects regression.
//! * The kernel builds `no_std + alloc`.

#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

mod classify;
mod kernel;
mod num;

pub use classify::*;
pub use kernel::*;
pub use num::*;
