# Detached modals: dialogs as OS windows

A dialog opens in its own OS window with a native title bar (title, close,
move, raise), so it can be dragged off the main window. The in-window card
stays as the fallback when no window exists. The same body function renders
both; in a window the header slot is empty (`os_title_bar_slot`) and the
native title bar carries the title (`view/detached_title.rs`).

## How a dialog gets its window

`ModalId` names the dialog. `handle_detach_modal` opens the window
(`detached_modal_settings`: decorations, `exit_on_close_request: false`,
centred on the screen; iced 0.14 cannot centre on the owner window and the
main window's position is not tracked). `WindowKind::DetachedModal` maps the
window to the dialog, `view(window)` renders `view_detached_modal`, and the
in-window builder skips painting while `modal_detached` holds.

Closing: the OS close button runs `detached_modal_close_message`, the same
message as the in-dialog close; `SecondaryWindowClosed` clears the state as a
backstop; Esc typed in the window resolves through the Esc ladder.

**Followed dialogs.** For Find / Replace, Rename, Remove, Enable Version
Control, Grid Properties, Custom Selection Filter, About, Keyboard Shortcuts,
the Passive Calculator, Create Sheet, the error notice, the netlist-incomplete
prompt, the autosave recovery prompt, and the project-close and app-quit confirms, `sync_detached_modals`
(`app/handlers/detached_modals.rs`) runs after every `update` and opens or
closes the window to match the dialog's open state. Whatever code flips the
flag (a menu row, F1, Esc, a handler) needs no wiring.

The first-run tour stays in the main window on purpose: it opens on the very
first update, when a new window can land behind the main window on Windows, and
it is a corner card, not a dialog.

F1 toggles the shortcuts sheet from the main window and from the sheet's own
window.

## Window behaviour and size

- **Fixed dialogs** (the confirm and warning class: app-quit, project-close,
  error notice, netlist prompt, autosave and library recovery, rename, remove,
  annotate reset, Create Sheet, close-library, library delete; and the small
  dialogs: About, Grid Properties, Custom Selection Filter, Enable Version
  Control, custom net colour, library picker, primitive picker, document
  options, library options) open with `resizable: false` and `minimizable:
  false`. iced 0.14 maps `resizable` to winit's maximise / zoom button (so the
  macOS green button is disabled) and `minimizable` to the minimise button;
  iced exposes no separate fullscreen switch, so fullscreen is off only as a
  consequence of `resizable: false`. `modal_is_fixed` in `erc/modals.rs` is the
  one list.
- **Tool windows** (Preferences, ERC, BOM, Print Preview, Annotate, Parameter
  Manager, Find / Replace, shortcuts, calculator, edit row, updates) keep normal
  window behaviour (resizable, zoom allowed) and open at a sensible default size.
- **Sizing rule.** A fixed dialog's window is sized to its content
  (`modal_size`): width fits the text, height is the content plus the footer.
  iced cannot measure a body to size its own window, so the sizes are constants,
  and a test (`fixed_dialogs_fit_their_content_without_large_slack`) lays each
  body out in its window and fails if content is cut off or more than 110 px of
  empty window is left. The error notice and the netlist prompt scroll a long
  message instead of growing the window.

## Dialog surface in a window

A dialog window's theme (`styles::dialog_window_theme`) takes the active
theme's dialog surface colour (`tokens.paper`) as the window background, so no
strip of another colour can show, and a theme switch updates it. Every dialog
body is wrapped in a root that fills the window on that surface. The dialog
bodies in `view/dialogs/*` (and the error notice and netlist prompt) use the full
window width there instead of their in-window card width (`dialog_width`). The
library dialogs keep their own card widths and sit centred on the same surface.
A test checks, for every dialog window in a dark theme and Solarized Light, that
the body covers the window and that the window background is the surface token.

## Lock policy

Confirm and warning dialogs lock the main window; tool windows do not.

- **Locking:** Rename, Remove, Create Sheet and the annotate-reset confirm
  (detached), plus the in-window confirms (quit, project close, error notice,
  netlist prompt, recovery prompts, library confirms). The one predicate
  is `Signex::has_locking_confirm_modal` (read by the macOS menu bar), and
  `locking_confirm_window` names a locking dialog that has its own window.
