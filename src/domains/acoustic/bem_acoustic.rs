//! Boundary Element Method (BEM) for acoustic radiation and scattering.
//!
//! Solves the Helmholtz equation in the exterior domain using a
//! collocation BEM with constant triangular elements.

use crate::core::coord::Coord3D;
use crate::core::types::Scalar;
/// Complex scalar type alias to avoid `CS::new()` parsing issues.
type CS = num_complex::Complex<f64>;

/// BEM acoustic solver for radiation and scattering problems.
///
/// Discretises the surface of a radiating/scattering object into
/// triangular elements and solves for the surface pressure given
/// normal velocity boundary conditions.
#[derive(Debug, Clone)]
pub struct AcousticBEM {
    /// Surface nodes (3D coordinates).
    pub nodes: Vec<Coord3D>,
    /// Triangular element connectivity (i, j, k indices into nodes).
    pub elements: Vec<(usize, usize, usize)>,
    /// Analysis frequency (Hz).
    pub frequency: Scalar,
    /// Speed of sound in the medium (m/s).
    pub c: Scalar,
    /// Density of the medium (kg/m³).
    pub rho: Scalar,
}

impl AcousticBEM {
    /// Create a new acoustic BEM solver.
    pub fn new(
        nodes: Vec<Coord3D>,
        elements: Vec<(usize, usize, usize)>,
        frequency: Scalar,
        c: Scalar,
        rho: Scalar,
    ) -> Self {
        assert!(!nodes.is_empty(), "Must have at least one node");
        assert!(!elements.is_empty(), "Must have at least one element");
        assert!(frequency > 0.0, "Frequency must be positive");
        Self {
            nodes,
            elements,
            frequency,
            c,
            rho,
        }
    }

    /// Wave number: k = 2πf/c.
    pub fn wave_number(&self) -> Scalar {
        2.0 * std::f64::consts::PI * self.frequency / self.c
    }

    /// Green's function value: G(r) = exp(ikr) / (4πr).
    fn green_f(&self, r: Scalar) -> CS {
        let k = self.wave_number();
        if r < 1e-30 {
            return CS::new(0.0, 0.0);
        }
        let phase = k * r;
        let (c, s) = phase.sin_cos();
        CS::new(c, s) / (4.0 * std::f64::consts::PI * r)
    }

    /// Normal derivative of Green's function: ∂G/∂n.
    ///
    /// For G = e^{ikr}/(4πr), ∂G/∂n = G·cosθ·(ik − 1/r).
    fn green_dn(&self, r: Scalar, cos_angle: Scalar) -> CS {
        let k = self.wave_number();
        if r < 1e-30 {
            return CS::new(0.0, 0.0);
        }
        let phase = k * r;
        let (c, s) = phase.sin_cos();
        let g = CS::new(c, s) / (4.0 * std::f64::consts::PI * r);
        g * cos_angle * (CS::new(0.0, k) - CS::new(1.0, 0.0) / r)
    }

