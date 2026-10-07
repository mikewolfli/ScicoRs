// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Experiment design and batch aggregation (Phase 36).
//!
//! Provides a full-factorial factor design, a sample plan that maps design
//! points to reproducible run IDs, and an aggregator that collects batch
//! outcomes into a single table for analysis. This reuses the existing
//! execution machinery rather than duplicating task scheduling.

use crate::core::types::Scalar;

/// A single factor (experimental variable) and its levels.
#[derive(Debug, Clone, PartialEq)]
pub struct FactorLevel {
    /// Factor name (a stable parameter path).
    pub name: String,
    /// The discrete levels to test.
    pub levels: Vec<Scalar>,
}

impl FactorLevel {
    /// Construct a factor with the given levels. Rejects an empty level set.
    pub fn new(name: &str, levels: Vec<Scalar>) -> Result<Self, String> {
        if levels.is_empty() {
            return Err(format!("factor '{name}' has no levels"));
        }
        for &l in &levels {
            if !l.is_finite() {
                return Err(format!("factor '{name}' has a non-finite level {l}"));
            }
        }
        Ok(Self {
            name: name.to_string(),
            levels,
        })
    }
}

/// A full-factorial design: every combination of factor levels.
#[derive(Debug, Clone, PartialEq)]
pub struct FactorDesign {
    /// The factors, in order. Order defines the column order of each point.
    pub factors: Vec<FactorLevel>,
}

impl FactorDesign {
    /// Build a design from factors. Rejects an empty factor list or duplicate
    /// factor names.
    pub fn new(factors: Vec<FactorLevel>) -> Result<Self, String> {
        if factors.is_empty() {
            return Err("a design needs at least one factor".to_string());
        }
        for (i, f) in factors.iter().enumerate() {
            for g in factors.iter().skip(i + 1) {
                if f.name == g.name {
                    return Err(format!("duplicate factor name '{}'", f.name));
                }
            }
        }
        Ok(Self { factors })
    }

    /// Total number of design points (the product of the level counts).
    pub fn point_count(&self) -> usize {
        self.factors.iter().map(|f| f.levels.len()).product()
    }

    /// Enumerate every design point, in row-major (last factor varies fastest)
    /// order, so the generated plan is deterministic.
    pub fn points(&self) -> Vec<Vec<Scalar>> {
        let total = self.point_count();
        let mut out = Vec::with_capacity(total);
        for linear in 0..total {
            let mut rem = linear;
            let mut point = vec![0.0; self.factors.len()];
            for (k, f) in self.factors.iter().enumerate().rev() {
                let n = f.levels.len();
                let idx = rem % n;
                rem /= n;
                point[k] = f.levels[idx];
            }
            out.push(point);
        }
        out
    }
}

/// A planned experiment: a design plus the run IDs that will realize it.
#[derive(Debug, Clone, PartialEq)]
pub struct SamplePlan {
    /// The underlying design.
    pub design: FactorDesign,
    /// One row per design point, giving the value of each factor.
    pub rows: Vec<Vec<Scalar>>,
    /// A stable run ID per row (e.g. `exp-000003`).
    pub run_ids: Vec<String>,
}

impl SamplePlan {
    /// Build a plan from a design, assigning deterministic run IDs.
    pub fn from_design(design: FactorDesign) -> Self {
        let rows = design.points();
        let run_ids = (0..rows.len()).map(|i| format!("exp-{i:06}")).collect();
        Self {
            design,
            rows,
            run_ids,
        }
    }

