# Signex Feature Roadmap

## Completed

### Phase 0: Viewer
- [x] KiCad S-expr parser (iterative, safe, KiCad 8/9/10)
- [x] Canvas2D renderer with pan/zoom
- [x] Symbol rendering (rotation, mirror, text, pins)
- [x] Multi-sheet navigation via tabs

### Phase 1: Editor Foundation
- [x] Selection (click, shift-click, box select)
- [x] Move with rubber-banding (Ctrl = no rubber-band)
- [x] Wire drawing (Manhattan, diagonal, free routing)
- [x] Delete, rotate, undo/redo (50 levels)
- [x] Save to KiCad format (Ctrl+S)
- [x] Properties panel (context-aware)
- [x] Grid/snap toggle, component library browser (226 libs)

### Phase 2: Core Editing
- [x] Drag-box select (crossing/enclosing modes)
- [x] Auto-junction at T-intersections
- [x] Electrical snap to pins/wire endpoints
- [x] Copy/cut/paste, duplicate
- [x] Net labels (Net, Global, Hierarchical, Power)
- [x] Power ports, no-connect markers, ports
- [x] Bus drawing, text notes, drawing objects
- [x] Context menu, align/distribute, z-ordering
- [x] In-place text editing (F2 / double-click)
- [x] Find/Replace with regex

### Phase 3: Validation
- [x] ERC: 11 violation types + 12x12 pin connection matrix
- [x] Configurable ERC severity per violation
- [x] No ERC directives
- [x] Auto-annotation with lock/unlock designators
- [x] Alt+Click net highlight, net color override (F5)
- [x] AutoFocus (dim unrelated objects)

### Phase 4: Advanced
- [x] Library editor (canvas, toolbar, pin/graphic CRUD, save to .snxsym)
- [x] PDF export (single/multi-sheet, DPI, color/mono)
- [x] Print support (Ctrl+P)
- [x] Output Jobs panel (BOM, Netlist, PDF, PNG)
- [x] Custom symbol fields with add/remove
- [x] Title block parsing, editing, rendering
- [x] Special string substitution (=Title, =Date, =CurrentDate, etc.)
- [x] Sheet templates (ISO A4, ANSI A built-in)
- [x] Configurable BOM (CSV, TSV, HTML, Excel)
- [x] Netlist export (KiCad S-expression, generic XML)
- [x] ERC HTML report export

### Phase 4+: Altium Parity
- [x] Selection filter (per-type visibility/selectability, connected to hitTest + renderer)
- [x] Drawing tools: circle, polyline, ellipse, round rect, polygon, text frame, image
- [x] Line styles (solid/dash/dot/dash_dot) + arrow endpoints (open/closed/diamond)
- [x] Auto-pan during placement/drawing
- [x] Smart Paste (Shift+Ctrl+V), Paste Array
- [x] Selection memory (Ctrl+1-8 / Alt+1-8)
- [x] Find Similar Objects (Shift+F)
- [x] Annotation dialog (4 ordering modes, preview, scope, multi-part matching)
- [x] Preferences dialog (grid, snap, ERC severity, templates, net scope)
- [x] Break Wire, Align to Grid (Shift+Ctrl+D)
- [x] Bus entry placement
- [x] Sheet symbol placement + Ctrl+Double-Click navigation
- [x] Net classes (add/remove/assign)
- [x] Net identifier scope (global/flat/hierarchical)
- [x] Differential pairs (_P/_N)
- [x] Signal harnesses (nested members)
- [x] Design Constraint Manager (clearance, trace width, via size, etc.)
- [x] Design variants (fitted/not-fitted/alternate)
- [x] Document + project parameters with hierarchy resolution
- [x] Parameter Manager (spreadsheet editing)
- [x] Multi-channel design (Repeat on sheet symbols)
- [x] Group/Union
- [x] Drag-and-drop from Components panel
- [x] Hidden pins, multi-part component types, DeMorgan modes
- [x] Navigation history + bookmarks
- [x] Nudge (Ctrl+Arrow), enhanced context menu

---

## Upcoming

