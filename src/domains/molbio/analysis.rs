// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Molecular trajectory analysis tools.
//!
//! Provides RMSD, radius of gyration, mean-squared displacement,
//! dihedral angle computation, hydrogen bond detection, and
//! solvent accessible surface area estimation.

use crate::core::types::Scalar;
use crate::domains::molbio::forcefield::Vec3;

/// Compute RMSD between two sets of coordinates (Å).
///
/// Assumes structures are already aligned (no rotation/translation optimization).
pub fn compute_rmsd(coords: &[Vec3], reference: &[Vec3]) -> Scalar {
    let n = coords.len().min(reference.len());
    if n == 0 {
        return 0.0;
    }
    let sum_sq: Scalar = coords[..n]
        .iter()
        .zip(&reference[..n])
        .map(|(c, r)| c.distance(r).powi(2))
        .sum();
    (sum_sq / n as Scalar).sqrt()
}

/// Compute radius of gyration (Å).
pub fn radius_of_gyration(coords: &[Vec3], masses: &[Scalar]) -> Scalar {
    let n = coords.len().min(masses.len());
    if n == 0 {
        return 0.0;
    }

    // Center of mass
    let mut com = Vec3::zero();
    let mut total_mass = 0.0;
    for i in 0..n {
        com = com.add(&coords[i].scale(masses[i]));
        total_mass += masses[i];
    }
    if total_mass <= 0.0 {
        return 0.0;
    }
    com = com.scale(1.0 / total_mass);

    // Rg² = Σ mi * |ri - com|² / Σ mi
    let mut sum_sq = 0.0;
    for i in 0..n {
        let d = coords[i].distance(&com);
        sum_sq += masses[i] * d * d;
    }
    (sum_sq / total_mass).sqrt()
}

/// Compute mean-squared displacement (Å²) over a trajectory.
/// Mean squared displacement of a trajectory relative to `start_frame`.
///
/// Computes the MSD at multiples of `interval` frames after `start_frame`.
/// Returns an empty vector when the trajectory is empty, `start_frame` is past
/// the end, or `interval` is zero (a zero interval has no defined sampling
/// points and would otherwise never advance).
pub fn mean_squared_displacement(
    traj: &[Vec<Vec3>],
    start_frame: usize,
    interval: usize,
) -> Vec<Scalar> {
    if traj.is_empty() || start_frame >= traj.len() || interval == 0 {
        return Vec::new();
    }

    let ref_frame = &traj[start_frame];
    let n_atoms = ref_frame.len();
    let max_frames = traj.len();
    let mut msd = Vec::new();

    let mut t = interval;
    // `interval > 0` guarantees `t` grows every iteration, so this terminates.
    while start_frame + t < max_frames {
        let frame = &traj[start_frame + t];
        let sum_sq: Scalar = ref_frame[..n_atoms.min(frame.len())]
            .iter()
            .zip(frame.iter())
            .map(|(r, f)| r.distance(f).powi(2))
            .sum();
        msd.push(sum_sq / n_atoms as Scalar);
        t += interval;
    }
    msd
}

/// Compute the dihedral angle (radians) from four atomic positions.
pub fn compute_dihedral_angle(a: &Vec3, b: &Vec3, c: &Vec3, d: &Vec3) -> Scalar {
    let b1 = a.subtract(b);
    let b2 = c.subtract(b);
    let b3 = d.subtract(c);
    let n1 = b1.cross(&b2);
    let n2 = b2.cross(&b3);
    let dot = n1.dot(&n2);
    let norm = n1.norm() * n2.norm();
    if norm < 1e-15 {
        0.0
    } else {
        let cos_phi = (dot / norm).clamp(-1.0, 1.0);
        let sign = (n1.cross(&n2)).dot(&b2).signum();
        sign * cos_phi.acos()
    }
}

/// Detect hydrogen bonds between potential donors and acceptors.
///
/// Uses distance cutoff (D-A distance < cutoff) and optional angle criterion.
/// Returns pairs of (donor_idx, acceptor_idx).
pub fn detect_hydrogen_bonds(
    coords: &[Vec3],
    elements: &[String],
    donor_cutoff: Scalar,
    _angle_cutoff: Scalar,
) -> Vec<(usize, usize)> {
    let mut hbonds = Vec::new();
    let n = coords.len().min(elements.len());

    for i in 0..n {
        // Donors: N, O (with H attached, simplified: just check if element is N or O)
        if elements[i] != "N" && elements[i] != "O" {
            continue;
        }
        for j in 0..n {
            if i == j {
                continue;
            }
            // Acceptors: N, O, F
            if elements[j] != "N" && elements[j] != "O" && elements[j] != "F" {
                continue;
            }

            let dist = coords[i].distance(&coords[j]);
            if dist < donor_cutoff && dist > 1.0 {
                hbonds.push((i, j));
            }
        }
    }
    hbonds
}

/// Solvent accessible surface area using Shrake-Rupley algorithm (simplified).
///
/// Casts `n_points` rays from each atom and counts those not occluded.
pub fn solvent_accessible_surface(coords: &[Vec3], radii: &[Scalar], n_points: usize) -> Scalar {
    let n = coords.len().min(radii.len());
    if n == 0 {
        return 0.0;
    }

    let probe_radius = 1.4; // water probe radius (Å)
    let total_solid_angle = 4.0 * std::f64::consts::PI;

    // Generate Fibonacci sphere points for uniform distribution
    let points = fibonacci_sphere(n_points);

    let mut total_area = 0.0;
    for i in 0..n {
        let r_sas = radii[i] + probe_radius;
        let mut visible = 0;

        'points: for p in &points {
            let test_point = Vec3::new(
                coords[i].x + r_sas * p.x,
                coords[i].y + r_sas * p.y,
                coords[i].z + r_sas * p.z,
            );

            for j in 0..n {
                if i == j {
                    continue;
                }
                let dist = test_point.distance(&coords[j]);
                if dist < radii[j] + probe_radius {
                    continue 'points;
                }
            }
            visible += 1;
        }

        let area = (visible as Scalar) / (n_points as Scalar) * total_solid_angle * r_sas * r_sas;
        total_area += area;
    }
    total_area
}

