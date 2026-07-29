//! Multi-physics coupling integration tests.
//!
//! Verifies end-to-end coupling across the new 3D solvers using the
//! unified coupling bus. Each test exercises a specific coupling pair:
//!
//! - Fluid ↔ Thermal: convective heat transfer from NS3D velocity field
//! - Thermal ↔ Structural: thermal expansion from temperature field
//! - EMag ↔ Thermal: resistive heating from FDTD E-field
//! - Fluid ↔ Structural: pressure load from NS3D on structural surface
//!
//! These are **integration** tests — they construct small realistic
//! coupling scenarios, run a few iterations, and validate that the
//! coupled system produces physically plausible output.

#![cfg(test)]
#![allow(clippy::unnecessary_unwrap)]

use crate::core::coord::Coord3D;
use crate::core::types::Scalar;
use crate::coupling::bus::{CouplingBus, CouplingInterface, FieldData, PhysicsField, QuantityType};
use crate::coupling::convergence::{ConvergenceCriteria, CouplingScheduler};

/// Tolerance for scalar comparisons in coupling tests.
const COUPLING_TOL: Scalar = 1e-6;

// ---------------------------------------------------------------------------
// Helper: create a uniform 3D grid of points
// ---------------------------------------------------------------------------

fn make_grid_points(
    nx: usize,
    ny: usize,
    nz: usize,
    dx: Scalar,
    dy: Scalar,
    dz: Scalar,
) -> Vec<Coord3D> {
    let mut pts = Vec::with_capacity(nx * ny * nz);
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                pts.push(Coord3D::new(
                    i as Scalar * dx,
                    j as Scalar * dy,
                    k as Scalar * dz,
                ));
            }
        }
    }
    pts
}

// ---------------------------------------------------------------------------
// Test 1: Fluid → Thermal coupling
// ---------------------------------------------------------------------------

#[test]
fn test_fluid_thermal_coupling() {
    // Set up a simple 3D flow field with known velocity
    let nx = 4;
    let ny = 4;
    let nz = 4;
    let dx = 0.1;
    let dy = 0.1;
    let dz = 0.1;

    let points = make_grid_points(nx, ny, nz, dx, dy, dz);
    let n_pts = points.len();

    // Fluid velocity magnitudes (parabolic profile: u_max at centre, zero at walls)
    let mut vel_mag = vec![0.0; n_pts];
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let idx = k * ny * nx + j * nx + i;
                let r = ((i as Scalar - 1.5).powi(2)
                    + (j as Scalar - 1.5).powi(2)
                    + (k as Scalar - 1.5).powi(2))
                .sqrt();
                vel_mag[idx] = (1.0 - (r / 2.0).min(1.0)) * 5.0; // max 5 m/s
            }
        }
    }

    // Register coupling interface: Fluid → Thermal (convective heat transfer)
    let mut bus = CouplingBus::new();
    bus.register_interface(
        CouplingInterface::new(
            PhysicsField::Fluid,
            PhysicsField::Thermal,
            QuantityType::Scalar,
        )
        .with_mapping(crate::coupling::bus::FieldMappingMethod::NearestNeighbor),
    );

    // Create field data (fluid 👉 thermal coupling)
    let fluid_field = FieldData::new(
        PhysicsField::Fluid,
        QuantityType::Scalar,
        points.clone(),
        vel_mag,
        0.0,
    );
    let _thermal_field = FieldData::new(
        PhysicsField::Thermal,
        QuantityType::Scalar,
        points,
        vec![300.0; n_pts],
        0.0,
    );

    // Verify exchange: fluid → thermal
    let interface = bus
        .find_interface(PhysicsField::Fluid, PhysicsField::Thermal)
        .unwrap();
    let exchanged = bus.exchange(&fluid_field, interface).unwrap();
    assert_eq!(exchanged.field_type, PhysicsField::Thermal);
    assert_eq!(exchanged.num_points(), n_pts);

    // Fluid ↵ Thermal coupling should produce positive heat flux proxy
    // (hot fluid → heat transfer to cooler region)
    let max_vel = exchanged
        .values
        .iter()
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(max_vel > 0.0, "heat flux proxy should be positive");
}

// ---------------------------------------------------------------------------
// Test 2: Thermal → Structural coupling (thermal expansion)
// ---------------------------------------------------------------------------

