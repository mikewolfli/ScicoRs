// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Convergence control and coupling iteration scheduling.

use super::bus::{CouplingInterface, FieldData, PhysicsField};
use crate::core::types::Scalar;

/// Convergence criteria for coupled iterations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConvergenceCriteria {
    pub absolute_tolerance: Scalar,
    pub relative_tolerance: Scalar,
    pub max_iterations: usize,
    pub relaxation_factor: Scalar,
}

impl Default for ConvergenceCriteria {
    fn default() -> Self {
        Self {
            absolute_tolerance: 1e-8,
            relative_tolerance: 1e-6,
            max_iterations: 50,
            relaxation_factor: 0.5,
        }
    }
}

/// Running maximum that **propagates** non-finite values.
///
/// `f64::max` returns the non-NaN operand, so `0.0_f64.max(NaN) == 0.0`. Using it
/// to accumulate a residual silently turns a NaN (or overflow to infinity) into
/// `0.0`, which then passes any convergence test — reporting success on a broken
/// iterate. This helper keeps a non-finite value sticky so the check rejects it.
fn finite_max_propagating(acc: Scalar, value: Scalar) -> Scalar {
    if !acc.is_finite() {
        return acc;
    }
    if !value.is_finite() {
        return value;
    }
    acc.max(value)
}

/// Coupling solver scheduler.
pub struct CouplingScheduler {
    pub criteria: ConvergenceCriteria,
    pub interfaces: Vec<CouplingInterface>,
}

impl CouplingScheduler {
    pub fn new(criteria: ConvergenceCriteria) -> Self {
        Self {
            criteria,
            interfaces: Vec::new(),
        }
    }

    pub fn fixed_point_iteration(
        &self,
        initial_data: &[FieldData],
        compute_field: &dyn Fn(&[FieldData], PhysicsField) -> Result<FieldData, String>,
    ) -> Result<Vec<FieldData>, String> {
        let mut data = initial_data.to_vec();
        for _iter in 0..self.criteria.max_iterations {
            let mut new_data = Vec::new();
            for d in &data {
                let computed = compute_field(&data, d.field_type)?;
                // Relaxation: new = (1-ω)·old + ω·computed
                let omega = self.criteria.relaxation_factor;
                let relaxed = FieldData::new(
                    computed.field_type,
                    computed.quantity,
                    computed.points.clone(),
                    computed
                        .values
                        .iter()
                        .zip(d.values.iter())
                        .map(|(c, o)| (1.0 - omega) * o + omega * c)
                        .collect(),
                    computed.time,
                );
                new_data.push(relaxed);
            }
            // Check convergence against the absolute *and* relative criteria,
            // scaled by the magnitude of the data being iterated.
            let mut max_delta: Scalar = 0.0;
            let mut scale: Scalar = 0.0;
            for (new, old) in new_data.iter().zip(data.iter()) {
                for (nv, ov) in new.values.iter().zip(old.values.iter()) {
                    max_delta = finite_max_propagating(max_delta, (nv - ov).abs());
                    scale = scale.max(nv.abs());
                }
            }
            data = new_data;
            if self.converged(max_delta, scale) {
                return Ok(data);
            }
        }
        Err(format!(
            "fixed-point coupling did not converge in {} iterations \
             (atol={}, rtol={})",
            self.criteria.max_iterations,
            self.criteria.absolute_tolerance,
            self.criteria.relative_tolerance
        ))
    }

