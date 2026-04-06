# Alp EDA Feature Roadmap — Altium Parity

## Current Status (Phase 0+1)

### Done
- [x] KiCad S-expr parser with recursive sheet loading
- [x] Canvas2D renderer with pan/zoom
- [x] Symbol rendering (rotation, mirror, text, pins)
- [x] Multi-sheet navigation via tabs
- [x] Selection (click, shift-click)
- [x] Move (drag with snap to grid)
- [x] Wire drawing (Manhattan routing, live preview)
- [x] Delete selected
- [x] Rotate selected (R)
- [x] Undo/Redo (Ctrl+Z/Y)
- [x] Save to KiCad format (Ctrl+S)
- [x] Properties panel (view selected object)
- [x] Grid toggle (G) / Snap toggle (Shift+G)
- [x] Component library browser (search, preview, place)
- [x] Placement mode (ghost preview, R/X/Y rotate/mirror)
- [x] Menu actions wired
- [x] Signal panel stub (AI)
- [x] Path traversal security in all Tauri commands

---

## Phase 2: Core Editing (Altium Essentials)

### Wire Routing Improvements
- [ ] Wire routing modes: 90, 45, Any Angle (Shift+Space to cycle)
- [ ] Backspace to remove last wire vertex during placement
- [ ] Auto-junction at T-intersections
- [ ] Electrical grid snap (cursor snaps to nearby pins)
- [ ] Connection markers (visual indicator at valid hotspots)
- [ ] Rubber-banding (wires stretch when moving connected components)
- [ ] Wire segment editing (drag vertex, drag segment)

### Selection & Editing
- [ ] Drag box selection (rubber band)
- [ ] Copy/Paste (Ctrl+C/V) with paste anchor
- [ ] Ctrl+Arrow nudge selection by grid
- [ ] In-place text editing (F2 or click-pause-click)
- [ ] Smart Paste (Shift+Ctrl+V) — arrays, transform types
- [ ] Find Similar Objects (Shift+F)
- [ ] Alignment tools (Shift+Ctrl+L/R/T/B)
- [ ] Distribute evenly (Shift+Ctrl+H/D)
- [ ] Bring to front / Send to back

### Net Labels & Power Ports
- [ ] Place net label (P,L) with auto-increment
- [ ] Net label in-place editing
- [ ] Place power port (P,P) with style selector (VCC, GND, etc.)
- [ ] Power port global connectivity
- [ ] No-connect marker (P,X)
- [ ] Bus placement (P,B) with Data[0..7] syntax
- [ ] Bus entry placement

### Component Editing
- [ ] Tab to edit properties during placement
- [ ] Auto-increment designators (R1, R2, R3...)
- [ ] Editable properties in Properties panel (designator, value, footprint)
- [ ] Multi-part component support (unit selector)
- [ ] Ctrl+Q toggle mm/mil units

### Multi-Sheet
- [ ] Sheet symbol placement
- [ ] Sheet entry / port connectivity
- [ ] Ctrl+Double-Click jump between entry/port
- [ ] Hierarchical net scope modes
- [ ] Create sheet from sheet symbol

---

## Phase 3: Validation & Annotation

### ERC (Electrical Rules Check)
- [ ] Connection matrix (pin-to-pin validity)
- [ ] Duplicate designator check
- [ ] Unconnected pin detection
- [ ] Missing power net detection
- [ ] Floating wire detection
- [ ] ERC markers with severity levels
- [ ] Messages panel with violation navigation (double-click to zoom)
- [ ] No ERC directives to suppress

### Annotation
- [ ] Auto-annotate schematic (processing order options)
- [ ] Reset designators
- [ ] Reset duplicates only
- [ ] Lock/unlock individual designators
- [ ] Per-sheet annotation control

### Cross-Reference
- [ ] Port cross-references showing sheet/grid location
- [ ] Component cross-references
- [ ] Alt+Click to highlight entire net across sheets

---

## Phase 4: Advanced Features

### Properties Panel Enhancements
- [ ] Batch editing of multiple selected objects
- [ ] Document properties when nothing selected (grid, page, template)
- [ ] Parameter editing
- [ ] Model assignment (footprint, simulation)

### Drawing Objects
- [ ] Text string / Text frame / Note
- [ ] Line, Arc, Rectangle, Polygon
- [ ] Image placement

### Library Management
- [ ] Library manager panel
- [ ] Create/edit library symbols
- [ ] Footprint assignment
- [ ] Library search with parametric filtering
- [ ] Component comparison

### Output
- [ ] BOM generation
- [ ] Netlist export
- [ ] PDF schematic export
- [ ] Print support

---

## Phase 5: AI Integration (Signal)

### Signal AI Features
- [ ] Natural language schematic assistance
- [ ] Component suggestion based on circuit context
- [ ] ERC fix suggestions
- [ ] Auto-routing assistance
- [ ] Design review analysis
- [ ] Datasheet Q&A

---

## Phase 6: PCB Layout

### PCB Editor
- [ ] PCB canvas with layer stack
- [ ] Component placement from netlist
- [ ] Interactive routing
- [ ] DRC (Design Rule Check)
- [ ] Copper pour
- [ ] 3D viewer

### Cross-Probing
- [ ] Shift+Ctrl+X toggle cross-select
- [ ] Click component in schematic → highlight in PCB
- [ ] Forward/back annotation

---

## Priority Matrix (Next Session)

| Feature | Impact | Effort | Priority |
|---------|--------|--------|----------|
| Drag box selection | High | Low | P0 |
| Auto-junction | High | Med | P0 |
| Electrical grid snap | High | Med | P0 |
| Rubber-banding | High | High | P1 |
| Copy/Paste | High | Med | P1 |
| Net label placement | High | Med | P1 |
| Power port placement | High | Med | P1 |
| Wire routing modes | Med | Med | P1 |
| In-place text editing | Med | Med | P2 |
| ERC basic | High | High | P2 |
| No-connect marker | Med | Low | P2 |
| Annotation | Med | Med | P2 |
| Tab during placement | Med | Low | P2 |
