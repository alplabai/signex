# Keyboard Shortcut Profiles

Signex maps keyboard input to stable command ids first, then maps those command
ids to application messages. The shortcut profile files do not serialize Rust
`Message` variants directly.

## Files

- Built-in profiles live in
  `crates/signex-app/assets/keyboard-shortcuts/signex_standard.toml` and
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
base_profile = "signex_standard"

[[keyboard_shortcuts.sections]]
context = "global"

[keyboard_shortcuts.sections.bindings]
"Ctrl+S" = "save_document"
"Ctrl+Shift+P" = "open_command_palette"

[keyboard_shortcuts.sections.unbind]
"Ctrl+P" = "print"
```

Command labels and categories are not stored in the TOML. They are owned by
the command catalog in `crates/signex-commands/src/catalog/` (one file per
domain area, such as `documents.rs`, `wiring.rs` or `view.rs`), as message ids; the app's `keymap/catalog` only renders them
(`CatalogTextExt`).

## Runtime Flow

1. `Signex::new` loads built-in profiles and merges any user custom profiles.
2. The active profile is compiled into `CompiledKeymap`.
3. `app/bootstrap/subscription.rs` (`Signex::subscription`) receives `iced::keyboard::Event::KeyPressed`.
4. Modal and text-entry guards run first. For example, Escape closes open
   dialogs before normal shortcuts are considered.
5. Normal keys are converted to `KeyStroke` values and looked up against the
   active contexts, such as `global`, `schematic`, `pcb`, `board_3d`,
   `library`, and `footprint`.
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

Save All is `Mod+Alt+S` in both built-in profiles: Ctrl+Alt+S on Windows and
Linux, Cmd+Alt+S on macOS. Save As keeps `Mod+Shift+S`. Windows reports AltGr
as Ctrl+Alt, so on a layout where AltGr+S types a character the key types it,
and Save All is reached from the File menu.

Layouts: a key press is matched by the logical key first, exactly as before
(AZERTY `z` stays `z`; a binding on `Ctrl+ö` still fires). Only when that
matches nothing, and Ctrl, Cmd or Alt is held, is the physical key's
US-QWERTY character tried: on a Turkish keyboard `⌘ö` fires `Mod+,`. A bare or
Shift-only key is remapped only when its character is not Latin script: on a
Russian or Greek keyboard the W key still fires `W` (Place Wire) when nothing
binds `ц` or `ς`, while a Latin-script key (`é`, `ö`, Dvorak or AZERTY letters)
is never remapped (#2095). The symbol and footprint canvases read their own
letter keys (Ctrl+A, Ctrl+Z, Ctrl+Y, Ctrl+X/C/V, X) the same way.

Known limit: a key whose layout character is ASCII is never remapped, with or
without Ctrl. The Greek layout puts `;` on the Q key, so on a Greek keyboard
`Q` and `Ctrl+Q` reach a `;` or `Ctrl+;` binding, not a `Q` or `Ctrl+Q` one.

The macOS label (`⇧⌘S`) is display only and is never parsed back; the
Preferences recorder and editor carry the profile-file spelling or the parsed
stroke.

Existing user profiles are not rewritten silently: their `Ctrl+…` and `Cmd+…`
keep their physical meaning. The one exception is the one-time macOS prompt
below.

## Built-in profiles

Both built-in profiles spell the platform-standard set with `Mod`: `N O S P Z
X C V A F ,` plus `D R E L 4` and `Shift+S/Z/P/V/G/X/E/F/A`. Plain `G` is in
the set only in the default profile, where `Mod+G` is Create Union (⌘G on
macOS); Classic keeps `Ctrl+G` physical. Windows and Linux
resolve every row exactly as before (`Mod` is Ctrl there), so the resolved
golden `builtin_profiles_resolved.golden.tsv` does not change. Chords whose ⌘
twin is a macOS system shortcut stay on the physical Control key (⌃ on macOS):
`Ctrl+Q`, `Ctrl+W` (Signex Standard Place Wire), `Ctrl+H`, `Ctrl+M`, `Ctrl+Space`,
`Ctrl+Tab`, `Ctrl+G` (Classic), `Ctrl+F5`, `Ctrl+Y` (redo), `Ctrl+F4`,
`Ctrl+PageDown` and `Ctrl+Home`. Redo keeps its plain `Ctrl+Shift+Z` (the
profile has exactly one raw owner of that chord, and Windows and Linux are
unchanged) and gains the Mac chord from a `platform = "macos"` section:
`Mod+Shift+Z` (⇧⌘Z), so ⌃⇧Z also still redoes on macOS. The Signex Standard profile's
macOS section also has `Mod+Alt+F` (Replace, ⌥⌘F), `Mod+W` (Close Document,
⌘W) and `Alt+I` (⌥I, add vertex while drawing, else Copy Attributes): a Mac
keyboard has no Insert key, so ⌥I is that command's menu hint on macOS. It
also has `Shift+Tab` (⇧Tab) for `cycle_wiring_mode`, next to ⌃Space, which
macOS takes for the input source once two exist; ⇧Tab is that command's menu
hint on macOS. A held Shift does not change the drag (Shift+click adds to the
selection and Shift+wheel pans, neither mid-drag), unlike ⌥, which flips
whether the wires follow and so was not used.

Bindings in one section load in alphabetical order of their trigger text, and a
command's menu hint is its last binding. Respelling `Ctrl+…` as `Mod+…` would
reorder that, so the global sections of both profiles are split to keep the
order the all-`Ctrl` spelling had (redo still shows `Ctrl+Y`, Place Wire `P W`,
the command palette `Ctrl+Shift+P`). `mod_content_tests.rs` pins those hints.

On a board (the `pcb` context, #2406) both profiles bind `1` Board Sketch,
`L` Flip Selection (the selection, or the part on the cursor), `F` Change
Footprint and `Q` the unit toggle; the chord profile adds `N` Toggle
Ratsnest, `P K` Keepout and `T D` Run DRC, Classic `C` Toggle Ratsnest, `K`
Keepout, `F7` Run DRC and `Ctrl+M` Measure Distance (the chord profile's
global `Ctrl+M` already measures on a board). Edit ▸ Origin ▸ Set is `E O S`
in the chord profile and `Shift+O` in Classic, whose `E` alone is taken
(2026-11-18). The board Place tools (#2509) are `P T` Track, `P V` Via,
`P G` Polygon Pour, `P F` Fill and `P R` Solid Region in the chord profile;
Classic, whose `P` alone is taken, gives them the same letter with `Shift`
(`Shift+T`, `Shift+V`, `Shift+G`) and with `Shift+Alt` where `Shift` alone
is a global key (`Shift+Alt+F`, `Shift+Alt+R`), so none of them equals or
prefixes another Classic binding on a board. A board binding outranks a global one with the same keys, so
Classic's `L` and `Q` keep their sheet meaning on a sheet. `Q` sits in a
`pcb` section of its own before the global sections, so `cycle_unit` keeps
`Ctrl+Q` as its menu hint. A test checks that every `pcb` binding of both
profiles names a command enabled on a board; `H` and `Mod+Shift+C` (the 3D
view's lighting and screenshot copy) are the exception, greyed on a flat
board because they need the 3D view.

The `board_3d` context (mockup row 175) is live, after `pcb`, only while the
main window shows the board in 3D, so its bindings outrank the board's: both
profiles bind `F` there to Flip (`flip_board_3d`), and `F` stays Change
Footprint on a flat board. Its rows show under the PCB chip in Preferences
and the PCB scope of the shortcuts sheet.

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
  adopted: a warning names the backup, your live profiles are saved back over
  the file the write replaced (a failure of that is logged as an error), and
  the question returns next launch.
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

### The Keyboard shortcuts page (mockup row 145)

The page (`preferences/keymap/`) reads top to bottom:

- **Profile row**, on one line: the Profile picker, whose entries read
  `<name> · <n> bindings` for a built-in and `<name> (custom) · <n> bindings`
  for a custom profile; **Create** (a custom copy of the active profile);
  **Delete** (inert on a built-in profile); then **Import…** and **Export…**,
  which open the file dialogs.
- **Conflict banner**, a warning shown only when the active profile has a
  conflict. It counts the conflicts and names the first one: the keys as
  keycaps and the two commands bound to them (or, for a chord macOS keeps,
  the one command). **Show** turns on the Conflicts chip.
- The shortcuts-file banner (#595), the backup row with Restore and Delete
  (#603), and the status line of the last action stay above the table.
- **Filter row**: the search field (label, command id or keys) and the
  context chips **All**, **Global**, **Schematic**, **PCB**, **Footprint**,
  **Library** and **Conflicts** (with the conflict count). One chip is on at
  a time. A row shows under the chip of its context (a board 3D view row
  under PCB); rows of dialogs, text
  fields, the palette and placement show under All only. Conflicts lists
  the rows that take part in a conflict of the active profile. The chip is
  page view state (`KeymapViewState`): it never marks Preferences dirty and
  starts on All each time Preferences opens.
- **Table**: Command, Category, Context, Shortcut and State, sorted by
  command name. Keys show as keycaps, with "then" between the strokes of a
  sequence; a pointer gesture shows as text; a command with no key shows a
  dash. State tones: Conflict and Invalid as error pills, Unbound in muted
  italics, Gesture and Reserved by macOS as neutral pills, Editable and
  Create custom as a quiet caption. A row whose keys a fixed app key takes
  first (F1, Esc) names that key in its tooltip ("Overridden by …").
- **Edit** shows while the pointer is over a keyboard row of a custom
  profile. It opens the **recorder popover**, anchored under that row over
  the table: the command, a recording box with the captured keycaps, the
  current keys, a help line, a warning when a fixed app key would swallow
  the recorded keys, and Clear, Stop (Record once stopped), Cancel and OK.
  OK puts the new keys in the working copy; like every Preferences change
  they are saved by the dialog's Apply or OK. Every keystroke while
  recording is recorded, Esc included; only Cancel closes the recorder.
  OK with nothing recorded (after Clear) takes the row's key away.
- **Commands with no key** (#2495, Caner 2026-11-25): the table also lists
  every command the active profile gives no key, built-in or from an
  installed extension, as an Unbound row in the command's own context (the
  one editor it acts in, else Global). A built-in command is listed when a
  key can run it, the rule the command palette uses. In a custom profile
  its Edit opens the same recorder, and OK adds a binding at the end of the
  profile (`KeymapEditorModel::assign_active_trigger`), saved with the
  profile in `keyboard_shortcuts.toml`; an extension command is saved under
  its `ext.` id. On a built-in profile these rows have no Edit: Create
  copies the profile first.

### Key bindings from the CLI (#2495)

Two catalog commands edit a custom profile without the window, for
`signex-cli` (which needs `--save` or `--allow save` for them) or a script:

- `keymap_assign command=<id> keys=<keys> profile=<id> [context=<context>]`
  gives the command the key sequence `keys` (profile spelling, `Ctrl+Alt+Q`,
  `P W`). `context` is a section name of the file (`global`, `schematic`,
  `pcb`, `board_3d`, …); without it, the one editor the command acts in, else
  `global`. The command is a catalog id or an installed extension's.
- `keymap_clear command=<id> profile=<id> [keys=<keys>]` takes that key, or
  every key, from the command; an extension id that is not installed is
  accepted, so a kept binding can be cleared.

Both edit the named profile with `KeymapEditorModel` (the active profile is
not switched), write `keyboard_shortcuts.toml` through `atomic_write`, make
the change the running keymap, and answer with JSON: `profile`, `command`,
`keys`, `unchanged`, `file`, and `context` and `conflicts` (assign) or
`removed` (clear). `conflicts` lists each clash the new key takes part in,
as the page's State column marks it: its `keys`, `context`, the two
`commands` and `reserved_by_os`. A key the command already has in that
context is not added again and is answered `"unchanged": true`, and a clear
that finds no key writes nothing. They are refused, with nothing written:

- for a built-in profile, which stays read-only (Create copies it first);
- while the file failed to load (#595);
- while Preferences ▸ Keyboard shortcuts holds changes of its own or a
  typed trigger, since Apply would then write the page's copy over the
  command's edit ("save or discard the keyboard changes in Preferences
  first").

On a page with no changes of its own the command's edit shows at once, and
an open recorder closes, since the rows it pointed at may have moved.

In a headless `signex-cli` run, which otherwise works under a throwaway
config root, these two commands read and write the user's own
`keyboard_shortcuts.toml` (the one carve-out, `docs/CLI.md`), read fresh for
each command. Import… and Export… of a profile file stay Preferences actions
(`read_profile_file`, `write_profile_file`).

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

While a wire or bus is drawn, `switch_segment_posture` flips the corner
(horizontal first or vertical first; in a 45° mode, diagonal first or last) and
`cycle_wire_bus_graphic_mode` steps the corner mode: 90°, 45° start, 45° end,
any angle (#2372). The choice holds for later segments, and the status bar names
it. The Classic profile binds them to `/` and `Shift+Space`. In the Signex Standard
profile `Space` and `Shift+Space` stay bound to `rotate_counterclockwise` and
`rotate_clockwise`; while a wire or bus run is open (its first point placed),
the rotate handler bends the run instead: `rotate_counterclockwise` flips the
corner and `rotate_clockwise` steps the corner mode. Outside a run they rotate
as usual. Neither adds an undo step; only a committed segment does.

`copy_attributes_or_add_vertex` (Signex Standard profile: `Insert`) fixes a vertex at the
cursor while a wire, bus, line or polyline is drawn, as a click there would, and
the run goes on; mid rectangle, circle or arc it says so in Messages and does
nothing. With any other tool it copies attributes, also while Measure, a sheet
symbol or a directive region has its first point placed (#1669).

`cycle_wiring_mode` (Signex Standard profile: `Ctrl+Space`) steps how the wires that
follow a moved part bend in a drag and in click-click move: Auto (the corner
the rubber band picks), horizontal first, vertical first, straight (no
corner, any slope), 45° first (a 45° leg from the wire's fixed end, then a
straight leg to the pin) and 45° last (the 45° leg reaches the pin), then Auto
again. Each mode bends as the wire tool's corner mode of the same shape does;
when its route is blocked it takes the other L, or the 45° leg at the other
end (#2516). Press it during the move and the ghost changes at once; the
drop commits what the ghost shows. The mode holds for later moves, and the
status bar names it while it is not Auto. Move By, align and distribute,
position edits, rotate and mirror always use Auto (#1669).

In every mode, a pin that sits on a junction of two or more wires leaves those
wires where they are: a stub grows or shrinks from the junction to the moved
pin, as for a pin on a wire that passes through. Only when no stub fits do the
wires follow the pin (#2540).

A key bound in the keymap runs some commands once, however long it is held:
`cycle_wiring_mode`, the corner mode and posture keys, the grid steps
(`next_grid`, `previous_grid`), the unit and selection-mode cycles, every
show/hide toggle, flip and mirror (`X`, `Y`, `L`), and `Insert` (one vertex per
press mid wire). `Space` and `Shift+Space` bend an open wire, bus or track run
once per press, in the window the run is open in, but turn a selection on
every repeat. The rule is the same in every profile, Classic included (a held
`X`, `Y` or `L` mirrors or flips once, a held `Shift+Space` steps the corner
mode once). Other commands, such as zoom, undo, nudges and layer stepping,
repeat while the key is held (`docs/DECISIONS.md`, "Held keys", 2026-11-19).
The footprint editor canvas handles its own `X` (flip the pad's layer) outside
the keymap; that one still repeats (#2545).

`Alt+Click` is reserved in `signex_standard.toml` for `highlight_related_net_objects`. The
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

1. Add a stable command id to the area file it belongs to under
   `crates/signex-commands/src/catalog/` (for example `placement.rs` or
   `nets.rs`). The file's place in `TABLES` (`catalog/mod.rs`) and the row's
   place in the file set where the command lists. Add the id to
   `scripts/catalog-ids.txt` too: the command-id gate fails on a catalog id
   that is not listed, and on a listed id that is gone. A command is removed
   only by removing its line there, with a maintainer decision.
2. Add default bindings to the relevant built-in TOML profiles.
3. Map the command id in `core_to_message`'s table (`app/command/bridge.rs`).
4. Use active-keymap lookup for visible labels instead of hardcoded strings.
5. Add or update keymap tests in `*_tests.rs`.

