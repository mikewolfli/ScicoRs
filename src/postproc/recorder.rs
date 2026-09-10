//! Data recording, replay, and offline analysis.

use crate::core::types::Scalar;
use std::collections::HashMap;

/// Configuration for data recording.
pub struct RecorderConfig {
    pub max_samples: usize,
    pub sampling_interval: Scalar,
    pub record_signals: Vec<String>,
    pub enable_streaming: bool,
    pub output_path: Option<String>,
}

impl Default for RecorderConfig {
    fn default() -> Self {
        Self {
            max_samples: 10000,
            sampling_interval: 0.01,
            record_signals: Vec::new(),
            enable_streaming: false,
            output_path: None,
        }
    }
}

/// Data recorder for simulation signals.
///
/// When `config.enable_streaming` is `true`, the recorder flushes buffered
/// data to disk whenever the in-memory buffer reaches `max_samples`, then
/// clears the buffer and continues recording. This prevents unbounded memory
/// growth in long-running simulations.
pub struct DataRecorder {
    pub config: RecorderConfig,
    pub time_stamps: Vec<Scalar>,
    pub recorded_data: HashMap<String, Vec<Scalar>>,
    pub current_count: usize,
    /// Total samples written across all flushes (monotonic counter).
    pub total_written: usize,
    /// Number of flush events that have occurred.
    pub flush_count: usize,
    /// Samples rejected because the buffer was full in non-streaming mode.
    pub dropped_samples: usize,
}

impl DataRecorder {
    pub fn new(config: RecorderConfig) -> Self {
        let max_samples = config.max_samples;
        Self {
            config,
            time_stamps: Vec::with_capacity(max_samples),
            recorded_data: HashMap::new(),
            current_count: 0,
            total_written: 0,
            flush_count: 0,
            dropped_samples: 0,
        }
    }

    /// Record one sample. When streaming is enabled and the buffer is full,
    /// the buffer is automatically flushed to disk and cleared.
    pub fn record(&mut self, time: Scalar, signals: &HashMap<String, Scalar>) {
        if self.config.enable_streaming && self.current_count >= self.config.max_samples {
            // Flush buffered data to disk and clear
            if let Err(e) = self.append_csv() {
                log_warn(&format!("Streaming flush failed: {}", e));
            }
            self.flush_buffer();
        }

        if !self.config.enable_streaming && self.current_count >= self.config.max_samples {
            // Non-streaming mode: the buffer is full and data cannot be kept.
            // Count the loss so callers can tell "recorded 10000" apart from
            // "recorded 10000 and dropped 40000".
            self.dropped_samples += 1;
            return;
        }

        self.time_stamps.push(time);
        // Keep every series exactly as long as `time_stamps` so that columns
        // cannot silently shift when a signal first appears part-way through
        // the run. Newly-seen signals are back-filled with NaN ("no data at
        // this time"), matching `postproc::batch`.
        for series in self.recorded_data.values_mut() {
            if series.len() < self.time_stamps.len() - 1 {
                series.resize(self.time_stamps.len() - 1, Scalar::NAN);
            }
        }
        for (name, &val) in signals {
            let series = self.recorded_data.entry(name.clone()).or_default();
            series.resize(series.len().max(self.time_stamps.len() - 1), Scalar::NAN);
            series.push(val);
        }
        // Signals present earlier but absent now get NaN rather than a
        // truncated series.
        for series in self.recorded_data.values_mut() {
            series.resize(self.time_stamps.len(), Scalar::NAN);
        }
        self.current_count += 1;
    }

    /// Append the current buffer to the output CSV file.
    /// Creates the file with headers on the first flush.
    fn append_csv(&self) -> Result<(), String> {
        let path = self
            .config
            .output_path
            .as_deref()
            .ok_or("No output path set")?;
        let file_exists = std::path::Path::new(path).exists();

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| format!("Open error: {}", e))?;

        use std::io::Write;
        let mut writer = std::io::BufWriter::new(&mut file);

        // Write header only on first creation
        if !file_exists {
            write!(writer, "time").map_err(|e| format!("Write error: {}", e))?;
            for name in self.signal_names() {
                write!(writer, ",{}", name).map_err(|e| format!("Write error: {}", e))?;
            }
            writeln!(writer).map_err(|e| format!("Write error: {}", e))?;
        }

