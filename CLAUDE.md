# Alp EDA — AI-First Electronic Design Automation

## Project Overview
Desktop EDA tool built on KiCad's open-source core with Altium Designer-class UX.
Target: schematic capture, PCB layout, 3D viewer, SI simulation, AI copilot.

## Architecture
- **Desktop shell:** Tauri v2 (Rust backend)
- **Frontend:** React 19 + TypeScript + Vite + Tailwind CSS 4
- **Canvas:** wgpu (GPU-accelerated, Rust plugin) — placeholder in Phase 0
- **3D:** Three.js (future)
- **Core engine:** KiCad C++ fork as headless lib via FFI (future)
- **AI:** Claude API via Rust reqwest client
- **State:** Zustand (7 stores: layout, project, editor, selection, ai, collab, prefs)

## Project Structure
```
src-tauri/src/          Rust backend
  commands/             Tauri IPC commands
  engine/               KiCad parser + document model
  renderer/             wgpu rendering (future)
  ai/                   Claude API client (future)
src/                    React frontend
  components/           Shell: MenuBar, ToolbarStrip, TabBar, StatusBar
  panels/               Dockable panels: Project, Properties, Messages
  canvas/               Central editor canvas
  editors/              Schematic, PCB, Library editors (future)
  stores/               Zustand state stores
  hooks/                React hooks (useTauriCommand, etc.)
  types/                TypeScript type definitions
  lib/                  Utilities (cn, etc.)
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
- Altium-compatible keyboard shortcuts where possible
- GPL-3.0 license (KiCad derivative)

## Phase Status
- [x] Phase 0 Week 1: Tauri + React scaffold, UI shell
- [ ] Phase 0 Week 2: KiCad S-expression parser
- [ ] Phase 0 Week 3: wgpu canvas basics
- [ ] Phase 0 Week 4: Real schematic rendering
