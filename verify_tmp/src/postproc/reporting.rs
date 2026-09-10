//! Report generation and data export.

use super::recorder::DataRecorder;
use super::visualization::ChartGenerator;
use crate::core::types::Scalar;

/// A section within a simulation report.
pub struct ReportSection {
    pub title: String,
    pub content: String,
    pub tables: Vec<ReportTable>,
    pub charts: Vec<ChartGenerator>,
}

impl ReportSection {
    pub fn new(title: &str, content: &str) -> Self {
        Self {
            title: title.to_string(),
            content: content.to_string(),
            tables: Vec::new(),
            charts: Vec::new(),
        }
    }
}

/// A table within a report section.
pub struct ReportTable {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub caption: String,
}

impl ReportTable {
    pub fn new(headers: Vec<String>, caption: &str) -> Self {
        Self {
            headers,
            rows: Vec::new(),
            caption: caption.to_string(),
        }
    }
    pub fn add_row(&mut self, row: Vec<String>) {
        self.rows.push(row);
    }
}

/// A complete simulation report.
pub struct SimulationReport {
    pub title: String,
    pub description: String,
    pub sections: Vec<ReportSection>,
    pub generated_at: String,
}

impl SimulationReport {
    pub fn new(title: &str, description: &str) -> Self {
        Self {
            title: title.to_string(),
            description: description.to_string(),
            sections: Vec::new(),
            generated_at: chrono_now(),
        }
    }
    pub fn add_section(&mut self, section: ReportSection) {
        self.sections.push(section);
    }

    pub fn to_markdown(&self) -> String {
        let mut md = format!("# {}\n\n{}\n\n", self.title, self.description);
        for sec in &self.sections {
            md.push_str(&format!("## {}\n\n{}\n\n", sec.title, sec.content));
            for table in &sec.tables {
                md.push_str(&format!("**{}**\n\n", table.caption));
                md.push_str("| ");
                for h in &table.headers {
                    md.push_str(&format!("{} | ", h));
                }
                md.push('\n');
                md.push_str("| ");
                for _ in &table.headers {
                    md.push_str("--- | ");
                }
                md.push('\n');
                for row in &table.rows {
                    md.push_str("| ");
                    for cell in row {
                        md.push_str(&format!("{} | ", cell));
                    }
                    md.push('\n');
                }
                md.push('\n');
            }
        }
        md.push_str(&format!("_Generated: {}_\n", self.generated_at));
        md
    }

    pub fn to_html(&self) -> String {
        let mut html = format!(
            "<!DOCTYPE html><html><head><title>{}</title></head><body>",
            self.title
        );
        html.push_str(&format!(
            "<h1>{}</h1><p>{}</p>",
            self.title, self.description
        ));
        for sec in &self.sections {
            html.push_str(&format!("<h2>{}</h2><p>{}</p>", sec.title, sec.content));
            for table in &sec.tables {
                html.push_str(&format!(
                    "<table><caption>{}</caption><thead><tr>",
                    table.caption
                ));
                for h in &table.headers {
                    html.push_str(&format!("<th>{}</th>", h));
                }
                html.push_str("</tr></thead><tbody>");
                for row in &table.rows {
                    html.push_str("<tr>");
                    for cell in row {
                        html.push_str(&format!("<td>{}</td>", cell));
                    }
                    html.push_str("</tr>");
                }
                html.push_str("</tbody></table>");
            }
        }
        html.push_str(&format!("<p><em>Generated: {}</em></p>", self.generated_at));
        html.push_str("</body></html>");
        html
    }

    pub fn to_json(&self) -> String {
        let mut json = format!(
            "{{\"title\":\"{}\",\"description\":\"{}\",\"sections\":[",
            self.title, self.description
        );
        for (i, sec) in self.sections.iter().enumerate() {
            if i > 0 {
                json.push(',');
            }
            json.push_str(&format!(
                "{{\"title\":\"{}\",\"content\":\"{}\"}}",
                sec.title, sec.content
            ));
        }
        json.push_str("]}");
        json
    }
}

