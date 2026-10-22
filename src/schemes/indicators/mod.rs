//! Technical indicators used by Strategy Lab Playbook top schemes.
//! All implementations operate on daily OHLC bars (close-focused where original recipes specify).
//! Formulas match standard definitions used in the lab (Supertrend ATR Wilder, Aroon Chande, KAMA Kaufman, etc.).
//!
//! Vendored 2026-10-22 from hidden_files/strategy-upload/jupiter-strategies
//! (N7.2 scheme library). One-line divergence from the upload, fixed here:
//! `pub use ema::EmaCrossover;` — the upload's strategies/ema_cross.rs imports
//! `crate::indicators::EmaCrossover`, but the upload's indicators/mod.rs never
//! re-exported it, so the crate as uploaded does not compile (rustc E0432).
//! Fix confirmed by the Vercel Rust build.

pub mod aroon;
pub mod bollinger;
pub mod ema;
pub mod kama;
pub mod supertrend;

pub use aroon::Aroon;
pub use bollinger::BollingerBands;
pub use ema::{Ema, EmaCrossover};
pub use kama::Kama;
pub use supertrend::Supertrend;

/// Minimal daily bar for indicator calculation.
#[derive(Clone, Debug)]
pub struct Bar {
    pub high: f64,
    pub low: f64,
    pub close: f64,
    /// Optional open if needed later.
    pub open: Option<f64>,
}

impl Bar {
    pub fn new(high: f64, low: f64, close: f64) -> Self {
        Self {
            high,
            low,
            close,
            open: None,
        }
    }
}
