# Why `.snxprj` is JSON

Signex has six native formats. Five (`.snxsch`, `.snxpcb`, `.snxlib`,
`.snxsym`, `.snxfpt`) are a TOML envelope plus TSV bulk blocks. `.snxprj`
(the project file) is **pretty-printed JSON** (`ProjectData` in
`crates/signex-types/src/project.rs`, written by `write_project`, read by
`parse_project`).

## Short answer

It was never a design decision to use JSON *instead of* TOML+TSV. The project
file got its JSON body two days **after** the TOML+TSV cutover had already
scoped itself to document formats, and nobody has revisited the split since.
What follows separates what the history shows from what is only a current
constraint.

## What the history shows

| Date | Commit | What happened |
|---|---|---|
| 2026-04-11 | `2edcbd25` | `.snxprj` is recognised as a project extension. It is an empty marker file; the parser infers everything from the directory. |
| 2026-04-29 | `4381e4c8` | The v0.9 Apache-clean cutover (#62) moves `.snxsch` / `.snxpcb` to TOML+TSV and adds the line "`.snxprj` is unchanged and uses its own pre-existing format. Stays as-is." The project file is left alone because the cutover was about document formats. |
| 2026-05-01 | `0222a07c` | "wire v0.9 project root menu + JSON .snxprj persistence". `.snxprj` gains a JSON body (`write_project` / `parse_project`) so File > New Project, Add Existing, Rename and Remove from Project can persist. Empty markers still load through the directory probe. |
| 2026-07-11 | `11da24ec` | The format god-file is split into `format/` (#222); the "stays as-is" line is carried forward verbatim. |

No commit message, ADR or doc gives a reason for choosing JSON over TOML for
the project body. The choice appears to be `serde_json` being the cheapest way
to persist `ProjectData` in a feature commit. Treat that as an inference from
the history, not a recorded rationale.

## Constraints that hold today

These are why leaving it as JSON is defensible now, not why it was chosen:

- **Small, flat pointer record.** `ProjectData` is a name, a root path, an
  optional PCB path, a list of sheet entries, a list of library references,
  variant names and the `enable_git` flag. It has no bulk rows, so the TSV
  block pattern (built to make wire/component/track tables diff and compress
  well) buys nothing here.
- **Atomic write is format-agnostic.** `write_project` goes through
  `atomic_io::atomic_write`, same as the other formats.
- **Legacy sniffing is trivial.** `parse_project` treats a file whose first
  non-blank byte is `{` as JSON and anything else as an old empty marker. A
  JSON body that fails to parse is an error (`ProjectError::Corrupt`), never a
  fall-through to the directory probe, so a bad file cannot be silently
  replaced by an empty project on the next save.

## Known gaps (open questions)

- **No forward compatibility.** New fields use `#[serde(default)]`, which
  handles *old file, new Signex*. The reverse direction loses data:
  `ProjectData` has no `deny_unknown_fields`, no `flatten` catch-all and no
  extras bag, so a key written by a newer Signex is dropped on load and absent
  on the next save, with no error or warning. `.snxsch` and `.snxpcb` preserve unknown
  keys and unknown TSV columns (#312, #313, #314, #315); `.snxprj` is
  excluded from that work, and `.snxlib`, `.snxsym` and `.snxfpt` carry a
  format token but no unknown-key preservation either. **Current position:
  `.snxprj` accepts unknown-key loss. This is an unreviewed gap, not a
  deliberate trade-off.** Giving it an unknown-key bag and a version token
  is tracked in #788.
- **No format/version manifest.** The TOML envelope gives the other five a
  `format` token that lets an older build refuse a newer file. `.snxprj` has
  none, so an older build cannot tell that it is about to drop fields.
- **Should `.snxprj` be TOML?** Undecided. Moving it would give it the
  envelope, the version token and the unknown-key machinery for free, at the
  cost of a breaking file-format change and a migration path for existing
  projects. If it is ever done it needs its own breaking-change issue.
