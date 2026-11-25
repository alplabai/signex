# Snippets and templates

Design reuse on the schematic (#1283). Code: `signex_types::reuse` (where
each entry is kept, its name rules, what a template gives),
`signex_engine::placement_sources` (the sheet a selection is saved as) and
`signex-app/src/app/design_reuse/` (folders, file work off the update
thread, the commands). Board reuse blocks are a separate store
(`app/board_reuse/`, #2410).

## What each one is

- A **snippet** is a `.snxsch` holding a circuit fragment: parts, wires,
  buses, labels, junctions, no-connects, text and drawings, with the bodies
  its parts use. It is placed again the way a design block is: designators
  are numbered as a paste numbers them, and the placement is one undo step.
  Sheet symbols are not saved into a snippet.
- A **sheet template** is a `.snxsch` whose frame a new sheet starts from:
  paper size and font, border and title block, page settings, title block
  values, sheet parameters, and its drawings and text (with new ids). Parts,
  wires and labels in the template stay behind; a circuit is a snippet.
- A **project template** is a project folder holding one `.snxprj`. A new
  project takes the settings it sets (parameters, net classes, designator
  standard, schematic defaults, channel naming, harness and differential
  pair classes, sheet, board and profile defaults); its first sheet takes
  the frame of the template's root sheet (else its first listed sheet).

## Where they are kept

Each entry is kept in one of two scopes:

| Scope | Folder |
|---|---|
| `project` | the folder of the project the sheet belongs to (the project in view when no sheet is named) |
| `library` | `library/` under the user's config folder, shared by every project |

Under either folder:

| Kind | Path |
|---|---|
| Snippet | `snippets/<name>.snxsch` |
| Sheet template | `templates/sheets/<name>.snxsch` |
| Project template | `templates/projects/<name>/` (library only) |

A name is trimmed and at most `MAX_NAME_CHARS` characters of letters,
digits, spaces, `-`, `_` and `.` inside; a name the operating system keeps
for itself is refused. No new file format: snippets and sheet templates are
ordinary sheets, a project template an ordinary project.

## Writing and reading

- A snippet is written off the update thread. A name already taken is
  refused unless `overwrite=true`, which replaces the file atomically; a new
  file is created without clobbering one that appeared meanwhile.
- A sheet template is read when the new sheet is written. A template that
  cannot be read refuses the sheet: no file is written and the project does
  not list it.
- A project template is read before anything of the new project is made. A
  template that cannot be read refuses the project and nothing is made.

## Commands

All four are catalog rows reachable from the CLI (`docs/CLI.md`,
"Snippets and templates"):

- `save_snippet name=.. [scope=..] [ids=..] [document=..] [overwrite=..]`
- `place_snippet name=.. [scope=..] at=x,y [document=..]`
- `list_snippets`
- `list_templates [kind=sheet|project]`

`new_schematic_sheet` takes `template=.. [scope=..]` and
`new_project_with_profile` takes `template=..` (a library project template).

In a window, Save as Snippet saves the selection on the sheet in view to the
project, named after the designators of its parts (`snippet` when it holds
none); Place Snippet (`place_snippet`) opens a file dialog on the project's snippets
folder (else the library's) and puts the picked snippet on the cursor.

## Not drawn yet

The Snippets panel (mockup row 60) is still the planned card: the row draws
no list. Its content (the `list_snippets` entries by scope, drag one onto a
sheet, which places it as `place_snippet` does) and the template pickers of
the New Project and Add New dialogs wait for their mockup rows.
