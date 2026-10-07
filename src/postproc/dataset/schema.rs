// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Self-describing dataset schema: result variables, dimensions, coordinates
//! and units.
//!
//! This module describes *what* a dataset contains independently of *how* it is
//! stored on disk. A [`DatasetSchema`] is a versioned, serializable description
//! of the result variables (their physical quantity, unit, dimensionality,
//! sampling location, coordinate axes, time axis, missing-value encoding and
//! precision) plus the time and coordinate axes themselves.
//!
//! # Unit conversion
//!
//! [`ResultVariable::convert_values_to`] performs a **real numeric
//! transformation** between two compatible units using the crate's
//! [`crate::core::units::Unit`] conversion machinery. It does not merely relabel
//! the stored numbers: converting from `km` to `m` multiplies by 1000.

use crate::core::types::Scalar;

/// Current on-disk schema version produced by this crate.
///
/// See [`crate::postproc::dataset::migration`] for the version history and the
/// upgrade path from older versions.
pub const CURRENT_SCHEMA_VERSION: u32 = 2;

/// The oldest schema version this crate can still read (and upgrade).
pub const MIN_SUPPORTED_SCHEMA_VERSION: u32 = 1;

/// Where a result variable is sampled on the computational mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SampleLocation {
    /// Sampled at mesh nodes (vertices).
    Node,
    /// Sampled at cell centres.
    Cell,
    /// Sampled on cell faces (e.g. fluxes).
    Face,
    /// Sampled on cell edges.
    Edge,
    /// A global / lumped quantity with no spatial location.
    Global,
}

impl SampleLocation {
    /// Stable, human-readable identifier used in files and error messages.
    pub fn name(self) -> &'static str {
        match self {
            SampleLocation::Node => "node",
            SampleLocation::Cell => "cell",
            SampleLocation::Face => "face",
            SampleLocation::Edge => "edge",
            SampleLocation::Global => "global",
        }
    }
}

/// Numeric storage precision of a variable's samples.
///
/// The first-version container always stores `f64` values as JSON numbers; the
/// declared precision records what the *producer* actually used so a reader can
/// reason about expected accuracy and round-trip loss.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Precision {
    /// IEEE-754 binary32 (single precision).
    F32,
    /// IEEE-754 binary64 (double precision).
    F64,
}

impl Precision {
    /// Stable identifier used in files.
    pub fn name(self) -> &'static str {
        match self {
            Precision::F32 => "f32",
            Precision::F64 => "f64",
        }
    }
}

/// How a missing sample is encoded in the stored values.
///
/// Missing data is *always* encoded as an IEEE-754 `NaN` in the numeric stream.
/// The variant records how a *reader of the numeric stream* (e.g. a CSV
/// consumer) should interpret absence, so the encoding is explicit rather than
/// implicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MissingValueEncoding {
    /// A missing sample is stored as the IEEE-754 `NaN` value.
    Nan,
    /// A missing sample is stored as `NaN`; the container additionally records
    /// the sentinel used by legacy exports so they can round-trip.
    NanWithSentinel,
    /// The variable may legitimately never miss a sample.
    None,
}

impl MissingValueEncoding {
    /// Stable identifier used in files.
    pub fn name(self) -> &'static str {
        match self {
            MissingValueEncoding::Nan => "nan",
            MissingValueEncoding::NanWithSentinel => "nan_with_sentinel",
            MissingValueEncoding::None => "none",
        }
    }
}

