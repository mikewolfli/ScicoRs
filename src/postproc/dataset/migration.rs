// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Schema version upgrade and compatibility strategy.
//!
//! Datasets are long-lived: a written file may be read years later by a newer
//! build. This module defines the *compatibility contract* and implements the
//! up-conversion path.
//!
//! # Contract
//!
//! * A dataset whose `schema_version` is **newer** than this crate understands
//!   is rejected with [`MigrationError::UnsupportedVersion`]. Reading a newer
//!   file with an older reader cannot be made safe by guessing, so it is
//!   refused rather than silently mis-read.
//! * A dataset whose `schema_version` is **older** than the current version is
//!   upgraded in memory by [`migrate`], which applies each version's step in
//!   order and records what changed.
//! * Migrations never destroy information: unknown-but-present fields are kept
//!   where the schema carries them, and every structural change is reported via
//!   [`MigrationReport`].
//!
//! # Version history
//!
//! * **v1** — initial schema: variables, coordinate axes, time axis, precision
//!   and missing-value encoding.
//! * **v2** — adds per-variable **semantic tags** (`ResultVariable.tags`) so a
//!   reader can identify canonical fields (e.g. `sensor.reading`) without
//!   matching on the display name. Migrating v1 -> v2 backfills an empty tag
//!   list, which is a lossless operation.
//!
//! [`migrate`] applies the v1 -> v2 step. The current version is
//! [`CURRENT_SCHEMA_VERSION`] (`2`), so the migration path is live rather than
//! hypothetical.

use crate::postproc::dataset::schema::{DatasetSchema, ResultVariable};

/// Oldest schema version this crate can still read.
pub const MIN_SUPPORTED_SCHEMA_VERSION: u32 = 1;

/// Newest schema version this crate understands (alias of
/// [`crate::postproc::dataset::schema::CURRENT_SCHEMA_VERSION`], re-exported
/// here so migration callers have a single obvious import).
pub use crate::postproc::dataset::schema::CURRENT_SCHEMA_VERSION;

/// Errors produced by the migration module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationError {
    /// The dataset's schema version is newer than this crate supports.
    UnsupportedVersion {
        /// The version found in the dataset.
        found: u32,
        /// The newest version this crate supports.
        supported: u32,
    },
    /// The schema version is below the oldest supported version.
    TooOld {
        /// The version found in the dataset.
        found: u32,
        /// The oldest version this crate supports.
        minimum: u32,
    },
    /// The schema failed validation after (or before) migration.
    InvalidSchema(String),
}

impl std::fmt::Display for MigrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MigrationError::UnsupportedVersion { found, supported } => write!(
                f,
                "dataset schema version {} is newer than the supported version {}; \
                 upgrade ScicoRs to read it",
                found, supported
            ),
            MigrationError::TooOld { found, minimum } => write!(
                f,
                "dataset schema version {} predates the minimum supported version {}",
                found, minimum
            ),
            MigrationError::InvalidSchema(m) => write!(f, "invalid schema during migration: {}", m),
        }
    }
}

impl std::error::Error for MigrationError {}

/// What a migration changed, for audit logging.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MigrationReport {
    /// Version the schema started at.
    pub from_version: u32,
    /// Version the schema ended at.
    pub to_version: u32,
    /// Names of variables whose declared `tags` were backfilled (empty).
    pub tags_backfilled: Vec<String>,
    /// Human-readable description of each step applied.
    pub steps: Vec<String>,
}

impl MigrationReport {
    /// Whether the migration actually changed the schema.
    pub fn changed(&self) -> bool {
        self.from_version != self.to_version
    }
}

/// Migrate a schema up to [`CURRENT_SCHEMA_VERSION`].
///
/// Returns the upgraded schema plus a report of what was changed. For a schema
/// already at the current version this is a no-op that still validates the
/// input.
pub fn migrate(
    mut schema: DatasetSchema,
) -> Result<(DatasetSchema, MigrationReport), MigrationError> {
    let from = schema.schema_version;
    let mut report = MigrationReport {
        from_version: from,
        to_version: from,
        ..Default::default()
    };

    if from > CURRENT_SCHEMA_VERSION {
        return Err(MigrationError::UnsupportedVersion {
            found: from,
            supported: CURRENT_SCHEMA_VERSION,
        });
    }
    if from < MIN_SUPPORTED_SCHEMA_VERSION {
        return Err(MigrationError::TooOld {
            found: from,
            minimum: MIN_SUPPORTED_SCHEMA_VERSION,
        });
    }

    if from < 2 {
        apply_v1_to_v2(&mut schema, &mut report);
    }

    schema.schema_version = CURRENT_SCHEMA_VERSION;
    report.to_version = schema.schema_version;
    schema.validate().map_err(MigrationError::InvalidSchema)?;
    Ok((schema, report))
}

