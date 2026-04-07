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
- [x] Library editor (canvas, toolbar, pin/graphic CRUD, save to .sxsym)
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
- [ ] Claude API integration via Rust reqwest
- [ ] Natural language schematic assistance
- [ ] Component suggestion based on circuit context
- [ ] ERC fix suggestions
- [ ] Design review analysis
- [ ] Datasheet Q&A

### Phase 6: PCB Layout
- [ ] PCB canvas with wgpu (GPU-accelerated)
- [ ] Layer stack manager
- [ ] Component placement from netlist
- [ ] Interactive routing (single track, diff pair)
- [ ] DRC (Design Rule Check)
- [ ] Copper pour
- [ ] 3D viewer (Three.js)
- [ ] Cross-probing (Shift+Ctrl+X between schematic and PCB)
- [ ] Forward/back annotation

### Phase 7: Simulation
- [ ] SPICE netlist generation
- [ ] Mixed-signal circuit simulator
- [ ] Signal integrity analysis
- [ ] Power analysis

### Phase 8: Manufacturing Output
- [ ] Gerber (RS-274X) export
- [ ] NC Drill export
- [ ] ODB++ export
- [ ] Assembly drawings
- [ ] Pick-and-place files
- [ ] IPC-2581

### Phase 9: Collaboration
- [ ] Comment threads on schematic
- [ ] Schematic diff/comparison
- [ ] Git integration
- [ ] Real-time multi-user editing

### Phase 10: Ecosystem
- [ ] Plugin system
- [ ] Community library marketplace
- [ ] Design templates
- [ ] Cloud project storage
