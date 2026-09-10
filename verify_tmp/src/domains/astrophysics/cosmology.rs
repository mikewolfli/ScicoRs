//! Cosmology: ΛCDM model, distances, NFW profile.

use crate::core::types::Scalar;

/// Scale factor a = 1/(1+z).
pub fn scale_factor(redshift: Scalar) -> Scalar {
    1.0 / (1.0 + redshift)
}

/// Hubble parameter H(z) for ΛCDM model.
pub fn hubble_parameter(redshift: Scalar, h0: Scalar, omega_m: Scalar, omega_l: Scalar) -> Scalar {
    let omega_k = 1.0 - omega_m - omega_l;
    h0 * ((omega_m * (1.0 + redshift).powi(3)
        + omega_k * (1.0 + redshift).powi(2)
        + omega_l)
        .sqrt())
}

/// Comoving distance (Mpc) in ΛCDM — simplified numerical integration.
///
/// d_C = (c/H₀)·∫₀ᶻ dz'/E(z'), where `dist = ∫ dz'/H(z')` already carries the
/// `1/H₀` factor (H(z') = H₀·E(z')). The result is therefore `c·dist` in Mpc
/// (with c in km/s and H₀ in km/s/Mpc) — no extra division is needed.
pub fn comoving_distance(redshift: Scalar, h0: Scalar, omega_m: Scalar, omega_l: Scalar) -> Scalar {
    let _h0 = h0; // H₀ is embedded in dist via hubble_parameter
    let steps = 1000;
    let dz = redshift / steps as Scalar;
    let mut dist = 0.0;
    for i in 0..steps {
        let z = (i as Scalar + 0.5) * dz;
        let hz = hubble_parameter(z, h0, omega_m, omega_l);
        dist += dz / hz;
    }
    let c_ms = 299792.458; // km/s
    c_ms * dist
}

/// Luminosity distance (Mpc): d_L = (1+z) * d_C.
pub fn luminosity_distance(redshift: Scalar, h0: Scalar, omega_m: Scalar, omega_l: Scalar) -> Scalar {
    let dc = comoving_distance(redshift, h0, omega_m, omega_l);
    (1.0 + redshift) * dc
}

/// Universe age (Gyr) for ΛCDM.
pub fn universe_age(h0: Scalar, omega_m: Scalar, omega_l: Scalar) -> Scalar {
    let steps = 10000;
    let z_max = 1000.0;
    let dz = z_max / steps as Scalar;
    let mut integral = 0.0;
    for i in 0..steps {
        let z = (i as Scalar + 0.5) * dz;
        let hz = hubble_parameter(z, h0, omega_m, omega_l);
        integral += dz / (hz * (1.0 + z));
    }
    // H₀ in 1/s: 1 Mpc = 3.085677581e19 km, so 1/s = h0 [km/s/Mpc] / 3.0857e19.
    let h0_s = h0 / (3.085677581e19); // km/s/Mpc → 1/s
    let age_s = integral / h0_s;
    age_s / (3.15576e16) // seconds → Gyr
}

/// Einstein radius for gravitational lensing (radians).
pub fn einstein_radius(lens_mass: Scalar, d_l: Scalar, d_s: Scalar, d_ls: Scalar) -> Scalar {
    if d_l <= 0.0 || d_s <= 0.0 { return 0.0; }
    let gm = 6.67430e-11 * lens_mass;
    let c2 = 299792458.0_f64.powi(2);
    ((4.0 * gm / c2) * (d_ls / (d_l * d_s))).sqrt()
}

/// NFW dark matter halo density profile.
pub fn nfw_profile(radius: Scalar, scale_radius: Scalar, rho0: Scalar) -> Scalar {
    if radius <= 0.0 || scale_radius <= 0.0 { return 0.0; }
    let x = radius / scale_radius;
    rho0 / (x * (1.0 + x).powi(2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scale_factor() {
        assert!((scale_factor(0.0) - 1.0).abs() < 1e-10);
        assert!((scale_factor(1.0) - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_hubble_parameter_z0() {
        let h = hubble_parameter(0.0, 70.0, 0.3, 0.7);
        assert!((h - 70.0).abs() < 1.0);
    }

    #[test]
    fn test_comoving_distance_positive() {
        let d = comoving_distance(0.1, 70.0, 0.3, 0.7);
        assert!(d > 0.0);
    }

    #[test]
    fn test_luminosity_distance() {
        let dl = luminosity_distance(0.1, 70.0, 0.3, 0.7);
        let dc = comoving_distance(0.1, 70.0, 0.3, 0.7);
        assert!(dl > dc);
    }

    #[test]
    fn test_universe_age_positive() {
        let age = universe_age(70.0, 0.3, 0.7);
        assert!(age > 0.0);
        assert!(age < 100.0);
    }

    #[test]
    fn test_nfw_profile() {
        let rho = nfw_profile(10.0, 20.0, 1.0);
        assert!(rho > 0.0);
    }

    #[test]
    fn test_einstein_radius() {
        let theta = einstein_radius(1.0e12 * 1.989e30, 1000.0, 2000.0, 1000.0);
        assert!(theta > 0.0);
    }
}
