// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
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
/// `max_samples` bounds how many samples are held in memory. Reaching it is
/// handled differently depending on `config.enable_streaming`:
///
/// * **streaming enabled** — the buffered block is appended to `output_path`
///   and the buffer is reset, so a long-running simulation has bounded memory.
/// * **streaming disabled** — the recorder stops accepting samples and counts
///   them in [`DataRecorder::dropped_samples`], so the loss is observable.
///
/// The streaming path is both memory-safe **and** lossless, including when the
/// signal set changes mid-run. Each flushed block is written to a temporary
/// *segment* holding only the signals present in that block; the final write
/// ([`DataRecorder::flush_to_disk`]) reads every segment, computes the union of
/// all signals, and emits one rectangular CSV whose rows are the segments'
/// rows in chronological order. A signal that appears late therefore gets a real
/// column, and earlier rows carry NaN for it.
///
/// The alternative — freezing the column set at the first flush — would keep the
/// file rectangular but silently *drop* every later signal, which is data loss
/// disguised as a format limitation.
pub struct DataRecorder {
    pub config: RecorderConfig,
    pub time_stamps: Vec<Scalar>,
    pub recorded_data: HashMap<String, Vec<Scalar>>,
    pub current_count: usize,
    /// Total samples written across all flushes (monotonic counter).
    pub total_written: usize,
    /// Number of flush events that have occurred.
    pub flush_count: usize,
    /// Number of samples rejected because the buffer was full in non-streaming mode.
    pub dropped_samples: usize,
    /// Column order chosen when the CSV header was first written. Used by the
    /// direct-write path; the streaming path derives its columns from the union
    /// of all segments at the end.
    csv_columns: Option<Vec<String>>,
    /// Number of rows already appended to the CSV file.
    rows_written: usize,
    /// Temp files holding flushed blocks, in write order.
    ///
    /// Each segment is itself a CSV with its own header (the signals present in
    /// that block), so a changing signal set is preserved rather than truncated.
    segments: Vec<Segment>,
    /// Monotonic counter used to name segment files uniquely.
    segment_sequence: u64,
}