/// A typed, self-describing result variable inside a dataset.
///
/// The variable carries everything a reader needs to interpret the numbers:
/// its name, physical quantity and unit, dimensionality, sampling location, the
/// coordinate and time axes it is laid out against, how missing values are
/// encoded, and the declared numeric precision.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ResultVariable {
    /// Unique name of the variable within its dataset (non-empty).
    pub name: String,
    /// Physical quantity this variable represents (e.g. `"temperature"`).
    pub quantity: String,
    /// Canonical URN of the unit, e.g. `"urn:scicors:unit:m"`.
    ///
    /// A URN (rather than a bare symbol like `"m"`) keeps the identifier stable
    /// and unambiguous even when symbols collide across quantity systems.
    pub unit_urn: String,
    /// Human-readable unit symbol (e.g. `"m"`, `"°C"`).
    pub unit_symbol: String,
    /// Conversion scale factor to the SI base unit of this dimension.
    pub unit_scale: Scalar,
    /// Additive offset to the SI base unit (e.g. `273.15` for °C).
    pub unit_offset: Scalar,
    /// Shape of the variable's data, outermost dimension first. An empty vector
    /// means the variable is a scalar (one value per time sample).
    pub dimensions: Vec<usize>,
    /// Sampling location on the mesh.
    pub location: SampleLocation,
    /// Names of the coordinate axes this variable's samples are laid out over,
    /// in storage order. Empty for lumped/global variables.
    pub coordinate_axes: Vec<String>,
    /// Name of the time axis this variable is sampled against, if any.
    pub time_axis: Option<String>,
    /// How missing samples are encoded.
    pub missing_value: MissingValueEncoding,
    /// Declared producer precision.
    pub precision: Precision,
    /// Free-form semantic tags (schema v2+). A tag identifies a canonical role,
    /// e.g. [`TAGS_SENSOR_READING`], so a reader need not match on the display
    /// name. v1 datasets carry no tags; migration backfills this to empty.
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Canonical tag marking a variable as a measured sensor reading.
pub const TAGS_SENSOR_READING: &str = "sensor.reading";

/// Canonical tag marking a variable as a solver residual/diagnostic.
pub const TAGS_DIAGNOSTIC: &str = "diagnostic";

impl ResultVariable {
    /// Build a scalar (one value per time sample) variable in SI base units.
    pub fn scalar(name: &str, quantity: &str, unit_symbol: &str) -> Self {
        Self {
            name: name.to_string(),
            quantity: quantity.to_string(),
            unit_urn: unit_urn_for(unit_symbol),
            unit_symbol: unit_symbol.to_string(),
            unit_scale: 1.0,
            unit_offset: 0.0,
            dimensions: Vec::new(),
            location: SampleLocation::Global,
            coordinate_axes: Vec::new(),
            time_axis: None,
            missing_value: MissingValueEncoding::Nan,
            precision: Precision::F64,
            tags: Vec::new(),
        }
    }

    /// Attach the given semantic tags (replacing any existing tags).
    pub fn with_tags(mut self, tags: &[&str]) -> Self {
        self.tags = tags.iter().map(|t| t.to_string()).collect();
        self
    }

    /// Whether this variable carries `tag`.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    /// Declare the unit explicitly, including scale and offset relative to the
    /// SI base unit.
    pub fn with_unit(mut self, symbol: &str, scale: Scalar, offset: Scalar) -> Self {
        self.unit_symbol = symbol.to_string();
        self.unit_urn = unit_urn_for(symbol);
        self.unit_scale = scale;
        self.unit_offset = offset;
        self
    }

    /// Declare the shape (outermost dimension first).
    pub fn with_dimensions(mut self, dims: &[usize]) -> Self {
        self.dimensions = dims.to_vec();
        self
    }

    /// Declare the sampling location.
    pub fn with_location(mut self, location: SampleLocation) -> Self {
        self.location = location;
        self
    }

    /// Bind the variable to named coordinate axes (storage order).
    pub fn with_coordinates(mut self, axes: &[&str]) -> Self {
        self.coordinate_axes = axes.iter().map(|a| a.to_string()).collect();
        self
    }

    /// Bind the variable to a time axis by name.
    pub fn with_time_axis(mut self, axis: &str) -> Self {
        self.time_axis = Some(axis.to_string());
        self
    }

    /// Declare the producer precision.
    pub fn with_precision(mut self, precision: Precision) -> Self {
        self.precision = precision;
        self
    }

    /// Declare the missing-value encoding.
    pub fn with_missing_value(mut self, encoding: MissingValueEncoding) -> Self {
        self.missing_value = encoding;
        self
    }

    /// Number of values a single time sample of this variable holds (the
    /// product of its declared dimensions; `1` for a scalar variable).
    pub fn values_per_sample(&self) -> usize {
        self.dimensions.iter().product::<usize>().max(1)
    }

