// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Adapters between a self-describing dataset and the legacy CSV/JSON exports.
//!
//! Existing pipeline outputs (CSV time series, the `serde_json` signal map of
//! [`crate::postproc::recorder`]) stay usable: this module converts them *into*
//! a dataset, and exports a dataset back *out* to those simple formats.
//!
//! # Export is never the only storage
//!
//! A CSV or JSON file cannot represent the manifest, the units, the coordinate
//! axes or the missing-value encoding. Rather than pretending those fields do
//! not exist, [`DatasetAdapter::export_csv`] and
//! [`DatasetAdapter::export_json`] return an [`ExportOutcome`] whose
//! [`ExportOutcome::lost_fields`] enumerates **exactly** what the target format
//! could not carry. The caller can therefore warn, or keep the dataset as the
//! authoritative store and treat the export as a derived view.
//!
//! A `# ` comment header is written at the top of the CSV naming the source
//! dataset so an exported file at least points back at its authoritative
//! original.

use crate::core::types::Scalar;
use crate::postproc::dataset::reader::{DatasetReadError, DatasetReader, VariableInfo};
use crate::postproc::dataset::schema::{
    DatasetSchema, MissingValueEncoding, Precision, ResultVariable, SampleLocation, TimeAxis,
};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

/// What a legacy export could not represent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportOutcome {
    /// Path the file was written to.
    pub path: String,
    /// Names of metadata fields the target format cannot store.
    pub lost_fields: Vec<String>,
    /// Number of data rows written.
    pub rows_written: usize,
    /// Number of variables written.
    pub variables_written: usize,
}

impl ExportOutcome {
    /// Whether the export lost any metadata.
    pub fn lost_metadata(&self) -> bool {
        !self.lost_fields.is_empty()
    }
}

/// Errors produced by the adapters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterError {
    /// A dataset read failed.
    Read(String),
    /// A filesystem write failed.
    Io(String),
    /// The input text could not be parsed into a dataset.
    Parse(String),
    /// The input used a construct the adapter cannot represent.
    Unsupported(String),
    /// The dataset (or import spec) is internally inconsistent.
    Invalid(String),
}

impl std::fmt::Display for AdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AdapterError::Read(m) => write!(f, "dataset read error: {}", m),
            AdapterError::Io(m) => write!(f, "i/o error: {}", m),
            AdapterError::Parse(m) => write!(f, "parse error: {}", m),
            AdapterError::Unsupported(m) => write!(f, "unsupported: {}", m),
            AdapterError::Invalid(m) => write!(f, "invalid dataset: {}", m),
        }
    }
}

impl std::error::Error for AdapterError {}

impl From<DatasetReadError> for AdapterError {
    fn from(e: DatasetReadError) -> Self {
        AdapterError::Read(e.to_string())
    }
}

/// Metadata an import cannot recover from a bare CSV/JSON file.
///
/// The caller supplies the missing description so the import is explicit rather
/// than silently inventing units.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportSpec {
    /// Dataset name.
    pub name: String,
    /// Quantity name to record for every column.
    pub quantity: String,
    /// Unit symbol of the columns.
    pub unit_symbol: String,
    /// Scale to the SI base unit of the quantity.
    pub unit_scale: Scalar,
    /// Offset to the SI base unit.
    pub unit_offset: Scalar,
    /// Sampling location to record.
    pub location: SampleLocation,
    /// If `true`, the first column of each CSV row is the time stamp and is not
    /// stored as a variable.
    pub first_column_is_time: bool,
}

impl Default for ImportSpec {
    fn default() -> Self {
        Self {
            name: "imported".to_string(),
            quantity: "unknown".to_string(),
            unit_symbol: "1".to_string(),
            unit_scale: 1.0,
            unit_offset: 0.0,
            location: SampleLocation::Global,
            first_column_is_time: true,
        }
    }
}

