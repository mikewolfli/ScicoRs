//! Counters and timers for discrete-time logic / PLC simulation.
//!
//! - `Counter`: an up, down, or up-down hardware counter with preset.
//! - `Timer`: a period/pulse-width timer producing a boolean output.

use crate::core::types::Scalar;

/// Direction of a counter's counting operation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CounterDirection {
    /// Counts upward from 0 to `preset - 1`, then wraps to 0.
    Up,
    /// Counts downward from `preset` to 0, then wraps to `preset`.
    Down,
    /// Counts up to `preset`, then down to 0, repeating.
    UpDown,
}

/// A digital counter that counts clock ticks and indicates the terminal count
/// via its `output` flag.
///
/// ## Counting sequences
///
/// | Direction | Sequence (preset = N) |
/// |-----------|----------------------|
/// | `Up`      | 0, 1, 2, …, N‑1, 0, … |
/// | `Down`    | N, N‑1, N‑2, …, 0, N, … |
/// | `UpDown`  | 0, 1, …, N‑1, N, N‑1, …, 1, 0, … |
///
/// `output` is `true` when the counter is at its terminal count
/// (0 for Up, 0 for Down, 0 or N for UpDown).
#[derive(Debug, Clone)]
pub struct Counter {
    /// Counting direction.
    pub direction: CounterDirection,
    /// Terminal count value.
    pub preset: u64,
    /// Current count value.
    pub current: u64,
    /// Output state: `true` when the counter is at a terminal count.
    pub output: bool,
}

impl Counter {
    /// Create a new counter with the given direction and preset.
    ///
    /// # Panics
    /// Panics if `preset` is zero.
    pub fn new(direction: CounterDirection, preset: u64) -> Self {
        assert!(preset > 0, "Counter preset must be positive, got {preset}");
        let (current, output) = match direction {
            CounterDirection::Up => (0, true),         // terminal at 0
            CounterDirection::Down => (preset, false), // not yet at 0
            CounterDirection::UpDown => (0, true),     // terminal at 0
        };
        Self {
            direction,
            preset,
            current,
            output,
        }
    }

    /// Advance the counter by one clock tick.
    ///
    /// Returns `true` if the counter is at a terminal count after this tick.
    pub fn clock(&mut self) -> bool {
        match self.direction {
            CounterDirection::Up => {
                self.current = (self.current + 1) % self.preset;
                self.output = self.current == 0;
            }
            CounterDirection::Down => {
                self.current = if self.current == 0 {
                    self.preset
                } else {
                    self.current - 1
                };
                self.output = self.current == 0;
            }
            CounterDirection::UpDown => {
                // Triangular wave: step 0..2*preset-1, then back.
                // current stores a step counter; map to actual count.
                self.current = (self.current + 1) % (2 * self.preset);
                let count = if self.current <= self.preset {
                    self.current
                } else {
                    2 * self.preset - self.current
                };
                self.output = count == 0 || count == self.preset;
            }
        }
        self.output
    }

    /// Reset the counter to its initial state.
    pub fn reset(&mut self) {
        self.current = match self.direction {
            CounterDirection::Up | CounterDirection::UpDown => 0,
            CounterDirection::Down => self.preset,
        };
        self.output = match self.direction {
            CounterDirection::Up | CounterDirection::UpDown => self.current == 0,
            CounterDirection::Down => self.current == self.preset,
        };
    }

    /// Load an arbitrary value into the counter.
    ///
    /// The value is clamped to `[0, preset]` (or `[0, 2*preset]` for UpDown).
    pub fn load(&mut self, value: u64) {
        let max = match self.direction {
            CounterDirection::Up => self.preset - 1,
            CounterDirection::Down => self.preset,
            CounterDirection::UpDown => 2 * self.preset,
        };
        self.current = value.min(max);
        self.output = match self.direction {
            CounterDirection::Up => self.current == 0,
            CounterDirection::Down => self.current == 0,
            CounterDirection::UpDown => self.current == 0 || self.current == self.preset,
        };
    }

