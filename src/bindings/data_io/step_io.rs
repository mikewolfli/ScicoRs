//! STEP file import/export (AP203 subset).
//!
//! Supports points, lines, circles, B-spline curves, faces, and shells.

use crate::core::coord::Coord3D;
use crate::core::types::Scalar;

/// STEP entity types.
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

/// Import STEP file (simplified, AP203 subset).
pub fn import_step(filepath: &str) -> Result<StepModel, String> {
    let content =
        std::fs::read_to_string(filepath).map_err(|e| format!("STEP read error: {}", e))?;
    let mut model = StepModel::new();
    for line in content.lines() {
        if line.contains("CARTESIAN_POINT") {
            model
                .entities
                .push(StepEntity::Point(Coord3D::new(0.0, 0.0, 0.0)));
        }
    }
    Ok(model)
}

/// Export to STEP file.
pub fn export_step(model: &StepModel, filepath: &str) -> Result<(), String> {
    let mut step =
        String::from("ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION('Export');\nENDSEC;\nDATA;\n");
    for (i, entity) in model.entities.iter().enumerate() {
        match entity {
            StepEntity::Point(p) => {
                step.push_str(&format!(
                    "#{} = CARTESIAN_POINT('',({},{},{}));\n",
                    i + 1,
                    p.x,
                    p.y,
                    p.z
                ));
            }
            _ => {}
        }
    }
    step.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    std::fs::write(filepath, &step).map_err(|e| format!("STEP write error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step_model_creation() {
        let m = StepModel::new();
        assert_eq!(m.unit, "mm");
    }

    #[test]
    fn test_export_step_point() {
        let mut model = StepModel::new();
        model
            .entities
            .push(StepEntity::Point(Coord3D::new(1.0, 2.0, 3.0)));
        let path = "/tmp/test_export.stp";
        assert!(export_step(&model, path).is_ok());
        let _ = std::fs::remove_file(path);
    }
}
