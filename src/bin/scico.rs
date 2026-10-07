// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! The `scico` command-line tool (BLUE13 Phase 41).
//!
//! This is the thin binary entry point for the SCIcoRS toolchain. It collects the
//! process arguments and delegates to [`scico_rs::cli::cli_main`], which parses
//! and executes the command, prints a human-readable message, and returns a
//! stable non-zero exit code on failure. Keeping the logic in the library means
//! the exact same dispatch is unit-tested without spawning a process.
//!
//! Usage:
//!
//! ```text
//! scico <check|run|status|results|cancel|resume|validate|version> [args]
//! ```

use std::process::ExitCode;

fn main() -> ExitCode {
    // argv[0] is the program name; the CLI dispatcher expects arguments only.
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = scico_rs::cli::cli_main(&args);
    // The library exit code is a small non-negative integer; map it onto a
    // process exit code. `u8::try_from` guards against an out-of-range value.
    ExitCode::from(u8::try_from(code).unwrap_or(u8::MAX))
}
