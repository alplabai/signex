<p align="center">
  <h1 align="center">Signex</h1>
  <p align="center"><strong>AI-First Electronic Design Automation</strong></p>
  <p align="center">Altium-class UX. KiCad compatibility. Open source.</p>
</p>

<p align="center">
  <a href="https://github.com/alplabai/signex/blob/main/LICENSE"><img src="https://img.shields.io/github/license/alplabai/signex?color=blue&label=License" alt="License"></a>
  <a href="https://github.com/alplabai/signex/releases"><img src="https://img.shields.io/github/v/release/alplabai/signex?include_prereleases&label=Release&color=orange" alt="Release"></a>
  <a href="https://github.com/alplabai/signex/stargazers"><img src="https://img.shields.io/github/stars/alplabai/signex?style=flat&color=yellow" alt="Stars"></a>
  <a href="https://github.com/alplabai/signex/issues"><img src="https://img.shields.io/github/issues/alplabai/signex?color=green" alt="Issues"></a>
  <a href="https://github.com/alplabai/signex/pulls"><img src="https://img.shields.io/github/issues-pr/alplabai/signex?color=purple" alt="PRs"></a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Tauri-v2-blue?logo=tauri&logoColor=white" alt="Tauri v2">
  <img src="https://img.shields.io/badge/React-19-61dafb?logo=react&logoColor=white" alt="React 19">
  <img src="https://img.shields.io/badge/Rust-stable-orange?logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/TypeScript-5.8-3178c6?logo=typescript&logoColor=white" alt="TypeScript">
  <img src="https://img.shields.io/badge/KiCad-8%2F9%2F10-blue" alt="KiCad">
  <img src="https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey" alt="Platform">
</p>

---

Signex is an open-source desktop EDA tool built for hardware engineers who want Altium Designer-class UX without the Altium price tag. It reads and writes KiCad files natively, so you can use existing KiCad libraries and schematics while enjoying a modern, fast editing experience.

> **Status:** Alpha. Schematic capture is fully functional with ~85% Altium feature parity. Signal AI copilot is integrated. PCB layout and simulation are next.

## Why Signex?

- **Altium-class UX** on an open-source foundation
- **KiCad file compatibility** (.kicad_sch, .kicad_sym read/write, KiCad 8/9/10)
- **Native desktop app** via Tauri v2 (Rust backend, React frontend) -- fast startup, low memory
- **Signal AI copilot** -- Claude-powered design assistant with streaming, tool use, and visual context
- **Built by hardware engineers** for hardware engineers

## Features

<details>
<summary><strong>Schematic Capture</strong> (click to expand)</summary>

- Canvas2D rendering with pan/zoom and auto-pan
- 226 KiCad symbol libraries with search, preview, and drag-and-drop
- Wire drawing (Manhattan, diagonal, free routing modes)
- Bus drawing and bus entry placement
- Component placement with auto-designator and Tab to edit properties
- Net labels (Net, Global, Hierarchical, Power) with all label shapes
- Sheet symbols with pin support and Ctrl+Double-Click navigation
- Drawing objects: line, rectangle, circle, arc, polyline, ellipse, round rect, polygon, text frame, image
- Line styles (solid, dash, dot, dash-dot) and arrow endpoints (open, closed, diamond)
- Text string placement with special string substitution (=Title, =Date, =Rev, etc.)
- Sheet templates (ISO A4, ANSI A built-in)
</details>

<details>
<summary><strong>Editing</strong></summary>

- Selection (click, Shift+click, box select with crossing/enclosing modes)
- Selection filter panel (per-type visibility and selectability)
- Selection memory (Ctrl+1-8 store, Alt+1-8 recall)
- Move with rubber-banding (Ctrl = move without rubber-band)
- Copy/Cut/Paste, Smart Paste (Shift+Ctrl+V), Paste Array
- Duplicate, Rotate, Mirror X/Y, Align (6 directions), Distribute H/V
- Undo/Redo (50 levels), Bring to Front / Send to Back
- Nudge (Ctrl+Arrow, Shift+Ctrl+Arrow for 10x)
- Break Wire, Align to Grid (Shift+Ctrl+D)
- In-place text editing (double-click or F2)
- Find/Replace with regex, Find Similar Objects (Shift+F)
- Group/Union for collective selection
- Right-click context menu with full depth
</details>

<details>
<summary><strong>Validation & Output</strong></summary>

