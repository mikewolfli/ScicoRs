//! Quantum error correction codes and decoding.
//!
//! Implements several quantum error correction codes:
//! - [[3,1,3]] Repetition code (bit-flip only)
//! - [[7,1,3]] Steane code (CSS code)
//! - [[9,1,3]] Shor code (9-qubit code)
//! - Surface code with adjustable distance

use super::state::ComplexScalar;
use crate::core::types::Scalar;
use std::sync::atomic::{AtomicU64, Ordering};

/// Generate a deterministic pseudo-random f64 in [0, 1) using xorshift64*.
fn fast_rand() -> Scalar {
    static STATE: AtomicU64 = AtomicU64::new(123456789);
    let mut x = STATE.load(Ordering::Relaxed);
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    STATE.store(x, Ordering::Relaxed);
    (x as f64) * (1.0 / 18446744073709551615.0_f64)
}

/// Supported quantum error correction codes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QuantumCode {
    /// [[3,1,3]] Bit-flip repetition code.
    Repetition3,
    /// [[7,1,3]] Steane CSS code.
    Steane7,
    /// [[9,1,3]] Shor 9-qubit code.
    Shor9,
    /// Surface code with code distance `d`.
    SurfaceCode { d: usize },
}

/// Logical qubit encoded in a quantum error correction code.
#[derive(Debug, Clone)]
pub struct LogicalQubit {
    /// Physical qubit state vector (amplitudes for computational basis states).
    pub amplitudes: Vec<ComplexScalar>,
    /// Number of physical qubits.
    pub n_physical: usize,
    /// The code used.
    pub code: QuantumCode,
    /// Measured syndrome (error signatures).
    pub syndrome: Vec<usize>,
}

impl LogicalQubit {
    /// Create a new logical qubit in the |0_L⟩ state.
    pub fn new(code: QuantumCode) -> Self {
        let (n_physical, amplitudes) = match code {
            QuantumCode::Repetition3 => {
                // |0_L⟩ = |000⟩, |1_L⟩ = |111⟩
                let mut amp = vec![ComplexScalar::new(0.0, 0.0); 8];
                amp[0] = ComplexScalar::new(1.0, 0.0); // |000⟩
                (3, amp)
            }
            QuantumCode::Steane7 => {
                // |0_L⟩ = 1/√8 Σ_{even weight codewords}
                let mut amp = vec![ComplexScalar::new(0.0, 0.0); 128];
                let codewords: [usize; 16] = [
                    0b0000000, 0b0001111, 0b0110011, 0b0111100, 0b1010101, 0b1011010, 0b1100110,
                    0b1101001, 0b1111111, 0b1110000, 0b1001100, 0b1000011, 0b0101010, 0b0100101,
                    0b0011001, 0b0010110,
                ];
                let norm = (codewords.len() as Scalar).sqrt().recip();
                for &cw in &codewords {
                    amp[cw] = ComplexScalar::new(norm, 0.0);
                }
                (7, amp)
            }
            QuantumCode::Shor9 => {
                // |0_L⟩ = (|000⟩+|111⟩)³ / 2√2
                let mut amp = vec![ComplexScalar::new(0.0, 0.0); 512];
                let norm = (8.0_f64).sqrt().recip();
                // All basis states with even parity in each block of 3
                for b0 in [0b000, 0b111] {
                    for b1 in [0b000, 0b111] {
                        for b2 in [0b000, 0b111] {
                            let idx = (b2 << 6) | (b1 << 3) | b0;
                            amp[idx] = ComplexScalar::new(norm, 0.0);
                        }
                    }
                }
                (9, amp)
            }
            QuantumCode::SurfaceCode { d } => {
                let n = d * d;
                let mut amp = vec![ComplexScalar::new(0.0, 0.0); 1 << n];
                amp[0] = ComplexScalar::new(1.0, 0.0);
                (n, amp)
            }
        };
        Self {
            amplitudes,
            n_physical,
            code,
            syndrome: Vec::new(),
        }
    }

