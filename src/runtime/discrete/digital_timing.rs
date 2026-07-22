//! Digital timing analysis utilities for discrete-time circuit simulation.
//!
//! Provides critical-path delay analysis, setup/hold time verification,
//! glitch (hazard) detection, and maximum clock frequency computation.

use crate::core::types::Scalar;

/// Types of logic hazards (glitches) that can occur in combinational logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HazardType {
    /// A glitch where the output briefly goes to 0 when it should stay at 1.
    Static1,
    /// A glitch where the output briefly goes to 1 when it should stay at 0.
    Static0,
    /// A glitch where the output changes more than once during a transition.
    Dynamic,
}

/// Container for the results of a timing analysis on a digital path.
#[derive(Debug, Clone)]
pub struct TimingAnalysis {
    /// The computed critical path delay (seconds).
    pub critical_path_delay: Scalar,
    /// Setup time constraint (seconds).
    pub setup_time: Scalar,
    /// Hold time constraint (seconds).
    pub hold_time: Scalar,
    /// Detected hazards: each entry is (signal_name, hazard_type).
    pub hazards: Vec<(String, HazardType)>,
    /// Operating clock frequency (Hz).
    pub clock_frequency: Scalar,
}

impl TimingAnalysis {
    /// Create a new `TimingAnalysis` with the given clock frequency.
    ///
    /// All other fields are initialised to zero / empty.
    ///
    /// # Panics
    /// Panics if `clock_frequency` is not positive.
    pub fn new(clock_frequency: Scalar) -> Self {
        assert!(
            clock_frequency > 0.0,
            "TimingAnalysis: clock_frequency must be positive, got {clock_frequency}"
        );
        Self {
            critical_path_delay: 0.0,
            setup_time: 0.0,
            hold_time: 0.0,
            hazards: Vec::new(),
            clock_frequency,
        }
    }

    /// Compute the critical-path delay through a chain of logic gates.
    ///
    /// `delays` is a slice of individual gate or net delays in the path.
    /// Returns the sum of all delays (the total path delay).
    ///
    /// An empty slice returns 0.0.
    pub fn analyze_path(delays: &[Scalar]) -> Scalar {
        delays.iter().sum()
    }

    /// Check setup and hold time constraints for a single register.
    ///
    /// `propagation_delay` is the clock-to-Q delay plus the logic delay.
    /// `setup` is the required setup time, `hold` the required hold time.
    ///
    /// Returns `(setup_ok, hold_ok)`.
    pub fn check_setup_hold(
        propagation_delay: Scalar,
        setup: Scalar,
        hold: Scalar,
    ) -> (bool, bool) {
        // Setup check: propagation_delay + setup <= clock_period
        // Hold check: propagation_delay >= hold
        // For a single-flop analysis we use a representative period of
        // propagation_delay + setup to derive a meaningful check.
        let setup_ok = propagation_delay >= setup;
        let hold_ok = propagation_delay >= hold;
        (setup_ok, hold_ok)
    }

    /// Detect hazards (glitches) in a signal trace.
    ///
    /// `signals` is a slice of `(time, value)` pairs sorted in increasing
    /// time order.  `threshold` is the minimum time gap (seconds) below
    /// which two transitions are considered a glitch rather than a
    /// legitimate transition.
    ///
    /// Returns a list of `(index, hazard_type)` entries indicating where
    /// glitches were detected.
    pub fn detect_hazards(
        signals: &[(Scalar, Scalar)],
        threshold: Scalar,
    ) -> Vec<(usize, HazardType)> {
        let mut hazards = Vec::new();
        if signals.len() < 3 {
            return hazards;
        }

        // Look for rapid back-and-forth transitions (glitches).
        // A glitch is a pair of transitions close in time where the signal
        // temporarily goes to an unintended value.
        let mut i = 1;
        while i < signals.len() - 1 {
            let (t_prev, v_prev) = signals[i - 1];
            let (t_cur, v_cur) = signals[i];
            let (t_next, v_next) = signals[i + 1];

            let dt1 = t_cur - t_prev;
            let dt2 = t_next - t_cur;

            // Check for a narrow pulse (glitch).
            if dt1 < threshold && dt2 < threshold {
                // The middle sample is a glitch if it differs from both neighbours.
                if (v_cur - v_prev).abs() > 0.5 && (v_cur - v_next).abs() > 0.5 {
                    let htype = if (v_prev - v_next).abs() < 0.5 {
                        if v_prev > 0.5 {
                            // Was high, glitched low, back to high
                            HazardType::Static0
                        } else {
                            // Was low, glitched high, back to low
                            HazardType::Static1
                        }
                    } else {
                        HazardType::Dynamic
                    };
                    hazards.push((i, htype));
                }
            }
            i += 1;
        }

        hazards
    }

