# Project tab sections

The Project tab (Project ▸ Project Options...) edits the `.snxprj`
in memory. Every edit marks the project dirty, is written by Save, and is
one undo step of the project's history (Ctrl+Z in the tab, or `undo
document=<.snxprj>`). Storing ERC rules is a step of the project's rule
history, which the ERC dialog and the Rules tab share.

## Page tabs (#2523)

The Project, Rules, Layer Stack and Variant Manager tabs are page tabs:
document tabs that show one of a project's pages rather than a drawing. The
later page kinds (Length Matching, Polygon Manager, BOM Studio, Export Job,
Design Notebook) join them. Two rules hold for every page tab, in
`app/page_tab.rs`:

- **Title.** `<project> — <page>`, for example `dcdc-buck — Rules` or
  `sensor-node — Layer Stack`. The project part is the project's name (the
  `.snxprj` file stem when the name is blank; for a Layer Stack tab of a board
  with no project, the board's file stem) and follows a project rename;
  the page part is a catalog message, read in the current language. The tab
  strip also shows the kind glyph and the close button.
- **Dirty.** A page tab has no unsaved state of its own. It shows the accent
  dot exactly when the file it saves into, its owner, has unsaved work: the
  `.snxprj` for Project and Rules, the board for a Layer Stack tab. The
  owner's path is the tab's path, so a page kind never adds a second dirty
  source (#661).

A Layer Stack or Polygon Manager tab (a board page; the Polygon Manager is in
[PCB_POLYGON_MANAGER.md](PCB_POLYGON_MANAGER.md)) shares its board's path,
and path lookups take the first tab, so it always sits after the board's tab
(`app/layer_stack_tab.rs`). It stays on the main strip while its board is
undocked, and closes with its board.

A symbol's Pin table tab (mockup row 122b, opened by the Pins tab's `Edit pins
in table…`) is a page of its symbol in the same way: its path is the
`.snxsym`, it sits after the symbol editor's tab, Save, Undo and the dirty dot
are the symbol's, and closing the symbol closes it (`app/pin_table_tab.rs`).
Its title is `<symbol> · Pin table`, the symbol's own name rather than a
project's.

A shared page shell, `signex_widgets::doc_tab_shell`, holds the Layer Stack
and Polygon Manager tabs; the Project and Rules tabs do not use it yet. It
draws, from the top, a toolbar row (items from the left, wrapping when the tab is
narrow, and an end group of totals or pills at the right), a strip of
sub-tabs (the active one in the ink colour with an accent underline; a
sub-tab may carry a tag such as "proposal" or a count), the body, and a
footer with a status line and an optional note at the right. The shell holds
no state: the caller passes the selected sub-tab and maps a press to its own
message. The chrome catalog draws a sample of it.

## Sections (#1689-#1693)

- **Parameters** (#1080) — the values every sheet inherits, in one list
  with two groups, each folded shut by its header: Title block (the fifteen
  title-block fields: title, document number, revision, date, company,
  drawn by, checked by, approved by, author, engineer, organization and
  address 1 to 4; a sheet's own Title Block value overrides one) and
  Variables (every other parameter). The filter above keeps the rows whose
  shown name or value holds the typed text, ignoring case; the caption
  beside it counts all of them. A value box shows its outline on hover or
  focus; an empty value reads "Not set". A variable's name is edited in
  place: Enter renames it as one undo step, keeping its value. A name that
  is blank, is not a variable name (letters, digits and `_`, not starting
  with a digit, as Add requires, #1987), is a title-block key (`title`,
  `rev`, ...) or is already used (names are case-sensitive) is refused: the
  name stays in its box with the reason under the row and nothing changes.
  A rename moves only the project key: sheet text that uses `${OLD}` and a
  sheet value named OLD keep the old name and stop resolving to the project
  value, as after Remove; while a typed name would be accepted, a warning
  under the row says so before Enter. × on a hovered row removes the variable; Name and Add at the end of
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
  mm. Its expand button opens the sheet in a larger window, which Close, Esc
  or a press beside it closes. An undo puts the order back for the sheets it names; a sheet added or
  renamed since keeps its page. The PDF export does not follow this list:
  it numbers pages by the sheet hierarchy (the root sheet, then the other
  pages by file key), and changing that would renumber the nets of an
  exported netlist.
- **Variants** — a link: the row opens the project's Variant Manager tab
  (mockup row 32), `<project> — Variants`, and does not change the Project
  tab's section; the project tree's Variant Manager row opens the same tab,
  and opening it again focuses it. The tab saves into the `.snxprj` and its
  edits are undo steps of the project's General history. It sits in the
  shared document-tab shell (`signex_widgets::doc_tab_shell`): the top bar
  is the shell's toolbar and the status line its footer. Its layout (#1293):
  the top bar holds the
  Active variant select (Base or a definition), the parts filter (designator,
  value as written or value shown; terms split at commas and spaces, `U*` a wildcard over the whole
  text, a plain term such as `10k` anywhere in it) and Only rows that differ
  with its count. The list on the left shows Base and each definition with
  how many parts it fits and how many it fits differently from Base, and the
  active pill. A click selects a variant (the selection is not the active
  variant); a double-click or Rename opens its name in place (Enter renames;
  the active variant follows), Delete removes it (removing the active one
  makes Base active). Base cannot be renamed or deleted. `+` (and New
  variant from Base when the project has Base only) adds the first free
  "Variant N" and opens its name. The default variant (Base until another
  is set; not the active one) has a star and a default pill on its row and
  card. Under the list, Duplicate adds "<name> copy" (then "<name> copy 2",
  ...) carrying what the source changes and the fits its sheets give it, and
  opens its name; it waits for a parts read that holds. Set as default moves
  the star. The top bar ends with Settings..., greyed with a "Not built yet"
  tooltip (the variant defaults are `project_set_default_variant`; the
  surface waits for a mockup), Compare variants... and Open in BOM Studio
  (#2949). Compare variants... shows two variants side by side in place of
  the cards and the matrix (mockup row 59, view d): Base on the left, the
  selected variant on the right (the first one while Base is selected), a
  select per side with a swap button between them, one line per difference
  (a part one side fits and the other does not; else each value, footprint
  or field the two fitted sides give another value, the right side as a
  pill), and a footer with the count of differences, the fitted parts on
  each side and Close. It reads the parts read with the project's variant
  settings over it (`VariantMatrix::compare`), as the matrix and the BOM
  do, and changes nothing. Open in BOM Studio opens the BOM preview of the
  project's sheets (its root sheet when open, else its first open sheet)
  with the variant selected in the list picked; with no sheet of the
  project open, Messages says so and nothing opens. The centre shows a
  card per variant
  (parts fitted, cost "—" while no prices are known, changes against Base)
  and the parts matrix: one row per part, a column per variant, Fitted or
  Not fitted, only cells that differ from Base coloured; a cell with a value
  of its own shows the value and what it replaces (value, footprint,
  override). A click selects a cell and the right pane shows that part in
  that variant: a Fitting switch (Fitted, Not fitted; Alternate is greyed,
  not built yet; As on the sheets drops the cell's own fit; picking the fit
  the sheets already give records nothing), the parameter overrides
  (Parameter, Base and variant columns: Value, Footprint and the part's
  fields with Base's value, the variant's value typed in place and stored on
  Enter or when the typing moves to another parameter or cell, an empty one
  or the reset arrow back to Base; greyed while the part is not fitted) and
  the part in the other variants. A cell whose part is not fitted shows Not
  fitted, never a value of its own. Base's cells are the sheets: shown, not
  edited. These settings are stored in the `.snxprj` (`variant_settings`,
  see `docs/architecture/snxprj-format.md`) on the placed part, so they stay
  on it through a re-annotation, and laid over the read as it is shown, so
  an edit shows without reading the sheets again; the BOM lists a variant
  with the same settings. A part on a repeated sheet is one row for every
  channel's copy (fold chevron, its designator on the sheet, the number of
  channels; per variant Fitted, Not fitted or "mixed · 1 of 3 not fitted")
  that a click opens into one row per copy, each set on its own; the right
  pane of a copy also sets every channel's copy at once, one undo step
  (`docs/MULTI_CHANNEL.md`, "Per-channel variant state"). No sheet preview
  of the part yet. Its
  parts are read off the UI thread from every sheet of
  the project, open ones with their unsaved edits, each child sheet once per
  placement as the BOM counts it, with the BOM's own fit rule
  (`signex_output::variant_fit`) and the value the BOM lists (an `=` value
  evaluated on its sheet, #1986); after a sheet edit or a change of the
  project parameters the footer says so and the top bar offers Read again. Names are trimmed, at most 64 characters, no control
  characters, unique ignoring ASCII case, and never `Base`; a refused edit
  shows its reason and changes nothing. The schematic canvas, SVG, PDF and
  print draw a part's value as the active variant (on paper, the printed
  variant) gives it; Base, or a cleared value, draws the sheet's value
  again (#2948). The Settings surface, alternate parts, the cell menu and
  the sheet preview are not built yet.
- **Libraries** — the project's libraries in list order, with the path as
  stored (cut from the start so the file name stays visible), the kind (in
  the project, shared, global) and the mount state read from the app as a
  pill (mounted, mounted read-only with a lock, missing at the last probe,
  not mounted); a missing library shows a warning glyph before its name.
  Move up / Move down are icon buttons on row hover, as in Sheets, and a
  grip in front of each row (Sheets too) drags it: a drop is one undo step,
  Esc cancels. A missing library offers Locate…, the Library Missing
  dialog's repoint: the picked file mounts, the project's entry follows and
  the project is left unsaved. Adding, removing and renaming stay in the
  project tree; a line under the table says where.
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
  gives the status, the Branch button with the branch switcher it shares
  with the History panel (`app/project_git`), the last commit and "Open
  History panel"; without one, Enable Version Control… (the project tree's
  dialog) is the primary button. Both surfaces run one switch
  (`app/project_git/switch.rs`). It is refused, naming why, while a document
  has unsaved edits or a save or commit runs, when it would overwrite a file
  changed since its last commit, when an open library differs, when the
  branch's `.snxprj` does not parse, and when a Git LFS file's content is not
  in `.git/lfs/objects`; a refused or failed switch leaves the project and the
  working tree as they were. When the branch's `.snxprj` differs, unsaved
  edits offer Save and switch, and the project closes and reopens from the new
  file with its tabs. LFS content is restored from `.git/lfs/objects`. Clean
  documents reload; a document edited during the switch keeps its edits and
  raises the File Changed on Disk card. Below it,
  "Commit every save" (`enable_git`) is a switch with its help under the
  label: turning it on needs a repository, turning it off keeps the
  repository, and a note says so when it cannot be turned on. "Commits in
  progress" shows only while commits run. **Author** shows the identity git
  resolves; Enter writes `Name <email>` to the repository's own git config,
  and an empty field removes it there so the user-level identity applies. It
  is never stored in the project file. Remotes are not shown yet. The Enable Version Control
  dialog's own "Commit every save" switch (on by default) decides whether a
  successful Enable turns `enable_git` on, as the same General step.
- **Schematic defaults** (#2026, mockup row 36) — two cards side by
  side. **Lines**: Wire width, Bus width, Symbol line and pin width, Shape
  stroke (mm), then Wire line style. **Text and markers**: Text size (pt),
  Font, Junction diameter (mm), then New shape fill. A number box carries
  its unit as a suffix and shows the built-in value in grey while empty.
  An edited field has an accent border and a reset button, which clears the
  override exactly as emptying the field does. A typed value is never
  clamped: one that is not a usable number or is outside the field's range
  is refused, the stored default stays and the box keeps the text, with an
  error border, a tinted fill and a warning icon; a number outside the range
  also gets a note under the field naming the allowed range. A value already
  in a `.snxprj` is not rewritten and resolves into the range. The two
  pickers show "Built-in (…)" while unset. Under the cards a live preview
  draws a small schematic in the canvas colours with the values in effect
  (power port, wires with a junction, a resistor, a net label, a bus with
  entries, a shape) and writes each value beside its item, grey italic while
  the field is empty. A click on a value opens it for typing on the drawing:
  Enter commits, Tab commits and moves to the next value, Esc cancels, and
  an emptied value returns to the built-in one. It is the field's own edit
  and one undo step; a refused value keeps the label open and shows the
  field's refusal. Handles on the junction and the bus drag their value
  (one undo step; Esc puts the value back). The expand button opens the
  drawing in a larger window, which Close, Esc or a press beside it closes.

Each section but Variants shows its title and help line over its content in
a column that scrolls inside the tab. Variants (mockup row 32) opens the
Variant Manager tab instead; the Variant Manager fills that tab.

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
expand button flips a card; the flip follows its entry through an add or a
remove of another, like a typed value, and goes with a removed entry. An open card shows the severity, Allow warnings and the
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
not an edit. An entry with no id takes its verdict from the run under its
place in the list (`req::#1` for the first), the name the check gives it.

