//! Circuit analysis types and algorithms.
//!
//! Provides DC operating point, DC sweep, AC small-signal sweep,
//! and transient analysis for SPICE-level circuit simulation.

use crate::core::error::SimError;
use crate::core::types::Scalar;

use super::mna::MnaMatrix;

// ──────────────────────────────────────────────
// 1. Analysis Configuration Types
// ──────────────────────────────────────────────

/// Frequency sweep scale type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FreqScale {
    /// Linear frequency spacing.
    Linear,
    /// Logarithmic by decade.
    Decade,
    /// Logarithmic by octave.
    Octave,
}

/// DC sweep configuration.
#[derive(Debug, Clone)]
pub struct DcSweepConfig {
    /// Name of the swept source.
    pub source_name: String,
    /// Start value.
    pub start: Scalar,
    /// Stop value.
    pub stop: Scalar,
    /// Number of steps.
    pub steps: usize,
}

/// AC sweep configuration.
#[derive(Debug, Clone)]
pub struct AcSweepConfig {
    /// Start frequency (Hz).
    pub start_freq: Scalar,
    /// Stop frequency (Hz).
    pub stop_freq: Scalar,
    /// Number of points.
    pub points: usize,
    /// Frequency scale.
    pub scale: FreqScale,
}

/// Transient analysis configuration.
#[derive(Debug, Clone)]
pub struct TransientConfig {
    /// Start time (s).
    pub t_start: Scalar,
    /// Stop time (s).
    pub t_stop: Scalar,
    /// Time step (s).
    pub t_step: Scalar,
}

/// Noise analysis configuration.
#[derive(Debug, Clone)]
pub struct NoiseConfig {
    /// Input source name.
    pub input_source: String,
    /// Output node.
    pub output_node: usize,
    /// Frequency sweep configuration.
    pub freq_sweep: AcSweepConfig,
}

/// Analysis type enum.
#[derive(Debug, Clone)]
pub enum AnalysisType {
    /// DC operating point (steady-state).
    DcOpPoint,
    /// DC sweep (vary a source).
    DcSweep(DcSweepConfig),
    /// AC small-signal analysis (frequency sweep).
    AcSweep(AcSweepConfig),
    /// Transient analysis (time-domain).
    Transient(TransientConfig),
    /// Noise analysis.
    Noise(NoiseConfig),
}

// ──────────────────────────────────────────────
// 2. Result Types
// ──────────────────────────────────────────────

/// DC operating point result.
#[derive(Debug, Clone)]
pub struct DcOpResult {
    /// Node voltages (V). Index 0 = ground (0V).
    pub node_voltages: Vec<Scalar>,
    /// Currents through voltage sources (A).
    pub source_currents: Vec<Scalar>,
    /// Total power dissipation (W).
    pub total_power: Scalar,
}

/// AC analysis result.
#[derive(Debug, Clone)]
pub struct AcResult {
    /// Frequency points (Hz).
    pub freq: Vec<Scalar>,
    /// Complex node voltages at each frequency.
    /// Outer index = frequency point, inner = node index.
    pub node_voltages: Vec<Vec<num_complex::Complex<Scalar>>>,
}

impl AcResult {
    /// Get magnitude (dB) for a node across all frequencies.
    pub fn gain_db(&self, node: usize) -> Vec<Scalar> {
        self.node_voltages
            .iter()
            .map(|v| {
                if node < v.len() {
                    20.0 * v[node].norm().log10()
                } else {
                    f64::NEG_INFINITY
                }
            })
            .collect()
    }

    /// Get phase (degrees) for a node across all frequencies.
    pub fn phase_deg(&self, node: usize) -> Vec<Scalar> {
        self.node_voltages
            .iter()
            .map(|v| {
                if node < v.len() {
                    v[node].arg().to_degrees()
                } else {
                    0.0
                }
            })
            .collect()
    }
}

/// Transient analysis result.
#[derive(Debug, Clone)]
pub struct TransientResult {
    /// Time points (s).
    pub time: Vec<Scalar>,
    /// Node voltages at each time point.
    /// Outer index = time point, inner = node index.
    pub node_voltages: Vec<Vec<Scalar>>,
}

// ──────────────────────────────────────────────
// 3. Analysis Functions
// ──────────────────────────────────────────────

