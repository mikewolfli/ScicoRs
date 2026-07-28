//! Acoustic analysis tools: A-weighting, RT60, FRF, damping, octave bands.

use crate::core::types::Scalar;

/// Octave band center frequencies.
///
/// f_n = f_ref · 2^(n/N), where N=1 for octave, N=3 for 1/3 octave.
pub fn octave_band_center_frequencies(base_freq: Scalar, n_bands: usize, n: u32) -> Vec<Scalar> {
    let mut freqs = Vec::with_capacity(n_bands);
    for i in 0..n_bands {
        let f = base_freq * 2.0_f64.powf(i as Scalar / n as Scalar);
        freqs.push(f);
    }
    freqs
}

/// A-weighting correction (IEC 61672).
///
/// A(f) = 20·log₁₀(R_A(f)) where
/// R_A(f) = 12194²·f⁴ / ((f²+20.6²)·√((f²+107.7²)·(f²+737.9²))·(f²+12194²))
pub fn a_weighting(freq: Scalar) -> Scalar {
    if freq <= 0.0 {
        return f64::NEG_INFINITY;
    }
    let f2 = freq * freq;
    let num = 12194.0_f64.powi(2) * f2 * f2;
    let denom = (f2 + 20.6_f64.powi(2))
        * ((f2 + 107.7_f64.powi(2)) * (f2 + 737.9_f64.powi(2))).sqrt()
        * (f2 + 12194.0_f64.powi(2));
    if denom <= 0.0 {
        return f64::NEG_INFINITY;
    }
    let ra = num / denom;
    20.0 * ra.log10()
}

/// Equivalent continuous sound level Leq over a period.
///
/// Leq = 10·log₁₀((1/T)·∫p²(t) dt / p_ref²)
pub fn equivalent_sound_level(spl_trace: &[Scalar], duration: Scalar) -> Scalar {
    if duration <= 0.0 || spl_trace.is_empty() {
        return 0.0;
    }
    let mut sum = 0.0;
    for &spl in spl_trace {
        sum += 10.0_f64.powf(spl / 10.0);
    }
    10.0 * (sum / spl_trace.len() as Scalar).log10()
}

/// Frequency Response Function (H1 estimator).
///
/// H(f) = Gxy(f) / Gxx(f)
pub fn frequency_response_function(
    input_fft: &[num_complex::Complex<Scalar>],
    output_fft: &[num_complex::Complex<Scalar>],
) -> Vec<num_complex::Complex<Scalar>> {
    let n = input_fft.len().min(output_fft.len());
    let mut h = Vec::with_capacity(n);
    for i in 0..n {
        let gxx = input_fft[i].norm_sqr();
        let gxy = output_fft[i] * input_fft[i].conj();
        if gxx > 0.0 {
            h.push(gxy / gxx);
        } else {
            h.push(num_complex::Complex::new(0.0, 0.0));
        }
    }
    h
}

/// Damping ratio estimated from half-power bandwidth.
///
/// ζ = (f₂ - f₁) / (2·f₀)
/// f₀ = peak frequency, f₁/f₂ = -3 dB points.
pub fn damping_ratio_from_peak(peak_freq: Scalar, bandwidth_3db: Scalar) -> Scalar {
    if peak_freq <= 0.0 {
        return 0.0;
    }
    bandwidth_3db / (2.0 * peak_freq)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_octave_band_center_frequencies() {
        let freqs = octave_band_center_frequencies(125.0, 3, 1);
        assert_eq!(freqs.len(), 3);
        assert!((freqs[0] - 125.0).abs() < 0.01);
        assert!((freqs[1] - 250.0).abs() < 0.01);
        assert!((freqs[2] - 500.0).abs() < 0.01);
    }

    #[test]
    fn test_a_weighting_1khz() {
        let a = a_weighting(1000.0);
        // A-weighting at 1 kHz should be close to 0 dB
        assert!(a > -5.0 && a < 5.0);
    }

    #[test]
    fn test_a_weighting_100hz() {
        let a = a_weighting(100.0);
        // 100 Hz should be attenuated
        assert!(a < -10.0);
    }

    #[test]
    fn test_equivalent_sound_level_constant() {
        let leq = equivalent_sound_level(&[94.0, 94.0, 94.0], 3.0);
        assert!((leq - 94.0).abs() < 0.01);
    }

    #[test]
    fn test_frf_identity() {
        let x = vec![num_complex::Complex::new(1.0, 0.0); 4];
        let y = x.clone();
        let h = frequency_response_function(&x, &y);
        for hi in &h {
            assert!((hi.norm() - 1.0).abs() < 1e-10);
        }
    }

    #[test]
    fn test_damping_ratio() {
        let zeta = damping_ratio_from_peak(100.0, 10.0);
        assert!((zeta - 0.05).abs() < 1e-15);
    }

    #[test]
    fn test_a_weighting_high_freq() {
        let a = a_weighting(10000.0);
        assert!(a > -5.0 && a < 5.0);
    }
}