        // Write data rows. Columns follow `signal_names()` so the header and
        // the values are guaranteed to line up.
        let names = self.signal_names();
        for i in 0..self.time_stamps.len() {
            write!(writer, "{}", self.time_stamps[i]).map_err(|e| format!("Write error: {}", e))?;
            for name in &names {
                let value = self
                    .recorded_data
                    .get(*name)
                    .and_then(|data| data.get(i))
                    .copied()
                    .unwrap_or(Scalar::NAN);
                write!(writer, ",{}", value).map_err(|e| format!("Write error: {}", e))?;
            }
            writeln!(writer).map_err(|e| format!("Write error: {}", e))?;
        }

        writer.flush().map_err(|e| format!("Flush error: {}", e))?;
        Ok(())
    }

    /// Flush remaining buffered data to disk (final flush at end of simulation).
    pub fn flush_to_disk(&self) -> Result<(), String> {
        if self.current_count == 0 {
            return Ok(());
        }
        if let Some(ref path) = self.config.output_path {
            self.export_csv(path)?;
        }
        Ok(())
    }

    pub fn get_timeseries(&self, signal_name: &str) -> Option<&[Scalar]> {
        self.recorded_data.get(signal_name).map(|v| v.as_slice())
    }

    pub fn signal_names(&self) -> Vec<&String> {
        // Sorted so CSV headers and columns are deterministic across runs;
        // `HashMap` iteration order is not stable.
        let mut names: Vec<&String> = self.recorded_data.keys().collect();
        names.sort();
        names
    }

    /// Reset the buffer without touching the accounting counters.
    ///
    /// This is the internal counterpart of the public [`Self::clear`], used by
    /// the streaming flush path where the samples have genuinely been written
    /// out and `total_written`/`flush_count` must be updated.
    fn flush_buffer(&mut self) {
        self.total_written += self.current_count;
        self.flush_count += 1;
        self.time_stamps.clear();
        self.recorded_data.clear();
        self.current_count = 0;
    }

    /// Discard all buffered samples and reset the counters.
    ///
    /// This is a full reset: unlike the previous implementation it does not
    /// increment `total_written`/`flush_count`, which made calling `clear()`
    /// for its documented purpose corrupt the statistics.
    pub fn clear(&mut self) {
        self.time_stamps.clear();
        self.recorded_data.clear();
        self.current_count = 0;
        self.total_written = 0;
        self.flush_count = 0;
        self.dropped_samples = 0;
    }

    /// Total number of samples accepted into the buffer (including samples
    /// already flushed to disk).
    pub fn total_written(&self) -> usize {
        self.total_written
    }

    /// Number of streaming flushes performed.
    pub fn flush_count(&self) -> usize {
        self.flush_count
    }

    /// Number of samples rejected because the non-streaming buffer was full.
    /// Use this to detect silent data loss in non-streaming mode.
    pub fn dropped_samples(&self) -> usize {
        self.dropped_samples
    }

    pub fn export_csv(&self, filepath: &str) -> Result<(), String> {
        if self.time_stamps.is_empty() {
            return Ok(());
        }

        let file_exists = std::path::Path::new(filepath).exists();
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(filepath)
            .map_err(|e| format!("Open error: {}", e))?;

        use std::io::Write;
        let mut writer = std::io::BufWriter::new(&mut file);

        if !file_exists {
            write!(writer, "time").map_err(|e| format!("Write error: {}", e))?;
            for name in self.signal_names() {
                write!(writer, ",{}", name).map_err(|e| format!("Write error: {}", e))?;
            }
            writeln!(writer).map_err(|e| format!("Write error: {}", e))?;
        }

        // Write data rows. Columns follow `signal_names()` so the header and
        // the values are guaranteed to line up.
        let names = self.signal_names();
        for i in 0..self.time_stamps.len() {
            write!(writer, "{}", self.time_stamps[i]).map_err(|e| format!("Write error: {}", e))?;
            for name in &names {
                let value = self
                    .recorded_data
                    .get(*name)
                    .and_then(|data| data.get(i))
                    .copied()
                    .unwrap_or(Scalar::NAN);
                write!(writer, ",{}", value).map_err(|e| format!("Write error: {}", e))?;
            }
            writeln!(writer).map_err(|e| format!("Write error: {}", e))?;
        }

        writer.flush().map_err(|e| format!("Flush error: {}", e))?;
        Ok(())
    }
}

