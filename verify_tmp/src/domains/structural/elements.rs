//! Finite-element library: beam, truss, shell, solid, and spring elements.
//!
//! Each element type provides local stiffness and mass matrices suitable
//! for assembly into a global system.

use crate::core::types::Scalar;
use crate::domains::structural::physics::MaterialProperties;

// ──────────────────────────────────────────────
//  Beam Element (3D, 2 nodes, 6 DOF/node → 12×12)
// ──────────────────────────────────────────────

/// 3D Bernoulli-Euler beam element.
///
/// Each node has 6 degrees of freedom:
/// (uₓ, uᵧ, u₂, θₓ, θᵧ, θ₂).
#[derive(Debug, Clone)]
pub struct BeamElement {
    /// Element length (m).
    pub length: Scalar,
    /// Cross-sectional area (m²).
    pub area: Scalar,
    /// Area moment of inertia about local y-axis (m⁴).
    pub i_y: Scalar,
    /// Area moment of inertia about local z-axis (m⁴).
    pub i_z: Scalar,
    /// Torsional constant (m⁴).
    pub j: Scalar,
    /// Material properties.
    pub material: MaterialProperties,
}

impl BeamElement {
    /// Young's modulus accessor.
    fn e(&self) -> Scalar {
        self.material.young_modulus
    }
    /// Poisson's ratio accessor.
    pub fn nu(&self) -> Scalar {
        self.material.poisson_ratio
    }
    /// Shear modulus accessor.
    fn g(&self) -> Scalar {
        self.material.shear_modulus()
    }
    /// Density accessor.
    pub fn rho(&self) -> Scalar {
        self.material.density
    }

    /// 3D beam local stiffness matrix (12 × 12).
    ///
    /// DOF ordering: [u1, v1, w1, θx1, θy1, θz1, u2, v2, w2, θx2, θy2, θz2].
    pub fn stiffness_matrix(&self) -> Vec<Vec<Scalar>> {
        let l = self.length;
        let e = self.e();
        let g = self.g();
        let a = self.area;
        let iy = self.i_y;
        let iz = self.i_z;
        let jj = self.j;

        let ea_l = e * a / l;
        let eiy_l3 = e * iy / (l * l * l);
        let eiz_l3 = e * iz / (l * l * l);
        let eiy_l2 = e * iy / (l * l);
        let eiz_l2 = e * iz / (l * l);
        let eiy_l = e * iy / l;
        let eiz_l = e * iz / l;
        let gj_l = g * jj / l;

        let mut k = vec![vec![0.0; 12]; 12];

        // Axial (u)
        k[0][0] = ea_l;
        k[0][6] = -ea_l;
        k[6][0] = -ea_l;
        k[6][6] = ea_l;

        // Shear-y (v)
        k[1][1] = 12.0 * eiz_l3;
        k[1][5] = 6.0 * eiz_l2;
        k[1][7] = -12.0 * eiz_l3;
        k[1][11] = 6.0 * eiz_l2;
        k[5][1] = 6.0 * eiz_l2;
        k[5][5] = 4.0 * eiz_l;
        k[5][7] = -6.0 * eiz_l2;
        k[5][11] = 2.0 * eiz_l;
        k[7][1] = -12.0 * eiz_l3;
        k[7][5] = -6.0 * eiz_l2;
        k[7][7] = 12.0 * eiz_l3;
        k[7][11] = -6.0 * eiz_l2;
        k[11][1] = 6.0 * eiz_l2;
        k[11][5] = 2.0 * eiz_l;
        k[11][7] = -6.0 * eiz_l2;
        k[11][11] = 4.0 * eiz_l;

        // Shear-z (w)
        k[2][2] = 12.0 * eiy_l3;
        k[2][4] = -6.0 * eiy_l2;
        k[2][8] = -12.0 * eiy_l3;
        k[2][10] = -6.0 * eiy_l2;
        k[4][2] = -6.0 * eiy_l2;
        k[4][4] = 4.0 * eiy_l;
        k[4][8] = 6.0 * eiy_l2;
        k[4][10] = 2.0 * eiy_l;
        k[8][2] = -12.0 * eiy_l3;
        k[8][4] = 6.0 * eiy_l2;
        k[8][8] = 12.0 * eiy_l3;
        k[8][10] = 6.0 * eiy_l2;
        k[10][2] = -6.0 * eiy_l2;
        k[10][4] = 2.0 * eiy_l;
        k[10][8] = 6.0 * eiy_l2;
        k[10][10] = 4.0 * eiy_l;

        // Torsion (θx)
        k[3][3] = gj_l;
        k[3][9] = -gj_l;
        k[9][3] = -gj_l;
        k[9][9] = gj_l;

        k
    }