    /// Encode a single physical qubit state into the logical qubit.
    ///
    /// The physical state [α, β] is encoded as α·|0_L⟩ + β·|1_L⟩.
    pub fn encode(&self, alpha: ComplexScalar, beta: ComplexScalar) -> Result<Self, String> {
        let mut logical = Self::new(self.code);
        match self.code {
            QuantumCode::Repetition3 => {
                // |0_L⟩ = |000⟩, |1_L⟩ = |111⟩
                logical.amplitudes[0] = alpha; // |000⟩
                logical.amplitudes[7] = beta; // |111⟩
            }
            QuantumCode::Steane7 => {
                // |0_L⟩ and |1_L⟩ both supported
                let zero_state = Self::new(QuantumCode::Steane7);
                let one_state = self.steane_one_state();
                for i in 0..logical.amplitudes.len() {
                    logical.amplitudes[i] =
                        alpha * zero_state.amplitudes[i] + beta * one_state.amplitudes[i];
                }
            }
            QuantumCode::Shor9 => {
                let zero_amp = Self::new(QuantumCode::Shor9);
                let one_amp = self.shor_one_state();
                for i in 0..logical.amplitudes.len() {
                    logical.amplitudes[i] =
                        alpha * zero_amp.amplitudes[i] + beta * one_amp.amplitudes[i];
                }
            }
            QuantumCode::SurfaceCode { d } => {
                // Repetition-style encoding over the d² physical qubits:
                // |0_L⟩ = |0...0⟩, |1_L⟩ = |1...1⟩. A full topological
                // surface-code encoding is out of scope.
                let n = d * d;
                logical.amplitudes[0] = alpha;
                logical.amplitudes[(1 << n) - 1] = beta;
            }
        }
        Ok(logical)
    }

    /// Construct the |1_L⟩ state for Steane code.
    fn steane_one_state(&self) -> Self {
        let mut logical = Self::new(QuantumCode::Steane7);
        // |1_L⟩ = X⊗7 |0_L⟩ (bitwise complement)
        let zero_state = Self::new(QuantumCode::Steane7);
        for i in 0..zero_state.amplitudes.len() {
            if zero_state.amplitudes[i].norm_sqr() > 1e-15 {
                let flipped = (!i) & 0x7F;
                logical.amplitudes[flipped] = zero_state.amplitudes[i];
            }
        }
        logical
    }

    /// Construct the |1_L⟩ state for Shor code.
    fn shor_one_state(&self) -> Self {
        let mut logical = Self::new(QuantumCode::Shor9);
        // |1_L⟩ = (|000⟩−|111⟩)³ / 2√2
        let norm = (8.0_f64).sqrt().recip();
        for &b0 in [0usize, 0b111].iter() {
            for &b1 in [0usize, 0b111].iter() {
                for &b2 in [0usize, 0b111].iter() {
                    let idx = (b2 << 6) | (b1 << 3) | b0;
                    let parity = (b0.count_ones() + b1.count_ones() + b2.count_ones()) % 2;
                    let sign = if parity == 0 { 1.0 } else { -1.0 };
                    logical.amplitudes[idx] = ComplexScalar::new(norm * sign, 0.0);
                }
            }
        }
        logical
    }

