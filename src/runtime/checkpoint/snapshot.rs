// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Simulation state, event-queue, RNG and recorder snapshots (Phase 38).
//!
//! A [`SimulationSnapshot`] captures everything needed to resume a run exactly:
//! the continuous/discrete state, the pending event queue, the RNG state, the
//! engine time/step counters and the recorder position. It serializes to a
//! compact, versioned byte payload that the checkpoint layer hashes and stores.

use super::format::{CheckpointError, EventRecord, ModelSignature, RecorderSnapshot};
use crate::core::types::{Scalar, SignalValue};

/// Serialization schema version for the snapshot payload itself.
pub const SNAPSHOT_VERSION: u32 = 1;

/// A full simulation snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct SimulationSnapshot {
    /// Snapshot payload schema version.
    pub version: u32,
    /// Serialization engine version (snapshot embedded version).
    pub sim_time: Scalar,
    /// Current step size.
    pub dt: Scalar,
    /// Number of steps executed.
    pub step_count: u64,
    /// Continuous state values `x`.
    pub continuous_x: Vec<Scalar>,
    /// Continuous state derivatives `dx`.
    pub continuous_dx: Vec<Scalar>,
    /// Discrete state values.
    pub discrete_z: Vec<SignalValue>,
    /// Pending events, in queue order.
    pub events: Vec<EventRecord>,
    /// RNG seed in effect.
    pub rng_seed: u64,
    /// Number of RNG draws consumed so far.
    pub rng_draws: u64,
    /// Recorder position snapshot.
    pub recorder: RecorderSnapshot,
    /// Model/solver/plugin signature for compatibility checks.
    pub signature: ModelSignature,
}

impl SimulationSnapshot {
    /// Create an empty snapshot with the given signature.
    pub fn new(signature: ModelSignature) -> Self {
        Self {
            version: SNAPSHOT_VERSION,
            sim_time: 0.0,
            dt: 0.0,
            step_count: 0,
            continuous_x: Vec::new(),
            continuous_dx: Vec::new(),
            discrete_z: Vec::new(),
            events: Vec::new(),
            rng_seed: 0,
            rng_draws: 0,
            recorder: RecorderSnapshot::default(),
            signature,
        }
    }

    /// Number of continuous variables captured.
    pub fn continuous_len(&self) -> usize {
        self.continuous_x.len()
    }

    /// Number of discrete variables captured.
    pub fn discrete_len(&self) -> usize {
        self.discrete_z.len()
    }

    /// Serialize the snapshot to a compact, versioned byte payload.
    ///
    /// The format is a simple length-prefixed binary encoding so it is
    /// self-describing enough to detect truncation, and deterministic so its
    /// hash is stable across runs.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.version.to_le_bytes());
        out.extend_from_slice(&self.sim_time.to_le_bytes());
        out.extend_from_slice(&self.dt.to_le_bytes());
        out.extend_from_slice(&self.step_count.to_le_bytes());
        out.extend_from_slice(&self.rng_seed.to_le_bytes());
        out.extend_from_slice(&self.rng_draws.to_le_bytes());

        put_scalar_vec(&mut out, &self.continuous_x);
        put_scalar_vec(&mut out, &self.continuous_dx);

        // Discrete state: length + tagged values.
        out.extend_from_slice(&(self.discrete_z.len() as u64).to_le_bytes());
        for v in &self.discrete_z {
            put_signal_value(&mut out, v);
        }

        // Events.
        out.extend_from_slice(&(self.events.len() as u64).to_le_bytes());
        for e in &self.events {
            put_str(&mut out, &e.id);
            out.extend_from_slice(&e.time.to_le_bytes());
            put_str(&mut out, &e.kind);
            out.extend_from_slice(&e.priority.to_le_bytes());
            match &e.target {
                Some(t) => {
                    out.push(1);
                    put_str(&mut out, t);
                }
                None => out.push(0),
            }
            match e.scalar {
                Some(s) => {
                    out.push(1);
                    out.extend_from_slice(&s.to_le_bytes());
                }
                None => out.push(0),
            }
        }

        // Recorder.
        match &self.recorder.output_path {
            Some(p) => {
                out.push(1);
                put_str(&mut out, p);
            }
            None => out.push(0),
        }
        out.extend_from_slice(&(self.recorder.samples_written as u64).to_le_bytes());
        out.extend_from_slice(&(self.recorder.signals.len() as u64).to_le_bytes());
        for s in &self.recorder.signals {
            put_str(&mut out, s);
        }

        // Signature.
        put_str(&mut out, &self.signature.model_name);
        out.extend_from_slice(&self.signature.model_hash.to_le_bytes());
        put_str(&mut out, &self.signature.solver);
        put_str(&mut out, &self.signature.library_version);
        out.extend_from_slice(&(self.signature.plugin_versions.len() as u64).to_le_bytes());
        for (k, v) in &self.signature.plugin_versions {
            put_str(&mut out, k);
            put_str(&mut out, v);
        }

