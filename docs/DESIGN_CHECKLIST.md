# Design checklist

The design checklist (#1291, mockup row 206) holds the review items a project
closes before a release, grouped by stage: Schematic, Layout and Fabrication.
Each item is Open, Done, or Waived with a reason, has an owner, notes and a
history, and may link to a check whose result it shows or to a design object.
It is a page tab of its project, `<project> — Design Checklist` (see
[PROJECT_TAB.md](PROJECT_TAB.md), "Page tabs"), opened from Project ▸ Design
Checklist or from the project's row in the Projects panel (right-click ▸
Design Checklist).

## The tab

- **Empty.** The tab offers three templates: Release review (12 items),
  Prototype build (6 items) and Blank (no items), each with its item count per
  stage, then the user's own templates under "Your templates". Start from
  template adds the picked template's items. Each built-in item's title is
  written in the current language when the checklist starts and is then plain
  text of the item. Import from another project… picks a `.snxprj` and starts
  from its checklist's items (see "Import"). Nothing is written until the
  project is saved.
- **Started.** The bar shows the checklist's template, the item counts, the
  filter chips with how many items each shows (All, Open, Done, Waived, Mine),
  a filter box over titles, owners and links, Run checks, Add item and More
  (Save as template…, Export CSV…, Reset from template). Under it, one card per
  stage and the release total show closed of all and a bar: done is solid,
  waived is light, so a waived item counts as closed and stays visible. The
  items follow, grouped by stage, with their state, title, owner, link and the
  day of their last edit. A design object's link words cross-probe when
  clicked.
- **Mine.** The items whose owner is the user. The user is the git identity
  (`user.name`), read once off the UI thread when the tab opens, as a waiver's
  author is; case and surrounding spaces are ignored. Until it is known, or
  when git has none, Mine shows nothing.
- **Item pane.** A click on a row opens the item at the right: its title,
  Open, Done and Waived…, the owner, the stage, the link card, the notes, the
  history and Remove. A text box commits on Enter. Waived… asks for the reason
  and does not save without one; Open and Done drop the reason.
- **Unreadable.** A checklist written by a newer version that this one cannot
  read is kept as written. The tab says so and edits nothing.

Every edit is one step of the project's undo history (the one the Project tab
uses) and marks the `.snxprj` unsaved. Undo and redo show the stored values
again.

## Linked checks

An item may link to:

| Link | Text form | Passes when | Open |
|---|---|---|---|
| ERC | `erc` | the project's ERC run has no error finding | the ERC panel |
| DRC | `drc` | the DRC run of the project's board has no error finding | the DRC panel |
| Title block | `title_block` | the project parameters `title`, `document_number`, `rev` and `date` are set | the Project tab's Parameters |
| Requirement | `requirement:<id>` | the project's requirement with that id passes (`check_requirements`) | the Project tab's Requirements |
| Design object | `object:<what>` | nothing is checked | the sheet that holds it, with it selected |

Warnings do not fail the ERC or DRC link. The title block is read at once.
The other checks run when Run checks is pressed, off the UI thread, as the
requirement checks run (see [CLI.md](CLI.md), "Requirement checks"): the
requirements the items link to, and the checklist's own ERC and DRC checks
(`req::checklist::erc`, `req::checklist::drc` in the report). Before a run they
read "not run". The link card's Run check runs only that item's check and
keeps what the last run said of the others.

A linked check that passes on an open item shows **Check passes · Mark done**.
The item does not close by itself: the owner presses Mark done.

### Design objects

A design object's `<what>` is `part:<designator>`, `pin:<designator>.<pin>`,
`net:<name>`, `sheet:<file>` or `board`; any other text is matched as a
designator (the whole text, then its first word), then as a net name. Open
looks for it in the project's open sheets: a part or a pin selects the part's
symbol, a net its first label, a sheet opens the sheet and `board` opens the
project's board. An object no open document holds is not found, and the tab
says so.

### The link picker