/// Internal helper: log a warning message (avoids pulling in full logging infra).
fn log_warn(msg: &str) {
    eprintln!("[WARN] DataRecorder: {}", msg);
}

// ── 3D Field Snapshot Recorder ──────────────────────────────────────────

/// A snapshot of a 3D field at a given simulation time.
#[derive(Debug, Clone)]
pub struct FieldSnapshot3D {
    pub time: Scalar,
    pub name: String,
    pub field: Vec<Vec<Vec<Scalar>>>, // [z][y][x]
    pub dx: Scalar,
    pub dy: Scalar,
    pub dz: Scalar,
}

/// Recorder for 3D field snapshots (e.g. temperature, pressure, velocity magnitude).
///
/// Unlike `DataRecorder` (which records 1D time-series), this captures
/// entire 3D fields at configurable intervals for post-processing and
/// visualization.
pub struct FieldRecorder3D {
    pub snapshots: Vec<FieldSnapshot3D>,
    pub interval: usize,
    pub max_snapshots: usize,
    pub step_counter: usize,
}

impl FieldRecorder3D {
    pub fn new(interval: usize, max_snapshots: usize) -> Self {
        Self {
            snapshots: Vec::new(),
            interval,
            max_snapshots,
            step_counter: 0,
        }
    }

    /// Record a snapshot every `interval` steps.
    #[allow(clippy::manual_is_multiple_of)]
    pub fn record(
        &mut self,
        name: &str,
        field: Vec<Vec<Vec<Scalar>>>,
        dx: Scalar,
        dy: Scalar,
        dz: Scalar,
        time: Scalar,
    ) {
        self.step_counter += 1;
        if self.step_counter % self.interval != 0 {
            return;
        }
        if self.snapshots.len() >= self.max_snapshots {
            return; // Max snapshots reached
        }
        self.snapshots.push(FieldSnapshot3D {
            time,
            name: name.to_string(),
            field,
            dx,
            dy,
            dz,
        });
    }

    /// Extract a 2D slice from the most recent snapshot.
    pub fn latest_slice(&self, axis: char, index: usize) -> Option<Vec<Vec<Scalar>>> {
        let snap = self.snapshots.last()?;
        if snap.field.is_empty() || snap.field[0].is_empty() {
            return None;
        }
        match axis {
            'z' => snap.field.get(index).cloned(),
            'y' => {
                let ny = snap.field[0].len();
                if index >= ny {
                    return None;
                }
                let mut slice = vec![vec![0.0; snap.field[0][0].len()]; snap.field.len()];
                for k in 0..snap.field.len() {
                    for i in 0..snap.field[0][0].len() {
                        slice[k][i] = snap.field[k][index][i];
                    }
                }
                Some(slice)
            }
            'x' => {
                let nx = snap.field[0][0].len();
                if index >= nx {
                    return None;
                }
                let mut slice = vec![vec![0.0; snap.field[0].len()]; snap.field.len()];
                for k in 0..snap.field.len() {
                    for j in 0..snap.field[0].len() {
                        slice[k][j] = snap.field[k][j][index];
                    }
                }
                Some(slice)
            }
            _ => None,
        }
    }

    /// Number of snapshots recorded.
    pub fn num_snapshots(&self) -> usize {
        self.snapshots.len()
    }

    /// Clear all snapshots.
    pub fn clear(&mut self) {
        self.snapshots.clear();
        self.step_counter = 0;
    }

    /// Get total memory estimate in bytes.
    pub fn memory_estimate_bytes(&self) -> usize {
        self.snapshots
            .iter()
            .map(|s| {
                std::mem::size_of::<Scalar>()
                    * s.field
                        .iter()
                        .map(|k| k.iter().map(|j| j.len()).sum::<usize>())
                        .sum::<usize>()
                    + s.name.len()
                    + std::mem::size_of::<Scalar>() * 4
            })
            .sum()
    }
}

/// Data replayer for playing back recorded signals.
pub struct DataReplayer {
    pub data: HashMap<String, Vec<Scalar>>,
    pub time: Vec<Scalar>,
    pub current_index: usize,
}

impl DataReplayer {
    pub fn new(data: HashMap<String, Vec<Scalar>>, time: Vec<Scalar>) -> Self {
        Self {
            data,
            time,
            current_index: 0,
        }
    }

