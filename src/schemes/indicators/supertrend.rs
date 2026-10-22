//! Supertrend indicator (ATR-based trend following).
//!
//! Standard construction:
//! - ATR period N, multiplier M (lab defaults (10,3) and (7,2))
//! - True Range = max(H-L, |H-prevC|, |L-prevC|)
//! - ATR = Wilder smooth: ATR_t = (ATR_{t-1}*(N-1) + TR_t) / N
//! - Basic upper = (H+L)/2 + M * ATR
//! - Basic lower = (H+L)/2 - M * ATR
//! - Final bands carry only in trend direction; Supertrend line sits on lower (bull) or upper (bear)
//! - Trend +1 when close > Supertrend (or final lower), -1 when close < Supertrend
//!
//! Entry/exit in lab: enter long on bullish flip (or while +1), exit on bearish flip (or time-stop).

use crate::indicators::Bar;

#[derive(Clone, Debug)]
pub struct Supertrend {
    atr_period: usize,
    multiplier: f64,
    atr: Option<f64>,
    prev_close: Option<f64>,
    final_upper: Option<f64>,
    final_lower: Option<f64>,
    /// Current Supertrend value (the line)
    value: Option<f64>,
    /// +1 bull, -1 bear, 0 flat (warmup)
    trend: i8,
    bars_seen: usize,
}

impl Supertrend {
    pub fn new(atr_period: usize, multiplier: f64) -> Self {
        Self {
            atr_period,
            multiplier,
            atr: None,
            prev_close: None,
            final_upper: None,
            final_lower: None,
            value: None,
            trend: 0,
            bars_seen: 0,
        }
    }

    /// Lab defaults.
    pub fn default_10_3() -> Self {
        Self::new(10, 3.0)
    }

    pub fn tight_7_2() -> Self {
        Self::new(7, 2.0)
    }

    pub fn update(&mut self, bar: &Bar) -> i8 {
        self.bars_seen += 1;
        let tr = match self.prev_close {
            Some(pc) => {
                let hl = bar.high - bar.low;
                let hc = (bar.high - pc).abs();
                let lc = (bar.low - pc).abs();
                hl.max(hc).max(lc)
            }
            None => bar.high - bar.low,
        };

        // Seed / update ATR (Wilder)
        self.atr = match self.atr {
            None => {
                // Simple average seed over first N would be ideal; for streaming we use first TR then smooth.
                // For exact lab match a full history SMA seed is preferred; this is online approximation.
                Some(tr)
            }
            Some(prev_atr) => {
                if self.bars_seen <= self.atr_period {
                    // Still building; use running mean approximation
                    Some((prev_atr * (self.bars_seen as f64 - 1.0) + tr) / self.bars_seen as f64)
                } else {
                    Some((prev_atr * (self.atr_period as f64 - 1.0) + tr) / self.atr_period as f64)
                }
            }
        };

        let atr = self.atr.unwrap_or(tr);
        let hl2 = (bar.high + bar.low) / 2.0;
        let basic_upper = hl2 + self.multiplier * atr;
        let basic_lower = hl2 - self.multiplier * atr;

        // Final bands: only move with trend (standard Supertrend carry rule)
        let (final_upper, final_lower) = match (self.final_upper, self.final_lower) {
            (Some(fu), Some(fl)) => {
                let new_fu = if basic_upper < fu || self.prev_close.unwrap_or(0.0) > fu {
                    basic_upper
                } else {
                    fu
                };
                let new_fl = if basic_lower > fl || self.prev_close.unwrap_or(0.0) < fl {
                    basic_lower
                } else {
                    fl
                };
                (new_fu, new_fl)
            }
            _ => (basic_upper, basic_lower),
        };

        self.final_upper = Some(final_upper);
        self.final_lower = Some(final_lower);

        // Determine trend and Supertrend line
        let prev_trend = self.trend;
        let (new_trend, st_value) = if self.bars_seen < self.atr_period {
            (0i8, None)
        } else if prev_trend <= 0 {
            // Was bear or flat: check for flip to bull
            if bar.close > final_upper {
                (1, Some(final_lower))
            } else {
                (-1, Some(final_upper))
            }
        } else {
            // Was bull: check for flip to bear
            if bar.close < final_lower {
                (-1, Some(final_upper))
            } else {
                (1, Some(final_lower))
            }
        };

        self.trend = new_trend;
        self.value = st_value;
        self.prev_close = Some(bar.close);
        new_trend
    }

    pub fn trend(&self) -> i8 {
        self.trend
    }

    pub fn value(&self) -> Option<f64> {
        self.value
    }

    /// True on the bar the trend flips to +1 (entry candidate).
    pub fn bullish_flip(&self, prev_trend: i8) -> bool {
        prev_trend <= 0 && self.trend == 1
    }

    /// True on the bar the trend flips to -1 (exit candidate).
    pub fn bearish_flip(&self, prev_trend: i8) -> bool {
        prev_trend >= 0 && self.trend == -1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supertrend_warms_up() {
        let mut st = Supertrend::default_10_3();
        for i in 0..15 {
            let c = 100.0 + i as f64;
            let t = st.update(&Bar::new(c + 1.0, c - 1.0, c));
            if i < 9 {
                assert_eq!(t, 0);
            }
        }
        assert!(st.trend() != 0);
    }
}
