# Project tab sections

The Project tab (Project ▸ Project Options...) edits the `.snxprj`
in memory. Every edit marks the project dirty, is written by Save, and is
one undo step of the project's history (Ctrl+Z in the tab, or `undo
document=<.snxprj>`). Storing ERC rules is a step of the project's rule
history, which the ERC dialog and the Rules tab share.

## Sections (#1689-#1693)

- **Sheets** — the project's sheets in list order (the Sheet N of M a
  sheet's title block shows on the canvas, and the project tree order) with
  Move up / Move down, then what a new sheet starts from (size,
  orientation, title block), edited with the Project setup fields. An undo
  puts the order back for the sheets it names; a sheet added or renamed
  since keeps its page. The PDF export does not follow this list: it
  numbers pages by the sheet hierarchy (the root sheet, then the other
  pages by file key), and changing that would renumber the nets of an
  exported netlist.
- **Variants** — Base and the variant definitions, the active one picked
  with its radio. A name is edited in place (Enter renames; the active
  variant follows), Remove makes Base active when it removes the active
  one, Add appends. Names are trimmed, at most 64 characters, no control
  characters, unique ignoring ASCII case, and never `Base`; a refused edit
  shows its reason and changes nothing.
- **Libraries** — the project's libraries in list order, with the path as
  stored, the kind (in the project, shared, global) and the mount state read
  from the app (mounted, mounted read-only, missing at the last probe, not
  mounted), and Move up / Move down. Adding, removing and renaming stay in
  the project tree.
- **ERC and annotation** — where the ERC rules come from (stored in the
  project, this machine's Preferences, or an unreadable set from a newer
  Signex), how many rules run at each severity, and "Store ERC rules in this
  project". Severities are edited in the ERC dialog. The Annotate undo
  choice is stored in the project (`annotation.annotate_undo`) or follows
  Preferences.
- **Version control** — whether the project folder has a Git repository
  (from the last probe), Enable Version Control… (the project tree's dialog)
  when it has none, and "Commit every save" (`enable_git`). Turning it on
  needs a repository; turning it off keeps the repository.

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
`present` / `absent` / `unknown`, `pending_commits`).

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

A refused edit changes nothing and says why. The edits are on the agent
channel and refused inside `apply_edits`: they write the project file, which
no sheet's undo step covers. From the palette or a key, without operands,
they point to the Project tab.
