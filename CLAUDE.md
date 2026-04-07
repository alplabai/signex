# Signex — AI-First Electronic Design Automation

## Project Overview
Desktop EDA tool with Altium Designer-class UX.
Target: schematic capture, PCB layout, 3D viewer, SI simulation, AI copilot (Signal).

## Architecture
- **Desktop shell:** Tauri v2 (Rust backend)
- **Frontend:** React 19 + TypeScript + Vite + Tailwind CSS 4
- **Canvas:** Canvas2D (schematic rendering, hit testing, selection)
- **Parser:** Pure Rust S-expression parser for KiCad format (.kicad_sch, .kicad_sym)
- **3D:** Three.js (future)
- **AI:** Claude API via Rust reqwest client — branded "Signal"
- **State:** Zustand (4 stores: layout, project, editor, schematic)

## Project Structure
```
src-tauri/src/          Rust backend
  commands/             Tauri IPC commands (project, schematic, save, library)
  engine/               KiCad S-expr parser, document model, writer
    parser.rs           Schematic + symbol library parser
    sexpr.rs            Generic S-expression tokenizer
    writer.rs           KiCad S-expr serializer
    document.rs         Document model (future)
src/                    React frontend
  components/           Shell: MenuBar, ToolbarStrip, TabBar, StatusBar, ComponentSearch
  panels/               Dockable panels: Project, Properties, Messages, Signal
  canvas/               SchematicRenderer (Canvas2D), EditorCanvas, hitTest
  stores/               Zustand state: layout, project, editor, schematic
  hooks/                useResizable, useTauriCommand
  types/                TypeScript type definitions
  lib/                  Utilities (cn)
```

## Commands
- `npm run dev` — Vite dev server (frontend only)
- `npm run tauri dev` — Full Tauri dev (frontend + Rust)
- `npm run build` — Frontend production build
- `npx tsc --noEmit` — TypeScript check

## Conventions
- Dark theme (Catppuccin Mocha-inspired palette)
- 13px base font size (dense EDA UI)
- All panels collapsible, layout persisted to localStorage
- Altium-compatible keyboard shortcuts (see docs/altium-schematic-reference.md)
- KiCad file format compatibility (.kicad_sch read/write)
- Native format: .sxsch/.sxpcb/.sxproj (future)
- GPL-3.0 license (KiCad derivative)

## Phase Status
- [x] Phase 0: Viewer — KiCad parser, Canvas2D renderer, symbol transforms, multi-sheet nav
- [x] Phase 1: Editor foundation — selection, move, wire, delete, rotate, undo/redo, save, properties
- [x] Phase 2: Core editing — drag-box, auto-junction, rubber-band, copy/paste, labels, power ports, ERC
- [x] Phase 3: Validation — ERC 11 checks + pin matrix, annotation, AutoFocus, net color override
- [x] Phase 4: Advanced — library editor, PDF/print, output jobs, templates, title block, BOM formats
- [x] Phase 4+: Altium parity — 40+ features: selection filter, drawing tools, net classes, diff pairs, harnesses, constraints, variants, parameter manager, multi-channel
- [x] Phase 5: Signal AI — Claude API streaming, tool use, visual context, circuit templates, design review
- [ ] Phase 6: PCB layout — WebGL2 renderer, layer stack, routing, DRC, copper pour, 3D viewer

## Architecture Decisions
- Canvas2D for schematic (adequate for 1000+ components per sheet)
- WebGL2 planned for PCB (GPU-accelerated, handles 100k+ objects)
- GAL abstraction layer for future WebGPU/wgpu migration
- Pure Rust parser instead of KiCad C++ FFI — simpler build, no C++ toolchain dependency
- Wire cursor + placement cursor use refs (not Zustand) to avoid 60Hz state churn
- structuredClone for undo snapshots instead of JSON roundtrip