    /// Detect errors by measuring stabilizer generators (syndrome extraction).
    ///
    /// Returns a vector of syndrome bit values.
    pub fn detect_error(&self) -> Vec<usize> {
        match self.code {
            QuantumCode::Repetition3 => {
                // Two stabilizers: Z⊗Z⊗I, I⊗Z⊗Z
                let mut syndrome = Vec::new();
                // Z·Z·I: (parity of qubits 0 and 1)
                let mut s0 = 0;
                for i in 0..self.amplitudes.len() {
                    if self.amplitudes[i].norm_sqr() > 1e-15 {
                        let q0 = (i >> 2) & 1;
                        let q1 = (i >> 1) & 1;
                        let zz = if q0 == q1 { 0 } else { 1 };
                        s0 |= zz;
                    }
                }
                syndrome.push(s0);
                // I·Z·Z: (parity of qubits 1 and 2)
                let mut s1 = 0;
                for i in 0..self.amplitudes.len() {
                    if self.amplitudes[i].norm_sqr() > 1e-15 {
                        let q1 = (i >> 1) & 1;
                        let q2 = i & 1;
                        let zz = if q1 == q2 { 0 } else { 1 };
                        s1 |= zz;
                    }
                }
                syndrome.push(s1);
                syndrome
            }
            QuantumCode::Steane7 => {
                // [[7,1,3]] Steane code. Six stabilizers:
                //  - bits 0..3: Z-type checks (Z-parity over qubit sets),
                //    detecting single-qubit X errors;
                //  - bits 3..6: X-type checks (measured via the overlap
                //    Re(⟨ψ|S|ψ⟩)), detecting single-qubit Z errors.
                let mut syndrome = vec![0; 6];
                // Z-checks are measured in the direct bit convention (bit p =
                // qubit p), matching the codeword representation.
                let z_stabs: [usize; 3] = [0b0001111, 0b0110011, 0b1010101];
                for (si, &stab) in z_stabs.iter().enumerate() {
                    for i in 0..self.amplitudes.len() {
                        if self.amplitudes[i].norm_sqr() > 1e-15 {
                            let parity = (i & stab).count_ones() % 2;
                            syndrome[si] |= parity as usize;
                        }
                    }
                }
                // X-type stabilizers: qubit sets whose bit_pos masks are the
                // valid code-preserving patterns {0,1,2,3},{0,1,4,5},{0,2,4,6}
                // (i.e. masks 0b0001111, 0b0110011, 0b1010101 in the bit_pos
                // convention). These detect single-qubit Z errors.
                let x_groups: [&[usize]; 3] = [&[6, 5, 4, 3], &[6, 5, 2, 1], &[6, 4, 2, 0]];
                for (si, &group) in x_groups.iter().enumerate() {
                    syndrome[3 + si] = self.x_stabilizer_bit(group);
                }
                syndrome
            }
            QuantumCode::Shor9 => {
                // 8 stabilizer generators of the [[9,1,3]] Shor code:
                //   6 Z-stabilizers inside each 3-qubit block (bit-flip detection),
                //   2 X-stabilizers between blocks (phase-flip detection).
                let mut syndrome = vec![0; 8];
                // Z-type: parity of the two physical qubits (0 = same, 1 = differ).
                let z_pairs: [(usize, usize); 6] = [(0, 1), (1, 2), (3, 4), (4, 5), (6, 7), (7, 8)];
                for (si, &(qa, qb)) in z_pairs.iter().enumerate() {
                    let pa = self.bit_pos(qa);
                    let pb = self.bit_pos(qb);
                    for i in 0..self.amplitudes.len() {
                        if self.amplitudes[i].norm_sqr() > 1e-15 {
                            let ba = (i >> pa) & 1;
                            let bb = (i >> pb) & 1;
                            if ba != bb {
                                syndrome[si] = 1;
                                break;
                            }
                        }
                    }
                }
                // X-type: measured via the overlap <ψ|S|ψ> (eigenvalue ±1).
                syndrome[6] = self.x_stabilizer_bit(&[0, 1, 2, 3, 4, 5]);
                syndrome[7] = self.x_stabilizer_bit(&[3, 4, 5, 6, 7, 8]);
                syndrome
            }
            QuantumCode::SurfaceCode { d } => {
                // Simplified repetition decoder over the d² physical qubits
                // (consistent with `encode`). A full topological surface-code
                // decoder is out of scope; this detects a single bit-flip on
                // any physical qubit via Z-parity checks between neighbours.
                let n = d * d;
                let mut syndrome = vec![0; n.saturating_sub(1)];
                for (si, qa) in (0..n.saturating_sub(1)).enumerate() {
                    let pa = self.bit_pos(qa);
                    let pb = self.bit_pos(qa + 1);
                    for i in 0..self.amplitudes.len() {
                        if self.amplitudes[i].norm_sqr() > 1e-15 {
                            let ba = (i >> pa) & 1;
                            let bb = (i >> pb) & 1;
                            if ba != bb {
                                syndrome[si] = 1;
                                break;
                            }
                        }
                    }
                }
                syndrome
            }
        }
    }