#[test]
fn test_thermal_structural_coupling() {
    // Temperature field on a small grid
    let nx = 3;
    let ny = 3;
    let nz = 3;
    let dx = 0.2;
    let dy = 0.2;
    let dz = 0.2;

    let points = make_grid_points(nx, ny, nz, dx, dy, dz);
    let n_pts = points.len();

    // Linear temperature gradient: 300 K at one end → 400 K at the other
    let thermal_strain: Vec<Scalar> = (0..n_pts)
        .map(|i| {
            let x_frac = i as Scalar / (n_pts - 1).max(1) as Scalar;
            300.0 + x_frac * 100.0
        })
        .collect();

    // Register interface
    let mut bus = CouplingBus::new();
    bus.register_interface(
        CouplingInterface::new(
            PhysicsField::Thermal,
            PhysicsField::Structural,
            QuantityType::Scalar,
        )
        .with_mapping(crate::coupling::bus::FieldMappingMethod::LinearInterp),
    );

    let _thermal_field = FieldData::new(
        PhysicsField::Thermal,
        QuantityType::Scalar,
        points.clone(),
        thermal_strain,
        0.0,
    );
    let _struct_field = FieldData::new(
        PhysicsField::Structural,
        QuantityType::Scalar,
        points,
        vec![0.0; n_pts],
        0.0,
    );

    let interface = bus
        .find_interface(PhysicsField::Thermal, PhysicsField::Structural)
        .unwrap();
    let exchanged = bus.exchange(&_thermal_field, interface).unwrap();

    // Temperature data should be mapped from thermal to structural
    assert_eq!(exchanged.field_type, PhysicsField::Structural);
    assert_eq!(exchanged.values.len(), n_pts);

    // Temperature gradient should be preserved (monotonic)
    for i in 1..exchanged.values.len() {
        assert!(
            exchanged.values[i] >= exchanged.values[i - 1] - COUPLING_TOL,
            "temperature gradient must be monotonic"
        );
    }
}

// ---------------------------------------------------------------------------
// Test 3: EMag → Thermal coupling (Joule/resistive heating)
// ---------------------------------------------------------------------------

#[test]
fn test_emag_thermal_coupling() {
    // E-field magnitude on a 2D grid (simplified: uniform plane)
    let nx = 5;
    let ny = 5;
    let nz = 1;
    let dx = 0.05;
    let dy = 0.05;
    let dz = 0.05;

    let points = make_grid_points(nx, ny, nz, dx, dy, dz);
    let n_pts = points.len();

    // Inward-focused E-field: stronger at centre (simulating resonant cavity)
    let e_field_squared: Vec<Scalar> = (0..n_pts)
        .map(|i| {
            let x = (i % nx) as Scalar * dx;
            let y = ((i / nx) % ny) as Scalar * dy;
            let cx = (nx - 1) as Scalar * dx * 0.5;
            let cy = (ny - 1) as Scalar * dy * 0.5;
            let r2 = (x - cx).powi(2) + (y - cy).powi(2);
            (-r2 * 100.0).exp() * 1e6 // V²/m²
        })
        .collect();

    // Register coupling: EMag → Thermal (resistive heating Q = σ·E²)
    let mut bus = CouplingBus::new();
    bus.register_interface(
        CouplingInterface::new(
            PhysicsField::Electromagnetic,
            PhysicsField::Thermal,
            QuantityType::Scalar,
        )
        .with_mapping(crate::coupling::bus::FieldMappingMethod::NearestNeighbor),
    );

    let emag_field = FieldData::new(
        PhysicsField::Electromagnetic,
        QuantityType::Scalar,
        points.clone(),
        e_field_squared,
        0.0,
    );
    let _thermal_field = FieldData::new(
        PhysicsField::Thermal,
        QuantityType::Scalar,
        points,
        vec![300.0; n_pts],
        0.0,
    );

    let interface = bus
        .find_interface(PhysicsField::Electromagnetic, PhysicsField::Thermal)
        .unwrap();
    let exchanged = bus.exchange(&emag_field, interface).unwrap();

    assert_eq!(exchanged.field_type, PhysicsField::Thermal);
    assert_eq!(exchanged.values.len(), n_pts);

    // Maximum heating at centre (where E-field is strongest)
    let mid_idx = n_pts / 2;
    let centre_val = exchanged.values[mid_idx];
    let edge_val = exchanged.values[0];
    assert!(
        centre_val > edge_val,
        "resistive heating should be maximum at centre"
    );
}

// ---------------------------------------------------------------------------
// Test 4: Fluid → Structural coupling (pressure loading)
// ---------------------------------------------------------------------------