/// Return the current wall-clock time as an RFC 3339 UTC timestamp
/// (e.g. `2026-09-10T14:23:45.123456789Z`).
///
/// Computed entirely from [`std::time::SystemTime`] — no date/time dependency
/// is required. The conversion uses the standard civil-calendar algorithms
/// (Howard Hinnant's `civil_from_days`), which are exact for the whole range
/// representable by `i64` seconds. If the host clock reports a time before the
/// Unix epoch, the timestamp is prefixed with a `-` sign (the instant itself is
/// still the true reading of the system clock).
fn chrono_now() -> String {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(since_epoch) => format_rfc3339(since_epoch.as_secs(), since_epoch.subsec_nanos(), false),
        Err(before_epoch) => {
            let d = before_epoch.duration();
            // `d` is how far `now` lies *before* the epoch, so the instant is
            // epoch - d. When `now` is an exact whole-second instant before the
            // epoch, `subsec_nanos()` is 0 and there is no fractional borrow.
            format_rfc3339(d.as_secs(), d.subsec_nanos(), true)
        }
    }
}

/// Format an instant expressed as a signed offset from the Unix epoch in
/// seconds plus nanoseconds, as an RFC 3339 UTC timestamp.
fn format_rfc3339(secs_from_epoch: u64, nanos: u32, negative: bool) -> String {
    // Reduce to the signed civil decomposition. `days` may be negative for
    // pre-epoch instants; the remainder is always in `0..86_400`.
    let secs_i64 = secs_from_epoch.min(i64::MAX as u64) as i64;
    let days = secs_i64.div_euclid(86_400);
    let secs_of_day = secs_i64.rem_euclid(86_400);

    let (year, month, day) = civil_from_days(days);
    let hour = secs_of_day / 3_600;
    let minute = (secs_of_day % 3_600) / 60;
    let second = secs_of_day % 60;

    let sign = if negative { "-" } else { "" };
    if nanos == 0 {
        format!(
            "{}{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            sign, year, month, day, hour, minute, second
        )
    } else {
        let fraction = format!("{:09}", nanos);
        let fraction = fraction.trim_end_matches('0');
        format!(
            "{}{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{}Z",
            sign, year, month, day, hour, minute, second, fraction
        )
    }
}

/// Convert a day count relative to 1970-01-01 into a proleptic Gregorian
/// `(year, month, day)` triple. Algorithm from Howard Hinnant's
/// `chrono`-compatible date algorithms; valid for the full `i64` day range.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097); // day-of-era [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // year-of-era [0, 399]
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day-of-year [0, 365]
    let mp = (5 * doy + 2) / 153; // month index, March = 0
    let day = doy - (153 * mp + 2) / 5 + 1; // day [1, 31]
    let month = if mp < 10 { mp + 3 } else { mp - 9 }; // month [1, 12]
    (year + i64::from(month <= 2), month as u32, day as u32)
}

/// Supported export formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// Comma-separated values, one row per recorded sample.
    Csv,
    /// Schema-free JSON object mapping signal name to an array of samples.
    Json,
    /// TOML table mapping signal name to an array of samples.
    Toml,
    /// Legacy ASCII VTK structured-points time series.
    Vtk,
}

impl ExportFormat {
    /// The formats this exporter can actually produce, for error messages and
    /// capability discovery. Every variant is listed, so the error text can
    /// never advertise a format the exporter would refuse.
    pub const SUPPORTED: [ExportFormat; 4] = [
        ExportFormat::Csv,
        ExportFormat::Json,
        ExportFormat::Toml,
        ExportFormat::Vtk,
    ];

    /// Human-readable name used in error messages.
    pub fn name(self) -> &'static str {
        match self {
            ExportFormat::Csv => "Csv",
            ExportFormat::Json => "Json",
            ExportFormat::Toml => "Toml",
            ExportFormat::Vtk => "Vtk",
        }
    }
}

/// Data exporter utility.
///
/// Every [`ExportFormat`] variant is fully implemented; there are no
/// advertised-but-missing formats.
pub struct DataExporter;