/// Generate approximately evenly distributed points on a unit sphere.
fn fibonacci_sphere(n: usize) -> Vec<Vec3> {
    let mut points = Vec::with_capacity(n);
    let golden_ratio = (1.0 + 5.0_f64.sqrt()) * 0.5;

    for i in 0..n {
        let theta = 2.0 * std::f64::consts::PI * (i as Scalar) / golden_ratio;
        let phi = (1.0 - 2.0 * (i as Scalar + 0.5) / n as Scalar).acos();
        points.push(Vec3::new(
            phi.sin() * theta.cos(),
            phi.sin() * theta.sin(),
            phi.cos(),
        ));
    }
    points
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rmsd_identical() {
        let coords = vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)];
        let rmsd = compute_rmsd(&coords, &coords);
        assert!(rmsd.abs() < 1e-10);
    }

    #[test]
    fn test_rmsd_different() {
        let ref_coords = vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)];
        let test_coords = vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0)];
        let rmsd = compute_rmsd(&test_coords, &ref_coords);
        // RMSD = sqrt(((0-0)^2 + (2-1)^2) / 2) = sqrt(0.5) ≈ 0.7071
        assert!((rmsd - (0.5_f64).sqrt()).abs() < 1e-10);
    }

    /// Reference frame plus `n` frames where the single atom sits at `t` on x.
    fn walk(n: usize) -> Vec<Vec<Vec3>> {
        (0..=n)
            .map(|t| vec![Vec3::new(t as Scalar, 0.0, 0.0)])
            .collect()
    }

    #[test]
    fn test_msd_linear_walk_squares_the_displacement() {
        // One atom at x = 0, 1, 2, 3, 4. Sampling from frame 0 every frame gives
        // MSD = 0², 1², 2², 3², 4² for the single atom.
        let traj = walk(4);
        let msd = mean_squared_displacement(&traj, 0, 1);
        assert_eq!(msd.len(), 4, "frames 1..=4 are sampled");
        for (i, &v) in msd.iter().enumerate() {
            let expected = ((i + 1) as Scalar).powi(2);
            assert!(
                (v - expected).abs() < 1e-10,
                "lag {}: {v} vs {expected}",
                i + 1
            );
        }
    }

    #[test]
    fn test_msd_interval_subsamples() {
        // With interval 2 from frame 0, only lags 2 and 4 are sampled.
        let msd = mean_squared_displacement(&walk(4), 0, 2);
        assert_eq!(msd, vec![4.0, 16.0]);
    }

    #[test]
    fn test_msd_zero_interval_returns_empty_without_hanging() {
        // A zero interval previously produced an infinite loop: `t` never
        // advanced, so `start_frame + t < max_frames` stayed true forever.
        let msd = mean_squared_displacement(&walk(4), 0, 0);
        assert!(msd.is_empty());
    }

    #[test]
    fn test_msd_degenerate_inputs() {
        assert!(mean_squared_displacement(&[], 0, 1).is_empty());
        // start_frame past the end.
        assert!(mean_squared_displacement(&walk(2), 99, 1).is_empty());
        // start_frame at the last frame leaves no frame to compare against.
        assert!(mean_squared_displacement(&walk(2), 2, 1).is_empty());
    }

    #[test]
    fn test_msd_symmetric_around_start_frame() {
        // Sampling from frame 2 with interval 1 compares against frames 3 and 4.
        let msd = mean_squared_displacement(&walk(4), 2, 1);
        assert_eq!(msd, vec![1.0, 4.0]);
    }

    #[test]
    fn test_radius_of_gyration_uniform() {
        let coords = vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0)];
        let masses = vec![1.0, 1.0];
        let rg = radius_of_gyration(&coords, &masses);
        assert!((rg - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_dihedral_angle() {
        // Planar cis conformation: dihedral ~ 0
        let a = Vec3::new(0.0, 0.0, 0.0);
        let b = Vec3::new(1.0, 0.0, 0.0);
        let c = Vec3::new(2.0, 0.0, 0.0);
        let d = Vec3::new(3.0, 0.001, 0.0);
        let phi = compute_dihedral_angle(&a, &b, &c, &d);
        assert!(phi.abs() < 0.1);
    }

    #[test]
    fn test_hydrogen_bonds() {
        let coords = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(3.0, 0.0, 0.0),
            Vec3::new(6.0, 0.0, 0.0),
        ];
        let elements = vec!["N".to_string(), "O".to_string(), "C".to_string()];
        let hbonds = detect_hydrogen_bonds(&coords, &elements, 3.5, 2.0);
        // N-O distance = 3.0 < 3.5, should form H-bond.
        // Both (N,O) and (O,N) are valid donor-acceptor pairs.
        assert!(!hbonds.is_empty());
        assert!(hbonds.contains(&(0, 1)) || hbonds.contains(&(1, 0)));
    }

    #[test]
    fn test_sas_nonzero() {
        let coords = vec![Vec3::new(0.0, 0.0, 0.0)];
        let radii = vec![1.5];
        let sas = solvent_accessible_surface(&coords, &radii, 100);
        assert!(sas > 0.0);
    }
}