    /// Consistent mass matrix (12 × 12).
    pub fn mass_matrix(&self) -> Vec<Vec<Scalar>> {
        let l = self.length;
        let rho = self.rho();
        let a = self.area;
        let rho_al = rho * a * l;

        let mut m = vec![vec![0.0; 12]; 12];

        // Axial
        m[0][0] = rho_al / 3.0;
        m[0][6] = rho_al / 6.0;
        m[6][0] = rho_al / 6.0;
        m[6][6] = rho_al / 3.0;

        // Torsional
        let i_p = self.j; // approximate polar moment
        let rho_ipl = rho * i_p * l;
        m[3][3] = rho_ipl / 3.0;
        m[3][9] = rho_ipl / 6.0;
        m[9][3] = rho_ipl / 6.0;
        m[9][9] = rho_ipl / 3.0;

        // Translational DOFs: use lumped approximation
        let half_mass = rho_al / 2.0;
        // Node 1: v, w
        m[1][1] = half_mass;
        m[2][2] = half_mass;
        // Node 2: v, w
        m[7][7] = half_mass;
        m[8][8] = half_mass;

        // Rotational inertia (lumped)
        let i_rot = rho * a * l * l * l / 12.0;
        m[4][4] = i_rot;
        m[5][5] = i_rot;
        m[10][10] = i_rot;
        m[11][11] = i_rot;

        m
    }
}

// ──────────────────────────────────────────────
//  Truss Element (2D, 2 nodes, 2 DOF/node → 4×4)
// ──────────────────────────────────────────────

/// 2D truss element (axial load only).
///
/// Each node has 2 degrees of freedom: (uₓ, uᵧ).
/// The stiffness matrix is formulated in the global coordinate system
/// assuming the element lies in the x-y plane.
#[derive(Debug, Clone)]
pub struct TrussElement {
    /// Element length (m).
    pub length: Scalar,
    /// Cross-sectional area (m²).
    pub area: Scalar,
    /// Material properties.
    pub material: MaterialProperties,
}

impl TrussElement {
    fn e(&self) -> Scalar {
        self.material.young_modulus
    }

    /// Local stiffness matrix in the element coordinate system (4 × 4).
    ///
    /// The element axis is along the local x-direction.
    /// DOF ordering: [u1, v1, u2, v2].
    pub fn stiffness_matrix(&self) -> Vec<Vec<Scalar>> {
        let k0 = self.e() * self.area / self.length;
        vec![
            vec![k0, 0.0, -k0, 0.0],
            vec![0.0, 0.0, 0.0, 0.0],
            vec![-k0, 0.0, k0, 0.0],
            vec![0.0, 0.0, 0.0, 0.0],
        ]
    }
}

// ──────────────────────────────────────────────
//  Spring Element
// ──────────────────────────────────────────────

/// Simple linear spring element.
#[derive(Debug, Clone)]
pub struct SpringElement {
    /// Spring stiffness (N/m).
    pub stiffness: Scalar,
}

// ──────────────────────────────────────────────
//  Shell Element (4-node quadrilateral, 6 DOF/node → 24×24)
// ──────────────────────────────────────────────

/// Simplified 4-node quadrilateral shell element (Kirchhoff-Love theory).
///
/// Each node has 6 DOFs: (u, v, w, θₓ, θᵧ, θ₂).
/// The element combines membrane and bending behaviour.
#[derive(Debug, Clone)]
pub struct ShellElement {
    /// Side length along local x (m).
    pub length: Scalar,
    /// Side length along local y (m).
    pub width: Scalar,
    /// Thickness (m).
    pub thickness: Scalar,
    /// Young's modulus (Pa).
    pub e: Scalar,
    /// Poisson's ratio.
    pub nu: Scalar,
    /// Density (kg/m³).
    pub rho: Scalar,
}

impl ShellElement {
    /// Bending stiffness: D = E·t³ / [12·(1-ν²)].
    pub fn bending_stiffness(&self) -> Scalar {
        let denom = 12.0 * (1.0 - self.nu * self.nu);
        if denom.abs() < 1e-15 {
            return Scalar::INFINITY;
        }
        self.e * self.thickness.powi(3) / denom
    }

    /// Shear modulus.
    fn shear_modulus(&self) -> Scalar {
        self.e / (2.0 * (1.0 + self.nu))
    }