/// One flushed block of samples, parked on disk until the final write.
#[derive(Debug, Clone)]
struct Segment {
    path: std::path::PathBuf,
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
            csv_columns: None,
            rows_written: 0,
            segments: Vec::new(),
            segment_sequence: 0,
        }
    }

    /// Record one sample. When streaming is enabled and the buffer is full,
    /// the buffer is appended to disk and reset.
    ///
    /// Streaming writes each full buffer to its own segment file. The final
    /// `flush_to_disk`/`export_csv` merges the segments and takes the **union**
    /// of the signals seen across all of them, so a signal that appears after
    /// the first flush is still written (earlier rows read back as `NaN`) rather
    /// than being dropped. Data loss is not an acceptable way to keep the file
    /// rectangular.
    pub fn record(&mut self, time: Scalar, signals: &HashMap<String, Scalar>) {
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

        // Streaming: flush this block to disk and reset, which is what bounds
        // memory. The column set is frozen by the first block (a CSV header
        // cannot be extended once rows follow it); any signal appearing later is
        // still written, as a NaN-filled trailing column.
        if self.config.enable_streaming && self.current_count >= self.config.max_samples {
            self.flush_buffer();
        }
    }

    /// Write the buffered block to a fresh segment and reset the buffer.
    ///
    /// Each segment carries only the signals present in this block, so a
    /// changing signal set is preserved. `flush_to_disk` merges the segments at
    /// the end; nothing is lost by deferring.
    fn flush_buffer(&mut self) {
        let Some(output) = self.config.output_path.clone() else {
            // No path configured: there is nowhere to stream to, so keep the
            // data rather than silently discarding it.
            return;
        };
        self.segment_sequence += 1;
        let segment =
            std::path::PathBuf::from(format!("{}.part{}.csv", output, self.segment_sequence));
        let columns: Vec<String> = self.signal_names().into_iter().cloned().collect();

        match self.write_segment(&segment, &columns) {
            Ok(()) => {
                self.total_written += self.current_count;
                self.flush_count += 1;
                self.segments.push(Segment { path: segment });
            }
            Err(e) => {
                // A failed flush must not lose the data silently: report it and
                // count the samples as dropped so the caller can detect the loss.
                eprintln!("[WARN] DataRecorder: streaming flush failed: {}", e);
                self.dropped_samples += self.current_count;
            }
        }

        self.time_stamps.clear();
        self.recorded_data.clear();
        self.current_count = 0;
    }

    /// Write the current buffer to `path` with the given columns, overwriting.
    fn write_segment(&self, path: &std::path::Path, columns: &[String]) -> Result<(), String> {
        use std::io::Write;
        let mut file = std::fs::File::create(path).map_err(|e| format!("Create error: {}", e))?;

        write!(file, "time").map_err(|e| format!("Write error: {}", e))?;
        for name in columns {
            write!(file, ",{}", name).map_err(|e| format!("Write error: {}", e))?;
        }
        writeln!(file).map_err(|e| format!("Write error: {}", e))?;

        for i in 0..self.time_stamps.len() {
            write!(file, "{}", self.time_stamps[i]).map_err(|e| format!("Write error: {}", e))?;
            for name in columns {
                let value = self
                    .recorded_data
                    .get(name)
                    .and_then(|data| data.get(i))
                    .copied()
                    .unwrap_or(Scalar::NAN);
                write!(file, ",{}", value).map_err(|e| format!("Write error: {}", e))?;
            }
            writeln!(file).map_err(|e| format!("Write error: {}", e))?;
        }
        Ok(())
    }

    /// Read every flushed segment, then write one rectangular CSV at `path`.
    ///
    /// The column set is the **union** of every segment's signals, so a signal
    /// that appeared mid-run gets a real column and the rows before its first
    /// sample carry NaN. Rows keep their chronological order because segments
    /// were written in flush order. The segment files are removed afterwards.
    fn merge_segments(&mut self, path: &str) -> Result<(), String> {
        use std::io::Write;

        // Union of all columns across segments, sorted for deterministic output.
        let mut all_columns: Vec<String> = Vec::new();
        let mut segments_data: Vec<(Vec<String>, Vec<(Scalar, Vec<Scalar>)>)> = Vec::new();

        for segment in &self.segments {
            let text = std::fs::read_to_string(&segment.path)
                .map_err(|e| format!("Segment read error: {}", e))?;
            let mut lines = text.lines().filter(|l| !l.trim().is_empty());
            let Some(header) = lines.next() else {
                continue;
            };
            let mut names: Vec<String> = header.split(',').skip(1).map(|s| s.to_string()).collect();
            names.sort();
            for name in &names {
                if !all_columns.contains(name) {
                    all_columns.push(name.clone());
                }
            }

            let mut rows = Vec::new();
            for line in lines {
                let mut fields = line.split(',');
                let Some(time_str) = fields.next() else {
                    continue;
                };
                let time: Scalar = time_str
                    .parse()
                    .map_err(|e| format!("Segment time parse error: {}", e))?;
                // Values in segment-column order.
                let values: Vec<Scalar> = fields
                    .map(|f| f.parse::<Scalar>().unwrap_or(Scalar::NAN))
                    .collect();
                rows.push((time, values));
            }
            segments_data.push((names, rows));
        }
        all_columns.sort();

        let mut file = std::fs::File::create(path).map_err(|e| format!("Create error: {}", e))?;
        write!(file, "time").map_err(|e| format!("Write error: {}", e))?;
        for name in &all_columns {
            write!(file, ",{}", name).map_err(|e| format!("Write error: {}", e))?;
        }
        writeln!(file).map_err(|e| format!("Write error: {}", e))?;

        // Emit rows in segment (i.e. flush) order, resolving each column by name
        // so a signal absent from this segment becomes NaN.
        for (names, rows) in &segments_data {
            // Map column name -> index within this segment's value vector.
            let index_of: Vec<Option<usize>> = all_columns
                .iter()
                .map(|c| names.iter().position(|n| n == c))
                .collect();
            for (time, values) in rows {
                write!(file, "{}", time).map_err(|e| format!("Write error: {}", e))?;
                for idx in &index_of {
                    let v = idx
                        .and_then(|i| values.get(i).copied())
                        .unwrap_or(Scalar::NAN);
                    write!(file, ",{}", v).map_err(|e| format!("Write error: {}", e))?;
                }
                writeln!(file).map_err(|e| format!("Write error: {}", e))?;
            }
        }

        // Clean up the intermediate files; their content is now in `path`.
        for segment in &self.segments {
            let _ = std::fs::remove_file(&segment.path);
        }
        self.segments.clear();
        Ok(())
    }

    /// Write the header and every buffered row to `path`.
    fn write_csv(&mut self, path: &str, columns: &[String]) -> Result<(), String> {
        use std::io::Write;
        if self.time_stamps.is_empty() {
            return Ok(());
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| format!("Open error: {}", e))?;
        // Determine emptiness *before* the writer borrows `file`.
        let is_empty = file.metadata().map(|m| m.len()).unwrap_or(0) == 0;
        let mut writer = std::io::BufWriter::new(&mut file);

        // Write the header only when the file is new: a caller appending to an
        // existing file must not get a second header line.
        if is_empty {
            write!(writer, "time").map_err(|e| format!("Write error: {}", e))?;
            for name in columns {
                write!(writer, ",{}", name).map_err(|e| format!("Write error: {}", e))?;
            }
            writeln!(writer).map_err(|e| format!("Write error: {}", e))?;
        }

        for i in 0..self.time_stamps.len() {
            write!(writer, "{}", self.time_stamps[i]).map_err(|e| format!("Write error: {}", e))?;
            for name in columns {
                let value = self
                    .recorded_data
                    .get(name)
                    .and_then(|data| data.get(i))
                    .copied()
                    .unwrap_or(Scalar::NAN);
                write!(writer, ",{}", value).map_err(|e| format!("Write error: {}", e))?;
            }
            writeln!(writer).map_err(|e| format!("Write error: {}", e))?;
        }

        writer.flush().map_err(|e| format!("Flush error: {}", e))?;
        self.rows_written += self.time_stamps.len();
        Ok(())
    }

    /// Write all recorded data to the configured output path.
    ///
    /// This is the single write point. When streaming flushed segments to disk
    /// during the run, they are merged here into one rectangular CSV whose
    /// columns are the union of every segment's signals; otherwise the in-memory
    /// buffer is written directly.
    pub fn flush_to_disk(&mut self) -> Result<(), String> {
        let Some(path) = self.config.output_path.clone() else {
            return Ok(());
        };

        if self.segments.is_empty() {
            // Nothing was streamed: write the buffer as-is.
            if self.time_stamps.is_empty() {
                return Ok(());
            }
            let columns: Vec<String> = self.signal_names().into_iter().cloned().collect();
            return self.write_csv(&path, &columns);
        }

        // Streaming ran: merge the segments (and any tail still buffered) into a
        // single lossless file.
        if !self.time_stamps.is_empty() {
            self.segment_sequence += 1;
            let segment =
                std::path::PathBuf::from(format!("{}.part{}.csv", path, self.segment_sequence));
            let columns: Vec<String> = self.signal_names().into_iter().cloned().collect();
            self.write_segment(&segment, &columns)?;
            self.segments.push(Segment { path: segment });
            self.time_stamps.clear();
            self.recorded_data.clear();
            self.current_count = 0;
        }
        self.merge_segments(&path)
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

    /// Discard all buffered samples and reset the counters.
    ///
    /// This is a full reset: unlike the previous implementation it does not
    /// increment `total_written`/`flush_count`, which made calling `clear()`
    /// for its documented purpose corrupt the statistics.
    pub fn clear(&mut self) {
        // Remove any unmerged segment files so a reset does not leak them.
        for segment in &self.segments {
            let _ = std::fs::remove_file(&segment.path);
        }
        self.segments.clear();
        self.segment_sequence = 0;

        self.time_stamps.clear();
        self.recorded_data.clear();
        self.current_count = 0;
        self.total_written = 0;
        self.flush_count = 0;
        self.dropped_samples = 0;
        // Forget the pinned columns too: otherwise a later run would be written
        // against this run's columns, turning its signals into all-NaN columns
        // and dropping the new ones.
        self.csv_columns = None;
        self.rows_written = 0;
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

    /// Append the recorded data to a CSV file.
    ///
    /// When streaming has already flushed segments, they are merged here so the
    /// result is identical to what [`Self::flush_to_disk`] would write: one
    /// rectangular file whose columns are the union of every segment's signals.
    /// Otherwise the in-memory buffer is written directly.
    pub fn export_csv(&mut self, filepath: &str) -> Result<(), String> {
        if self.segments.is_empty() {
            if self.time_stamps.is_empty() {
                return Ok(());
            }
            let columns: Vec<String> = self.signal_names().into_iter().cloned().collect();
            return self.write_csv(filepath, &columns);
        }

        if !self.time_stamps.is_empty() {
            self.segment_sequence += 1;
            let segment =
                std::path::PathBuf::from(format!("{}.part{}.csv", filepath, self.segment_sequence));
            let columns: Vec<String> = self.signal_names().into_iter().cloned().collect();
            self.write_segment(&segment, &columns)?;
            self.segments.push(Segment { path: segment });
            self.time_stamps.clear();
            self.recorded_data.clear();
            self.current_count = 0;
        }
        self.merge_segments(filepath)
    }
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

    /// In streaming mode a signal that appears **after** the first flush must
    /// still get its own column and its samples must reach the file.
    ///
    /// Freezing the column set at the first flush keeps the file rectangular but
    /// silently drops every later signal. Segments avoid that: the final merge
    /// takes the union of all signals, so nothing is lost and the file is still
    /// rectangular (earlier rows carry NaN for the late signal).
    #[test]
    fn test_streaming_csv_keeps_a_signal_that_appears_after_the_first_flush() {
        let path = std::env::temp_dir()
            .join(format!("scico_rec_stream_{}.csv", std::process::id()))
            .to_string_lossy()
            .into_owned();
        let _ = std::fs::remove_file(&path);

        let cfg = RecorderConfig {
            max_samples: 2,
            enable_streaming: true,
            output_path: Some(path.clone()),
            ..Default::default()
        };
        let mut r = DataRecorder::new(cfg);

        // Flush 1: only signal `a` exists.
        for i in 0..2 {
            let mut s = HashMap::new();
            s.insert("a".to_string(), i as Scalar);
            r.record(i as Scalar, &s);
        }
        // Flush 2: `b` appears only now.
        for i in 0..2 {
            let mut s = HashMap::new();
            s.insert("a".to_string(), (i + 10) as Scalar);
            s.insert("b".to_string(), (i + 20) as Scalar);
            r.record((i + 2) as Scalar, &s);
        }
        r.flush_to_disk().unwrap();

        let text = std::fs::read_to_string(&path).expect("the streamed CSV must exist");
        let _ = std::fs::remove_file(&path);

        let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(lines.len(), 5, "header + 4 rows, got:\n{text}");
        assert_eq!(lines[0], "time,a,b", "both signals must have a column");

        // Rectangular: every row has the same field count as the header.
        for (i, line) in lines.iter().enumerate().skip(1) {
            assert_eq!(
                line.split(',').count(),
                3,
                "row {i} disagrees with the header:\n{text}"
            );
        }

        // The late signal's samples must be present, not dropped.
        assert_eq!(
            lines[1], "0,0,NaN",
            "`b` has no sample yet: NaN, not absent"
        );
        assert_eq!(lines[2], "1,1,NaN");
        assert_eq!(lines[3], "2,10,20", "`b` must carry its real value");
        assert_eq!(lines[4], "3,11,21");
    }

    /// A signal that disappears mid-run must keep its column, with NaN for the
    /// rows where it is absent.
    #[test]
    fn test_streaming_csv_keeps_a_signal_that_disappears_mid_run() {
        let path = std::env::temp_dir()
            .join(format!("scico_rec_gone_{}.csv", std::process::id()))
            .to_string_lossy()
            .into_owned();
        let _ = std::fs::remove_file(&path);

        let cfg = RecorderConfig {
            max_samples: 2,
            enable_streaming: true,
            output_path: Some(path.clone()),
            ..Default::default()
        };
        let mut r = DataRecorder::new(cfg);

        // Flush 1: `a` and `b`.
        for i in 0..2 {
            let mut s = HashMap::new();
            s.insert("a".to_string(), i as Scalar);
            s.insert("b".to_string(), (i + 100) as Scalar);
            r.record(i as Scalar, &s);
        }
        // Flush 2: only `a`.
        for i in 0..2 {
            let mut s = HashMap::new();
            s.insert("a".to_string(), (i + 10) as Scalar);
            r.record((i + 2) as Scalar, &s);
        }
        r.flush_to_disk().unwrap();

        let text = std::fs::read_to_string(&path).expect("the streamed CSV must exist");
        let _ = std::fs::remove_file(&path);
        let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();

        assert_eq!(lines[0], "time,a,b", "`b` keeps its column after it stops");
        assert_eq!(lines[1], "0,0,100");
        assert_eq!(lines[2], "1,1,101");
        assert_eq!(
            lines[3], "2,10,NaN",
            "`b` is absent here: NaN, not truncated"
        );
        assert_eq!(lines[4], "3,11,NaN");
    }

    /// The key streaming guarantee: when the signal set *is* known from the
    /// start, every sample reaches the file (no loss), and memory stays bounded
    /// by `max_samples`.
    #[test]
    fn test_streaming_is_bounded_and_lossless_for_a_known_signal_set() {
        let path = std::env::temp_dir()
            .join(format!("scico_rec_bounded_{}.csv", std::process::id()))
            .to_string_lossy()
            .into_owned();
        let _ = std::fs::remove_file(&path);

        let cfg = RecorderConfig {
            max_samples: 10,
            enable_streaming: true,
            output_path: Some(path.clone()),
            ..Default::default()
        };
        let mut r = DataRecorder::new(cfg);

        let total = 1000usize;
        for i in 0..total {
            let mut s = HashMap::new();
            s.insert("x".to_string(), i as Scalar);
            r.record(i as Scalar, &s);
        }
        r.flush_to_disk().unwrap();

        // Bounded memory: the in-memory buffer never exceeded `max_samples`.
        assert!(
            r.time_stamps.len() <= 10,
            "the streaming buffer must stay bounded, got {}",
            r.time_stamps.len()
        );
        // No loss: every sample was written.
        assert_eq!(
            r.dropped_samples(),
            0,
            "a known signal set must not lose samples"
        );
        assert_eq!(
            r.total_written(),
            total,
            "all samples must be accounted for"
        );

        let text = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        let data_rows = text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count()
            .saturating_sub(1);
        assert_eq!(data_rows, total, "every sample must reach the file");
        assert!(text.contains("999"), "the final sample must be present");
    }

    /// `export_csv` and streaming must agree on the merged, rectangular layout.
    #[test]
    fn test_export_csv_merges_streamed_segments_into_one_rectangle() {
        let path = std::env::temp_dir()
            .join(format!("scico_rec_mix_{}.csv", std::process::id()))
            .to_string_lossy()
            .into_owned();
        let _ = std::fs::remove_file(&path);

        let cfg = RecorderConfig {
            max_samples: 2,
            enable_streaming: true,
            output_path: Some(path.clone()),
            ..Default::default()
        };
        let mut r = DataRecorder::new(cfg);

        // One streaming flush with `a` only.
        for i in 0..2 {
            let mut s = HashMap::new();
            s.insert("a".to_string(), i as Scalar);
            r.record(i as Scalar, &s);
        }
        // Then a signal appears and the tail is written via `export_csv`.
        for i in 0..2 {
            let mut s = HashMap::new();
            s.insert("a".to_string(), (i + 5) as Scalar);
            s.insert("c".to_string(), (i + 7) as Scalar);
            r.record((i + 2) as Scalar, &s);
        }
        r.export_csv(&path).unwrap();

        let text = std::fs::read_to_string(&path).expect("the CSV must exist");
        let _ = std::fs::remove_file(&path);
        let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
        let header_cols = lines[0].split(',').count();
        for (i, line) in lines.iter().enumerate().skip(1) {
            assert_eq!(
                line.split(',').count(),
                header_cols,
                "row {i} from export_csv disagrees with the streamed header:\n{text}"
            );
        }
    }

    /// `clear()` must discard the streaming state, otherwise a reused recorder
    /// leaks stale segment files (and writes the previous run's columns).
    #[test]
    fn test_clear_drops_streaming_state() {
        let dir = std::env::temp_dir();
        let path = dir
            .join(format!("scico_clear_probe_{}.csv", std::process::id()))
            .to_string_lossy()
            .into_owned();

        let cfg = RecorderConfig {
            max_samples: 1,
            enable_streaming: true,
            output_path: Some(path.clone()),
            ..Default::default()
        };
        let mut r = DataRecorder::new(cfg);
        let mut s = HashMap::new();
        s.insert("alpha".to_string(), 1.0);
        // Two flushes -> two `.partN.csv` files exist on disk.
        for i in 0..2 {
            r.record(i as Scalar, &s);
        }
        assert!(
            !r.segments.is_empty(),
            "a streaming flush must have produced a segment file"
        );
        let segment_paths: Vec<_> = r.segments.iter().map(|s| s.path.clone()).collect();

        r.clear();

        assert!(r.csv_columns.is_none(), "stale columns after clear()");
        assert!(r.segments.is_empty(), "stale segments after clear()");
        assert_eq!(r.segment_sequence, 0, "the segment counter must reset");
        for segment_path in segment_paths {
            assert!(
                !segment_path.exists(),
                "clear() leaked the segment file {}",
                segment_path.display()
            );
        }
        let _ = std::fs::remove_file(&path);
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