### Phase 5: Signal AI
- [x] Claude API integration via Rust reqwest with streaming SSE
- [x] Chat panel with markdown rendering and model selection (Sonnet 4 / Opus 4)
- [x] Rich schematic context: component list, net connectivity, ERC details
- [x] Visual context: schematic screenshot sent to Claude vision
- [x] Tool use: add_component, add_wire, set_component_value, add_net_label, run_erc
- [x] 6 circuit templates (LDO, Decoupling, Pull-ups, Op-Amp, RC Filter, Power Header)
- [x] Design Brief: persistent design intent
- [x] Design review (one-click analysis)
- [x] ERC fix suggestions (from Messages panel)
- [x] BOM optimization prompt
- [x] Component suggestion prompt builder
- [x] Inline copilot suggestions (hover popover)
- [x] Session cost tracking with per-model pricing
- [x] Export chat as markdown
- [x] Ctrl+Shift+A shortcut

---

### Phase 6: PCB Layout
- [x] KiCad .kicad_pcb parser (footprints, pads, segments, vias, zones, nets)
- [x] Canvas2D renderer with layer-ordered rendering
- [x] WebGL2 renderer framework (shaders, instanced rendering, ready for integration)
- [x] 32 copper layers + full tech layer stack with Altium naming
- [x] Layer stack panel with visibility toggles and active layer selection
- [x] Component placement (move, rotate, flip with pad layer mirroring)
- [x] Push/shove placement (move overlapping components)
- [x] Netlist import from schematic (forward annotation)
- [x] Interactive routing (walkaround, push/shove, ignore modes)
- [x] Corner styles: 45, 90, arc45, arc90, any angle
- [x] Differential pair routing with gap control
- [x] Length tuning with meander patterns (trombone/sawtooth/accordion)
- [x] Multi-track bus routing (parallel traces)
- [x] BGA fanout with dog-bone escape routing
- [x] Via placement with auto-net detection
- [x] Via stitching (grid + fence patterns)
- [x] Teardrops for pad/via-to-trace transitions
- [x] DRC engine: 15 check types (clearance, width, via, annular ring, hole-to-hole, short circuit, solder mask sliver, silk-to-mask, trace-to-pad, via-to-pad, via-to-trace, board outline, unrouted, minimum drill)
- [x] Copper pour with polygon clipping, thermal relief, obstacle subtraction
- [x] Dead copper removal (remove isolated islands)
- [x] Ratsnest engine (MST-based, union-find connectivity)
- [x] Cross-probing (schematic ↔ PCB, bidirectional, with cross-select mode)
- [x] Back annotation / ECO (detect and apply PCB→schematic changes)
- [x] 3D viewer (Three.js: board body, pads, traces, vias, component bodies, orbit/zoom/pan)
- [x] Board cross-section stackup visualization
- [x] Single layer mode (Shift+S: off/hide/grayscale/monochrome)
- [x] Board flip view (Ctrl+F)
- [x] Net color override (F5)
- [x] PCB toolbar with all edit modes and layer selector
- [x] PCB context menus (right-click with context-aware actions)
- [x] PCB properties panel (board, footprint, segment, via details)
- [x] Gerber RS-274X export (7 layers)
- [x] Gerber X2 extended attributes (file function, polarity, net/pad attributes)
- [x] Excellon drill file export
- [x] ODB++ export (matrix, profile, layers, components, netlists)
- [x] STEP 3D export (board body + component placements)
- [x] IPC-2581 simplified XML export
- [x] PCB PDF export (multi-layer with traces, pads, vias)
- [x] Pick-and-place CSV export
- [x] Assembly drawing SVG export
- [x] Layer sets (7 presets + custom save/load)
- [x] Placement alignment and distribution tools
- [x] Footprint swap
- [x] Design variant management panel
- [x] Inspector panel (detailed object properties)
- [x] Snippets panel (save/reuse design fragments)

---

## Upcoming

### Phase 7: Simulation
- [ ] SPICE netlist generation
- [ ] Mixed-signal circuit simulator
- [ ] Signal integrity analysis
- [ ] Power analysis
- [ ] Impedance-controlled routing

### Phase 8: Collaboration
- [ ] Comment threads on schematic/PCB
- [ ] Schematic/PCB diff/comparison
- [ ] Git integration
- [ ] Real-time multi-user editing

### Phase 9: Ecosystem
- [ ] Plugin system (Rust + WASM)
- [ ] Community library marketplace
- [ ] Design templates
- [ ] Cloud project storage
- [ ] Auto-router
