//! Physics Simulation Modules
//!
//! Contains domain-specific physics simulation building blocks:
//! structural mechanics, thermal, fluid dynamics, electromagnetics,
//! quantum, astrophysics, molecular dynamics, and more.

/// Common physical constants used across domains.
pub mod constants {
    use crate::core::types::Scalar;

    /// Speed of light in vacuum [m/s].
    pub const C: Scalar = 2.99792458e8;
    /// Gravitational constant [m³ kg⁻¹ s⁻²].
    pub const G: Scalar = 6.67430e-11;
    /// Planck constant [J·s].
    pub const H: Scalar = 6.62607015e-34;
    /// Reduced Planck constant [J·s].
    pub const HBAR: Scalar = H / (2.0 * std::f64::consts::PI);
    /// Boltzmann constant [J/K].
    pub const K_B: Scalar = 1.380649e-23;
    /// Elementary charge [C].
    pub const Q_E: Scalar = 1.602176634e-19;
    /// Electron rest mass [kg].
    pub const M_E: Scalar = 9.1093837015e-31;
    /// Proton rest mass [kg].
    pub const M_P: Scalar = 1.67262192369e-27;
    /// Neutron rest mass [kg].
    pub const M_N: Scalar = 1.67492749804e-27;
    /// Vacuum permittivity [F/m].
    pub const EPSILON_0: Scalar = 8.854187817e-12;
    /// Vacuum permeability [H/m].
    pub const MU_0: Scalar = 1.25663706127e-6;
    /// Avogadro number [mol⁻¹].
    pub const N_A: Scalar = 6.02214076e23;
    /// Standard gravity [m/s²].
    pub const G_STD: Scalar = 9.80665;
    /// Standard atmosphere [Pa].
    pub const ATM: Scalar = 101_325.0;
}

/// Structural mechanics and FEA domain.
pub mod structural {
    use crate::core::types::Scalar;

    /// Stress state at a point.
    #[derive(Debug, Clone)]
    pub struct StressState {
        pub sigma_xx: Scalar,
        pub sigma_yy: Scalar,
        pub sigma_zz: Scalar,
        pub tau_xy: Scalar,
        pub tau_yz: Scalar,
        pub tau_zx: Scalar,
    }

    /// Strain state at a point.
    #[derive(Debug, Clone)]
    pub struct StrainState {
        pub epsilon_xx: Scalar,
        pub epsilon_yy: Scalar,
        pub epsilon_zz: Scalar,
        pub gamma_xy: Scalar,
        pub gamma_yz: Scalar,
        pub gamma_zx: Scalar,
    }

    /// Von Mises equivalent stress.
    pub fn von_mises_stress(s: &StressState) -> Scalar {
        let s1 = s.sigma_xx - s.sigma_yy;
        let s2 = s.sigma_yy - s.sigma_zz;
        let s3 = s.sigma_zz - s.sigma_xx;
        (0.5 * (s1 * s1
            + s2 * s2
            + s3 * s3
            + 6.0 * (s.tau_xy * s.tau_xy + s.tau_yz * s.tau_yz + s.tau_zx * s.tau_zx)))
            .sqrt()
    }

    /// Hooke's law for isotropic linear elasticity.
    pub fn hookes_law(
        strain: &StrainState,
        young_modulus: Scalar,
        poisson_ratio: Scalar,
    ) -> StressState {
        let e = young_modulus;
        let nu = poisson_ratio;
        let lambda = e * nu / ((1.0 + nu) * (1.0 - 2.0 * nu));
        let mu = e / (2.0 * (1.0 + nu));
        let theta = strain.epsilon_xx + strain.epsilon_yy + strain.epsilon_zz;

        StressState {
            sigma_xx: lambda * theta + 2.0 * mu * strain.epsilon_xx,
            sigma_yy: lambda * theta + 2.0 * mu * strain.epsilon_yy,
            sigma_zz: lambda * theta + 2.0 * mu * strain.epsilon_zz,
            tau_xy: mu * strain.gamma_xy,
            tau_yz: mu * strain.gamma_yz,
            tau_zx: mu * strain.gamma_zx,
        }
    }
}