    /// Number of rows.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether the plan is empty (cannot happen for a validated design).
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

/// One aggregated batch result: a design point and its output statistic.
#[derive(Debug, Clone, PartialEq)]
pub struct AggregatedResult {
    /// The run ID from the plan.
    pub run_id: String,
    /// The factor tuple.
    pub point: Vec<Scalar>,
    /// The extracted output statistic, when available.
    pub output: Option<Scalar>,
    /// Whether the run produced a valid output.
    pub success: bool,
}

/// Aggregate a set of batch outcomes (one output value per run) against a plan.
///
/// `outputs` must be aligned with `plan.rows`; a `None` entry marks a failed or
/// missing run and is preserved as `success = false`, never silently dropped.
pub fn aggregate_batches(
    plan: &SamplePlan,
    outputs: &[Option<Scalar>],
) -> Result<Vec<AggregatedResult>, String> {
    if outputs.len() != plan.rows.len() {
        return Err(format!(
            "outputs length {} != plan rows {}",
            outputs.len(),
            plan.rows.len()
        ));
    }
    Ok(plan
        .rows
        .iter()
        .enumerate()
        .map(|(i, point)| {
            let out = outputs[i];
            AggregatedResult {
                run_id: plan.run_ids[i].clone(),
                point: point.clone(),
                output: out,
                success: out.map(|v| v.is_finite()).unwrap_or(false),
            }
        })
        .collect())
}

/// Compute the mean output over successful results (fraction of successes is
/// available through [`fraction_successful`]).
pub fn fraction_successful(results: &[AggregatedResult]) -> Scalar {
    if results.is_empty() {
        0.0
    } else {
        results.iter().filter(|r| r.success).count() as Scalar / results.len() as Scalar
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_factorial_enumerates_all_points() {
        let d = FactorDesign::new(vec![
            FactorLevel::new("a", vec![0.0, 1.0]).unwrap(),
            FactorLevel::new("b", vec![10.0, 20.0, 30.0]).unwrap(),
        ])
        .unwrap();
        assert_eq!(d.point_count(), 6);
        let pts = d.points();
        assert_eq!(pts.len(), 6);
        // Deterministic order: last factor varies fastest.
        assert_eq!(pts[0], vec![0.0, 10.0]);
        assert_eq!(pts[1], vec![0.0, 20.0]);
        assert_eq!(pts[2], vec![0.0, 30.0]);
        assert_eq!(pts[3], vec![1.0, 10.0]);
        assert_eq!(pts[5], vec![1.0, 30.0]);
    }

    #[test]
    fn rejects_empty_and_duplicate_factors() {
        assert!(FactorLevel::new("a", vec![]).is_err());
        assert!(FactorDesign::new(vec![]).is_err());
        let f = FactorLevel::new("a", vec![1.0]).unwrap();
        assert!(FactorDesign::new(vec![f.clone(), f]).is_err());
    }

    #[test]
    fn plan_assigns_stable_ids() {
        let d = FactorDesign::new(vec![FactorLevel::new("a", vec![0.0, 1.0]).unwrap()]).unwrap();
        let plan = SamplePlan::from_design(d);
        assert_eq!(plan.len(), 2);
        assert_eq!(plan.run_ids[0], "exp-000000");
        assert_eq!(plan.run_ids[1], "exp-000001");
    }

    #[test]
    fn aggregate_preserves_failures() {
        let d =
            FactorDesign::new(vec![FactorLevel::new("a", vec![0.0, 1.0, 2.0]).unwrap()]).unwrap();
        let plan = SamplePlan::from_design(d);
        let outputs = vec![Some(1.0), None, Some(3.0)];
        let agg = aggregate_batches(&plan, &outputs).unwrap();
        assert_eq!(agg.len(), 3);
        assert!(agg[0].success);
        assert!(!agg[1].success);
        assert!(agg[2].success);
        assert!(agg[1].output.is_none());
        assert!((fraction_successful(&agg) - 2.0 / 3.0).abs() < 1e-12);
    }

    #[test]
    fn aggregate_rejects_length_mismatch() {
        let d = FactorDesign::new(vec![FactorLevel::new("a", vec![0.0, 1.0]).unwrap()]).unwrap();
        let plan = SamplePlan::from_design(d);
        assert!(aggregate_batches(&plan, &[Some(1.0)]).is_err());
    }
}