        out
    }

    /// Parse a snapshot from bytes, rejecting truncation or a future version.
    pub fn from_bytes(data: &[u8]) -> Result<Self, CheckpointError> {
        let mut r = Reader::new(data);
        let version = r.u32()?;
        if version > SNAPSHOT_VERSION {
            return Err(CheckpointError::UnsupportedSchema {
                found: version,
                supported: SNAPSHOT_VERSION,
            });
        }
        let sim_time = r.f64()?;
        let dt = r.f64()?;
        let step_count = r.u64()?;
        let rng_seed = r.u64()?;
        let rng_draws = r.u64()?;

        let continuous_x = r.scalar_vec()?;
        let continuous_dx = r.scalar_vec()?;

        let nz = r.u64()? as usize;
        let mut discrete_z = Vec::with_capacity(nz);
        for _ in 0..nz {
            discrete_z.push(r.signal_value()?);
        }

        let ne = r.u64()? as usize;
        let mut events = Vec::with_capacity(ne);
        for _ in 0..ne {
            let id = r.string()?;
            let time = r.f64()?;
            let kind = r.string()?;
            let priority = r.i32()?;
            let target = if r.u8()? == 1 {
                Some(r.string()?)
            } else {
                None
            };
            let scalar = if r.u8()? == 1 { Some(r.f64()?) } else { None };
            events.push(EventRecord {
                id,
                time,
                kind,
                priority,
                target,
                scalar,
            });
        }

        let output_path = if r.u8()? == 1 {
            Some(r.string()?)
        } else {
            None
        };
        let samples_written = r.u64()? as usize;
        let ns = r.u64()? as usize;
        let mut signals = Vec::with_capacity(ns);
        for _ in 0..ns {
            signals.push(r.string()?);
        }

        let model_name = r.string()?;
        let model_hash = r.u64()?;
        let solver = r.string()?;
        let library_version = r.string()?;
        let np = r.u64()? as usize;
        let mut plugin_versions = std::collections::BTreeMap::new();
        for _ in 0..np {
            let k = r.string()?;
            let v = r.string()?;
            plugin_versions.insert(k, v);
        }

        Ok(Self {
            version,
            sim_time,
            dt,
            step_count,
            continuous_x,
            continuous_dx,
            discrete_z,
            events,
            rng_seed,
            rng_draws,
            recorder: RecorderSnapshot {
                output_path,
                samples_written,
                signals,
            },
            signature: ModelSignature {
                model_name,
                model_hash,
                solver,
                library_version,
                plugin_versions,
            },
        })
    }
}

fn put_str(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(&(s.len() as u64).to_le_bytes());
    out.extend_from_slice(s.as_bytes());
}

fn put_scalar_vec(out: &mut Vec<u8>, v: &[Scalar]) {
    out.extend_from_slice(&(v.len() as u64).to_le_bytes());
    for &x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
}

fn put_signal_value(out: &mut Vec<u8>, v: &SignalValue) {
    match v {
        SignalValue::Scalar(x) => {
            out.push(0);
            out.extend_from_slice(&x.to_le_bytes());
        }
        SignalValue::Integer(i) => {
            out.push(1);
            out.extend_from_slice(&i.to_le_bytes());
        }
        SignalValue::Boolean(b) => {
            out.push(2);
            out.push(u8::from(*b));
        }
        SignalValue::Complex(re, im) => {
            out.push(3);
            out.extend_from_slice(&re.to_le_bytes());
            out.extend_from_slice(&im.to_le_bytes());
        }
        SignalValue::Vector(vals) => {
            out.push(4);
            put_scalar_vec(out, vals);
        }
        SignalValue::Matrix(rows, cols, data) => {
            out.push(5);
            out.extend_from_slice(&(*rows as u64).to_le_bytes());
            out.extend_from_slice(&(*cols as u64).to_le_bytes());
            put_scalar_vec(out, data);
        }
        SignalValue::String(s) => {
            out.push(6);
            put_str(out, s);
        }
        SignalValue::Tensor(t) => {
            // Encode a tensor via its JSON form to avoid duplicating the tensor
            // schema here; the string is length-prefixed like any other.
            out.push(7);
            put_str(out, &format!("{t:?}"));
        }
        SignalValue::None => {
            out.push(8);
        }
    }
}

