//! Kaufman's Adaptive Moving Average (1995).
//!
//! Lab #9 parameters: er_period=10, fast=2, slow=30
//! SC_fast = 2/(2+1) = 0.6667, SC_slow = 2/(30+1) ≈ 0.0645
//! ER = |change over er_period| / sum(|bar-to-bar changes|)
//! SC = [ER * (SC_fast - SC_slow) + SC_slow]^2
//! KAMA_t = KAMA_{t-1} + SC * (price - KAMA_{t-1})
//! Seeded with SMA of first er_period closes.
//!
//! Signal: +1 on close crossing above KAMA (strict), -1 on close crossing below.
//!
//! Vendored 2026-10-22 from hidden_files/strategy-upload/jupiter-strategies
//! (N7.2 scheme library). One divergence from the upload, fixed here: on the
//! seeding bar (window == er_period closes) the efficiency-ratio index
//! `len - 1 - er_period` underflows usize and the original code panicked — a
//! latent runtime bug (the upload's runner never selected KamaCross, so it
//! never fired; it 500s the /api/backtest function on any KAMA run). The fix
//! establishes the cross-detection baseline on the seeding bar and waits one
//! bar for the full er_period + 1 window before computing the ratio.

use crate::indicators::Bar;
use std::collections::VecDeque;

#[derive(Clone, Debug)]
pub struct Kama {
    er_period: usize,
    fast_sc: f64,
    slow_sc: f64,
    closes: VecDeque<f64>,
    kama: Option<f64>,
    prev_close: Option<f64>,
    signal: i8,
    prev_above: Option<bool>,
}

impl Kama {
    pub fn new(er_period: usize, fast: usize, slow: usize) -> Self {
        let fast_sc = 2.0 / (fast as f64 + 1.0);
        let slow_sc = 2.0 / (slow as f64 + 1.0);
        Self {
            er_period,
            fast_sc,
            slow_sc,
            closes: VecDeque::with_capacity(er_period + 1),
            kama: None,
            prev_close: None,
            signal: 0,
            prev_above: None,
        }
    }

    /// Lab canonical (10,2,30).
    pub fn default_10_2_30() -> Self {
        Self::new(10, 2, 30)
    }

    pub fn update(&mut self, bar: &Bar) -> i8 {
        self.closes.push_back(bar.close);
        if self.closes.len() > self.er_period + 1 {
            self.closes.pop_front();
        }

        if self.closes.len() < self.er_period {
            self.prev_close = Some(bar.close);
            return 0;
        }

        // Seed on first full window
        if self.kama.is_none() {
            let sma: f64 = self.closes.iter().take(self.er_period).sum::<f64>()
                / self.er_period as f64;
            self.kama = Some(sma);
        }

        // The efficiency ratio needs the close from er_period bars ago, which
        // only exists once the window holds er_period + 1 closes. On the
        // seeding bar the index `len - 1 - er_period` underflows (usize) and
        // the original code panicked here. Establish the cross-detection
        // baseline and wait one bar for the full window instead.
        if self.closes.len() <= self.er_period {
            let k = self.kama.unwrap();
            self.prev_above = Some(bar.close > k);
            self.prev_close = Some(bar.close);
            return 0;
        }

        // Efficiency Ratio
        let change = (bar.close - self.closes[self.closes.len() - 1 - self.er_period]).abs();
        let mut volatility = 0.0;
        for i in 1..=self.er_period {
            let idx = self.closes.len() - i;
            volatility += (self.closes[idx] - self.closes[idx - 1]).abs();
        }
        let er = if volatility > 0.0 {
            change / volatility
        } else {
            0.0
        };

        let sc = (er * (self.fast_sc - self.slow_sc) + self.slow_sc).powi(2);

        let prev_k = self.kama.unwrap();
        let new_k = prev_k + sc * (bar.close - prev_k);
        self.kama = Some(new_k);

        // Cross detection
        let above = bar.close > new_k;
        if let Some(prev) = self.prev_above {
            if !prev && above {
                self.signal = 1;
            } else if prev && !above {
                self.signal = -1;
            }
        }
        self.prev_above = Some(above);
        self.prev_close = Some(bar.close);
        self.signal
    }

    pub fn value(&self) -> Option<f64> {
        self.kama
    }

    pub fn signal(&self) -> i8 {
        self.signal
    }
}
