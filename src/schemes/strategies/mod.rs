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
pub trait Strategy {
    fn name(&self) -> &str;
    fn on_bar(&mut self, bar: &Bar);
    fn should_enter(&self) -> bool;
    fn should_exit(&self, bars_held: usize) -> bool;
    /// Optional current trend/signal for logging.
    fn signal(&self) -> i8 {
        0
    }
}