    /// Check whether the counter is at its zero/start state.
    ///
    /// For `Up` this is `current == 0`.
    /// For `Down` this is `current == preset` (the equivalent starting point).
    /// For `UpDown` this is the step counter at 0 (count mapped to 0).
    pub fn is_zero(&self) -> bool {
        match self.direction {
            CounterDirection::Up => self.current == 0,
            CounterDirection::Down => self.current == self.preset,
            CounterDirection::UpDown => self.current == 0,
        }
    }
}

/// A periodic timer that produces a boolean pulse train.
///
/// The output is `true` for `pulse_width` seconds at the beginning of each
/// `period`-second cycle (inclusive of the exact pulse_width boundary).
#[derive(Debug, Clone)]
pub struct Timer {
    /// Total period in seconds.
    pub period: Scalar,
    /// Duration of the output pulse in seconds.
    pub pulse_width: Scalar,
    /// Elapsed time within the current period.
    elapsed: Scalar,
    /// Current output state.
    pub output: bool,
}

impl Timer {
    /// Create a new timer.
    ///
    /// # Panics
    /// Panics if `period` is not positive, or `pulse_width` exceeds `period`.
    pub fn new(period: Scalar, pulse_width: Scalar) -> Self {
        assert!(period > 0.0, "Timer period must be positive, got {period}");
        assert!(
            pulse_width >= 0.0 && pulse_width <= period,
            "Timer pulse_width ({pulse_width}) must be in [0, period] ({period})"
        );
        Self {
            period,
            pulse_width,
            elapsed: 0.0,
            output: pulse_width > 0.0,
        }
    }

    /// Advance the timer by `dt` seconds.
    ///
    /// Returns `true` if the output changed state on this update.
    pub fn update(&mut self, dt: Scalar) -> bool {
        if dt <= 0.0 {
            return false;
        }
        let prev_output = self.output;
        self.elapsed += dt;

        // Wrap elapsed into [0, period).
        if self.elapsed >= self.period {
            self.elapsed %= self.period;
        }

        // Output is high during the pulse at the start of each period
        // (inclusive of the exact pulse_width boundary).
        self.output = self.pulse_width > 0.0 && self.elapsed <= self.pulse_width;

        self.output != prev_output
    }

    /// Reset the timer to the beginning of a period.
    pub fn reset(&mut self) {
        self.elapsed = 0.0;
        self.output = self.pulse_width > 0.0;
    }

    /// Return the elapsed time within the current period.
    pub fn elapsed_time(&self) -> Scalar {
        self.elapsed
    }

    /// Check whether the timer output is currently active (high).
    pub fn is_active(&self) -> bool {
        self.output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: Scalar, b: Scalar) {
        assert!((a - b).abs() < 1e-10, "expected {b}, got {a}");
    }

    // ── Counter ──────────────────────────────────────────────────────────

    #[test]
    fn counter_up() {
        let mut c = Counter::new(CounterDirection::Up, 5);
        assert!(c.is_zero());
        assert!(c.output); // terminal at 0
        // Clock 4 times: output goes false (0→1,1→2,2→3,3→4)
        for _ in 0..4 {
            c.clock();
            assert!(!c.output);
        }
        // 5th clock wraps 4→0; output goes true again.
        c.clock();
        assert!(c.is_zero());
        assert!(c.output);
    }

    #[test]
    fn counter_down() {
        let mut c = Counter::new(CounterDirection::Down, 5);
        assert!(c.is_zero()); // current == preset is "zero state"
        assert!(!c.output); // not at terminal (0)
        c.clock(); // 4
        assert!(!c.output);
        c.clock(); // 3
        c.clock(); // 2
        c.clock(); // 1
        c.clock(); // 0 — terminal!
        assert!(c.output);
        assert!(c.current == 0);
        // Next clock wraps back to preset.
        c.clock();
        assert!(c.current == 5);
        assert!(!c.output);
    }

