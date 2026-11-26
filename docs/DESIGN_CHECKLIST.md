# Design checklist

The design checklist (#1291, mockup row 206) holds the review items a project
closes before a release, grouped by stage: Schematic, Layout and Fabrication.
Each item is Open, Done, or Waived with a reason, has an owner and notes, and
may link to a check whose result it shows. It is a page tab of its project,
`<project> — Design Checklist` (see [PROJECT_TAB.md](PROJECT_TAB.md), "Page
tabs"), opened from Project ▸ Design Checklist.

## The tab

- **Empty.** The tab offers three templates: Release review (12 items),
  Prototype build (6 items) and Blank (no items). Start from template adds the
  picked template's items. Each item's title is written in the current
  language when the checklist starts and is then plain text of the item.
  Nothing is written until the project is saved.
- **Started.** The bar shows the checklist's template, the item counts, the
  state filter (All, Open, Done, Waived), a filter box over titles, owners and
  links, Run checks and Add item. Under it, one card per stage and the release
  total show closed of all and a bar: done is solid, waived is light, so a
  waived item counts as closed and stays visible. The items follow, grouped by
  stage, with their state, title, owner, link and the day of their last edit.
- **Item pane.** A click on a row opens the item at the right: its title,
  Open, Done and Waived…, the stage, the owner, the link, what the link says
  now, the notes and Remove. A text box commits on Enter. Waived… asks for the
  reason and does not save without one; Open and Done drop the reason.
- **Unreadable.** A checklist written by a newer version that this one cannot
  read is kept as written. The tab says so and edits nothing.

Every edit is one step of the project's undo history (the one the Project tab
uses) and marks the `.snxprj` unsaved. Undo and redo show the stored values
again.

## Linked checks

An item may link to:

| Link | Text form | Passes when |
|---|---|---|
| ERC | `erc` | the project's ERC run has no error finding |
| DRC | `drc` | the DRC run of the project's board has no error finding |
| Title block | `title_block` | the project parameters `title`, `document_number`, `rev` and `date` are set |
| Requirement | `requirement:<id>` | the project's requirement with that id passes (`check_requirements`) |
| Design object | `object:<what>` | nothing is checked |

Warnings do not fail the ERC or DRC link. The title block is read at once.
The other checks run when Run checks is pressed, off the UI thread, as the
requirement checks run (see [CLI.md](CLI.md), "Requirement checks"): the
requirements the items link to, and the checklist's own ERC and DRC checks
(`req::checklist::erc`, `req::checklist::drc` in the report). Before a run they
read "not run".

A linked check that passes on an open item shows **Check passes · Mark done**.
The item does not close by itself: the owner presses Mark done.

## Stored in the project

The checklist is the top-level `design_checklist` key of the `.snxprj`. It is
written only once a checklist exists, so a project without one saves
byte-identical, and no format token changes: a version that predates the
checklist keeps the key as it is. Keys a newer version wrote, on the
checklist or on an item, are kept through every edit.

```json
"design_checklist": {
  "template": "release_review",
  "items": [
    {"id": "1", "stage": "schematic", "title": "ERC passes with no errors",
     "state": "done", "link": {"kind": "erc"}, "updated": "2026-11-26"},
    {"id": "4", "stage": "schematic", "title": "Fitted and not fitted parts reviewed",
     "state": "waived", "reason": "Single variant", "owner": "jb"}
  ]
}
```

An item id is a number written as text, one above the highest in use.
`updated` is the day of the item's last change.

## Commands

| Command | Operands | Answer |
|---|---|---|
| `open_design_checklist` | none | opens the tab of the active document's project (window only) |
| `project_checklist` | `document` | the checklist (below) |
| `project_start_checklist` | `document`, `template` (`release_review`, `prototype_build`, `blank`) | the checklist, `changed` |
| `project_add_checklist_item` | `document`, `stage`, `text` (the title) | the checklist, `changed`, `item` (the new id) |
| `project_set_checklist_item` | `document`, `item`, `field` (`title`, `stage`, `owner`, `link`, `notes`), `value` | the checklist, `changed` |
| `project_set_checklist_state` | `document`, `item`, `state` (`open`, `done`, `waived`), `reason` (needed for `waived`) | the checklist, `changed` |
| `project_remove_checklist_item` | `document`, `item` | the checklist, `changed` |
| `project_run_checklist` | `document` | runs the linked checks in the tab; `project_checklist` reads the results |

`document` names the project or any of its documents. The edits take the same
path as the tab and refuse with the reason (an unknown item, a blank title, a
waive without a reason, an unreadable checklist, a second start).

`project_checklist` answers with `project`, `last_run` (when the last run
finished, or `null`) and `started`; for a started checklist also `template`,
`counts` and `stages` (`open`, `done`, `waived`, `total`, the stages by name),
and `items`: each item as stored, with `result` (`passes`, `fails`,
`not_run`, `no_check`, or `null` without a link), `findings` for a failing
link, and `check_passes`, true when the link passes and the item is still
open. An unreadable checklist answers `unreadable: true`.
