# Signex — Master Plan

## Vision
AI-First Electronic Design Automation tool that replaces Altium Designer.
Built on KiCad's open-source foundation with Altium-class UX, native AI integration,
and cloud collaboration. Designed for professional hardware engineers who need
full-featured schematic capture, PCB layout, 3D visualization, and SI simulation.

## Target Users
- Hardware engineers currently using Altium Designer
- Alp Lab internal use for E1M SoM designs
- Open-source EDA community (GPL-3.0 core)

## Architecture

### Technology Stack
- **Desktop shell:** Tauri v2 (Rust backend, WebView2 frontend)
- **Frontend:** React 19 + TypeScript + Vite + Tailwind CSS 4
- **Schematic canvas:** Canvas2D (Phase 0–2), migrate to wgpu for PCB
- **PCB canvas:** wgpu (GPU-accelerated rendering via Rust plugin)
- **3D viewer:** Three.js with STEP/VRML model import
- **Core engine:** KiCad C++ fork as headless library via FFI (long-term)
- **Parser:** Pure Rust S-expression parser (current, handles KiCad format)
- **AI:** Claude API via Rust reqwest client — branded **"Signal"**
- **State management:** Zustand stores
  - layout — panel sizes, collapse state (persisted to localStorage)
  - project — open project, tabs, recent files
  - editor — mode, grid, snap, units, status bar
  - schematic — document data, selection, undo/redo, edit mode
  - pcb — PCB document, layers, routing state (future)
  - ai — Signal chat history, tool calls (future)
  - collab — real-time collaboration state (future)
- **File formats:**
  - Native: `.alpsch` (schematic), `.alppcb` (PCB), `.alplib` (library), `.alpproj` (project), `.alprules` (design rules), `.alpout` (output jobs)
  - Import/Export: KiCad (.kicad_sch, .kicad_pcb), Altium (future), Eagle (future)
- **License:** GPL-3.0 (KiCad derivative core) + proprietary cloud/AI (separate repos)

---

## Phase 0: Schematic Viewer (DONE)

**Goal:** Open and render KiCad schematics with pixel-accurate symbol graphics.

### Week 1: Desktop Scaffold
- [x] Tauri v2 + React 19 + Vite + Tailwind CSS 4
- [x] Dark theme (Catppuccin Mocha-inspired palette)
- [x] Altium-like UI shell: MenuBar, ToolbarStrip, DocumentTabBar, StatusBar
- [x] Collapsible dock panels: Projects, Properties, Messages
- [x] Central canvas placeholder
- [x] Rust backend: commands/, engine/ module structure
- [x] GitHub Actions CI for Windows, macOS, Linux

### Week 2: KiCad Parser
- [x] Generic S-expression tokenizer and tree parser (sexpr.rs)
- [x] Schematic parser: symbols, wires, junctions, labels, child sheets
- [x] Library symbol parser: graphics (polyline, rectangle, circle, arc), pins
- [x] Recursive child sheet loading
- [x] Project file parser (.kicad_pro)

### Week 3: Canvas Renderer
- [x] Canvas2D schematic renderer with pan/zoom
- [x] Altium mouse controls: scroll=zoom, right-drag=pan
- [x] Multi-sheet navigation via document tabs
- [x] Paper size rendering with grid overlay

### Week 4: Symbol Rendering
- [x] Symbol transform: Y-flip + negated rotation + post-rotation mirror
- [x] Property text: position, rotation, font_size, justify from schematic data
- [x] Text normalization: 180°→0°, 270°→90° with flipped justify
- [x] Pin rendering with visibility flags (pin_numbers, pin_names)
- [x] Global label shapes (input/output/bidirectional arrows)
- [x] Text notes, dashed rectangles, child sheet boxes
- [x] Power symbol rendering (always horizontal value text)

---

## Phase 1: Schematic Editor Foundation (DONE)

**Goal:** Transform the viewer into an interactive editor with core editing operations.

- [x] Hit testing (symbols, wires, junctions, labels)
- [x] Selection: click, shift-click, drag-box (inside + crossing modes)
- [x] Move: drag selected objects with grid snap
- [x] Wire drawing: Manhattan routing, live preview, click-to-place segments
- [x] Delete selected objects
- [x] Rotate selected (R key)
- [x] Undo/Redo (Ctrl+Z/Y, 50-level stack)
- [x] Save schematic (.kicad_sch S-expression writer)
- [x] Properties panel (view selected object: ref, value, position, rotation)
- [x] Toolbar with mode buttons (Select, Wire, Component)
- [x] Signal panel stub (AI foundation)
- [x] Grid/snap toggle (G / Shift+G), dynamic grid size
- [x] Component library browser (search 226 KiCad libs, preview, place)
- [x] Placement mode (ghost preview, R/X/Y rotate/mirror, auto-designator)
- [x] Auto-junction at T-intersections
- [x] Electrical grid snap (cursor snaps to nearby pins/wire endpoints)
- [x] Altium-style floating canvas toolbar (Active Bar)
- [x] Menu actions wired (Save, Undo, Redo, Delete, Grid, Snap, Wire, Component)
- [x] Security: path traversal prevention in all Tauri file commands

