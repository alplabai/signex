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
Force Annotate All Schematics (Shift+Alt+A), and the planned Back Annotate
Schematics... and Number Schematic Sheets....

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

## Save and Save As {#file-save}

Both are also enabled for a standalone symbol or footprint editor tab
(`.snxsym` / `.snxfpt`), which `save_active_document` already handles, so the
gate is the command's enablement, not "a schematic is open".

## Native macOS menu {#native-macos-menu}

On macOS the main menu lives in the system menu bar (NSMenu), described as
data by `iced::menu::Menu` from the same `menu_bar_model` the in-window row
renders, so a model edit shows up natively with no change to the native code.
Windows and Linux keep the in-window row; the iced runtime never asks them
for the native menu.

**Layers.** `menu_bar/native/mod.rs` turns the model into an
`iced::menu::Menu<Message>`: titles, labels, check marks, separators,
submenus, and for each live row the message it sends. It compiles on every OS
and carries the tests. `Signex::native_menu` (`app/handlers/native_menu.rs`)
reads the Model and builds it; `main.rs` hands it to `iced::daemon(..).menu`,
and the iced fork (alplabai/iced, `iced::menu`) syncs it to the NSMenu after
every `update`: flags (enabled, checked) in place, rows rebuilt only when
they changed, a stable id per row, a failed rebuild keeping the old menu.
Signex has no AppKit or `muda` code of its own.

**Build timing.** `main.rs` sets `Settings::default_menu: false`, so winit
installs no default menu and Signex needs no "install after the main window
opened" step: the iced runtime installs the menu once the application has
launched, and the in-window row is already left out from the first frame.

**App menu.** The first menu, titled by the process: About Signex (a custom
row, `MenuMessage::OpenAbout`), Preferences... (a custom row with the one real
key equivalent, Command+comma), Hide / Hide Others / Show All and Quit (the
stock items, `iced::menu::Predefined`).

**Quit.** `Settings::exit_on_quit_request` is `false`, so every quit the
system asks for reaches the app as `iced::system::quit_requests()`: Command+Q
and the app menu's Quit, Dock ▸ Quit, and logout (#2224; the old custom Quit
row guarded only the menu item and Command+Q, and the other two sent
`terminate:` past the guard). The subscription (`QuitRequested`) is always
running, since the runtime answers a request nobody listens to with "quit" at
once. It runs `handle_app_quit_requested`, the same unsaved-changes guard File
▸ Exit and the window close use: nothing unsaved closes the main window, which
ends the process (the runtime answers the pending request when the app exits);
otherwise the prompt opens. A request stays pending until it is answered, and
macOS waits (at logout only for a limited time), so every path that keeps the
app open answers it with `iced::system::cancel_quit()` (a no-op when nothing
is pending): the prompt's Cancel, a quit held back by a running rename or
delete, a Save All that left files unsaved, and the prompt closing itself
because everything was saved behind it, and a prompt window that vanishes
without a close request (treated as Cancel). Each decision is logged to the
Messages panel ("Quit: the system asked Signex to quit", "Quit: not quitting,
Signex stays open"). The second line is written on every OS whenever a quit
is declined, a system request or not.

**Check marks.** The View menu's panel rows carry `MenuItem::checked`, read
from `MenuContext::open_panels` (`Signex::open_panels`, the same
`is_panel_open` answer the status-bar panel list uses), and become check rows
(`Item::checked`). AppKit flips a check item itself on a click; the fork
writes the flags of a chosen row again on the next sync, so the mark is
redrawn from the Model. A panel row opens or reveals its panel; the panel's
shortcut toggles it.

**Shortcuts on model rows are text, not key equivalents.** Each native row
shows its keymap shortcut (`Shortcut::text`, from the active keymap, so a
rebind shows up, and the changed label makes the fork rebuild the row) as
text after its label, four spaces apart. It registers no AppKit key
equivalent (no `Item::shortcut`): one fires before iced sees the
key, which bypasses the widget-capture rule in `app/dispatch/input.rs` (a
focused text input owns Ctrl/Command + C, V, X, A, Z), and the keymap treats
`Ctrl` as the physical Control key on every OS (#1441), so a native Command
key equivalent would disagree with the profile. Keys keep going through the
keymap, so a key is never dispatched twice. The four app-menu key equivalents
(Preferences' Command+comma and the stock Quit, Hide and Hide Others) are the
only registered ones, and none collides with editing.

**Exit.** The native File menu leaves out Exit and the separator above it:
Quit in the app menu replaces it. Windows and Linux keep Exit.

**Ids.** The fork derives a row's id from its label path, `File/Save` or
`File/Export/PDF` (with the shortcut text, which is part of the label), never
from an index, so an event that arrives after a rebuild cannot fire a
different row; a repeated label within one menu gets a `#2` suffix, which
matters for Open Recent, where two projects can share a file name. Each
Open Recent row carries its own project path in its message.

**Activation.** A model row's message is
`WindowMsg::NativeMenuRowChosen(MenuMessage)`; `update` refuses it while a
confirm dialog is open (a backstop for a click on a row the bar had not yet
greyed), routes it to the focused window as below and dispatches. A greyed row
has no message, so a disabled row cannot fire. The app menu's About and
Preferences send `Message::Menu` directly, so the lock never blocks them.

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

**Sync.** The runtime calls `Signex::native_menu` after every `update`
(macOS only), so the menu reads the Model like a view: `CommandContext` of the
focused window, recent projects, the lock, open panels, the keymap and the
language. The built menu is cached in `NativeMenuState` with the
`MenuInputs` it came from and rebuilt only when one of them changed (the
runtime asks after every update batch, canvas mouse moves included); the fork
then decides what, if anything, to touch in AppKit.

**Window strip.** With the native menu in use the in-window row and the
wordmark are not drawn from the first frame on (so they do not flash before the
first install): the strip is the traffic lights, the drag zones and the centred
search.

**Fallback.** The app subscribes to `iced::system::menu_failures()`, which
fires once for each distinct menu the runtime could not build, with the error
text. A failed first install leaves no menu bar at all (`default_menu` is
off), so on a report `WindowMsg::NativeMenuFailed` sets a sticky flag
(`NativeMenuState::fall_back`), logs the reason as a warning in the Messages
panel, and `hides_in_window_menu()` turns false, so the in-window row and the
wordmark come back. The flag is not cleared if a later menu builds, so the
bar does not switch back mid-session. A failed rebuild of a later menu leaves
the previous system menu in place, and the in-window row shows as well.
