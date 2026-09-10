//! Digital signal processing filters for the discrete simulation runtime.
//!
//! Provides FIR (Finite Impulse Response) filters, IIR (Infinite Impulse
//! Response) filters, and a fast Moving Average implementation, each
//! operating on `Scalar` (f64) samples.

use crate::core::types::Scalar;
use std::collections::VecDeque;

// ── FIR Filter ───────────────────────────────────────────────────────────────

/// A Finite Impulse Response (FIR) filter with configurable coefficients.
///
/// The filter maintains an internal sample buffer and computes the
/// convolution `y[n] = sum_{k=0}^{N-1} b[k] * x[n - k]` at each step.
#[derive(Debug, Clone)]
pub struct FIRFilter {
    /// Feedforward coefficients (impulse response).
    pub coefficients: Vec<Scalar>,
    /// Input sample buffer (most recent sample at the front).
    buffer: VecDeque<Scalar>,
}

impl FIRFilter {
    /// Create a new FIR filter from the given coefficient slice.
    ///
    /// The first coefficient `b[0]` corresponds to the current input sample,
    /// `b[1]` to the previous, etc. A zero-length coefficient vector is
    /// treated as a pass-through (y[n] = 0).
    pub fn new(coefficients: &[Scalar]) -> Self {
        let order = coefficients.len();
        let mut buffer = VecDeque::with_capacity(order);
        buffer.resize(order, 0.0);
        Self {
            coefficients: coefficients.to_vec(),
            buffer,
        }
    }

    /// Advance the filter by one sample.
    ///
    /// Returns the filtered output `y[n]` for the given `input` sample `x[n]`.
    pub fn step(&mut self, input: Scalar) -> Scalar {
        // Push the new sample to the front, popping the oldest off the back.
        self.buffer.push_front(input);
        self.buffer.pop_back();

        // Compute the dot product of coefficients with the buffer contents.
        let mut y = 0.0;
        for (k, coeff) in self.coefficients.iter().enumerate() {
            y += coeff * self.buffer[k];
        }
        y
    }

    /// Reset the internal sample buffer to zero.
    ///
    /// The filter coefficients are preserved.
    pub fn reset(&mut self) {
        for sample in self.buffer.iter_mut() {
            *sample = 0.0;
        }
    }

    /// Return the filter order (number of taps minus one).
    ///
    /// For an N-tap filter the order is `N - 1`.
    pub fn order(&self) -> usize {
        self.coefficients.len().saturating_sub(1)
    }

    /// Compute the frequency response at normalised angular frequency `omega`.
    ///
    /// `omega` is in radians per sample (0 … π for the Nyquist range).
    /// Returns `(magnitude, phase)` where:
    /// - `magnitude` — the linear gain at `omega`
    /// - `phase` — the phase shift in radians
    pub fn frequency_response(&self, omega: Scalar) -> (Scalar, Scalar) {
        let mut real = 0.0;
        let mut imag = 0.0;
        for (k, coeff) in self.coefficients.iter().enumerate() {
            let angle = omega * k as Scalar;
            real += coeff * angle.cos();
            imag -= coeff * angle.sin();
        }
        let magnitude = (real * real + imag * imag).sqrt();
        let phase = imag.atan2(real);
        (magnitude, phase)
    }
}

// ── IIR Filter ───────────────────────────────────────────────────────────────

/// An Infinite Impulse Response (IIR) filter with configurable feedforward
/// and feedback coefficients.
///
/// Implements the difference equation:
/// ```text
/// a[0] * y[n] = b[0] * x[n] + b[1] * x[n-1] + … - a[1] * y[n-1] - a[2] * y[n-2] - …
/// ```
///
/// The filter is **normalised** at construction: if `a[0]` differs from 1.0
/// all coefficients are divided by `a[0]` so that the leading feedback
/// coefficient becomes unity.
#[derive(Debug, Clone)]
pub struct IIRFilter {
    /// Feedforward coefficients (normalised so that a[0] ≡ 1).
    pub b: Vec<Scalar>,
    /// Feedback coefficients (normalised so that a[0] ≡ 1).
    pub a: Vec<Scalar>,
    /// Input sample buffer, length = b.len().
    x_buffer: VecDeque<Scalar>,
    /// Output sample buffer, length = a.len().
    y_buffer: VecDeque<Scalar>,
}