A Part lifecycle card (#2686, mockup row 38) has a Variant pick (the active
variant, shown with its name and stored as no value; Base; then each of the
project's variants; a stored name the project no longer has stays shown) and
a Fitted parts only box, on by default. Both commit at once as one General
step, as `project_set_requirement` with `field=variant` or
`field=fitted_only` does; a variant the project does not have is refused. A
failed card checking fitted parts only says "N of M fitted parts". A net
class card that shows a verdict of the last run says "last run · edits not
checked yet" beside its pill once its fields differ from the values that
run checked, or hold a typed value or a refusal, until the next run.

The Board size card's Measured from names the board outline, the one
reference the check measures, and its preview is read-only: the limits are
typed in their fields (DECISIONS 2026-11-24, #2815).

## Preferences and the project

The Preferences values seed a new project (its ERC rules and its Annotate
undo choice) and are the default of a project that stores none. They never
overwrite a value the project stores.

## Commands

`project_settings document=<project or any of its documents>` answers every
section as one object: `sheets` (`page`, `name`, `file`, in page order),
`variants` (`defined`, `active`, `default`; `Base` when none is), `libraries` (`name`,
`path` as stored, `kind` `project_local` / `shared` / `global`, `state`
`mounted` / `read_only` / `missing` / `not_mounted`), `annotation`
(`annotate_undo` `project` / `per_sheet` / null for the Preferences
default, `annotate_undo_effective`, `numbering` `continuous` /
`page_hundreds` / `page_thousands` (#1292), `erc_rules` `project` /
`preferences` / `unreadable`) and `version_control` (`commit_on_save`, `repository`
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
| `project_set_default_variant` | `name`, optional; none or `Base` is Base (Set as default, #2949) |
| `project_move_library` | `library` (path as stored), `direction` |
| `project_set_annotate_undo` | `value` `project` / `per_sheet`, optional; none follows Preferences |
| `project_set_annotate_numbering` | `value` `continuous` / `page_hundreds` / `page_thousands`, optional; none is continuous (#1292) |
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

The Variant Manager's top bar actions are queries (#2949); both need an
open sheet of the project and change nothing.
`compare_variants document=<…> from=<variant>? to=<variant>?` (`from`
defaults to Base, `to` to the active variant, the first one while Base is
active) answers `project`, `from`, `to` (names as defined; `Base`),
`from_fitted`, `to_fitted` and `parts`: each part the two differ on, in
designator order, with `designator`, `from` and `to` (`fitted`,
`parameters` by name: `Value`, `Footprint`, fields) and `changes`
(`{"kind": "fit"}` or `{"kind": "parameter", "parameter": <name>}`).
`variant_bom document=<…> name=<variant>?` (none is the active variant,
`Base` is Base) answers `project`, `variant`, `parts` (the fitted count)
and `lines` (`designators`, `quantity`, `name`, `value`, `footprint`), the
lines Open in BOM Studio opens on. A project with no variant besides Base,
an unknown name or no open sheet is refused with the reason.

The Variant Manager's part edits and Duplicate are commands too (#3038).
A part is named by `designator` as the Variant Manager lists it (`CH2_R5`
for a channel's copy of R5); the project's parts are read from its sheets
as the tab reads them, so the setting lands on the placed part the matrix
links. Each runs the same section command as the tab's click, one step of
the General history, and answers with the Variants section, `changed` and
`parts` (each part set: `designator`, `fitted` as the variant now sets it,
null for as on the sheets, and its `parameters`):

| Command | Operands |
|---|---|
| `project_set_variant_fit` | `variant`, `designator`, `fitted` optional (none is as on the sheets), `scope` `part` (default) or `channels` (every channel's copy of a part on a repeated sheet) |
| `project_set_variant_alternate` | `variant`, `designator`, `mpn`, `value`, `footprint`, each optional; one left out is the part's own again, none is the part itself again; a value or footprint needs `mpn` |
| `project_set_variant_override` | `variant`, `designator`, `field` (`Value`, `Footprint` or a field name), `value` optional; none is the Base value again |
| `project_duplicate_variant` | `name` (the variant to copy, or `Base`), `value` optional (the copy's name; none is the first free "<name> copy") |

Picking the fit the sheets already give a part that sets none records
nothing (`changed` false), as the Fitting switch does. An alternate part is
its part number (the `MPN` field), value and footprint set together as one
step; the part picker that chooses one is not built.
`project_duplicate_variant` answers `name`, the copy's name. Set as default
is `project_set_default_variant`. A part the read does not list, a `scope`
of `channels` on a part that is not on a repeated sheet, Base and an
unknown variant are refused with the reason.

A refused edit changes nothing and says why. The edits are on the agent
channel and refused inside `apply_edits`: they write the project file, which
no sheet's undo step covers. From the palette or a key, without operands,
they point to the Project tab.