    /// Correct errors given a syndrome.
    pub fn correct(&mut self, syndrome: &[usize]) -> Result<(), String> {
        if syndrome.is_empty() {
            return Ok(());
        }
        match self.code {
            QuantumCode::Repetition3 => {
                if syndrome.len() >= 2 {
                    let error_pos = if syndrome[0] == 0 && syndrome[1] == 1 {
                        2 // qubit 2 flipped
                    } else if syndrome[0] == 1 && syndrome[1] == 0 {
                        0 // qubit 0 flipped
                    } else if syndrome[0] == 1 && syndrome[1] == 1 {
                        1 // qubit 1 flipped
                    } else {
                        return Ok(()); // No error
                    };
                    self.apply_x(error_pos);
                }
            }
            QuantumCode::Shor9 => {
                self.correct_shor9(syndrome)?;
            }
            QuantumCode::Steane7 => {
                if syndrome.len() < 6 {
                    return Err("Steane: expected 6 syndrome bits".to_string());
                }
                // Bits 0..3 are Z-checks over the qubit sets
                // {0,1,2,3},{0,1,4,5},{0,2,4,6} measured in the *direct* bit
                // convention, while apply_x flips bit (6−q). The resulting
                // syndrome→qubit map for X errors is:
                //   {1→3, 2→1, 3→5, 4→0, 5→4, 6→2, 7→6}.
                let decode_x = |sy: &[usize]| -> Result<Option<usize>, String> {
                    let v = sy[0] + 2 * sy[1] + 4 * sy[2];
                    Ok(match v {
                        0 => None,
                        1 => Some(3),
                        2 => Some(1),
                        3 => Some(5),
                        4 => Some(0),
                        5 => Some(4),
                        6 => Some(2),
                        7 => Some(6),
                        _ => return Err("Steane: invalid X-syndrome bits".to_string()),
                    })
                };
                // Bits 3..6 are X-checks on the qubit sets
                // {6,5,4,3},{6,5,2,1},{6,4,2,0} (measured via the overlap
                // method in the bit_pos convention); a Z error on qubit p
                // anticommutes with the X-stabilizer containing p, giving the
                // same mapping as decode_x: {1→3, 2→1, 3→5, 4→0, 5→4, 6→2, 7→6}.
                let decode_z = |sy: &[usize]| -> Result<Option<usize>, String> {
                    let v = sy[0] + 2 * sy[1] + 4 * sy[2];
                    Ok(match v {
                        0 => None,
                        1 => Some(3),
                        2 => Some(1),
                        3 => Some(5),
                        4 => Some(0),
                        5 => Some(4),
                        6 => Some(2),
                        7 => Some(6),
                        _ => return Err("Steane: invalid Z-syndrome bits".to_string()),
                    })
                };
                if let Some(p) = decode_x(&syndrome[0..3])? {
                    self.apply_x(p);
                }
                if let Some(p) = decode_z(&syndrome[3..6])? {
                    self.apply_z(p);
                }
            }
            QuantumCode::SurfaceCode { d } => {
                let n = d * d;
                // Repetition decoder: for a single bit-flip X_p the syndrome
                // has 1s at indices (p-1, p) (or just s0 for p=0, or s_{n-2}
                // for p=n-1). Decode via the first 1 position.
                if syndrome.len() != n.saturating_sub(1) {
                    return Err("surface code: syndrome length mismatch".to_string());
                }
                match syndrome.iter().position(|&s| s == 1) {
                    None => { /* no error */ }
                    Some(0) => {
                        let run = syndrome.iter().take_while(|&&s| s == 1).count();
                        self.apply_x(run - 1);
                    }
                    Some(i) => self.apply_x(i + 1),
                }
            }
        }
        self.syndrome = syndrome.to_vec();
        Ok(())
    }

    /// Apply a bit-flip (Pauli X) on physical qubit `q`.
    fn apply_x(&mut self, q: usize) {
        let pos = self.bit_pos(q);
        let n = self.amplitudes.len();
        let mut new_amps = self.amplitudes.clone();
        // X_q is an involution: swap amplitude pairs (i, i ^ (1<<pos)).
        for i in 0..n {
            let flipped = i ^ (1 << pos);
            if flipped < i {
                continue; // each pair processed once
            }
            new_amps.swap(i, flipped);
        }
        self.amplitudes = new_amps;
    }

    /// Apply a phase-flip (Pauli Z) on physical qubit `q`.
    fn apply_z(&mut self, q: usize) {
        let pos = self.bit_pos(q);
        for (i, amp) in self.amplitudes.iter_mut().enumerate() {
            if (i >> pos) & 1 == 1 {
                *amp = -(*amp);
            }
        }
    }

    /// Bit index of physical qubit `q` within the state index.
    fn bit_pos(&self, q: usize) -> usize {
        self.n_physical.saturating_sub(1).saturating_sub(q)
    }