impl IIRFilter {
    /// Create a new IIR filter from feedforward and feedback coefficients.
    ///
    /// # Panics
    /// Panics if `a` is empty or `a[0]` is zero.
    pub fn new(b: &[Scalar], a: &[Scalar]) -> Self {
        assert!(
            !a.is_empty(),
            "IIR filter must have at least one feedback coefficient"
        );
        assert!(
            a[0] != 0.0,
            "Leading feedback coefficient a[0] must be non-zero"
        );

        // Normalise so that a[0] = 1.
        let a0 = a[0];
        let b_norm: Vec<Scalar> = b.iter().map(|c| c / a0).collect();
        let a_norm: Vec<Scalar> = a.iter().map(|c| c / a0).collect();

        let b_len = b_norm.len();
        let a_len = a_norm.len();
        let mut x_buf = VecDeque::with_capacity(b_len);
        x_buf.resize(b_len, 0.0);
        let mut y_buf = VecDeque::with_capacity(a_len);
        y_buf.resize(a_len, 0.0);

        Self {
            b: b_norm,
            a: a_norm,
            x_buffer: x_buf,
            y_buffer: y_buf,
        }
    }

    /// Advance the filter by one sample.
    ///
    /// Returns the filtered output `y[n]`.
    pub fn step(&mut self, input: Scalar) -> Scalar {
        // Push input to the front of the x buffer.
        self.x_buffer.push_front(input);
        self.x_buffer.pop_back();

        // Compute the output: sum(b_k * x[n-k]) - sum(a_k * y[n-k]) for k >= 1
        let mut y = 0.0;
        for (k, coeff) in self.b.iter().enumerate() {
            y += coeff * self.x_buffer[k];
        }
        for (k, coeff) in self.a.iter().enumerate().skip(1) {
            y -= coeff * self.y_buffer[k - 1];
        }

        // Save the output in the y buffer.
        self.y_buffer.push_front(y);
        self.y_buffer.pop_back();

        y
    }

    /// Reset all internal buffers to zero.
    pub fn reset(&mut self) {
        for sample in self.x_buffer.iter_mut() {
            *sample = 0.0;
        }
        for sample in self.y_buffer.iter_mut() {
            *sample = 0.0;
        }
    }

    /// Return the filter order.
    ///
    /// This is `max(len(b), len(a)) - 1`.
    pub fn order(&self) -> usize {
        usize::max(self.b.len(), self.a.len()).saturating_sub(1)
    }

    /// Check whether the filter is BIBO stable.
    ///
    /// Stability requires that all poles (roots of the denominator polynomial)
    /// lie strictly inside the unit circle.  This implementation:
    /// - For **order 0**: always stable.
    /// - For **order 1**: checks `|a[1]| < 1`.
    /// - For **order 2**: applies the Schur-Cohn / Jury criterion:
    ///   `|a[2]| < 1` and `|a[1]| < |1 + a[2]|`.
    /// - For **higher orders**: computes the spectral radius of the companion
    ///   matrix via power iteration.
    pub fn is_stable(&self) -> bool {
        let n = self.a.len() - 1; // true order of the denominator
        if n == 0 {
            return true;
        }
        // Order 1: single pole at -a[1].
        if n == 1 {
            return self.a[1].abs() < 1.0;
        }
        // Order 2: Schur-Cohn / Jury stability test.
        if n == 2 {
            return self.a[2].abs() < 1.0 && self.a[1].abs() < (1.0 + self.a[2]).abs();
        }
        // Higher orders: estimate spectral radius of the companion matrix.
        self.spectral_radius() < 1.0 - 1e-12
    }

    /// Estimate the spectral radius (largest eigenvalue magnitude) of the
    /// companion matrix of the denominator polynomial using power iteration.
    fn spectral_radius(&self) -> Scalar {
        let n = self.a.len() - 1;
        // Companion matrix (Frobenius companion) for
        // p(z) = a[0]*z^n + a[1]*z^(n-1) + ... + a[n]
        // After normalisation a[0] = 1.
        let c_last: Vec<Scalar> = (1..=n).map(|i| -self.a[i]).collect();

        // Initial random-ish vector.
        let mut v: Vec<Scalar> = (0..n).map(|i| (i as Scalar + 1.0).cos()).collect();
        let mut prev_lambda = 0.0;

        for _iter in 0..200 {
            // Multiply v by the companion matrix.
            // C * v where:
            //   row 0: [0, 0, ..., 0, c_last[n-1]]
            //   row 1: [1, 0, ..., 0, c_last[n-2]]
            //   ...
            //   row n-1: [0, 0, ..., 1, c_last[0]]
            let w_last = v
                .iter()
                .enumerate()
                .map(|(i, &vi)| c_last[i] * vi)
                .sum::<Scalar>();
            let mut w = Vec::with_capacity(n);
            w.push(w_last);
            for &vi in v.iter().take(n - 1) {
                w.push(vi);
            }

            // Rayleigh quotient approximation: lambda ≈ w·v / v·v
            let v_dot = v.iter().map(|&x| x * x).sum::<Scalar>();
            let w_dot_v = w
                .iter()
                .zip(v.iter())
                .map(|(wi, vi)| wi * vi)
                .sum::<Scalar>();
            let lambda = if v_dot > 0.0 { w_dot_v / v_dot } else { 0.0 };

            if (lambda - prev_lambda).abs() < 1e-10 {
                return lambda.abs();
            }
            prev_lambda = lambda;

            // Normalise w into v.
            let norm = w.iter().map(|&x| x * x).sum::<Scalar>().sqrt();
            if norm < 1e-300 {
                return 0.0;
            }
            for (vi, &wi) in v.iter_mut().zip(w.iter()) {
                *vi = wi / norm;
            }
        }

        prev_lambda.abs()
    }
}

