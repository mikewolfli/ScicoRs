//! Neuron models: spike detection, leaky-integrate-and-fire, post-synaptic potential.

use crate::core::types::Scalar;

/// Leaky Integrate-and-Fire (LIF) neuron model.
pub struct NeuronModel {
    pub capacitance: Scalar,
    pub v_rest: Scalar,
    pub threshold: Scalar,
    pub refractory_period: Scalar,
    pub last_spike_time: Scalar,
    pub refractory_remaining: Scalar,
    /// Membrane potential deviation from rest (mV), tracked internally.
    pub v: Scalar,
    /// Current simulation time (s), advanced by each `lif_step` call.
    pub current_time: Scalar,
}

impl NeuronModel {
    /// Detect an action potential (threshold crossing).
    ///
    /// Returns `true` when V crosses the threshold from below.
    pub fn detect_spike(&self, v: Scalar, v_prev: Scalar) -> bool {
        v_prev < self.threshold && v >= self.threshold
    }

    /// Step the LIF model forward by `dt` with synaptic current `i_syn`.
    ///
    /// Returns `Some(spike_time)` if a spike occurred, otherwise `None`.
    ///
    /// dV/dt = (V_rest - V + i_syn * R) / τ, with τ = R·C_m = C_m (R ≈ 1).
    pub fn lif_step(&mut self, i_syn: Scalar, dt: Scalar) -> Option<Scalar> {
        // Advance the internal clock so reported spike times are real.
        self.current_time += dt;

        if self.refractory_remaining > 0.0 {
            // Neuron is in absolute refractory period
            self.refractory_remaining -= dt;
            self.v = self.v_rest;
            return None;
        }

        // Euler step: dV/dt = (v_rest - v + i_syn) / capacitance
        let dv = (self.v_rest - self.v + i_syn) / self.capacitance;
        let v_new = self.v + dv * dt;

        // Check for spike
        if self.detect_spike(v_new, self.v) {
            let spike_time = self.current_time;
            self.last_spike_time = spike_time;
            self.refractory_remaining = self.refractory_period;
            self.v = self.v_rest;
            Some(spike_time)
        } else {
            self.v = v_new;
            None
        }
    }

    /// Post-synaptic potential (exponential decay).
    ///
    /// PSP(t) = amplitude · exp(-t / τ) for t ≥ 0, 0 otherwise.
    pub fn psp_response(amplitude: Scalar, tau: Scalar, t: Scalar) -> Scalar {
        if t >= 0.0 {
            amplitude * (-t / tau).exp()
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_spike_threshold_crossing() {
        let n = NeuronModel {
            capacitance: 1.0,
            v_rest: -65.0,
            threshold: -55.0,
            refractory_period: 0.002,
            last_spike_time: 0.0,
            refractory_remaining: 0.0,
            v: -65.0,
            current_time: 0.0,
        };
        assert!(n.detect_spike(-50.0, -60.0));
        assert!(!n.detect_spike(-60.0, -65.0));
        assert!(!n.detect_spike(-50.0, -45.0)); // already above
    }

    #[test]
    fn test_lif_step_no_spike() {
        let mut n = NeuronModel {
            capacitance: 1.0,
            v_rest: -65.0,
            threshold: -55.0,
            refractory_period: 0.002,
            last_spike_time: 0.0,
            refractory_remaining: 0.0,
            v: -65.0,
            current_time: 0.0,
        };
        // Sub-threshold current
        let result = n.lif_step(5.0, 0.001);
        assert!(result.is_none());
        assert!(n.v > -65.0 && n.v < -55.0);
        assert!((n.current_time - 0.001).abs() < 1e-12);
    }

    #[test]
    fn test_lif_step_spike() {
        let mut n = NeuronModel {
            capacitance: 1.0,
            v_rest: -65.0,
            threshold: -55.0,
            refractory_period: 0.002,
            last_spike_time: 0.0,
            refractory_remaining: 0.0,
            v: -65.0,
            current_time: 0.0,
        };
        // Strong current should cause a spike
        let mut spiked = false;
        let mut spike_time = -1.0;
        for _ in 0..300 {
            if let Some(t) = n.lif_step(50.0, 0.001) {
                spiked = true;
                spike_time = t;
                break;
            }
        }
        assert!(spiked);
        // Spike time must be the real accumulated clock (> 0), not a stale 0.
        assert!(spike_time > 0.0);
        // After spike, neuron should be in refractory
        assert!(n.refractory_remaining > 0.0);
    }

    #[test]
    fn test_lif_step_refractory() {
        let mut n = NeuronModel {
            capacitance: 1.0,
            v_rest: -65.0,
            threshold: -55.0,
            refractory_period: 0.005,
            last_spike_time: 0.0,
            refractory_remaining: 0.005,
            v: -65.0,
            current_time: 0.0,
        };
        // During refractory, even strong current is ignored
        let result = n.lif_step(100.0, 0.001);
        assert!(result.is_none());
        assert!(n.refractory_remaining > 0.0);
    }

    #[test]
    fn test_psp_response_positive_t() {
        let psp = NeuronModel::psp_response(1.0, 0.01, 0.005);
        assert!(psp > 0.0 && psp < 1.0);
    }

    #[test]
    fn test_psp_response_at_zero() {
        let psp = NeuronModel::psp_response(1.0, 0.01, 0.0);
        assert!((psp - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_psp_response_negative_t() {
        let psp = NeuronModel::psp_response(1.0, 0.01, -1.0);
        assert!((psp).abs() < 1e-10);
    }

    #[test]
    fn test_psp_response_decay() {
        let psp1 = NeuronModel::psp_response(1.0, 0.01, 0.001);
        let psp2 = NeuronModel::psp_response(1.0, 0.01, 0.01);
        assert!(psp2 < psp1);
    }
}