impl DataExporter {
    pub fn export(recorder: &DataRecorder, format: ExportFormat, path: &str) -> Result<(), String> {
        match format {
            ExportFormat::Csv => recorder.export_csv(path),
            ExportFormat::Json => {
                let map = signal_map(recorder);
                let json =
                    serde_json::to_string_pretty(&map).map_err(|e| format!("JSON error: {}", e))?;
                std::fs::write(path, &json).map_err(|e| format!("Write error: {}", e))
            }
            ExportFormat::Toml => {
                let map = signal_map(recorder);
                let toml =
                    toml::to_string_pretty(&map).map_err(|e| format!("TOML error: {}", e))?;
                std::fs::write(path, &toml).map_err(|e| format!("Write error: {}", e))
            }
            ExportFormat::Vtk => export_vtk(recorder, path),
        }
    }
}

/// Build the `signal name -> samples` map shared by the JSON and TOML
/// exporters, so both formats carry byte-for-byte identical data.
fn signal_map(recorder: &DataRecorder) -> std::collections::BTreeMap<String, Vec<Scalar>> {
    let mut map = std::collections::BTreeMap::new();
    for name in recorder.signal_names() {
        if let Some(data) = recorder.get_timeseries(name) {
            map.insert(name.clone(), data.to_vec());
        }
    }
    map
}

/// Export recorded time series as a legacy ASCII VTK structured-points grid.
///
/// Points are the sampled instants `(time, 0, 0)`; each recorded signal becomes
/// a scalar point-data array named after the signal. This is valid legacy VTK
/// (`.vtk`) that ParaView and VisIt read directly, and is exact for 1-D time
/// series: sample `i` of every array corresponds to point `i`.
fn export_vtk(recorder: &DataRecorder, path: &str) -> Result<(), String> {
    let names = recorder.signal_names();
    let times = &recorder.time_stamps;
    let n = times.len();

    let mut vtk = String::new();
    vtk.push_str("# vtk DataFile Version 3.0\n");
    vtk.push_str("ScicoRs time-series export\n");
    vtk.push_str("ASCII\n");
    vtk.push_str("DATASET STRUCTURED_POINTS\n");
    vtk.push_str(&format!("DIMENSIONS {} 1 1\n", n));
    vtk.push_str("ORIGIN 0 0 0\n");
    vtk.push_str("SPACING 1 1 1\n");
    vtk.push_str(&format!("POINT_DATA {}\n", n));

    // Point coordinate proxy: the recorded time stamps themselves.
    vtk.push_str("SCALARS time double 1\n");
    vtk.push_str("LOOKUP_TABLE default\n");
    for t in times {
        vtk.push_str(&format!("{}\n", t));
    }

    for name in names {
        vtk.push_str(&format!("SCALARS {} double 1\n", name));
        vtk.push_str("LOOKUP_TABLE default\n");
        if let Some(data) = recorder.get_timeseries(name) {
            for v in data {
                vtk.push_str(&format!("{}\n", v));
            }
        }
    }

    std::fs::write(path, &vtk).map_err(|e| format!("Write error: {}", e))
}

/// Test helper: inverse of [`format_rfc3339`] for UTC timestamps, returning
/// whole seconds since the Unix epoch. Only the shapes this module produces
/// (`YYYY-MM-DDThh:mm:ss[.fff]Z`) are accepted.
#[cfg(test)]
fn parse_rfc3339_to_epoch_seconds(ts: &str) -> u64 {
    let ts = ts.trim_end_matches('Z');
    let (date, time) = ts.split_once('T').expect("timestamp must contain 'T'");
    let mut dparts = date.split('-');
    let year: i64 = dparts.next().unwrap().parse().unwrap();
    let month: i64 = dparts.next().unwrap().parse().unwrap();
    let day: i64 = dparts.next().unwrap().parse().unwrap();
    let time = time.split('.').next().unwrap();
    let mut tparts = time.split(':');
    let hour: i64 = tparts.next().unwrap().parse().unwrap();
    let minute: i64 = tparts.next().unwrap().parse().unwrap();
    let second: i64 = tparts.next().unwrap().parse().unwrap();
    // days_from_civil (Howard Hinnant), inverse of `civil_from_days`.
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let secs = days * 86_400 + hour * 3_600 + minute * 60 + second;
    u64::try_from(secs).expect("test timestamp must be after the Unix epoch")
}