    /// Gauss-Seidel coupling: updates each field in place, in order.
    ///
    /// Unlike the other two methods this is **in-place by design** (each field
    /// immediately sees its neighbours' updated values, which is what makes it
    /// Gauss-Seidel rather than Jacobi), so a caller must treat `fields` as the
    /// working state, not as an input to be preserved.
    ///
    /// On success `fields` holds the converged result. On failure it holds the
    /// last iterate — which is why the error is returned rather than `Ok(())`:
    /// the caller must not mistake a partial sweep for a converged one.
    pub fn gauss_seidel_coupling(
        &self,
        fields: &mut [FieldData],
        compute_fn: &dyn Fn(&mut FieldData) -> Result<(), String>,
    ) -> Result<(), String> {
        for _iter in 0..self.criteria.max_iterations {
            let mut max_delta: Scalar = 0.0;
            let mut scale: Scalar = 0.0;
            for i in 0..fields.len() {
                let previous_values = fields[i].values.clone();
                compute_fn(&mut fields[i])?;
                for (new_value, old_value) in fields[i].values.iter().zip(previous_values.iter()) {
                    max_delta = finite_max_propagating(max_delta, (new_value - old_value).abs());
                    scale = scale.max(new_value.abs());
                }
            }
            if self.converged(max_delta, scale) {
                return Ok(());
            }
        }
        Err(format!(
            "Gauss-Seidel coupling did not converge in {} iterations \
             (atol={}, rtol={})",
            self.criteria.max_iterations,
            self.criteria.absolute_tolerance,
            self.criteria.relative_tolerance
        ))
    }

    /// Parallel Jacobi coupling: iterates sweeps until convergence.
    ///
    /// Each sweep computes all field updates from the previous state in
    /// parallel (true Jacobi semantics), then checks convergence against
    /// `self.criteria` (with the configured relaxation factor applied to the
    /// updates). Returns the converged field set.
    pub fn jacobi_coupling(
        &self,
        fields: &[FieldData],
        compute_fn: &(dyn Fn(&FieldData) -> Result<FieldData, String> + Send + Sync),
    ) -> Result<Vec<FieldData>, String> {
        use rayon::prelude::*;
        let mut current = fields.to_vec();
        let relaxation = self.criteria.relaxation_factor;
        for _iter in 0..self.criteria.max_iterations {
            let updated: Vec<FieldData> = current
                .par_iter()
                .map(compute_fn)
                .collect::<Result<_, _>>()?;
            let mut max_delta: Scalar = 0.0;
            let mut scale: Scalar = 0.0;
            for (u, c) in updated.iter().zip(current.iter()) {
                for (nu, nc) in u.values.iter().zip(c.values.iter()) {
                    max_delta = finite_max_propagating(max_delta, (nu - nc).abs());
                    scale = scale.max(nu.abs());
                }
            }
            // Apply relaxation: current = (1−ω)·current + ω·updated.
            for (u, c) in updated.iter().zip(current.iter_mut()) {
                for (nu, nc) in u.values.iter().zip(c.values.iter_mut()) {
                    *nc = (1.0 - relaxation) * *nc + relaxation * *nu;
                }
            }
            if self.converged(max_delta, scale) {
                return Ok(current);
            }
        }
        // Report the failure instead of returning unconverged data as success:
        // the caller cannot otherwise distinguish the two.
        Err(format!(
            "Jacobi coupling did not converge in {} iterations \
             (atol={}, rtol={})",
            self.criteria.max_iterations,
            self.criteria.absolute_tolerance,
            self.criteria.relative_tolerance
        ))
    }

    pub fn check_convergence(&self, delta: &[Scalar]) -> bool {
        delta.iter().all(|&d| d < self.criteria.absolute_tolerance)
    }

    /// Convergence test combining the absolute and relative criteria:
    /// `|Δ| <= atol + rtol · |scale|`.
    ///
    /// The relative term matters for large-magnitude fields. A pressure field in
    /// Pa (`~1e5`) can never satisfy a bare `atol = 1e-8`, so a purely absolute
    /// test stalls at `max_iterations` and (previously) returned the unconverged
    /// data as if it had succeeded. `scale` is the magnitude of the quantity
    /// being iterated, typically `max |x|` over the field.
    ///
    /// A non-finite `delta` or `scale` is never converged: a NaN iterate must not
    /// be reported as success.
    pub fn check_convergence_scaled(&self, delta: &[Scalar], scale: Scalar) -> bool {
        if !scale.is_finite() {
            return false;
        }
        let tol = self.criteria.absolute_tolerance
            + self.criteria.relative_tolerance * scale.abs().max(0.0);
        if !tol.is_finite() {
            return false;
        }
        delta.iter().all(|&d| d.is_finite() && d.abs() <= tol)
    }