    /// Compute the surface pressure from given normal velocities.
    ///
    /// Solves the BEM system: H·p = G·v_n
    /// where H is the double-layer potential matrix and G is the
    /// single-layer potential matrix.
    pub fn surface_pressure(&self, v_n: &[Scalar]) -> Result<Vec<CS>, String> {
        let n = self.nodes.len();

        if v_n.len() != self.elements.len() {
            return Err(format!(
                "Expected {} velocities, got {}",
                self.elements.len(),
                v_n.len()
            ));
        }

        // Build G (single-layer) and H (double-layer) matrices
        // Simplified: lumped at nodes using element centroid approximation
        let mut g_mat = vec![vec![CS::new(0.0, 0.0); n]; n];
        let mut h_mat = vec![vec![CS::new(0.0, 0.0); n]; n];

        for &(ia, ib, ic) in &self.elements {
            // Element centroid
            let centre = Coord3D::new(
                (self.nodes[ia].x + self.nodes[ib].x + self.nodes[ic].x) / 3.0,
                (self.nodes[ia].y + self.nodes[ib].y + self.nodes[ic].y) / 3.0,
                (self.nodes[ia].z + self.nodes[ib].z + self.nodes[ic].z) / 3.0,
            );
            // Element area (half of cross-product magnitude)
            let v1 = Coord3D::new(
                self.nodes[ib].x - self.nodes[ia].x,
                self.nodes[ib].y - self.nodes[ia].y,
                self.nodes[ib].z - self.nodes[ia].z,
            );
            let v2 = Coord3D::new(
                self.nodes[ic].x - self.nodes[ia].x,
                self.nodes[ic].y - self.nodes[ia].y,
                self.nodes[ic].z - self.nodes[ia].z,
            );
            let cross = Coord3D::new(
                v1.y * v2.z - v1.z * v2.y,
                v1.z * v2.x - v1.x * v2.z,
                v1.x * v2.y - v1.y * v2.x,
            );
            let area = 0.5 * (cross.x * cross.x + cross.y * cross.y + cross.z * cross.z).sqrt();
            // Unit normal
            let norm = (cross.x * cross.x + cross.y * cross.y + cross.z * cross.z)
                .sqrt()
                .max(1e-30);
            let nx = cross.x / norm;
            let ny = cross.y / norm;
            let nz = cross.z / norm;

            let elem_nodes = [ia, ib, ic];

            for &node_j in &elem_nodes {
                let pj = self.nodes[node_j];
                let dx = centre.x - pj.x;
                let dy = centre.y - pj.y;
                let dz = centre.z - pj.z;
                let r = (dx * dx + dy * dy + dz * dz).sqrt();

                // cos(angle between r and normal)
                let cos_angle = if r > 1e-30 {
                    (dx * nx + dy * ny + dz * nz) / r
                } else {
                    -0.5 // Self-term approximation
                };

                let g_val = self.green_f(r);
                let h_val = self.green_dn(r, cos_angle);

                for &node_i in &elem_nodes {
                    // Distribute element contribution to its nodes
                    g_mat[node_i][node_j] += g_val * area / 3.0;
                    h_mat[node_i][node_j] += h_val * area / 3.0;
                }
            }
        }

        // Add diagonal for H: H_ii += 0.5 (solid angle for smooth surface)
        for i in 0..n {
            h_mat[i][i] += CS::new(0.5, 0.0);
        }

        // Solve H·p = G·(i·ρ·c·k·v_n) — actually G·(i·ω·ρ·v_n)
        let omega = 2.0 * std::f64::consts::PI * self.frequency;
        let rhs_factor = CS::new(0.0, omega * self.rho);

        // Build RHS: b_i = Σ_j G_ij * (i·ω·ρ·v_n_j)
        let mut rhs = vec![CS::new(0.0, 0.0); n];
        for i in 0..n {
            for (ej, &(_, _, _)) in self.elements.iter().enumerate() {
                let elem_nodes = [
                    self.elements[ej].0,
                    self.elements[ej].1,
                    self.elements[ej].2,
                ];
                let vn = v_n[ej];
                for &node_j in &elem_nodes {
                    rhs[i] += g_mat[i][node_j] * rhs_factor * vn;
                }
            }
        }

        // Solve linear system (simplified: use Gaussian elimination on H)
        let p = solve_complex_bem(&h_mat, &rhs, n)?;
        Ok(p)
    }

    /// Compute far-field pressure at a given direction.
    ///
    /// Uses the Fraunhofer approximation (large distance).
    pub fn far_field_pattern(&self, theta: Scalar, phi: Scalar, surface_p: &[CS]) -> CS {
        let k = self.wave_number();
        // Observation direction unit vector
        let ux = theta.sin() * phi.cos();
        let uy = theta.sin() * phi.sin();
        let uz = theta.cos();

        let mut p_far = CS::new(0.0, 0.0);

        for &(ia, ib, ic) in &self.elements {
            let centre = Coord3D::new(
                (self.nodes[ia].x + self.nodes[ib].x + self.nodes[ic].x) / 3.0,
                (self.nodes[ia].y + self.nodes[ib].y + self.nodes[ic].y) / 3.0,
                (self.nodes[ia].z + self.nodes[ib].z + self.nodes[ic].z) / 3.0,
            );
            let phase = k * (ux * centre.x + uy * centre.y + uz * centre.z);
            let (c, s) = phase.sin_cos();
            let ejkr = CS::new(c, s);

            let p_avg = (surface_p[ia] + surface_p[ib] + surface_p[ic]) / 3.0;
            p_far += p_avg * ejkr;
        }

        p_far
    }
}

