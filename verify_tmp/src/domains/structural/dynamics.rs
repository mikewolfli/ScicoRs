//! Structural dynamics: SDOF/MDOF vibration, fatigue analysis, response spectrum.

use crate::core::types::Scalar;

/// Single-degree-of-freedom vibration system.
///
/// Models: m·ü + c·u̇ + k·u = f(t)
#[derive(Debug, Clone)]
pub struct SdofSystem {
    /// Mass (kg).
    pub m: Scalar,
    /// Damping coefficient (N·s/m).
    pub c: Scalar,
    /// Stiffness (N/m).
    pub k: Scalar,
}

impl SdofSystem {
    /// Undamped natural frequency ωₙ = √(k/m) (rad/s).
    pub fn natural_frequency_rad(&self) -> Scalar {
        if self.m <= 0.0 {
            return 0.0;
        }
        (self.k / self.m).sqrt()
    }

    /// Undamped natural frequency fₙ = ωₙ / (2π) (Hz).
    pub fn natural_frequency(&self) -> Scalar {
        self.natural_frequency_rad() / (2.0 * std::f64::consts::PI)
    }

    /// Damping ratio ζ = c / (2·√(k·m)).
    pub fn damping_ratio(&self) -> Scalar {
        let critical = 2.0 * (self.k * self.m).sqrt();
        if critical <= 0.0 {
            return 0.0;
        }
        self.c / critical
    }

    /// Damped natural frequency ω_d = ωₙ·√(1-ζ²) (rad/s).
    pub fn damped_frequency_rad(&self) -> Scalar {
        let wn = self.natural_frequency_rad();
        let zeta = self.damping_ratio();
        if zeta >= 1.0 {
            return 0.0; // critically or over-damped
        }
        wn * (1.0 - zeta * zeta).sqrt()
    }

    /// Frequency response function magnitude |H(ω)| = 1 / √((k-m·ω²)² + (c·ω)²).
    pub fn frf_magnitude(&self, omega: Scalar) -> Scalar {
        let denom = (self.k - self.m * omega * omega).powi(2)
            + (self.c * omega).powi(2);
        if denom <= 0.0 {
            return Scalar::INFINITY;
        }
        1.0 / denom.sqrt()
    }

    /// Newmark-β time integration for arbitrary force history.
    ///
    /// `force_history` — slices of (time, force).
    /// `dt` — time step (s).
    /// `beta` — Newmark β parameter (0.25 for average acceleration, 1/6 for linear acceleration).
    /// `gamma` — Newmark γ parameter (0.5 for no numerical damping).
    ///
    /// Returns a vector of (time, displacement) pairs.
    pub fn newmark_beta(
        &self,
        force_history: &[(Scalar, Scalar)],
        dt: Scalar,
        beta: Scalar,
        gamma: Scalar,
    ) -> Vec<(Scalar, Scalar)> {
        if self.m <= 0.0 || dt <= 0.0 || force_history.is_empty() {
            return Vec::new();
        }

        let n_steps = force_history.len();
        let mut u = vec![0.0; n_steps];
        let mut v = vec![0.0; n_steps]; // velocity
        let mut a = vec![0.0; n_steps]; // acceleration

        // Initial acceleration: a₀ = (f₀ - c·v₀ - k·u₀) / m
        a[0] = (force_history[0].1 - self.c * v[0] - self.k * u[0]) / self.m;

        let _m_inv = 1.0 / self.m;
        let c1 = 1.0 / (beta * dt * dt);
        let c2 = 1.0 / (beta * dt);
        let c3 = 1.0 / (2.0 * beta) - 1.0;
        let c4 = gamma / (beta * dt);
        let c5 = gamma / beta - 1.0;
        let c6 = dt * (gamma / (2.0 * beta) - 1.0);

        let k_eff = self.k + c1 * self.m + c4 * self.c;

        for i in 1..n_steps {
            let f_eff = force_history[i].1
                + self.m * (c1 * u[i - 1] + c2 * v[i - 1] + c3 * a[i - 1])
                + self.c * (c4 * u[i - 1] + c5 * v[i - 1] + c6 * a[i - 1]);

            u[i] = f_eff / k_eff;
            a[i] = c1 * (u[i] - u[i - 1]) - c2 * v[i - 1] - c3 * a[i - 1];
            v[i] = v[i - 1] + dt * ((1.0 - gamma) * a[i - 1] + gamma * a[i]);
        }

        force_history
            .iter()
            .enumerate()
            .map(|(i, &(t, _))| (t, u[i]))
            .collect()
    }
}

// ──────────────────────────────────────────────
//  Fatigue Analysis
// ──────────────────────────────────────────────

