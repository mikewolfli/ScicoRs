//! Jones and Mueller matrix calculus for polarised light.
//!
//! Provides Jones matrices for common polarisation optics
//! (linear polariser, wave plates, rotators) and Mueller matrix
//! equivalents for partially polarised light.

use crate::core::types::Scalar;
/// Complex scalar type alias.
type CS = num_complex::Complex<f64>;

/// 2×2 Jones matrix for fully polarised light.
#[derive(Debug, Clone, Copy)]
pub struct JonesMatrix {
    pub data: [[CS; 2]; 2],
}

impl JonesMatrix {
    /// Create a new Jones matrix from its elements.
    pub fn new(data: [[CS; 2]; 2]) -> Self {
        Self { data }
    }

    /// Identity matrix (no transformation).
    pub fn identity() -> Self {
        Self {
            data: [
                [CS::new(1.0, 0.0), CS::new(0.0, 0.0)],
                [CS::new(0.0, 0.0), CS::new(1.0, 0.0)],
            ],
        }
    }

    /// Linear polariser with transmission axis at angle `θ` (radians).
    pub fn linear_polarizer(theta: Scalar) -> Self {
        let c = theta.cos();
        let s = theta.sin();
        Self {
            data: [
                [CS::new(c * c, 0.0), CS::new(c * s, 0.0)],
                [CS::new(c * s, 0.0), CS::new(s * s, 0.0)],
            ],
        }
    }

    /// Quarter-wave plate with fast axis at angle `θ` (radians).
    pub fn quarter_wave_plate(fast_axis: Scalar) -> Self {
        let c = fast_axis.cos();
        let s = fast_axis.sin();
        let c2 = c * c;
        let s2 = s * s;
        let cs = c * s;
        Self {
            data: [
                [
                    CS::new(c2 + s2 * CS::new(0.0, 1.0).re, 0.0),
                    CS::new(cs - cs * CS::new(0.0, 1.0).re, 0.0),
                ],
                [
                    CS::new(cs - cs * CS::new(0.0, 1.0).re, 0.0),
                    CS::new(s2 + c2 * CS::new(0.0, 1.0).re, 0.0),
                ],
            ],
        }
    }

    /// Half-wave plate with fast axis at angle `θ`.
    pub fn half_wave_plate(fast_axis: Scalar) -> Self {
        let c = (2.0 * fast_axis).cos();
        let s = (2.0 * fast_axis).sin();
        Self {
            data: [
                [CS::new(c, 0.0), CS::new(s, 0.0)],
                [CS::new(s, 0.0), CS::new(-c, 0.0)],
            ],
        }
    }

    /// Rotation matrix by angle `θ`.
    pub fn rotator(theta: Scalar) -> Self {
        let c = theta.cos();
        let s = theta.sin();
        Self {
            data: [
                [CS::new(c, 0.0), CS::new(s, 0.0)],
                [CS::new(-s, 0.0), CS::new(c, 0.0)],
            ],
        }
    }

    /// Apply Jones matrix to a Jones vector [Ex, Ey].
    pub fn apply(&self, state: &[CS; 2]) -> [CS; 2] {
        [
            self.data[0][0] * state[0] + self.data[0][1] * state[1],
            self.data[1][0] * state[0] + self.data[1][1] * state[1],
        ]
    }

    /// Multiply two Jones matrices: self × other.
    pub fn multiply(&self, other: &JonesMatrix) -> JonesMatrix {
        let mut result = [[CS::new(0.0, 0.0); 2]; 2];
        for i in 0..2 {
            for j in 0..2 {
                for k in 0..2 {
                    result[i][j] += self.data[i][k] * other.data[k][j];
                }
            }
        }
        JonesMatrix { data: result }
    }

    /// Determinant of the Jones matrix.
    pub fn determinant(&self) -> CS {
        self.data[0][0] * self.data[1][1] - self.data[0][1] * self.data[1][0]
    }

    /// Trace of the Jones matrix.
    pub fn trace(&self) -> CS {
        self.data[0][0] + self.data[1][1]
    }
}

/// 4×4 Mueller matrix for partially polarised light.
#[derive(Debug, Clone)]
pub struct MuellerMatrix {
    pub data: [[Scalar; 4]; 4],
}

impl MuellerMatrix {
    pub fn new(data: [[Scalar; 4]; 4]) -> Self {
        Self { data }
    }

    /// Identity Mueller matrix.
    pub fn identity() -> Self {
        let mut m = [[0.0; 4]; 4];
        m[0][0] = 1.0;
        m[1][1] = 1.0;
        m[2][2] = 1.0;
        m[3][3] = 1.0;
        Self { data: m }
    }

    /// Convert a Jones matrix to Mueller form (for non-depolarising elements).
    pub fn from_jones(j: &JonesMatrix) -> Self {
        let mut m = [[0.0; 4]; 4];
        // Jones → Mueller conversion
        let jj = [
            [j.data[0][0], j.data[0][1]],
            [j.data[1][0], j.data[1][1]],
        ];
        let t0 = jj[0][0] * jj[0][0].conj()
            + jj[0][1] * jj[0][1].conj()
            + jj[1][0] * jj[1][0].conj()
            + jj[1][1] * jj[1][1].conj();
        let t1 = jj[0][0] * jj[0][0].conj()
            + jj[0][1] * jj[0][1].conj()
            - jj[1][0] * jj[1][0].conj()
            - jj[1][1] * jj[1][1].conj();
        let t2 = jj[0][0] * jj[0][1].conj()
            + jj[0][1] * jj[0][0].conj()
            + jj[1][0] * jj[1][1].conj()
            + jj[1][1] * jj[1][0].conj();
        let t3 = (jj[0][0] * jj[0][1].conj()
            - jj[0][1] * jj[0][0].conj()
            + jj[1][0] * jj[1][1].conj()
            - jj[1][1] * jj[1][0].conj())
            * CS::new(0.0, 1.0);
        m[0][0] = t0.re * 0.5;
        m[0][1] = t1.re * 0.5;
        m[0][2] = t2.re * 0.5;
        m[0][3] = t3.re * 0.5;
        // ... simplified: additional rows for full conversion
        m[1][0] = (jj[0][0] * jj[0][0].conj()
            - jj[0][1] * jj[0][1].conj()
            + jj[1][0] * jj[1][0].conj()
            - jj[1][1] * jj[1][1].conj())
            .re
            * 0.5;
        m[1][1] = (jj[0][0] * jj[0][0].conj()
            - jj[0][1] * jj[0][1].conj()
            - jj[1][0] * jj[1][0].conj()
            + jj[1][1] * jj[1][1].conj())
            .re
            * 0.5;
        Self { data: m }
    }