    /// True when two variables share the same physical dimension, i.e. their
    /// stored numbers can be converted between each other.
    ///
    /// Two units are compatible when they scale/offset the *same* base SI
    /// dimension. Because the schema stores the scale/offset rather than the
    /// full exponent vector, compatibility is decided from the declared
    /// quantity and the derived SI scale: variables convert only when their
    /// quantity string matches and both declare a finite, non-zero scale.
    pub fn is_unit_compatible_with(&self, other: &ResultVariable) -> bool {
        self.quantity == other.quantity
            && self.unit_scale.is_finite()
            && other.unit_scale.is_finite()
            && self.unit_scale != 0.0
            && other.unit_scale != 0.0
    }

    /// Convert a slice of stored values from this variable's unit into
    /// `target`'s unit, **transforming the numbers**.
    ///
    /// Returns an error when the two variables are not unit-compatible. The
    /// arithmetic is `v_si = (v + from_offset) * from_scale` then
    /// `v_target = v_si / to_scale - to_offset`, i.e. exactly
    /// [`crate::core::units::Unit::convert`].
    ///
    /// `NaN` samples (the missing-value encoding) are preserved verbatim.
    pub fn convert_values_to(
        &self,
        values: &[Scalar],
        target: &ResultVariable,
    ) -> Result<Vec<Scalar>, String> {
        if !self.is_unit_compatible_with(target) {
            return Err(format!(
                "cannot convert variable '{}' [{}] to [{}]: incompatible units",
                self.name, self.unit_symbol, target.unit_symbol
            ));
        }
        let out = values
            .iter()
            .map(|&v| {
                if v.is_nan() {
                    Scalar::NAN
                } else {
                    (v + self.unit_offset) * self.unit_scale / target.unit_scale
                        - target.unit_offset
                }
            })
            .collect();
        Ok(out)
    }

    /// Produce the metadata of this variable rewritten for the `target` unit.
    ///
    /// The returned variable has the target's unit fields but keeps this
    /// variable's shape, coordinates and time binding — use it together with
    /// [`Self::convert_values_to`] so a converted stream carries consistent
    /// metadata.
    pub fn rebased_to(&self, target: &ResultVariable) -> Result<ResultVariable, String> {
        if !self.is_unit_compatible_with(target) {
            return Err(format!(
                "cannot rebase variable '{}' to incompatible unit [{}]",
                self.name, target.unit_symbol
            ));
        }
        let mut rebased = self.clone();
        rebased.unit_urn = target.unit_urn.clone();
        rebased.unit_symbol = target.unit_symbol.clone();
        rebased.unit_scale = target.unit_scale;
        rebased.unit_offset = target.unit_offset;
        Ok(rebased)
    }
}

/// Build a stable URN identifier for a unit symbol.
fn unit_urn_for(symbol: &str) -> String {
    format!("urn:scicors:unit:{}", symbol)
}

/// A named coordinate axis (spatial dimension) of a dataset.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CoordinateAxis {
    /// Unique axis name within the dataset (e.g. `"x"`).
    pub name: String,
    /// Canonical unit URN of the axis coordinate.
    pub unit_urn: String,
    /// Unit symbol of the axis coordinate.
    pub unit_symbol: String,
    /// Conversion scale to the SI base unit of the axis dimension.
    pub unit_scale: Scalar,
    /// Additive offset to the SI base unit.
    pub unit_offset: Scalar,
    /// Number of coordinate entries along this axis.
    pub length: usize,
    /// Coordinate values along the axis (length must equal `length`).
    pub values: Vec<Scalar>,
}

impl CoordinateAxis {
    /// Create a uniform axis of `length` samples starting at `start` with
    /// spacing `step`, in SI base units.
    pub fn uniform(name: &str, start: Scalar, step: Scalar, length: usize) -> Self {
        let values = (0..length).map(|i| start + step * i as Scalar).collect();
        Self {
            name: name.to_string(),
            unit_urn: unit_urn_for("m"),
            unit_symbol: "m".to_string(),
            unit_scale: 1.0,
            unit_offset: 0.0,
            length,
            values,
        }
    }

    /// Create an axis from explicit coordinate values, in SI base units.
    pub fn from_values(name: &str, values: Vec<Scalar>) -> Self {
        Self {
            name: name.to_string(),
            unit_urn: unit_urn_for("m"),
            unit_symbol: "m".to_string(),
            unit_scale: 1.0,
            unit_offset: 0.0,
            length: values.len(),
            values,
        }
    }

