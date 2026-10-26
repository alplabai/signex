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

## Net classes (`net_classes`)

`ProjectData.net_classes` is an ordered list of `NetClass` rows
(`crates/signex-types/src/net_class.rs`). It is a top-level key, so a build
that predates it carries the whole table in the `UnknownJson` bag and writes
it back untouched. It is omitted when empty: a project with no classes saves
byte-identical to before.

```json
"net_classes": [
  {
    "name": "Power",
    "nets": ["GND"],
    "patterns": ["VCC*"],
    "style": { "color": "#DC1E1E", "line": "dotted", "wire_width_mm": 0.35 }
  }
]
```

- `nets` are exact, case-sensitive net names. `patterns` are globs over the
  net name: `*` is any run of characters (including none), `?` is exactly one;
  everything else is literal and the whole name must match.
- A net name is the project netlist's name for the net, not a label's own
  text: a local label `LOCAL` on a child sheet names the net `/LOCAL`. The Net
  Class picker on a selected net label stores that name. A class cannot be
  named `(none)`; the picker uses that entry for "no class".
- A net that matches several classes takes the **first matching class in
  table order**. There is no specificity ranking; reorder the table to change
  the winner.
- `style.color` is `#RRGGBB` or `#RRGGBBAA`; `style.line` is `solid`
  (default, not written), `dashed` or `dotted`; `style.wire_width_mm` is the
  wire width in millimetres. Each is optional, and an absent value leaves the
  wire's own look alone. A malformed colour makes the project refuse to open
  rather than silently dropping the value.
- **PCB extension point.** The same struct carries the PCB rule block
  (`clearance`, `trace_width`, `via_diameter`, `via_drill`, `diff_pair_gap`,
  `diff_pair_width`, all millimetres, `0.0` = unset and not written). A new
  rule is one more defaulted, `skip_serializing_if`-guarded field on
  `NetClass`; no second model and no format-version bump.
- **Editing.** The table is edited through `signex_engine::NetClassCommand`
  (`add`, `remove`, `rename`, `set_style`, `set_members`, `assign_net`,
  `move`), which is
  `serde`-serialisable, validated (unique non-empty names, positive finite
  width) and applied through `NetClassHistory` as one undo step each. The
  Properties panel only sends these commands; the `.snxprj` is written on an
  explicit project save.
- **Assigning from the schematic.** A selected net label's Properties has a
  Net Class picker. It sends `assign_net`, which makes the label's net a
  member of exactly one class by exact name (it leaves every other class's
  `nets` list and joins the chosen one), or of none. Patterns are not edited,
  so a class earlier in the table whose pattern still matches keeps winning;
  the Net Classes section says so when that happens.
- **Drawing.** The canvas (CPU and GPU scene paths), SVG, PDF and the print
  preview all draw a member net's wires with the class style, and hit-testing
  uses the drawn width. The per-element style comes from joining the table
  with each sheet's netlist after every update (`signex_net::element_class_styles`);
  the canvas never reads the project. Precedence, per property:
  - **colour**: the per-net colour override (the Active Bar net-colour tool) wins
    over the class colour, which wins over the theme's wire colour;
  - **width**: the class width, else the wire's own `stroke_width`;
  - **line style**: the class line style, else solid.

  A class colour also tints the junctions of its nets. Exports honour the
  persisted per-net colours (`[extras.sheet.net_colors]`) in the same order.

## Project-scoped rules (#1085)

`ProjectData.rules` is an optional `rules` object. It is added without bumping
the `snxprj/1` token: the key is optional with a default, a file without it
loads as `None` and is written back without a `rules` key, and an older build
that sees the key keeps it in its unknown-key bag. The section has its own
`schema` number (currently 1), independent of the file token.

```json
"rules": {
  "schema": 1,
  "profile": { "name": "Signex Default", "source": "builtin", "version": "1.0", "sha256": "..." },
  "erc": {
    "severity":   { "pin_type_conflict": "error", "bus_unnamed": "warning" },
    "pin_matrix": { "output/power_out": "error" }
  }
}
```