/// Thermal and heat transfer domain.
pub mod thermal {
    use crate::core::types::Scalar;

    /// 1D steady-state conduction heat flux [W/m²].
    pub fn conduction_flux_1d(t1: Scalar, t2: Scalar, k: Scalar, dx: Scalar) -> Scalar {
        if dx == 0.0 { 0.0 } else { -k * (t2 - t1) / dx }
    }

    /// Convective heat flux [W/m²].
    pub fn convective_flux(t_surface: Scalar, t_fluid: Scalar, h: Scalar) -> Scalar {
        h * (t_surface - t_fluid)
    }

    /// Radiative heat flux [W/m²].
    pub fn radiative_flux(t_surface: Scalar, t_surroundings: Scalar, emissivity: Scalar) -> Scalar {
        let sigma = 5.670374419e-8; // Stefan-Boltzmann constant
        emissivity * sigma * (t_surface.powi(4) - t_surroundings.powi(4))
    }
}

/// Fluid dynamics (CFD) domain.
pub mod fluid {
    use crate::core::types::Scalar;

    /// Reynolds number.
    pub fn reynolds_number(
        density: Scalar,
        velocity: Scalar,
        char_length: Scalar,
        viscosity: Scalar,
    ) -> Scalar {
        if viscosity == 0.0 {
            0.0
        } else {
            density * velocity * char_length / viscosity
        }
    }

    /// Dynamic pressure.
    pub fn dynamic_pressure(density: Scalar, velocity: Scalar) -> Scalar {
        0.5 * density * velocity * velocity
    }
}

/// Molecular dynamics domain.
pub mod molecular {
    use crate::core::types::Scalar;

    /// Lennard-Jones potential energy.
    pub fn lennard_jones_potential(r: Scalar, epsilon: Scalar, sigma: Scalar) -> Scalar {
        if r == 0.0 {
            return 0.0;
        }
        let sr = sigma / r;
        let sr6 = sr.powi(6);
        let sr12 = sr6 * sr6;
        4.0 * epsilon * (sr12 - sr6)
    }

    /// Lennard-Jones force magnitude.
    pub fn lennard_jones_force(r: Scalar, epsilon: Scalar, sigma: Scalar) -> Scalar {
        if r == 0.0 {
            return 0.0;
        }
        let sr = sigma / r;
        let sr6 = sr.powi(6);
        24.0 * epsilon * (2.0 * sr6 * sr6 - sr6) / r
    }
}

/// Quantum physics domain.
pub mod quantum {
    use crate::core::types::Scalar;

    /// Energy of a photon: E = h * nu
    pub fn photon_energy(frequency: Scalar) -> Scalar {
        super::constants::H * frequency
    }

    /// De Broglie wavelength: lambda = h / p
    pub fn de_broglie_wavelength(momentum: Scalar) -> Scalar {
        if momentum == 0.0 {
            0.0
        } else {
            super::constants::H / momentum
        }
    }

    /// Bohr radius [m].
    pub const BOHR_RADIUS: Scalar = 5.29177210903e-11;

    /// Ground state energy of hydrogen atom [J].
    pub const HYDROGEN_GS_ENERGY: Scalar = -13.6 * 1.602176634e-19;
}

/// Astrophysics domain.
pub mod astrophysics {
    use crate::core::types::Scalar;

    /// Newtonian gravitational force between two bodies.
    pub fn gravitational_force(m1: Scalar, m2: Scalar, r: Scalar) -> Scalar {
        if r == 0.0 {
            0.0
        } else {
            super::constants::G * m1 * m2 / (r * r)
        }
    }

    /// Orbital velocity for a circular orbit.
    pub fn orbital_velocity(central_mass: Scalar, radius: Scalar) -> Scalar {
        if radius == 0.0 {
            0.0
        } else {
            (super::constants::G * central_mass / radius).sqrt()
        }
    }

    /// Schwarzschild radius.
    pub fn schwarzschild_radius(mass: Scalar) -> Scalar {
        2.0 * super::constants::G * mass / (super::constants::C * super::constants::C)
    }
}
