// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Uncertainty propagation and sampling (Phase 36).
//!
//! Provides Latin-hypercube and Monte-Carlo sampling over parameter
//! distributions, and a summary of the resulting output distribution: mean,
//! standard deviation, requested quantiles, confidence intervals and the
//! fraction of failed samples. A seeded, explicit RNG makes runs reproducible
//! and the seed is recorded in the result metadata.

use crate::analysis::objective::{ObjectiveSpec, ObservationSet, ParameterSpec};
use crate::analysis::simulation::SimulationFunction;
use crate::core::types::Scalar;

/// A small, self-contained, seedable xorshift RNG.
///
/// It is intentionally trivial so that a fixed seed produces an exactly
/// reproducible stream across platforms and runs; it is not used for
/// cryptography.
#[derive(Debug, Clone)]
pub struct SeededRng {
    state: u64,
}

impl SeededRng {
    /// Create an RNG from a 64-bit seed. A zero seed is remapped to a non-zero
    /// constant so the xorshift state never collapses.
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                seed
            },
        }
    }

    /// Next raw 64-bit value (xorshift64*).
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in `[0, 1)`.
    pub fn next_f64(&mut self) -> Scalar {
        (self.next_u64() >> 11) as Scalar / (1u64 << 53) as Scalar
    }

    /// Uniform in `[lo, hi)`.
    pub fn uniform(&mut self, lo: Scalar, hi: Scalar) -> Scalar {
        lo + (hi - lo) * self.next_f64()
    }

    /// Standard normal via the Box–Muller transform.
    pub fn normal(&mut self, mean: Scalar, std: Scalar) -> Scalar {
        let u1 = self.next_f64().max(1e-300);
        let u2 = self.next_f64();
        let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
        mean + std * z
    }

    /// Derive an independent child stream from this one (for parallel sub-streams
    /// that stay reproducible).
    pub fn split(&mut self, stream_index: u64) -> SeededRng {
        let mixed = self.next_u64() ^ stream_index.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        SeededRng::new(mixed)
    }
}

/// The sampling scheme to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SamplingScheme {
    /// Independent uniform/normal draws per parameter.
    MonteCarlo,
    /// Stratified sampling: each parameter's range is split into `n` equal
    /// intervals and sampled once each, then shuffled independently.
    LatinHypercube,
}

/// Configuration for an uncertainty run.
#[derive(Debug, Clone)]
pub struct UncertaintyConfig {
    /// Number of samples.
    pub n_samples: usize,
    /// Sampling scheme.
    pub scheme: SamplingScheme,
    /// Master seed (recorded in the result for reproducibility).
    pub seed: u64,
    /// Relative perturbation magnitude used to derive per-parameter spread when
    /// a parameter has no explicit distribution.
    pub relative_sigma: Scalar,
    /// Quantiles to report (each in `[0,1]`).
    pub quantiles: Vec<Scalar>,
    /// Confidence level for the reported interval (e.g. 0.95).
    pub confidence: Scalar,
}

impl Default for UncertaintyConfig {
    fn default() -> Self {
        Self {
            n_samples: 1000,
            scheme: SamplingScheme::MonteCarlo,
            seed: 0xC0FFEE,
            relative_sigma: 0.05,
            quantiles: vec![0.05, 0.5, 0.95],
            confidence: 0.95,
        }
    }
}

/// Summary statistics of a propagated output distribution.
#[derive(Debug, Clone, PartialEq)]
pub struct MonteCarloSummary {
    /// Arithmetic mean of successful samples.
    pub mean: Scalar,
    /// Sample standard deviation.
    pub std_dev: Scalar,
    /// Requested quantiles, paired with their probability.
    pub quantiles: Vec<(Scalar, Scalar)>,
    /// Symmetric confidence interval `(lo, hi)` at `config.confidence`.
    pub confidence_interval: (Scalar, Scalar),
    /// Number of successful samples.
    pub successes: usize,
    /// Number of failed samples.
    pub failures: usize,
    /// Standard error of the mean.
    pub standard_error: Scalar,
}

/// The result of an uncertainty propagation.
#[derive(Debug, Clone, PartialEq)]
pub struct UncertaintyResult {
    /// The output samples (only successful ones), in draw order.
    pub samples: Vec<Scalar>,
    /// Per-sample success flags, parallel to the *attempted* draws.
    pub success: Vec<bool>,
    /// Summary statistics.
    pub summary: MonteCarloSummary,
    /// Seed used (echoed for reproducibility).
    pub seed: u64,
    /// Sampling scheme used.
    pub scheme: SamplingScheme,
}

