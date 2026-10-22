//! Technical indicators used by Strategy Lab Playbook top schemes.
//! All implementations operate on daily OHLC bars (close-focused where original recipes specify).
//! Formulas match standard definitions used in the lab (Supertrend ATR Wilder, Aroon Chande, KAMA Kaufman, etc.).

pub mod aroon;
pub mod bollinger;
pub mod ema;
pub mod kama;
pub mod supertrend;

pub use aroon::Aroon;
pub use bollinger::BollingerBands;
pub use ema::Ema;
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