impl ImportSpec {
    /// Set the quantity/unit description applied to every imported column.
    pub fn with_unit(
        mut self,
        quantity: &str,
        symbol: &str,
        scale: Scalar,
        offset: Scalar,
    ) -> Self {
        self.quantity = quantity.to_string();
        self.unit_symbol = symbol.to_string();
        self.unit_scale = scale;
        self.unit_offset = offset;
        self
    }
}

/// Converters between datasets and the legacy CSV/JSON exports.
pub struct DatasetAdapter;

impl DatasetAdapter {
    /// Export a dataset's rows to a CSV file.
    ///
    /// Layout: a leading comment block (`# key: value`) describing the source
    /// dataset, then a header `time,<var>...` (the `time` column is omitted
    /// when the schema has no time axis), then one row per data row. Missing
    /// samples are written as `NaN`.
    ///
    /// The returned [`ExportOutcome`] lists the metadata a CSV cannot hold.
    pub fn export_csv(
        reader: &DatasetReader,
        path: impl AsRef<Path>,
    ) -> Result<ExportOutcome, AdapterError> {
        let path = path.as_ref();
        let schema = reader.schema();
        let mut out = String::new();
        out.push_str("# scicors-dataset export\n");
        out.push_str(&format!(
            "# simulation_id: {}\n",
            reader.manifest().simulation_id
        ));
        out.push_str(&format!("# dataset: {}\n", schema.name));
        out.push_str(&format!("# schema_version: {}\n", schema.schema_version));
        for v in &schema.variables {
            out.push_str(&format!(
                "# variable {}: quantity={} unit={}\n",
                v.name, v.quantity, v.unit_symbol
            ));
        }

        let has_time = schema.time_axis.is_some();
        if has_time {
            out.push_str("time");
        }
        for (i, v) in schema.variables.iter().enumerate() {
            if i > 0 || has_time {
                out.push(',');
            }
            out.push_str(&v.name);
        }
        out.push('\n');

        for row in reader.rows() {
            let mut first = true;
            if has_time {
                out.push_str(&format_scalar(row.time.unwrap_or(Scalar::NAN)));
                first = false;
            }
            for v in &schema.variables {
                if !first {
                    out.push(',');
                }
                first = false;
                out.push_str(&format_scalar(row.get(&v.name)));
            }
            out.push('\n');
        }

        std::fs::write(path, &out).map_err(|e| AdapterError::Io(e.to_string()))?;

        Ok(ExportOutcome {
            path: path.to_string_lossy().into_owned(),
            lost_fields: vec![
                "manifest".to_string(),
                "per-variable units".to_string(),
                "coordinate axes".to_string(),
                "sample location".to_string(),
                "missing-value encoding".to_string(),
                "precision".to_string(),
                "chunk checksums".to_string(),
            ],
            rows_written: reader.row_count(),
            variables_written: schema.variables.len(),
        })
    }

    /// Export a dataset to the recorder's JSON signal map: an object mapping
    /// each variable name to the array of its values (`NaN` serializes as
    /// `null`).
    pub fn export_json(
        reader: &DatasetReader,
        path: impl AsRef<Path>,
    ) -> Result<ExportOutcome, AdapterError> {
        let path = path.as_ref();
        let mut map: BTreeMap<String, Vec<Scalar>> = BTreeMap::new();
        for v in &reader.schema().variables {
            map.insert(v.name.clone(), reader.variable_array(&v.name)?);
        }
        let json =
            serde_json::to_string_pretty(&map).map_err(|e| AdapterError::Parse(e.to_string()))?;
        let json = format!(
            "// scicors dataset '{}' (simulation {})\n{}",
            reader.schema().name,
            reader.manifest().simulation_id,
            json
        );
        std::fs::write(path, &json).map_err(|e| AdapterError::Io(e.to_string()))?;
        Ok(ExportOutcome {
            path: path.to_string_lossy().into_owned(),
            lost_fields: vec![
                "manifest".to_string(),
                "units".to_string(),
                "coordinate axes".to_string(),
                "missing-value encoding".to_string(),
                "precision".to_string(),
            ],
            rows_written: reader.row_count(),
            variables_written: reader.schema().variables.len(),
        })
    }