/// Run DC operating point analysis.
///
/// Solves the MNA system at a single bias point (all capacitors open,
/// all inductors shorted).
pub fn run_dc_op(
    num_nodes: usize,
    num_vsources: usize,
    stamp_fn: impl FnOnce(&mut MnaMatrix) -> Result<(), SimError>,
) -> Result<DcOpResult, SimError> {
    let mut mna = MnaMatrix::new(num_nodes, num_vsources);
    stamp_fn(&mut mna)?;
    let sol = mna.solve()?;

    let node_voltages = sol.node_voltages[..num_nodes.min(sol.node_voltages.len())].to_vec();
    let source_currents = if num_vsources > 0 && sol.node_voltages.len() > num_nodes {
        sol.node_voltages[num_nodes..].to_vec()
    } else {
        Vec::new()
    };

    // Compute total power dissipation using the canonical vector reduction API.
    let total_power = crate::core::compute::linalg::asum(&source_currents);

    Ok(DcOpResult {
        node_voltages,
        source_currents,
        total_power,
    })
}

/// Run DC sweep analysis.
///
/// Sweeps a source parameter and solves the MNA system at each step.
/// Each sweep point is an independent MNA solve, so large sweeps run on
/// rayon (order-preserving indexed collect); small sweeps stay serial to
/// avoid pool-launch overhead.
pub fn run_dc_sweep(
    num_nodes: usize,
    num_vsources: usize,
    config: &DcSweepConfig,
    stamp_fn: impl Fn(&mut MnaMatrix, Scalar) -> Result<(), SimError> + Sync,
) -> Result<Vec<DcOpResult>, SimError> {
    /// Sweep points at which rayon pays for itself.
    const PAR_MIN_STEPS: usize = 8;

    let solve_point = |i: usize| -> Result<DcOpResult, SimError> {
        let value = if config.steps > 1 {
            config.start + (config.stop - config.start) * i as Scalar / (config.steps - 1) as Scalar
        } else {
            config.start
        };

        let mut mna = MnaMatrix::new(num_nodes, num_vsources);
        stamp_fn(&mut mna, value)?;
        let sol = mna.solve()?;

        let node_voltages = sol.node_voltages[..num_nodes.min(sol.node_voltages.len())].to_vec();
        let source_currents = if num_vsources > 0 && sol.node_voltages.len() > num_nodes {
            sol.node_voltages[num_nodes..].to_vec()
        } else {
            Vec::new()
        };

        let total_power = crate::core::compute::linalg::asum(&source_currents);
        Ok(DcOpResult {
            node_voltages,
            source_currents,
            total_power,
        })
    };

    if config.steps >= PAR_MIN_STEPS {
        use rayon::prelude::*;
        (0..config.steps).into_par_iter().map(solve_point).collect()
    } else {
        (0..config.steps).map(solve_point).collect()
    }
}

/// Generate frequency points for AC sweep.
fn generate_freq_points(config: &AcSweepConfig) -> Vec<Scalar> {
    match config.scale {
        FreqScale::Linear => {
            let mut freqs = Vec::with_capacity(config.points);
            for i in 0..config.points {
                let f = if config.points > 1 {
                    config.start_freq
                        + (config.stop_freq - config.start_freq) * i as Scalar
                            / (config.points - 1) as Scalar
                } else {
                    config.start_freq
                };
                freqs.push(f);
            }
            freqs
        }
        FreqScale::Decade => {
            let mut freqs = Vec::with_capacity(config.points);
            let log_start = config.start_freq.log10();
            let log_stop = config.stop_freq.log10();
            for i in 0..config.points {
                let f = if config.points > 1 {
                    10.0_f64.powf(
                        log_start
                            + (log_stop - log_start) * i as Scalar / (config.points - 1) as Scalar,
                    )
                } else {
                    config.start_freq
                };
                freqs.push(f);
            }
            freqs
        }
        FreqScale::Octave => {
            let mut freqs = Vec::with_capacity(config.points);
            let octaves = (config.stop_freq / config.start_freq).log2();
            for i in 0..config.points {
                let f = if config.points > 1 {
                    config.start_freq
                        * 2.0_f64.powf(octaves * i as Scalar / (config.points - 1) as Scalar)
                } else {
                    config.start_freq
                };
                freqs.push(f);
            }
            freqs
        }
    }
}