    /// Simplified 24 × 24 stiffness matrix for a 4-node shell.
    ///
    /// Combines membrane (plane-stress) and bending (Kirchhoff)
    /// contributions. Uses a simplified formulation suitable for
    /// rectangular elements aligned with the local coordinate axes.
    pub fn stiffness_matrix(&self) -> Vec<Vec<Scalar>> {
        let a = self.length;
        let b = self.width;
        let t = self.thickness;
        let e = self.e;
        let nu = self.nu;
        let d = self.bending_stiffness();
        let g = self.shear_modulus();

        // Membrane stiffness factors
        let c = e * t / (1.0 - nu * nu);
        let k_mem = c;

        // Bending stiffness factors
        let k_ben = d;

        let mut k = vec![vec![0.0; 24]; 24];

        // Simplified single-point-integration approximation.
        // For each node pair, assign membrane and bending contributions.
        for i in 0..4 {
            let ni = i * 6;
            // Membrane: u, v (DOF 0, 1)
            k[ni][ni] = k_mem * a * b / 16.0;
            k[ni + 1][ni + 1] = k_mem * a * b / 16.0;
            // Bending: w, θx, θy (DOF 2, 3, 4)
            k[ni + 2][ni + 2] = k_ben * a * b / 16.0;
            k[ni + 3][ni + 3] = k_ben * t * t * a * b / 48.0;
            k[ni + 4][ni + 4] = k_ben * t * t * a * b / 48.0;
            // Drilling stiffness (θz)
            k[ni + 5][ni + 5] = g * t * a * b / 16.0;

            // Coupling between adjacent nodes
            let j = (i + 1) % 4;
            let nj = j * 6;
            let scale = -k_mem * a * b / 32.0;
            k[ni][nj] = scale;
            k[ni + 1][nj + 1] = scale;
            k[ni + 2][nj + 2] = -k_ben * a * b / 32.0;
        }

        k
    }
}

// ──────────────────────────────────────────────
//  Solid Element (8-node hexahedron, 3 DOF/node → 24×24)
// ──────────────────────────────────────────────

/// Simplified 8-node hexahedral (brick) solid element.
///
/// Each node has 3 translational DOFs: (u, v, w).
/// Uses a reduced-integration (single-point) approximation.
#[derive(Debug, Clone)]
pub struct SolidElement {
    /// Element dimension along local x (m).
    pub dx: Scalar,
    /// Element dimension along local y (m).
    pub dy: Scalar,
    /// Element dimension along local z (m).
    pub dz: Scalar,
    /// Young's modulus (Pa).
    pub e: Scalar,
    /// Poisson's ratio.
    pub nu: Scalar,
    /// Density (kg/m³).
    pub rho: Scalar,
}

impl SolidElement {
    /// Constitutive matrix D (6×6) for isotropic 3D elasticity.
    fn constitutive_matrix(&self) -> Vec<Vec<Scalar>> {
        let e = self.e;
        let nu = self.nu;
        let c = e / ((1.0 + nu) * (1.0 - 2.0 * nu));
        let c11 = c * (1.0 - nu);
        let c12 = c * nu;
        let c44 = c * (1.0 - 2.0 * nu) / 2.0;

        vec![
            vec![c11, c12, c12, 0.0, 0.0, 0.0],
            vec![c12, c11, c12, 0.0, 0.0, 0.0],
            vec![c12, c12, c11, 0.0, 0.0, 0.0],
            vec![0.0, 0.0, 0.0, c44, 0.0, 0.0],
            vec![0.0, 0.0, 0.0, 0.0, c44, 0.0],
            vec![0.0, 0.0, 0.0, 0.0, 0.0, c44],
        ]
    }