- ERC: 11 violation types + 12x12 pin connection matrix
- Configurable ERC severity per violation type
- No ERC directives to suppress individual violations
- ERC HTML report export
- Annotation dialog with 4 ordering modes, preview table, designator lock, multi-part matching
- BOM export: CSV, TSV, HTML, Excel (SpreadsheetML)
- Netlist export: KiCad S-expression, generic XML
- PDF export: single sheet or multi-sheet, DPI/color/grid options
- PNG export
- Print support (Ctrl+P)
- Output Jobs panel: configure, run, batch execute
- Net Color Override (F5)
- AutoFocus: dim unrelated objects during inspection
</details>

<details>
<summary><strong>Library Editor</strong></summary>

- Symbol editor canvas with grid, origin cross, zoom/pan
- Add/edit/remove pins with auto-increment numbering
- Add/edit/remove graphics (rect, polyline, circle, arc)
- Hidden pin support for power connections
- Multi-part component types (LibSymbolUnit)
- DeMorgan alternate display mode toggle
- Save to native .sxsym format, read .kicad_sym
- New/Edit/Duplicate from Components panel
</details>

<details>
<summary><strong>Advanced Features</strong></summary>

- Net classes with add/remove/assign
- Differential pairs (_P/_N naming)
- Signal harnesses with nested members
- Design Constraint Manager (clearance, trace width, via size, diff pair gap, length match)
- Design variants (fitted/not-fitted/alternate)
- Document and project parameters with hierarchy resolution
- Parameter Manager (spreadsheet editing across all components)
- Multi-channel design (Repeat keyword on sheet symbols)
- Preferences dialog with grid, snap, ERC severity, template, net scope settings
</details>

<details>
<summary><strong>Signal AI Copilot</strong></summary>

- Claude API integration via Rust reqwest with streaming SSE
- Chat panel with markdown rendering (bold, code blocks, headers, lists)
- Model selection: Sonnet 4 (fast) / Opus 4 (deep analysis)
- Rich schematic context: component list, net connectivity, ERC details sent to Claude
- Visual context: schematic screenshot sent to Claude vision
- Tool use: Claude can add components, draw wires, set values, place labels, run ERC
- 6 circuit templates: LDO, Decoupling, Pull-ups, Op-Amp Buffer, RC Filter, Power Header
- Design Brief: persistent design intent description
- BOM optimization analysis
- ERC fix suggestions (one-click from Messages panel)
- Inline copilot suggestions on hover
- Session cost tracking with per-model pricing
- Export chat as markdown
- Ctrl+Shift+A to open Signal panel
</details>

<details>
<summary><strong>Panels (Altium-style)</strong></summary>

- **Properties** -- context-aware editing for all object types
- **Components** -- 226 KiCad libraries with search, preview, edit, drag-and-drop
- **SCH Filter** -- toggle visibility/selectability per object type (connected to renderer + hit test)
- **SCH List** -- sortable tables with editable cells and resolved nets tab
- **Navigator** -- schematic overview with object tree
- **Messages** -- ERC violations with click-to-focus and HTML report export
- **Output Jobs** -- BOM, Netlist, PDF, PNG job management
- **Projects** -- project tree with sheet navigation
- **Signal** -- AI chat with streaming, tool use, design review, ERC fix
- All panels tabbed: Left (Projects/Components/Nav), Right (Props/Filter/List), Bottom (Messages/Output Jobs/Signal)
</details>

## Roadmap

| Phase | Status | Features |
|-------|--------|----------|
| **Phase 0: Viewer** | Done | KiCad parser, Canvas2D renderer, symbol transforms, multi-sheet nav |
| **Phase 1: Editor** | Done | Selection, move, wire, delete, rotate, undo/redo, save, properties |
| **Phase 2: Core Editing** | Done | Drag-box select, auto-junction, rubber-band, copy/paste, net labels, power ports, ERC, BOM |
| **Phase 3: Validation** | Done | ERC (11 checks + connection matrix), annotation, No ERC directives, AutoFocus |
| **Phase 4: Advanced** | Done | Library editor, PDF/print export, output jobs, custom fields, title block, templates |
| **Phase 4+: Altium Parity** | Done | 40+ features: selection filter, drawing tools, net classes, diff pairs, harnesses, constraints, design variants, parameter manager, multi-channel, BOM formats |
| **Phase 5: Signal AI** | Done | Claude API with streaming, tool use, visual context, circuit templates, design review |
| **Phase 6: PCB Layout** | Next | WebGL2 renderer, layer stack, interactive routing, DRC, copper pour, 3D viewer |
| **Phase 7: Simulation** | Planned | SPICE integration, signal integrity, power analysis |
| **Phase 8: Manufacturing** | Planned | Gerber/drill export, assembly drawings, pick-and-place |

See [docs/feature-roadmap.md](docs/feature-roadmap.md) for the detailed roadmap.

## Quick Start

### Prerequisites