    /// Full convergence test for an iterative coupling sweep.
    ///
    /// The step is accepted when it is below `atol + rtol · |scale|`.
    ///
    /// Deliberate limitation: from a single step this cannot distinguish "at the
    /// fixed point" from "drifting at a constant rate" — both look like one small
    /// increment. The callers therefore **report non-convergence when the budget
    /// is exhausted** instead of claiming success, so a persistently drifting
    /// iteration fails loudly rather than returning plausible-looking data.
    pub fn converged(&self, delta: Scalar, scale: Scalar) -> bool {
        self.check_convergence_scaled(&[delta], scale)
    }
}

/// Time synchronization manager for coupled fields.
pub struct TimeSyncManager {
    pub time_step: Scalar,
    pub sync_points: Vec<Scalar>,
    pub current_index: usize,
}

impl TimeSyncManager {
    /// Build a schedule of synchronization points from `0` to `total_time`.
    ///
    /// The stride is `time_step * sync_interval`. Degenerate inputs are clamped
    /// so the schedule is always well-formed: a non-positive or non-finite
    /// stride would otherwise never advance time and spin forever, so it falls
    /// back to a single point at `total_time` (or an empty schedule when
    /// `total_time` is negative/non-finite).
    pub fn new(time_step: Scalar, sync_interval: usize, total_time: Scalar) -> Self {
        let mut points = Vec::new();
        let stride = time_step * sync_interval as Scalar;
        if total_time.is_finite() && total_time >= 0.0 {
            if stride.is_finite() && stride > 0.0 {
                let mut t = 0.0;
                // `stride > 0` guarantees progress, so this always terminates.
                while t <= total_time {
                    points.push(t);
                    t += stride;
                }
            } else {
                // Unusable stride: represent the schedule as a single instant
                // rather than looping without progress.
                points.push(total_time);
            }
        }
        Self {
            time_step,
            sync_points: points,
            current_index: 0,
        }
    }

    pub fn current_time(&self) -> Scalar {
        self.sync_points
            .get(self.current_index)
            .copied()
            .unwrap_or(0.0)
    }

    pub fn need_sync(&self) -> bool {
        self.current_index < self.sync_points.len()
    }

