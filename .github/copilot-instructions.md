# Project Code Review Mandatory Standards

Please strictly adhere to the following standards when reviewing any code in this project:

## 1. Empty Implementations and Placeholders
- Identify all empty functions, methods, and conditional branches that contain only `TODO` / `FIXME` comments.
- If an empty implementation is detected, prompt the developer to complete the logic. If sufficient context exists, directly generate a complete, reasonable implementation as a suggestion.

## 2. Infinite Loops and Logical Errors
- Analyze all `for` and `while` loops to check for unreachable exit conditions.
- Examine recursive functions to ensure there is a proper base case that can terminate the recursion.
- Review conditional logic (`if/else`, `switch`) to identify conditions that are always true or always false, as well as logical contradictions that could lead to incorrect program flow.

## 3. Cross‑Calls and Circular Dependencies
- Review calling relationships between modules, classes, and functions; flag any potential circular dependency risks.
- If a complex situation is found where A calls B and B calls back to A, analyze and suggest refactoring approaches to break the cycle.

## 4. Unused Code
- Mark all defined functions, variables, classes, or imports that are never referenced anywhere.
- For unused code, recommend deletion or request a justification for keeping it.

## 5. Function Completeness and Limitations
- Strictly examine function input parameters, processing logic, and return values.
- **Edge Cases**: Assess whether the function properly handles null values, out‑of‑bounds conditions, unexpected data types, etc.
- **Error Handling**: Identify uncaught potential exceptions (e.g., file read/write failures, network timeouts) and suggest adding `try-catch` or error‑return checks.
- **Feature Completeness**: Based on the function name and context, determine whether the implementation is complete. For example, if a function named `saveUser` updates the database but does not log the action, suggest completing it.
- **Limitations**: If a function has obvious constraints (e.g., supports only a specific format) while the context suggests it should be more general, point out those limitations and propose enhancement options.

# GitHub Copilot Component Instructions
# STRICTLY ENFORCED FOR ALL CODE EDITS, GENERATION, AND REFACTORING

## FORBIDDEN ACTIONS
- DO NOT delete code without validating and balancing all symbols: `{}`, `()`, `[]`, `<>`
- DO NOT leave unclosed braces, parentheses, or brackets
- DO NOT perform bulk deletions that break syntax structure
- DO NOT use auto-fix tools without full syntax and symbol validation
- DO NOT generate partial, incomplete, or placeholder implementations
- DO NOT use: `todo!()`, `unimplemented!()`, empty blocks `{}`, `simple_impl`, `stub`, `placeholder`
- DO NOT guess crate features or functions that do not exist
- DO NOT modify or delete unrelated code

## MANDATORY BEHAVIOR
1. Always validate symbol pairs before and after every edit.
2. Ensure all blocks are properly closed and balanced.
3. Only generate complete, valid, ready-to-compile Rust code.
4. Check for syntax errors before outputting any change.
5. Preserve existing code structure and logic unless explicitly instructed.
6. When modifying functions or blocks, maintain full structural integrity.
7. Fix all unbalanced symbols and syntax issues BEFORE finalizing the output.