// ── Moving Average ───────────────────────────────────────────────────────────

/// A sliding-window moving average filter.
///
/// Maintains a running sum of the last `window_size` samples for O(1) per-step
/// cost (amortised).
#[derive(Debug, Clone)]
pub struct MovingAverage {
    /// Number of samples in the window.
    window_size: usize,
    /// Circular buffer of samples in the window.
    buffer: VecDeque<Scalar>,
    /// Running sum of all samples currently in the buffer.
    sum: Scalar,
}

impl MovingAverage {
    /// Create a new moving-average filter with the given `window_size`.
    ///
    /// # Panics
    /// Panics if `window_size` is zero.
    pub fn new(window_size: usize) -> Self {
        assert!(
            window_size > 0,
            "MovingAverage window_size must be positive"
        );
        let mut buffer = VecDeque::with_capacity(window_size);
        buffer.resize(window_size, 0.0);
        Self {
            window_size,
            buffer,
            sum: 0.0,
        }
    }

    /// Feed the next input sample and return the current moving average.
    pub fn step(&mut self, input: Scalar) -> Scalar {
        // Remove the oldest sample from the sum.
        let oldest = self.buffer.pop_back().unwrap_or(0.0);
        self.sum -= oldest;

        // Insert the new sample at the front.
        self.buffer.push_front(input);
        self.sum += input;

        self.sum / self.window_size as Scalar
    }

    /// Reset the buffer to zero.
    pub fn reset(&mut self) {
        for sample in self.buffer.iter_mut() {
            *sample = 0.0;
        }
        self.sum = 0.0;
    }