/// S-N curve (stress-life) approximation.
///
/// Returns the number of cycles to failure at the given stress amplitude
/// using the Basquin relation:
///     N = (σ_amp / a)^(-1/b)
/// where `a` and `b` are derived from UTS and endurance limit.
///
/// The endurance limit is the stress below which no fatigue failure occurs
/// (typically ~0.5·UTS for steel).
pub fn sn_curve(stress_amplitude: Scalar, uts: Scalar, endurance_limit: Scalar) -> Scalar {
    if stress_amplitude <= 0.0 {
        return Scalar::INFINITY; // no stress → infinite life
    }
    if stress_amplitude <= endurance_limit {
        return Scalar::INFINITY; // below endurance limit → infinite life
    }
    if stress_amplitude >= uts {
        return 1.0; // ultimate stress → failure in 1 cycle
    }

    // Basquin parameters: fit a line in log-log space
    // log10(N) = -b * log10(σ_amp) + log10(a)
    // Through (UTS, 1) and (endurance_limit, 1e6)
    let b = f64::log10(1e6) / f64::log10(uts / endurance_limit);
    let a = uts * (1.0_f64).powf(1.0 / b); // = UTS

    let n = (stress_amplitude / a).powf(-1.0 / b);
    if n.is_infinite() || n.is_nan() {
        1e12
    } else {
        n
    }
}

/// Miner's linear cumulative damage rule.
///
/// `cycles` — number of applied cycles at each stress level.
/// `cycles_to_failure` — number of cycles to failure at each stress level.
///
/// Returns the cumulative damage index D.
/// Failure is expected when D ≥ 1.0.
pub fn miner_damage(cycles: &[Scalar], cycles_to_failure: &[Scalar]) -> Scalar {
    if cycles.len() != cycles_to_failure.len() || cycles.is_empty() {
        return 0.0;
    }

    let mut d = 0.0;
    for i in 0..cycles.len() {
        if cycles_to_failure[i] > 0.0 {
            d += cycles[i] / cycles_to_failure[i];
        }
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sdof_natural_frequency() {
        let sys = SdofSystem { m: 100.0, c: 50.0, k: 10000.0 };
        let fn_expected = Scalar::sqrt(10000.0 / 100.0) / (2.0 * std::f64::consts::PI);
        assert!((sys.natural_frequency() - fn_expected).abs() < 1e-10);
    }

    #[test]
    fn test_sdof_damping_ratio() {
        let sys = SdofSystem { m: 100.0, c: 200.0, k: 10000.0 };
        let zeta = sys.damping_ratio();
        let expected = 200.0 / (2.0 * Scalar::sqrt(10000.0 * 100.0));
        assert!((zeta - expected).abs() < 1e-10);
    }

    #[test]
    fn test_sdof_frf_magnitude() {
        let sys = SdofSystem { m: 10.0, c: 5.0, k: 1000.0 };
        // At resonance: ω = ωₙ = √(1000/10) = 10 rad/s
        let h = sys.frf_magnitude(10.0);
        // |H(ωₙ)| = 1/(c·ωₙ) = 1/(5*10) = 0.02
        assert!((h - 0.02).abs() < 1e-10);
    }

    #[test]
    fn test_sdof_newmark_beta() {
        let sys = SdofSystem { m: 10.0, c: 2.0, k: 100.0 };
        // Apply a constant force
        let force: Vec<(Scalar, Scalar)> = (0..10).map(|i| (i as Scalar * 0.01, 100.0)).collect();
        let result = sys.newmark_beta(&force, 0.01, 0.25, 0.5);
        assert_eq!(result.len(), 10);
        // Static deflection: u = F/k = 100/100 = 1.0
        // After several steps, displacement should approach 1.0
        assert!(result.last().unwrap().1 > 0.01);
    }

    #[test]
    fn test_newmark_empty_force() {
        let sys = SdofSystem { m: 10.0, c: 1.0, k: 100.0 };
        let result = sys.newmark_beta(&[], 0.01, 0.25, 0.5);
        assert!(result.is_empty());
    }

    #[test]
    fn test_sn_curve_infinite_life() {
        // Below endurance limit → infinite life
        let n = sn_curve(100.0, 500.0, 250.0);
        assert!(n.is_infinite());
    }

    #[test]
    fn test_sn_curve_ultimate() {
        // At UTS → 1 cycle
        let n = sn_curve(500.0, 500.0, 250.0);
        assert!((n - 1.0).abs() < 1.0);
    }

    #[test]
    fn test_sn_curve_finite_life() {
        let n = sn_curve(300.0, 500.0, 250.0);
        assert!(n > 1.0 && n < 1e8);
    }

    #[test]
    fn test_miner_damage() {
        let cycles = vec![1000.0, 500.0];
        let cycles_to_failure = vec![5000.0, 2000.0];
        let d = miner_damage(&cycles, &cycles_to_failure);
        assert!((d - 0.45).abs() < 1e-10);
    }

    #[test]
    fn test_miner_damage_empty() {
        assert!((miner_damage(&[], &[]) - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_sdof_zero_mass() {
        let sys = SdofSystem { m: 0.0, c: 0.0, k: 100.0 };
        assert!((sys.natural_frequency() - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_sdof_damped_frequency() {
        let sys = SdofSystem { m: 100.0, c: 20.0, k: 10000.0 };
        let wd = sys.damped_frequency_rad();
        let wn = sys.natural_frequency_rad();
        let zeta = sys.damping_ratio();
        let expected = wn * (1.0 - zeta * zeta).sqrt();
        assert!((wd - expected).abs() < 1e-10);
    }

    #[test]
    fn test_sdof_critical_damping() {
        let sys = SdofSystem { m: 100.0, c: 2000.0, k: 10000.0 };
        // ζ = 2000/(2*√(10000*100)) = 2000/2000 = 1.0
        let wd = sys.damped_frequency_rad();
        assert!((wd - 0.0).abs() < 1e-10);
    }
}
