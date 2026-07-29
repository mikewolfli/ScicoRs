//! Report generation and data export.

use super::visualization::ChartGenerator;
use super::recorder::DataRecorder;

/// A section within a simulation report.
pub struct ReportSection {
    pub title: String,
    pub content: String,
    pub tables: Vec<ReportTable>,
    pub charts: Vec<ChartGenerator>,
}

impl ReportSection {
    pub fn new(title: &str, content: &str) -> Self {
        Self { title: title.to_string(), content: content.to_string(), tables: Vec::new(), charts: Vec::new() }
    }
}

/// A table within a report section.
pub struct ReportTable {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub caption: String,
}

impl ReportTable {
    pub fn new(headers: Vec<String>, caption: &str) -> Self { Self { headers, rows: Vec::new(), caption: caption.to_string() } }
    pub fn add_row(&mut self, row: Vec<String>) { self.rows.push(row); }
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
        Self { title: title.to_string(), description: description.to_string(), sections: Vec::new(), generated_at: chrono_now() }
    }
    pub fn add_section(&mut self, section: ReportSection) { self.sections.push(section); }

    pub fn to_markdown(&self) -> String {
        let mut md = format!("# {}\n\n{}\n\n", self.title, self.description);
        for sec in &self.sections {
            md.push_str(&format!("## {}\n\n{}\n\n", sec.title, sec.content));
            for table in &sec.tables {
                md.push_str(&format!("**{}**\n\n", table.caption));
                md.push_str("| "); for h in &table.headers { md.push_str(&format!("{} | ", h)); } md.push('\n');
                md.push_str("| "); for _ in &table.headers { md.push_str("--- | "); } md.push('\n');
                for row in &table.rows {
                    md.push_str("| "); for cell in row { md.push_str(&format!("{} | ", cell)); } md.push('\n');
                }
                md.push('\n');
            }
        }
        md.push_str(&format!("_Generated: {}_\n", self.generated_at));
        md
    }

    pub fn to_html(&self) -> String {
        let mut html = format!("<!DOCTYPE html><html><head><title>{}</title></head><body>", self.title);
        html.push_str(&format!("<h1>{}</h1><p>{}</p>", self.title, self.description));
        for sec in &self.sections {
            html.push_str(&format!("<h2>{}</h2><p>{}</p>", sec.title, sec.content));
            for table in &sec.tables {
                html.push_str(&format!("<table><caption>{}</caption><thead><tr>", table.caption));
                for h in &table.headers { html.push_str(&format!("<th>{}</th>", h)); }
                html.push_str("</tr></thead><tbody>");
                for row in &table.rows {
                    html.push_str("<tr>"); for cell in row { html.push_str(&format!("<td>{}</td>", cell)); } html.push_str("</tr>");
                }
                html.push_str("</tbody></table>");
            }
        }
        html.push_str(&format!("<p><em>Generated: {}</em></p>", self.generated_at));
        html.push_str("</body></html>");
        html
    }

    pub fn to_json(&self) -> String {
        let mut json = format!("{{\"title\":\"{}\",\"description\":\"{}\",\"sections\":[", self.title, self.description);
        for (i, sec) in self.sections.iter().enumerate() {
            if i > 0 { json.push(','); }
            json.push_str(&format!("{{\"title\":\"{}\",\"content\":\"{}\"}}", sec.title, sec.content));
        }
        json.push_str("]}");
        json
    }
}

fn chrono_now() -> String {
    "2026-07-29T12:00:00Z".to_string()
}

/// Supported export formats.
pub enum ExportFormat { Csv, Json, Toml, Hdf5, Vtk }

/// Data exporter utility.
pub struct DataExporter;

impl DataExporter {
    pub fn export(recorder: &DataRecorder, format: ExportFormat, path: &str) -> Result<(), String> {
        match format {
            ExportFormat::Csv => recorder.export_csv(path),
            ExportFormat::Json => {
                let mut map = serde_json::Map::new();
                for name in recorder.signal_names() {
                    if let Some(data) = recorder.get_timeseries(name) {
                        map.insert(name.clone(), serde_json::Value::from(data.to_vec()));
                    }
                }
                let json = serde_json::to_string_pretty(&map).map_err(|e| format!("JSON error: {}", e))?;
                std::fs::write(path, &json).map_err(|e| format!("Write error: {}", e))
            }
            _ => Err("Format not yet implemented".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use super::super::recorder::{DataRecorder, RecorderConfig};
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
        let mut s = HashMap::new(); s.insert("x".to_string(), 1.0);
        r.record(0.0, &s);
        let path = "/tmp/test_export.csv";
        assert!(DataExporter::export(&r, ExportFormat::Csv, path).is_ok());
        let _ = std::fs::remove_file(path);
    }
}
