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
  - Native: `.snxsch` (schematic), `.snxpcb` (PCB), `.snxsym` (symbol library), `.snxprj` (project)
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

## Phase 2: Core Schematic Editing (DONE)

**Goal:** Reach Altium parity for daily schematic capture work.

### Wiring & Connectivity
- [x] Wire routing modes: 90°, 45°, any angle (Shift+Space to cycle)
- [x] Backspace to remove last wire vertex during placement
- [x] Rubber-banding (wires stretch when dragging connected components)
- [x] Wire segment editing (drag vertex, drag segment)
- [x] Bus placement (P,B) with Data[0..7] naming syntax
- [x] Bus entry placement
- [x] Net label placement (P,L) with auto-increment
- [x] Power port placement with style selector (VCC, GND, Arrow, Bar, etc.)
- [x] No-connect marker (P,X)
- [x] Port placement for multi-sheet connectivity

### Editing Operations
- [x] Copy/Paste (Ctrl+C/V) with paste anchor point
- [x] Smart Paste (Shift+Ctrl+V) — arrays, transform types
- [x] Ctrl+Arrow nudge selection by grid
- [x] In-place text editing (F2 or click-pause-click)
- [x] Tab during placement to edit properties before placing
- [x] Auto-increment designators (R1, R2, R3...)
- [x] Find Similar Objects (Shift+F)
- [x] Alignment tools (Shift+Ctrl+L/R/T/B)
- [x] Distribute evenly (Shift+Ctrl+H/D)
- [x] Bring to front / Send to back
- [x] Multi-part component support (unit selector)
- [x] Ctrl+Q toggle mm/mil units

### Multi-Sheet
- [x] Sheet symbol placement
- [x] Sheet entry / port connectivity
- [x] Ctrl+Double-Click jump between entry/port
- [x] Hierarchical net scope modes (Global, Flat, Hierarchical, Strict)
- [x] Create sheet from sheet symbol

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
- [x] No ERC directives to suppress individual violations
- [x] AutoFocus: dim unrelated wiring when inspecting violation

### Annotation
- [x] Auto-annotate schematic (processing order: top-to-bottom, left-to-right)
- [x] Reset designators
- [x] Reset duplicates only
- [x] Per-sheet annotation control
- [x] Lock/unlock individual designators
- [x] Back-annotation from PCB

### Cross-Reference
- [x] Alt+Click to highlight entire net across all sheets
- [x] Net color override (F5)
- [x] Port cross-references showing sheet/grid location
- [x] Component cross-references

---

## Phase 4: Library & Output (DONE)

**Goal:** Full library management and professional output generation.

### Library Management
- [x] Library manager panel
- [x] Create/edit schematic symbols (.snxsym)
- [x] Footprint assignment and management
- [x] Library search with parametric filtering
- [x] Component comparison (side-by-side diff)
- [ ] Part Choices with supplier data integration

### Drawing Objects
- [x] Text string / Text frame / Note
- [x] Line, Arc, Bezier, Rectangle, Polygon
- [x] Image placement
- [ ] Dimension annotations

### Output Generation
- [x] BOM generation (configurable columns, grouping)
- [x] Netlist export (KiCad S-expression, generic XML)
- [x] PDF schematic export (single/multi-sheet, DPI, color/mono)
- [x] Print support with page setup
- [x] Output Jobs configuration

### Properties Panel Enhancements
- [x] Batch editing of multiple selected objects
- [x] Document properties when nothing selected (grid, page, template)
- [x] Full parameter editing
- [x] Model assignment (footprint, simulation)

---

## Phase 5: PCB Layout — DONE

**Goal:** Full PCB editor with interactive routing and design rule checking.

