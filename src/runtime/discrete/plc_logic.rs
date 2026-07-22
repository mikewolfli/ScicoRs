//! PLC (Programmable Logic Controller) primitives for discrete logic simulation.
//!
//! Provides basic logic gates, RS and D flip-flops, and edge detectors.

// ── Logic Gates ──────────────────────────────────────────────────────────────

/// Logical AND of all inputs.  Empty input returns `false`.
pub fn and_gate(inputs: &[bool]) -> bool {
    inputs.iter().all(|&x| x)
}

/// Logical OR of all inputs.  Empty input returns `false`.
pub fn or_gate(inputs: &[bool]) -> bool {
    inputs.iter().any(|&x| x)
}

/// Logical NOT of a single input.
pub fn not_gate(input: bool) -> bool {
    !input
}

/// Logical NAND (NOT AND) of all inputs.
pub fn nand_gate(inputs: &[bool]) -> bool {
    !and_gate(inputs)
}

/// Logical NOR (NOT OR) of all inputs.
pub fn nor_gate(inputs: &[bool]) -> bool {
    !or_gate(inputs)
}

/// Logical XOR of two inputs.
pub fn xor_gate(a: bool, b: bool) -> bool {
    a != b
}

// ── RS Flip-Flop ────────────────────────────────────────────────────────────

/// A Set/Reset flip-flop (SR latch).
///
/// Truth table:
/// | S | R | Q (next) |
/// |---|---|----------|
/// | 0 | 0 | keep     |
/// | 1 | 0 | 1        |
/// | 0 | 1 | 0        |
/// | 1 | 1 | invalid  |
#[derive(Debug, Clone)]
pub struct RSFlipFlop {
    /// Set input.
    pub set: bool,
    /// Reset input.
    pub reset: bool,
    /// Current output.
    pub output: bool,
}

impl RSFlipFlop {
    /// Create a new RS flip-flop with output initialised to `false`.
    pub fn new() -> Self {
        Self {
            set: false,
            reset: false,
            output: false,
        }
    }

    /// Evaluate the flip-flop based on the current `set` and `reset` inputs.
    ///
    /// When both are `true`, the output goes to `false` (invalid/priority
    /// reset) as a safe default.  When both are `false` the output holds
    /// its previous value.
    pub fn clock(&mut self) {
        if self.set && !self.reset {
            self.output = true;
        } else if self.reset {
            self.output = false;
        }
        // else: both false → hold (no change)
    }

    /// Statically evaluate the next output for given `set` and `reset` values.
    ///
    /// When both are `true`, returns `false` (reset-dominant behaviour).
    pub fn evaluate(set: bool, reset: bool) -> bool {
        if reset {
            false // reset-dominant; covers S=1,R=1 case
        } else if set {
            true
        } else {
            // keep — cannot determine statically without previous state;
            // callers who need keep semantics should use the `clock` method
            // on an instance.  For static eval we default to `false`.
            false
        }
    }
}

impl Default for RSFlipFlop {
    fn default() -> Self {
        Self::new()
    }
}

// ── D Flip-Flop ─────────────────────────────────────────────────────────────

/// A D-type flip-flop with clock enable.
#[derive(Debug, Clone)]
pub struct DFlipFlop {
    /// Data input.
    pub data: bool,
    /// Current output.
    pub output: bool,
    /// Clock enable: when `true`, the clock rising edge captures data.
    pub clock_enable: bool,
}

impl DFlipFlop {
    /// Create a new D flip-flop with output initialised to `false`.
    pub fn new() -> Self {
        Self {
            data: false,
            output: false,
            clock_enable: true,
        }
    }

    /// Clock the flip-flop, capturing the `data` input to the output.
    ///
    /// The capture only occurs when `clock_enable` is `true`.
    pub fn clock(&mut self, data: bool) {
        self.data = data;
        if self.clock_enable {
            self.output = data;
        }
    }
}

impl Default for DFlipFlop {
    fn default() -> Self {
        Self::new()
    }
}

// ── Edge Detector ───────────────────────────────────────────────────────────

/// Detects rising and falling edges of a boolean signal.
///
/// On each `update`, the current input is compared to the previous value
/// to determine whether a rising or falling edge occurred.
#[derive(Debug, Clone)]
pub struct EdgeDetector {
    /// Previous input value.
    pub last: bool,
    /// `true` if a rising edge (false → true) occurred in the last update.
    pub rising: bool,
    /// `true` if a falling edge (true → false) occurred in the last update.
    pub falling: bool,
}

impl EdgeDetector {
    /// Create a new edge detector. The initial 'last' value is `false`.
    pub fn new() -> Self {
        Self {
            last: false,
            rising: false,
            falling: false,
        }
    }

    /// Update the detector with a new input value.
    pub fn update(&mut self, input: bool) {
        self.rising = !self.last && input;
        self.falling = self.last && !input;
        self.last = input;
    }