/// v1 -> v2: guarantee every variable carries a (possibly empty) tag list.
///
/// v1 had no tag field at all, so deserializing a v1 body with `#[serde(default)]`
/// yields empty tags; the step is explicit here so the audit report can name
/// the variables it touched.
fn apply_v1_to_v2(schema: &mut DatasetSchema, report: &mut MigrationReport) {
    for var in &mut schema.variables {
        if var.tags.is_empty() {
            report.tags_backfilled.push(var.name.clone());
        }
    }
    report
        .steps
        .push("v1->v2: backfilled per-variable semantic tags".to_string());
}

/// Whether this crate can read a schema of the given version without migration.
pub fn is_current(version: u32) -> bool {
    version == CURRENT_SCHEMA_VERSION
}

/// Whether a schema of the given version can be read at all (possibly after
/// migration).
pub fn is_readable(version: u32) -> bool {
    (MIN_SUPPORTED_SCHEMA_VERSION..=CURRENT_SCHEMA_VERSION).contains(&version)
}

/// Build a v1-shaped schema for testing and for the migration path: a v1 body
/// has no tags, so we construct the schema and then lower its declared version.
pub fn downgrade_to_v1(mut schema: DatasetSchema) -> DatasetSchema {
    for var in &mut schema.variables {
        var.tags.clear();
    }
    schema.schema_version = 1;
    schema
}

/// The canonical variable tag marking a field as a sensor reading.
pub const TAG_SENSOR_READING: &str = crate::postproc::dataset::schema::TAGS_SENSOR_READING;

/// Return the subset of variables carrying `tag`, in defined order.
pub fn variables_with_tag<'a>(schema: &'a DatasetSchema, tag: &str) -> Vec<&'a ResultVariable> {
    schema
        .variables
        .iter()
        .filter(|v| v.tags.iter().any(|t| t == tag))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::postproc::dataset::schema::{CoordinateAxis, ResultVariable, SampleLocation};

    fn base_schema() -> DatasetSchema {
        let mut schema = DatasetSchema::with_uniform_time("run", 3, 1.0);
        schema
            .add_variable(
                ResultVariable::scalar("T", "temperature", "K")
                    .with_location(SampleLocation::Node)
                    .with_time_axis("time"),
            )
            .unwrap();
        schema
    }

    #[test]
    fn test_migrate_v1_upgrades_to_current() {
        let v1 = downgrade_to_v1(base_schema());
        assert_eq!(v1.schema_version, 1);
        let (upgraded, report) = migrate(v1).unwrap();
        assert_eq!(upgraded.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(report.from_version, 1);
        assert_eq!(report.to_version, CURRENT_SCHEMA_VERSION);
        assert!(report.changed());
        assert_eq!(report.tags_backfilled, vec!["T".to_string()]);
        assert!(!report.steps.is_empty());
    }

    #[test]
    fn test_migrate_preserves_data_descriptions() {
        let mut v1 = downgrade_to_v1(base_schema());
        v1.add_coordinate(CoordinateAxis::uniform("x", 0.0, 1.0, 3))
            .unwrap();
        let (upgraded, _report) = migrate(v1.clone()).unwrap();
        assert_eq!(upgraded.variables.len(), v1.variables.len());
        assert_eq!(upgraded.variables[0].name, "T");
        assert_eq!(upgraded.variables[0].unit_symbol, "K");
        assert_eq!(upgraded.coordinates, v1.coordinates);
    }

    #[test]
    fn test_migrate_current_version_is_a_noop() {
        let schema = base_schema();
        let (upgraded, report) = migrate(schema.clone()).unwrap();
        assert_eq!(upgraded, schema);
        assert!(!report.changed());
        assert!(report.steps.is_empty());
    }

    #[test]
    fn test_migrate_rejects_future_version() {
        let mut schema = base_schema();
        schema.schema_version = CURRENT_SCHEMA_VERSION + 3;
        let err = migrate(schema).unwrap_err();
        match err {
            MigrationError::UnsupportedVersion { found, supported } => {
                assert_eq!(found, CURRENT_SCHEMA_VERSION + 3);
                assert_eq!(supported, CURRENT_SCHEMA_VERSION);
            }
            other => panic!("expected UnsupportedVersion, got {other:?}"),
        }
    }

    #[test]
    fn test_migrate_rejects_too_old_version() {
        let mut schema = base_schema();
        schema.schema_version = 0;
        assert!(matches!(
            migrate(schema),
            Err(MigrationError::TooOld { .. })
        ));
    }

    #[test]
    fn test_readability_predicates() {
        assert!(is_current(CURRENT_SCHEMA_VERSION));
        assert!(!is_current(1));
        assert!(is_readable(1));
        assert!(is_readable(CURRENT_SCHEMA_VERSION));
        assert!(!is_readable(0));
        assert!(!is_readable(CURRENT_SCHEMA_VERSION + 1));
    }

    #[test]
    fn test_tagged_variable_lookup() {
        let mut schema = base_schema();
        schema.variables[0]
            .tags
            .push(TAG_SENSOR_READING.to_string());
        let tagged = variables_with_tag(&schema, TAG_SENSOR_READING);
        assert_eq!(tagged.len(), 1);
        assert_eq!(tagged[0].name, "T");
        assert!(variables_with_tag(&schema, "nope").is_empty());
    }
}