    /// Declare the axis unit.
    pub fn with_unit(mut self, symbol: &str, scale: Scalar, offset: Scalar) -> Self {
        self.unit_urn = unit_urn_for(symbol);
        self.unit_symbol = symbol.to_string();
        self.unit_scale = scale;
        self.unit_offset = offset;
        self
    }

    /// Convert this axis' coordinate values into another compatible unit,
    /// returning `(symbol, values)` in the target unit.
    ///
    /// Returns an error when the two axes are not compatible (different names
    /// or non-positive scale).
    pub fn convert_values_to(&self, target: &CoordinateAxis) -> Result<Vec<Scalar>, String> {
        if self.name != target.name || self.unit_scale <= 0.0 || target.unit_scale <= 0.0 {
            return Err(format!(
                "cannot convert coordinate axis '{}' [{}] to '{}' [{}]",
                self.name, self.unit_symbol, target.name, target.unit_symbol
            ));
        }
        Ok(self
            .values
            .iter()
            .map(|&v| {
                (v + self.unit_offset) * self.unit_scale / target.unit_scale - target.unit_offset
            })
            .collect())
    }

    /// Validate internal consistency (non-empty name, values match length).
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("coordinate axis has an empty name".to_string());
        }
        if self.values.len() != self.length {
            return Err(format!(
                "coordinate axis '{}' declares length {} but carries {} values",
                self.name,
                self.length,
                self.values.len()
            ));
        }
        if self.unit_scale <= 0.0 || !self.unit_scale.is_finite() {
            return Err(format!(
                "coordinate axis '{}' has a non-positive unit scale {}",
                self.name, self.unit_scale
            ));
        }
        Ok(())
    }
}

/// The time axis of a dataset: the instants every time-varying variable is
/// sampled at.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TimeAxis {
    /// Unique axis name within the dataset (e.g. `"time"`).
    pub name: String,
    /// Canonical unit URN of the time values.
    pub unit_urn: String,
    /// Unit symbol of the time values (e.g. `"s"`).
    pub unit_symbol: String,
    /// Conversion scale to seconds.
    pub unit_scale: Scalar,
    /// Additive offset to seconds.
    pub unit_offset: Scalar,
    /// Sample instants, strictly increasing, in this axis' unit.
    pub values: Vec<Scalar>,
}

impl TimeAxis {
    /// Create a time axis from explicit instants (in the given unit).
    pub fn new(name: &str, unit_symbol: &str, scale: Scalar, values: Vec<Scalar>) -> Self {
        Self {
            name: name.to_string(),
            unit_urn: unit_urn_for(unit_symbol),
            unit_symbol: unit_symbol.to_string(),
            unit_scale: scale,
            unit_offset: 0.0,
            values,
        }
    }

    /// Create a uniform seconds axis `[0, step, 2*step, ...]` of `n` samples.
    pub fn uniform_seconds(n: usize, step: Scalar) -> Self {
        Self::new(
            "time",
            "s",
            1.0,
            (0..n).map(|i| step * i as Scalar).collect(),
        )
    }

    /// Number of instants on the axis.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether the axis holds no instants.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Convert stored instants into another compatible time unit, transforming
    /// the numbers.
    pub fn convert_to(
        &self,
        target_unit: &str,
        target_scale: Scalar,
    ) -> Result<Vec<Scalar>, String> {
        if self.unit_scale <= 0.0 || target_scale <= 0.0 {
            return Err(format!(
                "cannot convert time axis '{}' to '{}': non-positive scale",
                self.unit_symbol, target_unit
            ));
        }
        Ok(self
            .values
            .iter()
            .map(|&v| (v + self.unit_offset) * self.unit_scale / target_scale)
            .collect())
    }

    /// Validate that the axis is non-empty and strictly increasing.
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("time axis has an empty name".to_string());
        }
        if self.unit_scale <= 0.0 || !self.unit_scale.is_finite() {
            return Err(format!(
                "time axis '{}' has a non-positive unit scale {}",
                self.name, self.unit_scale
            ));
        }
        for pair in self.values.windows(2) {
            if pair[1] <= pair[0] {
                return Err(format!(
                    "time axis '{}' is not strictly increasing at {} -> {}",
                    self.name, pair[0], pair[1]
                ));
            }
        }
        for v in &self.values {
            if !v.is_finite() {
                return Err(format!(
                    "time axis '{}' carries a non-finite instant {}",
                    self.name, v
                ));
            }
        }
        Ok(())
    }
}