/// Generate a Latin-hypercube sample set in `[0,1]^d`.
///
/// Each of the `d` dimensions is stratified into `n` equal intervals with one
/// uniform draw per interval, then the per-dimension orders are independently
/// shuffled so the dimensions are decorrelated.
pub fn latin_hypercube(n: usize, d: usize, rng: &mut SeededRng) -> Vec<Vec<Scalar>> {
    let mut samples = vec![vec![0.0; d]; n];
    for dim in 0..d {
        // One jittered point per stratum.
        let mut perm: Vec<usize> = (0..n).collect();
        // Fisher–Yates shuffle for reproducibility.
        for i in (1..n).rev() {
            let j = (rng.next_u64() % (i as u64 + 1)) as usize;
            perm.swap(i, j);
        }
        for (i, &stratum) in perm.iter().enumerate() {
            let jitter = rng.next_f64();
            samples[i][dim] = (stratum as Scalar + jitter) / n as Scalar;
        }
    }
    samples
}

/// Propagate parameter uncertainty through a model and summarize the output.
///
/// For every sample the model is evaluated on the sampled parameter vector; the
/// resulting output value (at the final sample position, or the mean when the
/// record has several points) is collected. Failed evaluations are counted as
/// failures and never enter the statistics.
pub fn monte_carlo<M: SimulationFunction>(
    model: &M,
    specs: &[ParameterSpec],
    observations: &ObservationSet,
    objective: &ObjectiveSpec,
    config: &UncertaintyConfig,
) -> Result<UncertaintyResult, String> {
    if config.n_samples == 0 {
        return Err("n_samples must be >= 1".to_string());
    }
    if specs.is_empty() {
        return Err("no parameters supplied".to_string());
    }
    let d = specs.len();
    let mut rng = SeededRng::new(config.seed);

    // Build the unit-cube sample matrix.
    let unit_samples = match config.scheme {
        SamplingScheme::MonteCarlo => {
            let mut s = vec![vec![0.0; d]; config.n_samples];
            for row in s.iter_mut() {
                for v in row.iter_mut() {
                    *v = rng.next_f64();
                }
            }
            s
        }
        SamplingScheme::LatinHypercube => latin_hypercube(config.n_samples, d, &mut rng),
    };

    let mut samples = Vec::with_capacity(config.n_samples);
    let mut success = Vec::with_capacity(config.n_samples);

    for (i, unit) in unit_samples.iter().enumerate() {
        // Map unit-cube coordinates to a parameter vector.
        let params: Vec<Scalar> = specs
            .iter()
            .enumerate()
            .map(|(k, spec)| sample_parameter(spec, unit[k], config.relative_sigma, &mut rng))
            .collect();
        let run_id = format!("mc-{:06}", i);
        let rec = model.evaluate(&params, &run_id);
        if !rec.valid {
            success.push(false);
            continue;
        }
        let Some(ys) = rec.channel(&objective.output_channel) else {
            success.push(false);
            continue;
        };
        if ys.is_empty() || ys.iter().any(|v| !v.is_finite()) {
            success.push(false);
            continue;
        }
        // The output statistic: mean over the observation positions when present,
        // else the mean of the channel samples.
        let value = output_statistic(&rec, observations, objective);
        match value {
            Some(v) if v.is_finite() => {
                samples.push(v);
                success.push(true);
            }
            _ => success.push(false),
        }
    }

    let summary = summarize(&samples, config);
    Ok(UncertaintyResult {
        samples,
        success,
        summary,
        seed: config.seed,
        scheme: config.scheme,
    })
}

/// Map a unit-cube coordinate to a parameter value, respecting bounds and the
/// parameter's transform. Bounded parameters are sampled uniformly in their
/// range; unbounded ones use a Gaussian around the nominal value with a spread
/// of `relative_sigma · max(|value|, 1)`.
fn sample_parameter(
    spec: &ParameterSpec,
    unit: Scalar,
    relative_sigma: Scalar,
    rng: &mut SeededRng,
) -> Scalar {
    match (spec.lower, spec.upper) {
        (Some(lo), Some(hi)) => spec.clamp(lo + (hi - lo) * unit),
        _ => {
            let sigma = relative_sigma * spec.value.abs().max(1.0);
            spec.clamp(spec.value + sigma * standard_normal_from_unit(unit, rng))
        }
    }
}