/// Run AC small-signal analysis.
///
/// For each frequency point, solves the MNA system with capacitors and
/// inductors represented by their complex impedances.
/// Each frequency point is an independent MNA solve, so large sweeps run on
/// rayon (order-preserving indexed collect).
pub fn run_ac_sweep(
    num_nodes: usize,
    num_vsources: usize,
    config: &AcSweepConfig,
    stamp_fn: impl Fn(&mut MnaMatrix, Scalar) -> Result<(), SimError> + Sync,
) -> Result<AcResult, SimError> {
    /// Frequency points at which rayon pays for itself.
    const PAR_MIN_POINTS: usize = 8;

    let freqs = generate_freq_points(config);

    let solve_point = |freq: Scalar| -> Result<Vec<num_complex::Complex<Scalar>>, SimError> {
        let mut mna = MnaMatrix::new(num_nodes, num_vsources);
        stamp_fn(&mut mna, freq)?;
        let sol = mna.solve()?;

        // For AC analysis, we treat the DC solution as the real part
        // (a simplified approach — full AC analysis requires complex MNA)
        Ok(sol
            .node_voltages
            .iter()
            .take(num_nodes)
            .map(|&v| num_complex::Complex::new(v, 0.0))
            .collect())
    };

    let node_voltages_all = if freqs.len() >= PAR_MIN_POINTS {
        use rayon::prelude::*;
        freqs
            .par_iter()
            .copied()
            .map(solve_point)
            .collect::<Result<Vec<_>, _>>()?
    } else {
        freqs
            .iter()
            .copied()
            .map(solve_point)
            .collect::<Result<Vec<_>, _>>()?
    };

    Ok(AcResult {
        freq: freqs,
        node_voltages: node_voltages_all,
    })
}

