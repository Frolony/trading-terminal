//! Bollinger Bands (standard) + lab drift gate.
//!
//! BB(period, mult): middle = SMA(close, period), upper/lower = middle ± mult * stdev
//! Lab gated entry: take lower-band long ONLY when |close / SMA(200) - 1| <= drift_band (0.25)
//! Exit: middle band always honored (close crosses above middle).

use crate::indicators::Bar;
use std::collections::VecDeque;

#[derive(Clone, Debug)]
pub struct BollingerBands {
    period: usize,
    mult: f64,
    closes: VecDeque<f64>,
    // Long-term SMA for drift gate
    lt_period: usize,
    lt_closes: VecDeque<f64>,
    drift_band: f64,
    middle: Option<f64>,
    upper: Option<f64>,
    lower: Option<f64>,
    lt_sma: Option<f64>,
}

impl BollingerBands {
    pub fn new(period: usize, mult: f64, lt_period: usize, drift_band: f64) -> Self {
        Self {
            period,
            mult,
            closes: VecDeque::with_capacity(period + 1),
            lt_period,
            lt_closes: VecDeque::with_capacity(lt_period + 1),
            drift_band,
            middle: None,
            upper: None,
            lower: None,
            lt_sma: None,
        }
    }

    /// Lab default: BB(20,2) gated by 200-SMA ±25%.
    pub fn drift_gated_20_2() -> Self {
        Self::new(20, 2.0, 200, 0.25)
    }

    pub fn update(&mut self, bar: &Bar) {
        self.closes.push_back(bar.close);
        if self.closes.len() > self.period {
            self.closes.pop_front();
        }
        self.lt_closes.push_back(bar.close);
        if self.lt_closes.len() > self.lt_period {
            self.lt_closes.pop_front();
        }

        if self.closes.len() >= self.period {
            let mean: f64 = self.closes.iter().sum::<f64>() / self.period as f64;
            let var: f64 = self
                .closes
                .iter()
                .map(|c| (c - mean).powi(2))
                .sum::<f64>()
                / self.period as f64;
            let std = var.sqrt();
            self.middle = Some(mean);
            self.upper = Some(mean + self.mult * std);
            self.lower = Some(mean - self.mult * std);
        }

        if self.lt_closes.len() >= self.lt_period {
            let mean: f64 = self.lt_closes.iter().sum::<f64>() / self.lt_period as f64;
            self.lt_sma = Some(mean);
        } else {
            // Lab: gate passes through (allows entry) before 200 closes exist
            self.lt_sma = None;
        }
    }

    pub fn middle(&self) -> Option<f64> {
        self.middle
    }
    pub fn upper(&self) -> Option<f64> {
        self.upper
    }
    pub fn lower(&self) -> Option<f64> {
        self.lower
    }

    /// True if drift gate allows a mean-reversion long (no strong directional drift).
    pub fn drift_gate_ok(&self, close: f64) -> bool {
        match self.lt_sma {
            None => true, // warmup pass-through per lab
            Some(sma) if sma > 0.0 => {
                let ratio = (close / sma - 1.0).abs();
                ratio <= self.drift_band
            }
            _ => false,
        }
    }

    /// Entry: close touches/crosses below lower band AND drift gate ok.
    pub fn entry_signal(&self, close: f64) -> bool {
        match self.lower {
            Some(lo) if close <= lo && self.drift_gate_ok(close) => true,
            _ => false,
        }
    }

    /// Exit: close crosses above middle band (always honored).
    pub fn exit_signal(&self, close: f64) -> bool {
        match self.middle {
            Some(mid) if close >= mid => true,
            _ => false,
        }
    }
}