/// Convert a single unit-cube value plus a fresh draw into an approximate
/// standard normal (keeps the RNG deterministic and seed-dependent).
fn standard_normal_from_unit(unit: Scalar, rng: &mut SeededRng) -> Scalar {
    let u1 = unit.max(1e-300);
    let u2 = rng.next_f64();
    (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
}

/// The output statistic used for one Monte-Carlo sample.
fn output_statistic(
    rec: &crate::analysis::simulation::SimulationRecord,
    obs: &ObservationSet,
    objective: &ObjectiveSpec,
) -> Option<Scalar> {
    let ys = rec.channel(&objective.output_channel)?;
    if obs.is_empty() {
        // Mean of the channel samples.
        if ys.is_empty() {
            return None;
        }
        return Some(ys.iter().sum::<Scalar>() / ys.len() as Scalar);
    }
    // Mean of the channel sampled at the observation positions.
    let mut acc = 0.0;
    let mut n = 0usize;
    for i in 0..obs.len() {
        if obs.is_missing(i) {
            continue;
        }
        if let Some(v) = rec.sample_at(&objective.output_channel, obs.positions[i]) {
            acc += v;
            n += 1;
        }
    }
    if n == 0 {
        None
    } else {
        Some(acc / n as Scalar)
    }
}

/// Compute summary statistics from a set of successful output samples.
fn summarize(samples: &[Scalar], config: &UncertaintyConfig) -> MonteCarloSummary {
    let n = samples.len();
    let total_attempts = config.n_samples;
    if n == 0 {
        return MonteCarloSummary {
            mean: Scalar::NAN,
            std_dev: Scalar::NAN,
            quantiles: config.quantiles.iter().map(|&q| (q, Scalar::NAN)).collect(),
            confidence_interval: (Scalar::NAN, Scalar::NAN),
            successes: 0,
            failures: total_attempts,
            standard_error: Scalar::NAN,
        };
    }
    let mean = samples.iter().sum::<Scalar>() / n as Scalar;
    let var = if n > 1 {
        samples
            .iter()
            .map(|&x| (x - mean) * (x - mean))
            .sum::<Scalar>()
            / (n - 1) as Scalar
    } else {
        0.0
    };
    let std_dev = var.sqrt();
    let standard_error = std_dev / (n as Scalar).sqrt();

    let mut sorted = samples.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let quantile_at = |q: Scalar| -> Scalar {
        let q = q.clamp(0.0, 1.0);
        if sorted.is_empty() {
            return Scalar::NAN;
        }
        let pos = q * (sorted.len() - 1) as Scalar;
        let lo = pos.floor() as usize;
        let hi = pos.ceil() as usize;
        if lo == hi {
            sorted[lo]
        } else {
            let w = pos - lo as Scalar;
            sorted[lo] * (1.0 - w) + sorted[hi] * w
        }
    };
    let quantiles: Vec<(Scalar, Scalar)> = config
        .quantiles
        .iter()
        .map(|&q| (q, quantile_at(q)))
        .collect();
    let alpha = (1.0 - config.confidence) / 2.0;
    let ci = (quantile_at(alpha), quantile_at(1.0 - alpha));

    MonteCarloSummary {
        mean,
        std_dev,
        quantiles,
        confidence_interval: ci,
        successes: n,
        failures: total_attempts - n,
        standard_error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::simulation::{FnSimulationFunction, SimulationRecord};

    fn rng_stream(seed: u64, n: usize) -> Vec<Scalar> {
        let mut rng = SeededRng::new(seed);
        (0..n).map(|_| rng.next_f64()).collect()
    }

    #[test]
    fn seeded_rng_is_reproducible() {
        let a = rng_stream(42, 32);
        let b = rng_stream(42, 32);
        assert_eq!(a, b);
        let c = rng_stream(43, 32);
        assert_ne!(a, c);
    }

    #[test]
    fn uniform_is_in_range() {
        let mut rng = SeededRng::new(7);
        for _ in 0..1000 {
            let v = rng.uniform(2.0, 5.0);
            assert!((2.0..5.0).contains(&v));
        }
    }

    #[test]
    fn normal_moments_are_reasonable() {
        let mut rng = SeededRng::new(123);
        let n = 20000;
        let mut sum = 0.0;
        let mut sumsq = 0.0;
        for _ in 0..n {
            let v = rng.normal(1.0, 2.0);
            sum += v;
            sumsq += v * v;
        }
        let mean = sum / n as Scalar;
        let var = sumsq / n as Scalar - mean * mean;
        assert!((mean - 1.0).abs() < 0.1, "mean={mean}");
        assert!((var.sqrt() - 2.0).abs() < 0.1, "std={}", var.sqrt());
    }

    #[test]
    fn latin_hypercube_is_stratified() {
        let mut rng = SeededRng::new(99);
        let n = 100;
        let d = 2;
        let samples = latin_hypercube(n, d, &mut rng);
        assert_eq!(samples.len(), n);
        // Every sample stays in [0,1).
        for s in &samples {
            for &v in s {
                assert!((0.0..1.0).contains(&v));
            }
        }
        // Stratification: each dimension should have one point per 1/n band.
        for dim in 0..d {
            let mut bins = vec![0usize; n];
            for s in &samples {
                let bin = (s[dim] * n as Scalar).floor() as usize;
                bins[bin.min(n - 1)] += 1;
            }
            assert!(bins.iter().all(|&c| c == 1), "not stratified in dim {dim}");
        }
    }

    #[test]
    fn split_streams_are_independent_and_reproducible() {
        let mut a = SeededRng::new(5);
        let mut b = SeededRng::new(5);
        let s1: Vec<Scalar> = (0..4).map(|i| a.split(i).next_f64()).collect();
        let s2: Vec<Scalar> = (0..4).map(|i| b.split(i).next_f64()).collect();
        assert_eq!(s1, s2);
        // Different stream indices give different values.
        assert_ne!(s1[0], s1[1]);
    }

    #[test]
    fn monte_carlo_propagates_linear_model() {
        // y = p·t at t=1 → y = p. Sampling p uniformly in [0,1], mean ≈ 0.5.
        let model = FnSimulationFunction::new(1, "scale", |p, id| {
            SimulationRecord::single("y", vec![1.0], vec![p[0]], id)
        });
        let specs = vec![ParameterSpec::free("p", 0.5).with_bounds(0.0, 1.0)];
        let obs = ObservationSet::new(vec![1.0], vec![0.5], vec![]).unwrap();
        let cfg = UncertaintyConfig {
            n_samples: 4000,
            scheme: SamplingScheme::LatinHypercube,
            seed: 2024,
            quantiles: vec![0.1, 0.5, 0.9],
            confidence: 0.95,
            ..Default::default()
        };
        let result =
            monte_carlo(&model, &specs, &obs, &ObjectiveSpec::for_output("y"), &cfg).unwrap();
        assert_eq!(result.summary.failures, 0);
        assert!(
            (result.summary.mean - 0.5).abs() < 0.05,
            "mean={}",
            result.summary.mean
        );
        // Median ≈ 0.5.
        let median = result
            .summary
            .quantiles
            .iter()
            .find(|(q, _)| (*q - 0.5).abs() < 1e-9)
            .unwrap()
            .1;
        assert!((median - 0.5).abs() < 0.05);
        // CI bracket the median.
        assert!(result.summary.confidence_interval.0 < median);
        assert!(result.summary.confidence_interval.1 > median);
    }

    #[test]
    fn monte_carlo_reproducible_for_fixed_seed() {
        let model = FnSimulationFunction::new(1, "scale", |p, id| {
            SimulationRecord::single("y", vec![1.0], vec![p[0] * 2.0], id)
        });
        let specs = vec![ParameterSpec::free("p", 1.0).with_bounds(0.0, 2.0)];
        let obs = ObservationSet::new(vec![1.0], vec![1.0], vec![]).unwrap();
        let cfg = UncertaintyConfig {
            n_samples: 500,
            seed: 77,
            ..Default::default()
        };
        let r1 = monte_carlo(&model, &specs, &obs, &ObjectiveSpec::for_output("y"), &cfg).unwrap();
        let r2 = monte_carlo(&model, &specs, &obs, &ObjectiveSpec::for_output("y"), &cfg).unwrap();
        assert_eq!(r1.samples, r2.samples);
        assert_eq!(r1.summary.mean, r2.summary.mean);
    }

    #[test]
    fn failures_are_counted_not_hidden() {
        let model = FnSimulationFunction::new(1, "sometimes-fails", |p, id| {
            if p[0] > 0.5 {
                SimulationRecord::failed(id, "diverged")
            } else {
                SimulationRecord::single("y", vec![1.0], vec![p[0]], id)
            }
        });
        let specs = vec![ParameterSpec::free("p", 0.2).with_bounds(0.0, 1.0)];
        let obs = ObservationSet::new(vec![1.0], vec![0.2], vec![]).unwrap();
        let cfg = UncertaintyConfig {
            n_samples: 1000,
            scheme: SamplingScheme::LatinHypercube,
            seed: 1,
            ..Default::default()
        };
        let result =
            monte_carlo(&model, &specs, &obs, &ObjectiveSpec::for_output("y"), &cfg).unwrap();
        assert!(result.summary.failures > 0);
        assert_eq!(result.summary.successes + result.summary.failures, 1000);
        // The reported failure fraction is real: roughly half exceed 0.5.
        let frac = result.summary.failures as Scalar / 1000.0;
        assert!((frac - 0.5).abs() < 0.1, "frac={frac}");
    }
}
