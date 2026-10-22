//! #1 Supertrend(10,3) + 25d time-stop [XRP] and #3/#10 Supertrend(7,2) variants.
//! Entry on bullish Supertrend, exit on bearish flip OR max_hold bars.

use crate::indicators::{Bar, Supertrend};
use crate::strategies::Strategy;

pub struct SupertrendTimeStop {
    name: String,
    st: Supertrend,
    max_hold: usize,
    prev_trend: i8,
    current_trend: i8,
}

impl SupertrendTimeStop {
    pub fn new(atr_period: usize, mult: f64, max_hold: usize) -> Self {
        Self {
            name: format!("Supertrend({},{}) + {}d time-stop", atr_period, mult, max_hold),
            st: Supertrend::new(atr_period, mult),
            max_hold,
            prev_trend: 0,
            current_trend: 0,
        }
    }

    /// Playbook #1: Supertrend(10,3) + 25-day time-stop (originally XRP; portable to SOL).
    pub fn xrp_style_10_3_25() -> Self {
        Self::new(10, 3.0, 25)
    }

    /// Playbook #3 / #10: tighter Supertrend(7,2) pure (no explicit time-stop, or set large).
    pub fn tight_7_2(max_hold: usize) -> Self {
        Self::new(7, 2.0, max_hold)
    }
}

impl Strategy for SupertrendTimeStop {
    fn name(&self) -> &str {
        &self.name
    }

    fn on_bar(&mut self, bar: &Bar) {
        self.prev_trend = self.current_trend;
        self.current_trend = self.st.update(bar);
    }

    fn should_enter(&self) -> bool {
        // Enter on fresh bullish flip or while already in bull and flat externally.
        // Lab: enter long on bullish signal at close.
        self.current_trend == 1 && self.prev_trend <= 0
    }

    fn should_exit(&self, bars_held: usize) -> bool {
        // Exit on ST flip to bear OR time-stop.
        (self.current_trend == -1 && self.prev_trend >= 0) || bars_held >= self.max_hold
    }

    fn signal(&self) -> i8 {
        self.current_trend
    }
}
