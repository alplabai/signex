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
- **Legacy sniffing is trivial.** `parse_project` strips any number of leading
  UTF-8 BOMs (U+FEFF, via `trim_start_matches`) after a successful decode; a
  file that is not valid UTF-8 is `ProjectError::Corrupt`. Only an empty or
  whitespace-only file takes the directory probe. Any other non-blank text
  that does not start with `{`, and any JSON body that fails to parse, is
  `ProjectError::Corrupt`, so a bad file cannot be silently replaced by an
  empty project on the next save.

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
- `style.color` is `#RRGGBB` or `#RRGGBBAA`; `style.line` is `solid`,
  `dashed` or `dotted`, absent meaning Default: inherit the project's
  `schematic_defaults.wire_line_style`, then solid (#2026). Builds before
  #2026 never wrote `solid`, so an absent key in an older file keeps its
  meaning and an explicit `solid` overrides a dashed project default;
  `style.wire_width_mm` is the wire width in millimetres. Each is optional, and an absent value leaves the
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
  - **width**: the wire's own `stroke_width`, else the class width, else the
    project default, else 0.254 mm (#2026; before it the class width won);
  - **line style**: the class line style when set, else the project default,
    else solid.

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
- `profile` records which portable profile the rules came from (name,
  `source` of `builtin` / `user` / `file`, the profile's `version` and the
  `sha256` of its rules; see [`snxrules-format.md`](snxrules-format.md)). It is
  written by "Store ERC rules in this project" when the snapshot equals the
  shipped profile, by "Save as profile", and by "Update from profile". It is
  provenance only: ERC never reads a profile file, only the project's own
  `erc`. `drc` is reserved: it is carried verbatim in the section's
  unknown-key bag until its feature lands.
- A section this build cannot interpret never stops the project from opening
  (#1180). A `schema` above 1, or a value whose shape changed (a severity that
  is an object, say), makes the whole section opaque: it is kept verbatim in
  the project's unknown-key bag, written back untouched, and not applied; ERC
  uses the machine preferences and says so once per open. A later build may
  therefore change a value's shape only together with a `schema` bump.
- `schema`, `erc` and every `profile` field are optional and omitted when
  absent or empty. An explicitly empty value (`"erc": {}`, `"name": ""`) has
  the same meaning as an absent one and is dropped on re-save.

## Annotation settings (#1232)

`ProjectData.annotation` is an optional top-level `annotation` object, added
without bumping the `snxprj/1` token. It is deliberately not a child of
`rules`: a present `rules` section means "this project stores its ERC rules".

```json
"annotation": { "designator_standard": "din40719_2" }
```

- `designator_standard` is one of `ieee315`, `din40719_2`, `iec81346_2`; absent
  means the library symbol's own designator letters decide. The section is
  omitted entirely when nothing is set, so an untouched project re-saves
  byte-identical.
- Placement derives the reference prefix from the part's component class under
  the standard of the project that owns the target sheet
  (`signex_types::designator::resolve_prefix`). `#` references (power ports,
  flags) and the `generic` class are never remapped.
- A library row can override the table prefix with the `designator_prefix` text
  parameter in its `ParamMap` (`ComponentRow::designator_prefix_override`);
  precedence is the edited reference, the row override, the class table, then
  the symbol designator. Re-prefix to standard (#1315) only rewrites
  `Symbol.reference` on the sheets; the `.snxprj` format does not change.
- Parsing is tolerant like `rules`: an unknown standard string (or any value
  this build cannot read) never stops the project opening. The section is kept
  verbatim in the unknown-key bag, written back untouched, not applied, and
  reported in the Messages panel when a part is placed. Choosing a standard in
  the Project tab replaces it.

## Schematic defaults (`schematic_defaults`, #2026)

`ProjectData.schematic_defaults` is an optional top-level object, edited in
the Project tab's Schematic defaults section, added without bumping the
`snxprj/1` token.

```json
"schematic_defaults": { "wire_width_mm": 0.3, "bus_width_mm": 0.6, "text_size_pt": 12, "font_family": "Roboto" }
```

- Keys: `wire_width_mm`, `bus_width_mm`, `symbol_line_width_mm` (symbol lines,
  and so pins), `shape_stroke_mm`, `junction_diameter_mm`, `text_size_pt`,
  `font_family`, `wire_line_style` (`solid` / `dashed` / `dotted`),
  `shape_fill` (`none` / `outline` / `background`). Every key is optional; a missing key, or a number that is not
  above zero, falls back to the built-in table
  (`signex_types::schematic::SchematicDefaults::BUILT_IN`: 0.254, 0.45, the
  wire width, 0.254, 0.7 mm, 10 pt, Iosevka; a stored 0 draws at the
  default and an explicit width is floored at 0.15 mm, as #2091 set). An unset symbol line follows the
  project's wire width.
- Only unset values follow it: an item stored with width or size `0`. An
  explicit value in a sheet is never rewritten. A wire resolves explicit width
  > net class width > project > built-in; a bus stores no width and has no
  class, so project > built-in.
- The canvas and every export resolve through the same table, and so do the
  hit tolerance and selection boxes of wires and buses (symbol boxes do not,
  see the known gaps); placement tools start from the project's shape stroke
  and shape fill, and labels and notes placed by click from its text size and
  font.
- Line style: a wire stores none, so a wire whose net class sets none (the
  Net Classes panel's Default) strokes in the project's `wire_line_style`; a
  class's explicit Solid, Dashed or Dotted overrides it.
- Shape fill and font apply to placement only: a placed shape always stores
  its fill, and a placed label or note stores the project font (or no font,
  the sheet default, when the project names none). Stored values are never
  reinterpreted.
- Omitted when nothing is set. Read key by key: every readable key applies;
  a key this build does not know, or a known key whose value it cannot read
  (a newer build's line style name, say), is kept raw and written back on
  every save, edits of other keys included, until that same default is set
  here. A section that is not an object is kept verbatim and not applied.
- Editing in the Project tab: each default is its own undo step (typing in one
  field is one step); text that is not a usable number keeps the stored value
  and the typed text, and only empty text clears the default. The junction
  diameter range starts at 0.7 mm, its draw floor.
- Ranges: widths resolve clamped to 0.15–5 mm (bus 0.15–10 mm), text to
  1 pt–the size the `.snxsch` loader repairs past (100 mm). The Project tab
  stores a typed value clamped and says so under the field; a file value out
  of range resolves clamped and is not rewritten. Placement uses whole points
  (`SchematicDefaults::placed_text_pt`), the same on every placement path.
- Known gaps: text ghosts (labels, ports, off-sheet connectors, notes) are
  armed at a fixed 10 pt with no font while the click places at the project
  text size, and the designator and value text of a placed part stays at the
  built-in 1.27 mm; symbol hit and selection boxes and the PDF fit-to-page box use
  the symbol geometry and stored widths, not the resolved stroke, so with a
  thick project symbol line width the ink can reach past them (growing them
  needs a maintainer decision); an explicit `null` key is dropped on save; clearing a field
  also drops an unreadable value kept for that key (unlike the
  designator-standard rule, where clearing over an unreadable section keeps
  it); a polyline placed with Width untouched stores 0, not the shape default
  (`selection_workflow.rs`), so it follows a later default change while other
  shapes keep the value they were placed with.

## Description (`description`) and the name (#1202)

`ProjectData.description` is an optional free-text string, edited in the
Project tab's General section. It is omitted when absent, and the editor stores
an empty description as absent, so a project without one saves byte-identical
to before.

`name` is written to the file but is not authoritative: `parse_project`
replaces it with the `.snxprj` file stem. The Project tab's Rename… therefore
renames the file, through the same flow as the project tree's Rename; saving
never renames anything
(`docs/superpowers/specs/2026-10-26-project-tab-shell-design.md`).

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
  A `.snxlib` keeps unknown keys by writing back the text it was read from
  with only the edited content changed (#1435). Class `key`s must be unique:
  a file with duplicates still opens (a warning goes to the Messages panel),
  but a save of it is refused with `DuplicateClassKey` until the file is
  edited so each key appears once. `.snxsym` and `.snxfpt` still
  carry a token but no unknown-key preservation.
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
  before the Community Preview, so a build older than #960 refuses a newer file that carries
  extended columns with `PadsTsvSchemaMismatch` rather than dropping them.
  Free-text cells (`net`, `template`, `template_library`) must be free of
  control characters and `'''`; a save with one is refused.
- **Should `.snxprj` be TOML?** Undecided. Moving it would give it the
  envelope, the version token and the unknown-key machinery for free, at the
  cost of a breaking file-format change and a migration path for existing
  projects. If it is ever done it needs its own breaking-change issue.
