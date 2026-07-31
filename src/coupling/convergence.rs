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
            // Check convergence
            let mut max_delta: Scalar = 0.0;
            for (new, old) in new_data.iter().zip(data.iter()) {
                for (nv, ov) in new.values.iter().zip(old.values.iter()) {
                    max_delta = max_delta.max((nv - ov).abs());
                }
            }
            data = new_data;
            if self.check_convergence(&[max_delta]) {
                return Ok(data);
            }
        }
        Ok(data)
    }

    pub fn gauss_seidel_coupling(
        &self,
        fields: &mut [FieldData],
        compute_fn: &dyn Fn(&mut FieldData) -> Result<(), String>,
    ) -> Result<(), String> {
        for _iter in 0..self.criteria.max_iterations {
            let mut max_delta: Scalar = 0.0;
            for i in 0..fields.len() {
                let previous_values = fields[i].values.clone();
                compute_fn(&mut fields[i])?;
                for (new_value, old_value) in fields[i].values.iter().zip(previous_values.iter()) {
                    max_delta = max_delta.max((new_value - old_value).abs());
                }
            }
            if self.check_convergence(&[max_delta]) {
                break;
            }
        }
        Ok(())
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
            for (u, c) in updated.iter().zip(current.iter()) {
                for (nu, nc) in u.values.iter().zip(c.values.iter()) {
                    max_delta = max_delta.max((nu - nc).abs());
                }
            }
            // Apply relaxation: current = (1−ω)·current + ω·updated.
            for (u, c) in updated.iter().zip(current.iter_mut()) {
                for (nu, nc) in u.values.iter().zip(c.values.iter_mut()) {
                    *nc = (1.0 - relaxation) * *nc + relaxation * *nu;
                }
            }
            if self.check_convergence(&[max_delta]) {
                break;
            }
        }
        Ok(current)
    }

    pub fn check_convergence(&self, delta: &[Scalar]) -> bool {
        delta.iter().all(|&d| d < self.criteria.absolute_tolerance)
    }
}

/// Time synchronization manager for coupled fields.
pub struct TimeSyncManager {
    pub time_step: Scalar,
    pub sync_points: Vec<Scalar>,
    pub current_index: usize,
}

impl TimeSyncManager {
    pub fn new(time_step: Scalar, sync_interval: usize, total_time: Scalar) -> Self {
        let mut points = Vec::new();
        let mut t = 0.0;
        while t <= total_time {
            points.push(t);
            t += time_step * sync_interval as Scalar;
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

    pub fn advance(&mut self) -> bool {
        if self.current_index < self.sync_points.len() - 1 {
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
    fn test_jacobi_coupling() {
        let cc = ConvergenceCriteria::default();
        let s = CouplingScheduler::new(cc);
        let fields = vec![make_dummy_field(5.0)];
        let r = s.jacobi_coupling(&fields, &|f| Ok(f.clone())).unwrap();
        assert_eq!(r.len(), 1);
    }
}