#[cfg(test)]
mod tests {
    use super::super::recorder::{DataRecorder, RecorderConfig};
    use super::*;
    use std::collections::HashMap;
    #[test]
    fn test_report_creation() {
        let r = SimulationReport::new("Test", "A test report");
        assert_eq!(r.title, "Test");
    }
    #[test]
    fn test_report_markdown() {
        let mut r = SimulationReport::new("Report", "Desc");
        r.add_section(ReportSection::new("Results", "Data"));
        let md = r.to_markdown();
        assert!(md.contains("# Report"));
        assert!(md.contains("Results"));
    }
    #[test]
    fn test_report_html() {
        let mut r = SimulationReport::new("R", "D");
        r.add_section(ReportSection::new("S", "C"));
        let html = r.to_html();
        assert!(html.contains("<h1>R</h1>"));
    }
    #[test]
    fn test_report_json() {
        let mut r = SimulationReport::new("R", "D");
        r.add_section(ReportSection::new("S", "C"));
        let json = r.to_json();
        assert!(json.contains("\"title\":\"R\""));
    }
    #[test]
    fn test_table_creation() {
        let mut t = ReportTable::new(vec!["A".to_string(), "B".to_string()], "Table");
        t.add_row(vec!["1".to_string(), "2".to_string()]);
        assert_eq!(t.rows.len(), 1);
    }
    #[test]
    fn test_data_exporter_csv() {
        let mut r = DataRecorder::new(RecorderConfig::default());
        let mut s = HashMap::new();
        s.insert("x".to_string(), 1.0);
        r.record(0.0, &s);
        let path = "/tmp/test_export.csv";
        assert!(DataExporter::export(&r, ExportFormat::Csv, path).is_ok());
        let _ = std::fs::remove_file(path);
    }

    /// Build a recorder holding two samples of a single signal, for the
    /// format-specific export tests.
    fn sample_recorder() -> DataRecorder {
        let mut r = DataRecorder::new(RecorderConfig::default());
        let mut s = HashMap::new();
        s.insert("x".to_string(), 1.5);
        r.record(0.0, &s);
        s.insert("x".to_string(), 2.5);
        r.record(0.5, &s);
        r
    }

    // ── Fix (5): real timestamps ────────────────────────────────────────

    #[test]
    fn test_chrono_now_tracks_real_time_and_is_well_formed() {
        // The timestamp must be the real wall clock, not a fixed literal, and
        // it must be a valid RFC 3339 UTC instant no earlier than the moment
        // this test started.
        use std::time::{SystemTime, UNIX_EPOCH};

        let before = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is before the Unix epoch");
        let before_secs = before.as_secs();
        let ts_before = chrono_now();
        let after = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is before the Unix epoch");
        let after_secs = after.as_secs();
        let ts_after = chrono_now();

        assert!(
            ts_before.ends_with('Z'),
            "timestamp must be UTC (ends with Z): {}",
            ts_before
        );
        assert!(
            ts_before.len() >= 20,
            "timestamp must carry a full date and time: {}",
            ts_before
        );

        // Parse the year out of the timestamp and confirm it is a plausible
        // current year — a hardcoded literal from the past decade would fail.
        let year: i64 = ts_before[..4]
            .parse()
            .expect("timestamp must start with a 4-digit year");
        assert!(
            (2020..=2100).contains(&year),
            "timestamp year {} is not a plausible current year: {}",
            year,
            ts_before
        );

        // Two calls straddling a known epoch bound must be consistent with the
        // real clock: the instants reported must sit inside the measured window.
        let epoch_of = |ts: &str| -> u64 { parse_rfc3339_to_epoch_seconds(ts) };
        let lo = epoch_of(&ts_before);
        let hi = epoch_of(&ts_after);
        assert!(
            lo >= before_secs && lo <= after_secs,
            "first call ({}) is outside the measured window [{}, {}]",
            lo,
            before_secs,
            after_secs
        );
        assert!(
            hi >= before_secs && hi <= after_secs,
            "second call ({}) is outside the measured window [{}, {}]",
            hi,
            before_secs,
            after_secs
        );
        assert!(
            hi >= lo,
            "successive calls must not go backwards ({lo} -> {hi})"
        );
    }

