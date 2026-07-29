//! Chemical reactor models.
//!
//! Provides Continuous Stirred-Tank Reactor (CSTR), Plug Flow Reactor (PFR),
//! and Batch Reactor models for chemical reaction engineering.
//!
//! # Reactor Types
//!
//! - **CSTR**: Steady-state and dynamic mass/energy balances
//! - **PFR**: Axial concentration profile via spatial discretization
//! - **BatchReactor**: Time-dependent batch conversion profile

use crate::core::types::Scalar;
use crate::domains::chemical::kinetics::ReactionKinetics;

// ──────────────────────────────────────────────
// Continuous Stirred-Tank Reactor (CSTR)
// ──────────────────────────────────────────────

/// Continuous Stirred-Tank Reactor model.
///
/// Assumes perfect mixing, uniform temperature and concentration
/// throughout the reactor volume.
pub struct Cstr {
    /// Reactor volume (m³).
    pub volume: Scalar,
    /// Volumetric flow rate into the reactor (m³/s).
    pub flow_rate_in: Scalar,
    /// Volumetric flow rate out of the reactor (m³/s).
    pub flow_rate_out: Scalar,
    /// Inlet concentrations for each species (mol/m³).
    pub inlet_concentrations: Vec<Scalar>,
    /// Overall heat transfer coefficient (W/(m²·K)).
    pub heat_transfer_coeff: Scalar,
    /// Heat transfer area (m²).
    pub heat_transfer_area: Scalar,
    /// Coolant temperature (K).
    pub coolant_temperature: Scalar,
}

impl Cstr {
    /// Create a new CSTR model.
    pub fn new(
        volume: Scalar,
        flow_rate_in: Scalar,
        flow_rate_out: Scalar,
        inlet_concentrations: Vec<Scalar>,
        heat_transfer_coeff: Scalar,
        heat_transfer_area: Scalar,
        coolant_temperature: Scalar,
    ) -> Self {
        Self {
            volume,
            flow_rate_in,
            flow_rate_out,
            inlet_concentrations,
            heat_transfer_coeff,
            heat_transfer_area,
            coolant_temperature,
        }
    }

    /// Compute mass balance dC/dt for each species at steady-state approximation.
    ///
    /// Accumulation = In - Out + Generation
    /// V·dCᵢ/dt = Q·Cᵢ,in - Q·Cᵢ,out + V·rᵢ
    pub fn mass_balance(
        &self,
        concentrations: &[Scalar],
        reaction: &ReactionKinetics,
    ) -> Vec<Scalar> {
        let n = concentrations.len();
        let mut balances = vec![0.0; n];
        let reaction_terms = reaction.concentration_derivatives(concentrations, 0.0);

        for i in 0..n {
            // Inlet contribution
            let inlet_term = if i < self.inlet_concentrations.len() {
                self.flow_rate_in * self.inlet_concentrations[i]
            } else {
                0.0
            };

            // Outlet + reaction
            let outlet_term = self.flow_rate_out * concentrations[i];
            balances[i] = (inlet_term - outlet_term) / self.volume + reaction_terms[i];
        }

        balances
    }

    /// Compute energy balance: dT/dt.
    ///
    /// ρ·Cp·V·dT/dt = Q·ρ·Cp·(T_in - T) + V·(-ΔH)·r + UA·(T_c - T)
    pub fn energy_balance(
        &self,
        t: Scalar,
        concentrations: &[Scalar],
        reaction: &ReactionKinetics,
        delta_h: Scalar,
    ) -> Scalar {
        // Reaction heat generation
        let reaction_terms = reaction.concentration_derivatives(concentrations, t);
        let heat_gen: Scalar = reaction_terms.iter().sum::<Scalar>() * (-delta_h);

        // Heat transfer
        let heat_transfer = self.heat_transfer_coeff * self.heat_transfer_area
            * (self.coolant_temperature - t);

        // Inlet-outlet enthalpy (simplified: assume ρCp constant)
        let flow_term = self.flow_rate_in * (self.inlet_concentrations.iter().sum::<Scalar>())
            - self.flow_rate_out * concentrations.iter().sum::<Scalar>();

        // Simplified energy balance (per unit volume)
        heat_gen + heat_transfer / self.volume + flow_term
    }

    /// Solve for steady-state concentrations and temperature.
    ///
    /// Uses a simple iteration to drive the mass balance to zero.
    pub fn steady_state(
        &self,
        reaction: &ReactionKinetics,
        t_guess: Scalar,
    ) -> Option<(Vec<Scalar>, Scalar)> {
        let n = self.inlet_concentrations.len();
        let mut conc: Vec<Scalar> = self.inlet_concentrations.clone();
        let t = t_guess;

        for _iter in 0..10_000 {
            let mb = self.mass_balance(&conc, reaction);
            let max_residual = mb.iter().map(|v| v.abs()).fold(0.0_f64, f64::max);

            // Update concentrations
            let dt = 0.1;
            for i in 0..n {
                conc[i] = (conc[i] + mb[i] * dt).max(0.0);
            }

            if max_residual < 1e-10 {
                return Some((conc, t));
            }
        }
        None
    }
}

// ──────────────────────────────────────────────
// Plug Flow Reactor (PFR)
// ──────────────────────────────────────────────

