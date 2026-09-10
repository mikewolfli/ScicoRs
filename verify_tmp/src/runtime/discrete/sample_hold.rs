//! Sample-and-hold and resampling utilities for discrete-time simulation.
//!
//! The `SampleHold` struct implements a classic sample-and-hold: it tracks
//! a phase accumulator against a fixed sample rate and updates the held
//! value only at sample instants.
//!
//! Standalone functions provide linear interpolation and sample-rate
//! conversion (upsampling / downsampling) via linear interpolation.

use crate::core::types::{Scalar, Time};

/// A sample-and-hold that captures input values at a fixed rate.
///
/// The `phase` accumulator wraps at `sample_rate * period` so that the
/// first sample is taken after `phase / sample_rate` seconds.
#[derive(Debug, Clone)]
pub struct SampleHold {
    /// Sample rate in Hz (samples per second).
    pub sample_rate: Scalar,
    /// Initial phase offset as a fraction of the sample period (0 … 1).
    pub phase: Scalar,
    /// The most recently held output value.
    pub held_value: Scalar,
    /// The simulation time at which the last sample was taken.
    pub last_sample_time: Time,
}

impl SampleHold {
    /// Create a new sample-and-hold.
    ///
    /// # Panics
    /// Panics if `sample_rate` is not positive.
    pub fn new(sample_rate: Scalar, phase: Scalar) -> Self {
        assert!(
            sample_rate > 0.0,
            "SampleHold: sample_rate must be positive, got {sample_rate}"
        );
        Self {
            sample_rate,
            phase: phase.clamp(0.0, 1.0),
            held_value: 0.0,
            last_sample_time: 0.0,
        }
    }

    /// Update the sample-and-hold with the current `input` at simulation `time`.
    ///
    /// Returns the held output value (updated if a sample occurred).
    pub fn update(&mut self, input: Scalar, time: Time) -> Scalar {
        if self.should_sample(time) {
            self.held_value = input;
            self.last_sample_time = time;
        }
        self.held_value
    }

    /// Reset the held value and phase state.
    pub fn reset(&mut self) {
        self.held_value = 0.0;
        self.last_sample_time = 0.0;
    }

    /// Check whether a sample should be taken at the given `time`.
    ///
    /// The first sample occurs at `phase / sample_rate`. Thereafter,
    /// samples occur every `1.0 / sample_rate` seconds.
    pub fn should_sample(&self, time: Time) -> bool {
        let period = 1.0 / self.sample_rate;
        let first_sample = self.phase * period;
        if time < first_sample - 1e-12 {
            return false;
        }
        // Compute the number of sample periods since the first sample.
        let t_since_first = time - first_sample;
        let n_periods = (t_since_first / period + 1e-12).floor();
        // Trigger when the time aligns with a sample point (including the first).
        let sample_time = first_sample + n_periods * period;
        (time - sample_time).abs() < 1e-12
    }

    /// Return the current held value without updating.
    pub fn output(&self) -> Scalar {
        self.held_value
    }
}

/// Resample a signal from `from_rate` (Hz) to `to_rate` (Hz) using linear
/// interpolation.
///
/// The input signal is assumed to have been sampled at `from_rate`.  The
/// output length is `ceil(N * to_rate / from_rate)` where `N` is the input
/// length.  An input length of zero returns an empty vector.
pub fn resample(input: &[Scalar], from_rate: Scalar, to_rate: Scalar) -> Vec<Scalar> {
    if input.is_empty() || from_rate <= 0.0 || to_rate <= 0.0 {
        return Vec::new();
    }
    if (from_rate - to_rate).abs() < 1e-12 {
        return input.to_vec();
    }
    let n_in = input.len() as Scalar;
    let t_max = (n_in - 1.0) / from_rate;
    let n_out = (t_max * to_rate).ceil() as usize + 1;
    let mut output = Vec::with_capacity(n_out);

    for i in 0..n_out {
        let t = i as Scalar / to_rate;
        let t_idx = t * from_rate; // index in input domain
        let idx0 = t_idx.floor() as usize;
        let idx1 = (idx0 + 1).min(input.len() - 1);
        let frac = t_idx - idx0 as Scalar;
        let y = linear_interpolate(frac, input[idx0], input[idx1], 0.0, 1.0);
        output.push(y);
    }

    output
}

