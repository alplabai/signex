# Workspace layouts

A workspace layout is a named arrangement of the docked panels: which panels
are open, in which dock (left, right, bottom) and in which tab order, whether
a dock is collapsed, the side docks' widths, and the split of the document
area (View ▸ Window ▸ Tile). Signex ships four built-in layouts; you can save
your own. Mockup row 166 (`group-new-release.html`).

Code: `crates/signex-app/src/workspace_layouts/` (data, rules, store),
`crates/signex-app/src/app/handlers/workspace_layouts.rs` (apply, the dialogs'
updates, the file tasks) and
`crates/signex-app/src/app/view/dialogs/workspace_layouts/` (the dialogs).

## What a layout holds, and what it leaves alone

- Stored: the docked panels per dock and their order, each dock's collapsed
  flag, the left and right docks' widths, and the document-area split when
  the main window is tiled (side by side, stacked, or Tile Grid's
  `"tile_grid"`, mockup row 207). Applying a layout with a split tiles the
  documents that way. The pane sizes of a splitter drag are not part of a
  layout: they stay with the project's kept panes (`docs/MENU_BAR.md`,
  "Panes").
- Not stored: open documents. Applying a layout never opens, closes or
  switches a document; it only arranges panels around them.
- Not stored yet: extra windows and their monitors (a document or panel in a
  window of its own). Save Layout shows the option greyed with the number of
  windows open. Follow-up issue: "Workspace layouts: extra windows and
  monitors".
- A panel that is in a window of its own when a layout is applied stays in
  that window. A floating panel the layout does not place keeps floating.

## Built-in layouts {#built-in-layouts}

| Layout | Left dock | Right dock | Bottom dock |
|---|---|---|---|
| Schematic | Projects, Components | Properties | Messages |
| Layout | Projects | Properties, PCB | Layers |
| Review | (empty) | History | Messages, Command Log |
| Notebook | Projects | Annotations, Properties | (empty) |

Every built-in opens the side docks at the default widths (left 240 px,
right 300 px), with no dock collapsed and no split. A built-in with an
override, and any saved layout, keeps the widths it stored. The mock's "Layers strip" under the board is the
Layers panel in the bottom dock: a layout arranges dock panels, and the board
canvas's own layer tabs are not a panel.

A built-in cannot be renamed or deleted. **Update from current** on a
built-in (Manage Layouts) stores your arrangement as its override: the list
says "edited · Reset available", and while that built-in is the active
layout the menu offers **Reset "<name>"**, which drops the override and
applies the built-in as shipped.

## Menu {#menu}