    /// Advance to the next synchronization point.
    ///
    /// Returns `true` when the index moved; `false` when the schedule is
    /// exhausted. An empty schedule is handled explicitly so the length cannot
    /// underflow (which it would with `len() - 1`).
    pub fn advance(&mut self) -> bool {
        if self.current_index + 1 < self.sync_points.len() {
            self.current_index += 1;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::bus::{FieldData, PhysicsField, QuantityType};
    use super::*;
    use crate::core::coord::Coord3D;

    fn make_dummy_field(val: Scalar) -> FieldData {
        FieldData::new(
            PhysicsField::Thermal,
            QuantityType::Scalar,
            vec![Coord3D::new(0.0, 0.0, 0.0)],
            vec![val],
            0.0,
        )
    }

    #[test]
    fn test_convergence_criteria_default() {
        let cc: ConvergenceCriteria = Default::default();
        assert!((cc.absolute_tolerance - 1e-8).abs() < 1e-12);
        assert_eq!(cc.max_iterations, 50);
    }
    #[test]
    fn test_check_convergence() {
        let cc = ConvergenceCriteria::default();
        let s = CouplingScheduler::new(cc);
        assert!(s.check_convergence(&[1e-10]));
        assert!(!s.check_convergence(&[1.0]));
    }
    #[test]
    fn test_fixed_point_iteration() {
        let cc = ConvergenceCriteria {
            absolute_tolerance: 1.0,
            ..Default::default()
        };
        let s = CouplingScheduler::new(cc);
        let data = vec![make_dummy_field(10.0)];
        let result = s
            .fixed_point_iteration(&data, &|d, _| Ok(d[0].clone()))
            .unwrap();
        assert_eq!(result.len(), 1);
    }
    #[test]
    fn test_time_sync_manager() {
        let mut tsm = TimeSyncManager::new(0.01, 10, 1.0);
        assert!((tsm.current_time()).abs() < 1e-10);
        assert!(tsm.advance());
        assert!((tsm.current_time() - 0.1).abs() < 1e-10);
    }
    #[test]
    fn test_need_sync() {
        let tsm = TimeSyncManager::new(0.01, 10, 0.05);
        assert!(tsm.need_sync());
    }

    #[test]
    fn test_time_sync_zero_stride_terminates() {
        // A zero time step (or zero sync interval) makes the stride zero. The
        // schedule must still be produced without spinning forever.
        let tsm = TimeSyncManager::new(0.0, 10, 1.0);
        assert_eq!(
            tsm.sync_points,
            vec![1.0],
            "degenerate stride yields one point"
        );
        assert!(tsm.need_sync());

        let tsm = TimeSyncManager::new(0.01, 0, 1.0);
        assert_eq!(tsm.sync_points, vec![1.0]);
    }

    #[test]
    fn test_time_sync_negative_total_is_empty() {
        let tsm = TimeSyncManager::new(0.01, 10, -1.0);
        assert!(tsm.sync_points.is_empty());
        assert!(!tsm.need_sync());
        assert_eq!(tsm.current_time(), 0.0);
    }

    #[test]
    fn test_time_sync_advance_on_empty_schedule() {
        // An empty schedule must not underflow `len() - 1`.
        let mut tsm = TimeSyncManager::new(0.01, 10, -1.0);
        assert!(!tsm.advance());
        assert_eq!(tsm.current_index, 0);
    }

    #[test]
    fn test_time_sync_advance_walks_every_point() {
        // 0.1 stride over 0.5 → points 0.0, 0.1, 0.2, 0.3, 0.4, 0.5.
        let mut tsm = TimeSyncManager::new(0.1, 1, 0.5);
        assert_eq!(tsm.sync_points.len(), 6);
        for expected in 1..6 {
            assert!(tsm.advance(), "advance to index {expected}");
            assert!((tsm.current_time() - expected as Scalar * 0.1).abs() < 1e-10);
        }
        // At the last point there is nowhere left to go.
        assert!(!tsm.advance());
        assert!(tsm.need_sync(), "the final point still needs syncing");
    }
    #[test]
    fn test_jacobi_coupling() {
        let cc = ConvergenceCriteria::default();
        let s = CouplingScheduler::new(cc);
        let fields = vec![make_dummy_field(5.0)];
        let r = s.jacobi_coupling(&fields, &|f| Ok(f.clone())).unwrap();
        assert_eq!(r.len(), 1);
    }

    /// The relative criterion must actually participate. A large-magnitude field
    /// can never satisfy a bare `atol = 1e-8`, so a purely absolute test stalls
    /// and (before the fix) returned unconverged data as success.
    #[test]
    fn test_relative_tolerance_is_honoured_for_large_magnitude_fields() {
        let sched = CouplingScheduler::new(ConvergenceCriteria {
            absolute_tolerance: 1e-8,
            relative_tolerance: 1e-6,
            max_iterations: 50,
            relaxation_factor: 0.5,
        });

        // A pressure-scale field: `scale = 1e5 Pa`.
        let scale = 1e5;
        // A residual of 1e-3 is far above atol but well below rtol*scale = 1e-1,
        // so the *combined* criterion must accept it.
        assert!(
            sched.check_convergence_scaled(&[1e-3], scale),
            "a relative residual of 1e-8 against a 1e5 field must converge"
        );
        // The old, absolute-only test would have rejected it.
        assert!(
            !sched.check_convergence(&[1e-3]),
            "the bare absolute test cannot accept this residual, which is the bug"
        );
        // And a residual that fails even the relative bound must be rejected.
        assert!(
            !sched.check_convergence_scaled(&[1.0], scale),
            "a residual of 1 Pa against a 1e5 Pa field is 1e-5 relative: too coarse"
        );
    }

    /// A non-finite residual must never be treated as converged.
    #[test]
    fn test_non_finite_residual_is_never_converged() {
        let sched = CouplingScheduler::new(ConvergenceCriteria::default());
        for bad in [Scalar::NAN, Scalar::INFINITY, Scalar::NEG_INFINITY] {
            assert!(
                !sched.check_convergence_scaled(&[bad], 1.0),
                "residual {bad} must not be reported as converged"
            );
        }
    }

    /// A divergent coupling must be reported as an error, not returned as a
    /// successful result. This is the core fix: previously the caller could not
    /// distinguish convergence from hitting `max_iterations`.
    #[test]
    fn test_divergent_fixed_point_iteration_is_reported() {
        let sched = CouplingScheduler::new(ConvergenceCriteria {
            absolute_tolerance: 1e-12,
            relative_tolerance: 1e-12,
            max_iterations: 10,
            relaxation_factor: 1.0,
        });
        let points = vec![crate::core::coord::Coord3D::new(0.0, 0.0, 0.0)];
        let data = vec![FieldData::new(
            PhysicsField::Thermal,
            QuantityType::Scalar,
            points,
            vec![1.0],
            0.0,
        )];

        // `x ← 2x` diverges monotonically.
        let outcome = sched.fixed_point_iteration(&data, &|fields, field_type| {
            let doubled: Vec<Scalar> = fields[0].values.iter().map(|v| 2.0 * v).collect();
            Ok(FieldData::new(
                field_type,
                QuantityType::Scalar,
                fields[0].points.clone(),
                doubled,
                0.0,
            ))
        });
        let err = outcome.expect_err("a divergent iteration must be reported");
        assert!(
            err.contains("did not converge"),
            "the error must name non-convergence, got: {err}"
        );
    }

    /// A convergent coupling must still succeed, so the stricter reporting did
    /// not simply reject everything.
    #[test]
    fn test_convergent_fixed_point_iteration_still_succeeds() {
        let sched = CouplingScheduler::new(ConvergenceCriteria {
            absolute_tolerance: 1e-10,
            relative_tolerance: 1e-10,
            max_iterations: 200,
            relaxation_factor: 0.5,
        });
        let points = vec![crate::core::coord::Coord3D::new(0.0, 0.0, 0.0)];
        let data = vec![FieldData::new(
            PhysicsField::Thermal,
            QuantityType::Scalar,
            points,
            vec![100.0],
            0.0,
        )];

        // `x ← 0.5x` contracts to zero.
        let result = sched
            .fixed_point_iteration(&data, &|fields, field_type| {
                let halved: Vec<Scalar> = fields[0].values.iter().map(|v| 0.5 * v).collect();
                Ok(FieldData::new(
                    field_type,
                    QuantityType::Scalar,
                    fields[0].points.clone(),
                    halved,
                    0.0,
                ))
            })
            .expect("a contraction must converge");
        assert_eq!(result.len(), 1);
        assert!(
            result[0].values[0].abs() < 1e-6,
            "the iteration must have reached the fixed point, got {}",
            result[0].values[0]
        );
    }

    /// The Gauss-Seidel variant must use the same combined criterion and report
    /// divergence the same way.
    #[test]
    fn test_gauss_seidel_reports_divergence() {
        let sched = CouplingScheduler::new(ConvergenceCriteria {
            absolute_tolerance: 1e-12,
            relative_tolerance: 1e-12,
            max_iterations: 10,
            relaxation_factor: 1.0,
        });
        let points = vec![crate::core::coord::Coord3D::new(0.0, 0.0, 0.0)];
        let mut fields = vec![FieldData::new(
            PhysicsField::Thermal,
            QuantityType::Scalar,
            points,
            vec![1.0],
            0.0,
        )];

        let outcome = sched.gauss_seidel_coupling(&mut fields, &|field| {
            for v in &mut field.values {
                *v *= 2.0;
            }
            Ok(())
        });
        assert!(
            outcome.is_err(),
            "a divergent Gauss-Seidel sweep must be reported, not returned as Ok"
        );
    }

    /// A NaN residual must never be reported as converged.
    ///
    /// `f64::max` returns the non-NaN operand, so a naive
    /// `max_delta.max(delta.abs())` accumulator turns a NaN step into `0.0`,
    /// which then passes the convergence test and reports success on a broken
    /// iterate. This is the regression guard for that.
    #[test]
    fn test_nan_residual_is_not_reported_as_converged() {
        let sched = CouplingScheduler::new(ConvergenceCriteria {
            absolute_tolerance: 1e-8,
            relative_tolerance: 1e-6,
            max_iterations: 5,
            relaxation_factor: 1.0,
        });
        let points = vec![crate::core::coord::Coord3D::new(0.0, 0.0, 0.0)];
        let data = vec![FieldData::new(
            PhysicsField::Thermal,
            QuantityType::Scalar,
            points,
            vec![1.0],
            0.0,
        )];

        // A map that produces NaN on the first sweep must be reported as a
        // failure, not silently "converged" with a NaN field.
        let outcome = sched.fixed_point_iteration(&data, &|_, field_type| {
            Ok(FieldData::new(
                field_type,
                QuantityType::Scalar,
                data[0].points.clone(),
                vec![Scalar::NAN],
                0.0,
            ))
        });
        assert!(
            outcome.is_err(),
            "a NaN iterate must not be reported as converged, got Ok({:?})",
            outcome.map(|f| f[0].values.clone())
        );

        // Direct check of the criterion.
        assert!(
            !sched.check_convergence_scaled(&[Scalar::NAN], 1.0),
            "a NaN residual must be rejected"
        );
        assert!(
            !sched.check_convergence_scaled(&[1e-12], Scalar::NAN),
            "a NaN scale must be rejected"
        );
        assert!(
            !sched.check_convergence_scaled(&[Scalar::INFINITY], 1.0),
            "an infinite residual must be rejected"
        );
    }

    /// Overflow to infinity must also be reported, not swallowed.
    #[test]
    fn test_overflowing_iteration_is_reported_not_swallowed() {
        let sched = CouplingScheduler::new(ConvergenceCriteria {
            absolute_tolerance: 1e-8,
            relative_tolerance: 1e-6,
            max_iterations: 5,
            relaxation_factor: 1.0,
        });
        let points = vec![crate::core::coord::Coord3D::new(0.0, 0.0, 0.0)];
        let data = vec![FieldData::new(
            PhysicsField::Thermal,
            QuantityType::Scalar,
            points,
            vec![1.0],
            0.0,
        )];
        let outcome = sched.fixed_point_iteration(&data, &|fields, field_type| {
            // Repeatedly double: overflows to infinity within a few sweeps.
            let doubled: Vec<Scalar> = fields[0].values.iter().map(|v| v * 1e300).collect();
            Ok(FieldData::new(
                field_type,
                QuantityType::Scalar,
                fields[0].points.clone(),
                doubled,
                0.0,
            ))
        });
        assert!(
            outcome.is_err(),
            "an overflowing iteration must be reported as non-convergence"
        );
    }
}