    /// Import a single-column CSV time series into a dataset.
    ///
    /// The CSV must have a header; every column after the (optional) first
    /// time column becomes a variable described by `spec`. Blank cells and the
    /// literals `nan`/`NaN` become missing samples.
    pub fn import_csv(text: &str, spec: &ImportSpec) -> Result<ImportedDataset, AdapterError> {
        let mut lines = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'));

        let header = lines
            .next()
            .ok_or_else(|| AdapterError::Parse("CSV has no header row".to_string()))?;
        let columns: Vec<&str> = header.split(',').map(str::trim).collect();
        if columns.is_empty() {
            return Err(AdapterError::Parse("CSV header is empty".to_string()));
        }
        let value_start = usize::from(spec.first_column_is_time && columns.len() > 1);
        if value_start >= columns.len() {
            return Err(AdapterError::Unsupported(
                "CSV has no value columns after the time column".to_string(),
            ));
        }
        let value_names: Vec<String> = columns[value_start..]
            .iter()
            .map(|s| s.to_string())
            .collect();

        let mut times: Vec<Scalar> = Vec::new();
        let mut values: HashMap<String, Vec<Scalar>> = HashMap::new();
        for name in &value_names {
            values.insert(name.clone(), Vec::new());
        }

        for (row_index, line) in lines.enumerate() {
            let cells: Vec<&str> = line.split(',').map(str::trim).collect();
            if cells.len() != columns.len() {
                return Err(AdapterError::Parse(format!(
                    "row {} has {} cells but the header has {}",
                    row_index + 1,
                    cells.len(),
                    columns.len()
                )));
            }
            if spec.first_column_is_time && columns.len() > 1 {
                times.push(parse_cell(cells[0])?);
            } else {
                times.push(row_index as Scalar);
            }
            for (offset, name) in value_names.iter().enumerate() {
                let cell = cells[value_start + offset];
                values
                    .get_mut(name)
                    .expect("column was inserted above")
                    .push(parse_cell(cell)?);
            }
        }

        let mut schema = DatasetSchema::new(&spec.name);
        schema.time_axis = Some(TimeAxis::uniform_seconds(times.len(), 1.0));
        if let Some(axis) = schema.time_axis.as_mut() {
            axis.values = times.clone();
        }
        for name in &value_names {
            let var = ResultVariable::scalar(name, &spec.quantity, &spec.unit_symbol)
                .with_unit(&spec.unit_symbol, spec.unit_scale, spec.unit_offset)
                .with_location(spec.location)
                .with_time_axis("time");
            schema.add_variable(var).map_err(AdapterError::Invalid)?;
        }
        schema.validate().map_err(AdapterError::Invalid)?;

        Ok(ImportedDataset {
            schema,
            times,
            values,
            lost_fields: vec![
                "model signature".to_string(),
                "run-config hash".to_string(),
                "solver record".to_string(),
                "library versions".to_string(),
                "chunk checksums".to_string(),
            ],
        })
    }

