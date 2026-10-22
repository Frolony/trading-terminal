//! #7 EMA 20/50 crossover (daily).

use crate::indicators::{Bar, EmaCrossover};
use crate::strategies::Strategy;

pub struct Ema20_50 {
    cross: EmaCrossover,
}

impl Ema20_50 {
    pub fn new() -> Self {
        Self {
            cross: EmaCrossover::ema_20_50(),
        }
    }
}

impl Default for Ema20_50 {
    fn default() -> Self {
        Self::new()
    }
}

impl Strategy for Ema20_50 {
    fn name(&self) -> &str {
        "EMA 20/50 crossover"
    }

    fn on_bar(&mut self, bar: &Bar) {
        self.cross.update(bar);
    }

    fn should_enter(&self) -> bool {
        self.cross.signal() == 1
    }

    fn should_exit(&self, _bars_held: usize) -> bool {
        self.cross.signal() == -1
    }

    fn signal(&self) -> i8 {
        self.cross.signal()
    }
}
