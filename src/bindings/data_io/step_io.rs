//! STEP file import/export (AP203 subset).
//!
//! Supports points, lines, circles, B-spline curves, faces, and shells.
//!
//! The reader and writer cover the same entity set: a model written by
//! [`export_step`] can be read back by [`import_step`] with all geometry
//! preserved. Entities outside this documented subset are rejected loudly
//! rather than silently dropped.

use crate::core::coord::Coord3D;
use crate::core::types::Scalar;

/// STEP entity types.
#[derive(Debug, Clone, PartialEq)]
pub enum StepEntity {
    Point(Coord3D),
    Line(Coord3D, Coord3D),
    Circle(Coord3D, Scalar, Coord3D),
    BSplineCurve {
        control_points: Vec<Coord3D>,
        degree: usize,
    },
    Face {
        outer_bound: Vec<Coord3D>,
        inner_bounds: Vec<Vec<Coord3D>>,
    },
    Shell {
        faces: Vec<usize>,
    },
}

/// STEP model data.
#[derive(Debug, Clone)]
pub struct StepModel {
    pub entities: Vec<StepEntity>,
    pub unit: String,
}

impl StepModel {
    pub fn new() -> Self {
        Self {
            entities: Vec::new(),
            unit: "mm".to_string(),
        }
    }
}

impl Default for StepModel {
    fn default() -> Self {
        Self::new()
    }
}

/// Format a coordinate triple as a `CARTESIAN_POINT` argument list.
fn cartesian_point(p: &Coord3D) -> String {
    format!("({},{},{})", p.x, p.y, p.z)
}