    /// Measure an X-type stabiliser S via the overlap Re(⟨ψ|S|ψ⟩).
    /// Returns 1 when the eigenvalue is -1 (stabilizer violated), else 0.
    fn x_stabilizer_bit(&self, qubits: &[usize]) -> usize {
        let mut mask = 0usize;
        for &q in qubits {
            mask |= 1 << self.bit_pos(q);
        }
        let n = self.amplitudes.len();
        let mut overlap: Scalar = 0.0;
        for i in 0..n {
            let a = self.amplitudes[i];
            if a.norm_sqr() < 1e-30 {
                continue;
            }
            let j = i ^ mask;
            if j >= n {
                continue;
            }
            let b = self.amplitudes[j];
            // Real part of conj(a) * b.
            overlap += a.conj().re * b.re + a.conj().im * b.im;
        }
        if overlap < 0.0 { 1 } else { 0 }
    }

    /// Decode and correct a Shor-code syndrome (8 bits).
    fn correct_shor9(&mut self, syndrome: &[usize]) -> Result<(), String> {
        if syndrome.len() < 8 {
            return Err(format!(
                "shor code: expected 8 syndrome bits, got {}",
                syndrome.len()
            ));
        }
        // ── Bit-flip correction from the 6 Z-syndromes ──
        // Block b (qubits 3b..3b+2) uses syndrome bits (2b, 2b+1):
        //   (0,0)->none, (1,0)->first, (1,1)->middle, (0,1)->last.
        for b in 0..3 {
            let (s_first, s_second) = (syndrome[2 * b], syndrome[2 * b + 1]);
            let qubit = match (s_first, s_second) {
                (0, 0) => None,
                (1, 0) => Some(3 * b),
                (1, 1) => Some(3 * b + 1),
                (0, 1) => Some(3 * b + 2),
                _ => unreachable!("syndrome bits are 0 or 1"),
            };
            if let Some(q) = qubit {
                self.apply_x(q);
            }
        }
        // ── Phase-flip correction from the 2 X-syndromes ──
        // (0,0)->none, (1,0)->block 1, (1,1)->block 2, (0,1)->block 3.
        // Within a block all three single-qubit Z operators are equivalent on
        // the codespace, so we apply Z to the first qubit of the block.
        let (x_first, x_second) = (syndrome[6], syndrome[7]);
        let block = match (x_first, x_second) {
            (0, 0) => None,
            (1, 0) => Some(0),
            (1, 1) => Some(1),
            (0, 1) => Some(2),
            _ => unreachable!("syndrome bits are 0 or 1"),
        };
        if let Some(b) = block {
            self.apply_z(3 * b);
        }
        Ok(())
    }

    /// Estimate logical error rate from physical error rate via Monte Carlo.
    pub fn logical_error_rate(&self, physical_error_rate: Scalar, n_rounds: usize) -> Scalar {
        let mut n_errors = 0;
        for _ in 0..n_rounds {
            // Simplified: binomial model for code distance
            let d = match self.code {
                QuantumCode::Repetition3 => 3,
                QuantumCode::Steane7 => 3,
                QuantumCode::Shor9 => 3,
                QuantumCode::SurfaceCode { d } => d,
            };
            let t = (d - 1) / 2; // Error correction capability
            // Probability of ≥ t+1 errors
            let mut prob = 0.0;
            for k in (t + 1)..=d {
                let binom = (0..k).fold(1.0, |acc, i| acc * (d - i) as Scalar / (i + 1) as Scalar);
                prob += binom
                    * physical_error_rate.powi(k as i32)
                    * (1.0 - physical_error_rate).powi((d - k) as i32);
            }
            if fast_rand() < prob {
                n_errors += 1;
            }
        }
        n_errors as Scalar / n_rounds.max(1) as Scalar
    }

