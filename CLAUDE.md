# Claude / AI assistant directives for `signex-iced`

These rules apply to every assistant (Claude, Copilot, etc.) generating
or modifying code in this workspace.

## Clean-room policy (MANDATORY)

This repository is being developed under **clean-room** conditions to
keep it cleanly Apache-2.0 licensed. The following names MUST NOT
appear anywhere in this workspace's source code, comments, doc
comments, log messages, panel labels, identifiers, file names, error
strings, or test fixtures:

- `KiCad`, `kicad`, `KICAD`
- `.kicad_sch`, `.kicad_pcb`, `.kicad_sym`, `.kicad_pro`
- Any other GPL-licensed EDA tool name (Eeschema, Pcbnew, etc.)

Reasons:

- This crate is licensed Apache-2.0. References that imply derivation
  from a GPL-3.0 source contaminate the licensing story.
- Any foreign-format import / conversion logic lives in a SEPARATE
  companion repository. This workspace is format-agnostic.

When you need to refer to a non-Signex format, write "foreign format",
"imported document", or name the concrete neutral behaviour ("Center /
Bottom alignment", "S-expression input", etc.). Never explain a
behaviour by referencing where it came from.

If you find an existing reference to those forbidden names while
working on a file, remove it as part of your edit. Do not preserve
references "for context".

## Commits

- Messages in English, imperative present tense, conventional-commits
  style (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`).
- Never add yourself as a co-author. Only the human author appears.
- Never run `git push`, `git reset --hard`, or any history-rewriting
  command without explicit user confirmation.

## Naming

- `snake_case` for functions, variables, modules, packages.
- `CamelCase` for structs, enums.
- Meaningful descriptive names; avoid abbreviations.

## Comments

- English only. Add only when they explain intent or non-obvious
  behaviour.
- Never reference forbidden names (see clean-room policy).

## Conversation language

- Turkish is fine when chatting with the user.
- Code, identifiers, comments, commit messages, PR titles stay English.
