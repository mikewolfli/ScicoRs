// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! End-to-end checkpoint/resume integration tests (Phase 38).
//!
//! These tests exercise the real [`SimEngine`] rather than in-memory type
//! construction: a deterministic diagram is run without interruption, then run
//! again while capturing a checkpoint mid-run, and the resumed run's final state,
//! time and step count must match the uninterrupted run exactly. Determinism of a
//! real engine run is what makes "resume produces the same result" a meaningful
//! claim.

use crate::core::block::SimpleBlock;
use crate::core::diagram::Diagram;
use crate::runtime::checkpoint::{
    CheckpointStore, CompatibilityPolicy, ModelSignature, SimulationSnapshot,
};
use crate::runtime::context::TimeConfig;
use crate::runtime::engine::{SimEngine, SimStepResult};
use std::collections::BTreeMap;

fn time_config() -> TimeConfig {
    TimeConfig {
        start_time: 0.0,
        end_time: 0.5,
        max_step: 0.05,
        min_step: 1e-9,
        initial_step: 0.01,
    }
}

fn make_diagram() -> Diagram {
    let mut diagram = Diagram::new("ckpt_diagram");
    diagram.add_block(Box::new(SimpleBlock::new("b1", "Source")));
    diagram
}

fn signature() -> ModelSignature {
    ModelSignature {
        model_name: "ckpt_diagram".to_string(),
        model_hash: 0x1234,
        solver: "rk4".to_string(),
        library_version: "0.2.0".to_string(),
        plugin_versions: BTreeMap::new(),
    }
}

/// Snapshot an engine's observable state so two runs can be compared.
fn capture(engine: &SimEngine, sig: ModelSignature) -> SimulationSnapshot {
    let mut snap = SimulationSnapshot::new(sig);
    snap.sim_time = engine.context.t;
    snap.dt = engine.context.dt;
    snap.step_count = engine.context.step_count;
    snap.continuous_x = engine.state.continuous.values().to_vec();
    snap.continuous_dx = engine.state.continuous.derivatives().to_vec();
    snap.discrete_z = engine.state.discrete.values().to_vec();
    // The event queue exposes only peek/pop (no iteration), so pending events are
    // not captured here; this deterministic diagram schedules none. The snapshot
    // format supports events for diagrams that do.
    snap.events = Vec::new();
    snap
}

/// Restore an engine's observable state from a snapshot.
fn restore(engine: &mut SimEngine, snap: &SimulationSnapshot) {
    engine.context.t = snap.sim_time;
    engine.context.dt = snap.dt;
    engine.context.step_count = snap.step_count;
    // Continuous/discrete state lengths are fixed by the diagram; restore in place.
    if snap.continuous_x.len() == engine.state.continuous.len() {
        for (i, &v) in snap.continuous_x.iter().enumerate() {
            engine.state.continuous.set_index(i, v);
        }
    }
    if snap.continuous_dx.len() == engine.state.continuous.len() {
        engine.state.continuous.set_derivatives(&snap.continuous_dx);
    }
}

/// Run to completion, returning (final_time, step_count, final_x).
fn run_to_end() -> (f64, u64, Vec<f64>) {
    let mut engine = SimEngine::new(make_diagram(), time_config()).unwrap();
    engine.init().unwrap();
    engine.start().unwrap();
    for _ in 0..10000 {
        match engine.step().unwrap() {
            SimStepResult::Finished => break,
            SimStepResult::Error(e) => panic!("engine error: {e}"),
            _ => {}
        }
    }
    (
        engine.context.t,
        engine.context.step_count,
        engine.state.continuous.values().to_vec(),
    )
}

#[test]
fn checkpoint_resume_matches_uninterrupted_run() {
    let dir = std::env::temp_dir().join(format!("scico_ckpt_e2e_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);

    // Reference: uninterrupted run.
    let (ref_time, ref_steps, ref_x) = run_to_end();

    // Interrupted run: run half the steps, checkpoint, tear down, restore, finish.
    let split = ref_steps / 2;
    let mut store = CheckpointStore::new(&dir, signature());

    let mut engine = SimEngine::new(make_diagram(), time_config()).unwrap();
    engine.init().unwrap();
    engine.start().unwrap();
    for _ in 0..split {
        if let SimStepResult::Finished = engine.step().unwrap() {
            break;
        }
    }
    let snap = capture(&engine, signature());
    let manifest = store.save(&snap).unwrap();
    assert_eq!(manifest.step_count, snap.step_count);
    drop(engine);

    // Resume in a brand-new engine built from the same diagram.
    let mut engine2 = SimEngine::new(make_diagram(), time_config()).unwrap();
    engine2.init().unwrap();
    engine2.start().unwrap();
    let (_m, loaded) = store.load().unwrap();
    restore(&mut engine2, &loaded);
    // Continue to the end.
    for _ in 0..10000 {
        match engine2.step().unwrap() {
            SimStepResult::Finished => break,
            SimStepResult::Error(e) => panic!("engine error: {e}"),
            _ => {}
        }
    }

    // The resumed run must match the uninterrupted run exactly on a deterministic
    // diagram: final time, step count, and continuous state.
    assert!(
        (engine2.context.t - ref_time).abs() < 1e-12,
        "resumed time {} != reference {}",
        engine2.context.t,
        ref_time
    );
    assert_eq!(engine2.context.step_count, ref_steps);
    let resumed_x = engine2.state.continuous.values().to_vec();
    assert_eq!(resumed_x.len(), ref_x.len());
    for (i, (a, b)) in resumed_x.iter().zip(ref_x.iter()).enumerate() {
        assert!(
            (a - b).abs() < 1e-9,
            "state[{i}] differs: resumed={a} reference={b}"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn incompatible_checkpoint_is_rejected_end_to_end() {
    let dir = std::env::temp_dir().join(format!("scico_ckpt_e2e_reject_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);

    let mut engine = SimEngine::new(make_diagram(), time_config()).unwrap();
    engine.init().unwrap();
    engine.start().unwrap();
    for _ in 0..3 {
        let _ = engine.step().unwrap();
    }
    let mut store = CheckpointStore::new(&dir, signature());
    store.save(&capture(&engine, signature())).unwrap();

    // A store whose signature (solver) differs must reject the checkpoint.
    let mut other = signature();
    other.solver = "bdf2".to_string();
    let store2 = CheckpointStore::new(&dir, other).with_policy(CompatibilityPolicy::default());
    assert!(store2.load().is_err());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn checkpoint_survives_process_independent_reload() {
    // Verify the checkpoint is a real on-disk artifact, not process memory: write
    // it, read the raw files back, and reparse the snapshot independently.
    let dir = std::env::temp_dir().join(format!("scico_ckpt_disk_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);

    let mut engine = SimEngine::new(make_diagram(), time_config()).unwrap();
    engine.init().unwrap();
    engine.start().unwrap();
    for _ in 0..5 {
        let _ = engine.step().unwrap();
    }
    let snap = capture(&engine, signature());
    let mut store = CheckpointStore::new(&dir, signature());
    store.save(&snap).unwrap();

    assert!(dir.join("manifest.json").exists());
    assert!(dir.join("payload.bin").exists());

    // Raw bytes on disk must decode back to the same snapshot.
    let payload = std::fs::read(dir.join("payload.bin")).unwrap();
    let decoded = SimulationSnapshot::from_bytes(&payload).unwrap();
    assert_eq!(decoded.sim_time, snap.sim_time);
    assert_eq!(decoded.step_count, snap.step_count);
    assert_eq!(decoded.continuous_x, snap.continuous_x);

    let _ = std::fs::remove_dir_all(&dir);
}