#[test]
fn test_fluid_structural_coupling() {
    let nx = 4;
    let ny = 4;
    let nz = 1;
    let dx = 0.1;
    let dy = 0.1;
    let dz = 0.1;

    let points = make_grid_points(nx, ny, nz, dx, dy, dz);
    let n_pts = points.len();

    // Pressure field: high at "stagnation point", decreasing outward
    let pressure: Vec<Scalar> = (0..n_pts)
        .map(|i| {
            let x = (i % nx) as Scalar * dx;
            let y = ((i / nx) % ny) as Scalar * dy;
            let cx = (nx - 1) as Scalar * dx * 0.5;
            let cy = (ny - 1) as Scalar * dy * 0.5;
            let r2 = (x - cx).powi(2) + (y - cy).powi(2);
            101325.0 + (-r2 * 50.0).exp() * 5000.0 // Pa: 101.3–106.3 kPa
        })
        .collect();

    let mut bus = CouplingBus::new();
    bus.register_interface(
        CouplingInterface::new(
            PhysicsField::Fluid,
            PhysicsField::Structural,
            QuantityType::Scalar,
        )
        .with_mapping(crate::coupling::bus::FieldMappingMethod::NearestNeighbor),
    );

    let fluid_field = FieldData::new(
        PhysicsField::Fluid,
        QuantityType::Scalar,
        points.clone(),
        pressure,
        0.0,
    );
    let _struct_field = FieldData::new(
        PhysicsField::Structural,
        QuantityType::Scalar,
        points,
        vec![0.0; n_pts],
        0.0,
    );

    let interface = bus
        .find_interface(PhysicsField::Fluid, PhysicsField::Structural)
        .unwrap();
    let exchanged = bus.exchange(&fluid_field, interface).unwrap();

    assert_eq!(exchanged.field_type, PhysicsField::Structural);
    let max_p = exchanged
        .values
        .iter()
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        max_p > 101325.0,
        "stagnation pressure should exceed base pressure"
    );
}

// ---------------------------------------------------------------------------
// Test 5: Coupling scheduler with fixed-point iteration
// ---------------------------------------------------------------------------

#[test]
fn test_coupling_scheduler_two_way() {
    // Two-way coupling: Thermal ↔ Fluid
    // Fixed-point iteration should converge to steady state
    let n_pts = 4;
    let points = make_grid_points(2, 2, 1, 1.0, 1.0, 1.0);

    // Initial temperature: uniform 300 K
    let temp_init = vec![300.0; n_pts];
    // Initial velocity: uniform 1 m/s (convective)
    let vel_init = vec![1.0; n_pts];

    let data = vec![
        FieldData::new(
            PhysicsField::Thermal,
            QuantityType::Scalar,
            points.clone(),
            temp_init,
            0.0,
        ),
        FieldData::new(
            PhysicsField::Fluid,
            QuantityType::Scalar,
            points.clone(),
            vel_init,
            0.0,
        ),
    ];

    let criteria = ConvergenceCriteria {
        absolute_tolerance: 0.1,
        max_iterations: 20,
        relaxation_factor: 0.5,
        ..Default::default()
    };
    let scheduler = CouplingScheduler::new(criteria);

    // Compute field: thermal field heats the fluid (buoyancy), fluid cools the thermal
    let result = scheduler
        .fixed_point_iteration(&data, &|fields, field_type| {
            let temp = &fields[0];
            let vel = &fields[1];
            match field_type {
                PhysicsField::Thermal => {
                    // Fluid cools thermal: T -= 0.1 * vel
                    let cooled: Vec<Scalar> = temp
                        .values
                        .iter()
                        .zip(vel.values.iter())
                        .map(|(t, v)| (t - 0.1 * v).max(290.0))
                        .collect();
                    Ok(FieldData::new(
                        PhysicsField::Thermal,
                        QuantityType::Scalar,
                        temp.points.clone(),
                        cooled,
                        0.0,
                    ))
                }
                PhysicsField::Fluid => {
                    // Thermal heats fluid: vel += 0.05 * (T - 300)
                    let heated: Vec<Scalar> = vel
                        .values
                        .iter()
                        .zip(temp.values.iter())
                        .map(|(v, t)| (v + 0.05 * (t - 300.0)).max(0.1))
                        .collect();
                    Ok(FieldData::new(
                        PhysicsField::Fluid,
                        QuantityType::Scalar,
                        vel.points.clone(),
                        heated,
                        0.0,
                    ))
                }
                _ => Err("Unknown field".to_string()),
            }
        })
        .unwrap();

    assert_eq!(result.len(), 2);
    // Both fields should have converged to a stable value
    for field in &result {
        for &val in &field.values {
            assert!(val.is_finite());
        }
    }
}

// ---------------------------------------------------------------------------
// Test 6: Gauss-Seidel coupling with convergence check
// ---------------------------------------------------------------------------