    pub fn from_csv(filepath: &str) -> Result<Self, String> {
        let content =
            std::fs::read_to_string(filepath).map_err(|e| format!("Read error: {}", e))?;
        let lines: Vec<&str> = content.lines().collect();
        if lines.len() < 2 {
            return Err("CSV too short".to_string());
        }
        let headers: Vec<&str> = lines[0].split(',').collect();
        let mut data: HashMap<String, Vec<Scalar>> = HashMap::new();
        for h in &headers {
            data.insert(h.to_string(), Vec::new());
        }
        for line in &lines[1..] {
            let vals: Vec<&str> = line.split(',').collect();
            for (j, &h) in headers.iter().enumerate() {
                if j < vals.len() {
                    if let Ok(v) = vals[j].trim().parse::<Scalar>() {
                        data.get_mut(h).unwrap().push(v);
                    }
                }
            }
        }
        let time = data.remove("time").unwrap_or_default();
        Ok(Self::new(data, time))
    }

    pub fn current_values(&self) -> HashMap<String, Scalar> {
        let mut vals = HashMap::new();
        for (name, vec_data) in &self.data {
            if self.current_index < vec_data.len() {
                vals.insert(name.clone(), vec_data[self.current_index]);
            }
        }
        vals
    }

    pub fn advance(&mut self) -> bool {
        if self.current_index + 1 < self.time.len() {
            self.current_index += 1;
            true
        } else {
            false
        }
    }

    pub fn reset(&mut self) {
        self.current_index = 0;
    }
}

/// Offline analysis tools.
pub struct OfflineAnalysis {
    pub recorder: DataRecorder,
}

impl OfflineAnalysis {
    pub fn new(recorder: DataRecorder) -> Self {
        Self { recorder }
    }

    pub fn rms(&self, signal: &str) -> Option<Scalar> {
        let data = self.recorder.get_timeseries(signal)?;
        if data.is_empty() {
            return None;
        }
        let sum_sq: Scalar = data.iter().map(|x| x * x).sum();
        Some((sum_sq / data.len() as Scalar).sqrt())
    }

    pub fn mean(&self, signal: &str) -> Option<Scalar> {
        let data = self.recorder.get_timeseries(signal)?;
        if data.is_empty() {
            return None;
        }
        Some(data.iter().sum::<Scalar>() / data.len() as Scalar)
    }

    pub fn min_max(&self, signal: &str) -> Option<(Scalar, Scalar)> {
        let data = self.recorder.get_timeseries(signal)?;
        data.iter().fold(None, |acc, &x| {
            Some(acc.map_or((x, x), |(min, max): (Scalar, Scalar)| {
                (min.min(x), max.max(x))
            }))
        })
    }