### Canvas & Rendering
- [x] Canvas2D renderer with layer-ordered rendering
- [x] WebGL2 renderer framework (shaders, instancing, camera)
- [x] 32 copper layers + full tech layer stack with Altium naming
- [x] Layer stack panel with visibility toggles
- [x] Component placement (move, rotate, flip, push/shove)
- [x] Ratsnest visualization (MST-based)
- [x] Copper zone rendering with filled polygons
- [x] 3D viewer (Three.js: board, pads, traces, vias, components)
- [x] Board cross-section stackup visualization
- [x] Single layer mode, board flip, net colors

### Routing
- [x] Interactive routing (walkaround, push/shove, ignore)
- [x] Corner styles (45, 90, arc45, arc90, any angle)
- [x] Differential pair routing with gap control
- [x] Length tuning (meander patterns)
- [x] Multi-track bus routing
- [x] BGA fanout (dog-bone escape)
- [x] Teardrop generation
- [x] Via stitching (grid + fence)
- [x] Online DRC during routing

### Design Rules
- [x] DRC engine: 15 check types
- [x] Clearance (trace-to-trace, trace-to-pad, via-to-pad, via-to-trace)
- [x] Width rules, via size rules, drill size rules
- [x] Annular ring, hole-to-hole, solder mask sliver
- [x] Short circuit detection, board outline clearance
- [x] Silk-to-mask clearance

### Board Features
- [x] Board outline editor
- [x] Copper pour with polygon clipping + thermal relief
- [x] Polygon boolean operations (clip, subtract, offset)
- [x] Dead copper removal
- [x] Keepout zone support

### Cross-Probing & Output
- [x] Bidirectional cross-probing (schematic ↔ PCB)
- [x] Cross-select mode (auto-sync selections)
- [x] Forward annotation (schematic → PCB netlist import)
- [x] Back annotation / ECO (PCB → schematic change detection + apply)
- [x] Gerber RS-274X + X2, Excellon drill, ODB++, STEP, IPC-2581
- [x] PCB PDF, pick-and-place, assembly SVG

---

## Phase 6: 3D Viewer — DONE (integrated into PCB)

- [x] Three.js 3D canvas with orbit/zoom/pan
- [x] Board body from outline polygon
- [x] Component body placeholders
- [x] Pads, traces, vias rendered in 3D
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

## Phase 8: Signal AI (DONE)

**Goal:** AI-powered design assistance that understands electronics.

### Signal Chat
- [x] Claude API integration via Rust reqwest with streaming SSE
- [x] Chat panel with markdown rendering and model selection (Sonnet 4 / Opus 4)
- [x] Context-aware: Signal sees current schematic, selection, ERC results
- [x] Visual context: schematic screenshot sent to Claude vision

### Signal Tools
- [x] Component suggestion based on circuit context
- [x] ERC fix suggestions with one-click apply
- [ ] Auto-routing assistance
- [x] Design review analysis (best practices, common mistakes)
- [ ] Datasheet Q&A (parse PDF datasheets, answer pin questions)
- [x] BOM optimization (suggest alternatives, check availability)

### Signal Automation
- [x] Natural language to schematic operations (tool use: add_component, add_wire, set_value, add_label, run_erc)
- [x] Circuit template generation (6 templates: LDO, Decoupling, Pull-ups, Op-Amp, RC Filter, Power Header)
- [x] Design intent documentation (Design Brief feature)
- [x] Session cost tracking with per-model pricing
- [x] Export chat as markdown

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

- [ ] Native file format: .snxsch, .snxpcb, .snxsym, .snxprj
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
| File extensions | .snxsch/.snxpcb/.snxprj | .kicad_sch/.kicad_pcb | Brand identity (Signex = SNX), format independence |
| State management | Zustand | Redux, Jotai, Context | Minimal boilerplate, great with React 19 |
| Desktop framework | Tauri v2 | Electron, Qt | Native performance, small binary, Rust backend |
| Wire cursor | Ref (not Zustand) | Zustand state | Avoids 60Hz state churn during mouse move |
| Undo cloning | structuredClone | JSON roundtrip | Native API, faster than JSON.parse/stringify |