/// A bounds-checked little-endian reader.
struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], CheckpointError> {
        if self.pos + n > self.data.len() {
            return Err(CheckpointError::Parse(format!(
                "snapshot truncated at byte {} (need {n} more)",
                self.pos
            )));
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8, CheckpointError> {
        Ok(self.take(1)?[0])
    }

    fn u32(&mut self) -> Result<u32, CheckpointError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn i32(&mut self) -> Result<i32, CheckpointError> {
        let b = self.take(4)?;
        Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn u64(&mut self) -> Result<u64, CheckpointError> {
        let b = self.take(8)?;
        Ok(u64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    fn f64(&mut self) -> Result<Scalar, CheckpointError> {
        let b = self.take(8)?;
        Ok(Scalar::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    fn string(&mut self) -> Result<String, CheckpointError> {
        let n = self.u64()? as usize;
        let bytes = self.take(n)?;
        String::from_utf8(bytes.to_vec())
            .map_err(|e| CheckpointError::Parse(format!("invalid UTF-8 in string: {e}")))
    }

    fn scalar_vec(&mut self) -> Result<Vec<Scalar>, CheckpointError> {
        let n = self.u64()? as usize;
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            out.push(self.f64()?);
        }
        Ok(out)
    }

    fn signal_value(&mut self) -> Result<SignalValue, CheckpointError> {
        let tag = self.u8()?;
        match tag {
            0 => Ok(SignalValue::Scalar(self.f64()?)),
            1 => Ok(SignalValue::Integer(self.u64()? as i64)),
            2 => Ok(SignalValue::Boolean(self.u8()? != 0)),
            3 => {
                let re = self.f64()?;
                let im = self.f64()?;
                Ok(SignalValue::Complex(re, im))
            }
            4 => Ok(SignalValue::Vector(self.scalar_vec()?)),
            5 => {
                let rows = self.u64()? as usize;
                let cols = self.u64()? as usize;
                let data = self.scalar_vec()?;
                Ok(SignalValue::Matrix(rows, cols, data))
            }
            6 => Ok(SignalValue::String(self.string()?)),
            7 => {
                // Tensor payloads are stored in their Debug form; recovery is
                // intentionally limited and reported rather than silently wrong.
                Err(CheckpointError::Parse(
                    "tensor signal values are not supported by checkpoint snapshots".to_string(),
                ))
            }
            8 => Ok(SignalValue::None),
            other => Err(CheckpointError::Parse(format!(
                "unknown signal value tag {other}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap() -> SimulationSnapshot {
        let mut s = SimulationSnapshot::new(ModelSignature {
            model_name: "m".to_string(),
            model_hash: 42,
            solver: "rk4".to_string(),
            library_version: "0.2.0".to_string(),
            plugin_versions: std::collections::BTreeMap::new(),
        });
        s.sim_time = 1.25;
        s.dt = 0.01;
        s.step_count = 125;
        s.continuous_x = vec![1.0, 2.0, 3.0];
        s.continuous_dx = vec![0.1, 0.2, 0.3];
        s.discrete_z = vec![
            SignalValue::Scalar(5.0),
            SignalValue::Integer(-3),
            SignalValue::Boolean(true),
            SignalValue::Complex(1.0, -1.0),
            SignalValue::Vector(vec![1.0, 2.0]),
            SignalValue::None,
        ];
        s.events = vec![EventRecord {
            id: "ev1".to_string(),
            time: 2.0,
            kind: "timer".to_string(),
            priority: 3,
            target: Some("blk".to_string()),
            scalar: Some(9.5),
        }];
        s.rng_seed = 1234;
        s.rng_draws = 99;
        s.recorder.output_path = Some("out.csv".to_string());
        s.recorder.samples_written = 7;
        s.recorder.signals = vec!["a".to_string(), "b".to_string()];
        s
    }

    #[test]
    fn snapshot_roundtrip_is_lossless() {
        let s = snap();
        let bytes = s.to_bytes();
        let back = SimulationSnapshot::from_bytes(&bytes).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn truncated_snapshot_is_rejected() {
        let s = snap();
        let bytes = s.to_bytes();
        let truncated = &bytes[..bytes.len() / 2];
        assert!(SimulationSnapshot::from_bytes(truncated).is_err());
    }

    #[test]
    fn future_version_rejected() {
        let s = snap();
        let mut bytes = s.to_bytes();
        // Overwrite the version field with a larger value.
        bytes[0..4].copy_from_slice(&(SNAPSHOT_VERSION + 1).to_le_bytes());
        let err = SimulationSnapshot::from_bytes(&bytes).unwrap_err();
        assert!(matches!(err, CheckpointError::UnsupportedSchema { .. }));
    }

    #[test]
    fn empty_snapshot_roundtrips() {
        let s = SimulationSnapshot::new(ModelSignature::default());
        let back = SimulationSnapshot::from_bytes(&s.to_bytes()).unwrap();
        assert_eq!(s, back);
        assert_eq!(back.continuous_len(), 0);
        assert_eq!(back.discrete_len(), 0);
    }

    #[test]
    fn determinstic_bytes_for_same_state() {
        // Two identical snapshots must produce identical payloads so their hash
        // is stable across runs.
        let a = snap().to_bytes();
        let b = snap().to_bytes();
        assert_eq!(a, b);
    }
}
