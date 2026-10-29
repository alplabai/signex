# Keyboard Shortcut Profiles

Signex maps keyboard input to stable command ids first, then maps those command
ids to application messages. The shortcut profile files do not serialize Rust
`Message` variants directly.

## Files

- Built-in profiles live in
  `crates/signex-app/assets/keyboard-shortcuts/altium.toml` and
  `crates/signex-app/assets/keyboard-shortcuts/classic.toml`.
- User profiles persist to the OS config directory at
  `<config_dir>/signex/keyboard_shortcuts.toml`.
- The Rust model lives under `crates/signex-app/src/keymap/`.

## TOML Format

Every shortcut TOML document starts with a machine-readable Signex header:

```toml
[signex_settings]
application = "signex"
file_kind = "keyboard_shortcuts"
version = 1
```

The shortcut payload is stored under `[keyboard_shortcuts]`. Built-in and custom
profiles use Zed-like sections: each section has a context and maps trigger
strings to command ids.

```toml
[keyboard_shortcuts]
schema_version = 1
profile_id = "my-profile"
profile_name = "My Profile"
profile_kind = "custom"
base_profile = "altium"

[[keyboard_shortcuts.sections]]
context = "global"

[keyboard_shortcuts.sections.bindings]
"Ctrl+S" = "save_document"
"Ctrl+Shift+P" = "open_command_palette"

[keyboard_shortcuts.sections.unbind]
"Ctrl+P" = "print"
```

Command labels and categories are not stored in the TOML. They are owned by
`crates/signex-app/src/keymap/catalog/` (one file per context).

## Runtime Flow

1. `Signex::new` loads built-in profiles and merges any user custom profiles.
2. The active profile is compiled into `CompiledKeymap`.
3. `app/bootstrap/subscription.rs` (`Signex::subscription`) receives `iced::keyboard::Event::KeyPressed`.
4. Modal and text-entry guards run first. For example, Escape closes open
   dialogs before normal shortcuts are considered.
5. Normal keys are converted to `KeyStroke` values and looked up against the
   active contexts, such as `global`, `schematic`, `pcb`, `library`, and
   `footprint`.
6. A resolved `AppCommandId` goes through `Signex::dispatch_command`, which maps
   it to a `Message` with `core_to_message` (`app/command/bridge.rs`).

If a keymap entry matches but maps to an unknown command or an explicit unbind,
the event is consumed as `Noop`. This prevents the old fallback code from making
custom unbinds ineffective.

## Modifiers

`Ctrl` is the physical Control key on every OS. `Cmd`, `Win` and `Super` all
mean the logo key (Command on macOS, Windows key elsewhere). So `Ctrl+C` in a
built-in profile is Control on macOS too, not Command. Profile files always
write the logo key as `Cmd`; the parser also accepts `command`, `super` and
`win`, so existing files load unchanged.

## Chords

A trigger can be a sequence of keys, such as `Ctrl+K Ctrl+S`. When a complete
trigger is also the start of a longer one, Signex waits `CHORD_TIMEOUT`
(800 ms) for the next key before running the shorter one. Esc, a pointer press
or a focus change cancels the wait. Only a live command binding makes a
shorter chord wait; an `unbind` entry never does.

## Backups

When `keyboard_shortcuts.toml` cannot be read, Signex uses the built-in
profiles for the session and leaves the file untouched. Before the next Apply
would write over it, the unreadable file is copied aside as
`keyboard_shortcuts.toml.bak`. A backup never overwrites an earlier one: the
names go `.bak`, `.bak.2`, `.bak.3` and so on. Restore and Discard in
Preferences act on the newest backup.

## Profiles And Editing

Built-in profiles are immutable. Editing starts by creating a custom profile
copied from the currently active profile. Custom profiles can then be selected,
edited, deleted, imported, and exported from Preferences > Keyboard Shortcuts.

The editor is backed by `KeymapEditorModel`, which exposes rows with:

- command label and category from command metadata
- context
- current trigger text
- conflict, invalid, unbound, gesture, and editable state

An `[unbind]` entry is not listed as a row of its own. It marks the matching
`bindings` row (same command, context and trigger, earlier in the profile) as
**Unbound**: the row stays visible, the shortcut does not work, and the
reference sheet leaves it out.

Pointer gestures such as `DoubleClick` and `Shift+Click` can appear in a
profile for reference, but the current editor only supports editing keyboard
sequences.

Two canvas modifiers are hard-wired rather than profile bindings: `Ctrl+Click`
toggles an item in the selection, and `Alt` held through a drag flips
"Keep wires connected when moving" for that drag only (#1266). The status bar
names the behaviour while the drag is live.
`Alt+Click` is reserved in `altium.toml` for `highlight_related_net_objects`. The
command is wired (it highlights the selected net and selects its members), but the
Alt+Click gesture itself is not dispatched yet; when it is, an Alt-press that turns
into a drag and an Alt+Click will need telling apart. Some Linux window managers take Alt+drag to
move the window; remap the WM modifier if the drag never reaches Signex. A selection box dragged left to right selects what it fully contains and
right to left what it touches (#1257); Preferences > Editing > "Select by drag
direction" turns that off, and the Inside / Touching mode then decides for both.

## Shortcut Labels

Visible shortcut labels should query the active keymap instead of hardcoding
display text. Existing examples:

- menu labels use `shortcut_for` in `menu_bar/mod.rs`
- context menus use `keymap_shortcut_label` (`app/view/context_menu/items.rs`)

A command the active profile does not bind, or has unbound, shows no
shortcut hint. The hardcoded fallback text in the menu bar applies only when
there is no active keymap at all. A hint is the last `Command` binding that no
later `Unbind` of the same command, context and strokes shadows. Fallback text
never drives runtime dispatch.

## Intentional Non-Configurable Behavior

Some keyboard behavior remains outside the normal profile system:

- modal and text-entry guards, especially Escape handling
- command-palette navigation while the palette is open
- selection-memory shortcuts using `Ctrl+1..8` and `Alt+1..8`

Selection memory stays outside the keymap for now because the current TOML
format stores only command ids, not command arguments such as the selected slot
number.

## Adding A New Shortcut Command

1. Add a stable command id to the matching file under `keymap/catalog/`.
2. Add default bindings to the relevant built-in TOML profiles.
3. Map the command id in `core_to_message`'s table (`app/command/bridge.rs`).
4. Use active-keymap lookup for visible labels instead of hardcoded strings.
5. Add or update keymap tests in `*_tests.rs`.