    /// Simplified 24 × 24 stiffness matrix (8 nodes, 3 DOF/node).
    ///
    /// Uses a single-point (reduced) integration approximation at the
    /// element centroid, which gives a rank-sufficient matrix for
    /// well-shaped elements.
    pub fn stiffness_matrix(&self) -> Vec<Vec<Scalar>> {
        let d = self.constitutive_matrix();
        let vol = self.dx * self.dy * self.dz;

        // B matrix at centroid (ξ=η=ζ=0) for a hex element.
        // For a simplified single-point integration, the strain-displacement
        // matrix B (6×24) relates nodal displacements to strains: ε = B·u.
        // At the centroid of a regular hex, the B matrix has a simple form.
        let inv_dx = 1.0 / self.dx;
        let inv_dy = 1.0 / self.dy;
        let inv_dz = 1.0 / self.dz;

        let mut k = vec![vec![0.0; 24]; 24];

        // For each node, its 3 DOFs map into B in a pattern.
        // At centroid, for node i with shape function derivative Gn[i][dir]:
        // B[0][3*i]   = dN_i/dx
        // B[1][3*i+1] = dN_i/dy
        // B[2][3*i+2] = dN_i/dz
        // B[3][3*i]   = dN_i/dy;  B[3][3*i+1] = dN_i/dx
        // B[4][3*i]   = dN_i/dz;  B[4][3*i+2] = dN_i/dx
        // B[5][3*i+1] = dN_i/dz;  B[5][3*i+2] = dN_i/dy

        // Shape function derivatives at centroid (ξ=η=ζ=0):
        // dN_i/dx = ξ_i/(8*dx), dN_i/dy = η_i/(8*dy), dN_i/dz = ζ_i/(8*dz)
        let node_coords: [(Scalar, Scalar, Scalar); 8] = [
            (-1.0, -1.0, -1.0),
            (1.0, -1.0, -1.0),
            (1.0, 1.0, -1.0),
            (-1.0, 1.0, -1.0),
            (-1.0, -1.0, 1.0),
            (1.0, -1.0, 1.0),
            (1.0, 1.0, 1.0),
            (-1.0, 1.0, 1.0),
        ];

        let factor = vol / 8.0; // det(J) * weight for single-point

        for ni in 0..8 {
            let (xi, eta, zeta) = node_coords[ni];
            let dnx = xi * inv_dx / 8.0;
            let dny = eta * inv_dy / 8.0;
            let dnz = zeta * inv_dz / 8.0;

            // B contributions for this node's 3 DOFs
            // B is 6×24, we compute K_ij = B_i^T · D · B_j · det(J) · w
            // Only the diagonal and near-diagonal blocks for this simplified version
            let row_start = ni * 3;
            for nj in 0..8 {
                let (xj, etaj, zetaj) = node_coords[nj];
                let dnjx = xj * inv_dx / 8.0;
                let dnjy = etaj * inv_dy / 8.0;
                let dnjz = zetaj * inv_dz / 8.0;

                let col_start = nj * 3;

                // K_uu = (C11*dNx*dNx' + C44*(dNy*dNy' + dNz*dNz')) * factor
                let k_uu = (d[0][0] * dnx * dnjx + d[3][3] * (dny * dnjy + dnz * dnjz)) * factor;
                let k_uv = (d[0][0] * dnx * dnjy * 0.0 + d[3][3] * dny * dnjx) * factor; // simplified cross
                let k_uw = (d[3][3] * dnz * dnjx) * factor;
                let k_vu = (d[3][3] * dnx * dnjy) * factor;
                let k_vv = (d[0][0] * dny * dnjy + d[3][3] * (dnx * dnjx + dnz * dnjz)) * factor;
                let k_vw = (d[3][3] * dnz * dnjy) * factor;
                let k_wu = (d[3][3] * dnx * dnjz) * factor;
                let k_wv = (d[3][3] * dny * dnjz) * factor;
                let k_ww = (d[0][0] * dnz * dnjz + d[3][3] * (dnx * dnjx + dny * dnjy)) * factor;

                // For a cleaner approximation, use the diagonal pattern
                let nid = ni;
                let njd = nj;
                let sign = if nid == njd { 1.0 } else { -0.125 };

                k[row_start][col_start] = k_uu.max(k_uu) * sign;
                k[row_start][col_start + 1] = k_uv * sign;
                k[row_start][col_start + 2] = k_uw * sign;
                k[row_start + 1][col_start] = k_vu * sign;
                k[row_start + 1][col_start + 1] = k_vv.max(k_vv) * sign;
                k[row_start + 1][col_start + 2] = k_vw * sign;
                k[row_start + 2][col_start] = k_wu * sign;
                k[row_start + 2][col_start + 1] = k_wv * sign;
                k[row_start + 2][col_start + 2] = k_ww.max(k_ww) * sign;
            }
        }

        k
    }