    #[test]
    fn counter_updown() {
        let mut c = Counter::new(CounterDirection::UpDown, 3);
        assert!(c.is_zero());
        assert!(c.output);
        // Up phase
        c.clock();
        assert!(!c.output); // 1
        c.clock();
        assert!(!c.output); // 2
        c.clock();
        assert!(c.output); // 3 (terminal)
        // Down phase
        c.clock();
        assert!(!c.output); // 2
        c.clock();
        assert!(!c.output); // 1
        c.clock();
        assert!(c.output); // 0 (terminal)
    }

    #[test]
    fn counter_preset() {
        let mut c = Counter::new(CounterDirection::Up, 1);
        assert!(c.is_zero());
        assert!(c.output);
        // Single-step: every clock wraps (0→0).
        c.clock();
        assert!(c.is_zero());
        assert!(c.output);
    }

    #[test]
    fn counter_reset() {
        let mut c = Counter::new(CounterDirection::Up, 10);
        c.clock();
        c.clock();
        assert_eq!(c.current, 2);
        c.reset();
        assert!(c.is_zero());
    }

    #[test]
    fn counter_load() {
        let mut c = Counter::new(CounterDirection::Up, 10);
        c.load(7);
        assert_eq!(c.current, 7);
        assert!(!c.output);
        c.load(0);
        assert!(c.output); // loaded to terminal
    }

    #[test]
    #[should_panic(expected = "preset must be positive")]
    fn counter_zero_preset_panics() {
        Counter::new(CounterDirection::Up, 0);
    }

    // ── Timer ────────────────────────────────────────────────────────────

    #[test]
    fn timer_period() {
        let mut t = Timer::new(10.0, 2.0);
        assert!(t.is_active());
        // t=0→2.0: still within pulse (≤ pulse_width), no transition.
        t.update(2.0);
        assert!(t.is_active());
        // t=2.0→2.1: now past pulse, transitions off.
        let changed = t.update(0.1);
        assert!(!t.is_active());
        assert!(changed);
        // t=2.1→10.0: wraps to next period, transitions on.
        let changed = t.update(7.9);
        assert!(t.is_active());
        assert!(changed);
    }

    #[test]
    fn timer_pulse() {
        let mut t = Timer::new(5.0, 1.0);
        // Initial: active (pulse > 0)
        // After 0.5s, still within pulse, output unchanged (first transition
        // isn't a change since we start active).
        t.update(0.5);
        assert!(t.is_active());
        // Move past pulse.
        let toggled = t.update(0.6); // now elapsed = 1.1 > 1.0
        assert!(!t.is_active());
        assert!(toggled);
    }

    #[test]
    fn timer_reset() {
        let mut t = Timer::new(10.0, 3.0);
        t.update(5.0);
        assert!(!t.is_active());
        t.reset();
        assert!(t.is_active());
        approx_eq(t.elapsed_time(), 0.0);
    }

    #[test]
    fn timer_elapsed_time() {
        let mut t = Timer::new(10.0, 2.0);
        t.update(3.5);
        approx_eq(t.elapsed_time(), 3.5);
    }

    #[test]
    fn timer_zero_pulse() {
        let t = Timer::new(10.0, 0.0);
        assert!(!t.is_active());
    }

    #[test]
    fn timer_update_no_change() {
        let mut t = Timer::new(10.0, 5.0);
        // First update; output was already true (pulse > 0).
        let changed = t.update(3.0);
        assert!(!changed); // still in pulse, no change
        let changed = t.update(2.5); // cumulative 5.5 > 5.0
        assert!(changed); // left the pulse
    }

    #[test]
    #[should_panic(expected = "period must be positive")]
    fn timer_zero_period_panics() {
        Timer::new(0.0, 0.0);
    }

    #[test]
    #[should_panic(expected = "pulse_width")]
    fn timer_pulse_exceeds_period_panics() {
        Timer::new(5.0, 6.0);
    }
}