/// Linear interpolation between points `(t0, y0)` and `(t1, y1)` at time `t`.
///
/// Returns `y0` when `t ≤ t0`, `y1` when `t ≥ t1`, and linearly interpolates
/// in between.
///
/// # Panics
/// Panics if `t0` and `t1` are equal (would cause division by zero).
pub fn linear_interpolate(t: Scalar, y0: Scalar, y1: Scalar, t0: Scalar, t1: Scalar) -> Scalar {
    assert!(
        (t1 - t0).abs() > 0.0,
        "linear_interpolate: t0 and t1 must differ, got t0={t0}, t1={t1}"
    );
    if t <= t0 {
        return y0;
    }
    if t >= t1 {
        return y1;
    }
    let alpha = (t - t0) / (t1 - t0);
    y0 + alpha * (y1 - y0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: Scalar, b: Scalar) {
        assert!((a - b).abs() < 1e-10, "expected {b}, got {a}");
    }

    // ── SampleHold ───────────────────────────────────────────────────────

    #[test]
    fn sample_hold_creation() {
        let sh = SampleHold::new(100.0, 0.5);
        approx_eq(sh.sample_rate, 100.0);
        approx_eq(sh.phase, 0.5);
        approx_eq(sh.held_value, 0.0);
    }

    #[test]
    fn sample_hold_holds_value() {
        let mut sh = SampleHold::new(10.0, 0.0);
        // At t=0.0 the first sample is taken.
        let v = sh.update(std::f64::consts::PI, 0.0);
        approx_eq(v, std::f64::consts::PI);
        // Between samples the held value should persist.
        let v = sh.update(99.0, 0.05);
        approx_eq(v, std::f64::consts::PI); // still holds old value
        // At t=0.1 the next sample occurs.
        let v = sh.update(2.71, 0.1);
        approx_eq(v, 2.71);
    }

    #[test]
    fn sample_hold_resets() {
        let mut sh = SampleHold::new(10.0, 0.0);
        sh.update(42.0, 0.0);
        sh.reset();
        approx_eq(sh.held_value, 0.0);
        approx_eq(sh.last_sample_time, 0.0);
    }

    #[test]
    fn sample_hold_trigger_time() {
        let sh = SampleHold::new(100.0, 0.0);
        assert!(sh.should_sample(0.0));
        assert!(sh.should_sample(0.01));
        assert!(!sh.should_sample(0.005));
    }

    #[test]
    fn sample_hold_phase_offset() {
        let sh = SampleHold::new(10.0, 0.5);
        // First sample at t = 0.5 * 0.1 = 0.05
        assert!(!sh.should_sample(0.0));
        assert!(sh.should_sample(0.05));
        assert!(!sh.should_sample(0.1));
        assert!(sh.should_sample(0.15)); // 0.05 + 1
        assert!(sh.should_sample(0.25)); // 0.05 + 2
    }

    #[test]
    #[should_panic(expected = "sample_rate must be positive")]
    fn sample_hold_zero_rate_panics() {
        SampleHold::new(0.0, 0.0);
    }

    // ── Resample ─────────────────────────────────────────────────────────

    #[test]
    fn resample_up() {
        // Upsample a 2 Hz signal to 4 Hz.
        let input = vec![0.0, 1.0, 2.0, 3.0, 4.0];
        let out = resample(&input, 2.0, 4.0);
        // Length: ceil(5 * 4 / 2) = ceil(10) = 10 ... Actually:
        // t_max = 4/2 = 2.0, n_out = ceil(2.0 * 4.0) + 1 = 9
        assert_eq!(out.len(), 9);
        approx_eq(out[0], 0.0);
        approx_eq(out[2], 1.0);
        approx_eq(out[4], 2.0);
    }

    #[test]
    fn resample_down() {
        // Downsample a 4 Hz signal to 2 Hz.
        let input: Vec<Scalar> = (0..9).map(|i| i as Scalar).collect(); // 0..8
        let out = resample(&input, 4.0, 2.0);
        assert_eq!(out.len(), 5);
        approx_eq(out[0], 0.0);
        approx_eq(out[1], 2.0);
        approx_eq(out[2], 4.0);
    }

    #[test]
    fn resample_same_rate() {
        let input = vec![1.0, 2.0, 3.0];
        let out = resample(&input, 10.0, 10.0);
        assert_eq!(out, input);
    }

    #[test]
    fn resample_empty_input() {
        let out = resample(&[], 10.0, 20.0);
        assert!(out.is_empty());
    }

    #[test]
    fn resample_zero_rate() {
        let input = vec![1.0, 2.0];
        let out = resample(&input, 0.0, 10.0);
        assert!(out.is_empty());
    }

    // ── Linear Interpolate ───────────────────────────────────────────────

    #[test]
    fn linear_interpolate_basic() {
        let y = linear_interpolate(0.5, 0.0, 10.0, 0.0, 1.0);
        approx_eq(y, 5.0);
    }

    #[test]
    fn linear_interpolate_left_clamp() {
        let y = linear_interpolate(-1.0, 0.0, 10.0, 0.0, 1.0);
        approx_eq(y, 0.0);
    }

    #[test]
    fn linear_interpolate_right_clamp() {
        let y = linear_interpolate(2.0, 0.0, 10.0, 0.0, 1.0);
        approx_eq(y, 10.0);
    }

    #[test]
    fn linear_interpolate_exact() {
        let y = linear_interpolate(0.0, 5.0, 20.0, 0.0, 5.0);
        approx_eq(y, 5.0);
        let y = linear_interpolate(5.0, 5.0, 20.0, 0.0, 5.0);
        approx_eq(y, 20.0);
    }

    #[test]
    #[should_panic(expected = "t0 and t1 must differ")]
    fn linear_interpolate_equal_times_panics() {
        linear_interpolate(0.5, 0.0, 1.0, 1.0, 1.0);
    }
}