    /// Apply Mueller matrix to a Stokes vector [I, Q, U, V].
    pub fn apply(&self, stokes: &[Scalar; 4]) -> [Scalar; 4] {
        let mut result = [0.0; 4];
        for i in 0..4 {
            for j in 0..4 {
                result[i] += self.data[i][j] * stokes[j];
            }
        }
        result
    }

    /// Multiply two Mueller matrices.
    pub fn multiply(&self, other: &MuellerMatrix) -> MuellerMatrix {
        let mut result = [[0.0; 4]; 4];
        for i in 0..4 {
            for j in 0..4 {
                for k in 0..4 {
                    result[i][j] += self.data[i][k] * other.data[k][j];
                }
            }
        }
        MuellerMatrix { data: result }
    }
}

/// Jones vector representing a polarisation state [Ex, Ey].
pub fn jones_vector(phase_x: Scalar, phase_y: Scalar, amplitude_ratio: Scalar) -> [CS; 2] {
    let norm = (1.0 + amplitude_ratio * amplitude_ratio).sqrt().recip();
    [
        CS::new(norm * phase_x.cos(), norm * phase_x.sin()),
        CS::new(
            norm * amplitude_ratio * phase_y.cos(),
            norm * amplitude_ratio * phase_y.sin(),
        ),
    ]
}

/// Stokes vector from Jones vector.
pub fn stokes_from_jones(j: &[CS; 2]) -> [Scalar; 4] {
    let ex = j[0];
    let ey = j[1];
    let i = ex.norm_sqr() + ey.norm_sqr();
    let q = ex.norm_sqr() - ey.norm_sqr();
    let u = 2.0 * (ex * ey.conj()).re;
    let v = 2.0 * (ex * ey.conj()).im;
    [i, q, u, v]
}

/// Degree of polarisation from Stokes vector.
pub fn degree_of_polarisation(stokes: &[Scalar; 4]) -> Scalar {
    if stokes[0] <= 0.0 {
        return 0.0;
    }
    (stokes[1] * stokes[1] + stokes[2] * stokes[2] + stokes[3] * stokes[3]).sqrt() / stokes[0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jones_identity() {
        let j = JonesMatrix::identity();
        let v = j.apply(&[CS::new(1.0, 0.0), CS::new(0.0, 0.0)]);
        assert!((v[0].re - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_linear_polarizer() {
        // Horizontal polariser (θ=0): passes Ex
        let j = JonesMatrix::linear_polarizer(0.0);
        let v = j.apply(&[CS::new(1.0, 0.0), CS::new(1.0, 0.0)]);
        assert!((v[0].re - 1.0).abs() < 1e-10);
        assert!((v[1].re - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_half_wave_plate() {
        // HWP at 45° flips H↔V
        let j = JonesMatrix::half_wave_plate(std::f64::consts::FRAC_PI_4);
        let v = j.apply(&[CS::new(1.0, 0.0), CS::new(0.0, 0.0)]);
        assert!(v[0].re.abs() < 1e-10, "H should become V");
        assert!(v[1].re.abs() > 0.9, "should have V component");
    }

    #[test]
    fn test_rotator() {
        let j = JonesMatrix::rotator(std::f64::consts::FRAC_PI_2);
        let v = j.apply(&[CS::new(1.0, 0.0), CS::new(0.0, 0.0)]);
        assert!((v[0].re - 0.0).abs() < 1e-10);
        assert!((v[1].re + 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_jones_multiply() {
        let j1 = JonesMatrix::linear_polarizer(0.0);
        let j2 = JonesMatrix::rotator(std::f64::consts::FRAC_PI_4);
        let j3 = j2.multiply(&j1);
        assert!(j3.data[0][0].norm_sqr() > 0.0);
    }

    #[test]
    fn test_jones_determinant() {
        let j = JonesMatrix::identity();
        let det = j.determinant();
        assert!((det.re - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_mueller_identity() {
        let m = MuellerMatrix::identity();
        let s = m.apply(&[1.0, 0.0, 0.0, 1.0]);
        assert!((s[0] - 1.0).abs() < 1e-10);
        assert!((s[3] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_jones_vector() {
        let jv = jones_vector(0.0, 0.0, 1.0);
        assert!((jv[0].norm_sqr() + jv[1].norm_sqr() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_stokes_from_jones() {
        let jv = [CS::new(1.0, 0.0), CS::new(0.0, 0.0)];
        let s = stokes_from_jones(&jv);
        assert!((s[0] - 1.0).abs() < 1e-10);
        assert!((s[1] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_degree_of_polarisation() {
        let s = [1.0, 0.6, 0.0, 0.8];
        let dop = degree_of_polarisation(&s);
        assert!((dop - 1.0).abs() < 1e-10);
    }
}
