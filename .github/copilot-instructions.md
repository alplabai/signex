All commit messages must be in English. Messages should clearly describe what changed and why. Keep them short and concise, but informative enough. For example:

```text
feat: Add new feature X
fix: Fix bug Y
docs: Update documentation for Z
refactor: Refactor code for better readability
test: Add tests for feature A
```

This format helps quickly understand both the type and purpose of a change. Also pay attention to verb tense in commit messages; use imperative/present form (for example, use "Add", not "Added").

Follow Rust idioms for naming:

- Use `snake_case` for functions, variables, modules, and packages.
- Use `CamelCase` for structs and enums.

Examples:

```rust
fn calculate_total_price() -> f64 {
    // function body
}

mod user_management {
    // module body
}

struct UserProfile {
    // struct body
}
```

Use meaningful, descriptive names for functions and variables. Avoid abbreviations when possible. For example, `calculate_total_price` is clearer than `calc_price`.

Write comments in English and only when they add value. Comments should explain intent or non-obvious behavior, not restate trivial code. Keep functions small and focused on a single responsibility to improve readability and maintainability.

All code must be written in English. Variable names, function names, comments, and other code elements should be in English. Avoid non-English characters or words in code because they can reduce readability and increase the chance of mistakes.

Do not add yourself as a co-author; only the actual author should appear.

## Clean-room policy (MANDATORY)

This repository is being developed under **clean-room** conditions. The
following names MUST NOT appear anywhere in this workspace's source code,
comments, doc comments, log messages, panel labels, identifiers, file
names, or test fixtures:

- `KiCad`, `kicad`, `KICAD`
- `.kicad_sch`, `.kicad_pcb`, `.kicad_sym`, `.kicad_pro`
- Any other GPL-licensed EDA tool name (Eeschema, Pcbnew, etc.)

Reasons:

- This crate is licensed Apache-2.0. We must not contaminate it with
  references that imply derivation from a GPL-3.0 source.
- Any foreign-format import / conversion logic lives in a SEPARATE
  companion repository. This workspace is format-agnostic.

When you need to refer to a non-Signex format, write "foreign format",
"imported document", or name the concrete neutral concept ("Center /
Bottom alignment", "S-expression input", etc.). Never explain a behaviour
by referencing where it came from.

If you find an existing reference to those names, remove it as part of
your edit. Do not preserve them "for context".
