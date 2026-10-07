// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! CAD/CAE data I/O: STEP, STL, mesh formats.
//!
//! # Sub-modules (per blue11.md)
//!
//! - **`step_io`** — STEP file import/export (AP203 subset)
//! - **`stl_io`** — Binary STL file import/export
//! - **`mesh_io`** — Multi-format mesh I/O (VTK, Gmsh, Abaqus, Ansys)

pub mod mesh_io;
pub mod step_io;
pub mod stl_io;

// Backward-compatible re-exports
pub use mesh_io::*;
pub use step_io::*;
pub use stl_io::*;