---

## Phase 2: Core Schematic Editing

**Goal:** Reach Altium parity for daily schematic capture work.

### Wiring & Connectivity
- [ ] Wire routing modes: 90°, 45°, any angle (Shift+Space to cycle)
- [ ] Backspace to remove last wire vertex during placement
- [ ] Rubber-banding (wires stretch when dragging connected components)
- [ ] Wire segment editing (drag vertex, drag segment)
- [ ] Bus placement (P,B) with Data[0..7] naming syntax
- [ ] Bus entry placement
- [ ] Net label placement (P,L) with auto-increment
- [ ] Power port placement with style selector (VCC, GND, Arrow, Bar, etc.)
- [ ] No-connect marker (P,X)
- [ ] Port placement for multi-sheet connectivity

### Editing Operations
- [ ] Copy/Paste (Ctrl+C/V) with paste anchor point
- [ ] Smart Paste (Shift+Ctrl+V) — arrays, transform types
- [ ] Ctrl+Arrow nudge selection by grid
- [ ] In-place text editing (F2 or click-pause-click)
- [ ] Tab during placement to edit properties before placing
- [ ] Auto-increment designators (R1, R2, R3...)
- [ ] Find Similar Objects (Shift+F)
- [ ] Alignment tools (Shift+Ctrl+L/R/T/B)
- [ ] Distribute evenly (Shift+Ctrl+H/D)
- [ ] Bring to front / Send to back
- [ ] Multi-part component support (unit selector)
- [ ] Ctrl+Q toggle mm/mil units

### Multi-Sheet
- [ ] Sheet symbol placement
- [ ] Sheet entry / port connectivity
- [ ] Ctrl+Double-Click jump between entry/port
- [ ] Hierarchical net scope modes (Global, Flat, Hierarchical, Strict)
- [ ] Create sheet from sheet symbol

---

## Phase 3: Design Validation (DONE)

**Goal:** Ensure designs are electrically correct before moving to PCB.

### ERC (Electrical Rules Check)
- [x] Connection matrix (pin-to-pin validity rules — 12x12 matrix)
- [x] Duplicate designator detection
- [x] Unconnected pin detection
- [x] Missing power net detection
- [x] Floating wire detection
- [x] ERC markers with severity levels (Error, Warning)
- [x] Messages panel with violation navigation (click to select)
- [x] Pin conflict detection (output-to-output, output-to-tristate, etc.)
- [x] Power pin not driven check
- [x] Net with no label warning
- [x] Unannotated component detection
- [x] Multiple conflicting net names on same net
- [ ] No ERC directives to suppress individual violations
- [ ] AutoFocus: dim unrelated wiring when inspecting violation

### Annotation
- [x] Auto-annotate schematic (processing order: top-to-bottom, left-to-right)
- [x] Reset designators
- [x] Reset duplicates only
- [ ] Per-sheet annotation control
- [ ] Lock/unlock individual designators
- [ ] Back-annotation from PCB

### Cross-Reference
- [x] Alt+Click to highlight entire net across all sheets
- [x] Net color override (F5)
- [ ] Port cross-references showing sheet/grid location
- [ ] Component cross-references

---

## Phase 4: Library & Output

**Goal:** Full library management and professional output generation.

### Library Management
- [ ] Library manager panel
- [ ] Create/edit schematic symbols (.alplib)
- [ ] Footprint assignment and management
- [ ] Library search with parametric filtering
- [ ] Component comparison (side-by-side diff)
- [ ] Part Choices with supplier data integration

### Drawing Objects
- [ ] Text string / Text frame / Note
- [ ] Line, Arc, Bezier, Rectangle, Polygon
- [ ] Image placement
- [ ] Dimension annotations

### Output Generation
- [ ] BOM generation (configurable columns, grouping)
- [ ] Netlist export (KiCad, Altium, generic)
- [ ] PDF schematic export
- [ ] Print support with page setup
- [ ] Output Jobs configuration (.alpout)

### Properties Panel Enhancements
- [ ] Batch editing of multiple selected objects
- [ ] Document properties when nothing selected (grid, page, template)
- [ ] Full parameter editing
- [ ] Model assignment (footprint, simulation)

---

## Phase 5: PCB Layout

**Goal:** Full PCB editor with interactive routing and design rule checking.

### Canvas & Rendering
- [ ] wgpu GPU-accelerated canvas (Rust plugin)
- [ ] Layer stack manager (copper, silkscreen, mask, paste, mechanical)
- [ ] Component placement from netlist
- [ ] Ratsnest visualization
- [ ] Copper zone rendering

