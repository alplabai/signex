# Command Registry — user-addressable action audit

**Date:** 2026-07-25 · **Measured on:** `trunk` @ `f211e7d1` · **Issue:** #274 (epic #278)

Scoping audit for Phase 2 of the Command Registry epic. Enumerates the real
user-invokable action surface, deduplicates it into canonical commands,
classifies it against the existing catalog, and sizes what Phase 2 has to cover.

This is a **decision record pinned to a commit**, not a living registry. It will
go stale, and that is fine — the artifact that must not go stale is the coverage
ratchet test proposed in §10, which turns the measurement below into CI. Read the
numbers here as "true at `f211e7d1`".

No runtime code — this document (plus its row table, linked in §11) is the
deliverable #274 asks for.

---

## 0. Correction to the issue's premise

#274 (and RFC open-question #6) size the problem at **"~281 variants"** across
`Message` / `ActiveBarAction` / `ContextMenuAction` / `LibraryMessage` /
`PanelMsg`. Three things are wrong with that framing:

- **The message surface is far larger.** 1239 variants across 61 message-like
  enums in `crates/signex-app/src` — 4.4× the stated figure.
- **`ContextMenuAction` does not exist.** Context-menu actions are per-surface:
  `ContextAction` (`app/contracts/state.rs:12`), `FootprintContextAction`
  (`library/editor/footprint/state/context_menu.rs:38`), and — for the symbol
  editor — a boxed `SymbolEditorMsg::ContextMenuAction`
  (`library/messages/symbol.rs:251`), which is a routing wrapper, not an action
  enum. The enum list also omits the single biggest block of user-invokable
  actions: `ActiveBarAction` (`active_bar/mod.rs:305`, 84 variants).
- **Variants are the wrong unit.** Only 301 of the 1239 are ever constructed at a
  user-invocation surface, and those 301 contain heavy duplication — `Copy` is
  three variants (`MenuMessage::Copy`, `EditMsg::Copy`, `ContextAction::Copy`)
  and one command. Phase 2 mints *command ids*, so the audit counts commands.

Sizing Phase 2 off the raw variant count overstates it ~6×; sizing it off "281"
understates the addressable set while overstating the work.

## 1. Method

Reproducible against the tree at `f211e7d1`.

1. **Inventory.** Parse every `enum` in `crates/signex-app/src` whose name ends in
   `Msg` / `Message` / `Action` / `Request` / `Event`; count top-level variants
   with attributes, doc comments and nested generics stripped.
   → **61 enums, 1239 variants**.
2. **Invocation surfaces.** Collect every `Enum::Variant` path constructed in the
   files that render a user-facing action surface:

   | surface | files |
   | --- | --- |
   | menu bar | `menu_bar/{view,mod}.rs` |
   | app context menus | `app/view/context_menu/{items,menus,project_tree,submenu}.rs` |
   | active bar (schematic) | `active_bar/{mod,dropdown,legacy}.rs` |
   | command palette | `app/command_palette.rs` |
   | keymap bridge | `app/command/bridge.rs` |
   | symbol-editor menus | `library/editor/symbol/context_menu/{mod,rows}.rs`, `library/editor/symbol/active_bar/{mod,dropdowns}.rs` |
   | footprint-editor menus | `library/editor/footprint/{context_menu,unified_active_bar,pads_active_bar,active_bar_dropdowns}.rs`, `library/editor/footprint/sketch_mode/active_bar.rs` |

   → **301 distinct variants constructed**.
3. **Classify** each of the 301 as an *action* or as *plumbing* (§3) → **244
   action variants**.
4. **Deduplicate** action variants into canonical commands (§4) → **199 canonical
   commands**.
5. **Cross-reference** against the 134-entry catalog
   (`keymap/catalog/{general,schematic,pcb,threed}.rs`), the bridge's arms
   (`app/command/bridge.rs`), and the two shipped keymap profiles
   (`assets/keyboard-shortcuts/{altium,classic}.toml`).

**Deliberately out of scope:** buttons inside modal dialogs (OK / Cancel / field
edits), panel-internal widgets, and async/lifecycle variants (`HistoryLoaded`,
`…Loaded`, `…Failed`). These are reachable by a user but are not *named,
discoverable actions* — no menu row, no shortcut, no meaning outside the dialog
that owns them. A command id for `PrintPreviewMsg::ZoomChanged` would be public
API for a slider.