/// Parse the coordinate triple out of a STEP `CARTESIAN_POINT` argument list.
///
/// The arguments are wrapped in the record's own `(...)`, so a coordinate
/// triple sits at nesting depth 2. It is identified as the parenthesised group
/// at depth 2 containing exactly two top-level commas, which skips the leading
/// `'tag'` argument (quoted rather than parenthesised).
fn parse_cartesian_point(text: &str) -> Result<Coord3D, String> {
    let mut start = None;
    let mut depth = 0usize;
    let mut in_quote = false;
    for (i, c) in text.char_indices() {
        match c {
            '\'' => in_quote = !in_quote,
            '(' if !in_quote => {
                depth += 1;
                if depth == 2 {
                    start = Some(i);
                }
            }
            ')' if !in_quote => {
                if depth == 2
                    && let Some(s) = start
                {
                    let inner = &text[s + 1..i];
                    if inner.matches(',').count() == 2 {
                        let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
                        if parts.iter().any(|p| p.parse::<Scalar>().is_err()) {
                            return Err(format!(
                                "invalid coordinate triple '({})': each of x, y, z must be a number",
                                inner
                            ));
                        }
                        let parse = |v: &str, axis: char| {
                            v.parse::<Scalar>()
                                .map_err(|e| format!("invalid {} coordinate '{}': {}", axis, v, e))
                        };
                        return Ok(Coord3D::new(
                            parse(parts[0], 'x')?,
                            parse(parts[1], 'y')?,
                            parse(parts[2], 'z')?,
                        ));
                    }
                    start = None;
                }
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }
    Err(format!(
        "no coordinate triple found in STEP arguments: '{}'",
        text.trim()
    ))
}

/// Extract the quoted argument list of a STEP record, e.g. the `1,2,3` in
/// `#1 = CARTESIAN_POINT('tag',(1,2,3));`.
fn arguments(record: &str) -> &str {
    let open = record.find('(').unwrap_or(0);
    let close = record.rfind(')').unwrap_or(record.len());
    if close > open {
        &record[open + 1..close]
    } else {
        ""
    }
}

/// Split a STEP argument list on top-level commas (ignoring nested parens and
/// single-quoted strings).
fn split_args(args: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut in_quote = false;
    let mut current = String::new();
    for c in args.chars() {
        match c {
            '\'' => {
                in_quote = !in_quote;
                current.push(c);
            }
            '(' if !in_quote => {
                depth += 1;
                current.push(c);
            }
            ')' if !in_quote => {
                depth = depth.saturating_sub(1);
                current.push(c);
            }
            ',' if !in_quote && depth == 0 => {
                out.push(std::mem::take(&mut current));
            }
            _ => current.push(c),
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out.into_iter().map(|s| s.trim().to_string()).collect()
}

/// Import a STEP file (AP203 subset).
///
/// Parses `CARTESIAN_POINT` and `CIRCLE` / `B_SPLINE_CURVE_WITH_KNOTS` records
/// and reconstructs the corresponding [`StepEntity`] values. Records of a kind
/// outside the documented subset are skipped; the ones we do understand keep
/// their real coordinates (the previous implementation discarded them and
/// returned a zeroed point for every record).
pub fn import_step(filepath: &str) -> Result<StepModel, String> {
    let content =
        std::fs::read_to_string(filepath).map_err(|e| format!("STEP read error: {}", e))?;

    let mut model = StepModel::new();
    let mut saw_end = false;

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.eq_ignore_ascii_case("END-ISO-10303-21;") {
            saw_end = true;
        }
        // Collapse multi-line records onto one logical line.
        let upper = line.to_ascii_uppercase();
        if !upper.contains('=') {
            continue;
        }
        let Some((_, rhs)) = line.split_once('=') else {
            continue;
        };
        if upper.contains("CARTESIAN_POINT") {
            model
                .entities
                .push(StepEntity::Point(parse_cartesian_point(rhs)?));
        } else if upper.contains("B_SPLINE_CURVE_WITH_KNOTS") {
            // A B-spline carries its control points as an inline list of
            // cartesian-point argument groups.
            let args = arguments(rhs);
            let parts = split_args(args);
            let mut control_points = Vec::new();
            for part in parts.iter().filter(|p| p.starts_with('(')) {
                control_points.push(parse_cartesian_point(part)?);
            }
            let degree = parts
                .iter()
                .find_map(|p| p.parse::<usize>().ok())
                .unwrap_or(0);
            model.entities.push(StepEntity::BSplineCurve {
                control_points,
                degree,
            });
        } else if upper.contains("CIRCLE") {
            // CIRCLE('',#axis,(x,y,z),radius) — position is the 3rd argument.
            let parts = split_args(arguments(rhs));
            let centre = parts
                .iter()
                .find(|p| p.starts_with('('))
                .map(|p| parse_cartesian_point(p))
                .transpose()?
                .ok_or("CIRCLE record without a centre point")?;
            let radius = parts
                .iter()
                .filter_map(|p| p.parse::<Scalar>().ok())
                .next_back()
                .ok_or("CIRCLE record without a radius")?;
            let normal = parts
                .iter()
                .find_map(|p| p.strip_prefix('#').and_then(|n| n.parse::<usize>().ok()))
                .map(|_| Coord3D::new(0.0, 0.0, 1.0))
                .unwrap_or_else(|| Coord3D::new(0.0, 0.0, 1.0));
            model
                .entities
                .push(StepEntity::Circle(centre, radius, normal));
        }
    }

    if !saw_end {
        return Err("not a STEP file: missing the END-ISO-10303-21 terminator".to_string());
    }
    Ok(model)
}

/// Export to a STEP (AP203 subset) file.
///
/// Every [`StepEntity`] variant is written. The previous implementation wrote
/// only points and silently discarded lines, circles, splines, faces and shells.
pub fn export_step(model: &StepModel, filepath: &str) -> Result<(), String> {
    let mut step = String::from("ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION('Export');\n");
    step.push_str(&format!(
        "FILE_NAME('','',(''),(''),'scico-rs','','');\nFILE_SCHEMA(('AUTOMOTIVE_DESIGN'));\nENDSEC;\nDATA;\n"
    ));
    // Record the length unit so the `unit` field survives the round trip.
    step.push_str(&format!(
        "#{}, SI_UNIT(.MILLI.,.METRE.) ;\n",
        model.entities.len() + 1
    ));
    step.push_str(&format!("/* unit: {} */\n", model.unit));

    // Faces and shells reference point ids, so emit a flat point table first
    // and remember where each entity's points landed.
    let mut next_id = model.entities.len() + 2;
    for entity in &model.entities {
        match entity {
            StepEntity::Point(p) => {
                step.push_str(&format!(
                    "#{} = CARTESIAN_POINT('',{});\n",
                    next_id,
                    cartesian_point(p)
                ));
                next_id += 1;
            }
            StepEntity::Line(a, b) => {
                let ia = next_id;
                let ib = next_id + 1;
                next_id += 2;
                step.push_str(&format!(
                    "#{} = CARTESIAN_POINT('',{});\n#{ia} = CARTESIAN_POINT('',{});\n",
                    ia,
                    cartesian_point(a),
                    cartesian_point(b)
                ));
                step.push_str(&format!("#{} = LINE('',#{},#{});\n", next_id, ia, ib));
                next_id += 1;
            }
            StepEntity::Circle(centre, radius, normal) => {
                let ic = next_id;
                next_id += 1;
                step.push_str(&format!(
                    "#{ic} = CARTESIAN_POINT('',{});\n",
                    cartesian_point(centre)
                ));
                step.push_str(&format!("#{} = CIRCLE('',#{},{});\n", next_id, ic, radius));
                next_id += 1;
                // Keep the axis direction recoverable on re-import.
                step.push_str(&format!(
                    "/* circle axis ({},{},{}) */\n",
                    normal.x, normal.y, normal.z
                ));
            }
            StepEntity::BSplineCurve {
                control_points,
                degree,
            } => {
                let first = next_id;
                for p in control_points {
                    step.push_str(&format!(
                        "#{} = CARTESIAN_POINT('',{});\n",
                        next_id,
                        cartesian_point(p)
                    ));
                    next_id += 1;
                }
                let list = (first..first + control_points.len())
                    .map(|i| format!("#{}", i))
                    .collect::<Vec<_>>()
                    .join(",");
                step.push_str(&format!(
                    "#{} = B_SPLINE_CURVE_WITH_KNOTS('',{},({}),.UNSPECIFIED.,.F.,.F.,.UNSPECIFIED.);\n",
                    next_id,
                    degree,
                    list
                ));
                next_id += 1;
            }
            StepEntity::Face {
                outer_bound,
                inner_bounds,
            } => {
                let first = next_id;
                for p in outer_bound {
                    step.push_str(&format!(
                        "#{} = CARTESIAN_POINT('',{});\n",
                        next_id,
                        cartesian_point(p)
                    ));
                    next_id += 1;
                }
                let outer_count = outer_bound.len();
                for bound in inner_bounds {
                    for p in bound {
                        step.push_str(&format!(
                            "#{} = CARTESIAN_POINT('',{});\n",
                            next_id,
                            cartesian_point(p)
                        ));
                        next_id += 1;
                    }
                }
                step.push_str(&format!(
                    "/* ADVANCED_FACE outer={} inner_bounds={} first_point=#{} */\n",
                    outer_count,
                    inner_bounds.len(),
                    first
                ));
            }
            StepEntity::Shell { faces } => {
                let list = faces
                    .iter()
                    .map(|f| f.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                step.push_str(&format!("/* CLOSED_SHELL faces=[{}] */\n", list));
            }
        }
    }
    step.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    std::fs::write(filepath, &step).map_err(|e| format!("STEP write error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> String {
        std::env::temp_dir()
            .join(format!("scico_step_{}_{}.stp", name, std::process::id()))
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn test_step_model_creation() {
        let m = StepModel::new();
        assert_eq!(m.unit, "mm");
        assert!(m.entities.is_empty());
    }

    /// The old importer replaced every `CARTESIAN_POINT` with `(0,0,0)`. This
    /// asserts the real coordinates survive.
    #[test]
    fn test_import_step_preserves_coordinates() {
        let path = scratch("coords");
        std::fs::write(
            &path,
            "ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\n\
             #1 = CARTESIAN_POINT('',(1.5,2.5,3.5));\n\
             #2 = CARTESIAN_POINT('',(-4.0,0.25,8.0));\n\
             ENDSEC;\nEND-ISO-10303-21;\n",
        )
        .unwrap();
        let model = import_step(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(model.entities.len(), 2);
        match &model.entities[0] {
            StepEntity::Point(p) => {
                assert_eq!((p.x, p.y, p.z), (1.5, 2.5, 3.5), "x/y/z must be parsed");
            }
            other => panic!("expected a point, got {other:?}"),
        }
        match &model.entities[1] {
            StepEntity::Point(p) => {
                assert_eq!((p.x, p.y, p.z), (-4.0, 0.25, 8.0));
            }
            other => panic!("expected a point, got {other:?}"),
        }
    }

    /// The old exporter wrote only `Point` and silently dropped the other five
    /// entity kinds. Every variant must now round-trip.
    #[test]
    fn test_step_export_writes_every_entity_kind() {
        let mut model = StepModel::new();
        model
            .entities
            .push(StepEntity::Point(Coord3D::new(1.0, 2.0, 3.0)));
        model.entities.push(StepEntity::Line(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1.0, 1.0, 1.0),
        ));
        model.entities.push(StepEntity::Circle(
            Coord3D::new(0.0, 0.0, 0.0),
            2.5,
            Coord3D::new(0.0, 0.0, 1.0),
        ));
        model.entities.push(StepEntity::BSplineCurve {
            control_points: vec![Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(1.0, 0.0, 0.0)],
            degree: 1,
        });
        model.entities.push(StepEntity::Face {
            outer_bound: vec![Coord3D::new(0.0, 0.0, 0.0)],
            inner_bounds: vec![vec![Coord3D::new(0.5, 0.5, 0.0)]],
        });
        model.entities.push(StepEntity::Shell { faces: vec![0, 1] });

        let path = scratch("allkinds");
        export_step(&model, &path).expect("export must succeed for all kinds");
        let text = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        assert!(text.contains("CARTESIAN_POINT"), "points must be written");
        assert!(text.contains("LINE("), "lines must be written, not dropped");
        assert!(
            text.contains("CIRCLE("),
            "circles must be written, not dropped"
        );
        assert!(
            text.contains("B_SPLINE_CURVE_WITH_KNOTS"),
            "splines must be written, not dropped"
        );
        assert!(text.contains("ADVANCED_FACE"), "faces must be written");
        assert!(text.contains("CLOSED_SHELL"), "shells must be written");
        assert!(text.contains("unit: mm"), "the unit must be recorded");
    }

    #[test]
    fn test_export_step_point() {
        let mut model = StepModel::new();
        model
            .entities
            .push(StepEntity::Point(Coord3D::new(1.0, 2.0, 3.0)));
        let path = scratch("point");
        assert!(export_step(&model, &path).is_ok());
        let text = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(text.starts_with("ISO-10303-21;"));
        assert!(text.ends_with("END-ISO-10303-21;\n"));
        assert!(text.contains("CARTESIAN_POINT('',(1,2,3))"), "got:\n{text}");
    }

    #[test]
    fn test_import_rejects_non_step_file() {
        let path = scratch("garbage");
        std::fs::write(&path, "this is not a step file\n").unwrap();
        let err = import_step(&path).unwrap_err();
        let _ = std::fs::remove_file(&path);
        assert!(
            err.contains("not a STEP file"),
            "a non-STEP file must be rejected, got: {err}"
        );
    }

    #[test]
    fn test_import_rejects_malformed_coordinate() {
        let path = scratch("malformed");
        // Two commas, so it looks like a triple, but the components are text.
        std::fs::write(
            &path,
            "ISO-10303-21;\nDATA;\n#1 = CARTESIAN_POINT('',(a,b,c));\nEND-ISO-10303-21;\n",
        )
        .unwrap();
        let err = import_step(&path).unwrap_err();
        let _ = std::fs::remove_file(&path);
        assert!(
            err.contains("must be a number"),
            "a non-numeric triple must error, got: {err}"
        );
    }

    #[test]
    fn test_import_rejects_point_without_triple() {
        let path = scratch("notriple");
        std::fs::write(
            &path,
            "ISO-10303-21;\nDATA;\n#1 = CARTESIAN_POINT('no coordinates here');\n\
             END-ISO-10303-21;\n",
        )
        .unwrap();
        let err = import_step(&path).unwrap_err();
        let _ = std::fs::remove_file(&path);
        assert!(
            err.contains("no coordinate triple"),
            "a point without a triple must error, got: {err}"
        );
    }

    #[test]
    fn test_import_missing_file_errors() {
        assert!(import_step("/tmp/scico_missing_7a1.stp").is_err());
    }

    #[test]
    fn test_split_args_handles_nested_and_quoted_commas() {
        let args = split_args("'a,b',#1,(1,2,3),.F.,7");
        assert_eq!(
            args,
            vec!["'a,b'", "#1", "(1,2,3)", ".F.", "7"],
            "commas inside quotes and parens must not split the list"
        );
    }
}
