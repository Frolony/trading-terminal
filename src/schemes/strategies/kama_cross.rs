//! #9 KAMA(10,2,30) price crossover.

use crate::indicators::{Bar, Kama};
use crate::strategies::Strategy;

pub struct KamaCross {
    kama: Kama,
}

impl KamaCross {
    pub fn new() -> Self {
        Self {
            kama: Kama::default_10_2_30(),
        }
    }
}

impl Default for KamaCross {
    fn default() -> Self {
        Self::new()
    }
}

impl Strategy for KamaCross {
    fn name(&self) -> &str {
        "KAMA(10,2,30) price crossover"
    }

    fn on_bar(&mut self, bar: &Bar) {
        self.kama.update(bar);
    }

    fn should_enter(&self) -> bool {
        self.kama.signal() == 1
    }

    fn should_exit(&self, _bars_held: usize) -> bool {
        self.kama.signal() == -1
    }

    fn signal(&self) -> i8 {
        self.kama.signal()
    }
}
