# TRAE AI PROJECT HARD RULES – 100% MANDATORY
# APPLIES TO ALL RUST CODE GENERATION; NO EXCEPTIONS

# ------------------------------------------------------------------------------
# 🔴 FORBIDDEN PATTERNS – ZERO TOLERANCE
# ------------------------------------------------------------------------------
- No `todo!()`, `unimplemented!()`, or empty code blocks `{}`
- No `simple_impl`, `stub_impl`, `placeholder`, `dummy`, or fake implementations
- No `Ok(())` or `Ok(Default::default())` as placeholders
- No hidden incomplete logic inside `if/else/match/loop` or nested branches
- No repeated retry loops on the same error (e.g., crate feature issues)
- No ignoring `Cargo.toml` features; never guess functions from disabled crates
- No deleting or modifying unrelated existing code
- No batch edits that leave unclosed symbols (`{}`, `()`, `[]`, `<>`)
- No lazy tool usage that skips syntax validation
- No "auto-fix" without full syntax check first
- No partial deletions that break symbol pairing

# ------------------------------------------------------------------------------
# 🟢 MANDATORY WORKFLOW & VALIDATION
# ------------------------------------------------------------------------------
1. **Full Implementation Mandate**: All code paths, branches, and `match` arms MUST be fully implemented. No hidden placeholders in any nesting level.
2. **Compilation Requirement**: Code MUST compile as-is without manual modification.
3. **Rust Error Handling**: Use proper `Result` error handling; avoid panics.
4. **WASM Support**: Ensure all generated code compiles to WASM.
5. **Architecture Compliance**: Follow project structure, data types, and naming conventions.
6. **Cargo Feature Check (Critical for Rust)**:
   - Before generating code: Verify crate existence and enabled features in `Cargo.toml`.
   - Confirm the target function exists in the current crate/feature set.
   - If a feature is missing: Output the **exact `Cargo.toml` fix** first, then generate code.
   - If a function is unavailable: Explain clearly **ONCE**; do not retry or guess.
7. **Self-Check**: After generation, scan for forbidden patterns. Fix all before output.
8. **Output Only**: Generate real, complete, runnable, production-ready Rust code only.
9. **Symbol Pair Validation**:
   - For all edits (add/delete/modify), validate **all symbol pairs** (`{}`, `()`, `[]`, `<>`) are balanced.
   - Batch edits: Recheck entire block for balanced symbols after each change.
   - If unbalanced: Fix immediately; do not output broken code.
10. **Tool Usage Rules**:
    - Use tools only after manual syntax review.
    - No tool reliance for critical syntax checks.
    - Auto-fix must include full syntax validation step.
    - Never skip symbol pairing checks when using tools.
    - No blind application of tool suggestions without review.

# ------------------------------------------------------------------------------
# TASK GENERATION & EXECUTION RULES (FOR TRAE)
# ------------------------------------------------------------------------------
- DO NOT limit tasks to 10 or any small number.
- DO NOT simplify, merge, or reduce tasks.
- List ALL tasks in FULL, complete detail.
- Preserve the original order: 1, 2, 3, ... until the END.
- Execute tasks strictly ONE BY ONE, in order, without skipping.
- Do NOT stop early, do NOT skip steps, do NOT combine steps.
- Ensure every single task is visible, numbered, and executed fully.