    /// Measure all qubits in the Z basis (collapse to |0⟩ or |1⟩).
    pub fn measure(&self) -> Vec<usize> {
        let mut result = Vec::with_capacity(self.n_physical);
        for q in 0..self.n_physical {
            let mut p0 = 0.0;
            for i in 0..self.amplitudes.len() {
                let bit = (i >> (self.n_physical - 1 - q)) & 1;
                if bit == 0 {
                    p0 += self.amplitudes[i].norm_sqr();
                }
            }
            let outcome = if fast_rand() < p0 { 0 } else { 1 };
            result.push(outcome);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repetition_code_new() {
        let lq = LogicalQubit::new(QuantumCode::Repetition3);
        assert_eq!(lq.n_physical, 3);
        assert!((lq.amplitudes[0].norm_sqr() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_steane_code_new() {
        let lq = LogicalQubit::new(QuantumCode::Steane7);
        assert_eq!(lq.n_physical, 7);
        // Should be normalised
        let norm: Scalar = lq.amplitudes.iter().map(|a| a.norm_sqr()).sum();
        assert!((norm - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_shor_code_new() {
        let lq = LogicalQubit::new(QuantumCode::Shor9);
        assert_eq!(lq.n_physical, 9);
        let norm: Scalar = lq.amplitudes.iter().map(|a| a.norm_sqr()).sum();
        assert!((norm - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_surface_code_new() {
        let lq = LogicalQubit::new(QuantumCode::SurfaceCode { d: 3 });
        assert_eq!(lq.n_physical, 9);
    }

    #[test]
    fn test_repetition_encode() {
        let lq = LogicalQubit::new(QuantumCode::Repetition3);
        let encoded = lq.encode(1.0.into(), 0.0.into()).unwrap();
        // Should be |000⟩
        assert!((encoded.amplitudes[0].norm_sqr() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_repetition_syndrome_no_error() {
        let lq = LogicalQubit::new(QuantumCode::Repetition3);
        let synd = lq.detect_error();
        assert_eq!(synd, vec![0, 0]);
    }

    #[test]
    fn test_repetition_correct_bit_flip() {
        let mut lq = LogicalQubit::new(QuantumCode::Repetition3);
        // Inject error: flip qubit 2 (LSB in our representation)
        lq.amplitudes[0] = ComplexScalar::new(0.0, 0.0);
        lq.amplitudes[1] = ComplexScalar::new(1.0, 0.0); // |001⟩ = LSB flipped
        let synd = lq.detect_error();
        lq.correct(&synd).unwrap();
        // Should recover |000⟩
        assert!((lq.amplitudes[0].norm_sqr() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_logical_error_rate() {
        let lq = LogicalQubit::new(QuantumCode::Repetition3);
        let ler = lq.logical_error_rate(0.01, 1000);
        assert!(ler >= 0.0 && ler <= 1.0);
    }

    #[test]
    fn test_shor_syndrome_no_error() {
        let lq = LogicalQubit::new(QuantumCode::Shor9);
        let synd = lq.detect_error();
        assert_eq!(synd.len(), 8);
        assert_eq!(synd, vec![0; 8]);
    }

    #[test]
    fn test_shor_correct_bit_flip() {
        // Encode |0_L>, inject a bit-flip on qubit 4, then decode and correct.
        let lq = LogicalQubit::new(QuantumCode::Shor9);
        let mut encoded = lq.encode(1.0.into(), 0.0.into()).unwrap();
        // Inject X_4: flip physical qubit 4 (bit position n-1-4 = 4).
        encoded.apply_x(4);
        let synd = encoded.detect_error();
        // Bit flip on qubit 4 (middle of block 2) -> Z-syndromes 2 and 3 set.
        assert_eq!(synd[2], 1);
        assert_eq!(synd[3], 1);
        assert_eq!(synd[6], 0);
        assert_eq!(synd[7], 0);
        encoded.correct(&synd).unwrap();
        // Should recover |0_L>.
        let zero = lq.encode(1.0.into(), 0.0.into()).unwrap();
        let mut fid = 0.0;
        for (a, b) in encoded.amplitudes.iter().zip(zero.amplitudes.iter()) {
            fid += (a.conj() * b).re;
        }
        assert!(fid > 0.9999);
    }

    #[test]
    fn test_shor_correct_phase_flip() {
        // Inject a phase-flip (Z_0) on the first qubit of block 1.
        let lq = LogicalQubit::new(QuantumCode::Shor9);
        let mut encoded = lq.encode(1.0.into(), 0.0.into()).unwrap();
        encoded.apply_z(0);
        let synd = encoded.detect_error();
        // Z_0 anticommutes with the first X-stabilizer only.
        assert_eq!(synd[6], 1);
        assert_eq!(synd[7], 0);
        encoded.correct(&synd).unwrap();
        let zero = lq.encode(1.0.into(), 0.0.into()).unwrap();
        let mut fid = 0.0;
        for (a, b) in encoded.amplitudes.iter().zip(zero.amplitudes.iter()) {
            fid += (a.conj() * b).re;
        }
        assert!(fid > 0.9999);
    }

    #[test]
    fn test_shor_correct_combined() {
        // Bit-flip on qubit 2 and phase-flip on block 2 (qubit 3).
        let lq = LogicalQubit::new(QuantumCode::Shor9);
        let mut encoded = lq.encode(1.0.into(), 0.0.into()).unwrap();
        encoded.apply_x(2);
        encoded.apply_z(3);
        let synd = encoded.detect_error();
        encoded.correct(&synd).unwrap();
        let zero = lq.encode(1.0.into(), 0.0.into()).unwrap();
        let mut fid = 0.0;
        for (a, b) in encoded.amplitudes.iter().zip(zero.amplitudes.iter()) {
            fid += (a.conj() * b).re;
        }
        assert!(fid > 0.9999);
    }

    #[test]
    fn test_surface_syndrome_and_correct() {
        let lq = LogicalQubit::new(QuantumCode::SurfaceCode { d: 3 });
        assert_eq!(lq.detect_error(), vec![0; 8]); // 9-1 syndrome bits
        let mut encoded = lq.encode(1.0.into(), 0.0.into()).unwrap();
        encoded.apply_x(5);
        let synd = encoded.detect_error();
        // Bit flip on qubit 5 -> syndrome leading ones ends before 5.
        assert!(synd.contains(&1));
        encoded.correct(&synd).unwrap();
        let zero = lq.encode(1.0.into(), 0.0.into()).unwrap();
        let mut fid = 0.0;
        for (a, b) in encoded.amplitudes.iter().zip(zero.amplitudes.iter()) {
            fid += (a.conj() * b).re;
        }
        assert!(fid > 0.9999);
    }

    #[test]
    fn test_measure() {
        let lq = LogicalQubit::new(QuantumCode::Repetition3);
        let result = lq.measure();
        assert_eq!(result.len(), 3);
        // |000⟩ should always measure [0,0,0]
        assert_eq!(result, vec![0, 0, 0]);
    }

    #[test]
    fn test_encode_superposition() {
        let lq = LogicalQubit::new(QuantumCode::Repetition3);
        let alpha = ComplexScalar::new(1.0 / 2.0_f64.sqrt(), 0.0);
        let beta = ComplexScalar::new(1.0 / 2.0_f64.sqrt(), 0.0);
        let encoded = lq.encode(alpha, beta).unwrap();
        let norm: Scalar = encoded.amplitudes.iter().map(|a| a.norm_sqr()).sum();
        assert!((norm - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_steane_no_error_syndrome() {
        let lq = LogicalQubit::new(QuantumCode::Steane7);
        let synd = lq.detect_error();
        assert_eq!(synd.len(), 6);
        assert_eq!(synd, vec![0; 6]);
    }

    #[test]
    fn test_steane_correct_bit_flip() {
        let lq = LogicalQubit::new(QuantumCode::Steane7);
        let mut encoded = lq.encode(1.0.into(), 0.0.into()).unwrap();
        // Inject a single X error on qubit 4.
        encoded.apply_x(4);
        let synd = encoded.detect_error();
        assert!(synd[0..3].contains(&1), "X error must be detected");
        encoded.correct(&synd).unwrap();
        let zero = lq.encode(1.0.into(), 0.0.into()).unwrap();
        let mut fid = 0.0;
        for (a, b) in encoded.amplitudes.iter().zip(zero.amplitudes.iter()) {
            fid += (a.conj() * b).re;
        }
        assert!(fid > 0.9999);
    }

    #[test]
    fn test_steane_correct_phase_flip() {
        let lq = LogicalQubit::new(QuantumCode::Steane7);
        let mut encoded = lq.encode(1.0.into(), 0.0.into()).unwrap();
        // Inject a single Z error on qubit 2.
        encoded.apply_z(2);
        let synd = encoded.detect_error();
        assert!(synd[3..6].contains(&1), "Z error must be detected");
        encoded.correct(&synd).unwrap();
        let zero = lq.encode(1.0.into(), 0.0.into()).unwrap();
        let mut fid = 0.0;
        for (a, b) in encoded.amplitudes.iter().zip(zero.amplitudes.iter()) {
            fid += (a.conj() * b).re;
        }
        assert!(fid > 0.9999);
    }
}
