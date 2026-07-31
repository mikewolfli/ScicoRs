//! Base-2 Cooley-Tukey Fast Fourier Transform.
//!
//! Provides forward and inverse FFT for real and complex sequences.
//! The input length must be a power of two.

use crate::core::types::Scalar;

/// Compute the forward FFT of a complex sequence (in-place, radix-2 DIT).
///
/// `data` is interleaved real/imag pairs: [re0, im0, re1, im1, ...].
/// Length must be a power of two.
pub fn fft(data: &mut [Scalar]) -> Result<(), String> {
    let n = data.len() / 2;
    if n == 0 || (n & (n - 1)) != 0 {
        return Err("FFT length must be a power of two and non-zero".to_string());
    }
    if data.len() != 2 * n {
        return Err("Data length must be 2 * n (interleaved complex)".to_string());
    }

    // Bit-reversal permutation (standard radix-2 DIT pre-processing).
    // For each i, j is the bit-reversal of i; the increment carries through
    // the lower bits with an inner loop instead of XORing a constant, and the
    // loop starts at i=1 (i=0 always maps j to n/2, which would corrupt the
    // permutation with an extra swap).
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            data.swap(2 * i, 2 * j);
            data.swap(2 * i + 1, 2 * j + 1);
        }
    }

    // Twiddle table: W_n^k = exp(-2πi·k/n) for k in 0..n/2. Computed once (n/2
    // trig calls instead of n·log₂n/2 per FFT) and indexed per stage, so the
    // butterfly inner loop contains zero transcendental calls.
    let mut tw = Vec::with_capacity(n / 2);
    for k in 0..n / 2 {
        let ang = -2.0 * std::f64::consts::PI * k as Scalar / n as Scalar;
        tw.push((ang.cos(), ang.sin()));
    }

    // Cooley-Tukey radix-2 DIT
    let mut len = 2;
    while len <= n {
        let half_len = len / 2;
        let step = n / len; // table stride for this stage (W_{len}^j = W_n^{j·step})
        for i in (0..n).step_by(len) {
            for j in 0..half_len {
                let (w_re, w_im) = tw[j * step];

                let even_idx = 2 * (i + j);
                let odd_idx = 2 * (i + j + half_len);

                let t_re = data[odd_idx] * w_re - data[odd_idx + 1] * w_im;
                let t_im = data[odd_idx] * w_im + data[odd_idx + 1] * w_re;

                data[odd_idx] = data[even_idx] - t_re;
                data[odd_idx + 1] = data[even_idx + 1] - t_im;
                data[even_idx] += t_re;
                data[even_idx + 1] += t_im;
            }
        }
        len *= 2;
    }
    Ok(())
}

/// Compute the inverse FFT (in-place).
///
/// Same layout as `fft()`. Result is scaled by 1/n.
pub fn ifft(data: &mut [Scalar]) -> Result<(), String> {
    let n = data.len() / 2;
    // Conjugate
    for i in 1..data.len() {
        if i % 2 == 1 {
            data[i] = -data[i];
        }
    }
    fft(data)?;
    // Scale by 1/n and conjugate back
    let inv_n = 1.0 / n as Scalar;
    for i in 0..data.len() / 2 {
        let re_idx = 2 * i;
        let im_idx = 2 * i + 1;
        data[re_idx] *= inv_n;
        data[im_idx] = -data[im_idx] * inv_n;
    }
    Ok(())
}

