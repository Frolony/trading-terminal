//! Aroon(25/40) trend-state and time-stop overlays.
//! #2 Aroon(25)+10d [SOL], #5 Aroon(25)+40d [ETH], #6 pure Aroon(25), #8 Aroon(40).

use crate::indicators::{Aroon, Bar};
use crate::strategies::Strategy;

pub struct AroonTimeStop {
    name: String,
    aroon: Aroon,
    max_hold: Option<usize>,
    current: i8,
}

impl AroonTimeStop {
    pub fn new(period: usize, max_hold: Option<usize>) -> Self {
        let name = match max_hold {
            Some(h) => format!("Aroon({}) + {}d time-stop", period, h),
            None => format!("Aroon({}) trend-state", period),
        };
        Self {
            name,
            aroon: Aroon::new(period),
            max_hold,
            current: 0,
        }
    }

    /// #2 SOL: Aroon(25) entries + 10-day time-stop.
    pub fn sol_25_10() -> Self {
        Self::new(25, Some(10))
    }

    /// #5 ETH: Aroon(25) + 40d.
    pub fn eth_25_40() -> Self {
        Self::new(25, Some(40))
    }

    /// #6 pure Aroon(25) trend state.
    pub fn pure_25() -> Self {
        Self::new(25, None)
    }

    /// #8 Aroon(40).
    pub fn pure_40() -> Self {
        Self::new(40, None)
    }
}

impl Strategy for AroonTimeStop {
    fn name(&self) -> &str {
        &self.name
    }

    fn on_bar(&mut self, bar: &Bar) {
        self.current = self.aroon.update(bar);
    }

    fn should_enter(&self) -> bool {
        // Fresh +1 flip (lab language: "fresh Aroon(25) +1 flip")
        self.aroon.fresh_bullish_flip()
    }

    fn should_exit(&self, bars_held: usize) -> bool {
        let flip_exit = self.aroon.fresh_bearish_flip();
        let time_exit = self.max_hold.map(|m| bars_held >= m).unwrap_or(false);
        flip_exit || time_exit
    }

    fn signal(&self) -> i8 {
        self.current
    }
}
