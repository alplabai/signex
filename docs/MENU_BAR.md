# Main menu bar

The bar is plain data: `menu_bar_model(&MenuContext)` in
`crates/signex-app/src/menu_bar/model/` builds it, and `menu_bar/view.rs`
renders it as iced_aw menus. Its content is pinned by
`menu_bar/testdata/menu_bar.golden.tsv`; changing a menu, label, shortcut,
order or message is a content change and needs the maintainers' sign-off.

## File ▸ Library {#file-library}

File ▸ Library carries only "Open Library..." (libraries outside the active
project) and the canvas-side "Place Component..." picker. Creating a new
component moved to the project tree: right-click ▸ Add New to Project ▸
Component Library, then Add New ▸ Component on the library node.

## File ▸ Export ▸ PDF {#file-export-pdf}

Print Preview used to be a top-level File row that duplicated Export ▸ PDF's
shortcut. The two were consolidated under Export ▸ PDF so one surface opens
the preview flow. Ctrl+P still opens the preview through the keymap command
bridge (`app/command/bridge.rs`); only the menu row moved.

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
`applicationDidFinishLaunching`, and iced_winit offers no way to turn that
off. Installing from `Signex::new` would be overwritten, so the menu is
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

**No accelerators on model rows.** An AppKit key equivalent fires before iced
sees the key, which bypasses the widget-capture rule in `app/dispatch/input.rs`
(a focused text input owns Ctrl/Command + C, V, X, A, Z), and the Ctrl-to-
Command mapping (#1441) is not in yet. Native rows show no shortcut and
register no key equivalent; keys keep going through the keymap.
`Shortcut::command_id` stays in the model for a later change. The four
app-menu key equivalents are the only ones, and none collides with editing.

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

**Sync.** After every `update` (macOS only) the state compares the inputs the
spec reads (`CommandContext`, recent projects) with the last sync; only a
difference builds a new spec. A spec that differs only in enablement flips the
flags in place; any other difference rebuilds the NSMenu. The keymap is not an
input because the spec has no shortcut text.

**Window strip.** With the native menu installed the in-window row and the
wordmark are not drawn: the strip is the traffic lights, the drag zones and the
centred search. If installing fails, the in-window row stays and the Messages
panel says why.