"Link a design object or check…" opens the picker under the link card: a
filter box over the checks, the project's requirements, the parts and nets of
its open sheets, its sheets and its board. A row click links the item. Typed
text that matches no row offers "Link to …", which links the text as written
(a link's own text form, else a design object). Remove link clears it.

## History

Each change to an item adds a line to its `history`: what was done
(`created`, `imported`, `title`, `stage`, `owner`, `link`, `notes`, `open`,
`done`, `waived`), the new value where there is one, who (`by`, the git
identity) and the day (`on`). An edit that changes nothing adds nothing. An
item keeps its 50 newest lines. An event a newer version wrote is kept and
shown by its name.

## Templates

Save as template… writes the checklist's items, each with its stage, title and
link (never its state, owner, notes or history), under a name to
`checklist-templates.json` in the user's config folder, off the UI thread and
atomically; a template of the same name is replaced. A file this version
cannot read is never written over: the save is refused and says why. The
empty tab lists the saved templates, and a checklist started from one names it
`user:<name>` as its `template`.

Reset from template puts the items of the template the checklist started from
back in place of every item, numbered from 1 again, as one undo step; keys a
newer version wrote on the checklist are kept. A checklist that started from
no template (its first item was added by hand, or it was imported from one
that had none) has nothing to reset from.

## Export CSV

More ▸ Export CSV… writes one row per item, in list order, under a header of
the file's own key names: `id,stage,state,title,owner,link,reason,notes,updated`.
The header is the same in every language. Lines end in CRLF; a cell holding a
comma, a quote or a line break is quoted. The write is reported in Messages.

## Import

Import from another project… reads the picked `.snxprj` off the UI thread and
starts this checklist from its checklist's items: each comes in open, with its
stage, title and link only, and its history says where it came from. The
checklist keeps the other one's `template`. It is refused when this project
already has a checklist, the picked project has none, or it cannot be read.

## Stored in the project

The checklist is the top-level `design_checklist` key of the `.snxprj`. It is
written only once a checklist exists, so a project without one saves
byte-identical, and no format token changes: a version that predates the
checklist keeps the key as it is. Keys a newer version wrote, on the
checklist, on an item or on a history line, are kept through every edit.

```json
"design_checklist": {
  "template": "release_review",
  "items": [
    {"id": "1", "stage": "schematic", "title": "ERC passes with no errors",
     "state": "done", "link": {"kind": "erc"}, "updated": "2026-11-26",
     "history": [
       {"event": "created", "value": "release_review", "by": "j.berg", "on": "2026-11-18"},
       {"event": "done", "by": "j.berg", "on": "2026-11-26"}
     ]},
    {"id": "4", "stage": "schematic", "title": "Fitted and not fitted parts reviewed",
     "state": "waived", "reason": "Single variant", "owner": "jb",
     "link": {"kind": "object", "target": "part:U5"}}
  ]
}
```

An item id is a number written as text, one above the highest in use.
`updated` is the day of the item's last change.

## Commands

| Command | Operands | Answer |
|---|---|---|
| `open_design_checklist` | none | opens the tab of the active document's project, or of the Projects panel row's project (window only) |
| `project_checklist` | `document`, `filter` (`all`, `open`, `done`, `waived`, `mine`), `owner` | the checklist (below) |
| `project_start_checklist` | `document`, `template` (`release_review`, `prototype_build`, `blank`, `user:<name>`) | the checklist, `changed` |
| `project_add_checklist_item` | `document`, `stage`, `text` (the title) | the checklist, `changed`, `item` (the new id) |
| `project_set_checklist_item` | `document`, `item`, `field` (`title`, `stage`, `owner`, `link`, `notes`), `value` | the checklist, `changed` |
| `project_set_checklist_state` | `document`, `item`, `state` (`open`, `done`, `waived`), `reason` (needed for `waived`) | the checklist, `changed` |
| `project_remove_checklist_item` | `document`, `item` | the checklist, `changed` |
| `project_run_checklist` | `document`, `item` | runs the linked checks (or `item`'s) in the tab; `project_checklist` reads the results |
| `project_reset_checklist` | `document` | the checklist, `changed` |
| `project_import_checklist` | `document`, `path` (the `.snxprj`) | the checklist, `changed` |
| `project_checklist_templates` | `document` | `builtin` and `saved`: each `template` key, `items` and `stages` counts; a saved one also `name` and `saved_from` |
| `project_save_checklist_template` | `document`, `name` | `name`, `template` (`user:<name>`), `templates` (the saved ones) |
| `project_export_checklist_csv` | `document`, `path` | `path`, `rows` |
| `project_checklist_link_choices` | `document` | `choices`: each `group` (`check`, `requirement`, `part`, `net`, `sheet`, `board`) and `link` (the text `project_set_checklist_item` takes) |
| `project_open_checklist_link` | `document`, `item` | `item`, `link`, `opens` (`requirements`, `erc`, `drc`, `parameters`, `sheet`, `board`, `not_found`), and `document` and `object` for a sheet |

`document` names the project or any of its documents. The edits take the same
path as the tab and refuse with the reason (an unknown item, a blank title, a
waive without a reason, an unreadable checklist, a second start, a reset with
no template, an unknown saved template). The template, import and library
commands answer once their file work has run. A headless run has a config
folder of its own, so templates saved in one run are not there in the next.
On the CLI, `project_save_checklist_template` needs the `save` capability and
`project_export_checklist_csv` the `export` one; the import's `path` is
confined to the project folder like any path operand.

`project_checklist` answers with `project`, `last_run` (when the last run
finished, or `null`), `user` (who Mine is: `owner`, else the git identity)
and `started`; for a started checklist also `template`, `counts`, `chips`
(`all`, `open`, `done`, `waived`, `mine`), `filter`, `shown` (the ids the
filter shows) and `stages` (`open`, `done`, `waived`, `total`, the stages by
name), and `items`: each item as stored, with `result` (`passes`, `fails`,
`not_run`, `no_check`, or `null` without a link), `findings` for a failing
link, `check_passes`, true when the link passes and the item is still open,
and `mine`. An unreadable checklist answers `unreadable: true`.