- [Node.js](https://nodejs.org/) 18+
- [Rust](https://www.rust-lang.org/tools/install) (stable)
- **Windows:** Visual Studio Build Tools 2022 (C++ workload)
- **macOS:** `xcode-select --install`
- **Linux:** `sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev`

### Install & Run

```bash
git clone https://github.com/alplabai/signex.git
cd signex
npm install
npm run tauri dev
```

### First Steps

1. **Open** a KiCad project: File > Open Project (Ctrl+O) -- select a `.kicad_pro` file
2. **Navigate** -- click a schematic sheet in the Projects panel
3. **Edit** -- W=wire, L=label, T=text, B=bus, Space=rotate, Del=delete
4. **Save** -- Ctrl+S writes back to KiCad format

## Keyboard Shortcuts

<details>
<summary>Full shortcut table</summary>

| Shortcut | Action |
|----------|--------|
| `W` | Draw wire |
| `B` | Draw bus |
| `L` | Place net label |
| `T` | Place text |
| `P` | Open components panel |
| `Space` | Rotate |
| `X` / `Y` | Flip horizontal / vertical |
| `Ctrl+C/X/V` | Copy / Cut / Paste |
| `Shift+Ctrl+V` | Smart Paste |
| `Ctrl+D` | Duplicate |
| `Shift+Ctrl+D` | Align to Grid |
| `Ctrl+Z/Y` | Undo / Redo |
| `Ctrl+A` | Select all |
| `Ctrl+F/H` | Find / Replace |
| `Ctrl+P` | Print |
| `Ctrl+Q` | Toggle mm/mil/inch |
| `Ctrl+Arrow` | Nudge by grid |
| `Shift+Ctrl+Arrow` | Nudge by 10x grid |
| `Ctrl+1-8` | Store selection |
| `Alt+1-8` | Recall selection |
| `Shift+F` | Find similar objects |
| `F2` | Edit text in-place |
| `F5` | Net color override |
| `F11` | Properties panel |
| `G` | Cycle grid size |
| `Del` | Delete selected |
| `Esc` | Cancel / deselect |
| `Home` | Fit to view |
| `Tab` | Open properties panel |

</details>

## Tech Stack

| Layer | Technology |
|-------|------------|
| Desktop | [Tauri v2](https://tauri.app/) (Rust) |
| Frontend | React 19 + TypeScript + Vite 7 |
| Styling | Tailwind CSS 4 |
| Canvas | Canvas2D (schematic), WebGL2 (PCB, planned) |
| State | Zustand (7 stores: layout, project, editor, schematic, libraryEditor, outputJobs, signal) |
| Parser | Pure Rust S-expression parser |
| AI | Claude API via Rust reqwest (streaming SSE, tool use, vision) |

## Project Structure

```
src-tauri/src/
  commands/           Tauri IPC (project, schematic, save, library, export, signal)
  engine/             KiCad S-expr parser, writer, document model
src/
  canvas/             SchematicRenderer, LibraryEditorCanvas, hitTest
  components/         MenuBar, ToolbarStrip, StatusBar, dialogs (17 files)
  panels/             Properties, Components, Messages, Signal, Filter, List, Navigator, OutputJobs (9 files)
  stores/             Zustand: layout, project, editor, schematic, libraryEditor, outputJobs, signal
  lib/                Net resolver, ERC, geometry, PDF export, BOM formats, Signal AI context/tools/templates
  __tests__/          Vitest test suite
docs/                 Roadmap, master plan, Altium reference
```

## Contributing

We welcome contributions! See [CONTRIBUTING.md](CONTRIBUTING.md) for the full guide.

1. **Fork** the repo
2. **Branch from `dev`**: `git checkout dev && git checkout -b feature/my-feature`
3. **Code** and add tests
4. **Test**: `npm run test` + `cd src-tauri && cargo test`
5. **PR against `dev`** with a clear description

> **Branches:** `main` = stable releases, `dev` = active development. All PRs target `dev`.

## License

[GPL-3.0](LICENSE) -- compatible with KiCad's license.

## Disclaimer

> [!CAUTION]
> Signex is in **alpha**. It may have bugs that affect your schematic files. **Always keep backups.**

- Not affiliated with Altium, KiCad, or any other EDA vendor
- KiCad file format compatibility is best-effort; not all features supported yet
- Signal AI requires an Anthropic API key (stored in memory only, never saved to disk)

## Credits

Built by [Caner Alp](https://github.com/alpCaner) at [Alp Lab AB](https://alplab.ai).

---

<p align="center">
  <strong>Signex</strong>: Signal + Nexus<br>
  <em>Connecting hardware engineers to better design tools.</em>
</p>
