//! Strategy Lab Playbook top-10 schemes, ported for long-only daily execution.
//!
//! Each strategy exposes:
//! - `on_bar(&mut self, bar)` → updates internal state
//! - `should_enter(&self) -> bool`
//! - `should_exit(&self, bars_held: usize) -> bool`
//!
//! Position management (95% deploy, one position, fees) is left to the executor.
//! Time-stop is expressed in calendar/trading bars held.

pub mod aroon_timestop;
pub mod bollinger_gated;
pub mod ema_cross;
pub mod kama_cross;
pub mod supertrend_timestop;

pub use aroon_timestop::AroonTimeStop;
pub use bollinger_gated::DriftGatedBollinger;
pub use ema_cross::Ema20_50;
pub use kama_cross::KamaCross;
pub use supertrend_timestop::SupertrendTimeStop;

use crate::indicators::Bar;

/// Common interface for lab strategies.
///
/// Vendored 2026-10-22 from hidden_files/strategy-upload/jupiter-strategies
/// (N7.2 scheme library). One divergence from the upload: the `Send`
/// supertrait. The terminal's backtest handler holds the boxed strategy across
/// `.await` points, and vercel_runtime's `service_fn` requires the handler
/// future to be `Send` — without the bound the N7.2 build fails with E0277.
/// All five strategy structs are plain data (no interior mutability), so the
/// bound is always satisfiable.
pub trait Strategy: Send {
    fn name(&self) -> &str;
    fn on_bar(&mut self, bar: &Bar);
    fn should_enter(&self) -> bool;
    fn should_exit(&self, bars_held: usize) -> bool;
    /// Optional current trend/signal for logging.
    fn signal(&self) -> i8 {
        0
    }
}
