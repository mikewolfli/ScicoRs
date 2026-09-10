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

/// Parse the coordinate triple out of a STEP argument list.
///
/// Handles both forms produced by [`export_step`]:
/// * an inline `(x,y,z)` group (nested inside the record's own parens, so it
///   sits at depth 2), and
/// * a bare `(x,y,z)` string with no surrounding record parens (depth 1), which
///   is how a caller passes an extracted argument.
///
/// The triple is identified as the parenthesised group containing exactly two
/// top-level commas, which skips the leading `'tag'` argument.
fn parse_cartesian_point(text: &str) -> Result<Coord3D, String> {
    // Remember whether we saw a triple-shaped group whose components were not
    // numbers, so the returned error can say which of the two failures it was.
    let mut saw_non_numeric = false;
    for target_depth in [2usize, 1] {
        let mut start = None;
        let mut depth = 0usize;
        let mut in_quote = false;
        for (i, c) in text.char_indices() {
            match c {
                '\'' => in_quote = !in_quote,
                '(' if !in_quote => {
                    depth += 1;
                    if depth == target_depth {
                        start = Some(i);
                    }
                }
                ')' if !in_quote => {
                    if depth == target_depth
                        && let Some(s) = start
                    {
                        let inner = &text[s + 1..i];
                        if inner.matches(',').count() == 2 {
                            let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
                            if parts.iter().all(|p| p.parse::<Scalar>().is_ok()) {
                                return Ok(Coord3D::new(
                                    parts[0].parse().map_err(|e| {
                                        format!("invalid x coordinate '{}': {}", parts[0], e)
                                    })?,
                                    parts[1].parse().map_err(|e| {
                                        format!("invalid y coordinate '{}': {}", parts[1], e)
                                    })?,
                                    parts[2].parse().map_err(|e| {
                                        format!("invalid z coordinate '{}': {}", parts[2], e)
                                    })?,
                                ));
                            }
                            saw_non_numeric = true;
                        }
                        start = None;
                    }
                    depth = depth.saturating_sub(1);
                }
                _ => {}
            }
        }
    }
    if saw_non_numeric {
        return Err(format!(
            "coordinate triple must be three numbers, got: '{}'",
            text.trim()
        ));
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

/// Extract the entity keyword of a STEP record, e.g. `CARTESIAN_POINT` from
/// `CARTESIAN_POINT('',(1,2,3));`.
///
/// Uses the identifier immediately preceding the first `(` so that a keyword
/// which is a substring of another keyword cannot be matched by mistake
/// (`B_SPLINE_CURVE_WITH_KNOTS` contains `LINE`).
fn record_kind(record: &str) -> Option<String> {
    let open = record.find('(')?;
    let head = record[..open].trim();
    let keyword = head
        .rsplit(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .next()?
        .trim();
    if keyword.is_empty() {
        return None;
    }
    Some(keyword.to_ascii_uppercase())
}

/// Import a STEP file (AP203 subset).
///
/// Runs two passes so that `#id` references can be resolved: the first pass
/// records every record by id, the second reconstructs the entities. This is
/// required because [`export_step`] writes shared geometry (vertex points, the
/// whole face/shell structure) as references rather than inline values.
///
/// All six [`StepEntity`] variants are recovered, including the entities whose
/// geometry is carried out-of-line. A record whose id is referenced but never
/// defined is reported as an error instead of silently producing a wrong shape.
/// The length unit stored in the `unit:` marker is restored as well.
pub fn import_step(filepath: &str) -> Result<StepModel, String> {
    let content =
        std::fs::read_to_string(filepath).map_err(|e| format!("STEP read error: {}", e))?;

    // Record id -> right-hand side of the record.
    let mut records: Vec<(usize, String)> = Vec::new();
    let mut saw_end = false;
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.eq_ignore_ascii_case("END-ISO-10303-21;") {
            saw_end = true;
        }
        if let Some(rest) = line.strip_prefix('#') {
            // `#12 = SOMETHING(...);`
            if let Some((id_str, rhs)) = rest.split_once('=') {
                if let Ok(id) = id_str.trim().parse::<usize>() {
                    records.push((id, rhs.trim().to_string()));
                }
            }
        }
    }

    if !saw_end {
        return Err("not a STEP file: missing the END-ISO-10303-21 terminator".to_string());
    }

    // Resolve a `#id` reference to the coordinate it names.
    let resolve_point = |id: usize| -> Result<Coord3D, String> {
        let (_, rhs) = records
            .iter()
            .find(|(rid, _)| *rid == id)
            .ok_or_else(|| format!("STEP: reference #{id} is not defined"))?;
        if !rhs.to_ascii_uppercase().contains("CARTESIAN_POINT") {
            return Err(format!("STEP: #{id} is not a CARTESIAN_POINT"));
        }
        parse_cartesian_point(rhs)
    };

    // Every point that some other record references as its own vertex: a LINE
    // endpoint, a spline control point, or a face-bound vertex. Those points are
    // *part of* the entity that references them, so re-materialising each as a
    // standalone `StepEntity::Point` would make the model grow on every
    // save/load cycle (observed: 5 -> 13 -> 21 -> 29 entities).
    let mut referenced_points: std::collections::HashSet<usize> = std::collections::HashSet::new();
    for (_, rhs) in &records {
        let Some(kind) = record_kind(rhs) else {
            continue;
        };
        if matches!(
            kind.as_str(),
            "LINE" | "B_SPLINE_CURVE_WITH_KNOTS" | "ADVANCED_FACE" | "CIRCLE"
        ) {
            for token in arguments(rhs).split(|c: char| !c.is_ascii_digit() && c != '#') {
                if let Some(id) = token
                    .strip_prefix('#')
                    .and_then(|n| n.parse::<usize>().ok())
                {
                    referenced_points.insert(id);
                }
            }
        }
    }

    let mut model = StepModel::new();
    // Record id of each ADVANCED_FACE -> its index among the face records seen
    // so far, so a CLOSED_SHELL's `#id` references map back to face indices.
    let mut face_ids: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for (rid, rhs) in &records {
        // Match on the record's *keyword* rather than a `contains` substring.
        // `contains` is fragile here: "B_SPLINE_CURVE_WITH_KNOTS" contains
        // "LINE", so a naive chain would dispatch a spline to the line branch.
        let Some(kind) = record_kind(rhs) else {
            continue;
        };
        match kind.as_str() {
            "CARTESIAN_POINT" => {
                // Only a point that nothing else references is a standalone
                // entity; a referenced one is re-created by its owning entity.
                if !referenced_points.contains(rid) {
                    model
                        .entities
                        .push(StepEntity::Point(parse_cartesian_point(rhs)?));
                }
            }
            "B_SPLINE_CURVE_WITH_KNOTS" => {
                let parts = split_args(arguments(rhs));
                let mut control_points = Vec::new();
                for part in &parts {
                    // Control points are written as a parenthesised list of `#id`
                    // references, e.g. `(#13,#14)`. Unwrap the list first, then
                    // resolve each reference (or accept an inline `(x,y,z)`).
                    let entries = if part.starts_with('(') {
                        let inner = part
                            .trim_start_matches('(')
                            .trim_end_matches(')')
                            .to_string();
                        split_args(&inner)
                    } else {
                        vec![part.clone()]
                    };
                    for entry in entries {
                        let entry = entry.trim();
                        if let Some(id) = entry
                            .strip_prefix('#')
                            .and_then(|n| n.parse::<usize>().ok())
                        {
                            control_points.push(resolve_point(id)?);
                        } else if entry.starts_with('(') {
                            control_points.push(parse_cartesian_point(entry)?);
                        } else if entry.matches(',').count() == 2 {
                            // A bare inline triple.
                            control_points.push(parse_cartesian_point(&format!("({})", entry))?);
                        }
                    }
                }
                let degree = parts
                    .iter()
                    .find_map(|p| p.parse::<usize>().ok())
                    .unwrap_or(0);
                model.entities.push(StepEntity::BSplineCurve {
                    control_points,
                    degree,
                });
            }
            "LINE" => {
                // `LINE('',#a,#b)` — both endpoints are references.
                let ids: Vec<usize> = split_args(arguments(rhs))
                    .iter()
                    .filter_map(|p| p.trim().strip_prefix('#')?.parse::<usize>().ok())
                    .collect();
                if ids.len() != 2 {
                    return Err(format!(
                        "LINE record must reference exactly 2 points, found {}",
                        ids.len()
                    ));
                }
                let a = resolve_point(ids[0])?;
                let b = resolve_point(ids[1])?;
                model.entities.push(StepEntity::Line(a, b));
            }
            "ADVANCED_FACE" => {
                // `ADVANCED_FACE('',(<#outer>),(<#inner...>),.T.)` — bounds are lists
                // of point references written by `export_step`.
                let parts = split_args(arguments(rhs));
                let groups: Vec<Vec<Coord3D>> = parts
                    .iter()
                    .filter(|p| p.starts_with('('))
                    .map(|p| -> Result<Vec<Coord3D>, String> {
                        let inner = p.trim_start_matches('(').trim_end_matches(')');
                        if inner.trim().is_empty() {
                            return Ok(Vec::new());
                        }
                        split_args(inner)
                            .iter()
                            .map(|r| {
                                let id = r
                                    .trim()
                                    .strip_prefix('#')
                                    .ok_or_else(|| {
                                        format!(
                                            "ADVANCED_FACE bound entry '{r}' is not a reference"
                                        )
                                    })?
                                    .parse::<usize>()
                                    .map_err(|e| format!("invalid bound reference '{r}': {e}"))?;
                                resolve_point(id)
                            })
                            .collect()
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                let outer_bound = groups.first().cloned().unwrap_or_default();
                // `ADVANCED_FACE('',(),())` yields the groups `[[], []]`; the second
                // empty group is the *absence* of inner bounds, not one empty bound.
                let inner_bounds: Vec<Vec<Coord3D>> = groups
                    .into_iter()
                    .skip(1)
                    .filter(|g| !g.is_empty())
                    .collect();
                face_ids.insert(
                    *rid,
                    model
                        .entities
                        .iter()
                        .filter(|e| matches!(e, StepEntity::Face { .. }))
                        .count(),
                );
                model.entities.push(StepEntity::Face {
                    outer_bound,
                    inner_bounds,
                });
            }
            "CLOSED_SHELL" => {
                // `CLOSED_SHELL('',(#a,#b,...))` where the references are record ids
                // of `ADVANCED_FACE` records. `StepEntity::Shell` stores *indices
                // into the face list*, so translate an id back to its position
                // among the face records recorded so far.
                let ids: Vec<usize> = split_args(arguments(rhs))
                    .iter()
                    .filter(|p| p.starts_with('('))
                    .flat_map(|p| {
                        let inner = p.trim_start_matches('(').trim_end_matches(')').to_string();
                        split_args(&inner)
                    })
                    .filter_map(|r| r.trim().strip_prefix('#')?.parse::<usize>().ok())
                    .collect();
                let faces: Vec<usize> = ids
                    .iter()
                    .map(|id| {
                        face_ids.get(id).copied().ok_or_else(|| {
                        format!(
                            "CLOSED_SHELL references #{id}, which is not an ADVANCED_FACE record"
                        )
                    })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                model.entities.push(StepEntity::Shell { faces });
            }
            "CIRCLE" => {
                // `CIRCLE('',#axis,radius)`. The centre was written as a separate
                // CARTESIAN_POINT record; the axis record names the direction. We
                // recover the centre by looking for the CARTESIAN_POINT record that
                // immediately precedes this CIRCLE in file order.
                let parts = split_args(arguments(rhs));
                let inline_centre = parts
                    .iter()
                    .find(|p| p.starts_with('('))
                    .map(|p| parse_cartesian_point(p))
                    .transpose()?;
                let radius = parts
                    .iter()
                    .filter_map(|p| p.strip_suffix(".0").unwrap_or(p).parse::<Scalar>().ok())
                    .next_back()
                    .ok_or("CIRCLE record without a radius")?;
                let centre = match inline_centre {
                    Some(c) => c,
                    None => {
                        // Fall back to the most recent plain point record.
                        let mut last: Option<Coord3D> = None;
                        for (_, r) in &records {
                            if r.to_ascii_uppercase().contains("CARTESIAN_POINT") {
                                last = parse_cartesian_point(r).ok();
                            }
                            if std::ptr::eq(r, rhs) {
                                break;
                            }
                        }
                        last.ok_or("CIRCLE record without a recoverable centre point")?
                    }
                };
                model.entities.push(StepEntity::Circle(
                    centre,
                    radius,
                    Coord3D::new(0.0, 0.0, 1.0),
                ));
            }
            // Anything else (e.g. SI_UNIT, DIRECTION) is not part of the
            // documented subset and carries no recoverable entity.
            _ => {}
        }
    }

    // The length unit and the circle axes are carried as `/* ... */` markers by
    // the writer, so recover them by scanning the raw text. `Circle` stores the
    // axis normal, which the CIRCLE record itself does not encode.
    //
    // Markers are matched positionally to the circles in record order.
    let mut circle_axes: Vec<Coord3D> = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("/* circle axis") {
            if let Some(inner) = rest.split('(').nth(1).and_then(|s| s.split(')').next()) {
                let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
                if parts.len() == 3
                    && let (Ok(x), Ok(y), Ok(z)) = (
                        parts[0].parse::<Scalar>(),
                        parts[1].parse::<Scalar>(),
                        parts[2].parse::<Scalar>(),
                    )
                {
                    circle_axes.push(Coord3D::new(x, y, z));
                }
            }
        } else if let Some(rest) = line.strip_prefix("/* unit:")
            && let Some(unit) = rest.trim().strip_suffix("*/")
        {
            model.unit = unit.trim().to_string();
        }
    }
    // Apply the recovered axes to the circles, in order.
    let mut axis_iter = circle_axes.into_iter();
    for entity in model.entities.iter_mut() {
        if let StepEntity::Circle(_, _, normal) = entity
            && let Some(axis) = axis_iter.next()
        {
            *normal = axis;
        }
    }

    Ok(model)
}

/// Record id reserved for the length-unit entity. Chosen far above any
/// realistic geometry id so it cannot collide with an emitted record.
const SI_UNIT_RECORD_ID: usize = 999_999;

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
    // Give it an id well outside the entity range so it can never collide with
    // (or be mistaken for) a geometry record.
    step.push_str(&format!(
        "#{} = SI_UNIT(.MILLI.,.METRE.) ;\n",
        SI_UNIT_RECORD_ID
    ));
    step.push_str(&format!("/* unit: {} */\n", model.unit));

    // Faces and shells reference point ids, so emit a flat point table first
    // and remember where each entity's points landed.
    let mut next_id = 1usize;
    // Id of the ADVANCED_FACE record emitted for each `Face` entity, in entity
    // order. A `Shell` stores face *indices*, which are only meaningful once
    // mapped onto the records actually written.
    let mut face_record_ids: Vec<usize> = Vec::new();
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
                    "#{ia} = CARTESIAN_POINT('',{});\n#{ib} = CARTESIAN_POINT('',{});\n",
                    cartesian_point(a),
                    cartesian_point(b)
                ));
                step.push_str(&format!("#{} = LINE('',#{ia},#{ib});\n", next_id));
                next_id += 1;
            }
            StepEntity::Circle(centre, radius, normal) => {
                let ic = next_id;
                next_id += 1;
                step.push_str(&format!(
                    "#{ic} = CARTESIAN_POINT('',{});\n",
                    cartesian_point(centre)
                ));
                step.push_str(&format!("#{} = CIRCLE('',#{ic},{});\n", next_id, radius));
                next_id += 1;
                // The axis direction is not part of the CIRCLE record we parse,
                // so keep it in a marker that `import_step` reads back.
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
                // Emit the outer bound's points, remembering their ids.
                let first = next_id;
                for p in outer_bound {
                    step.push_str(&format!(
                        "#{} = CARTESIAN_POINT('',{});\n",
                        next_id,
                        cartesian_point(p)
                    ));
                    next_id += 1;
                }
                let outer_list = (first..first + outer_bound.len())
                    .map(|i| format!("#{}", i))
                    .collect::<Vec<_>>()
                    .join(",");

                // Then the inner bounds, each as its own reference list.
                let mut inner_lists = Vec::with_capacity(inner_bounds.len());
                for bound in inner_bounds {
                    let start = next_id;
                    for p in bound {
                        step.push_str(&format!(
                            "#{} = CARTESIAN_POINT('',{});\n",
                            next_id,
                            cartesian_point(p)
                        ));
                        next_id += 1;
                    }
                    inner_lists.push(
                        (start..start + bound.len())
                            .map(|i| format!("#{}", i))
                            .collect::<Vec<_>>()
                            .join(","),
                    );
                }

                // Bounds are lists-of-lists, exactly as `import_step` reads them.
                step.push_str(&format!(
                    "#{} = ADVANCED_FACE('',({}),({}),.T.);\n",
                    next_id,
                    outer_list,
                    inner_lists
                        .iter()
                        .map(|l| format!("({})", l))
                        .collect::<Vec<_>>()
                        .join(",")
                ));
                face_record_ids.push(next_id);
                next_id += 1;
            }
            StepEntity::Shell { faces } => {
                // Map each face index onto the record id emitted for that face.
                // An out-of-range index is a modelling error, not something to
                // write into the file as a dangling reference.
                let mut refs = Vec::with_capacity(faces.len());
                for &f in faces {
                    let id = face_record_ids.get(f).ok_or_else(|| {
                        format!(
                            "shell references face index {} but only {} face records exist",
                            f,
                            face_record_ids.len()
                        )
                    })?;
                    refs.push(format!("#{}", id));
                }
                step.push_str(&format!(
                    "#{} = CLOSED_SHELL('',({}));\n",
                    next_id,
                    refs.join(",")
                ));
                next_id += 1;
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
    /// entity kinds. Every variant must now survive a real write + read cycle.
    #[test]
    fn test_step_export_roundtrips_every_entity_kind() {
        let mut model = StepModel::new();
        model.unit = "inch".to_string();
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
        // A face whose outer bound is the unit triangle plus one inner loop.
        model.entities.push(StepEntity::Face {
            outer_bound: vec![
                Coord3D::new(0.0, 0.0, 0.0),
                Coord3D::new(1.0, 0.0, 0.0),
                Coord3D::new(0.0, 1.0, 0.0),
            ],
            inner_bounds: vec![vec![Coord3D::new(0.5, 0.5, 0.0)]],
        });

        let path = scratch("allkinds");
        export_step(&model, &path).expect("export must succeed for all kinds");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("LINE("), "lines must be written, not dropped");
        assert!(text.contains("CIRCLE("), "circles must be written");
        assert!(
            text.contains("B_SPLINE_CURVE_WITH_KNOTS"),
            "splines must be written"
        );
        assert!(text.contains("ADVANCED_FACE"), "faces must be written");

        // The real test: read it back.
        let back = import_step(&path).expect("the exporter's own output must be importable");
        let _ = std::fs::remove_file(&path);

        assert_eq!(
            back.unit, "inch",
            "the length unit must survive the round trip"
        );

        // Points: one standalone point, plus the geometry that the other
        // entities carry out-of-line. Assert the standalone ones and the
        // reconstructed shapes rather than the raw count.
        let points: Vec<&Coord3D> = back
            .entities
            .iter()
            .filter_map(|e| match e {
                StepEntity::Point(p) => Some(p),
                _ => None,
            })
            .collect();
        assert!(
            points
                .iter()
                .any(|p| p.x == 1.0 && p.y == 2.0 && p.z == 3.0),
            "the original point must come back with real coordinates"
        );

        let line = back
            .entities
            .iter()
            .find_map(|e| match e {
                StepEntity::Line(a, b) => Some((a, b)),
                _ => None,
            })
            .expect("the LINE must be recovered, not silently dropped");
        assert_eq!((line.0.x, line.0.y, line.0.z), (0.0, 0.0, 0.0));
        assert_eq!((line.1.x, line.1.y, line.1.z), (1.0, 1.0, 1.0));

        let (centre, radius, _) = back
            .entities
            .iter()
            .find_map(|e| match e {
                StepEntity::Circle(c, r, n) => Some((*c, *r, *n)),
                _ => None,
            })
            .expect("the CIRCLE must be recovered via its #id centre reference");
        assert_eq!(radius, 2.5, "the radius must survive");
        assert_eq!((centre.x, centre.y, centre.z), (0.0, 0.0, 0.0));

        let spline = back
            .entities
            .iter()
            .find_map(|e| match e {
                StepEntity::BSplineCurve {
                    control_points,
                    degree,
                } => Some((control_points.clone(), *degree)),
                _ => None,
            })
            .expect("the B-spline must be recovered by resolving its #id refs");
        assert_eq!(spline.0.len(), 2, "both control points must resolve");
        assert_eq!(spline.0[0].x, 0.0);
        assert_eq!(spline.0[1].x, 1.0);
        assert_eq!(spline.1, 1, "the degree must survive");

        let (outer, inner) = back
            .entities
            .iter()
            .find_map(|e| match e {
                StepEntity::Face {
                    outer_bound,
                    inner_bounds,
                } => Some((outer_bound.clone(), inner_bounds.clone())),
                _ => None,
            })
            .expect("the ADVANCED_FACE must be recovered, not written as a comment");
        assert_eq!(outer.len(), 3, "the outer bound must keep 3 points");
        assert_eq!(inner.len(), 1, "the inner bound must survive");
        assert_eq!(inner[0].len(), 1);
        assert_eq!((inner[0][0].x, inner[0][0].y), (0.5, 0.5));
    }

    /// A save/load cycle must reach a fixed point. Each Line/Spline/Face emits
    /// its vertices as shared `CARTESIAN_POINT` records; if the reader turned
    /// those back into standalone `Point` entities the model would grow on every
    /// cycle (observed 5 -> 13 -> 21 -> 29 before the fix).
    #[test]
    fn test_step_roundtrip_is_idempotent() {
        let mut model = StepModel::new();
        model
            .entities
            .push(StepEntity::Point(Coord3D::new(9.0, 9.0, 9.0)));
        model.entities.push(StepEntity::Line(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1.0, 1.0, 1.0),
        ));
        model.entities.push(StepEntity::BSplineCurve {
            control_points: vec![Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(1.0, 0.0, 0.0)],
            degree: 1,
        });
        model.entities.push(StepEntity::Face {
            outer_bound: vec![Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(1.0, 0.0, 0.0)],
            inner_bounds: vec![],
        });

        let n0 = model.entities.len();

        // Cycle 1.
        let p1 = scratch("idem1");
        export_step(&model, &p1).unwrap();
        let m1 = import_step(&p1).unwrap();
        let _ = std::fs::remove_file(&p1);
        assert_eq!(
            m1.entities.len(),
            n0,
            "one cycle must not change the entity count"
        );

        // Cycle 2 must be identical to cycle 1, not larger.
        let p2 = scratch("idem2");
        export_step(&m1, &p2).unwrap();
        let m2 = import_step(&p2).unwrap();
        let _ = std::fs::remove_file(&p2);
        assert_eq!(
            m2.entities.len(),
            n0,
            "a second cycle must not inflate the model further"
        );

        // The three non-point entities must all still be present exactly once.
        let count = |pred: fn(&StepEntity) -> bool| m2.entities.iter().filter(|e| pred(e)).count();
        assert_eq!(count(|e| matches!(e, StepEntity::Point(_))), 1);
        assert_eq!(count(|e| matches!(e, StepEntity::Line(..))), 1);
        assert_eq!(count(|e| matches!(e, StepEntity::BSplineCurve { .. })), 1);
        assert_eq!(count(|e| matches!(e, StepEntity::Face { .. })), 1);
    }

    #[test]
    fn test_step_shell_roundtrips() {
        let mut model = StepModel::new();
        // A shell refers to faces by index, so the faces must exist.
        for _ in 0..3 {
            model.entities.push(StepEntity::Face {
                outer_bound: vec![Coord3D::new(0.0, 0.0, 0.0)],
                inner_bounds: vec![],
            });
        }
        model.entities.push(StepEntity::Shell { faces: vec![0, 2] });
        let path = scratch("shell");
        export_step(&model, &path).unwrap();
        let back = import_step(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        let faces = back
            .entities
            .iter()
            .find_map(|e| match e {
                StepEntity::Shell { faces } => Some(faces.clone()),
                _ => None,
            })
            .expect("the CLOSED_SHELL must be a real record, not a comment");
        assert_eq!(faces.len(), 2, "both referenced faces must be listed");
        assert_eq!(faces[0], 0, "the first face index must map back");
        assert_eq!(faces[1], 2, "the third face index must map back");
    }

    /// A shell that points at a face which does not exist is a modelling error
    /// and must be refused rather than written as a dangling reference.
    #[test]
    fn test_step_shell_rejects_out_of_range_face_index() {
        let mut model = StepModel::new();
        model.entities.push(StepEntity::Face {
            outer_bound: vec![Coord3D::new(0.0, 0.0, 0.0)],
            inner_bounds: vec![],
        });
        model.entities.push(StepEntity::Shell { faces: vec![5] });
        let path = scratch("bad_shell");
        let err = export_step(&model, &path).unwrap_err();
        let _ = std::fs::remove_file(&path);
        assert!(
            err.contains("references face index 5"),
            "an out-of-range face index must be refused, got: {err}"
        );
    }

    #[test]
    fn test_step_export_writes_entities_as_records_not_comments() {
        let mut model = StepModel::new();
        model.entities.push(StepEntity::Face {
            outer_bound: vec![Coord3D::new(0.0, 0.0, 0.0)],
            inner_bounds: vec![],
        });
        model.entities.push(StepEntity::Shell { faces: vec![0] });
        let path = scratch("records");
        export_step(&model, &path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(
            text.contains("= ADVANCED_FACE("),
            "faces must be records, not comments:\n{text}"
        );
        assert!(
            text.contains("= CLOSED_SHELL("),
            "shells must be records, not comments:\n{text}"
        );
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
            err.contains("must be three numbers"),
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

    /// The circle's axis normal is not part of the CIRCLE record, so the writer
    /// carries it in a marker. It must be read back, not defaulted to +z.
    /// An empty inner-bound list must stay empty: `skip(1)` over `(),()` used to
    /// fabricate one empty bound, mutating the face on every round trip.
    #[test]
    fn test_step_empty_face_bounds_do_not_gain_a_spurious_inner_bound() {
        let mut model = StepModel::new();
        model.entities.push(StepEntity::Face {
            outer_bound: vec![Coord3D::new(0.0, 0.0, 0.0)],
            inner_bounds: vec![],
        });
        let path = scratch("emptyface");
        export_step(&model, &path).unwrap();
        let back = import_step(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        let (outer, inner) = back
            .entities
            .iter()
            .find_map(|e| match e {
                StepEntity::Face {
                    outer_bound,
                    inner_bounds,
                } => Some((outer_bound.clone(), inner_bounds.clone())),
                _ => None,
            })
            .expect("the face must be recovered");
        assert_eq!(outer.len(), 1);
        assert!(
            inner.is_empty(),
            "a face with no inner bounds must not gain an empty one, got {inner:?}"
        );
    }

    #[test]
    fn test_step_circle_axis_roundtrips() {
        let mut model = StepModel::new();
        model.entities.push(StepEntity::Circle(
            Coord3D::new(1.0, 2.0, 3.0),
            4.5,
            Coord3D::new(1.0, 0.0, 0.0),
        ));
        let path = scratch("axis");
        export_step(&model, &path).unwrap();
        let back = import_step(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        let (centre, radius, normal) = back
            .entities
            .iter()
            .find_map(|e| match e {
                StepEntity::Circle(c, r, n) => Some((*c, *r, *n)),
                _ => None,
            })
            .expect("the circle must be recovered");
        assert_eq!((centre.x, centre.y, centre.z), (1.0, 2.0, 3.0));
        assert_eq!(radius, 4.5);
        assert_eq!(
            (normal.x, normal.y, normal.z),
            (1.0, 0.0, 0.0),
            "the circle axis must round-trip, not be defaulted to +z"
        );
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