    /// Import the recorder-style JSON signal map (variable name -> values).
    ///
    /// The optional leading `// ...` comment line emitted by
    /// [`Self::export_json`] is ignored. The time axis is reconstructed as
    /// `[0, 1, ..., n-1]` seconds; the original instants are *not* recoverable
    /// from this format and are reported as lost.
    pub fn import_json(text: &str, spec: &ImportSpec) -> Result<ImportedDataset, AdapterError> {
        let cleaned: String = text
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        // `serde_json` writes `NaN` as `null`, so decode through `Option` and
        // map a missing value back onto `NaN` rather than failing the parse.
        let map: BTreeMap<String, Vec<Option<Scalar>>> = serde_json::from_str(&cleaned)
            .map_err(|e| AdapterError::Parse(format!("JSON parse error: {}", e)))?;
        if map.is_empty() {
            return Err(AdapterError::Parse("JSON signal map is empty".to_string()));
        }
        let map: BTreeMap<String, Vec<Scalar>> = map
            .into_iter()
            .map(|(k, series)| {
                (
                    k,
                    series
                        .into_iter()
                        .map(|v| v.unwrap_or(Scalar::NAN))
                        .collect(),
                )
            })
            .collect();

        let n = map.values().map(Vec::len).max().unwrap_or(0);
        for (name, series) in &map {
            if series.len() != n {
                return Err(AdapterError::Unsupported(format!(
                    "variable '{}' has {} samples but the longest series has {}",
                    name,
                    series.len(),
                    n
                )));
            }
        }

        let mut schema = DatasetSchema::with_uniform_time(&spec.name, n, 1.0);
        for name in map.keys() {
            let var = ResultVariable::scalar(name, &spec.quantity, &spec.unit_symbol)
                .with_unit(&spec.unit_symbol, spec.unit_scale, spec.unit_offset)
                .with_location(spec.location)
                .with_time_axis("time");
            schema.add_variable(var).map_err(AdapterError::Invalid)?;
        }
        schema.validate().map_err(AdapterError::Invalid)?;

        let mut values = HashMap::new();
        for (name, series) in map {
            values.insert(name, series);
        }
        let times: Vec<Scalar> = (0..n).map(|i| i as Scalar).collect();
        Ok(ImportedDataset {
            schema,
            times,
            values,
            lost_fields: vec![
                "original time instants".to_string(),
                "manifest".to_string(),
                "coordinate axes".to_string(),
                "chunk checksums".to_string(),
            ],
        })
    }
}

/// The result of importing a legacy CSV/JSON file.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedDataset {
    /// The reconstructed schema (caller may refine units before writing).
    pub schema: DatasetSchema,
    /// Time instants recovered from the file.
    pub times: Vec<Scalar>,
    /// Variable name -> values.
    pub values: HashMap<String, Vec<Scalar>>,
    /// Metadata the source format could not carry.
    pub lost_fields: Vec<String>,
}

impl ImportedDataset {
    /// Number of data rows.
    pub fn row_count(&self) -> usize {
        self.times.len()
    }

    /// Describe a variable in the reconstructed schema.
    pub fn variable(&self, name: &str) -> Option<&ResultVariable> {
        self.schema.variable(name)
    }

    /// A compact description list matching [`DatasetReader::variables`].
    pub fn variable_infos(&self) -> Vec<VariableInfo> {
        self.schema
            .variables
            .iter()
            .map(|v| VariableInfo {
                name: v.name.clone(),
                quantity: v.quantity.clone(),
                unit_symbol: v.unit_symbol.clone(),
                location: v.location,
                values_per_sample: v.values_per_sample(),
            })
            .collect()
    }
}

/// Format a scalar for CSV output, spelling missing values as `NaN`.
fn format_scalar(v: Scalar) -> String {
    if v.is_nan() {
        "NaN".to_string()
    } else {
        format!("{}", v)
    }
}

/// Parse one CSV cell into a scalar; blank cells and `nan` become `NaN`.
fn parse_cell(cell: &str) -> Result<Scalar, AdapterError> {
    let trimmed = cell.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("nan") {
        return Ok(Scalar::NAN);
    }
    trimmed
        .parse::<Scalar>()
        .map_err(|_| AdapterError::Parse(format!("'{}' is not a number", trimmed)))
}

/// The precision an adapter records for values parsed from text.
pub fn text_precision() -> Precision {
    Precision::F64
}