**Known bias from that exclusion:** it silently pre-answers the `gui_only?`
column for dialog-only actions. An export that only exists behind an options
dialog, or annotate-with-settings, is genuinely GUI-only today and does not
appear in this census at all. When Phase 5 (CLI/headless) arrives, those are
exactly the actions that will need a headless entry point, and they will have to
be found by a second pass over the dialog surfaces — not by re-reading this
document.

## 2. Headline numbers

| | count |
| --- | ---: |
| message-like enums | 61 |
| total variants | 1239 |
| variants constructed at an invocation surface | 301 |
| — plumbing / routing (§3) | 57 |
| — action variants | 244 |
| **canonical commands after dedup + parameterisation (§4)** | **199** |
| catalog ids today | 134 |
| catalog ids with a `core_to_message` arm | 59 |
| catalog ids with **no** arm | **75** |
| orphan arms (arm with no catalog entry) | 0 |

**Registry coverage of the real action surface: 40 of 199 = 20%.**

59 catalog ids carry a bridge arm, but the bridge aliases several ids onto one
action (`find` and `find_text` both → `OverlayMsg::OpenFind`; `zoom_to_fit`,
`center_view_at_cursor` and `show_all_design_objects` all →
`CanvasEvent::FitAll`), so the 59 collapse to **40 distinct canonical commands**
actually reachable through an id. The remaining **156 are new** — user-invokable
today, with no command id at all. 3 more share a name with a dead catalog id.

## 3. The admission rule

A message variant earns a stable command id when **all** of the following hold:

1. **Named and discoverable.** It appears as a menu row, a context-menu row, an
   active-bar / toolbar button, a palette row, or is bindable to a key. If a user
   cannot name it, it is not a command.
2. **Meaningful outside its widget.** It does something a CLI or plugin could
   sensibly ask for. `EditMsg::Copy` qualifies; `ContextMenuMsg::SubmenuOpen`
   does not.
3. **Not a live-input echo.** Text-input `…Changed`, drag deltas, hover, scroll
   and tick timers are state transport, not commands.
4. **Not a routing wrapper.** `Message::Edit(..)`, `Message::Library(..)`,
   `ContextAction::ActiveBar(..)`, `ActiveBarMsg::Action(..)`,
   `ContextMenuMsg::{Action, ProjectTreeAction, TabAction}` exist to dispatch,
   not to act. Counting a wrapper *and* its payload double-counts.

Applying 2–4 removes 57 of the 301 as plumbing. Rule 4 also excludes
`CommandAction::{Menu, Panel, OpenFile, FocusSymbol}` — the command palette's own
dispatch enum, which routes rows to their real targets and is not itself an
action surface.

## 4. Dedup and parameterisation

Two collapses take 244 action variants to 199 canonical commands.

**Dedup (−25).** The same command surfaces on several enums because each surface
has its own action type. 20 canonical commands have more than one variant:
`copy`, `cut`, `paste`, `smart_paste`, `delete`, `delete_selected`, `duplicate`,
`undo`, `redo`, `save`, `select_all`, `rotate_selected`, `rotate_selection`,
`place_component`, `set_tool`, `toggle_auto_focus`, `toggle_all_filters`,
`toggle_selection_filter` and two more. `select_all` alone is four variants
(`ActiveBarAction`, `FootprintEditorMsg`, `MenuMessage`, `SelectionRequest`).

**Parameterisation (−20).** A family of variants that enumerates a *data domain*
becomes one command plus an argument, not N ids. The test, applied in order:

- Does the label read as a verb ("Align Left") or verb-plus-value ("Set Net
  Color: Fuchsia")?
- Is the varying part a closed set of verbs, or a value that could grow or be
  user-extended?
- Do the variants share identical enablement / undo / gui flags (argues
  parameter) or differ (argues distinct ids)?

Applied to `ActiveBarAction`'s 83:

| family | variants | verdict |
| --- | ---: | --- |
| `NetColor*` (Blue, LightGreen, LightBlue, Red, Fuchsia, Yellow, DarkGreen, Custom) | 8 | **one** `set_net_color(color)`. Colours are a value domain, and `Custom` already proves it is open-ended. `ClearNetColor` / `ClearAllNetColors` stay distinct — different semantics. |
| `PlacePower*` (GND, VCC, Plus12, Plus5, Minus5, Arrow, Wave, Bar, Circle, SignalGND, Earth) | 11 | **one** `place_power_port(style)`. Style is a closed-today, open-tomorrow value domain. |
| `InsideArea` / `OutsideArea` / `TouchingRectangle` / `TouchingLine` | 4 | **one** `set_selection_mode(mode)`. The catalog already treats selection mode as a single cycling axis — `cycle_selection_mode` is bridged at `bridge.rs:31`. |
| `Align*` / `Distribute*` (Left, Right, HorizontalCenters, Top, Bottom, VerticalCenters, ToGrid, DistributeHorizontally, DistributeVertically) | 9 | **stay distinct.** Small closed verb set, individually key-bound by Altium convention, individually named in every peer tool. |
| `Draw*` / `Place*` tools | 30+ | **stay distinct.** Each is a different tool with a different gesture. |

Net: `ActiveBarAction` contributes ~60 commands, not 83.

**Why parameterise at all:** command ids are frozen public API — a user's TOML
and a future plugin both pin them. Every id not minted is a name never to be
deprecated. Minting `net_color_fuchsia` today means deprecating it when
`set_net_color` inevitably arrives. When in doubt, parameterise.

**This is Phase-2 work, not Phase 4.** #274's own column list includes `args?`,
so *classifying* a command as parameterised belongs here. Only the `ArgSchema`
machinery that carries the argument is Phase 4.

One variant is accounted for explicitly: `ActiveBarAction::PlaceComponent`
(`active_bar/mod.rs:403`) is the 84th, and is constructed at **no** scanned
surface. It dedups against `MenuMessage::PlaceComponent` /
`MenuMessage::LibraryPlaceComponent`, which are live.

## 5. Gap A — 72 shipped keyboard shortcuts do nothing

The most severe finding, and the only one a user can hit today. **Severity:
HIGH** — no data-loss or destructive path (every dead id fails in the safe
direction, doing nothing), but a shipped default profile where more than half the
bindings are silently swallowed is a trust defect, and the Keyboard Shortcuts
pane advertises them as live.

| | count |
| --- | ---: |
| catalog ids bound to a **key sequence** in `altium.toml` or `classic.toml` | 127 |
| — **live** (have a bridge arm) | 55 |
| — **dead** (no bridge arm) | **72** |
| catalog ids bound only by a **pointer gesture** | 3 |
| unmapped catalog ids bound by neither | 0 |

Per profile: `altium.toml` binds 73 catalog ids, 28 dead. `classic.toml` binds
89, 51 dead. Every unmapped catalog id is bound somewhere — the 75-without-an-arm
set and the bound set are the same set, split 72 key / 3 pointer.

The 3 pointer-only ids — `toggle_selection` (`altium.toml:67`,
`"Shift+Click"`), `highlight_related_net_objects` (`"Alt+Click"`) and
`edit_object_properties` (`"DoubleClick"`) — are a **different, benign case**.
`ShortcutTrigger::parse` routes any `*+Click` / `Click` / `DoubleClick` string to
`ShortcutTrigger::PointerGesture` (`keymap/binding.rs:180-183`), and
`resolve_keymap_stroke` only ever receives keyboard `KeyStroke`s. They are
reference rows for the shortcuts pane, never dispatched, and they do **not**
intercept the canvas's own shift-click multi-select. No bug there; they are
excluded from the 72 so the headline stays honest.

**The 72 fail silently.** `take_keymap_match` (`app/dispatch/keymap.rs:63-68`)
resolves the stroke to an `AppCommandId`, calls `core_to_message`, and returns
`Some(Task::none())` when the bridge yields `None`. The keystroke is
**consumed** — no fallthrough to the canvas, no diagnostic, no log line. The user
presses the key from the shipped Altium profile and nothing happens, with nothing
to look at. `bridge.rs:14-17` documents this as deferred.

**The 72 are a mix of two very different problems**, and the split matters
because it decides whether a fix is one bridge arm or a feature:

- **Bridge gap** — the action exists and the user can click it; only the keyboard
  route is dead. Verified by grep: `place_no_erc` (`ActiveBarAction::PlaceNoERC`),
  `place_compile_mask` (`::PlaceCompileMask`), `place_power_symbol`
  (`::PlacePower*`), `place_wire_to_bus_entry` (`::PlaceBusEntry`),
  `move_selection` (`::MoveSelection`).
- **Unimplemented** — the catalog entry and the key binding are the *only*
  occurrences in the tree; there is no action to reach. Verified absent from
  `crates/signex-app/src` and `crates/signex-engine/src` entirely:
  `measure_distance`, `break_wire`, `place_global_label`,
  `place_hierarchical_label`, `repeat_last_item`, `rubber_stamp_copy`.
  `place_junction` and `place_no_connect` have a *data type*
  (`clipboard_junctions`, `documents.rs:652`) but no placement action.

A precise per-id split of all 72 is not attempted here — name-based matching is
too noisy to trust (`place_no_erc` vs `PlaceNoERC` does not match mechanically),
and it needs a per-id review. Both categories are confirmed present by the
spot-checks above. **That per-id triage is the natural first deliverable of the
ratchet PR in §10**, because the ratchet's pinned list is exactly the worklist
that needs classifying.

Either way the user-facing symptom is identical and the §10 fix order is
unchanged: a dead key is a dead key whether the cause is a missing arm or a
missing feature, and the diagnostic tells them apart for free.

The full 72 are listed in the row table (§11) with status `dead key`.

This is **pre-existing**, not introduced by any open PR: trunk's behaviour and
PR #504's `dispatch_command` funnel are identical here. But #504 is the moment to
pin it, because it makes the funnel single.

## 6. Gap B — the command descriptor is 0% populated

#275 landed via #479 and extended `CommandMetadata` into a descriptor:
`icon: Option<IconId>`, `keybind: Option<KeyBind>`, `enable: Enablement`,
`flags: CommandFlags { gui_only, mutates_doc, undoable, hidden }`
(`keymap/catalog/mod.rs`).

**All four fields are explicitly set in 0 of the 134 entries.** Every entry ends
in `..CommandMetadata::DEFAULT`, so the catalog carries
`icon: None, keybind: None, enable: Enablement::Always, flags: all-false` for
every command, universally.

Consequences for the open slices:

- **#368 (enablement predicates + dispatch gating)** has nothing to gate on.
  Populating `enable` across the existing 134 is a prerequisite, not part of
  #368's dispatch work, and it needs a per-command call between
  `RequiresSelection` / `RequiresDocument(kind)` / `RequiresNetColor`.
- **#374 (show the bound shortcut in palette rows)** cannot read `keybind`; it
  must read the active profile. That is arguably correct — a profile binding
  overrides a catalog default — but it means `keybind` is not #374's source and
  stays unpopulated until something needs it.
- `flags.gui_only` is the natural home for this audit's `gui_only?` column. It is
  empty, so that column is unanswered for all 134 and is proposed, not recorded,
  in §11.

## 7. Gap C — every surface keeps its own action table

Only the keybinding surface is registry-driven. The others each maintain a
private list, which is a direct violation of the epic's founding principle that
menus, keybindings, the palette, the CLI and plugins are *consumers of the same
table*.

| surface | actions it offers | id-backed | gap |
| --- | ---: | ---: | ---: |
| keybindings (`altium` + `classic`) | 127 key-bound ids | 127 | 0 (but 72 dead — §5) |
| menu bar (`menu_bar/view.rs`) | 69 leaf rows | 31 | 38 |
| command palette (`app/command_palette.rs`) | 48 rows | 0 | 48 |
| footprint-editor menus | 50 | 0 | 50 |
| app context menus | 33 | 5 | 28 |
| symbol-editor menus | 24 | 0 | 24 |
| active bar (schematic) | 83 | 0 | 83 |

- **Palette (#366) — a named consumer-drift gap.** `menu_command_table()`
  (`app/command_palette.rs:299`, consumed at `:82`) is a hand-kept 48-row
  `&[(&str, MenuMessage)]`. `command_palette.rs` contains **zero** references to
  `AppCommandId`, `metadata_for`, or the catalog. 48 palette rows against 134
  catalog ids and 60 `MenuMessage` variants — three tables, three drift paths.
  A command added to the catalog today does not appear in the palette, and
  nothing fails.
- **Active bar (#271).** `active_bar/{mod,dropdown,legacy}.rs` contain **zero**
  references to `AppCommandId` / `metadata_for` / the keymap. All 83 actions
  dispatch `ActiveBarAction` directly. This is the largest unmapped block and the
  reason #271 is bigger than "wire three surfaces together".
- **Menu bar.** The best-covered chrome surface at 31 / 69 id-backed rows,
  courtesy of #270 / #282. The remaining 38 leaf rows are label-only.
- **Library editors.** 74 actions across the symbol and footprint editors, none
  id-backed. Neither editor is mentioned in any open slice of the epic.

**A sharper form of the same drift: the menu and the keyboard reach the same
command through different message variants.** `MenuMessage::PlaceWire` is what
the Place ▸ Wire menu row sends; `place_wire` — the id the Altium profile binds —
dispatches `ToolMessage::SelectTool(Tool::Wire)`. Same command, two code paths,
neither aware of the other. Eleven such pairs are tabulated in §11's companion
file (`place_wire`, `place_bus`, `place_label`, `place_text`,
`open_components_panel`, `save`, `delete`, `smart_paste`, `rotate_selected`,
`toggle_grid`, `zoom_fit`). They can diverge in behaviour with nothing failing.
This is why #271 is a behaviour change rather than a rewiring, and it is the best
argument for landing #367's single `dispatch_command` funnel first.

## 8. Gap D — no generated command reference exists (#277)

`docs/KEYBOARD_SHORTCUTS.md` documents the *TOML profile format*, not the
commands: it contains no command table at all. There is today no document,
generated or hand-written, listing the 134 command ids with their labels and
default bindings — so a user cannot discover what they may bind a key to except
by reading `keymap/catalog/*.rs`.

## 9. Phase-2 sizing

Two independent bodies of work, often conflated:

| work | size |
| --- | ---: |
| **A.** drain the 72 dead key-bound catalog ids — arm where the action exists, implement or retire where it does not (§5) | 72 ids, mixed cost |
| **B.** mint ids + arms for the 156 canonical commands that have none | 156 |
| — of which `ActiveBarAction`, after parameterisation | ~60 |
| — of which the library editors (footprint 36 + symbol 6 + `SymbolToolMsg` 7 + `FootprintContextAction` 3) | 52 |
| — of which remaining menu-bar / context-menu leaves | ~38 |
| **catalog size** | **134 → ~290** |

A and B barely overlap: only 3 of the 156 new commands share a name with one of
the 72 dead ids. A is a debt paydown on ids that already ship; B is the actual
Phase-2 expansion. Doing B first while A rots would double the dead-binding
count.

The catalog roughly doubles. Since command ids are public API, the **naming pass
matters more than the wiring pass**, and #276's golden snapshot (PR #506) should
land *before* the bulk admission, not after.

## 10. Recommended order

1. **Stop the bleeding on §5**, in one PR, in this order:
   **(b) a diagnostic first** — log (and ideally surface on the status line) when
   a resolved id has no arm, at the `None` branch in `take_keymap_match`
   (`app/dispatch/keymap.rs:67`). Cheapest possible change, alters no behaviour,
   and converts every future user report from "key does nothing" to an
   actionable id. **(a) a coverage-ratchet test second** — embed the current
   72-id unmapped set and fail if it grows. This matches house style: the bridge
   already self-scans its own source (`bridge.rs:100-125`), and the repo has
   ratchet precedent in the god-file gate.
   **Not (c) — do not strip the bindings from the shipped TOMLs.** They are the
   published spec of the Altium and classic profiles and the ids are the public
   contract; removing them churns a user-facing file twice and destroys the very
   worklist the ratchet pins.
2. **Land #276 (PR #506)** so ids are frozen before the catalog grows.
3. **Land #367 (PR #504)** — the single `dispatch_command` funnel every later
   slice routes through.
4. **Populate the descriptor (§6)** — `enable` and `flags` for the existing 134.
   Unblocks #368 and answers this audit's `gui_only?` / `enable` columns with
   recorded values instead of proposals.
5. **Admit `ActiveBarAction`** (§4, §9) — the largest block, and the one whose
   naming decisions are hardest to reverse.
6. **#366 / #374** — palette from the registry, then shortcuts on the rows.
7. **#271** — the remaining chrome surfaces, once the ids they need exist.
8. **#277** — generate the reference from the catalog; near-free once the catalog
   is the single source.

## 11. The row table

The full 199-row classification — command id, `args?`, group, proposed `enable`
predicate, status (bridged / dead key / new), and every message variant that
dedups into it — lives in
[`command-registry-action-surface-2026-07-25-rows.md`](./command-registry-action-surface-2026-07-25-rows.md).

It is keyed by **command id**, not by variant, because the id is the stable thing
and the bridge is id→`Message`. The variant→id direction stays derivable from the
`message variant(s)` column.

## 12. Open questions for Caner

Per the standing menu-content authority rule, these are not mine to decide:

- **The parameterisation calls in §4.** Command labels surface in menus, so
  collapsing `NetColorFuchsia` into `set_net_color(color)` changes what a menu
  row can say. The `NetColor*` and `PlacePower*` calls look clear; the
  selection-arm modes (`InsideArea` / `OutsideArea` / `TouchingRectangle` /
  `TouchingLine`) are borderline.
- **`BringToFrontOf` / `SendToBackOf`** are interactive two-step gestures rather
  than one-shot commands. Distinct ids, or not commands at all?
- **Whether the Keyboard Shortcuts pane should mark the 72 dead bindings** while
  they are being filled in, or stay silent and let the ratchet drain them.