/// Solve a complex linear system using Gaussian elimination (small BEM systems).
fn solve_complex_bem(a: &[Vec<CS>], b: &[CS], n: usize) -> Result<Vec<CS>, String> {
    // Augmented matrix
    let mut aug = vec![vec![CS::new(0.0, 0.0); n + 1]; n];
    for i in 0..n {
        for j in 0..n {
            aug[i][j] = a[i][j];
        }
        aug[i][n] = b[i];
    }

    // Forward elimination
    for col in 0..n {
        // Partial pivot
        let mut max_row = col;
        for row in (col + 1)..n {
            if aug[row][col].norm_sqr() > aug[max_row][col].norm_sqr() {
                max_row = row;
            }
        }
        aug.swap(col, max_row);

        let pivot = aug[col][col];
        if pivot.norm_sqr() < 1e-30 {
            return Err("Singular BEM matrix".to_string());
        }

        for row in (col + 1)..n {
            let factor = aug[row][col] / pivot;
            let pivot_row = col;
            for j in col..=n {
                let fv = factor;
                let pv = aug[pivot_row][j];
                aug[row][j] -= fv * pv;
            }
        }
    }

    // Back substitution
    let mut x = vec![CS::new(0.0, 0.0); n];
    for i in (0..n).rev() {
        let mut sum = CS::new(0.0, 0.0);
        for j in (i + 1)..n {
            sum += aug[i][j] * x[j];
        }
        x[i] = (aug[i][n] - sum) / aug[i][i];
    }
    Ok(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sphere_bem() -> AcousticBEM {
        // Simple 8-node cube surface (6 faces, 12 triangles)
        let nodes = vec![
            Coord3D::new(-0.5, -0.5, -0.5),
            Coord3D::new(0.5, -0.5, -0.5),
            Coord3D::new(0.5, 0.5, -0.5),
            Coord3D::new(-0.5, 0.5, -0.5),
            Coord3D::new(-0.5, -0.5, 0.5),
            Coord3D::new(0.5, -0.5, 0.5),
            Coord3D::new(0.5, 0.5, 0.5),
            Coord3D::new(-0.5, 0.5, 0.5),
        ];
        let elements = vec![
            (0, 1, 2),
            (0, 2, 3), // bottom
            (4, 5, 6),
            (4, 6, 7), // top
            (0, 1, 5),
            (0, 5, 4), // front
            (2, 3, 7),
            (2, 7, 6), // back
            (0, 3, 7),
            (0, 7, 4), // left
            (1, 2, 6),
            (1, 6, 5), // right
        ];
        AcousticBEM::new(nodes, elements, 1000.0, 343.0, 1.2)
    }

    #[test]
    fn test_bem_new() {
        let bem = make_sphere_bem();
        assert_eq!(bem.nodes.len(), 8);
        assert_eq!(bem.elements.len(), 12);
        assert!((bem.frequency - 1000.0).abs() < 1e-10);
    }

    #[test]
    fn test_wave_number() {
        let bem = make_sphere_bem();
        let k = bem.wave_number();
        let expected = 2.0 * std::f64::consts::PI * 1000.0 / 343.0;
        assert!((k - expected).abs() < 1e-10);
    }

    #[test]
    fn test_green_function() {
        let bem = make_sphere_bem();
        let g = bem.green_f(1.0);
        assert!(g.norm_sqr() > 0.0);
    }

    #[test]
    fn test_surface_pressure() {
        let bem = make_sphere_bem();
        let v_n = vec![0.01; 12]; // Uniform normal velocity
        let p = bem.surface_pressure(&v_n);
        assert!(p.is_ok());
        let p = p.unwrap();
        assert_eq!(p.len(), 8);
        // Pressure should be finite
        for &pi in &p {
            assert!(pi.norm_sqr().is_finite());
        }
    }

    #[test]
    fn test_far_field() {
        let bem = make_sphere_bem();
        let v_n = vec![0.01; 12];
        if let Ok(p) = bem.surface_pressure(&v_n) {
            let p_far = bem.far_field_pattern(0.0, 0.0, &p);
            assert!(p_far.norm_sqr().is_finite());
        }
    }

    #[test]
    fn test_solve_complex_bem() {
        // Simple 2×2 system
        let a = vec![
            vec![CS::new(2.0, 0.0), CS::new(1.0, 0.0)],
            vec![CS::new(1.0, 0.0), CS::new(3.0, 0.0)],
        ];
        let b = vec![CS::new(5.0, 0.0), CS::new(6.0, 0.0)];
        let x = solve_complex_bem(&a, &b, 2).unwrap();
        // Check: 2*x0 + x1 = 5, x0 + 3*x1 = 6 → x = [1.8, 1.4]
        assert!((x[0].re - 1.8).abs() < 1e-10, "x[0] = {}", x[0].re);
        assert!((x[1].re - 1.4).abs() < 1e-10, "x[1] = {}", x[1].re);
    }

    #[test]
    fn test_solve_singular() {
        let a = vec![vec![CS::new(0.0, 0.0); 2]; 2];
        let b = vec![CS::new(1.0, 0.0); 2];
        assert!(solve_complex_bem(&a, &b, 2).is_err());
    }

    #[test]
    fn test_surface_pressure_nonuniform() {
        let bem = make_sphere_bem();
        // Non-uniform normal velocity (piston-like)
        let v_n: Vec<Scalar> = (0..12)
            .map(|i| 0.01 * (1.0 + (i as Scalar / 12.0)))
            .collect();
        let p = bem.surface_pressure(&v_n).unwrap();
        assert_eq!(p.len(), 8);
    }
}
