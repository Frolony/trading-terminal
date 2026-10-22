//! Exponential Moving Average and crossover signals.
//! Lab #7: EMA(20)/EMA(50) daily crossover on ETH (adaptable to SOL).

use crate::indicators::Bar;

#[derive(Clone, Debug)]
pub struct Ema {
    period: usize,
    k: f64,
    value: Option<f64>,
    count: usize,
}

impl Ema {
    pub fn new(period: usize) -> Self {
        Self {
            period,
            k: 2.0 / (period as f64 + 1.0),
            value: None,
            count: 0,
        }
    }

    pub fn update(&mut self, price: f64) -> Option<f64> {
        self.count += 1;
        self.value = match self.value {
            None => Some(price), // seed with first price; for purity could SMA first N
            Some(prev) => Some(price * self.k + prev * (1.0 - self.k)),
        };
        if self.count >= self.period {
            self.value
        } else {
            None
        }
    }

    pub fn value(&self) -> Option<f64> {
        self.value
    }
}

/// Two-EMA crossover system.
#[derive(Clone, Debug)]
pub struct EmaCrossover {
    fast: Ema,
    slow: Ema,
    prev_fast: Option<f64>,
    prev_slow: Option<f64>,
    signal: i8, // +1 long, -1 flat/short (lab is long-only)
}

impl EmaCrossover {
    pub fn new(fast_period: usize, slow_period: usize) -> Self {
        Self {
            fast: Ema::new(fast_period),
            slow: Ema::new(slow_period),
            prev_fast: None,
            prev_slow: None,
            signal: 0,
        }
    }

    pub fn ema_20_50() -> Self {
        Self::new(20, 50)
    }

    pub fn update(&mut self, bar: &Bar) -> i8 {
        let f = self.fast.update(bar.close);
        let s = self.slow.update(bar.close);

        if let (Some(fv), Some(sv), Some(pf), Some(ps)) = (f, s, self.prev_fast, self.prev_slow) {
            // Strict crossover
            if pf <= ps && fv > sv {
                self.signal = 1;
            } else if pf >= ps && fv < sv {
                self.signal = -1;
            }
            // else hold prior signal
        }

        self.prev_fast = f;
        self.prev_slow = s;
        self.signal
    }

    pub fn signal(&self) -> i8 {
        self.signal
    }

    pub fn bullish_cross(&self) -> bool {
        self.signal == 1
            && self.prev_fast.is_some()
            && self.prev_slow.is_some()
            && self.prev_fast.unwrap() <= self.prev_slow.unwrap()
    }
}