### Routing
- [ ] Interactive routing with clearance enforcement
- [ ] Differential pair routing
- [ ] Length tuning (meander)
- [ ] Fanout generator
- [ ] Teardrop generation
- [ ] Push-and-shove routing

### Design Rules
- [ ] DRC (Design Rule Check)
- [ ] Clearance rules (net class, component, area)
- [ ] Width rules
- [ ] Via rules
- [ ] Plane rules
- [ ] Manufacturing rules (minimum annular ring, drill sizes)

### Board Features
- [ ] Board outline editor
- [ ] Keepout regions
- [ ] Copper pour (polygon fill with thermal relief)
- [ ] Drill table
- [ ] Stackup configuration

### Cross-Probing
- [ ] Shift+Ctrl+X toggle cross-select between schematic and PCB
- [ ] Click component in schematic → highlight in PCB
- [ ] Forward annotation (schematic changes → PCB update)
- [ ] Back annotation (PCB changes → schematic update)
- [ ] ECO (Engineering Change Order) dialog

---

## Phase 6: 3D Viewer

**Goal:** Visualize the assembled PCB in 3D.

- [ ] Three.js 3D canvas
- [ ] STEP model import for components
- [ ] VRML model import
- [ ] Board with copper, silkscreen, mask layers
- [ ] Component placement visualization
- [ ] Cross-probing: click 3D component → select in schematic/PCB
- [ ] Export STEP assembly for mechanical integration
- [ ] Collision detection
- [ ] Measurement tools

---

## Phase 7: Simulation

**Goal:** Integrated SPICE simulation for analog/mixed-signal verification.

- [ ] SPICE netlist generation from schematic
- [ ] ngspice integration as simulation engine
- [ ] Simulation probes on schematic
- [ ] Waveform viewer panel
- [ ] AC/DC/Transient/Noise analysis
- [ ] Parameter sweeps
- [ ] Monte Carlo analysis
- [ ] Simulation models library

---

## Phase 8: Signal (AI Integration)

**Goal:** AI-powered design assistance that understands electronics.

### Signal Chat
- [ ] Claude API integration via Rust reqwest
- [ ] Chat panel with markdown rendering
- [ ] Context-aware: Signal sees current schematic, selection, ERC results

### Signal Tools
- [ ] Component suggestion based on circuit context
- [ ] ERC fix suggestions with one-click apply
- [ ] Auto-routing assistance
- [ ] Design review analysis (best practices, common mistakes)
- [ ] Datasheet Q&A (parse PDF datasheets, answer pin questions)
- [ ] BOM optimization (suggest alternatives, check availability)

### Signal Automation
- [ ] Natural language to schematic operations ("add a 10k pullup on SDA")
- [ ] Circuit template generation ("create an LDO circuit for 3.3V 500mA")
- [ ] Design intent documentation (auto-generate design notes)

---

## Phase 9: Cloud & Collaboration

**Goal:** Real-time multi-user collaboration and cloud project management.

- [ ] User accounts (Alp Lab cloud)
- [ ] Project hosting and versioning (Git-based)
- [ ] Real-time collaborative editing (CRDT)
- [ ] Design review workflow (comments, approvals)
- [ ] Component lifecycle management
- [ ] Supply chain integration (Digi-Key, Mouser, LCSC APIs)
- [ ] CI/CD for design verification (automated ERC/DRC on push)

---

## Phase 10: Native Format & Ecosystem

**Goal:** Establish Signex as an independent platform.

- [ ] Native file format: .alpsch, .alppcb, .alplib, .alpproj, .alprules, .alpout
- [ ] Import from: KiCad, Altium, Eagle, OrCAD
- [ ] Export to: KiCad, Gerber, ODB++, IPC-2581
- [ ] Plugin system (Rust + WASM)
- [ ] Marketplace for plugins and libraries
- [ ] Command palette (Ctrl+K) with fuzzy search
- [ ] Scripting console (TypeScript/Python)

---

## Architecture Decisions Log

| Decision | Chosen | Alternatives Considered | Rationale |
|----------|--------|------------------------|-----------|
| Canvas (schematic) | Canvas2D | wgpu, WebGL, SVG | Fastest iteration; wgpu for PCB phase |
| Parser | Pure Rust S-expr | KiCad C++ FFI, WASM | Simpler build, no C++ toolchain dependency |
| AI name | Signal | AI Copilot, Assistant | Domain-relevant, clean branding |
| File extensions | .alpsch/.alppcb | .kicad_sch/.kicad_pcb | Brand identity, format independence |
| State management | Zustand | Redux, Jotai, Context | Minimal boilerplate, great with React 19 |
| Desktop framework | Tauri v2 | Electron, Qt | Native performance, small binary, Rust backend |
| Wire cursor | Ref (not Zustand) | Zustand state | Avoids 60Hz state churn during mouse move |
| Undo cloning | structuredClone | JSON roundtrip | Native API, faster than JSON.parse/stringify |
