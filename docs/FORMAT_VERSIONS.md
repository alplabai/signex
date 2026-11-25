# Format versions

Every native file Signex writes starts with a format token,
`<kind>/<major>[.<minor>]` (`snxsch/1.1`). The symbol and simulation files
spell the major with a `v` (`snxsym/v3`). One policy, `FormatVersion` in
`signex_types::format_version` (#2207), checks the token of every format; a
format declares only its own table row below.

## The rules

- **Save** writes the format's current token. The exceptions are `.snxsym`,
  `.snxfpt`, `.snxsch`, `.snxpcb` and `.snxprj`: each writes the lowest token the content needs. A
  symbol file without polygons, De Morgan bodies or the `v4` shapes (table below) stays `snxsym/v1`; a
  schematic stays `snxsch/1.1`, byte for byte, unless a No-ERC directive
  names the rules it silences, which writes `snxsch/1.2` (#1699), or it holds
  any Schematic Refinement content (a harness object, a union, a Bezier,
  dimension or table drawing, a Net Parameters directive, or a Blanket or
  Differential Pair using a field added with them), which writes
  `snxsch/1.3` (`docs/architecture/snxsch-refinement-objects.md`), or a bus
  with its own width or colour, which writes `snxsch/1.4`
  (`docs/architecture/snxsch-wire-color.md`). A board
  stays `snxpcb/1.4`, byte for byte, writes `snxpcb/1.5` when a layer has a
  colour of its own, `snxpcb/1.6` when it has rooms (#2457) and `snxpcb/1.8`
  when its stackup has a name (#2833), a board sketch pattern repeats a baked
  target (#2517) or an array has a numbering sweep other than row by row
  (mockup row 126), `snxpcb/1.9` when a pour is shelved (#2738),
  `snxpcb/1.10` when its stackup carries a finish or a layer's loss tangent
  (mockup rows 176 and 177) and `snxpcb/1.11` when it has a pour cutout
  (#2871), a dimension with a non-default look (#2868) or of a kind other
  than aligned (#2507), a drawing or
  string on a copper layer (#2883) or an unfilled pour (#2506). A project stays `snxprj/1`, byte for
  byte, and writes `snxprj/1.1` when it has variant settings. A build that
  reads only the lower token opens such a sheet or board read-only. A project
  is different: a released build refuses a `snxprj/1.1` project on load; see
  the `.snxprj` variant settings row below.
- **Load** reads every token in the format's readable list. `snxsch/1` and
  `snxsch/1.0` are the same version.
- **A newer token** is refused on load with an error that names the token,
  says a newer Signex wrote it and lists what this build reads. The caller
  adds the file name (`parse schematic <path>`, `project file <path>`).
- **A newer minor** of `.snxsch` / `.snxpcb` / `.snxprj` opens for viewing instead: the
  document remembers the token and every save, Save As included, is refused,
  so content this build cannot represent is never dropped (#977, #1402). A
  read-only project shows a banner on its Project tab, gets no recovery copy,
  and changes to its file list are refused before any file moves. Close
  Project and Exit still save every other file with Save All, skipping it,
  and offer a discard that drops only its unsaved settings (#2904). The one
  write of such a project is File ▸ Save Project As… (#2908): a copy in a new
  folder whose `.snxprj` is written in this build's format. Unknown keys are
  kept where the object holding them has a bag for them. A key the copy
  would still leave out or change (a leniently read value such as
  `fab_outputs`, or a field of an object with no such bag) is listed in the
  dialog, and nothing is written until the person agrees to go without it
  (`docs/PROJECT_SETUP.md`, "Save Project As"). A newer-minor sheet or board
  is copied byte for byte, so it stays read-only in the copy.
  The original file is never written.
- **An older or foreign token** that is not in the list, or a garbled one
  (`snxsch/01`, `snxsch/1.`, a number in a `.snxprj`), is refused naming it.
- **No token** is refused, except in `.snxprj`, where a file with no token
  predates it and reads as `snxprj/1` (#788).

## Formats

| File | Writes | Reads | Newer minor | No token |
|---|---|---|---|---|
| `.snxsch` | `snxsch/1.1`, `1.2` with a No-ERC rule list, `1.3` with Schematic Refinement content, `1.4` with a bus's own width or colour | `snxsch/1`, `1.1`, `1.2`, `1.3`, `1.4` | opens read-only | refused |
| `.snxpcb` | `snxpcb/1.4`; `1.5` with layer colours; `1.6` with rooms | `snxpcb/1` to `snxpcb/1.6` | opens read-only | refused |
| `.snxpcb`, board sketch targets | `snxpcb/1.7` while the board sketch carries a slot, pour, mechanical drawing or component anchor (decision 2, 2026-11-20), or a reference dimension, pattern link, pattern mirror or pad override (decisions 5, 9, 10), rooms or not; else the token above, byte for byte | adds `snxpcb/1.7` | opens read-only | refused |
| `.snxpcb`, stackup name, patterned targets, numbering sweep | `snxpcb/1.8` while `[stackup]` carries `stack_name` (mockup row 105, #2833), an array or mirror repeats a baked target (a source on or connected to one; #2517), or an array's numbering sweep is not row by row (mockup row 126), whatever else the board holds; else the tokens above, byte for byte | adds `snxpcb/1.8` | opens read-only | refused |
| `.snxpcb`, shelved pour | `snxpcb/1.9` while any pour is shelved (#2738, DECISIONS 2026-11-23), whatever else the board holds; else the tokens above, byte for byte | adds `snxpcb/1.9` | opens read-only | refused |
| `.snxpcb`, stack materials | `snxpcb/1.10` while `[stackup]` carries a `finish` table (solder mask, silkscreen, surface finish; mockup row 176) or a stack row carries `loss_tangent` copied from the material library (mockup row 177), whatever else the board holds; else the tokens above, byte for byte. A 1.9 build would drop the row's Df on save | adds `snxpcb/1.10` | opens read-only | refused |
| `.snxpcb`, pour cutouts, dimension look | `snxpcb/1.11` while the board has a pour cutout (`[[pour_cutouts]]`, #2871) or a `[[free.dimensions]]` row writes a look key or a `kind` key (`format::board_uses_dimension_style`, #2868, #2507; DECISIONS 2026-11-24), whatever else the board holds; else the tokens above, byte for byte. A 1.10 build would drop the look and kind keys on save | adds `snxpcb/1.11` | opens read-only | refused |
| `.snxpcb`, drawn copper | `snxpcb/1.11` while a board drawing (`graphics`) or board string (`texts`) sits on a copper layer (`format::board_has_drawn_copper`, #2883; DECISIONS 2026-11-25), whatever else the board holds; else the tokens above, byte for byte. A 1.10 build would not count it as copper | `snxpcb/1.11`, as above | opens read-only | refused |
| `.snxpcb`, unfilled pour | `snxpcb/1.11` while a pour that is not shelved carries `unfilled = true` (`Zone::is_unfilled_pour`, #2506; DECISIONS 2026-11-25), whatever else the board holds; else the tokens above, byte for byte, and the key is written only when on. A 1.10 build would flood the pour and export copper the author cleared | `snxpcb/1.11`, as above | opens read-only | refused |
| `.snxmat`, user material library | `snxmat/1` (`materials.snxmat` in the config folder: the person's own cores, prepregs, copper foils and masks, mockup row 177) | `snxmat/1` | refused | refused |
| `.snxprj` | `snxprj/1` | `snxprj/1` | opens read-only | read as `snxprj/1` |
| `.snxprj`, variant settings | `snxprj/1.1` while `variant_settings` is not empty (a default variant or a part entry, `project::snxprj_token`; DECISIONS 2026-11-25); else `snxprj/1`, byte for byte. A `snxprj/1` build would keep the section but not follow a variant rename or removal. Builds released before this policy (v26.11.0 and earlier) refuse any newer minor on load, so they refuse a 1.1 project rather than open it read-only | adds `snxprj/1.1` | opens read-only | read as `snxprj/1` |
| `.snxlib` | `snxlib/2` | `snxlib/2` | refused | refused |
| `.snxfpt` | `snxfpt/1.1`, `1.2` with sketch links (reference dimensions, pattern sources and mirrors, per-instance pad overrides), or `1.3` with generated features (silkscreen outline, pad clearance, pin 1 marker, assembly outline; mockup row 124, #2834) or a pattern numbering sweep other than row by row (mockup row 126), whatever else it holds | `snxfpt/1`, `1.1`, `1.2`, `1.3` | refused | refused |
| `.snxblk`, block diagram | `snxblk/1` (mockup row 171, #2796; `docs/architecture/snxblk-format.md`) | `snxblk/1` | opens read-only | refused |
| `.snxsym` | `snxsym/v1`, `v2` or `v3` by content; `v4` while any graphic is a Bezier, Pie Chart, Elliptical Arc or Round Rectangle (#2838), whatever else it holds | `snxsym/v1`, `v2`, `v3`, `v4` | refused | refused |
| `.snxsim` | `snxsim/v1` | `snxsim/v1` | refused | refused |

`.snxrules` profiles carry a numeric `schema` key instead of a token
(`docs/architecture/snxrules-format.md`); a newer schema is refused there too.
It is content-driven: `3` while the `fab` block names a `min_slot`, else `2`
while it names a `router_tool_diameter` (mockup row 179d), else as read; this
build reads `1` to `3`.
The project's copy, the top-level `.snxprj` key `router_tool_diameter`, is a
declared `ProjectData` field and needs no token: an older build keeps it as an
unknown key, as `fab_outputs`. No `.snxpcb` content is added for it.

The project's min slot is stored with the rules, `rules.min_slot`, and steps
the `rules` section's own `schema` to `2` while it is present (`1` again once
it is cleared; `ProjectRules::written_schema`). A build that reads only rules
schema `1` keeps a `2` section verbatim and does not apply it, so the value
is never dropped. The `.snxprj` token stays `snxprj/1`.

**`snxsch/1.3` and older builds.** A build that reads up to `snxsch/1.2`
opens a 1.3 sheet read-only and refuses its saves. A build older than the
new drawing kinds and directive fields may refuse to parse the sheet instead
(an unknown drawing or directive `type`); either way nothing is dropped.

**`snxsch/1.4` and older builds.** A build that reads up to `snxsch/1.3`
drops unknown keys of a typed `buses` row on save, so it opens a 1.4 sheet
read-only and refuses its saves. A sheet with no styled bus keeps its lower
token, byte for byte.

**`.snxprj` stays `snxprj/1`.** The Schematic Refinement project settings
(`schematic_design`: channel naming, harness definitions, differential pair
classes) are one new top-level key, omitted at the defaults. An older build
keeps an unknown top-level key and writes it back unchanged, so the key
needs no new token (`docs/architecture/snxprj-format.md`).

## Changing a format

A change an older build would misread takes a new token: add it as the
format's current token, keep the old one in the readable list while files
in it still load, and add a row to `docs/DECISIONS.md`. A minor bump suits
an addition the older build of the same major can show but not save; a
major bump suits a change it cannot show at all.