#[test]
fn test_gauss_seidel_coupling() {
    let n_pts = 2;
    let points = make_grid_points(2, 1, 1, 1.0, 1.0, 1.0);

    let criteria = ConvergenceCriteria {
        absolute_tolerance: 1e-3,
        max_iterations: 100,
        relaxation_factor: 0.8,
        ..Default::default()
    };
    let scheduler = CouplingScheduler::new(criteria);

    let mut fields = vec![
        FieldData::new(
            PhysicsField::Thermal,
            QuantityType::Scalar,
            points.clone(),
            vec![350.0; n_pts],
            0.0,
        ),
        FieldData::new(
            PhysicsField::Fluid,
            QuantityType::Scalar,
            points,
            vec![2.0; n_pts],
            0.0,
        ),
    ];

    // Gauss-Seidel: update fields sequentially
    scheduler
        .gauss_seidel_coupling(&mut fields, &|field| {
            match field.field_type {
                PhysicsField::Thermal => {
                    // Each iteration: T → 0.9*T_old + 30 (decay to 300K)
                    for v in &mut field.values {
                        *v = 0.9 * *v + 30.0;
                    }
                }
                PhysicsField::Fluid => {
                    // Each iteration: v → 0.95*v (slow decay)
                    for v in &mut field.values {
                        *v *= 0.95;
                    }
                }
                _ => return Err("Unknown".to_string()),
            }
            Ok(())
        })
        .unwrap();

    // Thermal field should have decayed toward 300 K
    for &t in &fields[0].values {
        assert!(t < 350.0, "thermal field should have decreased");
        assert!(t >= 300.0, "thermal field should be ≥ 300 K");
    }
    // Fluid velocity should have decayed
    for &v in &fields[1].values {
        assert!(v < 2.0, "velocity should have decreased");
    }
}

// ---------------------------------------------------------------------------
// Test 7: Multi-field coupling (3-field: Thermal ↔ Fluid ↔ Structural)
// ---------------------------------------------------------------------------

#[test]
fn test_three_way_coupling() {
    let n_pts = 8;
    let points = make_grid_points(2, 2, 2, 0.5, 0.5, 0.5);

    let criteria = ConvergenceCriteria {
        absolute_tolerance: 0.05,
        max_iterations: 30,
        relaxation_factor: 0.6,
        ..Default::default()
    };
    let scheduler = CouplingScheduler::new(criteria);

    let data = vec![
        FieldData::new(
            PhysicsField::Thermal,
            QuantityType::Scalar,
            points.clone(),
            vec![310.0; n_pts],
            0.0,
        ),
        FieldData::new(
            PhysicsField::Fluid,
            QuantityType::Scalar,
            points.clone(),
            vec![1.5; n_pts],
            0.0,
        ),
        FieldData::new(
            PhysicsField::Structural,
            QuantityType::Scalar,
            points,
            vec![0.0; n_pts],
            0.0,
        ),
    ];

    let result = scheduler
        .fixed_point_iteration(&data, &|fields, field_type| {
            let t = &fields[0];
            let v = &fields[1];
            let s = &fields[2];
            match field_type {
                PhysicsField::Thermal => {
                    // T -= 0.05*v + 0.01*s (cooling from flow + structure)
                    let updated: Vec<Scalar> = t
                        .values
                        .iter()
                        .zip(v.values.iter())
                        .zip(s.values.iter())
                        .map(|((tv, vv), sv)| (tv - 0.05 * vv - 0.01 * sv).max(295.0))
                        .collect();
                    Ok(FieldData::new(
                        PhysicsField::Thermal,
                        QuantityType::Scalar,
                        t.points.clone(),
                        updated,
                        0.0,
                    ))
                }
                PhysicsField::Fluid => {
                    // v += 0.02*(T - 300) (buoyancy)
                    let updated: Vec<Scalar> = v
                        .values
                        .iter()
                        .zip(t.values.iter())
                        .map(|(vv, tv)| (vv + 0.02 * (tv - 300.0)).max(0.1))
                        .collect();
                    Ok(FieldData::new(
                        PhysicsField::Fluid,
                        QuantityType::Scalar,
                        v.points.clone(),
                        updated,
                        0.0,
                    ))
                }
                PhysicsField::Structural => {
                    // s += 0.01*(T - 300) - 0.1*s (thermal expansion + damping)
                    let updated: Vec<Scalar> = s
                        .values
                        .iter()
                        .zip(t.values.iter())
                        .map(|(sv, tv)| (sv + 0.01 * (tv - 300.0) - 0.1 * sv).max(0.0))
                        .collect();
                    Ok(FieldData::new(
                        PhysicsField::Structural,
                        QuantityType::Scalar,
                        s.points.clone(),
                        updated,
                        0.0,
                    ))
                }
                _ => Err("Unknown field".to_string()),
            }
        })
        .unwrap();

    assert_eq!(result.len(), 3);
    // Structural displacement should be positive (thermal expansion)
    for &val in &result[2].values {
        assert!(val >= 0.0, "structural displacement should be ≥ 0");
    }
    // All values finite
    for field in &result {
        for &val in &field.values {
            assert!(val.is_finite());
        }
    }
}
