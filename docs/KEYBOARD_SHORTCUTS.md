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
`crates/signex-app/src/keymap/catalog.rs`.

## Runtime Flow

1. `Signex::new` loads built-in profiles and merges any user custom profiles.
2. The active profile is compiled into `CompiledKeymap`.
3. `app/bootstrap.rs::subscription` receives `iced::keyboard::Event::KeyPressed`.
4. Modal and text-entry guards run first. For example, Escape closes open
   dialogs before normal shortcuts are considered.
5. Normal keys are converted to `KeyStroke` values and looked up against the
   active contexts, such as `global`, `schematic`, `pcb`, `library`, and
   `footprint`.
6. A resolved `AppCommandId` is dispatched by `message_for_keymap_command`.

If a keymap entry matches but maps to an unknown command or an explicit unbind,
the event is consumed as `Noop`. This prevents the old fallback code from making
custom unbinds ineffective.

## Profiles And Editing

Built-in profiles are immutable. Editing starts by creating a custom profile
copied from the currently active profile. Custom profiles can then be selected,
edited, deleted, imported, and exported from Preferences > Keyboard Shortcuts.

The editor is backed by `KeymapEditorModel`, which exposes rows with:

- command label and category from command metadata
- context
- current trigger text
- conflict, invalid, unbound, gesture, and editable state

Pointer gestures such as `DoubleClick` and `Shift+Click` can appear in a
profile for reference, but the current editor only supports editing keyboard
sequences.

## Shortcut Labels

Visible shortcut labels should query the active keymap instead of hardcoding
display text. Existing examples:

- menu labels use `shortcut_for` in `menu_bar.rs`
- panel labels use `shortcut_label` in `panels/mod.rs`
- context menus use the active `CompiledKeymap`

Fallback display strings are allowed only as display fallbacks for missing
profiles or unavailable commands. They should not drive runtime dispatch.

## Intentional Non-Configurable Behavior

Some keyboard behavior remains outside the normal profile system:

- modal and text-entry guards, especially Escape handling
- command-palette navigation while the palette is open
- selection-memory shortcuts using `Ctrl+1..8` and `Alt+1..8`

Selection memory stays outside the keymap for now because the current TOML
format stores only command ids, not command arguments such as the selected slot
number.

## Adding A New Shortcut Command

1. Add a stable command id to `keymap/catalog.rs`.
2. Add default bindings to the relevant built-in TOML profiles.
3. Map the command id in `message_for_keymap_command`.
4. Use active-keymap lookup for visible labels instead of hardcoded strings.
5. Add or update keymap tests in `*_tests.rs`.