/// A complete, versioned description of a dataset's contents.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DatasetSchema {
    /// Schema version this dataset was written with.
    pub schema_version: u32,
    /// Human-readable dataset name.
    pub name: String,
    /// The time axis, if the dataset is time-varying.
    pub time_axis: Option<TimeAxis>,
    /// Spatial coordinate axes, in the order used by variable layouts.
    pub coordinates: Vec<CoordinateAxis>,
    /// Result variables, in a **stable, defined order** (the write order).
    pub variables: Vec<ResultVariable>,
}

impl DatasetSchema {
    /// Create an empty schema at the current version.
    pub fn new(name: &str) -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            name: name.to_string(),
            time_axis: None,
            coordinates: Vec::new(),
            variables: Vec::new(),
        }
    }

    /// Create a schema with a single uniform time axis of `n` samples.
    pub fn with_uniform_time(name: &str, n: usize, step: Scalar) -> Self {
        let mut schema = Self::new(name);
        schema.time_axis = Some(TimeAxis::uniform_seconds(n, step));
        schema
    }

    /// Look up a variable by name.
    pub fn variable(&self, name: &str) -> Option<&ResultVariable> {
        self.variables.iter().find(|v| v.name == name)
    }

    /// Append a variable, rejecting duplicates and empty names.
    pub fn add_variable(&mut self, variable: ResultVariable) -> Result<(), String> {
        if variable.name.trim().is_empty() {
            return Err("result variable has an empty name".to_string());
        }
        if self.variables.iter().any(|v| v.name == variable.name) {
            return Err(format!("duplicate result variable '{}'", variable.name));
        }
        self.variables.push(variable);
        Ok(())
    }

    /// Append a coordinate axis, rejecting duplicates and empty names.
    pub fn add_coordinate(&mut self, axis: CoordinateAxis) -> Result<(), String> {
        if axis.name.trim().is_empty() {
            return Err("coordinate axis has an empty name".to_string());
        }
        if self.coordinates.iter().any(|a| a.name == axis.name) {
            return Err(format!("duplicate coordinate axis '{}'", axis.name));
        }
        self.coordinates.push(axis);
        Ok(())
    }

    /// Names of all variables in their defined order.
    pub fn variable_names(&self) -> Vec<&str> {
        self.variables.iter().map(|v| v.name.as_str()).collect()
    }

    /// Validate the whole schema for internal consistency.
    ///
    /// Checks the schema version is supported, the name is non-empty, variable
    /// names are unique, and every declared coordinate/time axis reference
    /// resolves.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version < MIN_SUPPORTED_SCHEMA_VERSION
            || self.schema_version > CURRENT_SCHEMA_VERSION
        {
            return Err(format!(
                "unsupported schema version {} (supported: {}..={})",
                self.schema_version, MIN_SUPPORTED_SCHEMA_VERSION, CURRENT_SCHEMA_VERSION
            ));
        }
        if self.name.trim().is_empty() {
            return Err("dataset schema has an empty name".to_string());
        }
        let mut seen = std::collections::HashSet::new();
        for v in &self.variables {
            if v.name.trim().is_empty() {
                return Err("result variable has an empty name".to_string());
            }
            if !seen.insert(v.name.as_str()) {
                return Err(format!("duplicate result variable '{}'", v.name));
            }
            if v.quantity.trim().is_empty() {
                return Err(format!(
                    "result variable '{}' has an empty quantity",
                    v.name
                ));
            }
            if v.unit_symbol.trim().is_empty() {
                return Err(format!("result variable '{}' has an empty unit", v.name));
            }
            if !v.unit_scale.is_finite() || v.unit_scale == 0.0 {
                return Err(format!(
                    "result variable '{}' has an invalid unit scale {}",
                    v.name, v.unit_scale
                ));
            }
            for axis in &v.coordinate_axes {
                if !self.coordinates.iter().any(|c| &c.name == axis) {
                    return Err(format!(
                        "result variable '{}' references unknown coordinate axis '{}'",
                        v.name, axis
                    ));
                }
            }
            if let Some(time) = &v.time_axis {
                match &self.time_axis {
                    Some(axis) if &axis.name == time => {}
                    Some(axis) => {
                        return Err(format!(
                            "result variable '{}' references time axis '{}' but the dataset's axis is '{}'",
                            v.name, time, axis.name
                        ));
                    }
                    None => {
                        return Err(format!(
                            "result variable '{}' references time axis '{}' but the dataset declares none",
                            v.name, time
                        ));
                    }
                }
            }
        }
        let mut coord_seen = std::collections::HashSet::new();
        for c in &self.coordinates {
            c.validate()?;
            if !coord_seen.insert(c.name.as_str()) {
                return Err(format!("duplicate coordinate axis '{}'", c.name));
            }
        }
        if let Some(t) = &self.time_axis {
            t.validate()?;
        }
        Ok(())
    }

    /// Total number of time samples declared by the schema (`1` when there is
    /// no time axis).
    pub fn time_sample_count(&self) -> usize {
        self.time_axis.as_ref().map(|t| t.len()).unwrap_or(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_result_variable_scalar_defaults() {
        let v = ResultVariable::scalar("temperature", "temperature", "K");
        assert_eq!(v.name, "temperature");
        assert_eq!(v.unit_symbol, "K");
        assert_eq!(v.values_per_sample(), 1);
        assert_eq!(v.location, SampleLocation::Global);
        assert_eq!(v.precision, Precision::F64);
    }

    #[test]
    fn test_values_per_sample_from_dimensions() {
        let v = ResultVariable::scalar("field", "velocity", "m/s")
            .with_dimensions(&[3, 4])
            .with_location(SampleLocation::Cell);
        assert_eq!(v.values_per_sample(), 12);
    }

    #[test]
    fn test_unit_conversion_really_transforms_numbers() {
        // km -> m must multiply by 1000, not just relabel.
        let source = ResultVariable::scalar("x", "length", "km").with_unit("km", 1000.0, 0.0);
        let target = ResultVariable::scalar("x", "length", "m").with_unit("m", 1.0, 0.0);
        let values = vec![1.0, 2.5, -0.5];
        let converted = source.convert_values_to(&values, &target).unwrap();
        assert!((converted[0] - 1000.0).abs() < 1e-9);
        assert!((converted[1] - 2500.0).abs() < 1e-9);
        assert!((converted[2] + 500.0).abs() < 1e-9);
        // The metadata is genuinely rebased too.
        let rebased = source.rebased_to(&target).unwrap();
        assert_eq!(rebased.unit_symbol, "m");
        assert_eq!(rebased.unit_scale, 1.0);
    }

    #[test]
    fn test_temperature_offset_conversion() {
        // 0 degC == 273.15 K; converting must add the offset, not relabel.
        let celsius = ResultVariable::scalar("T", "temperature", "°C").with_unit("°C", 1.0, 273.15);
        let kelvin = ResultVariable::scalar("T", "temperature", "K").with_unit("K", 1.0, 0.0);
        let converted = celsius.convert_values_to(&[0.0, 100.0], &kelvin).unwrap();
        assert!((converted[0] - 273.15).abs() < 1e-9);
        assert!((converted[1] - 373.15).abs() < 1e-9);
    }

    #[test]
    fn test_incompatible_unit_conversion_is_rejected() {
        let length = ResultVariable::scalar("x", "length", "m");
        let time = ResultVariable::scalar("t", "time", "s");
        assert!(length.convert_values_to(&[1.0], &time).is_err());
    }

    #[test]
    fn test_missing_values_survive_conversion() {
        let source = ResultVariable::scalar("x", "length", "km").with_unit("km", 1000.0, 0.0);
        let target = ResultVariable::scalar("x", "length", "m").with_unit("m", 1.0, 0.0);
        let converted = source
            .convert_values_to(&[Scalar::NAN, 1.0], &target)
            .unwrap();
        assert!(converted[0].is_nan(), "missing value must stay missing");
        assert!((converted[1] - 1000.0).abs() < 1e-9);
    }

    #[test]
    fn test_coordinate_axis_conversion() {
        let mm =
            CoordinateAxis::from_values("x", vec![0.0, 500.0, 1000.0]).with_unit("mm", 0.001, 0.0);
        let m = CoordinateAxis::from_values("x", vec![0.0]).with_unit("m", 1.0, 0.0);
        let converted = mm.convert_values_to(&m).unwrap();
        assert!((converted[1] - 0.5).abs() < 1e-12);
        assert!((converted[2] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_time_axis_conversion_ms_to_s() {
        let ms = TimeAxis::new("time", "ms", 0.001, vec![0.0, 500.0, 1000.0]);
        let secs = ms.convert_to("s", 1.0).unwrap();
        assert!((secs[1] - 0.5).abs() < 1e-12);
        assert!((secs[2] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_time_axis_must_be_strictly_increasing() {
        let good = TimeAxis::new("time", "s", 1.0, vec![0.0, 1.0, 2.0]);
        assert!(good.validate().is_ok());
        let bad = TimeAxis::new("time", "s", 1.0, vec![0.0, 2.0, 1.0]);
        assert!(bad.validate().is_err());
    }

    #[test]
    fn test_schema_rejects_duplicate_variables() {
        let mut schema = DatasetSchema::new("run");
        schema
            .add_variable(ResultVariable::scalar("a", "q", "m"))
            .unwrap();
        assert!(
            schema
                .add_variable(ResultVariable::scalar("a", "q", "m"))
                .is_err()
        );
    }

    #[test]
    fn test_schema_validation_catches_dangling_axis() {
        let mut schema = DatasetSchema::new("run");
        let mut v = ResultVariable::scalar("a", "q", "m");
        v.coordinate_axes.push("missing".to_string());
        schema.variables.push(v);
        assert!(schema.validate().is_err());
    }

    #[test]
    fn test_schema_round_trip_json() {
        let mut schema = DatasetSchema::with_uniform_time("run", 3, 0.1);
        schema
            .add_coordinate(CoordinateAxis::uniform("x", 0.0, 1.0, 3))
            .unwrap();
        schema
            .add_variable(
                ResultVariable::scalar("T", "temperature", "K")
                    .with_location(SampleLocation::Node)
                    .with_coordinates(&["x"])
                    .with_time_axis("time"),
            )
            .unwrap();
        schema.validate().unwrap();
        let json = serde_json::to_string(&schema).unwrap();
        let back: DatasetSchema = serde_json::from_str(&json).unwrap();
        assert_eq!(schema, back);
    }

    #[test]
    fn test_schema_rejects_unsupported_version() {
        let mut schema = DatasetSchema::new("run");
        schema.schema_version = CURRENT_SCHEMA_VERSION + 7;
        let err = schema.validate().unwrap_err();
        assert!(err.contains("unsupported schema version"));
    }

    #[test]
    fn test_add_coordinate_rejects_duplicate() {
        let mut schema = DatasetSchema::new("run");
        schema
            .add_coordinate(CoordinateAxis::uniform("x", 0.0, 1.0, 2))
            .unwrap();
        assert!(
            schema
                .add_coordinate(CoordinateAxis::uniform("x", 0.0, 1.0, 2))
                .is_err()
        );
    }

    #[test]
    fn test_unit_compatibility_requires_matching_quantity() {
        let a = ResultVariable::scalar("a", "length", "m");
        let b = ResultVariable::scalar("b", "length", "km").with_unit("km", 1000.0, 0.0);
        assert!(a.is_unit_compatible_with(&b));
        let c = ResultVariable::scalar("c", "time", "s");
        assert!(!a.is_unit_compatible_with(&c));
    }

    #[test]
    fn test_tags_default_empty_and_can_be_set() {
        let v = ResultVariable::scalar("T", "temperature", "K");
        assert!(v.tags.is_empty());
        let tagged =
            ResultVariable::scalar("T", "temperature", "K").with_tags(&[TAGS_SENSOR_READING]);
        assert!(tagged.has_tag(TAGS_SENSOR_READING));
        assert!(!tagged.has_tag(TAGS_DIAGNOSTIC));
    }

    #[test]
    fn test_v1_schema_without_tags_deserializes() {
        // A v1 body has no `tags` field; `#[serde(default)]` must tolerate it.
        let json = r#"{
            "name": "T",
            "quantity": "temperature",
            "unit_urn": "urn:scicors:unit:K",
            "unit_symbol": "K",
            "unit_scale": 1.0,
            "unit_offset": 0.0,
            "dimensions": [],
            "location": "Node",
            "coordinate_axes": [],
            "time_axis": null,
            "missing_value": "Nan",
            "precision": "F64"
        }"#;
        let v: ResultVariable = serde_json::from_str(json).unwrap();
        assert!(v.tags.is_empty());
    }
}
