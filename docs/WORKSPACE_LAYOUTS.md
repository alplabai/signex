# Workspace layouts

A workspace layout is a named arrangement of the docked panels: which panels
are open, in which dock (left, right, bottom) and in which tab order, whether
a dock is collapsed, the side docks' widths, and the split of the document
area (View ▸ Window ▸ Tile). Signex ships four built-in layouts; you can save
your own. Graphite mock: m166 in `group-new-release.html` (#166).

Code: `crates/signex-app/src/workspace_layouts/` (data, rules, store),
`crates/signex-app/src/app/handlers/workspace_layouts.rs` (apply, the dialogs'
updates, the file tasks) and
`crates/signex-app/src/app/view/dialogs/workspace_layouts/` (the dialogs).

## What a layout holds, and what it leaves alone

- Stored: the docked panels per dock and their order, each dock's collapsed
  flag, the left and right docks' widths, and the document-area split when
  the main window is tiled (side by side or stacked).
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

Every built-in opens the side docks at the default widths, with no dock
collapsed and no split. The mock's "Layers strip" under the board is the
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

Not drawn as in the mock: the menu model has no row icons, so Save Layout…
and Reset carry no icon, and the active row's check mark is the menu's usual
check, not an accent one. The mock's hover preview beside the menu is not
possible in a menu; the thumbnails show in Manage Layouts.

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
  Update from current and Apply. Apply keeps the dialog open.
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

Export writes your layouts (not the built-ins or their edits) to a JSON file
picked through the file dialog port:
`{"signex_workspace_layouts": 1, "layouts": [ … ]}`. Import reads such a
file, adds each layout at the end of your list with a new id, and gives a
name in use " (2)", " (3)", …; an entry it cannot read is skipped and said
in Messages, and a file of a newer format is refused.