/// The missing-value encoding an adapter records for text imports.
pub fn text_missing_encoding() -> MissingValueEncoding {
    MissingValueEncoding::Nan
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::postproc::dataset::manifest::DatasetManifest;
    use crate::postproc::dataset::schema::{CoordinateAxis, ResultVariable, SampleLocation};
    use crate::postproc::dataset::writer::{DatasetWriter, Row};
    use std::path::PathBuf;

    fn scratch(name: &str, ext: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "scico_adapter_{}_{}_{}.{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
            ext
        ))
    }

    fn scratch_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "scico_adapter_dir_{}_{}_{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    }

    /// Build a small finished dataset for adapter tests.
    fn build_dataset(dir: &Path, n: usize) {
        let mut schema = DatasetSchema::with_uniform_time("run", n, 1.0);
        schema
            .add_coordinate(CoordinateAxis::uniform("x", 0.0, 1.0, n))
            .unwrap();
        schema
            .add_variable(
                ResultVariable::scalar("T", "temperature", "K")
                    .with_location(SampleLocation::Node)
                    .with_coordinates(&["x"])
                    .with_time_axis("time"),
            )
            .unwrap();
        schema
            .add_variable(
                ResultVariable::scalar("p", "pressure", "Pa")
                    .with_location(SampleLocation::Node)
                    .with_time_axis("time"),
            )
            .unwrap();
        let mut w =
            DatasetWriter::create(dir, schema, DatasetManifest::new("sim-adapter")).unwrap();
        for i in 0..n {
            let mut r = Row::at(i as Scalar);
            r.set("T", Some(300.0 + i as Scalar));
            // Second row of `p` is missing, to exercise the NaN encoding.
            if i == 1 {
                r.set("p", None);
            } else {
                r.set("p", Some(100.0 + i as Scalar));
            }
            w.append(r).unwrap();
        }
        w.complete();
        w.finish().unwrap();
    }

    #[test]
    fn test_export_csv_is_derived_view_not_only_storage() {
        let dir = scratch_dir("csv");
        build_dataset(&dir, 4);
        let reader = DatasetReader::open(&dir).unwrap();
        let csv = scratch("export", "csv");
        let outcome = DatasetAdapter::export_csv(&reader, &csv).unwrap();
        assert!(outcome.lost_metadata());
        assert!(outcome.lost_fields.iter().any(|f| f.contains("manifest")));
        assert!(outcome.lost_fields.iter().any(|f| f.contains("units")));
        assert_eq!(outcome.rows_written, 4);

        let text = std::fs::read_to_string(&csv).unwrap();
        // A CSV cannot hold the manifest, so it must *point back* at it.
        assert!(text.contains("simulation_id: sim-adapter"));
        assert!(text.contains("variable T: quantity=temperature unit=K"));
        assert!(text.contains("time,T,p"));
        let _ = std::fs::remove_file(&csv);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_csv_round_trip_recovers_values_and_documents_loss() {
        let dir = scratch_dir("roundtrip");
        build_dataset(&dir, 4);
        let reader = DatasetReader::open(&dir).unwrap();
        let csv = scratch("roundtrip", "csv");
        DatasetAdapter::export_csv(&reader, &csv).unwrap();

        let text = std::fs::read_to_string(&csv).unwrap();
        let spec = ImportSpec {
            name: "reimport".to_string(),
            first_column_is_time: true,
            ..Default::default()
        }
        .with_unit("temperature", "K", 1.0, 0.0);
        let imported = DatasetAdapter::import_csv(&text, &spec).unwrap();

        assert_eq!(imported.row_count(), 4);
        let t = imported.values.get("T").unwrap();
        assert_eq!(t, &vec![300.0, 301.0, 302.0, 303.0]);
        let p = imported.values.get("p").unwrap();
        assert!(p[1].is_nan(), "missing value must survive the round trip");
        assert_eq!(p[0], 100.0);
        // Time instants survive CSV (unlike JSON).
        assert_eq!(imported.times, vec![0.0, 1.0, 2.0, 3.0]);
        // The loss is explicit, not silent.
        assert!(imported.lost_fields.iter().any(|f| f.contains("solver")));
        // The reconstructed schema is valid and carries the header time axis.
        imported.schema.validate().unwrap();
        assert_eq!(imported.schema.variables.len(), 2);

        let _ = std::fs::remove_file(&csv);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_json_export_and_import_round_trip() {
        let dir = scratch_dir("json");
        build_dataset(&dir, 3);
        let reader = DatasetReader::open(&dir).unwrap();
        let json_path = scratch("export", "json");
        let outcome = DatasetAdapter::export_json(&reader, &json_path).unwrap();
        assert!(outcome.lost_fields.iter().any(|f| f.contains("units")));

        let text = std::fs::read_to_string(&json_path).unwrap();
        let spec = ImportSpec {
            name: "reimport".to_string(),
            ..Default::default()
        }
        .with_unit("temperature", "K", 1.0, 0.0);
        let imported = DatasetAdapter::import_json(&text, &spec).unwrap();
        assert_eq!(imported.row_count(), 3);
        assert_eq!(
            imported.values.get("T").unwrap(),
            &vec![300.0, 301.0, 302.0]
        );
        assert!(imported.values.get("p").unwrap()[1].is_nan());
        // The original instants are NOT recoverable from the JSON signal map.
        assert!(
            imported
                .lost_fields
                .iter()
                .any(|f| f.contains("time instants")),
            "JSON import must document the loss of the time axis"
        );
        assert_eq!(imported.times, vec![0.0, 1.0, 2.0]);

        let _ = std::fs::remove_file(&json_path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_import_csv_rejects_ragged_rows() {
        let text = "time,a\n0,1\n1,2,3\n";
        let err = DatasetAdapter::import_csv(text, &ImportSpec::default()).unwrap_err();
        assert!(matches!(err, AdapterError::Parse(_)));
    }

    #[test]
    fn test_import_csv_rejects_non_numeric_cells() {
        let text = "time,a\n0,hello\n";
        let err = DatasetAdapter::import_csv(text, &ImportSpec::default()).unwrap_err();
        assert!(matches!(err, AdapterError::Parse(_)));
    }

    #[test]
    fn test_import_json_rejects_mismatched_series_lengths() {
        let text = r#"{"a":[1,2,3],"b":[1,2]}"#;
        let err = DatasetAdapter::import_json(text, &ImportSpec::default()).unwrap_err();
        assert!(matches!(err, AdapterError::Unsupported(_)));
    }

    #[test]
    fn test_import_csv_handles_missing_time_column() {
        let text = "a,b\n1,2\n3,4\n";
        let spec = ImportSpec {
            name: "no_time".to_string(),
            first_column_is_time: false,
            ..Default::default()
        };
        let imported = DatasetAdapter::import_csv(text, &spec).unwrap();
        assert_eq!(imported.row_count(), 2);
        assert_eq!(imported.values.get("a").unwrap(), &vec![1.0, 3.0]);
        assert_eq!(imported.values.get("b").unwrap(), &vec![2.0, 4.0]);
        // Synthesised instants are row indices.
        assert_eq!(imported.times, vec![0.0, 1.0]);
    }

    #[test]
    fn test_imported_dataset_variable_infos_match_schema() {
        let text = "time,T\n0,300\n1,301\n";
        let spec = ImportSpec::default().with_unit("temperature", "K", 1.0, 0.0);
        let imported = DatasetAdapter::import_csv(text, &spec).unwrap();
        let infos = imported.variable_infos();
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].name, "T");
        assert_eq!(infos[0].unit_symbol, "K");
        assert_eq!(imported.variable("T").unwrap().unit_scale, 1.0);
    }

    #[test]
    fn test_text_encoding_helpers() {
        assert_eq!(text_precision(), Precision::F64);
        assert_eq!(text_missing_encoding(), MissingValueEncoding::Nan);
    }
}
