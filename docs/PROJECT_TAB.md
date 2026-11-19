# Project tab sections

The Project tab (Project ▸ Project Options...) edits the `.snxprj`
in memory. Every edit marks the project dirty, is written by Save, and is
one undo step of the project's history (Ctrl+Z in the tab, or `undo
document=<.snxprj>`). Storing ERC rules is a step of the project's rule
history, which the ERC dialog and the Rules tab share.

## Sections (#1689-#1693)

- **Parameters** (#1080) — the values every sheet inherits, in one list
  with two groups, each folded shut by its header: Title block (the eight
  title-block fields; a sheet's own Title Block value overrides one) and
  Variables (every other parameter). The filter above keeps the rows whose
  shown name or value holds the typed text, ignoring case; the caption
  beside it counts all of them. A value box shows its outline on hover or
  focus; an empty value reads "Not set". A variable's name is edited in
  place: Enter renames it as one undo step, keeping its value. A name that
  is blank, holds a control character, is a title-block key (`title`,
  `rev`, ...) or is already used (names are case-sensitive) is refused: the
  name stays in its box with the reason under the row and nothing changes.
  × on a hovered row removes the variable; Name and Add at the end of
  Variables add one with an empty value. The filter and the folded groups
  are tab state, not project data.
- **Sheets** — the project's sheets in list order (the Sheet N of M a
  sheet's title block shows on the canvas, and the project tree order):
  page, name, file. The root sheet carries a `root` pill and a page whose
  file the last probe found absent (and no open or unsaved document holds)
  a `missing` pill. Move up / Move down are two icon buttons shown while the
  pointer is over the row, dimmed at the ends of the list. A project that
  lists no sheets shows that in a line with **Add New ▸ Schematic**, which
  opens the project tree's New Schematic setup for the project. Under the
  list, the New sheets card holds what a new sheet starts from (size,
  orientation, title block), edited with the Project setup fields, beside a
  preview of that sheet in the schematic canvas colours: the outline, the
  zone border and title block of the canvas's own frame, and the size in
  mm. An undo puts the order back for the sheets it names; a sheet added or
  renamed since keeps its page. The PDF export does not follow this list:
  it numbers pages by the sheet hierarchy (the root sheet, then the other
  pages by file key), and changing that would renumber the nets of an
  exported netlist.
- **Variants** — Base and the variant definitions, the active one picked
  with its radio. A name is edited in place (Enter renames; the active
  variant follows), Remove makes Base active when it removes the active
  one, Add appends. Names are trimmed, at most 64 characters, no control
  characters, unique ignoring ASCII case, and never `Base`; a refused edit
  shows its reason and changes nothing.
- **Libraries** — the project's libraries in list order, with the path as
  stored (cut from the start so the file name stays visible), the kind (in
  the project, shared, global) and the mount state read from the app as a
  pill (mounted, mounted read-only with a lock, missing at the last probe,
  not mounted); a missing library shows a warning glyph before its name.
  Move up / Move down are icon buttons on row hover, as in Sheets. Adding,
  removing and renaming stay in the project tree; a line under the table
  says where.
- **ERC and annotation** — two cards. The ERC rules card names where the
  rules come from in a header pill (User Default for this machine's
  Preferences, Stored in project, or Newer version for an unreadable set
  from a newer Signex). With machine rules, "Store ERC rules in this
  project" sits on the same line as the message. For a readable rule set,
  four tiles count how many rules are set to Error, Warning, Info and off
  (how the rules are set, not ERC results), and "Open Electrical Rules
  Check…" opens the ERC dialog for this project, where severities are
  edited. The Annotation card holds the Annotate undo choice, stored in the
  project (`annotation.annotate_undo`) or following Preferences.
- **Version control** — the Repository card shows the last probe of the
  project folder as a pill (Present, None, Checking…). With a repository it
  gives the status and "Open History panel"; without one, Enable Version
  Control… (the project tree's dialog) is the primary button. Below it,
  "Commit every save" (`enable_git`) is a switch with its help under the
  label: turning it on needs a repository, turning it off keeps the
  repository, and a note says so when it cannot be turned on. "Commits in
  progress" shows only while commits run. The branch, the last commit, the
  commit author and remotes are not shown yet.
- **Schematic defaults** (#2026, Graphite mockup m36) — two cards side by
  side. **Lines**: Wire width, Bus width, Symbol line and pin width, Shape
  stroke (mm), then Wire line style. **Text and markers**: Text size (pt),
  Font, Junction diameter (mm), then New shape fill. A number box carries
  its unit as a suffix and shows the built-in value in grey while empty.
  An edited field has an accent border and a reset button, which clears the
  override exactly as emptying the field does; a value that is not a usable
  number, or one that was clamped into the field's range, gives the box an
  error border, a tinted fill and a warning icon, and a clamped value says
  under the field what is used instead. The two pickers show "Built-in (…)"
  while unset. Under the cards a live preview draws a small schematic in
  the canvas colours with the values in effect (power port, wires with a
  junction, a resistor, a net label, a bus with entries, a shape) and writes
  each value beside its item: grey italic while the field is empty, in the
  warning colour with the typed number when it was clamped. The drawing is
  not editable; values are edited in the fields.

Each section shows its title and help line over its content in a column
that scrolls inside the tab.

## Requirements (#2339)

The project's requirements list (the `.snxprj` `requirements` key; kinds and
fields in [CLI.md](CLI.md), "Requirement checks"), one card per entry. Run
checks is the section header's primary button. Under the header, a summary
row shows the last run's counts (Passed, Failed, and Not checked when there
are any), when it finished, and Add requirement…, which picks a kind and
adds an entry with starting values (`board_size` 100 × 100 mm, `net_class`
all nets with 0.15 mm minimums, `bom_cost` 100 boards at 10 USD each) and
the kind's name as its id, made unique.

A card's header row holds the Enabled switch, the kind, its kind id, the
requirement's id (read-only; `project_set_requirement` with `field=id`
changes it), a one-line summary of its values, the outcome pill, Remove and
an expand button. A passed card is closed to that row and every other
card is open; a card holding a typed value or a refusal stays open. The
expand button flips a card; the flip is forgotten when an add or a remove
moves the list. An open card shows the severity, Allow warnings and the
kind's fields three to a row with the caption above each, then each finding
of the last run with its measured values. A field box commits on Enter; a
value that is not valid (a length or cost that is no number above 0, a count
below 1, an unknown lifecycle state, a `net_class` with neither a class nor a
minimum) stays in its box, which turns to the error colours, with the reason
under it, and changes nothing. An empty box clears an optional field. A
requirement switched off is dimmed, its fields take no input, its pill reads
Off and the counts leave it out. An entry this build cannot read is shown
with the reason and can only be removed; a `requirements` value that is not
a list is not edited at all.

Run checks runs the same job as `check_requirements` over the project as it
is in memory, off the UI thread, and shows each requirement as passed,
failed (with each finding's message and measured values) or not checked,
the run's notes under the summary row, and when it finished. It writes
nothing and says nothing in Messages. Edits are General undo steps; a run is
not an edit.

## Preferences and the project

The Preferences values seed a new project (its ERC rules and its Annotate
undo choice) and are the default of a project that stores none. They never
overwrite a value the project stores.

## Commands

`project_settings document=<project or any of its documents>` answers every
section as one object: `sheets` (`page`, `name`, `file`, in page order),
`variants` (`defined`, `active`; `Base` when none is), `libraries` (`name`,
`path` as stored, `kind` `project_local` / `shared` / `global`, `state`
`mounted` / `read_only` / `missing` / `not_mounted`), `annotation`
(`annotate_undo` `project` / `per_sheet` / null for the Preferences
default, `annotate_undo_effective`, `erc_rules` `project` / `preferences` /
`unreadable`) and `version_control` (`commit_on_save`, `repository`
`present` / `absent` / `unknown`, `pending_commits`) and the requirements:
`requirements` (each entry as stored), `unreadable` (`index`, `reason` of
each entry this build cannot read) and `list_unreadable`.

Each edit runs the tab's own commit path and answers with its section plus
`changed` (false when nothing changed, as for a move at the end of the
list):

| Command | Operands |
|---|---|
| `project_move_sheet` | `sheet` (file as listed), `direction` `up` / `down` |
| `project_add_variant` | `name` |
| `project_rename_variant` | `name`, `value` (the new name) |
| `project_remove_variant` | `name` |
| `project_set_active_variant` | `name`, optional; none or `Base` is Base |
| `project_move_library` | `library` (path as stored), `direction` |
| `project_set_annotate_undo` | `value` `project` / `per_sheet`, optional; none follows Preferences |
| `project_set_version_control` | `on`; on is refused without a repository |
| `project_add_requirement` | `kind`, `id` optional (the kind's name, made unique) |
| `project_set_requirement` | `requirement` (its id), `field` (as the file names it), `value` as typed; empty clears an optional field |
| `project_remove_requirement` | `requirement` |
| `project_enable_requirement` | `requirement`, `on` |

A requirement edit answers with the requirements (as `project_settings`
does) plus `changed`; an id that names no entry, or more than one, is
refused. `project_run_requirements` runs the tab's Run checks and answers at
once; the results land in the tab. `check_requirements` is the query that
answers with the report.

A refused edit changes nothing and says why. The edits are on the agent
channel and refused inside `apply_edits`: they write the project file, which
no sheet's undo step covers. From the palette or a key, without operands,
they point to the Project tab.
