//! Cavity acoustics: room modes, Helmholtz resonance, reverberation time.

use crate::core::coord::Coord3D;
use crate::core::types::Scalar;

/// Rectangular cavity/room acoustic parameters.
#[derive(Debug, Clone)]
pub struct Cavity {
    pub dimensions: Coord3D,
    pub wall_material: String,
}

impl Cavity {
    pub fn new(dimensions: Coord3D) -> Self {
        Self {
            dimensions,
            wall_material: "generic".to_string(),
        }
    }
}

/// Rectangular room eigenfrequencies.
///
/// f_{nx,ny,nz} = c/2 · √((nx/Lx)² + (ny/Ly)² + (nz/Lz)²)
pub fn rectangular_room_modes(
    dims: &Coord3D,
    c: Scalar,
    max_freq: Scalar,
) -> Vec<(i32, i32, i32, Scalar)> {
    let max_nx = (2.0 * max_freq * dims.x / c) as i32;
    let max_ny = (2.0 * max_freq * dims.y / c) as i32;
    let max_nz = (2.0 * max_freq * dims.z / c) as i32;
    let (nxw, nyw, nzw) = (max_nx as i64 + 1, max_ny as i64 + 1, max_nz as i64 + 1);
    let total = nxw * nyw * nzw;

    // Each mode (nx, ny, nz) is an independent frequency evaluation; large
    // mode spaces are enumerated on rayon (order doesn't matter — sorted after).
    let mode_at = |idx: i64| -> (i32, i32, i32, Scalar) {
        let nx = (idx / (nyw * nzw)) as i32;
        let rem = idx % (nyw * nzw);
        let ny = (rem / nzw) as i32;
        let nz = (rem % nzw) as i32;
        let fx = (nx as Scalar / dims.x).powi(2);
        let fy = (ny as Scalar / dims.y).powi(2);
        let fz = (nz as Scalar / dims.z).powi(2);
        let f = 0.5 * c * (fx + fy + fz).sqrt();
        (nx, ny, nz, f)
    };

    /// Mode-space size at which rayon pays for itself.
    const PAR_MIN_MODES: i64 = 1024;
    let mut modes: Vec<(i32, i32, i32, Scalar)> = if total >= PAR_MIN_MODES {
        use rayon::prelude::*;
        (0..total)
            .into_par_iter()
            .map(mode_at)
            .filter(|&(nx, ny, nz, f)| !(nx == 0 && ny == 0 && nz == 0) && f <= max_freq)
            .collect()
    } else {
        (0..total)
            .map(mode_at)
            .filter(|&(nx, ny, nz, f)| !(nx == 0 && ny == 0 && nz == 0) && f <= max_freq)
            .collect()
    };
    modes.sort_by(|a, b| a.3.partial_cmp(&b.3).unwrap());
    modes
}

/// Helmholtz resonance frequency.
///
/// f₀ = (c/(2π)) · √(A/(V·L))
/// A = neck cross-sectional area, L = neck length, V = cavity volume.
pub fn helmholtz_resonance(
    c: Scalar,
    neck_area: Scalar,
    neck_length: Scalar,
    volume: Scalar,
) -> Scalar {
    if volume <= 0.0 || neck_length <= 0.0 {
        return 0.0;
    }
    (c / (2.0 * std::f64::consts::PI)) * (neck_area / (volume * neck_length)).sqrt()
}

/// Sabine reverberation time RT60.
///
/// T₆₀ = 0.161 · V / Σ(αᵢ·Sᵢ)
pub fn rt60_sabine(volume: Scalar, areas: &[Scalar], absorption_coeffs: &[Scalar]) -> Scalar {
    if volume <= 0.0 {
        return 0.0;
    }
    let mut total_absorption = 0.0;
    for (&area, &alpha) in areas.iter().zip(absorption_coeffs.iter()) {
        total_absorption += area * alpha;
    }
    if total_absorption <= 0.0 {
        return Scalar::INFINITY;
    }
    0.161 * volume / total_absorption
}

/// Critical distance: r_c = 0.057 · √(V / RT60).
pub fn critical_distance(volume: Scalar, rt60: Scalar) -> Scalar {
    if rt60 <= 0.0 {
        return Scalar::INFINITY;
    }
    0.057 * (volume / rt60).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rectangular_room_modes() {
        let dims = Coord3D::new(5.0, 4.0, 3.0);
        let modes = rectangular_room_modes(&dims, 343.0, 100.0);
        // Should find at least the fundamental mode (1,0,0) or (0,1,0)
        assert!(!modes.is_empty());
        // First mode frequency: f(1,0,0) = 343/(2*5) = 34.3 Hz
        assert!((modes[0].3 - 34.3).abs() < 0.1);
    }

    #[test]
    fn test_rectangular_room_modes_parallel_matches_serial_reference() {
        // max_freq=1000 Hz → ~10^4 mode space (> PAR_MIN_MODES=1024) → rayon
        // path; the sorted mode set must equal the original serial enumeration.
        let dims = Coord3D::new(5.0, 4.0, 3.0);
        let (c, max_freq) = (343.0, 1000.0);
        let modes = rectangular_room_modes(&dims, c, max_freq);

        let lx = 5.0;
        let ly = 4.0;
        let lz = 3.0;
        let max_nx = (2.0 * max_freq * lx / c) as i32;
        let max_ny = (2.0 * max_freq * ly / c) as i32;
        let max_nz = (2.0 * max_freq * lz / c) as i32;
        let mut ref_modes = Vec::new();
        for nx in 0..=max_nx {
            for ny in 0..=max_ny {
                for nz in 0..=max_nz {
                    if nx == 0 && ny == 0 && nz == 0 {
                        continue;
                    }
                    let f = 0.5
                        * c
                        * ((nx as Scalar / lx).powi(2)
                            + (ny as Scalar / ly).powi(2)
                            + (nz as Scalar / lz).powi(2))
                        .sqrt();
                    if f <= max_freq {
                        ref_modes.push((nx, ny, nz, f));
                    }
                }
            }
        }
        ref_modes.sort_by(|a, b| a.3.partial_cmp(&b.3).unwrap());

        assert_eq!(modes.len(), ref_modes.len());
        for (a, b) in modes.iter().zip(ref_modes.iter()) {
            assert_eq!(a.0, b.0);
            assert_eq!(a.1, b.1);
            assert_eq!(a.2, b.2);
            assert!((a.3 - b.3).abs() < 1e-9);
        }
    }

    #[test]
    fn test_helmholtz_resonance() {
        let f0 = helmholtz_resonance(343.0, 0.01, 0.05, 2.0);
        let expected = (343.0 / (2.0 * std::f64::consts::PI)) * f64::sqrt(0.01 / (2.0 * 0.05));
        assert!((f0 - expected).abs() < 0.01);
    }

    #[test]
    fn test_rt60_sabine() {
        let rt60 = rt60_sabine(100.0, &[20.0, 30.0, 50.0], &[0.1, 0.2, 0.3]);
        let expected = 0.161 * 100.0 / (20.0 * 0.1 + 30.0 * 0.2 + 50.0 * 0.3);
        assert!((rt60 - expected).abs() < 1e-10);
    }

    #[test]
    fn test_critical_distance() {
        let rc = critical_distance(100.0, 0.5);
        assert!((rc - 0.057 * f64::sqrt(100.0 / 0.5)).abs() < 1e-10);
    }

    #[test]
    fn test_empty_absorption_infinite_rt60() {
        let rt60 = rt60_sabine(100.0, &[10.0], &[0.0]);
        assert!(rt60.is_infinite());
    }
}