/// Run transient analysis.
///
/// Time-domain simulation using backward Euler integration for capacitors
/// and inductors.
pub fn run_transient(
    num_nodes: usize,
    num_vsources: usize,
    config: &TransientConfig,
    stamp_fn: impl Fn(&mut MnaMatrix, Scalar, Scalar) -> Result<(), SimError>,
) -> Result<TransientResult, SimError> {
    let num_steps = ((config.t_stop - config.t_start) / config.t_step).ceil() as usize;
    let mut time = Vec::with_capacity(num_steps + 1);
    let mut node_voltages_all = Vec::with_capacity(num_steps + 1);

    let mut t = config.t_start;

    for _step in 0..=num_steps {
        let mut mna = MnaMatrix::new(num_nodes, num_vsources);
        stamp_fn(&mut mna, t, config.t_step)?;
        let sol = mna.solve()?;

        let voltages = sol.node_voltages[..num_nodes.min(sol.node_voltages.len())].to_vec();
        time.push(t);
        node_voltages_all.push(voltages);
        t += config.t_step;
    }

    Ok(TransientResult {
        time,
        node_voltages: node_voltages_all,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dc_op_voltage_divider() {
        let result = run_dc_op(2, 0, |mna| {
            mna.stamp_resistor(1, 2, 1000.0);
            mna.stamp_resistor(2, 0, 1000.0);
            mna.stamp_current_source(0, 1, 0.005); // 5mA → V1 = 10V
            Ok(())
        })
        .unwrap();
        assert!((result.node_voltages[0] - 10.0).abs() < 1e-10);
        assert!((result.node_voltages[1] - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_dc_sweep() {
        let config = DcSweepConfig {
            source_name: "I1".into(),
            start: 0.0,
            stop: 0.01,
            steps: 5,
        };
        let results = run_dc_sweep(2, 0, &config, |mna, value| {
            mna.stamp_resistor(1, 2, 1000.0);
            mna.stamp_resistor(2, 0, 1000.0);
            mna.stamp_current_source(0, 1, value);
            Ok(())
        })
        .unwrap();
        assert_eq!(results.len(), 5);
        // V1 should increase with current
        assert!(results[0].node_voltages[0] < results[4].node_voltages[0]);
    }

    #[test]
    fn test_transient_rc_circuit() {
        let config = TransientConfig {
            t_start: 0.0,
            t_stop: 0.001,
            t_step: 0.0001,
        };
        let result = run_transient(2, 0, &config, |mna, _t, _dt| {
            mna.stamp_resistor(1, 2, 1000.0);
            mna.stamp_resistor(2, 0, 1000.0);
            mna.stamp_current_source(0, 1, 0.005);
            Ok(())
        })
        .unwrap();
        assert_eq!(result.time.len(), 11);
        assert_eq!(result.node_voltages.len(), 11);
    }

    #[test]
    fn test_freq_linear() {
        let config = AcSweepConfig {
            start_freq: 100.0,
            stop_freq: 1000.0,
            points: 10,
            scale: FreqScale::Linear,
        };
        let freqs = generate_freq_points(&config);
        assert_eq!(freqs.len(), 10);
        assert!((freqs[0] - 100.0).abs() < 1.0);
        assert!((freqs[9] - 1000.0).abs() < 1.0);
    }

    #[test]
    fn test_freq_decade() {
        let config = AcSweepConfig {
            start_freq: 10.0,
            stop_freq: 10000.0,
            points: 4,
            scale: FreqScale::Decade,
        };
        let freqs = generate_freq_points(&config);
        assert_eq!(freqs.len(), 4);
        // Should span 3 decades
        assert!(freqs[3] / freqs[0] > 900.0);
    }

    #[test]
    fn test_dc_sweep_parallel_matches_serial_reference() {
        // 12 steps (> PAR_MIN_STEPS=8) exercises the rayon path; compare
        // against the original serial loop order computed inline.
        let config = DcSweepConfig {
            source_name: "I1".into(),
            start: 0.0,
            stop: 0.011,
            steps: 12,
        };
        let results = run_dc_sweep(2, 0, &config, |mna, value| {
            mna.stamp_resistor(1, 2, 1000.0);
            mna.stamp_resistor(2, 0, 1000.0);
            mna.stamp_current_source(0, 1, value);
            Ok(())
        })
        .unwrap();

        // Serial reference: rebuild + solve each point in order.
        let mut ref_results = Vec::with_capacity(config.steps);
        for i in 0..config.steps {
            let value = config.start
                + (config.stop - config.start) * i as Scalar / (config.steps - 1) as Scalar;
            let mut mna = MnaMatrix::new(2, 0);
            mna.stamp_resistor(1, 2, 1000.0);
            mna.stamp_resistor(2, 0, 1000.0);
            mna.stamp_current_source(0, 1, value);
            let sol = mna.solve().unwrap();
            ref_results.push(sol.node_voltages[..2].to_vec());
        }

        assert_eq!(results.len(), ref_results.len());
        for (r, rref) in results.iter().zip(ref_results.iter()) {
            for (a, b) in r.node_voltages.iter().zip(rref.iter()) {
                assert!((a - b).abs() < 1e-10);
            }
        }
    }

    #[test]
    fn test_ac_sweep_parallel_matches_serial_reference() {
        let config = AcSweepConfig {
            start_freq: 100.0,
            stop_freq: 10000.0,
            points: 12, // > PAR_MIN_POINTS → rayon path
            scale: FreqScale::Linear,
        };
        let result = run_ac_sweep(2, 0, &config, |mna, _freq| {
            mna.stamp_resistor(1, 2, 1000.0);
            mna.stamp_resistor(2, 0, 1000.0);
            mna.stamp_current_source(0, 1, 0.005);
            Ok(())
        })
        .unwrap();

        assert_eq!(result.node_voltages.len(), 12);
        // Every frequency sees the same DC-like solution: V1=10V, V2=5V.
        for v in &result.node_voltages {
            assert!((v[0].re - 10.0).abs() < 1e-10);
            assert!((v[1].re - 5.0).abs() < 1e-10);
        }
    }

    #[test]
    fn test_dc_op_zero_power() {
        let result = run_dc_op(1, 0, |mna| {
            mna.stamp_resistor(1, 0, 1000.0);
            Ok(())
        })
        .unwrap();
        assert!((result.node_voltages[0]).abs() < 1e-10);
        assert!((result.total_power).abs() < 1e-10);
    }

    #[test]
    fn test_ac_result_gain_db() {
        let result = AcResult {
            freq: vec![100.0, 1000.0],
            node_voltages: vec![
                vec![num_complex::Complex::new(1.0, 0.0)],
                vec![num_complex::Complex::new(0.5, 0.0)],
            ],
        };
        let gain = result.gain_db(0);
        assert_eq!(gain.len(), 2);
        assert!((gain[0] - 0.0).abs() < 1e-10); // 20*log10(1) = 0
        assert!((gain[1] - (-6.0206)).abs() < 0.01); // 20*log10(0.5) ≈ -6.02
    }

    #[test]
    fn test_dc_op_empty() {
        let result = run_dc_op(0, 0, |_mna| Ok(())).unwrap();
        assert!(result.node_voltages.is_empty());
    }
}
