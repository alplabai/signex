# Changelog

All notable changes to Signex ship here. Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) loosely and [Semantic Versioning](https://semver.org/spec/v2.0.0.html) strictly.

Each release section is authored **before** the `vX.Y.Z` tag is created, so the release workflow picks it up as the GitHub Release body. See `.claude/hooks/pre-release-guard.sh` for the enforcement.

## [Unreleased]

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

[Full changelog](https://github.com/alplabai/signex/compare/v0.7.0...v0.8.0) · [Release artifacts](https://github.com/alplabai/signex/releases/tag/v0.8.0)

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