/// Compute the power spectral density (magnitude squared) of a real signal.
///
/// Returns (frequencies, magnitudes) for the positive frequency half.
/// The input signal length must be a power of two.
pub fn power_spectrum(signal: &[Scalar]) -> Result<(Vec<Scalar>, Vec<Scalar>), String> {
    let n = signal.len();
    if n == 0 || (n & (n - 1)) != 0 {
        return Err("Signal length must be a power of two".to_string());
    }

    // Pack real signal into complex array
    let mut data = Vec::with_capacity(2 * n);
    for &s in signal {
        data.push(s);
        data.push(0.0);
    }

    fft(&mut data)?;

    let half_n = n / 2;
    let mut freqs = Vec::with_capacity(half_n);
    let mut mags = Vec::with_capacity(half_n);

    for i in 0..half_n {
        let re = data[2 * i];
        let im = data[2 * i + 1];
        let mag = (re * re + im * im).sqrt() / n as Scalar;
        // DC component: mag as-is; others: double for one-sided spectrum
        let mag_scaled = if i == 0 { mag } else { 2.0 * mag };
        freqs.push(i as Scalar / n as Scalar);
        mags.push(mag_scaled);
    }

    Ok((freqs, mags))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fft_roundtrip() {
        let mut data = vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        fft(&mut data).unwrap();
        ifft(&mut data).unwrap();
        assert!((data[0] - 1.0).abs() < 1e-10);
        assert!((data[2]).abs() < 1e-10);
    }

    #[test]
    fn test_power_spectrum_sine() {
        // Simple 8-point sine wave test
        let n = 8;
        let signal: Vec<Scalar> = (0..n)
            .map(|i| (2.0 * std::f64::consts::PI * 1.0 * i as Scalar / n as Scalar).sin())
            .collect();

        let (freqs, mags) = power_spectrum(&signal).unwrap();
        // Should have a peak at bin 1 (normalized: 1/8 = 0.125)
        assert!((freqs[1] - 1.0 / 8.0).abs() < 0.01);
        assert!(mags[1] > 0.3);
    }

    #[test]
    fn test_fft_invalid_length() {
        let mut data = vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        assert!(fft(&mut data).is_err());
    }

    #[test]
    fn test_fft_empty() {
        let mut data = vec![];
        assert!(fft(&mut data).is_err());
    }

    #[test]
    fn test_power_spectrum_dc() {
        let signal = vec![1.0; 8];
        let (_, mags) = power_spectrum(&signal).unwrap();
        assert!((mags[0] - 1.0).abs() < 1e-10);
    }

    /// Verify the FFT against a direct O(n²) DFT. This catches bit-reversal
    /// and twiddle-factor errors that a round-trip (forward+inverse sharing
    /// the same permutation) would mask.
    #[test]
    fn test_fft_matches_direct_dft() {
        let n = 16;
        let x: Vec<Scalar> = (0..n)
            .map(|i| {
                (3.0 * std::f64::consts::PI * i as Scalar / n as Scalar).sin()
                    + 0.5 * (i as Scalar).cos()
            })
            .collect();
        let mut data = Vec::with_capacity(2 * n);
        for &v in &x {
            data.push(v);
            data.push(0.0);
        }

        fft(&mut data).unwrap();

        // Direct DFT for reference.
        for k in 0..n {
            let mut re = 0.0;
            let mut im = 0.0;
            for i in 0..n {
                let angle = -2.0 * std::f64::consts::PI * k as Scalar * i as Scalar / n as Scalar;
                re += x[i] * angle.cos();
                im += x[i] * angle.sin();
            }
            assert!(
                (data[2 * k] - re).abs() < 1e-9,
                "bin {} real: FFT {} vs DFT {}",
                k,
                data[2 * k],
                re
            );
            assert!(
                (data[2 * k + 1] - im).abs() < 1e-9,
                "bin {} imag: FFT {} vs DFT {}",
                k,
                data[2 * k + 1],
                im
            );
        }
    }

    /// An 8-point impulse should produce an all-ones spectrum.
    #[test]
    fn test_fft_impulse_spectrum() {
        let mut data = vec![
            1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        ];
        fft(&mut data).unwrap();
        for k in 0..4 {
            assert!((data[2 * k] - 1.0).abs() < 1e-10, "bin {} should be 1.0", k);
            assert!(data[2 * k + 1].abs() < 1e-10);
        }
    }

    /// The sine power-spectrum peak should be exactly at magnitude ~0.5
    /// (one-sided, amplitude 1 → magnitude n/2 → /n*2 = 1... for a real sine
    /// of amplitude A the one-sided peak is A).
    #[test]
    fn test_power_spectrum_sine_exact() {
        let n = 16;
        let signal: Vec<Scalar> = (0..n)
            .map(|i| 2.0 * (2.0 * std::f64::consts::PI * 2.0 * i as Scalar / n as Scalar).sin())
            .collect();
        let (_, mags) = power_spectrum(&signal).unwrap();
        // Frequency bin 2, amplitude 2.0 → one-sided peak = 2.0.
        assert!((mags[2] - 2.0).abs() < 1e-9);
        // All other bins (excluding DC and the mirror) should be ~0.
        for k in (1..n / 2).filter(|&k| k != 2) {
            assert!(mags[k] < 1e-9, "bin {} should be ~0, got {}", k, mags[k]);
        }
    }
}