View ▸ Window ▸ Layouts (the Window rows moved into View on 2026-11-19,
#2524; Layouts sits after the tiling rows and before Close All Documents):

- BUILT-IN (a header caption, not a row:
  [MENU_BAR.md](MENU_BAR.md#header-entries)), then Schematic, Layout, Review and Notebook. The
  active layout carries the check mark.
- YOUR LAYOUTS (a header caption) and one row per layout of yours, in your
  order, when you have any; the active one is checked.
- Save Layout…, Manage Layouts…, and Reset "<name>" while the active
  built-in is edited.

The four built-in rows run the commands `apply_layout_schematic`,
`apply_layout_board`, `apply_layout_review` and `apply_layout_notebook`; Save
Layout…, Manage Layouts… and Reset run `save_workspace_layout`,
`manage_workspace_layouts` and `reset_workspace_layout`. All are in the
command palette. A row of yours sends `MenuMessage::WorkspaceLayoutChosen`
with the layout's id. The macOS system menu bar shows the same rows (the
native menu takes the third level).

Save Layout… draws the save glyph and Reset the undo glyph in the check
column (`MenuItem::icon`, mockup row 166); the native macOS menu draws no
icon. Not drawn as in the mock: the active row's check mark is the menu's
usual check, not an accent one; there is no hover preview beside the menu
(2026-11-22 decision), the thumbnails show in Manage Layouts; and a layout
of yours carries no "2 windows" tag, as extra windows are not stored yet.

Applying is manual: Signex never switches layout by itself (for example when
a board tab comes to the front).

## Shortcuts {#shortcuts}

The built-in apply commands are unbound. Ctrl+Alt+1 to Ctrl+Alt+4 are free in
both built-in keyboard profiles (no binding in any context), but they were
left unbound: the catalog's suggested key is read by nothing (each profile
file binds its own keys), and on Windows Ctrl+Alt is AltGr, which types
characters such as `#` and `$` on some keyboard layouts (Turkish Q: AltGr+3
and AltGr+4). Binding them is the maintainer's call; any key can be bound to
the commands in Preferences ▸ Keyboard Shortcuts. Shortcuts for your own
layouts are not built yet (the Shortcut field is greyed). Follow-up issue:
"Workspace layouts: keyboard shortcuts for user layouts".

## Save Layout {#save-layout}

A 460 px dialog. Name is focused and offers the first free "Layout N".
Typing the name of one of your layouts (case and outer spaces do not count)
shows "Replaces your layout "X"." under the field and the button reads
Replace; Replace keeps that layout's place in your list. A built-in's name
shows ""X" is a built-in layout. Pick another name." and Save is off.
Shortcut is a greyed "None".

Store:

- Open panels, their docks and sizes (on).
- Split of the document area (on; greyed while the main window is not tiled,
  as there is no split to store).
- Extra windows and their monitors (off, greyed; "This arrangement has N
  windows.").

Save is off when the layout would store nothing (panels off and no split).
The saved layout becomes the active one. An info banner says open documents
are not part of a layout.

## Manage Layouts {#manage-layouts}

An 820 × 520 dialog, "stored in your preferences, not in the project".

- Left, 280 px: BUILT-IN rows (thumbnail, name, "built-in" or "edited ·
  Reset available", a lock), then YOUR LAYOUTS rows (a grip, thumbnail,
  name, "—" for the shortcut). Every row selects. Drag a row of yours by its
  grip to reorder; the list shows the new order while you drag and keeps it
  when you let go over the list.
- Right: the name in the heading weight and a "yours" or "built-in" pill;
  Rename (an inline field: Enter keeps the name, Esc closes the field; a
  name another layout has is refused and said under the field), Duplicate
  ("<name> copy", made unique with " (2)", placed after its source; a
  built-in's copy is yours and goes last) and Delete. Rename and Delete are
  greyed on a built-in, with the tooltips "Built-in layouts cannot be
  renamed" / "… deleted". Then a 200 px thumbnail; Panels (one chip per
  panel), Windows ("1 · main"), Shortcut (greyed "None") and Saved ("date ·
  from the current arrangement", or "built-in" / "built-in · edited date");
  Update from current, Apply and Undo. Apply keeps the dialog open.
- Undo (a ghost button next to Apply, #2930) takes back the last change
  made in the dialog: a rename, a duplicate, a delete or its toast Undo, a
  reorder, Update from current or an import. Each change is a step, newest
  first, up to 32; Undo is greyed with none left, and closing the dialog
  drops them. The list goes back as it was and is written again. Apply is
  not a step: the layout on screen keeps its check mark when the list still
  has it. Undo closes the "Deleted layout" toast, so the delete is not put
  back twice.
- Delete removes the layout at once and shows a dark toast at the bottom
  centre, "Deleted layout "X"" with Undo, for eight seconds. Undo puts the
  layout back at its place. No confirmation is asked: the layouts are
  preferences, and Undo is enough.
- Footer: Import… and Export… (ghost buttons; Export is greyed with no
  layout of yours) and Close. Esc closes an open Rename field first, then
  the dialog.

The thumbnail draws the docks in proportion to their widths: a sunken frame,
one block per dock (the dock's tabs share it, as they share the dock), and
the document area as a panel-coloured block with a border. The mock's accent
tint on some blocks is not drawn.

## Storage {#storage}

Per user, in `prefs.json`, never in a project:

```json
"workspace_layouts": {
  "active": "user-2",
  "order": ["user-2", "user-1"],
  "user": {
    "user-1": { "name": "Bring-up bench",
                "dock": { "left": {…}, "right": {…}, "bottom": {…} },
                "split": "side_by_side",
                "saved_at": "2026-11-17 16:42", "from_current": true }
  },
  "overrides": { "schematic": { "dock": {…}, "saved_at": "…", "from_current": true } }
}
```

The dock regions use the same form, and the same code, as the `dock` key
(`fonts::region_to_json` / `region_from_json`, `dock_from_snapshot`): an
applied layout and a restored dock are built the same way. `active` is a
built-in key (`schematic`, `layout`, `review`, `notebook`) or a user id.

The list is read once at start. Reading is tolerant: an unknown panel key
drops that panel only; an entry that is not readable (no name, no panels and
no split, an unknown built-in) is skipped and said in Messages; a user
layout missing from `order` is kept at the end. Every change is written off
the UI thread, in the order made, through the ordered preference writer, so
quit waits for it; a failed write is said in Messages.

What this build cannot read is never replaced (DECISIONS 2026-11-23, #2746;
`store::KeptEntries`):

- A skipped entry goes back into every write unchanged, a user entry at its
  place in `order`, and so does any key beside the four above. A new layout
  never takes a kept entry's id. Update from current is refused on a built-in
  whose stored edit is kept.
- A value without the list's shape (not an object, or a wrong type under
  `user`, `overrides`, `order` or `active`) is unreadable as a whole. Only the
  built-ins are offered and they still apply, but `prefs.json` keeps the value
  untouched: nothing is written, Save, Rename, Duplicate, Delete, reorder,
  Update from current and Import are refused, and Save Layout and Manage
  Layouts show a banner saying why.

Export writes your layouts (not the built-ins or their edits) to a JSON file
picked through the file dialog port:
`{"signex_workspace_layouts": 1, "layouts": [ … ]}`. Import reads such a
file, adds each layout at the end of your list with a new id, and gives a
name in use " (2)", " (3)", …; an entry it cannot read is skipped and said
in Messages, and a file of a newer format is refused.