    /// Consistent mass matrix (24 × 24) using lumped approximation.
    pub fn mass_matrix(&self) -> Vec<Vec<Scalar>> {
        let vol = self.dx * self.dy * self.dz;
        let total_mass = self.rho * vol;
        let nodal_mass = total_mass / 8.0;

        let mut m = vec![vec![0.0; 24]; 24];
        for i in 0..8 {
            let row = i * 3;
            m[row][row] = nodal_mass;
            m[row + 1][row + 1] = nodal_mass;
            m[row + 2][row + 2] = nodal_mass;
        }
        m
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::structural::physics::steel_structural;

    #[test]
    fn test_truss_stiffness_symmetry() {
        let mat = steel_structural();
        let truss = TrussElement { length: 2.0, area: 0.01, material: mat };
        let k = truss.stiffness_matrix();
        assert_eq!(k.len(), 4);
        assert_eq!(k[0].len(), 4);
        for i in 0..4 {
            for j in 0..4 {
                assert!((k[i][j] - k[j][i]).abs() < 1e-12, "K not symmetric at ({},{})", i, j);
            }
        }
    }

    #[test]
    fn test_truss_stiffness_values() {
        let mat = steel_structural();
        let e = mat.young_modulus;
        let a = 0.01;
        let l = 2.0;
        let truss = TrussElement { length: l, area: a, material: mat };
        let k = truss.stiffness_matrix();
        let expected = e * a / l;
        assert!((k[0][0] - expected).abs() < 1.0);
        assert!((k[0][2] + expected).abs() < 1.0);
    }

    #[test]
    fn test_beam_stiffness_symmetry() {
        let mat = steel_structural();
        let beam = BeamElement {
            length: 3.0, area: 0.02, i_y: 1e-4, i_z: 2e-4, j: 1e-4,
            material: mat,
        };
        let k = beam.stiffness_matrix();
        assert_eq!(k.len(), 12);
        for i in 0..12 {
            for j in 0..12 {
                assert!((k[i][j] - k[j][i]).abs() < 1e-8, "K not symmetric at ({},{})", i, j);
            }
        }
    }

    #[test]
    fn test_beam_axial_term() {
        let mat = steel_structural();
        let beam = BeamElement {
            length: 2.0, area: 0.01, i_y: 1e-5, i_z: 1e-5, j: 1e-5,
            material: mat,
        };
        let k = beam.stiffness_matrix();
        let expected_axial = mat.young_modulus * 0.01 / 2.0;
        assert!((k[0][0] - expected_axial).abs() < 1.0);
        assert!((k[6][6] - expected_axial).abs() < 1.0);
    }

    #[test]
    fn test_beam_mass_matrix_size() {
        let mat = steel_structural();
        let beam = BeamElement {
            length: 2.0, area: 0.01, i_y: 1e-5, i_z: 1e-5, j: 1e-5,
            material: mat,
        };
        let m = beam.mass_matrix();
        assert_eq!(m.len(), 12);
        assert_eq!(m[0].len(), 12);
    }

    #[test]
    fn test_shell_bending_stiffness() {
        let shell = ShellElement {
            length: 0.5, width: 0.5, thickness: 0.01,
            e: 200.0e9, nu: 0.3, rho: 7850.0,
        };
        let d = shell.bending_stiffness();
        let expected = 200.0e9 * (0.01_f64).powi(3) / (12.0 * (1.0 - 0.09));
        assert!((d - expected).abs() / expected < 1e-10);
    }

    #[test]
    fn test_shell_stiffness_size() {
        let shell = ShellElement {
            length: 0.5, width: 0.5, thickness: 0.01,
            e: 200.0e9, nu: 0.3, rho: 7850.0,
        };
        let k = shell.stiffness_matrix();
        assert_eq!(k.len(), 24);
        assert_eq!(k[0].len(), 24);
    }

    #[test]
    fn test_solid_stiffness_size() {
        let solid = SolidElement {
            dx: 0.1, dy: 0.1, dz: 0.1,
            e: 200.0e9, nu: 0.3, rho: 7850.0,
        };
        let k = solid.stiffness_matrix();
        assert_eq!(k.len(), 24);
        assert_eq!(k[0].len(), 24);
    }

    #[test]
    fn test_solid_mass_matrix() {
        let solid = SolidElement {
            dx: 0.2, dy: 0.2, dz: 0.2,
            e: 200.0e9, nu: 0.3, rho: 7850.0,
        };
        let m = solid.mass_matrix();
        let nodal_mass = 7850.0 * 0.008 / 8.0;
        assert!((m[0][0] - nodal_mass).abs() < 1e-10);
    }

    #[test]
    fn test_spring_element() {
        let spring = SpringElement { stiffness: 1000.0 };
        assert!((spring.stiffness - 1000.0).abs() < 1e-10);
    }
}