    /// Return the current output without consuming a sample.
    pub fn output(&self) -> Scalar {
        self.sum / self.window_size as Scalar
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── FIR ──────────────────────────────────────────────────────────────

    #[test]
    fn fir_creation() {
        let coeffs = vec![0.25, 0.5, 0.25];
        let fir = FIRFilter::new(&coeffs);
        assert_eq!(fir.coefficients.len(), 3);
        assert_eq!(fir.order(), 2);
    }

    #[test]
    fn fir_impulse_response() {
        // A simple 3-tap averaging filter: [1/3, 1/3, 1/3].
        let coeffs = vec![1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0];
        let mut fir = FIRFilter::new(&coeffs);
        // Impulse: x[0] = 1, x[1] = 0, x[2] = 0, ...
        let y0 = fir.step(1.0);
        approx_eq(y0, 1.0 / 3.0);
        let y1 = fir.step(0.0);
        approx_eq(y1, 1.0 / 3.0);
        let y2 = fir.step(0.0);
        approx_eq(y2, 1.0 / 3.0);
        let y3 = fir.step(0.0);
        approx_eq(y3, 0.0);
    }

    #[test]
    fn fir_step_response() {
        // Unit step response of [0.5, 0.5].
        let mut fir = FIRFilter::new(&[0.5, 0.5]);
        let y0 = fir.step(1.0);
        approx_eq(y0, 0.5);
        let y1 = fir.step(1.0);
        approx_eq(y1, 1.0);
        let y2 = fir.step(1.0);
        approx_eq(y2, 1.0);
    }

    #[test]
    fn fir_reset() {
        let coeffs = vec![0.2, 0.8];
        let mut fir = FIRFilter::new(&coeffs);
        fir.step(5.0);
        fir.step(10.0);
        fir.reset();
        // After reset, the buffer is zero, so the next output is 0.2*input
        // (only the newest sample matters since the delayed ones are zero).
        let y = fir.step(1.0);
        approx_eq(y, 0.2 * 1.0 + 0.8 * 0.0);
    }

    #[test]
    fn fir_frequency_response_dc() {
        let fir = FIRFilter::new(&[0.5, 0.5]);
        let (mag, phase) = fir.frequency_response(0.0);
        approx_eq(mag, 1.0);
        approx_eq(phase, 0.0);
    }

    #[test]
    fn fir_frequency_response_nyquist() {
        // [0.5, -0.5] has response 1.0 at omega=π, phase=0.
        let fir = FIRFilter::new(&[0.5, -0.5]);
        let (mag, phase) = fir.frequency_response(std::f64::consts::PI);
        approx_eq(mag, 1.0);
        approx_eq(phase, 0.0);
    }

    #[test]
    fn fir_zero_coefficients() {
        let mut fir = FIRFilter::new(&[]);
        assert_eq!(fir.order(), 0);
        let y = fir.step(42.0);
        approx_eq(y, 0.0);
    }

    // ── IIR ──────────────────────────────────────────────────────────────

    #[test]
    fn iir_creation() {
        // Simple first-order low-pass: a = [1, -0.9], b = [0.1]
        let iir = IIRFilter::new(&[0.1], &[1.0, -0.9]);
        assert_eq!(iir.a.len(), 2);
        assert_eq!(iir.b.len(), 1);
        assert_eq!(iir.order(), 1);
    }

    #[test]
    fn iir_lowpass() {
        // y[n] = 0.1 * x[n] + 0.9 * y[n-1]  (a=[1, -0.9], b=[0.1])
        let mut iir = IIRFilter::new(&[0.1], &[1.0, -0.9]);
        // Apply a unit step.
        let y0 = iir.step(1.0);
        approx_eq(y0, 0.1);
        let y1 = iir.step(1.0);
        approx_eq(y1, 0.1 + 0.9 * 0.1);
        // After many steps the output should approach 1.0.
        let mut y = 0.0;
        for _ in 0..200 {
            y = iir.step(1.0);
        }
        assert!((y - 1.0).abs() < 1e-6, "expected ≈1.0, got {y}");
    }

    #[test]
    fn iir_stability_check() {
        // Stable: a = [1, -0.5] => pole at 0.5.
        let stable = IIRFilter::new(&[1.0], &[1.0, -0.5]);
        assert!(stable.is_stable());

        // Unstable: a = [1, -1.5] => pole at 1.5.
        let unstable = IIRFilter::new(&[1.0], &[1.0, -1.5]);
        assert!(!unstable.is_stable());

        // Marginally stable: a = [1, -1.0] => pole at 1.0 (oscillator).
        let marginal = IIRFilter::new(&[1.0], &[1.0, -1.0]);
        assert!(!marginal.is_stable());
    }

    #[test]
    fn iir_reset() {
        let mut iir = IIRFilter::new(&[0.1], &[1.0, -0.9]);
        iir.step(10.0);
        iir.step(10.0);
        iir.reset();
        // After reset, the filter behaves as if no history exists.
        let y = iir.step(1.0);
        approx_eq(y, 0.1);
    }

    #[test]
    fn iir_second_order_stable() {
        // 2nd-order Butterworth: a = [1, -1.561, 0.6414], b = [0.0201, 0.0402, 0.0201]
        let iir = IIRFilter::new(&[0.0201, 0.0402, 0.0201], &[1.0, -1.561, 0.6414]);
        assert!(iir.is_stable());
    }

    #[test]
    fn iir_second_order_unstable() {
        // Deliberately unstable 2nd-order: poles outside unit circle.
        let iir = IIRFilter::new(&[1.0], &[1.0, 0.0, -1.5]);
        assert!(!iir.is_stable());
    }

    // ── Moving Average ───────────────────────────────────────────────────

    #[test]
    fn moving_average_creation() {
        let ma = MovingAverage::new(4);
        assert_eq!(ma.window_size, 4);
        approx_eq(ma.output(), 0.0);
    }

    #[test]
    fn moving_average_sliding_window() {
        let mut ma = MovingAverage::new(3);
        approx_eq(ma.step(1.0), 1.0 / 3.0);
        approx_eq(ma.step(2.0), 3.0 / 3.0);
        approx_eq(ma.step(3.0), 6.0 / 3.0);
        // Oldest sample (1.0) drops out.
        approx_eq(ma.step(4.0), 9.0 / 3.0);
    }

    #[test]
    fn moving_average_reset() {
        let mut ma = MovingAverage::new(3);
        ma.step(10.0);
        ma.step(20.0);
        ma.reset();
        approx_eq(ma.output(), 0.0);
        approx_eq(ma.step(5.0), 5.0 / 3.0);
    }

    #[test]
    fn moving_average_output() {
        let mut ma = MovingAverage::new(2);
        ma.step(3.0);
        ma.step(7.0);
        approx_eq(ma.output(), 5.0);
    }

    #[test]
    fn moving_average_constant_input() {
        let mut ma = MovingAverage::new(5);
        for _ in 0..10 {
            ma.step(42.0);
        }
        approx_eq(ma.output(), 42.0);
    }

    // ── Helpers ──────────────────────────────────────────────────────────

    fn approx_eq(a: Scalar, b: Scalar) {
        assert!((a - b).abs() < 1e-10, "expected {b}, got {a}");
    }
}