- Rule ids are the stable strings from `RuleKind::key`, severities are
  `error` / `warning` / `info` / `off`, and pin-matrix keys are
  `"<row axis>/<column axis>"` using `PinAxis::key` (`input`, `output`,
  `bidir`, `power_in`, `power_out`, `not_connected`). Maps are `BTreeMap`s so a
  rule change is a small diff.
- When `rules` is present, ERC reads only it; a rule it does not list runs at
  its built-in default and is reported in the Messages panel. When it is
  absent, ERC uses the machine preferences as before. `signex_erc::project_rules`
  owns the id vocabulary.
- The section is written only by an explicit action: "Store ERC rules in this
  project" in the ERC dialog snapshots the machine preferences into it (every
  rule id, plus the pin matrix cells that differ from the factory matrix), and
  later edits in the dialog change the project's set instead of the
  preferences. Both are undoable steps that mark the `.snxprj` dirty; the file
  is written by the normal save. Edits keep keys this build does not know.
- `profile` is provenance only (not written yet: portable `.snxrules` profiles
  are a later slice). `drc` and `annotation` are reserved: they are
  carried verbatim in the section's unknown-key bag until their features land.
- A section this build cannot interpret never stops the project from opening
  (#1180). A `schema` above 1, or a value whose shape changed (a severity that
  is an object, say), makes the whole section opaque: it is kept verbatim in
  the project's unknown-key bag, written back untouched, and not applied; ERC
  uses the machine preferences and says so once per open. A later build may
  therefore change a value's shape only together with a `schema` bump.
- `schema`, `erc` and every `profile` field are optional and omitted when
  absent or empty. An explicitly empty value (`"erc": {}`, `"name": ""`) has
  the same meaning as an absent one and is dropped on re-save.

## Known gaps (open questions)

- **Unknown keys and the version token (#788, fixed).** `ProjectData`,
  `SheetEntry` and `LibraryEntry` each carry a `#[serde(flatten)]` bag
  (`project::UnknownJson`, raw `serde_json::Value`s, so `null` survives) that
  is written back on save. Every save stamps `"format": "snxprj/1"`. A file
  with no token predates it and loads as version 1. A file with any other
  token (a newer `snxprj/2`, or garbage) is refused with
  `ProjectError::UnsupportedFormat` and is never rewritten: a newer version may
  change the meaning of existing keys, which the bag cannot preserve, so
  opening read-write would risk silent degradation. This matches the
  `.snxsch` / `.snxpcb` behaviour on a mismatched `format` token.
  `.snxlib`, `.snxsym` and `.snxfpt` still carry a token but no unknown-key
  preservation.
- **`.snxfpt` pad table (#960).** Each footprint's `pads_tsv` block starts
  with 13 base columns (`number` .. `paste_margin`), always in that order.
  The remaining `Pad` fields (net, testpoint, locked, per-side mask/paste
  overrides, tented/paste-enabled flags, thermal relief, features, hole
  tolerances, and so on) are optional trailing columns. A column is written
  only when at least one pad in that footprint holds a non-default value for
  it, in a fixed canonical order, so a footprint of plain pads keeps the
  13-column header and re-saves byte-identically. Readers map columns by
  name: a missing column means the `Pad::default()` value, and an unknown or
  duplicated column is refused with `PadsTsvSchemaMismatch` (there is no
  unknown-column preservation for this table). The token stays `snxfpt/1`
  before v1.0, so a build older than #960 refuses a newer file that carries
  extended columns with `PadsTsvSchemaMismatch` rather than dropping them.
  Free-text cells (`net`, `template`, `template_library`) must be free of
  control characters and `'''`; a save with one is refused.
- **Should `.snxprj` be TOML?** Undecided. Moving it would give it the
  envelope, the version token and the unknown-key machinery for free, at the
  cost of a breaking file-format change and a migration path for existing
  projects. If it is ever done it needs its own breaking-change issue.