/// Plug Flow Reactor model.
///
/// Assumes no axial mixing, radial uniformity, and steady-state operation.
/// Uses spatial discretization along the reactor length.
pub struct Pfr {
    /// Reactor length (m).
    pub length: Scalar,
    /// Reactor diameter (m).
    pub diameter: Scalar,
    /// Linear flow velocity (m/s).
    pub flow_velocity: Scalar,
}

impl Pfr {
    /// Create a new PFR model.
    pub fn new(length: Scalar, diameter: Scalar, flow_velocity: Scalar) -> Self {
        Self {
            length,
            diameter,
            flow_velocity,
        }
    }

    /// Compute steady-state concentration profile along the reactor length.
    ///
    /// Uses Euler integration along the spatial coordinate z.
    /// Returns Vec of concentration vectors at each spatial step.
    pub fn profile(
        &self,
        inlet: &[Scalar],
        reaction: &ReactionKinetics,
    ) -> Vec<Vec<Scalar>> {
        let n_steps = 100;
        let dz = self.length / n_steps as Scalar;
        let residence_time_step = dz / self.flow_velocity;

        let mut results = Vec::with_capacity(n_steps + 1);
        let mut conc: Vec<Scalar> = inlet.to_vec();
        results.push(conc.clone());

        for _ in 0..n_steps {
            let derivs = reaction.concentration_derivatives(&conc, 0.0);
            for i in 0..conc.len() {
                let new_val = conc[i] + derivs[i] * residence_time_step;
                conc[i] = new_val.max(0.0);
            }
            results.push(conc.clone());
        }

        results
    }
}

// ──────────────────────────────────────────────
// Batch Reactor
// ──────────────────────────────────────────────

/// Batch Reactor model.
///
/// No inflow/outflow; composition changes only due to chemical reaction.
pub struct BatchReactor {
    /// Reactor volume (m³).
    pub volume: Scalar,
}

impl BatchReactor {
    /// Create a new Batch Reactor.
    pub fn new(volume: Scalar) -> Self {
        Self { volume }
    }

    /// Compute concentration vs time profile.
    ///
    /// Uses Euler integration from t=0 to t=t_end with step dt.
    /// Returns Vec of concentration vectors at each time step.
    pub fn batch_profile(
        &self,
        initial: &[Scalar],
        reaction: &ReactionKinetics,
        t_end: Scalar,
        dt: Scalar,
    ) -> Vec<Vec<Scalar>> {
        let n_steps = (t_end / dt).ceil() as usize;
        let mut results = Vec::with_capacity(n_steps + 1);
        let mut conc: Vec<Scalar> = initial.to_vec();
        let mut t = 0.0;

        results.push(conc.clone());

        for _ in 0..n_steps {
            reaction.step(&mut conc, dt, t);
            t += dt;
            results.push(conc.clone());
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_kinetics() -> ReactionKinetics {
        // A -> B, k=0.1
        ReactionKinetics::new(vec![0.1], vec![vec![-1.0, 1.0]])
    }

    #[test]
    fn test_cstr_mass_balance() {
        let cstr = Cstr::new(1.0, 0.1, 0.1, vec![1.0, 0.0], 0.0, 0.0, 300.0);
        let kinetics = simple_kinetics();
        let mb = cstr.mass_balance(&[0.5, 0.5], &kinetics);

        // At C_A = 0.5: In = 0.1*1.0 = 0.1, Out = 0.1*0.5 = 0.05, Reaction = -0.1*0.5 = -0.05
        // dC_A/dt = (0.1 - 0.05)/1.0 + (-0.05) = 0.0
        assert!((mb[0]).abs() < 1e-12);
    }

    #[test]
    fn test_cstr_steady_state() {
        let cstr = Cstr::new(1.0, 0.1, 0.1, vec![1.0, 0.0], 0.0, 0.0, 300.0);
        let kinetics = simple_kinetics();
        let result = cstr.steady_state(&kinetics, 300.0);
        assert!(result.is_some());
        let (conc, _t) = result.unwrap();
        // At steady state for A->B, C_A should be less than inlet
        assert!(conc[0] < 1.0);
        assert!(conc[1] > 0.0);
    }

    #[test]
    fn test_pfr_profile() {
        let pfr = Pfr::new(10.0, 0.1, 0.5);
        let kinetics = simple_kinetics();
        let profile = pfr.profile(&[1.0, 0.0], &kinetics);

        // Profile should be length 101 (100 steps + initial)
        assert_eq!(profile.len(), 101);
        // Conversion should increase along the reactor
        assert!(profile[0][0] >= profile[50][0]);
        assert!(profile[50][0] >= profile[100][0]);
        // Product should increase
        assert!(profile[100][1] > profile[0][1]);
    }

    #[test]
    fn test_batch_reactor() {
        let batch = BatchReactor::new(1.0);
        let kinetics = simple_kinetics();
        let profile = batch.batch_profile(&[1.0, 0.0], &kinetics, 10.0, 0.1);

        assert_eq!(profile.len(), 101);
        // After 10 seconds with k=0.1, A should be mostly consumed
        assert!(profile[100][0] < 0.5);
        assert!(profile[100][1] > 0.5);
    }

    #[test]
    fn test_cstr_energy_balance() {
        let cstr = Cstr::new(1.0, 0.1, 0.1, vec![1.0, 0.0], 100.0, 0.5, 350.0);
        let kinetics = simple_kinetics();
        let eb = cstr.energy_balance(300.0, &[0.5, 0.5], &kinetics, 50000.0);
        // Should return a finite value
        assert!(eb.is_finite());
    }
}
