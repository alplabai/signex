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
the Passive Calculator and Create Sheet, `sync_detached_modals`
(`app/handlers/detached_modals.rs`) runs after every `update` and opens or
closes the window to match the dialog's open state. Whatever code flips the
flag (a menu row, F1, Esc, a handler) needs no wiring.

The first-run tour stays in the main window on purpose: it opens on the very
first update, when a new window can land behind the main window on Windows, and
it is a corner card, not a dialog.

F1 toggles the shortcuts sheet from the main window and from the sheet's own
window.

## Lock policy

Confirm and warning dialogs lock the main window; tool windows do not.

- **Locking:** Rename, Remove, Create Sheet and the annotate-reset confirm
  (detached), plus the in-window confirms (quit, project close, error notice,
  netlist prompt, recovery prompts, library confirms). The one predicate
  is `Signex::has_locking_confirm_modal` (read by the macOS menu bar), and
  `locking_confirm_window` names a locking dialog that has its own window.
- **While a detached confirm is open:** the main window and undocked tabs take
  no key (`route_key`; Esc still resolves, so the lock can be dismissed), a
  layer over the content eats the pointer and shows "A dialog is open: click to
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

## Still in the window

Command palette, context menus, dropdown pickers, Active Bar menus, the grid
picker, the panel list, text edit and drag ghosts stay in the main window.
Not yet detached: the error notice, netlist-incomplete prompt, custom net
colour, autosave recovery, project-close and app-quit confirms and the
library modals. The confirms need the same lock
treatment once detached; the library modals are tool windows.