    #[test]
    fn test_civil_from_days_known_instants() {
        // Spot-check the calendar conversion against known dates.
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(1), (1970, 1, 2));
        // 2000-01-01 is 10957 days after the Unix epoch.
        assert_eq!(civil_from_days(10_957), (2000, 1, 1));
        // 2020-02-29 (leap day) is 18321 days after the Unix epoch.
        assert_eq!(civil_from_days(18_321), (2020, 2, 29));
        // A pre-epoch instant must round-trip to a negative day count.
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
    }

    #[test]
    fn test_report_generated_at_is_a_real_timestamp() {
        // The report must stamp itself with the live clock rather than the
        // historical hardcoded constant.
        let r = SimulationReport::new("Report", "Desc");
        assert_ne!(r.generated_at, "2026-07-29T12:00:00Z");
        let year: i64 = r.generated_at[..4]
            .parse()
            .expect("generated_at must start with a 4-digit year");
        assert!(
            (2020..=2100).contains(&year),
            "unexpected year: {}",
            r.generated_at
        );
        assert!(r.generated_at.ends_with('Z'));
        // The timestamp must be wired into the rendered output, not just the
        // struct field.
        assert!(r.to_markdown().contains(&r.generated_at));
        assert!(r.to_html().contains(&r.generated_at));
    }

    // ── Fix (6): every advertised export format is implemented ──────────

    #[test]
    fn test_export_toml_matches_json_data() {
        let r = sample_recorder();
        let json_path = "/tmp/test_export_fmt.json";
        let toml_path = "/tmp/test_export_fmt.toml";

        DataExporter::export(&r, ExportFormat::Json, json_path).unwrap();
        DataExporter::export(&r, ExportFormat::Toml, toml_path).unwrap();

        let json = std::fs::read_to_string(json_path).unwrap();
        let toml = std::fs::read_to_string(toml_path).unwrap();

        // Both formats must describe the same map of real samples.
        let from_json: std::collections::BTreeMap<String, Vec<Scalar>> =
            serde_json::from_str(&json).unwrap();
        let from_toml: std::collections::BTreeMap<String, Vec<Scalar>> =
            toml::from_str(&toml).unwrap();
        assert_eq!(from_json, from_toml);
        assert_eq!(from_toml.get("x"), Some(&vec![1.5, 2.5]));

        let _ = std::fs::remove_file(json_path);
        let _ = std::fs::remove_file(toml_path);
    }

    #[test]
    fn test_export_vtk_contains_real_series() {
        let r = sample_recorder();
        let path = "/tmp/test_export_fmt.vtk";
        DataExporter::export(&r, ExportFormat::Vtk, path).unwrap();
        let vtk = std::fs::read_to_string(path).unwrap();

        // A structurally valid legacy VTK file for the two recorded samples.
        assert!(vtk.starts_with("# vtk DataFile Version 3.0"), "{}", vtk);
        assert!(vtk.contains("ASCII"));
        assert!(vtk.contains("DATASET STRUCTURED_POINTS"));
        assert!(vtk.contains("DIMENSIONS 2 1 1"), "{}", vtk);
        assert!(vtk.contains("POINT_DATA 2"), "{}", vtk);
        assert!(vtk.contains("SCALARS x double 1"));
        // The real values must appear as data, not just as metadata.
        assert!(vtk.contains("1.5"));
        assert!(vtk.contains("2.5"));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_every_export_format_is_implemented() {
        // Guards against re-introducing advertised-but-missing formats: every
        // variant must name itself and export successfully.
        let r = sample_recorder();
        for (i, format) in ExportFormat::SUPPORTED.iter().enumerate() {
            assert!(!format.name().is_empty());
            let path = format!("/tmp/test_export_all_{}", i);
            DataExporter::export(&r, *format, &path)
                .unwrap_or_else(|e| panic!("format {} failed: {}", format.name(), e));
            let written = std::fs::read_to_string(&path).unwrap();
            assert!(
                !written.is_empty(),
                "format {} produced an empty file",
                format.name()
            );
            let _ = std::fs::remove_file(&path);
        }
    }
}
