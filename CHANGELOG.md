# Changelog

All notable changes to Signex ship here. Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) loosely and [Semantic Versioning](https://semver.org/spec/v2.0.0.html) strictly.

Each release section is authored **before** the `vX.Y.Z` tag is created, so the release workflow picks it up as the GitHub Release body. See `.claude/hooks/pre-release-guard.sh` for the enforcement.

## [Unreleased]

## [0.10.0] — 2026-04-29

First slice of the **v0.10 Library & Polish** milestone — the Library Browser tab scaffold. Double-clicking a `.snxlib` file in the project tree now opens a dedicated tab that lists the components contained in the library package; the surface is intentionally read-only this release. v0.10.1 adds the side-by-side symbol preview pane on row click; v0.10.2 adds the filter / search bar above the table.

The library subsystem paused on 2026-04-29 alongside the v0.9.0 Apache-clean cutover (preserved at the `v0.9-snxlib-paused-2026-04-29` tag) is being re-landed file-by-file on top of the Apache-clean foundations rather than wholesale-rebased — every contributing change is verified against the issue #62 invariants before it ships.

### Added

- `signex-types::library::Library` and `LibraryComponent` — in-memory representation of a `.snxlib` package. `LibraryComponent` carries a sentinel-`nil` `symbol_uuid` / `footprint_uuid` for unbound rows; v0.10.8 wires the Pick Symbol/Footprint flow that fills them in.
- `signex-types::format::SnxLibrary` — TOML+TSV envelope for `.snxlib` files. `parse(&str)`, `write_string()`, and `write_string_borrowed(&str, &Library)` mirror the v0.9.1 borrow-based pattern from `SnxSchematic` / `SnxPcb`, so v0.10.6's async-save plumbing drops in unchanged.
- `signex-types::format::LibraryComponentRow` — TSV adapter row implementing `SnxTable` with columns `uuid name value footprint description symbol_uuid footprint_uuid`.
- `signex-app::TabDocument::Library(Library)` variant — Library Browser tabs cache the parsed library on the tab. New `as_library()` accessor; the existing `as_pcb()` is unchanged.
- `signex-app` open path — `open_document_path` and the project-tree handler both route `.snxlib` to a new `open_library_tab(path, title, library)` mirroring `open_pcb_tab`.
- `signex-app::view::view_library_browser` — read-only Library Browser tab body. Header strip (library name + component count + optional description), 4-column scrollable table (Name, Value, Footprint, Description), and an empty-state placeholder when the library has zero components.
- `assets/samples/library/resistors-standard.snxlib` — 3-component sample library shipped with the source tree for the smoke test.

### Changed

- **Project tree click semantics — single click highlights, double click opens.** Previously a single click on a leaf both highlighted and opened the file, which was easy to trigger accidentally while navigating. Now a single click only highlights the row (`panel_ctx.selected_tree_path`); a second click on the same row within 500 ms opens the file. The icon-gate (only schematic / PCB / `.snxprj` / `.snxlib` / `.snxsym` / `.snxfpt` / `.snxsim` leaves open) is unchanged. Right-click → Open in the context menu still opens immediately, bypassing the double-click latch. Folder rows still toggle expand/collapse on a single click. Implementation lives in `signex-app::handlers::dock::project_navigation::handle_dock_project_navigation_panel_message` and routes through a new `open_tree_path_if_document` helper shared with the right-click menu path.

### Apache-clean residual polish (bundled)

In response to ongoing discussion on [issue #62](https://github.com/alplabai/signex/issues/62), v0.10.0 also lands a residual-polish pass that removes vestigial KiCad-shaped names from the codebase. None of these changes alter user-visible functionality, but they reduce the surface that reads as "Signex was once derived from KiCad" — the substantive change was already delivered in v0.9.0; this is the cosmetic follow-through.

- **Style enum variant renames.** `MultisheetStyle::KiCad` / `LabelStyle::KiCad` / `PowerPortStyle::KiCad` → `::Standard` for all three. The `::Altium` variants stay (Altium is the project's stated design reference and isn't a GPL exposure). On-disk preference strings remain `"kicad"` / `"altium"` for backward compatibility with existing `prefs.json` files; the user-facing dropdown labels also stay "KiCad" / "Altium" so users coming from those tools recognise the modes.
- **Legacy KiCad symbol-library scanner removed.** `helpers::find_kicad_symbols_dir` (which walked `/usr/share/kicad/symbols`, `C:\Program Files\KiCad\…`, etc.) and `helpers::list_kicad_libraries` deleted. The associated `DocumentState::kicad_lib_dir` field, `PanelContext::kicad_libraries` field, and the dock-panel `library_browser.rs` handler are gone. The Components panel's library dropdown is replaced by an inline placeholder until the v0.10.x `.snxlib` plumbing repopulates it. The scanner had been a no-op since v0.9.0 (the load handler logged "convert with companion tool" and skipped); removing it cleans up code that pointed at KiCad install layouts without serving a real flow.
- **Direct-open KiCad-extension dispatch arm removed.** `open_document_path` no longer matches `"kicad_pro" | "kicad_sch" | "kicad_pcb"`; the same arm in the project-tree double-click handler is also gone. Opening a `.kicad_*` file now falls through to a generic "unsupported file type" error. The migration story for KiCad users is unchanged — run `signex-kicad-import` first; the README and `docs/LICENSING.md` continue to describe the flow.
- **`docs/LICENSING.md` strengthened.** New "LLM context discipline" section documenting that post-cutover development uses LLM-assisted workflows where KiCad source code is never placed in agent context, prompts, retrieval indexes, or reference material. Versions table updated through v0.10.0.
- **License Guard CI tightened.** Three new jobs added to `.github/workflows/license-guard.yml`: forbid re-introduction of the v0.10.0-renamed names (`MultisheetStyle::KiCad`, `find_kicad_symbols_dir`, etc.); forbid KiCad-numbered layer-id constants (`F_CU = 0`, `B_CU = 31`, …); forbid the `Net-(<r>-Pad<p>)` auto-net-name format string.

### Tests

- `signex_types::format::tests::snxlibrary_round_trip_preserves_components` — locks parser/writer round-trip parity.
- `signex_types::format::tests::snxlibrary_borrow_matches_owned` — owned/borrowed serialise parity.
- `signex_types::format::tests::snxlibrary_rejects_unknown_version` — `UnsupportedVersion` error path.
- `signex_types::format::tests::snxlibrary_parses_empty_components_block` — empty-library round-trip.
- `signex_types::format::tests::shipped_sample_library_parses` — guards the shipped sample against parser drift.

### Constraints — Apache-clean invariants (carry forward from v0.9.0)

- No `use kicad_parser` / `use kicad_writer` in `crates/`.
- No `kicad-parser` / `kicad-writer` Cargo.toml deps.
- License Guard 4 jobs and `cargo-deny` continue to gate every PR.

## [0.9.1] — 2026-04-29

The **async save + borrow-based serialise** patch deferred from v0.9.0. Schematic saves were already imperceptible; this release targets the huge-PCB Ctrl+S stutter (~1–2 s on ~500 K-track boards) by moving the disk write off the UI thread and skipping the full-document clone that the previous serialise required.

### Changed

- `signex-types::format::SnxSchematic::write_string_borrowed(&str, &SchematicSheet)` and the matching `SnxPcb::write_string_borrowed(&str, &PcbBoard)` — borrow-based serialise. The owned `write_string()` methods now delegate to these, so byte-for-byte output is unchanged. Skips the ~50–100 ms `self.sheet.clone()` / `self.board.clone()` that the engine previously paid before each serialise.
- `signex-engine::Engine::serialize_for_save(&self) -> Result<Vec<u8>, EngineError>` — pure, side-effect-free serialise using the borrow path. Cheap to call repeatedly; no path mutation.
- `signex-engine::Engine::write_to_file(path, bytes)` — stateless disk write half of the async-save pair. Pair with `serialize_for_save` to run the write off the UI thread.
- `signex-engine::Engine::record_saved_path(path)` — set the engine's path after an async save resolves.
- `signex-app` save handler — `Ctrl+S` and File → Save now serialise on the UI thread (cheap with the borrow-based path) and dispatch the disk write via `iced::Task::perform`. iced's tokio runtime runs the blocking `std::fs::write` on a worker thread, so the UI stays responsive even on huge boards.
- New `Message::SaveFileFinished(PathBuf, Result<(), String>)` completion arm.
- Status bar shows a small "Saving…" pill for the duration of the off-thread write; transient save errors surface as a 3-second pill before fading.

### Tests

- `signex_types::format::tests::schematic_borrow_matches_owned_serialise` — locks owned/borrowed parity for `SnxSchematic`.
- `signex_types::format::tests::pcb_borrow_matches_owned_serialise` — same, for `SnxPcb`.
- `signex_engine::tests::serialize_for_save_returns_parseable_bytes` — serialise + reparse round-trip.
- `signex_engine::tests::write_to_file_writes_serialised_bytes` — disk write + reparse round-trip via tempfile.

## [0.9.0] — 2026-04-29

The **Apache-clean cutover** release. Resolves [issue #62](https://github.com/alplabai/signex/issues/62) raised by Seth Hillbrand of the KiCad project flagging that several Signex crates derived from KiCad's GPL-3.0 source were shipping under Apache-2.0. The main `signex` repository is now Apache-2.0 clean and contains no KiCad-derived code; KiCad I/O moves to the optional [signex-kicad-import](https://github.com/alplabai/signex-kicad-import) companion tool (GPL-3.0-or-later), shipped independently.

The library subsystem (Library Browser, SCH Library editor, Component Editor) that was in flight on `feature/v0.9-snxlib-as-file` is preserved at the `v0.9-snxlib-paused-2026-04-29` tag and ships as **v0.10.0** on top of the Apache-clean foundations from this release.

### Native file formats — TOML + TSV bulk blocks

- **`.snxsch`** schematic format — TOML manifest (`format = "snxsch/1"`) wrapping the `SchematicSheet` payload via 4 adapter row tables (`[sheets.components]`, `[sheets.wires]`, `[sheets.junctions]`, `[sheets.labels]`) plus an `[extras.*]` TOML block for fields that don't fit a flat row schema. Same format family as `.snxlib` / `.snxsym` / `.snxfpt`.
- **`.snxpcb`** PCB format — same shape with adapter rows for `[footprints]`, `[pads]`, `[tracks]`, `[vias]`, plus `[stackup]` / `[[zones]]` in regular TOML for hierarchical data.
- Format chosen for line-diff-friendly git workflows, ~5× smaller than the equivalent JSON, single file per design, and zero S-expression-shaped grammar that could regress KiCad-derivation exposure.
- Round-trip preserves every field on `SchematicSheet` / `PcbBoard`.

### Apache-clean signex-types

- `PinElectricalType` (12-variant, KiCad-shaped) → **`PinDirection`** (14 variants — adds Signex-original `GroundReference`, `Differential`, `Clock`; collapses `Free`+`Unspecified` into `Unclassified`; renames `OpenCollector`/`OpenEmitter` → `OpenDrainLow`/`OpenDrainHigh`). Design rationale in `crates/signex-types/docs/pin-design.md`.
- `PinShape` (9-variant) → **`PinShapeStyle`** (7 variants — drops per-direction-low modifiers since polarity is now on `PinDirection`; adds `HysteresisInput` / `HysteresisOutput` / `Schmitt`).
- KiCad-numbered `LayerId(u8)` constants (`F_CU=0`, `B_CU=31`, …) → **`SignexLayer`** semantic enum + `LayerKind` categories + `altium_label()` per the Altium-flavoured Signex UI naming.
- KiCad markup parser (`~{X}` / `^{X}` / `_{X}` curly-brace syntax) → **`parse_signex_markup`** using a Markdown subset: `**bold**`, `*italic*`, `~~strike~~`, `^superscript^`, `~subscript~`, `_~overbar~_` (Signex extension for active-low signal naming), `[label](url)`, `\X` escape.
- `kicad_auto_net_name_from_pins` (`Net-(<r>-Pad<p>)` format string) → **`auto_net_name`** returning `unnamed-<sheet>:<ref>:<pin>`.

### Crates removed from main repo

- `crates/kicad-parser/` (3,938 LOC) — moved to the GPL-3.0 companion repo.
- `crates/kicad-writer/` (2,274 LOC) — moved to the GPL-3.0 companion repo.
- `crates/signex-output/src/netlist/kicad_sexpr.rs` (336 LOC) — KiCad netlist exporter; moved to the companion or a future `signex-kicad-export` sibling.

### Companion repo `signex-kicad-import` (GPL-3.0-or-later)

- New separate repository at <https://github.com/alplabai/signex-kicad-import>.
- Houses the relocated `kicad-parser` + `kicad-writer` crates plus a CLI binary `signex-kicad-import`.
- One-way conversion: `.kicad_sch` / `.kicad_pcb` / `.kicad_pro` → `.snxsch` / `.snxpcb` / `.snxprj`. Originals remain intact.
- Distributed independently — Apache consumers of Signex Community see no GPL aggregation in their build closure.

### Performance (deferred to v0.9.1)

**Async save** and **borrow-based serialise** were planned for v0.9.0 but
are deferred to **v0.9.1** to bound the v0.9.0 release scope. The
current `Engine::save_as` runs synchronously on the calling thread
with a full document-clone before serialisation; on huge PCBs (~500K
tracks) this blocks the UI for ~1–2 s on Ctrl+S. Tracked as a v0.9.1
follow-up:

- `Engine::save_as_async` returning an `iced::Task` that serialises +
  writes off the UI thread.
- Borrow-based `SnxSchematic::write_string_borrowed(&SchematicSheet)`
  to skip the document-clone (~50–100 ms saving on huge PCBs).

Schematics save instantly today; the perf work is targeted at the
high-end PCB case.

### CI guards

- `.github/workflows/license-guard.yml` — fails any push or PR that re-introduces `kicad-parser` / `kicad-writer` imports or removed KiCad-shaped types.
- `.github/workflows/pr-license-declaration.yml` — fails PRs whose description is missing the contributor self-declaration block or whose author marked `KiCad source consulted: yes`.
- `cargo-deny` license job in `.github/workflows/ci.yml` — rejects GPL / AGPL / LGPL / unlicensed transitive dependencies.
- `deny.toml` — Apache-compatible permissive allowlist.

### Documentation

- `docs/LICENSING.md` — canonical licensing rationale + audit-trail pointers + contributor guidance.
- `docs/audit/kicad-derivation.md` — file-by-file audit of every removed item plus the residual-mention catalog.
- `docs/audit/contributors-2026-04-29.md` — contributor consent record (audit trail).
- `docs/audit/third-party-kicad-parsers.md` — survey of clean-room third-party Rust KiCad parsers.
- `docs/audit/release-notes-remediation-v07-v08.md` — text to apply manually to v0.7.0 / v0.7.1 / v0.8.0 GitHub Release bodies (those releases shipped Apache-2.0 with KiCad-derived code in error; flagged superseded).
- `docs/audit/communication-drafts.md` — drafts of the issue #62 reply, signex.dev hero copy, GitHub Discussions sticky, Discord post.
- `CONTRIBUTING.md` — added License compliance section + PR self-declaration block.
- `crates/signex-types/docs/pin-design.md` — rationale for the curated `PinDirection` / `PinShapeStyle` variant sets.

### Breaking changes

- `.kicad_sch` / `.kicad_pcb` / `.kicad_pro` files no longer open directly in Signex. Users with existing KiCad projects install the [signex-kicad-import](https://github.com/alplabai/signex-kicad-import/releases) companion tool, run it once against their `.kicad_pro`, and open the resulting `.snxprj` from then on.
- `signex_types::schematic::PinElectricalType` and `PinShape` are gone — downstream code uses `PinDirection` / `PinShapeStyle`.
- `signex_types::layer::{F_CU, B_CU, F_SILKS, …}` constants are gone — downstream code uses `SignexLayer::*` variants.
- `signex_types::markup::parse_markup` and `kicad_auto_net_name_from_pins` are gone — downstream code uses `parse_signex_markup` and `auto_net_name`.
- `crates/kicad-parser` and `crates/kicad-writer` are gone from the workspace.

### Versions affected

The v0.7.0 / v0.7.1 / v0.8.0 release notes have been edited to flag those releases as superseded with the licensing notice. Binaries remain available for historical use; please prefer v0.9.0 (or later) for new installations.

## [0.8.0] — 2026-04-27

The output-and-polish release. Adds the full PDF / BOM / netlist export pipeline, multi-project workspaces, Altium-style dirty tracking, a chrome refactor with the new `TabPill` widget, hierarchical-sheet rendering parity, and KiCad-parity field autoplace. Every v0.8.x sub-feature ships under this one tag.

### Output subsystem

- **PDF export** — tabbed Export modal (file picker / settings / pan-drag preview), bookmarks per sheet, theme palette aware, DPI hookup, physical-structure tokens
- **BOM** — Altium-spec preview modal with column picker, variant picker, sort / drag / scroll, options applied on export, format-layer export (CSV / HTML / XLSX-ready)
- **Netlist** — date + path polish, KiCad-format export, output engine validation
- Unified PDF preview modal — File ▸ Export PDF and File ▸ Print Preview both open the same overlay; legacy `view_pdf_options_dialog` + 12 `ExportPdfSet*` / `ExportPdfDialog*` variants pruned

### Multi-project workspace

- Multiple `.snxprj` projects open side-by-side in the same window
- `TabInfo` carries `Option<ProjectId>`; `active_project` scoped to focused tab
- Accent-tinted active project root + per-project Close menu action
- Per-tree-path semantics for project tree actions (right-click on project B with project A active still operates on B)
- Phase 2.5 cleanup — legacy single-project fields removed from `DocumentState` (#54, #55, #56)

### Dirty tracking + tab right-click menu

- Altium-style `dirty_paths` model — closing a tab never prompts; engines park while dirty
- Project-close prompts with a Save All / Discard All / Cancel modal listing every dirty file
- Tab bar right-click menu replaces the inline close / undock buttons

### Hierarchical sheets

- Child-sheet pins rendered as Altium-style ports (no protruding stubs), inward direction
- Pin labels rotate vertical on top / bottom edges for parity
- Per-sheet stroke / fill colours round-trip and editable from the Properties panel
- `[[multisheet]]` Style preference (with sheet-alpha round-trip fix)
- Child-sheet name / filename rendered outside the box per multisheet style
- Inline preset palette under the colour row before opening the picker
- Altium-green default sheet palette

### Symbol field handling (KiCad parity)

- Autoplace fields on rotate / mirror — body-bbox classification, full text-height clearance, anchored to selection bbox including pins, two-text-height autoplace clearance, body-edge stack alignment
- Reference and Value rotate independently; rotate / mirror compose with symbol orientation
- Property `justify` mirrored under rotation fold and mirror flags
- KiCad `GetDrawRotation` toggle parity; symbol field rotation treated as absolute screen angle
- Re-autoplace marked fields on load to repair legacy rotations
- Property `justify` parsing defaults to Center per KiCad spec
- Pin numbers rotate along the pin axis for vertical pins
- Single em-size used for schematic text (parity with KiCad)

### Chrome refactor

- New `TabPill` custom widget — 3-sided borders sharing L / R edges between adjacent tabs, permanent strip baseline, theme-border instead of pure black, drag accent follows theme accent
- Modal close button unified across all 11 modals; chrome icon sizes bumped; thicker panel strokes; modal alignment so borders trace rounded corners
- Inactive tab fill is now visible (was theme-bg, blending in)
- New `chrome-catalog` crate + UI iteration workflow doc
- Taller top-chrome search bar (24 → 28); narrow Justification labels in Properties

### Canvas + engine

- Adaptive multi-level grid that scales smoothly with zoom
- Schematic editor grid style preference

### Plumbing

- `kicad-parser` / `kicad-writer` round-trip per-sheet stroke / fill colours and respect label style for defaults
- `signex-types::Label` gains `justify_v` to match renderer + parser contract
- Engine command surface expanded with multi-project routing

### Issues closed

- #54 multi-project workspace
- #55 multi-project + chrome polish + unified PDF preview
- #56 phase 2.5 cleanup of legacy single-project fields

[Full changelog](https://github.com/alplabai/signex/compare/v0.7.1...v0.8.0) · [Release artifacts](https://github.com/alplabai/signex/releases/tag/v0.8.0)

## [0.7.1] — 2026-04-24

Patch release addressing a macOS launch failure on Apple Silicon.

### Fixed

- **macOS (Apple Silicon) cannot launch the shipped `.app`** (#49). The
  DMG-packaged bundle was unsigned; arm64 macOS refuses to execute any
  binary without at least an ad-hoc signature, so users on M-series
  Macs saw "Signex is damaged and can't be opened" / "cannot be
  verified" immediately after dragging the app to Applications. The
  installer script now ad-hoc signs the bundle (`codesign --force
  --deep --sign -`) as part of DMG assembly. This is the minimum
  viable shipping state for arm64 until a Developer ID certificate
  and notarisation credentials are wired into CI.

### Known issues / workarounds

- The DMG still carries the downloaded-from-internet quarantine flag,
  so first-launch users will see a "cannot be verified" Gatekeeper
  prompt. Bypass it with **right-click → Open** on the app icon the
  first time, or run
  `xattr -dr com.apple.quarantine /Applications/Signex.app` in
  Terminal. Subsequent launches work without prompts.

## [0.7.0] — 2026-04-22

The schematic-phase release. Adds ERC & validation, project-wide annotation, real multi-window architecture via `iced::daemon`, per-window engine/canvas, borderless chrome, and a full Signex brand rollout. Every v0.7.x sub-feature ships under this one tag.

### ERC & validation

- New `signex-erc` crate with **11 rule kinds** (`run()` single-sheet, `run_with_project()` cross-sheet)
- Project-wide ERC across open, cached, and unopened sheets
- Cross-sheet BadHierSheetPin: parent pins ↔ child hier-labels, both directions
- ERC markers rendered as overlays — zero KiCad schema drift
- Messages panel with E/W/I severity pips and per-sheet cached results
- Altium-style ERC dialog + Preferences panel (per-rule Error / Warning / Info / Off grid)
- `ErcContext` abstraction + rule metadata infrastructure
- Shortcuts: `F8` Run ERC, `F9` AutoFocus, `Alt+A` Annotate, `Shift+Alt+A` Reset & Renumber

### Annotation

- Project-wide change list parsed from every sheet in the project
- Altium two-column Annotate dialog with Reset All / Reset & Renumber / Reset Duplicates
- Per-symbol lock, draggable modals
- Power-port skip in both annotation and change list
- Design → Annotation submenu matching Altium's layout

### Multi-window (`iced::daemon`)

- Borderless main window + OS-decorated secondary windows
- Modals, tabs, and panels all detach into real OS windows
- Per-window engine: `document_state.engines: HashMap<PathBuf, Engine>` — every open tab keeps its Engine loaded so undocked tabs edit independently
- Per-window canvas: `interaction_state.canvases: HashMap<window::Id, SchematicCanvas>` — pan / zoom / selection / render cache per window
- `document_state.window_active_path: HashMap<window::Id, PathBuf>` — each undocked-tab window can show a different tab
- Canvas events routed via `CanvasEventInWindow { window_id, event }`
- macOS: process exits when main window closes

### Editor & tools

- Lasso freehand select (Altium), bbox child-sheet hit, detached-field support
- Tab / panel drag-reorder with visual feedback
- Reorder picker polish — gray-X cursor while armed, Esc-cancel visible
- Unified gray-X placement cursor across every armed tool
- TAB-during-placement for Line / Rect / Circle / Arc / Polygon (width + fill pre-configured)
- Editable drawing Properties panel with live DrawingPreview canvas
- Stroke colour per drawing (round-trips in KiCad format)
- Erasable numeric `text_input` (per-field `String` buffer)
- Context-aware menus — Annotate / ERC / Save / Edit gated by `has_schematic` / selection
- Net-colour pen — strict-hit snap, union-find flood, render-only overrides, undo stack
- Move / z-order engine commands — `MoveSymbolAbsolute`, `ReorderObjects` (Front / Back / JustAbove / JustBelow)
- Active Bar — BringToFront / SendToBack + BringToFrontOf / SendToBackOf pickers
- Reset Duplicate Designators (project-wide, undoable for open tabs)
- Arc (3-click) + Polygon (click-by-click) placement tools

### Hierarchical sheets

- Double-click a sheet block → opens the child schematic
- Sheet-pin snaps to all four edges of the hierarchical sheet block
- Sheet-pin interactions + Altium label-style option

### Borderless chrome & brand

- Custom title bar: wordmark + menus + drag zone + search bar + min / max / close
- Per-monitor v2 DPI manifest (no bitmap stretching on hi-DPI)
- Roboto UI font (panels / toolbars / menus / dialogs); Iosevka stays the canvas font
- Windows 11 DWM rounded corners + drop shadow via `DwmSetWindowAttribute` (silent no-op on Win10 and non-Windows)
- Diagonal resize hit zones (NW / NE / SW / SE) via Stack overlay — keeps content y-origin natural
- Header logo bumped 74×24 → 96×31 for readability

### Icons & installer

- Signex brand SVGs (mark, wordmark, logo variants)
- Panton Bold wordmark regenerated from actual font (not fallback outlines)
- Tighter `signex-mark.svg` viewBox: S fills ~97 % of icon canvas (was ~58 %)
- Regenerated installer artifacts: Windows `.ico` (multi-size), macOS `.icns`, Linux PNGs
- Runtime window icon embedded via `iced::window::Icon`
- `signex.exe` icon + DPI manifest embedded at build time via `winres`
- Pure-Python fallback for `build-icons.sh` — `tools/build_icons.py`, no rsvg-convert / magick / inkscape needed

### Refactors

- `signex-engine/src/lib.rs` split into semantic modules
- `kicad-writer` migrated from `wln!` string formatting to full SExpr AST (`kicad-parser/sexpr_builder` → `kicad-writer/sexpr_render`)
- Named constants for PCB magic numbers; `lib_symbol` unit round-trip fix
- Wire rendering chains connected segments into polylines (rounded corners)
- Font-size constants corrected, hidden refs and pin-name rotation fixed

### Merged-in dependencies

- v0.6.1 render fixes (font scale, pin numbers, power-ref visibility)
- v0.6.2 AST sexpr pipeline
- v0.6.3 macOS runner pin (`macos-14`)
- v0.6.4 per-OS installers (`.exe`, `.dmg`, `.deb`, `.AppImage`)
- Node.js 24 Actions bump

### Performance

- `expand_to_net` is now `O(N)` via quantised `HashSet` (was `O(P²·N²)`)

[Full changelog](https://github.com/alplabai/signex/compare/v0.6.4...v0.7.0) · [Release artifacts](https://github.com/alplabai/signex/releases/tag/v0.7.0)

## [0.6.4] — 2026-04-20

First cumulative release since v0.6.1. Rolls in the abandoned v0.6.2 and the CI-fix-only v0.6.3.

### Installers (new)

- **Windows** — `signex-setup-x86_64-0.6.4.exe` / `signex-setup-aarch64-0.6.4.exe` via InnoSetup. Installs to `Program Files`, adds Start Menu entry and optional Desktop shortcut, proper uninstaller. Portable `.zip` also attached for scripted installs.
- **macOS** — `signex-macos-aarch64-0.6.4.dmg` with a full `Signex.app` bundle and `/Applications` drag-target. Registered as the editor for `.kicad_sch` / `.kicad_pro` files.
- **Linux** — native `.deb` (with `.desktop` entry + MIME types) plus a portable `.AppImage`. `.tar.gz` fallback also attached.

### KiCad pipeline refactor (from v0.6.2)

- AST-based S-expression pipeline in `kicad-parser` / `kicad-writer` with a property-metadata layer. More robust round-trip, less fragile than prior ad-hoc string handling.
- Named constants replace the magic numbers scattered through `signex-types`, `kicad-parser`, and `kicad-writer`.

### Release pipeline fixes (from v0.6.3)

- `aarch64-apple-darwin` pinned to `macos-14` so GitHub's `macos-latest` queue stalls don't take the whole release down (symptom that killed v0.6.2).

### Upgrade notes

No `.kicad_sch` / `.kicad_pcb` breaking changes — opening a v0.6.1 project in v0.6.4 is a clean round-trip. If you were on v0.6.2 or v0.6.3, nothing additional to migrate — v0.6.4 is a superset.

[Full changelog](https://github.com/alplabai/signex/compare/v0.6.1...v0.6.4)

## [0.6.3] — 2026-04-20 _(superseded by 0.6.4)_

- fix(ci): pin aarch64-apple-darwin to macos-14 (#34)

[Full changelog](https://github.com/alplabai/signex/compare/v0.6.2...v0.6.3)

## [0.6.1] — 2026-04-20

Render + KiCad round-trip fixes (font scale, pin numbers, power-ref visibility).

[Full changelog](https://github.com/alplabai/signex/compare/v0.6.0...v0.6.1)

## [0.6.0] — 2026-04-18

Full Schematic Editor — drag-move, properties editing, placement tools, iced_aw, Active Bar.

[Full changelog](https://github.com/alplabai/signex/commits/v0.6.0)