    /// Returns `true` if a rising edge was detected on the last update.
    pub fn is_rising(&self) -> bool {
        self.rising
    }

    /// Returns `true` if a falling edge was detected on the last update.
    pub fn is_falling(&self) -> bool {
        self.falling
    }
}

impl Default for EdgeDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Gates ────────────────────────────────────────────────────────────

    #[test]
    fn and_gate_all_true() {
        assert!(and_gate(&[true, true, true]));
    }

    #[test]
    fn and_gate_one_false() {
        assert!(!and_gate(&[true, false, true]));
    }

    #[test]
    fn and_gate_empty() {
        // Vacuous truth: all elements of an empty set satisfy any predicate.
        assert!(and_gate(&[]));
    }

    #[test]
    fn or_gate_any_true() {
        assert!(or_gate(&[false, true, false]));
    }

    #[test]
    fn or_gate_all_false() {
        assert!(!or_gate(&[false, false]));
    }

    #[test]
    fn or_gate_empty() {
        assert!(!or_gate(&[]));
    }

    #[test]
    fn not_gate_true() {
        assert!(!not_gate(true));
    }

    #[test]
    fn not_gate_false() {
        assert!(not_gate(false));
    }

    #[test]
    fn nand_nor_xor() {
        assert!(!nand_gate(&[true, true]));
        assert!(nand_gate(&[true, false]));

        assert!(nor_gate(&[false, false]));
        assert!(!nor_gate(&[true, false]));

        assert!(!xor_gate(true, true));
        assert!(xor_gate(true, false));
        assert!(!xor_gate(false, false));
    }

    // ── RS Flip-Flop ─────────────────────────────────────────────────────

    #[test]
    fn rs_flipflop_set_reset() {
        let mut rs = RSFlipFlop::new();
        assert!(!rs.output);

        rs.set = true;
        rs.reset = false;
        rs.clock();
        assert!(rs.output);

        rs.set = false;
        rs.reset = true;
        rs.clock();
        assert!(!rs.output);
    }

    #[test]
    fn rs_flipflop_invalid() {
        let mut rs = RSFlipFlop::new();
        rs.set = true;
        rs.reset = true;
        rs.clock();
        // Reset-dominant: output should be false.
        assert!(!rs.output);
    }

    #[test]
    fn rs_flipflop_hold() {
        let mut rs = RSFlipFlop::new();
        rs.set = true;
        rs.clock();
        assert!(rs.output);
        // No change.
        rs.set = false;
        rs.reset = false;
        rs.clock();
        assert!(rs.output); // holds
    }

    #[test]
    fn rs_flipflop_static_eval() {
        assert!(RSFlipFlop::evaluate(true, false));
        assert!(!RSFlipFlop::evaluate(false, true));
        assert!(!RSFlipFlop::evaluate(true, true)); // reset-dominant
        assert!(!RSFlipFlop::evaluate(false, false)); // keep (defaults false)
    }

    // ── D Flip-Flop ──────────────────────────────────────────────────────

    #[test]
    fn d_flipflop_clock() {
        let mut dff = DFlipFlop::new();
        assert!(!dff.output);

        dff.clock(true);
        assert!(dff.output);

        dff.clock(false);
        assert!(!dff.output);
    }

    #[test]
    fn d_flipflop_clock_enable() {
        let mut dff = DFlipFlop::new();
        dff.clock_enable = false;
        dff.clock(true);
        assert!(!dff.output); // did not capture
    }

    #[test]
    fn d_flipflop_default() {
        let dff = DFlipFlop::default();
        assert!(!dff.output);
        assert!(dff.clock_enable);
    }

    // ── Edge Detector ────────────────────────────────────────────────────

    #[test]
    fn edge_detector_rising() {
        let mut ed = EdgeDetector::new();
        ed.update(true);
        assert!(ed.is_rising());
        assert!(!ed.is_falling());

        ed.update(true);
        assert!(!ed.is_rising());
        assert!(!ed.is_falling());
    }

    #[test]
    fn edge_detector_falling() {
        let mut ed = EdgeDetector::new();
        ed.update(true); // rising
        ed.update(false);
        assert!(ed.is_falling());
        assert!(!ed.is_rising());

        ed.update(false);
        assert!(!ed.is_rising());
        assert!(!ed.is_falling());
    }

    #[test]
    fn edge_detector_no_edge() {
        let mut ed = EdgeDetector::new();
        ed.update(false);
        assert!(!ed.is_rising());
        assert!(!ed.is_falling());
    }

    #[test]
    fn edge_detector_reset_state() {
        let ed = EdgeDetector::new();
        assert!(!ed.is_rising());
        assert!(!ed.is_falling());
        assert!(!ed.last);
    }
}