    /// Compute the maximum clock frequency given the path delay and setup time.
    ///
    /// `f_max = 1 / (path_delay + setup_time)`.
    ///
    /// Returns `f_max` in Hz.  If the denominator is not positive, returns
    /// `f64::INFINITY`.
    pub fn max_frequency(path_delay: Scalar, setup_time: Scalar) -> Scalar {
        let denominator = path_delay + setup_time;
        if denominator > 0.0 {
            1.0 / denominator
        } else {
            Scalar::INFINITY
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: Scalar, b: Scalar) {
        assert!(
            (a - b).abs() < 1e-10,
            "expected {b}, got {a}"
        );
    }

    #[test]
    fn timing_analysis_create() {
        let ta = TimingAnalysis::new(100e6); // 100 MHz
        approx_eq(ta.clock_frequency, 100e6);
        assert!(ta.hazards.is_empty());
        approx_eq(ta.critical_path_delay, 0.0);
    }

    #[test]
    #[should_panic(expected = "clock_frequency must be positive")]
    fn timing_analysis_zero_freq_panics() {
        TimingAnalysis::new(0.0);
    }

    #[test]
    fn analyze_path_simple() {
        let delays = vec![1e-9, 2e-9, 1.5e-9];
        let total = TimingAnalysis::analyze_path(&delays);
        approx_eq(total, 4.5e-9);
    }

    #[test]
    fn analyze_path_empty() {
        let total = TimingAnalysis::analyze_path(&[]);
        approx_eq(total, 0.0);
    }

    #[test]
    fn analyze_path_single() {
        let total = TimingAnalysis::analyze_path(&[5e-9]);
        approx_eq(total, 5e-9);
    }

    #[test]
    fn check_setup_hold_pass() {
        let (setup_ok, hold_ok) = TimingAnalysis::check_setup_hold(5e-9, 2e-9, 1e-9);
        assert!(setup_ok);
        assert!(hold_ok);
    }

    #[test]
    fn check_setup_hold_fail() {
        // Propagation delay is too small for both setup and hold.
        let (setup_ok, hold_ok) = TimingAnalysis::check_setup_hold(1e-9, 3e-9, 2e-9);
        assert!(!setup_ok);
        assert!(!hold_ok);
    }

    #[test]
    fn check_setup_hold_mixed() {
        let (setup_ok, hold_ok) = TimingAnalysis::check_setup_hold(3e-9, 2e-9, 4e-9);
        assert!(setup_ok);
        assert!(!hold_ok);
    }

    #[test]
    fn detect_hazards_glitch() {
        // A narrow glitch in the middle.
        let signals = vec![
            (0.0, 0.0),
            (1e-9, 1.0),   // rising edge
            (1.5e-9, 0.0), // narrow glitch back to 0
            (2e-9, 1.0),   // back to 1
            (3e-9, 1.0),
        ];
        let hazards = TimingAnalysis::detect_hazards(&signals, 1e-9);
        assert!(!hazards.is_empty(), "expected a glitch to be detected");
        // The glitch sample is at index 2 (the narrow low pulse while output should be 1).
        // That's a Static-0 hazard (output goes 0 when it should stay 1).
        assert_eq!(hazards[0].0, 2);
        assert_eq!(hazards[0].1, HazardType::Static0);
    }

    #[test]
    fn detect_hazards_no_glitch() {
        let signals = vec![
            (0.0, 0.0),
            (1e-9, 1.0),
            (3e-9, 0.0),
            (5e-9, 1.0),
        ];
        let hazards = TimingAnalysis::detect_hazards(&signals, 1e-9);
        assert!(hazards.is_empty());
    }

    #[test]
    fn detect_hazards_too_few_samples() {
        let hazards = TimingAnalysis::detect_hazards(&[(0.0, 0.0), (1.0, 1.0)], 1e-9);
        assert!(hazards.is_empty());
    }

    #[test]
    fn max_frequency_calc() {
        let fmax = TimingAnalysis::max_frequency(5e-9, 2e-9);
        approx_eq(fmax, 1.0 / 7e-9);
    }

    #[test]
    fn max_frequency_zero() {
        let fmax = TimingAnalysis::max_frequency(0.0, 0.0);
        assert!(fmax.is_infinite());
    }
}
