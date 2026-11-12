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
mean the logo key (Command on macOS, Windows key elsewhere). So an explicit
`Ctrl+C` is Control on macOS too, not Command. Profile files always write the
logo key as `Cmd`; the parser also accepts `command`, `super` and `win`, so
existing files load unchanged.

`Mod` is the platform's standard modifier: Command on macOS, Control on
Windows and Linux (never Super). It is resolved once, when a profile is
compiled for the running OS, and every lookup, chord and conflict check
compares the resolved strokes. The spelling is kept on save, so a profile
reads `Mod+S` on every OS.

A section may carry `platform = "macos"` (also `"windows"`, `"linux"`); it is
not compiled, listed or edited on every other OS. Such a section is still
kept in the profile and written back with its tag, so saving on one OS never
deletes another OS's bindings, and a copy of a profile keeps it too. Every
section is parsed on every OS, so a typo in a `macos` section fails to load on
Windows and Linux as well. `Mod+Ctrl` is just Ctrl off macOS (both set Ctrl).

On macOS a stroke is shown with Apple symbols in Apple order and no `+`
(`⇧⌘S`, `⌃Q`, `⌥⌘F`); elsewhere it stays text (`Ctrl+Shift+S`). Menus, the F1
sheet, context menus and Preferences share that one display path. The
Preferences recorder stores a Command press as `Mod` on macOS so the profile is
portable; other OSes record exactly what was pressed. On macOS the conflict
check also flags a binding on ⌘Q, ⌘H, ⌘M, ⌘Space, ⌘Tab, ⌘` or ⌘, (unless it is
Preferences) as reserved by the system.

Layouts: a key press is matched by the logical key first, exactly as before
(AZERTY `z` stays `z`; a binding on `Ctrl+ö` still fires). Only when that
matches nothing, and Ctrl, Cmd or Alt is held, is the physical key's
US-QWERTY character tried: on a Turkish keyboard `⌘ö` fires `Mod+,`. A bare or
Shift-only key is never remapped. The macOS label (`⇧⌘S`) is display only and
is never parsed back; the Preferences recorder and editor carry the profile-file
spelling or the parsed stroke.

Existing user profiles are not rewritten silently: their `Ctrl+…` and `Cmd+…`
keep their physical meaning. The one exception is the one-time macOS prompt
below.

## Built-in profiles

Both built-in profiles spell the platform-standard set with `Mod`: `N O S P Z
X C V A F ,` plus `D R E L 4` and `Shift+S/Z/P/V/G/X/E/F/A`. Windows and Linux
resolve every row exactly as before (`Mod` is Ctrl there), so the resolved
golden `builtin_profiles_resolved.golden.tsv` does not change. Chords whose ⌘
twin is a macOS system shortcut stay on the physical Control key (⌃ on macOS):
`Ctrl+Q`, `Ctrl+W` (Altium Place Wire), `Ctrl+H`, `Ctrl+M`, `Ctrl+Space`,
`Ctrl+Tab`, `Ctrl+G` (Classic), `Ctrl+F5`, `Ctrl+Y` (redo), `Ctrl+F4`,
`Ctrl+PageDown` and `Ctrl+Home`. Redo keeps its plain `Ctrl+Shift+Z` (the
profile has exactly one raw owner of that chord, and Windows and Linux are
unchanged) and gains the Mac chord from a `platform = "macos"` section:
`Mod+Shift+Z` (⇧⌘Z), so ⌃⇧Z also still redoes on macOS. The Altium profile's
macOS section also has `Mod+Alt+F` (Replace, ⌥⌘F) and `Mod+W` (Close Document,
⌘W).

Bindings in one section load in alphabetical order of their trigger text, and a
command's menu hint is its last binding. Respelling `Ctrl+…` as `Mod+…` would
reorder that, so the global sections of both profiles are split to keep the
order the all-`Ctrl` spelling had (redo still shows `Ctrl+Y`, Place Wire `P W`,
the command palette `Ctrl+Shift+P`). `mod_content_tests.rs` pins those hints.

### One-time macOS prompt for a custom profile

On macOS, when the active profile is a custom one that still binds standard-set
chords with plain `Ctrl`, the first launch asks "Use ⌘ / Keep Control" in a
native dialog (`keymap_mod_prompt_boot`, built by `Signex::boot` only, never
by a headless boot). The outcome is logged to the Messages panel.

- **Use ⌘** copies `keyboard_shortcuts.toml` aside as `.bak` (never overwriting
  an earlier backup), respells only the standard-set `Ctrl+<k>` rows of the
  active profile as `Mod+<k>`, and saves atomically. Only once that succeeded
  is `keymap_mod_prompt_asked` set in `prefs.json`. If the backup or the write
  fails nothing is changed, the reason is logged and the question returns next
  launch. If Preferences was applied while the write ran, the moved copy is not
  adopted (a warning names the backup) and the question also returns.
- **Keep Control** leaves the profile as it is and sets the flag at once.

The rewrite is `keymap::migrate_active_to_mod`: it moves single-stroke
standard-set rows (and `unbind` rows, so they keep matching), and leaves
collision chords, multi-stroke chords, pointer gestures and platform-tagged
sections alone. A row whose `Mod` twin is already bound in the same context is
skipped and listed in Messages. Redo keeps its `Ctrl+Shift+Z` row and gains a
`platform = "macos"` `Mod+Shift+Z` row beside it, as the built-ins do. A second
run does nothing. An open Preferences keymap draft is moved to `Mod` too, so
Apply cannot write the Control rows back.

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
there is no active keymap at all; it is spelled `Mod+…` for the standard set
and shown through `keymap::shown`, as are the hints in prose messages and the
Symbol Editor status line (`⌘Z` on macOS). A hint is the last `Command` binding that no
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