- **While a detached confirm is open:** the main window and undocked tabs take
  no key (`route_key`; Esc still resolves, so the lock can be dismissed), a
  layer over the content dims it slightly, eats the pointer and shows "A dialog is open: click to
  show it, Esc to cancel", and clicking it raises the dialog
  (`WindowMsg::LockedWindowClicked`). The layer sits BELOW the in-window
  overlays, so a dialog that opens meanwhile (quit confirm, an error notice) is
  still painted above it and can be answered. Where the window system ignores a
  request to focus a window (some Wayland compositors), Esc and the dialog's own
  window still work. A text input that had focus is released when the lock
  starts (`operation::focusable::unfocus`, once per lock).
- **Tool windows** (Find / Replace, Grid Properties, Custom Selection Filter,
  Enable Version Control, About, Shortcuts, Calculator, tour, ERC, Preferences,
  Annotate, Print Preview, BOM, ...) leave the main window fully usable.

## Per-dialog table

| Dialog | Class | OS close means | Tests |
|---|---|---|---|
| Rename, Remove | locking | Cancel | `detached_modals` |
| Annotate reset confirm | locking | Cancel | `detached_modal_os_close` |
| Create Sheet | locking | Later (writes nothing) | `child_sheet_create_tests` |
| Error notice | locking | dismiss (OK) | `detached_confirm_tests`, `detached_modals` |
| Netlist-incomplete prompt | locking | Cancel (writes nothing) | `detached_confirm_tests` (close mapping) |
| Autosave recovery | locking | Decide later (copy kept; the offer returns if its window vanishes) | `detached_modals`, `detached_confirm_tests` |
| Project-close confirm | locking | Cancel (never Discard) | `detached_modals`, `detached_confirm_tests` |
| App-quit confirm | locking | Cancel (never Discard or exit) | `detached_confirm_tests`, `tests/regression/project.rs` |
| Close-library confirm | locking | Cancel (drafts untouched) | `detached_modals` |
| Library delete confirm (per browser) | locking | Cancel for THAT browser; nothing deleted | `detached_library_tests` |
| Library recovery | locking | Cancel (never a decision that deletes or rebinds; returns if its window vanishes, twice at most, then stays in the window) | `detached_library_tests`, `detached_confirm_tests` |
| Library picker, primitive picker, document options, create options, updates, custom net colour | tool | Cancel, no write | `detached_modals` generic loops, `detached_library_tests` (net colour) |
| Library edit row (per browser) | tool | Cancel for THAT browser | same code path as delete confirm; no edit-row fixture |
| Find / Replace, Grid Properties, Custom Selection Filter, Enable Version Control, About, Shortcuts, Calculator | tool | close | `detached_modals` |

## The quit and data-loss path

The app-quit confirm keeps every guarantee of the in-window one. Each quit
route (the menu's Quit, the red traffic light, Alt+F4, the taskbar) goes
through `handle_app_quit_requested`, which opens the confirm or, when it is
already open, raises its window instead of stacking another; the main
window never exits behind it. Save All settles against the live dirty set
(`SaveAllNext::Quit`), Discard All closes the main window, Cancel leaves
everything open, and closing the confirm's own window is Cancel. The main
window's own close event (`SecondaryWindowClosed(main)`) is still the one
that ends the app.

## Per-browser library dialogs

Edit row and delete confirm belong to one library browser each. Every browser
that has one open gets its own window (`UiState::library_modal_owners` maps the
window to the browser); the window shows and answers for THAT browser, and
closing the window or the browser cancels it. The in-window card paints the
first browser that has one open and no window of its own.

## What the lock closes

A lock starting closes the command palette, context menus, the Active Bar menu,
the grid picker, the panel list and an in-place text edit, and releases text
focus. The lock layer sits above the non-modal chrome and below the dialogs.
Discard All on the quit confirm discards only what the confirm listed: an edit
made since (through any route) refreshes the confirm and keeps every recovery
copy; the project-close confirm already re-prompts the same way.

## Accepted: in-window fallbacks paint above the lock

If a tool window (Preferences, Annotate, ERC) could not open its own window it
paints in the main window as before, above the lock layer, which sits below the
dialogs. That is accepted: it is the in-window fallback of a tool dialog.

## Still in the window

Command palette, context menus, dropdown pickers, Active Bar menus, the grid
picker, the panel list, text edit, drag ghosts and the first-run tour stay in
the main window.
