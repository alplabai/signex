# Main menu bar

The bar is plain data: `menu_bar_model(&MenuContext)` in
`crates/signex-app/src/menu_bar/model/` builds it, and `menu_bar/view.rs`
renders it as iced_aw menus. Its content is pinned by
`menu_bar/testdata/menu_bar.golden.tsv`; changing a menu, label, shortcut,
order or message is a content change and needs the maintainers' sign-off.

## How a row runs {#row-routing}

A row that carries a command id (`cmd_item`, `cmd_item_gated`; every row with
a shortcut) publishes `MenuMessage::Command(id)`. The main window sends it to
`handle_menu_message`, which runs the id through `Signex::dispatch_command`,
the same entry the shortcut, the palette and the context menu use, so an id has
one behaviour on every surface. An undocked window sends it as
`UiMsg::MenuInWindow`, and the native menu routes it the same way for the
focused window; both dispatch the id against that window, exactly as the
shortcut typed there does. Only some ids act on that window's own document:
Save, Save As, Print, Close Document and the tab steppers, Undo and Redo, and
the commands that send a canvas event (Fit All). Cut, Copy, Paste, Delete,
Select All, Find, the Place rows, Zoom In / Out and AutoFocus still act on the
main window's active tab, as their shortcuts do.

A menu click that cannot run is not silent: `run_menu_command` logs a command
the window cannot act on to the Messages panel ("Menu: <id> did nothing:
<reason>"), and the bridge already logs an unknown or unmapped id.

No button without a function: `menu_bar/tests/walk.rs` also presses every live
row in each window state (no document, a project alone, a schematic with and
without selection, a dirty one, a symbol editor, a footprint editor, a PCB tab,
an undocked window) and fails if a click moves nothing and starts no job.
`KNOWN_INERT` lists the rows allowed to do that and is empty; it may only
shrink.

A live row's command must run: `menu_bar/tests/walk.rs` checks, for every row
in each kind of window state, that a live row's `CommandOutcome` is `Ran` and a
greyed row's is `Disabled`. The golden fixture still names each such row by the
message it used to send; `COMMAND_ROW_NAMES` in `menu_bar/golden_tests.rs` maps
the id back to that name.

## Root menus {#root-menus}

Ten roots, left to right, as in the Graphite mock-up: File, Edit, View,
Project, Place, Design, Tools, Reports, Window, Help (`ROOT_TITLES`). The model
is shared, so every theme and the native macOS bar show all ten. Project and
Reports were added on 2026-11-07 (maintainer decision, `DECISIONS.md`); they
are built only from existing commands and messages, after the Altium
convention (`UX_REFERENCE_ALTIUM.md`). Moved rows keep their shortcut hint and
enablement; no command left the app.

**Project** (project-wide actions):

- Electrical Rules Check (`open_erc_dialog`; F8 `run_erc` runs at once)
- Project Options... (greyed with no project, like Close Project)
- Close Project

**Tools ▸ Annotation** (top of Tools, then a separator, as Altium's schematic
Tools menu has it): Annotate Schematics..., Reset Schematic Designators...,
Reset Duplicate Schematic Designators..., Annotate Schematics Quietly (Alt+A),
Force Annotate All Schematics (Shift+Alt+A), the planned Back Annotate
Schematics..., and Number Schematic Sheets... (see
[below](#number-schematic-sheets)).

**Reports** (outputs of the open schematic, all greyed without one):

- Bill of Materials... (the one BOM dialog; the old Design ▸ Generate BOM row
  was a second row for the same message and is folded in)
- Netlist (Protel .NET)...
- ERC Report...

**Design** keeps Parameter Manager... only. **File ▸ Export** keeps PDF only.
Project tree actions with no menu message or command id (add or remove a
document, Enable Version Control, History, Validate Project) stay in the tree's
context menu; the menu gets no stub for them.

## Rows that name a different command {#own-action}

Every row that carries a command id runs that id; no row keeps an action of its
own. Two rows point at an id other than the one their key suggests (#1991):

- View ▸ Cycle Grid Size runs `next_grid`, which steps the snap grid (the `N`
  key in the Classic profile). `cycle_snap_grid_forward` (`G` in the Altium
  profile) still opens the footprint editor's grid picker.
- Project ▸ Electrical Rules Check runs `open_erc_dialog`, which opens the ERC
  dialog to choose the rules. `run_erc` (F8) runs ERC at once. The row shows no
  key, as `open_erc_dialog` has no default binding.

## Tools ▸ Design Rule Check {#tools-drc}

Tools ▸ Design Rule Check runs `run_drc` (#2222) on the PCB in view and is
greyed while no PCB is open. The command is also in the command palette and
as the Run DRC button of the Messages panel; no keymap preset binds it.

## File ▸ Library {#file-library}

File ▸ Library carries only "Open Library..." (libraries outside the active
project) and the canvas-side "Place Component..." picker. Creating a new
component moved to the project tree: right-click ▸ Add New to Project ▸
Component Library, then Add New ▸ Component on the library node.

## File ▸ Export ▸ PDF {#file-export-pdf}

Print Preview used to be a top-level File row that duplicated Export ▸ PDF's
shortcut, so it was folded into Export ▸ PDF. File ▸ Print... (#1765) and
Export ▸ PDF open the same print preview / PDF export modal (`print`, Ctrl+P,
goes through the keymap command bridge in `app/command/bridge.rs`; Export ▸ PDF
runs as `print` in an undocked window, so both preview that window's document). Only
File ▸ Print... shows Ctrl+P, so one shortcut appears on one row.

Ctrl+F4 (Close Document, Altium profile) is a macOS system default ("Move
focus to active or next window") and may never reach Signex there; the menu
row always works.

## File: close, save all {#file-close-save-all}

Close Document (the `close_active_document` command, Ctrl+F4 in the Altium
profile, closes the active tab like the tab menu's Close and parks a dirty
draft) is greyed when no tab is open. Project ▸ Close Project (the project tree's Close
Project for the active project, behind its unsaved-changes gate) is greyed when
no project is open. Save All writes every unsaved document and Component
Preview row through the same Save All flow the close and quit prompts use
(`SaveAllNext::Nothing`: no close, no quit afterwards). It is greyed when
nothing is unsaved (`CommandContext::has_dirty_documents`), has no shortcut
(the catalog has no `save_all` command), and reports "Save All: saved N
item(s)" or the failures in the Messages panel.

## Place ▸ Sheet Entry, Tools ▸ Library Editor, Window ▸ Close All Documents {#tools-library-editor}

These three rows were planned stubs until #2247; each now runs a catalog
command, so the palette and a user key binding run the same id (none of the
three has a default key).

- **Place ▸ Sheet Entry** runs `place_sheet_entry`, the Active Bar's Sheet
  Entry tool. Greyed without a schematic, like the other Place rows. The
  Active Bar dropdown sends `ActiveBarAction::PlaceSheetEntry` directly and is
  not gated. The canvas Place flyout sends `ContextAction::Command` with
  `place_sheet_entry`, dispatched with `window = None`, so it is gated by the
  same `Needs::Schematic`. A flyout in an undocked sheet window runs inside
  that window's document swap, so the gate reads that sheet even while the
  main window shows a PCB (`place_design_rows` pins both). Passing the menu's
  own window instead of `None` is #2267.
- **Tools ▸ Library Editor** runs `open_library_browser` (maintainer decision,
  2026-11-09, `DECISIONS.md`). It focuses the active Library Browser tab, else
  the library of the active symbol or footprint editor tab, else the last
  Library Browser tab in the strip, else the first open library (the top row
  of the Libraries panel); a library with no browser tab gets a new one. From
  there the user picks a symbol or footprint to edit. Greyed when no library
  is open.
- **Window ▸ Close All Documents** runs `close_all_documents`, the project
  tree's Close Project Documents: every tab closes, and an unsaved document is
  parked as a draft exactly as the tab menu's Close All Documents does. Greyed
  when no tab is open.

## Tools ▸ Annotation ▸ Number Schematic Sheets {#number-schematic-sheets}

Runs `number_schematic_sheets` (#2247; editable dialog, maintainer decision
2026-11-09, `DECISIONS.md`). Greyed unless the active document (or, with no
document open, the active project's root sheet) belongs to a project; a loose
sheet has no project numbering. The dialog reads the active document's project off the UI
thread and lists every page in hierarchy order (the order and numbers of
`signex_net::sheet_numbering`, the ones PDF export prints), indented by depth,
with its file and its number.

- A page placed by a sheet symbol has a number field. A top-level page has no
  symbol to hold a number and keeps its position; it is shown read-only.
- Auto-number puts every page back at its position and clears the hand
  numbers. A number that is not a whole number from 1 turns its field red and
  greys Apply; a clash, a gap or a number above the page count is a warning
  only, as it is for an export.
- Apply writes each changed number into the `ChildSheet.instances` entry of
  the parent sheet's sheet symbol (`Command::SetChildSheetInstances`), scoped
  to the project name. A number equal to the page's position clears the entry
  instead. Every parent sheet takes the change in one undo group, so one
  Ctrl+Z on a changed sheet takes all of them back. A parent sheet that is not
  open is loaded into a tabless engine and left unsaved, as project Replace
  does; nothing is written to disk until the user saves.
- Apply is refused, with the reason in the dialog and the Messages panel, when
  a sheet was opened, closed or had a sheet symbol changed since the read, or
  when a parent sheet is being saved or moved, or is read-only.

## Window ▸ Tile Horizontally / Tile Vertically {#window-tile}

Run `tile_windows_horizontally` and `tile_windows_vertically` (#2247). They
arrange the main window and every undocked document window (main window
first, then the undocked ones by file path) on the main window's monitor: Tile
Horizontally side by side in equal columns, Tile Vertically stacked in equal
rows. Each window is un-maximized, resized and moved with `iced::window`
tasks. Greyed while no document is undocked, and until the main window's
monitor has been read (the settled geometry reading, shortly after start-up).
iced reports the monitor's full size, not its work area, so the tiles cover
the whole monitor. The Messages panel names how many windows were arranged.

## Rows still planned {#rows-still-planned}

- **Tools ▸ Assign Footprints** stays a stub: there is no bulk footprint
  assignment yet, and picking any library footprint for a placed library part
  changes how a part and its library row relate, which needs its own design
  (#2248).
- **Tools ▸ Net Inspector** stays a stub: there is no schematic net list
  panel; the PCB Nets panel lists a board's nets only (#1269).
- **Tools ▸ Annotation ▸ Back Annotate Schematics...** waits for PCB to
  schematic sync (#2271).

## Save and Save As {#file-save}

Both are also enabled for a standalone symbol or footprint editor tab
(`.snxsym` / `.snxfpt`), which `save_active_document` already handles, so the
gate is the command's enablement, not "a schematic is open".

## Native macOS menu {#native-macos-menu}

On macOS the main menu lives in the system menu bar (NSMenu), built with the
`muda` crate from the same `menu_bar_model` the in-window row renders, so a
model edit shows up natively with no change to the native code. Windows and
Linux keep the in-window row.

**Layers.** `menu_bar/native/mod.rs` turns the model into a `NativeMenuSpec`
(titles, labels, enabled flags, separators, submenus, stable ids) plus an
id -> `MenuMessage` map of the live, enabled rows. It compiles on every OS and
carries the tests. Only `native/macos.rs` calls `muda`. The state lives in
`Signex::native_menu`; `app/handlers/native_menu.rs` holds the two touch
points with `update`.

**Build timing.** winit installs its own default NSMenu in
`applicationDidFinishLaunching`. The fork's `Settings::default_menu` can turn
that off since iced-signex-v0.15.0-alpha.5, but Signex does not use it yet.
Installing from `Signex::new` would be overwritten, so the menu is
installed once the main window has opened (`WindowMsg::MainWindowOpened` arms
the state, the sync at the end of that `update` installs). `muda::Menu` panics
off the main thread and `Task` futures do not run there, so every muda call
happens inside `update`.

**App menu.** The first menu, titled by the process: About Signex,
Preferences... (Command+comma), Hide / Hide Others / Show All (stock AppKit
items), Quit Signex (Command+Q). Quit is a custom row routed to
`MenuMessage::Exit`, which reaches `handle_app_quit_requested`, the
unsaved-changes guard. The stock quit item sends `terminate:`, which exits
without it. Dock ▸ Quit and logout still send `terminate:`; those two paths
remain unguarded.

**Check marks.** The View menu's panel rows carry `MenuItem::checked`, read
from `MenuContext::open_panels` (`Signex::open_panels`, the same
`is_panel_open` answer the status-bar panel list uses). Natively they are
`CheckMenuItem`s; a moved mark refreshes in place. AppKit flips a check item
itself on a click, so choosing one drops the applied spec and the next sync
redraws every mark from the Model. A panel row opens or reveals its panel;
the panel's shortcut toggles it.

**Shortcuts on model rows are text, not key equivalents.** Each native row
shows its keymap shortcut (`Shortcut::text`, from the active keymap, so a
rebind shows up and the keymap change rebuilds the menu) as text after its
label. It registers no AppKit key equivalent: one fires before iced sees the
key, which bypasses the widget-capture rule in `app/dispatch/input.rs` (a
focused text input owns Ctrl/Command + C, V, X, A, Z), and the keymap treats
`Ctrl` as the physical Control key on every OS (#1441), so a native Command
key equivalent would disagree with the profile. Keys keep going through the
keymap, so a key is never dispatched twice. The four app-menu key equivalents
are the only registered ones, and none collides with editing.

**Exit.** The native File menu leaves out Exit and the separator above it:
Quit Signex in the app menu replaces it. Windows and Linux keep Exit.

**Ids.** `<title path>/<label>`, for example `File/Save` or
`File/Export/PDF`. A recent project uses its full path instead of its file
name (`File/Open Recent/<path>`), since file names repeat across folders. A
repeated id within one build gets a `#2` suffix. Ids come from the row alone,
never from an index, so an event that arrives after a rebuild cannot fire a
different row.

**Activation.** The muda event handler only forwards the id
(`WindowMsg::NativeMenuItemActivated`). `update` looks it up in the current
map, which holds live, enabled rows only, and dispatches `Message::Menu`. An
unknown, stale or disabled id is ignored and logged to the Messages panel.

**Focused window.** The bar acts on the focused window (`UiState::focused_window`,
fed by window focus events), falling back to the main window. Enablement comes
from that window's `CommandContext`. The per-window rows (Undo, Redo, Save,
Save As, Print, Export ▸ PDF, Close Document, the tab stepper) from a non-main
window route through `UiMsg::MenuInWindow`, as the in-window row does (#1546,
#1794, #1813), and Save, Save As and Print enable by that window's own document.

**Confirm dialogs lock the bar.** While a confirm or warning dialog is open
(`Signex::has_locking_confirm_modal`: quit and project-close confirms, error
notice, netlist-incomplete prompt, rename, remove, annotate-reset, close-library
and library-delete confirms, library and autosave recovery) the model rows are
greyed and refuse activation; About, Preferences and Quit stay live. Tool
windows (ERC, BOM, Preferences, Annotate, Print Preview, calculator, find /
replace, shortcuts) do not lock it.

**Sync.** After every `update` (macOS only) the state compares the inputs the
spec reads (`CommandContext` of the focused window, recent projects, the lock)
with the last sync; only a difference builds a new spec. A spec that differs only in enablement flips the
flags in place; any other difference rebuilds the NSMenu. The keymap is not an
input because the spec has no shortcut text.

**Window strip.** With the native menu installed the in-window row and the
wordmark are not drawn from the first frame on (so they do not flash before the
first install): the strip is the traffic lights, the drag zones and the centred
search. If installing or rebuilding fails, the system menu is taken down, the
action map is cleared, the in-window row comes back and the Messages panel says
why.