    pub fn fft_analysis(&self, signal: &str) -> Option<(Vec<Scalar>, Vec<Scalar>)> {
        let data = self.recorder.get_timeseries(signal)?;
        let n = data.len().next_power_of_two();
        if n < 2 {
            return None;
        }
        let truncated: Vec<Scalar> = data.iter().take(n).copied().collect();
        let (freqs, mags) = crate::core::compute::power_spectrum(&truncated).ok()?;
        Some((freqs, mags))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::Scalar;
    #[test]
    fn test_recorder_record() {
        let mut r = DataRecorder::new(RecorderConfig::default());
        let mut s = HashMap::new();
        s.insert("x".to_string(), 1.0);
        r.record(0.0, &s);
        assert_eq!(r.current_count, 1);
    }
    #[test]
    fn test_recorder_max_samples() {
        let cfg = RecorderConfig {
            max_samples: 2,
            ..Default::default()
        };
        let mut r = DataRecorder::new(cfg);
        for i in 0..5 {
            let mut s = HashMap::new();
            s.insert("x".to_string(), i as Scalar);
            r.record(i as Scalar, &s);
        }
        assert_eq!(r.current_count, 2);
        // Samples beyond the non-streaming capacity are dropped, and the loss
        // must be observable rather than silent.
        assert_eq!(
            r.dropped_samples(),
            3,
            "the 3 samples that did not fit must be counted as dropped"
        );
    }

    /// `clear()` is documented as a reset. It used to increment
    /// `total_written`/`flush_count`, corrupting the statistics of any caller
    /// that used it for its stated purpose.
    #[test]
    fn test_recorder_clear_resets_counters_instead_of_inflating_them() {
        let mut r = DataRecorder::new(RecorderConfig::default());
        let mut s = HashMap::new();
        s.insert("x".to_string(), 1.0);
        for i in 0..3 {
            r.record(i as Scalar, &s);
        }
        r.clear();
        assert_eq!(r.total_written(), 0, "clear() must not report writes");
        assert_eq!(r.flush_count(), 0, "clear() must not report a flush");
        assert_eq!(r.dropped_samples(), 0);
        assert_eq!(r.current_count, 0);
        assert!(r.time_stamps.is_empty());
        assert!(r.recorded_data.is_empty());
    }

    /// Every recorded series must stay the same length as `time_stamps`, so a
    /// signal that first appears part-way through cannot shift its values into
    /// the wrong CSV rows.
    #[test]
    fn test_recorder_series_stay_aligned_when_signals_appear_late() {
        let mut r = DataRecorder::new(RecorderConfig::default());
        // Step 0: only `a` exists.
        let mut s0 = HashMap::new();
        s0.insert("a".to_string(), 1.0);
        r.record(0.0, &s0);
        // Step 1: only `b` exists.
        let mut s1 = HashMap::new();
        s1.insert("b".to_string(), 2.0);
        r.record(1.0, &s1);
        // Step 2: both exist.
        let mut s2 = HashMap::new();
        s2.insert("a".to_string(), 3.0);
        s2.insert("b".to_string(), 4.0);
        r.record(2.0, &s2);

        assert_eq!(r.time_stamps.len(), 3);
        for (name, series) in &r.recorded_data {
            assert_eq!(
                series.len(),
                3,
                "series '{name}' must be aligned with the time stamps"
            );
        }
        // `b` was absent at step 0 and must read back as NaN, not silently shift.
        assert!(r.get_timeseries("b").unwrap()[0].is_nan());
        assert_eq!(r.get_timeseries("b").unwrap()[1], 2.0);
        assert_eq!(r.get_timeseries("b").unwrap()[2], 4.0);
        assert_eq!(r.get_timeseries("a").unwrap()[0], 1.0);
        assert!(r.get_timeseries("a").unwrap()[1].is_nan());
    }

    #[test]
    fn test_recorder_signal_names_are_deterministic() {
        let mut r = DataRecorder::new(RecorderConfig::default());
        let mut s = HashMap::new();
        s.insert("zeta".to_string(), 1.0);
        s.insert("alpha".to_string(), 2.0);
        s.insert("mid".to_string(), 3.0);
        r.record(0.0, &s);
        let names: Vec<String> = r.signal_names().into_iter().cloned().collect();
        assert_eq!(
            names,
            vec!["alpha", "mid", "zeta"],
            "columns must be sorted"
        );
    }
    #[test]
    fn test_recorder_get_timeseries() {
        let mut r = DataRecorder::new(RecorderConfig::default());
        let mut s = HashMap::new();
        s.insert("v".to_string(), 3.0);
        r.record(0.0, &s);
        assert_eq!(r.get_timeseries("v"), Some(&[3.0][..]));
    }
    #[test]
    fn test_offline_rms() {
        let mut r = DataRecorder::new(RecorderConfig::default());
        for i in 0..3 {
            let mut s = HashMap::new();
            s.insert("x".to_string(), i as Scalar);
            r.record(i as Scalar, &s);
        }
        let oa = OfflineAnalysis::new(r);
        let rms = oa.rms("x").unwrap();
        let sum: Scalar = 0.0 + 1.0 + 4.0;
        let expected: Scalar = (sum / 3.0).sqrt();
        assert!((rms - expected).abs() < 1e-10);
    }
    #[test]
    fn test_offline_mean() {
        let mut r = DataRecorder::new(RecorderConfig::default());
        for i in 0..4 {
            let mut s = HashMap::new();
            s.insert("x".to_string(), i as Scalar);
            r.record(i as Scalar, &s);
        }
        let oa = OfflineAnalysis::new(r);
        assert!((oa.mean("x").unwrap() - 1.5).abs() < 1e-10);
    }
    #[test]
    fn test_replayer_advance() {
        let mut data = HashMap::new();
        data.insert("v".to_string(), vec![1.0, 2.0]);
        let mut rp = DataReplayer::new(data, vec![0.0, 1.0]);
        assert_eq!(rp.current_values().get("v"), Some(&1.0));
        assert!(rp.advance());
        assert_eq!(rp.current_values().get("v"), Some(&2.0));
    }
    #[test]
    fn test_replayer_reset() {
        let mut data = HashMap::new();
        data.insert("v".to_string(), vec![1.0, 2.0]);
        let mut rp = DataReplayer::new(data, vec![0.0, 1.0]);
        rp.advance();
        rp.reset();
        assert_eq!(rp.current_index, 0);
    }
    #[test]
    fn test_offline_min_max() {
        let mut r = DataRecorder::new(RecorderConfig::default());
        for i in 0..5 {
            let mut s = HashMap::new();
            s.insert("x".to_string(), (i - 2) as Scalar);
            r.record(i as Scalar, &s);
        }
        let oa = OfflineAnalysis::new(r);
        let (min, max) = oa.min_max("x").unwrap();
        assert!((min - (-2.0)).abs() < 1e-10);
        assert!((max - 2.0).abs() < 1e-10);
    }
    // ── FieldRecorder3D tests ───────────────────────────────────────────
    #[test]
    fn test_field_recorder_3d_basic() {
        let mut fr = FieldRecorder3D::new(2, 5);
        assert_eq!(fr.num_snapshots(), 0);
        // Create a small 3D field (4×4×4)
        let field: Vec<Vec<Vec<Scalar>>> = vec![vec![vec![1.0; 4]; 4]; 4];
        fr.record("temperature", field.clone(), 0.1, 0.1, 0.1, 0.0);
        // Should not record yet (step_counter = 1, interval = 2)
        assert_eq!(fr.num_snapshots(), 0);
        fr.record("temperature", field, 0.1, 0.1, 0.1, 1.0);
        // Now should record (step_counter = 2, interval = 2)
        assert_eq!(fr.num_snapshots(), 1);
    }
    #[test]
    fn test_field_recorder_3d_max_snapshots() {
        let mut fr = FieldRecorder3D::new(1, 3); // every step, max 3
        let field: Vec<Vec<Vec<Scalar>>> = vec![vec![vec![0.0; 2]; 2]; 2];
        for i in 0..10 {
            fr.record("pressure", field.clone(), 0.1, 0.1, 0.1, i as Scalar);
        }
        assert_eq!(fr.num_snapshots(), 3);
    }
    #[test]
    fn test_field_recorder_3d_latest_slice_z() {
        let mut fr = FieldRecorder3D::new(1, 5);
        let field: Vec<Vec<Vec<Scalar>>> = (0..3)
            .map(|k| {
                (0..3)
                    .map(|j| (0..3).map(|i| (k * 100 + j * 10 + i) as Scalar).collect())
                    .collect()
            })
            .collect();
        fr.record("field", field, 0.5, 0.5, 0.5, 0.0);
        let slice = fr.latest_slice('z', 1).unwrap();
        assert_eq!(slice.len(), 3);
        assert_eq!(slice[0].len(), 3);
        // At k=1: value = 100 + 10j + i
        assert!((slice[0][0] - 100.0).abs() < 1e-10);
    }
    #[test]
    fn test_field_recorder_3d_latest_slice_out_of_range() {
        let mut fr = FieldRecorder3D::new(1, 5);
        let field: Vec<Vec<Vec<Scalar>>> = vec![vec![vec![1.0; 2]; 2]; 2];
        fr.record("f", field, 1.0, 1.0, 1.0, 0.0);
        assert!(fr.latest_slice('z', 5).is_none());
        assert!(fr.latest_slice('x', 5).is_none());
        assert!(fr.latest_slice('w', 0).is_none());
    }
    #[test]
    fn test_field_recorder_3d_clear() {
        let mut fr = FieldRecorder3D::new(1, 10);
        let field: Vec<Vec<Vec<Scalar>>> = vec![vec![vec![0.0; 2]; 2]; 2];
        fr.record("f", field, 1.0, 1.0, 1.0, 0.0);
        assert_eq!(fr.num_snapshots(), 1);
        fr.clear();
        assert_eq!(fr.num_snapshots(), 0);
        assert_eq!(fr.step_counter, 0);
    }
    #[test]
    fn test_field_recorder_3d_memory_estimate() {
        let mut fr = FieldRecorder3D::new(1, 5);
        let field: Vec<Vec<Vec<Scalar>>> = vec![vec![vec![1.0; 4]; 4]; 4];
        fr.record("test", field, 1.0, 1.0, 1.0, 0.0);
        // 4*4*4 = 64 scalars × 8 bytes = 512 for field + overhead
        assert!(fr.memory_estimate_bytes() > 500);
    }
}
