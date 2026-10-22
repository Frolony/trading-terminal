//! #4 Drift-gated Bollinger(20,2) on SOL.
//! Enter long on lower band only when |close/SMA200-1| <= 0.25; exit on middle band.

use crate::indicators::{Bar, BollingerBands};
use crate::strategies::Strategy;

pub struct DriftGatedBollinger {
    bb: BollingerBands,
    last_close: f64,
}

impl DriftGatedBollinger {
    pub fn new() -> Self {
        Self {
            bb: BollingerBands::drift_gated_20_2(),
            last_close: 0.0,
        }
    }
}

impl Default for DriftGatedBollinger {
    fn default() -> Self {
        Self::new()
    }
}

impl Strategy for DriftGatedBollinger {
    fn name(&self) -> &str {
        "Drift-gated Bollinger(20,2)"
    }

    fn on_bar(&mut self, bar: &Bar) {
        self.bb.update(bar);
        self.last_close = bar.close;
    }

    fn should_enter(&self) -> bool {
        self.bb.entry_signal(self.last_close)
    }

    fn should_exit(&self, _bars_held: usize) -> bool {
        // Always-honored middle-band exit
        self.bb.exit_signal(self.last_close)
    }
}
