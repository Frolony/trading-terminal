//! Aroon indicator (Tushar Chande, 1995).
//!
//! AroonUp   = 100 * (period - bars_since_highest_high) / period
//! AroonDown = 100 * (period - bars_since_lowest_low) / period
//!
//! Lab signal (trend state):
//!   +1 when AroonUp > AroonDown
//!   -1 when AroonDown > AroonUp
//!   carry prior on equality (no flip-flop)
//! Flat until `period` bars exist.
//!
//! "Fresh +1 flip" = previous was not +1 and now is +1 (entry).
//! Time-stop overlays: exit also after max_hold bars held.

use crate::indicators::Bar;
use std::collections::VecDeque;

#[derive(Clone, Debug)]
pub struct Aroon {
    period: usize,
    highs: VecDeque<f64>,
    lows: VecDeque<f64>,
    /// Current signal: +1 / -1 / 0
    signal: i8,
    prev_signal: i8,
}

impl Aroon {
    pub fn new(period: usize) -> Self {
        Self {
            period,
            highs: VecDeque::with_capacity(period + 1),
            lows: VecDeque::with_capacity(period + 1),
            signal: 0,
            prev_signal: 0,
        }
    }

    pub fn period_25() -> Self {
        Self::new(25)
    }

    pub fn period_40() -> Self {
        Self::new(40)
    }

    pub fn update(&mut self, bar: &Bar) -> i8 {
        self.prev_signal = self.signal;
        self.highs.push_back(bar.high);
        self.lows.push_back(bar.low);
        if self.highs.len() > self.period {
            self.highs.pop_front();
            self.lows.pop_front();
        }

        if self.highs.len() < self.period {
            self.signal = 0;
            return 0;
        }

        // Find bars since extreme (0 = extreme is the current/most recent bar)
        let mut bars_since_high = 0usize;
        let mut max_h = f64::MIN;
        for (i, &h) in self.highs.iter().enumerate() {
            if h >= max_h {
                max_h = h;
                bars_since_high = self.period - 1 - i;
            }
        }

        let mut bars_since_low = 0usize;
        let mut min_l = f64::MAX;
        for (i, &l) in self.lows.iter().enumerate() {
            if l <= min_l {
                min_l = l;
                bars_since_low = self.period - 1 - i;
            }
        }

        let aroon_up = 100.0 * (self.period as f64 - bars_since_high as f64) / self.period as f64;
        let aroon_down = 100.0 * (self.period as f64 - bars_since_low as f64) / self.period as f64;

        // Equality rule: carry prior (no flip-flop)
        if (aroon_up - aroon_down).abs() < 1e-9 {
            // keep self.signal
        } else if aroon_up > aroon_down {
            self.signal = 1;
        } else {
            self.signal = -1;
        }

        self.signal
    }

    pub fn signal(&self) -> i8 {
        self.signal
    }

    pub fn fresh_bullish_flip(&self) -> bool {
        self.prev_signal != 1 && self.signal == 1
    }

    pub fn fresh_bearish_flip(&self) -> bool {
        self.prev_signal != -1 && self.signal == -1
    }
}
