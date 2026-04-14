# Signex vs Altium Designer — Feature Gap Analysis

## How to Read This Document

- **Have** = implemented in current Signex (TypeScript/React/Tauri)
- **Planned** = in the migration plan but not yet in Iced codebase
- **GAP** = missing from both current code AND the migration plan — must be added
- **Defer** = not targeting for v1.0, may add post-launch

Every GAP item is assigned to a workstream (see Parallel Workstream Map at the end).

---

## 1. Schematic Editor

### 1.1 Placement & Drawing

| Feature | Altium | Signex | Status |
|---|---|---|---|
| Wire drawing (Manhattan/45/free) | Yes | Yes | **Have** |
| Bus drawing + bus entries | Yes | Yes | **Have** |
| Net label (4 types) | Yes | Yes | **Have** |
| Power port placement | Yes | Yes | **Have** |
| No-connect marker | Yes | Yes | **Have** |
| Component placement from library | Yes | Yes (226 libs) | **Have** |
| Sheet symbol (hierarchical) | Yes | Yes | **Have** |
| Port placement | Yes | Yes | **Have** |
| Text, text frame, note | Yes | Yes | **Have** |
| Line/rect/circle/arc/polyline/polygon | Yes | Yes | **Have** |
| Ellipse, rounded rectangle | Yes | Yes | **Have** |
| Image embedding | Yes | Yes | **Have** |
| Bezier curve | Yes | No | **GAP** |
| Dimension annotation | Yes | PCB only | **GAP** — add to schematic |
| Off-sheet connector | Yes | Via labels | Partial |
| Signal harness | Yes | Yes | **Have** |
| Harness connector/entry | Yes | Yes | **Have** |
| Parameter set directive | Yes | Yes | **Have** |
| Differential pair directive | Yes | Yes | **Have** |
| Blanket directive | Yes | Yes | **Have** |
| Compile mask | Yes | Yes | **Have** |

### 1.2 Editing Operations

| Feature | Altium | Signex | Status |
|---|---|---|---|
| Move with rubber-banding | Yes | Yes | **Have** |
| Stiff move (Ctrl+drag) | Yes | Yes | **Have** |
| Rotate 90 (Space) | Yes | Yes | **Have** |
| Mirror X/Y | Yes | Yes | **Have** |
| Copy/Cut/Paste | Yes | Yes | **Have** |
| Smart paste (arrays) | Yes | Yes | **Have** |
| Duplicate (Ctrl+D) | Yes | Yes | **Have** |
| Undo/Redo (50 levels) | Yes | Yes | **Have** |
| Align (L/R/T/B/CenterH/CenterV) | Yes | Yes | **Have** |
| Distribute (H/V) | Yes | Yes | **Have** |
| Z-order (front/back) | Yes | Yes | **Have** |
| Break wire at point | Yes | Yes | **Have** |
| Selection memory (Ctrl+1-8) | Yes | Yes | **Have** |
| Find Similar Objects (Shift+F) | Yes | Yes | **Have** |
| In-place text edit (F2) | Yes | Yes | **Have** |
| Nudge (Ctrl+Arrow) | Yes | Yes | **Have** |
| Rubber stamp (Ctrl+R, repeated paste) | Yes | No | **GAP** |
| Measure distance (Ctrl+M) | Yes | No | **GAP** |
| Change component (swap with different part) | Yes | No | **GAP** |
| Slice (cut crossing objects) | Yes | No | **Defer** |

### 1.3 Net & Connectivity

| Feature | Altium | Signex | Status |
|---|---|---|---|
| Net classes | Yes | Yes | **Have** |
| Diff pair naming (_P/_N) | Yes | Yes | **Have** |
| Net color override (F5) | Yes | Yes | **Have** |
| Net highlight (Alt+click) | Yes | Yes | **Have** |
| Multi-channel design (Repeat) | Yes | Yes | **Have** |
| Net identifier scope (global/flat/hier) | Yes | Yes | **Have** |
| xNet (cross-sheet net analysis) | Yes | No | **GAP** |
| Net ties | Yes | No | **GAP** |
| Net aliases | Yes | Via labels | Partial |
| Hidden pin connectivity | Yes | Yes | **Have** |

### 1.4 Validation & Compilation

| Feature | Altium | Signex | Status |
|---|---|---|---|
| ERC (11 violation types) | Yes | Yes | **Have** |
| Pin connection matrix (12x12) | Yes | Yes | **Have** |
| ERC severity per rule | Yes | Yes | **Have** |
| No-ERC directive | Yes | Yes | **Have** |
| Auto-junction at T-intersections | Yes | Yes | **Have** |
| Project compilation (multi-sheet) | Yes | Partial | **GAP** — need full compilation pass |
| Net scope rule enforcement | Yes | No | **GAP** |
| Component linking validation | Yes | No | **GAP** |
| Duplicate net name detection | Yes | Yes | **Have** (ERC #7) |

### 1.5 Annotation

| Feature | Altium | Signex | Status |
|---|---|---|---|
| Auto-annotation (4 modes) | Yes | Yes | **Have** |
| Designator locking | Yes | Yes | **Have** |
| Annotation preview | Yes | Yes | **Have** |
| Reset designators | Yes | Yes | **Have** |
| Schematic-level annotation | Yes | Yes | **Have** |
| PCB-level annotation (by board position) | Yes | No | **GAP** |
| Board-level annotation | Yes | No | **GAP** |

### 1.6 Parameters & Variants

| Feature | Altium | Signex | Status |
|---|---|---|---|
| Component parameters/fields | Yes | Yes | **Have** |
| Document parameters | Yes | Yes | **Have** |
| Project parameters | Yes | Yes | **Have** |
| Parameter hierarchy resolution | Yes | Yes | **Have** |
| Parameter Manager (spreadsheet) | Yes | Yes | **Have** |
| Text string substitution | Yes | Yes | **Have** |
| Design variants (fitted/not-fitted) | Yes | Yes | **Have** |
| Alternate value per variant | Yes | Yes | **Have** |
| Alternate footprint per variant | Yes | Yes | **Have** |
| Variant-specific BOM | Yes | Partial | **GAP** — BOM filter by variant |
| Variant visual indication | Yes | No | **GAP** — show fitted/not-fitted on canvas |

---

## 2. PCB Editor

### 2.1 Interactive Routing

| Feature | Altium | Signex | Status |
|---|---|---|---|
| Walkaround routing | Yes | Yes | **Have** |
| Push/shove routing | Yes | Yes | **Have** |
| Hug/push routing | Yes | Yes | **Have** |
| Ignore obstacles routing | Yes | Yes | **Have** |
| Differential pair routing | Yes | Yes | **Have** |
| Length-tuned routing (meander) | Yes | Yes (3 styles) | **Have** |
| Multi-track (bus) routing | Yes | Yes | **Have** |
| Route completion (loop removal) | Yes | No | **GAP** |
| Glossing (post-route optimization) | Yes | No | **GAP** |
| Impedance-controlled routing | Yes | No | **GAP** — need stackup-aware Z₀ display |
| Backdrilling | Yes | No | **GAP** |
| Any-angle routing | Yes | Yes (free mode) | **Have** |
| Arc routing (radius corners) | Yes | Yes (arc45/arc90) | **Have** |
| Interactive length gauge during route | Yes | No | **GAP** |
| Auto-router | Yes | No | **GAP** — topological autorouter |
| Routing topology constraints (star/chain/fly-by/T) | Yes | No | **GAP** |
| Via auto-stitching during route | Yes | Separate tool | Partial |

### 2.2 Via Management

| Feature | Altium | Signex | Status |
|---|---|---|---|
| Through vias | Yes | Yes | **Have** |
| Blind vias | Yes | Yes | **Have** |
| Buried vias | Yes | Yes | **Have** |
| Micro vias | Yes | Yes | **Have** |
| Via stitching (grid/fence) | Yes | Yes | **Have** |
| Via-in-pad | Yes | No | **GAP** |
| Tented vias (solder mask over) | Yes | No | **GAP** — mask expansion rule |
| Backdrilled vias | Yes | No | **GAP** |
| Via arrays | Yes | No | **GAP** |

### 2.3 Copper Management

| Feature | Altium | Signex | Status |
|---|---|---|---|
| Copper pour (solid/hatch/none) | Yes | Yes | **Have** |
| Thermal relief (spoke count/width) | Yes | Yes | **Have** |
| Direct connect pads | Yes | Partial | **GAP** — configurable per pad |
| Zone priority ordering | Yes | Yes | **Have** |
| Dead copper removal | Yes | Yes | **Have** |
| Split planes (negative plane layers) | Yes | No | **GAP** |
| Copper balancing (dummy fills) | Yes | No | **GAP** |
| Copper thieving (dot pattern fills) | Yes | No | **GAP** |
| Plane power connect style | Yes | No | **GAP** |
| Polygon pour cutouts (void regions) | Yes | Partial | **GAP** — arbitrary void shapes |

### 2.4 Design Rules

Altium has the most comprehensive rule system of any EDA tool. The key differentiator is **query-based rule scoping** — every rule can be scoped to any subset of the design using expressions like `InNetClass('DDR') AND OnLayer('Top')`. Multiple rules can overlap; the most specific wins.

| Feature | Altium | Signex | Status |
|---|---|---|---|
| **Electrical Rules** | | | |
| Clearance (copper-to-copper) | Yes | Yes | **Have** |
| Short circuit detection | Yes | Yes | **Have** |
| Unrouted net detection | Yes | Yes | **Have** |
| Un-connected pin | Yes | Yes | **Have** |
| Net antennae (dead-end stubs) | Yes | No | **GAP** |
| **Routing Rules** | | | |
| Width (min only) | Yes | Yes | **Have** |
| Width (min/preferred/max) | Yes | No | **GAP** |
| Routing topology (star/chain/fly-by/T) | Yes | No | **GAP** |
| Routing priority (per net) | Yes | No | **GAP** |
| Routing layers (restrict net to layers) | Yes | No | **GAP** |
| Routing corners (per net class) | Yes | Partial | **GAP** |
| Routing via style (via size per net class) | Yes | No | **GAP** |
| Fanout control (via-at-pad vs via-outside) | Yes | No | **GAP** |
| Diff pair gap + impedance | Yes | Partial | **GAP** |
| **Manufacturing Rules** | | | |
| Min via size / drill | Yes | Yes | **Have** |
| Annular ring minimum | Yes | Yes | **Have** |
| Hole-to-hole spacing | Yes | Yes | **Have** |
| Board outline clearance | Yes | Yes | **Have** |
| Min drill size | Yes | Yes | **Have** |
| Solder mask sliver | Yes | Yes | **Have** |
| Paste expansion (per pad/per rule) | Yes | No | **GAP** |
| Mask expansion (per pad/per rule) | Yes | No | **GAP** |
| Silk-to-mask clearance | Yes | Yes | **Have** |
| Silk-to-silk clearance | Yes | No | **GAP** |
| Acute angle detection | Yes | No | **GAP** |
| Acid trap detection | Yes | No | **GAP** |
| **High-Speed Rules** | | | |
| Matched lengths (group + tolerance) | Yes | Partial | **GAP** — need group UI |
| Max via count per net | Yes | No | **GAP** |
| Impedance target (from layer stack) | Yes | No | **GAP** |
| Propagation delay max | Yes | No | **GAP** — via OpenEMS |
| **Placement Rules** | | | |
| Component clearance (courtyard) | Yes | No | **GAP** |
| Component height (per region) | Yes | No | **GAP** |
| Testpoint rule | Yes | No | **GAP** |
| **Plane Rules** | | | |
| Plane connect style (thermal/direct) | Yes | No | **GAP** |
| Plane clearance (anti-pad size) | Yes | No | **GAP** |
| Polygon connect style (per pad override) | Yes | Partial | **GAP** |
| **Rule Scoping** | | | |
| Query-based scoping (InNet, InNetClass, OnLayer, boolean) | Yes | No | **GAP** — critical differentiator |
| Priority-based rule resolution | Yes | No | **GAP** |

### 2.5 Layer Stack Manager

| Feature | Altium | Signex | Status |
|---|---|---|---|
| Physical layer stackup definition | Yes | Partial | **GAP** — need full editor |
| Dielectric type (prepreg/core) | Yes | In sim bridges | **GAP** — need in PCB UI |
| Copper weight (oz/µm) | Yes | In sim bridges | **GAP** |
| Impedance profile per layer pair | Yes | No | **GAP** |
| Stackup templates (2/4/6/8 layer) | Yes | No | **GAP** |
| Signal vs plane layer designation | Yes | No | **GAP** |
| Flex-rigid stackup | Yes | No | **Defer** |
| Dielectric constant (εr) per material | Yes | In sim bridges | **GAP** |
| Loss tangent (tanδ) per material | Yes | In sim bridges | **GAP** |
| Layer pair definitions | Yes | Partial | **GAP** |

### 2.6 High-Speed Design

| Feature | Altium | Signex | Status |
|---|---|---|---|
| xSignals (pad-to-pad signal path) | Yes | No | **GAP** |
| Length matching groups | Yes | Partial | **GAP** — need grouping UI |
| Matched group tuning (accordion) | Yes | Partial | **GAP** — need visual tuning |
| Topology constraints (star/chain/fly-by) | Yes | No | **GAP** |
| Return path analyzer | Yes | No | **GAP** |
| Power plane analyzer | Yes | No | **GAP** |
| Impedance calculator | Yes | Via OpenEMS | Partial |
| Via counting in matched length | Yes | No | **GAP** |
| Propagation delay analysis | Yes | Via OpenEMS | Partial |
| Diff pair impedance control | Yes | Partial | **GAP** — need stackup-linked |

### 2.7 Component Management

| Feature | Altium | Signex | Status |
|---|---|---|---|
| Component placement (move/rotate/flip) | Yes | Yes | **Have** |
| Push/shove placement | Yes | Yes | **Have** |
| Footprint swap | Yes | Yes | **Have** |
| Locked footprints | Yes | Yes | **Have** |
| Component groups | Yes | Partial | **GAP** |
| Rooms (from multi-channel) | Yes | No | **GAP** |
| Component height constraints | Yes | No | **GAP** |
| Embedded components (cavity) | Yes | No | **Defer** |
| 3D clearance checking | Yes | No | **GAP** |

### 2.8 Board Mechanical

| Feature | Altium | Signex | Status |
|---|---|---|---|
| Board outline editing | Yes | Yes | **Have** |
| Board cutouts | Yes | Partial | **GAP** — arbitrary internal cutouts |
| Slots (elongated holes) | Yes | Partial | **GAP** |
| NPTH (non-plated holes) | Yes | Yes | **Have** |
| Mounting holes | Yes | Yes | **Have** |
| Keepout regions (per layer) | Yes | Partial | **GAP** — need per-layer keepout |
| Via keepout | Yes | No | **GAP** |
| Routing keepout | Yes | No | **GAP** |
| Component keepout | Yes | No | **GAP** |
| Rigid-flex board outline | Yes | No | **Defer** |
| Board bending simulation | Yes | No | **Defer** |

---

## 3. Manufacturing Output

| Feature | Altium | Signex | Status |
|---|---|---|---|
| Gerber RS-274X | Yes | Yes | **Have** |
| Gerber X2 (extended) | Yes | Yes | **Have** |
| Excellon drill | Yes | Yes | **Have** |
| ODB++ | Yes | Yes | **Have** |
| IPC-2581 | Yes | Yes (simplified) | **Have** |
| STEP 3D export | Yes | Yes | **Have** |
| Pick-and-place CSV | Yes | Yes | **Have** |
| Assembly SVG | Yes | Yes | **Have** |
| PDF schematic | Yes | Yes | **Have** |
| Panelization (step-and-repeat) | Yes | No | **GAP** |
| V-cut scoring | Yes | No | **GAP** |
| Tab routing (breakaway tabs) | Yes | No | **GAP** |
| Fiducials (auto-place) | Yes | No | **GAP** |
| Tooling holes (auto-place) | Yes | No | **GAP** |
| Drill table (fabrication drawing) | Yes | No | **GAP** |
| Board stackup report | Yes | No | **GAP** |
| Canvas Docs (fabrication/assembly docs) | Yes | No | **GAP** |
| DXF export | Yes | No | **GAP** |
| Test point report | Yes | No | **GAP** |

---

## 4. Library Management

| Feature | Altium | Signex | Status |
|---|---|---|---|
| Symbol library editor | Yes | Yes | **Have** |
| Footprint library editor | Yes | Yes | **Have** |
| 226 KiCad libraries | N/A | Yes | **Have** |
| Library search + browse | Yes | Yes | **Have** |
| Component preview | Yes | Yes | **Have** |
| Unified component model (sym+fp+3D+sim) | Yes | No | **GAP** |
| Managed libraries (cloud) | Yes (365) | No | **GAP** — Pro (Supabase) |
| Lifecycle management (active/EOL/obsolete) | Yes | No | **GAP** |
| Part choices (multi-supplier) | Yes | No | **GAP** |
| Supplier data (stock, price) | Yes | No | **GAP** — integration TBD |
| Where-used analysis | Yes | No | **GAP** |
| Library import (Eagle, OrCAD, PADS) | Yes | No | **Defer** |

---

## 5. ECO (Engineering Change Order)

| Feature | Altium | Signex | Status |
|---|---|---|---|
| Forward annotation (SCH→PCB) | Yes | Yes | **Have** |
| Back annotation (PCB→SCH) | Yes | Yes | **Have** |
| ECO dialog (change list) | Yes | Partial | **GAP** — need formal change list |
| Change review before apply | Yes | Partial | **GAP** |
| Component add/remove sync | Yes | Yes | **Have** |
| Net changes sync | Yes | Yes | **Have** |
| Footprint changes sync | Yes | Partial | **GAP** |

---

## 6. Cross-Probing & Navigation

| Feature | Altium | Signex | Status |
|---|---|---|---|
| SCH→PCB cross-probe | Yes | Yes | **Have** |
| PCB→SCH cross-probe | Yes | Yes | **Have** |
| Cross-select mode (Shift+Ctrl+X) | Yes | Yes | **Have** |
| Zoom-to-component | Yes | Yes | **Have** |
| Net highlight across editors | Yes | Yes | **Have** |
| Multi-sheet navigation (tabs) | Yes | Yes | **Have** |
| Hierarchical navigation (Ctrl+double-click) | Yes | Yes | **Have** |
| Navigation bookmarks | Yes | Yes | **Have** |

---

## 7. Import/Export Formats

| Feature | Altium | Signex | Status |
|---|---|---|---|
| KiCad import (.kicad_sch, .kicad_pcb) | No (native) | Yes | **Have** |
| KiCad export | No | Yes | **Have** |
| Eagle import | Yes | No | **GAP** |
| OrCAD import | Yes | No | **Defer** |
| PADS import | Yes | No | **Defer** |
| Mentor import | Yes | No | **Defer** |
| Altium import (.SchDoc, .PcbDoc) | N/A | No | **GAP** — critical for migration |
| DXF import (board outline) | Yes | No | **GAP** |
| DXF export | Yes | No | **GAP** |
| SVG export | Yes | Yes (assembly) | Partial |

---

## 8. Simulation

Signex's simulation stack goes far beyond Altium. Altium has basic SPICE and a simplified impedance calculator. Signex targets full DDR signal integrity, RF design, and multi-physics analysis.

### 8.1 SPICE (ngspice — schematic-level)

| Feature | Altium | Signex | Status |
|---|---|---|---|
| DC operating point | Yes | Planned (ngspice) | **Planned** |
| AC small signal (frequency response) | Yes | Planned (ngspice) | **Planned** |
| Transient analysis | Yes | Planned (ngspice) | **Planned** |
| DC sweep | Yes | Planned (ngspice) | **Planned** |
| Noise analysis | Yes | Planned (ngspice) | **Planned** |
| Pole-zero analysis | Yes | Planned (ngspice) | **Planned** |
| Transfer function | Yes | Planned (ngspice) | **Planned** |
| Fourier (FFT) | Yes | Planned (ngspice) | **Planned** |
| Mixed-signal (digital + analog) | Yes | Planned (ngspice xSpice) | **Planned** |
| Monte Carlo analysis | Yes | Planned (ngspice) | **Planned** |
| Parameter sweeps | Yes | No | **GAP** — add to ngspice UI |
| Temperature sweep | Yes | No | **GAP** |
| Simulation probing (V/I/P) | Yes | Planned | **Planned** |
| Waveform viewer (multi-trace, cursor) | Yes | Planned | **Planned** |
| IBIS model support | No | Planned (ngspice) | **Planned** — Signex advantage |
| S-parameter (.s2p/.s4p) import as SPICE element | No | No | **GAP** — critical for SI |
| Harmonic balance (RF) | No | Planned (ngspice HB) | **Planned** — Signex advantage |
| PAC / PNoise (RF noise) | No | Planned (ngspice) | **Planned** — Signex advantage |
| Envelope analysis | No | Planned (ngspice) | **Planned** — Signex advantage |

### 8.2 Signal Integrity — DDR (ngspice + OpenEMS)

DDR signal integrity requires analyzing the complete signal path from FPGA/SoC to memory — not just individual nets but the full channel including package, PCB traces, vias, and termination.

| Feature | Altium | Signex | Status |
|---|---|---|---|
| **IBIS-based SI simulation** | Via SIMetrix plugin | Planned (ngspice IBIS import) | **Planned** |
| **Eye diagram generation** | No | No | **GAP** — add to waveform viewer |
| **Bit error rate estimation** (bathtub curve) | No | No | **GAP** |
| **DDR timing analysis** (setup/hold/skew) | No | No | **GAP** — critical for DDR3/4/5 |
| **Channel simulation** (Tx IBIS → trace → Rx IBIS) | No | No | **GAP** — compose xSignal + IBIS + S-param |
| **S-parameter cascade** (connect .s2p models in series) | No | No | **GAP** — via/connector/package models |
| **Crosstalk analysis** (NEXT/FEXT) | No | Planned (OpenEMS) | **Planned** |
| **TDR/TDT** (time-domain reflectometry) | No | Planned (OpenEMS) | **Planned** |
| **Power delivery network (PDN) impedance** | No | No | **GAP** — Z(f) of PDN from VRM to IC |
| **Decoupling capacitor optimization** | No | No | **GAP** — Signal AI suggests cap values/placement |
| **Return loss / insertion loss** | No | Planned (OpenEMS) | **Planned** |
| **Jitter analysis** (deterministic + random) | No | No | **GAP** |

**DDR SI workflow in Signex:**
1. Define xSignals (pad-to-pad through series R) in PCB
2. Extract S-parameters per trace segment via OpenEMS FDTD
3. Import IBIS models for controller + memory
4. Cascade: Tx IBIS → via S-param → trace S-param → via S-param → Rx IBIS
5. Run transient in ngspice with PRBS stimulus
6. Generate eye diagram in waveform viewer
7. Check setup/hold timing margins vs DDR spec
8. Signal AI reports pass/fail per signal group + suggests fixes

### 8.3 RF/EM Simulation (OpenEMS FDTD)

| Feature | Altium | Signex | Status |
|---|---|---|---|
| S-parameter extraction (S11/S21/S12/S22) | No | Planned (OpenEMS) | **Planned** |
| Smith chart display | No | Planned | **Planned** |
| TDR/TDT impedance profile | No | Planned (OpenEMS) | **Planned** |
| Crosstalk (NEXT/FEXT) | No | Planned (OpenEMS) | **Planned** |
| Near-field E/H visualization (3D overlay) | No | Planned (OpenEMS) | **Planned** |
| **Antenna simulation** (radiation pattern, gain, S11) | No | No | **GAP** — OpenEMS supports this natively |
| **Filter design** (lumped + distributed, auto-tuning) | No | No | **GAP** — Signal AI + OpenEMS |
| **Transmission line analysis** (microstrip, stripline, coplanar) | Built-in calculator | Planned (OpenEMS full-wave) | **Planned** — more accurate than Altium |
| **Via transition modeling** (full-wave S-param extraction) | No | Planned (OpenEMS) | **Planned** |
| **Via impedance calculation** (Z₀ of via barrel from geometry + stackup) | No | No | **GAP** — Signex advantage |
| **Via delay calculation** (propagation delay through via from layer stack height) | No | No | **GAP** — Signex advantage |
| **Via stub resonance** (stub length → resonant frequency → performance limit) | No | No | **GAP** — Signex advantage |
| **Connector modeling** (SMA, USB-C, HDMI) | No | No | **GAP** — import connector .s2p from manufacturer |
| **PCB-to-package co-simulation** | No | No | **GAP** — import package model, co-sim with PCB |
| **Frequency sweep automation** | No | Planned (OpenEMS) | **Planned** |
| **GPU-accelerated FDTD** | N/A | feature/cuda-engine branch | **Planned** — Signex advantage (alplabai/openEMS) |

### 8.4 Thermal & Power Simulation (Elmer FEM)

| Feature | Altium | Signex | Status |
|---|---|---|---|
| Steady-state thermal analysis | No | Planned (Elmer HeatSolve) | **Planned** |
| Transient thermal analysis | No | No | **GAP** — Elmer supports this |
| DC IR drop (voltage distribution) | Simplified | Planned (Elmer StatCurrentSolve) | **Planned** — more accurate |
| Current density visualization | No | Planned (Elmer) | **Planned** |
| Joule heating (coupled thermal-electrical) | No | Planned (Elmer) | **Planned** |
| **Component junction temperature estimation** | No | No | **GAP** — Rjc from datasheet + thermal map |
| **Thermal via optimization** | No | No | **GAP** — Signal AI suggests via count/placement |
| **PDN impedance vs frequency** | No | No | **GAP** — Elmer + OpenEMS co-sim |
| **Power plane resonance detection** | No | No | **GAP** — cavity resonance via OpenEMS |
| 3D temperature overlay on PCB model | No | Planned (Elmer → Bevy vertex colors) | **Planned** |

### 8.5 Simulation Integration (Signex advantage)

No EDA tool integrates circuit sim + EM sim + thermal sim + AI in one environment:

| Integration | What It Does | Altium |
|---|---|---|
| **SPICE ↔ Waveform cross-probe** | Click net on schematic → see waveform. Click waveform peak → highlight net. | Partial |
| **PCB trace → S-parameters** | Select trace in PCB → run OpenEMS → see S11/S21 | No |
| **PCB → thermal map** | Run Elmer on PCB → see temperature overlay on 3D model | No |
| **IR drop → Joule heating → thermal** | Coupled: current density heats copper → temperature map | No |
| **S-param → eye diagram** | Extract S-params → cascade with IBIS → generate eye | No |
| **Signal AI analysis** | AI reads sim results + design context → suggests improvements | No |
| **Sim job queue** | Multiple sims run concurrently. UUID per job, progress bar, cancel. | No |
| **Sim results in design review** | Signal AI references specific sim data when reviewing design | No |

---

## 9. Preferences & Customization

| Feature | Altium | Signex | Status |
|---|---|---|---|
| 6 built-in themes | No (2) | Yes (6) | **Have** — Signex advantage |
| Theme editor | No | Yes | **Have** — Signex advantage |
| Keyboard shortcut customization | Yes | No | **GAP** |
| Toolbar customization | Yes | No | **Defer** |
| Workspace layouts (save/restore) | Yes | No | **GAP** |
| View configurations (save/restore) | Yes | No | **GAP** |
| Color profiles | Yes | Via themes | Partial |
| Preferences import/export | Yes | No | **GAP** |

---

## 10. Constraint Manager (Critical Gap — G59i)

Altium's Constraint Manager is a **schematic-side spreadsheet for defining ALL PCB design rules**. It's one of Altium's most powerful features and has no equivalent in any open-source EDA tool.

### What It Is

A tabbed spreadsheet dialog accessible from the schematic editor (`Design > Constraint Manager`) that lets the engineer define PCB routing and manufacturing constraints before ever opening the PCB editor. Constraints propagate to the PCB via ECO, so the PCB editor receives fully-configured rules.

### Tabs / Rule Categories

| Tab | Rules Defined | Example |
|---|---|---|
| **Electrical** | Clearance (net-to-net, class-to-class), short circuit | DDR_Data ↔ DDR_CLK clearance = 5mil |
| **Routing** | Width (min/pref/max per layer), via style, topology, layer restriction, impedance | CLK net: 4.5mil on Top, 3.8mil on In1, 50Ω target |
| **High Speed** | Matched length (group + tolerance), max via count, diff pair gap/skew | DDR_DQ group: match to ±50mil, max 2 vias |
| **Manufacturing** | Annular ring, hole size, mask expansion, paste expansion, silk clearance | Global: min annular ring 4mil, mask expansion 2mil |
| **Placement** | Component clearance, height, orientation, room rules | U1 zone: max height 3mm, clearance 10mil |

### Key Features

**Schematic-Driven:** Rules are defined in the schematic context where the engineer understands signal intent. No need to switch to PCB to set constraints. Rules attach to nets, net classes, diff pairs, and component groups that are visible in the schematic.

**Spreadsheet Editing:** Rows = design objects (nets, net classes, diff pairs, components). Columns = rule parameters. Cell editing with dropdown menus, numerical inputs. Copy/paste cells. Bulk editing (select multiple rows, change value). Sort and filter columns.

**Layer-Aware Width Rules:** A single net can have different trace widths per copper layer:
```
CLK net:
  Top Layer:    width = 4.5mil (microstrip impedance)
  In1.Cu:       width = 3.8mil (stripline impedance)
  In2.Cu:       width = 3.8mil
  Bottom Layer: width = 4.5mil
```

**Impedance-Linked:** Width rules can be auto-calculated from impedance targets using the layer stack's dielectric properties. The Constraint Manager shows both the target impedance and the resulting width.

**Diff Pair Constraints:** Gap, impedance, max uncoupled length, intra-pair skew — all editable per diff pair class in a single row.

**Matched Length Groups:** Define groups, assign nets/xSignals, set target length and tolerance. The dashboard shows in/out-of-tolerance status.

**ECO Propagation:** All constraints defined in the Constraint Manager generate PCB design rules via the ECO system. Changes in either direction stay synchronized.

### Signex Implementation Plan

The Constraint Manager belongs to **WS-B (Validation)** and should be implemented as:

1. An egui/Iced panel (not a dialog — always accessible) with tabbed spreadsheet layout
2. Rules stored in `signex-types` as a structured `ConstraintSet`
3. Parser integration: read existing KiCad design rules, write back
4. ECO integration: constraints propagate to PCB DRC engine
5. Impedance calculator integration: link to layer stack for width calculation
6. Query-based scoping: rules reference nets/classes using the query engine (G41)

**Sprint:** Sprint 3 (with WS-B ERC/validation) for schematic-side rules, Sprint 4 (with DRC) for PCB-side enforcement.

---

## 11-pre-00. Feature Details (G59x-G60v)

### Descriptive Undo History (G60o)

Instead of generic "Undo" / "Redo", the Edit menu and Ctrl+Z tooltip show the specific action:

```
Edit > Undo Move U1, R3, R4         (Ctrl+Z)
Edit > Undo Route NET_CLK on F.Cu   (Ctrl+Z)
Edit > Redo Delete C12              (Ctrl+Y)
```

**Implementation:** Each undo command stores a `description: String` alongside the state delta. The undo stack exposes a history list visible in `Edit > Undo History` submenu — click any entry to undo/redo to that point. Altium shows only the last action; Signex shows the full stack (50 levels) with descriptions.

**Sprint:** Sprint 3, WS-A (Editor). Requires command pattern undo to store descriptions.

### Gerber X3 / IPC-2581C (G60p)

Gerber X3 is the emerging successor to X2 — it embeds complete component data, netlist, BOM, and stackup information directly in Gerber files, eliminating the need for separate ODB++/IPC-2581 packages. Signex should be among the first EDA tools to support it.

**Sprint:** Sprint 5, WS-C (Output). Extension of the existing Gerber X2 exporter.

### Project-Level "Not Fitted" Display Toggle (G60q)

A project-wide setting that controls whether DNP (not-fitted) visual indicators appear on schematic sheets. Separate from the per-variant drawing style (G59m visual):

```
Project > Settings > Variants:
  Show not-fitted indicators: [Always / Active variant only / Never]
  Show variant name badge:    [Yes / No]
```

When set to "Never", schematics appear clean for documentation purposes. When set to "Active variant only", only the currently-selected variant's DNP markers appear.

**Sprint:** Sprint 3, WS-A (Editor). Simple rendering toggle.

### Convert 3D Body Group to STEP (G60r)

Select multiple 3D body objects in the footprint editor → right-click → "Export as STEP". Merges all selected bodies into a single `.step` file using `truck-modeling`. Useful for creating component 3D models from primitive shapes without needing external CAD.

**Sprint:** Sprint 5, WS-G (3D).

### Design Rule Variables (G60s)

Named variables that can be referenced in design rule values:

```
Variables:
  $DDR_CLR    = 5mil
  $DDR_WIDTH  = 4.5mil
  $POWER_CLR  = 8mil
  $MIN_VIA    = 0.3mm

Rules:
  DDR Data Clearance:   min = $DDR_CLR
  DDR Addr Clearance:   min = $DDR_CLR      ← change $DDR_CLR, both update
  DDR Data Width:       preferred = $DDR_WIDTH
  Power Clearance:      min = $POWER_CLR
```

Variables are defined in `Design > Rule Variables` dialog. When a variable value changes, all rules referencing it update. This eliminates duplicate rule values and makes global adjustments trivial.

**Sprint:** Sprint 4, WS-B (Validation). Extension of the query-based rule engine.

### Project Lock / Read-Only Mode (G60t)

`Project > Lock Project` sets all project files to read-only. A lock icon appears on the document tab bar. Any edit attempt shows "Project is locked. Unlock to edit." Prevents accidental changes to released designs. Can be combined with Git tags — locking a project at a tagged release.

**Sprint:** Sprint 3, WS-A (Editor). File permission + UI state.

### External Program in OutJob (G60u)

The Output Job system can run external programs as a step in the output pipeline:

```
Output Job "Manufacturing Release":
  1. Generate Gerber X2       → /output/gerber/
  2. Generate Excellon Drill  → /output/drill/
  3. Generate BOM CSV         → /output/bom/
  4. Run: python validate_gerber.py /output/gerber/   ← external script
  5. Run: zip -r release.zip /output/                 ← package
  6. Run: curl -X POST https://api.jlcpcb.com/...     ← submit to fab
```

External programs run as subprocesses with configurable working directory, args, environment variables, and timeout. Output (stdout/stderr) captured in the job log. Non-zero exit code marks the job as failed.

**Sprint:** Sprint 5, WS-C (Output). Subprocess execution in the output pipeline.

### Polygon Pour Rounded Edges Around Pads (G60x)

When a copper pour flows around a pad with thermal relief or clearance, the boundary between pour and pad is currently a sharp polygon edge. Sharp corners on pour boundaries cause:
- Acid traps during etching (acute angles hold etchant)
- Stress concentrations in copper (thermal cycling fatigue)
- DFM warnings from some fabs

**Signex improvement:** Polygon pour clearance boundaries use **filleted (rounded) corners** instead of sharp polygon vertices.

```rust
pub struct PourClearanceStyle {
    pub clearance: Coord,              // Gap between pour and pad copper
    pub corner_style: PourCornerStyle,
}

pub enum PourCornerStyle {
    Sharp,                  // Current behavior — polygon vertices are acute
    Rounded { radius: Coord },  // Fillet all clearance boundary corners with this radius
    Chamfered { size: Coord },  // 45° chamfer cut instead of round
}
```

**Configuration:**
```
Design > Rules > Polygon Pour:
  Clearance corner style: [Sharp / Rounded / Chamfered]
  Corner radius:          [0.1mm default, range 0.05-0.5mm]
```

This applies to:
- Thermal relief air gaps (the space between spokes)
- Direct clearance boundaries (pour edge near non-connected pads)
- Via clearance cutouts in pours

**Visual result:**
```
Sharp (current):              Rounded (new):
┌───────┐                     ╭───────╮
│  pad  │                     │  pad  │
│       │                     │       │
└───┐   └───                  ╰───╮   ╰───
    │                              │
    │ pour                         │ pour
```

The rounded version produces smoother, more manufacturable copper geometry.

**Sprint:** Sprint 4, WS-E (PCB Core). Extends the Clipper2 polygon offset with corner rounding in `pcb-geom/copper_pour.rs`.

### Micrometer (µm) Unit for PCB Layout (G60y)

The current unit cycle is mm → mil → inch. For HDI PCBs, MEMS, semiconductor packaging, and chip-on-board designs, engineers work in **micrometers (µm)**.

**Changes to the coordinate system:**

The `Coord = i64` nanometer base already supports µm natively:
```rust
pub const NM_PER_UM: i64 = 1_000;        // 1µm = 1000nm — exact integer
pub const NM_PER_MM: i64 = 1_000_000;    // 1mm = 1000µm
pub const NM_PER_MIL: i64 = 25_400;      // 1mil = 25.4µm
```

**Unit cycle becomes:** mm → mil → inch → **µm** → mm (Ctrl+Q cycles)

**Display formatting:**
| Unit | Precision | Example |
|---|---|---|
| mm | 2 decimals | `0.15mm` |
| mil | 1 decimal | `5.9mil` |
| inch | 4 decimals | `0.0059inch` |
| **µm** | **0 decimals** | **`150µm`** |

**Grid presets (µm mode):**
```rust
pub const GRID_MICROMETER: &[Coord] = &[
    1_000,       // 1µm
    5_000,       // 5µm
    10_000,      // 10µm
    25_000,      // 25µm
    50_000,      // 50µm
    100_000,     // 100µm (= 0.1mm)
    250_000,     // 250µm (= 0.25mm)
];
```

**Status bar:** Shows `µm` label when in micrometer mode. Cursor position: `X:1250µm Y:3800µm`.

**Use cases:**
- HDI PCB (laser-drilled microvias: 75-150µm drill, 200-300µm pad)
- Wire bonding (bond pad pitch: 50-100µm)
- MEMS packaging (feature sizes in single-digit µm)
- Chip-on-board (die attach: pad spacing in µm)
- Advanced packaging (2.5D/3D interposers, silicon bridges)

**Mixed-unit entry:** Type `150um` or `150µm` in any numeric field — auto-converts to current display unit.

**Sprint:** Sprint 1, WS-K (Types). Extends the unit system — must be in place before any UI displays coordinates.

### Git-Derived Parameters (G60w)

Automatically populate schematic text substitution strings from the current Git state:

| Parameter | Value | Use Case |
|---|---|---|
| `=GitBranch` | `feature/ddr-routing` | Show active branch on title block |
| `=GitCommit` | `a1b2c3d` | Short hash for traceability |
| `=GitCommitFull` | `a1b2c3d4e5f6...` | Full SHA for manufacturing release |
| `=GitTag` | `v1.2.0` | Version from nearest tag |
| `=GitAuthor` | `C.Alp` | Last commit author |
| `=GitDate` | `2026-04-10` | Last commit date |
| `=GitDirty` | `*` or `` | Shows `*` if uncommitted changes exist |
| `=GitDescription` | `v1.2.0-3-ga1b2c3d` | Full `git describe` output |

These render in title blocks, revision tables, and any text using `=` substitution. Values update on file save or manual refresh. The `=GitDirty` flag is especially useful — title blocks on unreleased prints show `*` to indicate work-in-progress.

**PDF export:** Git parameters are resolved at export time and embedded in the PDF. The exported PDF always shows the exact Git state at time of generation.

**Sprint:** Sprint 5, WS-A (Editor). Uses `git2` crate (same as G59x).

### Area-Selective DRC (G60v)

Select a rectangular or polygonal region on the PCB → `Design > Run DRC in Selection`. Only checks rules for objects within or crossing the selected area. Much faster than full-board DRC for iterative local edits.

**UI:** Draw a box while holding a modifier key (e.g., Alt+drag in select mode) → DRC runs for that region only → violations appear in DRC panel filtered to the selected area.

**Sprint:** Sprint 4, WS-B (Validation). Spatial filter on DRC engine.

---

### Git, Encrypted SPICE, Teardrops, Unions, Grid (G59x-G60n)

### Built-in Git Branching Support (G59x)

Native Git integration inside Signex — no need to switch to a terminal or external Git client. Altium has SVN/Git integration but it's clunky. Signex makes it first-class.

**Panel:** `Git` panel in left dock (tab alongside Projects).

**Features:**

| Feature | Description |
|---|---|
| **Init/Clone** | Initialize repo for a new project, or clone existing from URL |
| **Branch management** | Create, switch, rename, delete branches. Visual branch graph. |
| **Commit** | Stage files, write commit message, commit. Shows modified/added/deleted files. |
| **Diff** | Visual schematic + PCB diff (S3/S4 features). Click a file → see canvas overlay diff. |
| **Merge** | Merge branch into current. Conflict detection with visual resolution for SCH/PCB. |
| **Pull/Push** | Pull from remote, push to remote. Auth via SSH key or token. |
| **History** | Commit log with visual timeline. Click any commit → see canvas diff at that point. |
| **Stash** | Stash/pop uncommitted changes. |
| **Blame** | Per-object attribution — who last modified this component/trace. |
| **Tags** | Create version tags (v1.0, rev-A, etc.) |

**Visual merge conflict resolution:**
When two branches modify the same schematic sheet, Signex shows a 3-pane merge view:
- Left: "ours" (current branch)
- Right: "theirs" (incoming branch)
- Center: merged result
- Conflicts highlighted in red. Click to pick left/right/both.
- For PCB: region-based merge — accept all changes in a region from one side.

**Library:** Uses `git2` crate (libgit2 Rust bindings) — no external git binary needed.

```toml
git2 = { version = "0.19", default-features = false, features = ["ssh", "https"] }
```

**Pro integration:** When Pro collaboration is active, Git operations sync with Supabase. Local commits push to the shared project. Pull brings collaborator changes. The Supabase edit_log and Git history are complementary: Git for versioned snapshots, Supabase Realtime for live edits.

**Sprint:** Sprint 5, WS-A (Editor). The `git2` crate handles all Git operations. The visual diff (S3/S4) is the main rendering work.

---

### Encrypted SPICE Model Support (G59y)

Semiconductor vendors distribute SPICE models with IP protection — encrypted models that can only be used for simulation, not inspected or reverse-engineered. Altium's simulator supports PSpice-encrypted models (`.lib` with `*$` encryption markers).

**Signex approach:**

| Format | Description | Support |
|---|---|---|
| PSpice encrypted (`.lib`) | Standard `*$` marker encrypted models from TI, ADI, etc. | **Must support** — use ngspice's built-in PSpice compatibility |
| LTspice encrypted (`.enc`) | LTspice-specific encrypted subcircuits | **Must support** — ngspice has LTspice compatibility mode |
| IBIS encrypted | IBIS models with encrypted buffer data | Via ngspice IBIS import |
| Signex encrypted (`.snxmod`) | Signex-native encrypted model format | **New** — for vendor models distributed via Signex ecosystem |

**Signex-native encrypted format (`.snxmod`):**
```rust
pub struct EncryptedModel {
    pub header: ModelHeader,         // Unencrypted: name, pin list, description, vendor
    pub encrypted_netlist: Vec<u8>,  // AES-256-GCM encrypted SPICE subcircuit
    pub signature: Vec<u8>,         // Ed25519 signature by vendor
    pub vendor_pubkey: [u8; 32],    // Vendor's public key for verification
}
```

- The encrypted netlist is decrypted only in-memory during simulation
- Decryption key is derived from vendor license (vendor controls who can simulate)
- Model cannot be exported, printed, or inspected — only simulated
- Signature verification ensures the model hasn't been tampered with
- Vendors can distribute models via Signex component library with `.snxmod` attached

**Sprint:** Sprint 5, WS-J (Sim). Decryption layer wraps ngspice FFI.

---

### Teardrops as Design Rules + Pad/Via Property (G59z)

Current Signex (and most EDA tools) treat teardrops as a post-process: run "Generate Teardrops" after routing and they get added. This is fragile — teardrops break when traces are edited, they're not DRC-checked, and they can't be customized per pad.

**Signex improvement: teardrops are first-class.**

**As a design rule:**
```rust
pub struct TeardropRule {
    pub enabled: bool,
    pub scope: RuleScope,              // Global, per net class, per component, per pad
    pub style: TeardropStyle,
    pub auto_generate: bool,           // Generate during routing (not just post-process)
}

pub struct TeardropStyle {
    pub shape: TeardropShape,          // Arc, Linear, Curved
    pub length_ratio: f32,             // Teardrop length as ratio of pad/via size (0.5-2.0)
    pub width_ratio: f32,              // Max width at pad as ratio of trace width (1.5-3.0)
    pub prefer_sides: TeardropSides,   // Both, TraceEntry, PadSide
}

pub enum TeardropShape {
    Arc,        // Smooth circular arc transition (most common, best for impedance)
    Linear,     // Straight-line taper (simple, less copper)
    Curved,     // Bezier curve (smoothest transition, best SI)
}
```

**As a pad/via property:**
```rust
pub struct PcbPad {
    // ... existing fields ...
    pub teardrop: Option<TeardropOverride>,  // Per-pad override of global teardrop rule
}

pub struct TeardropOverride {
    pub enabled: Option<bool>,       // Override global enable/disable
    pub style: Option<TeardropStyle>, // Override style for this specific pad
}
```

**Behavior:**
- **During routing:** when a trace connects to a pad/via, the teardrop is generated automatically as part of the trace (not as a separate object)
- **On trace edit:** teardrop adjusts automatically when trace width or angle changes
- **DRC:** teardrop geometry is checked against clearance rules — it's real copper
- **Per-pad override:** thermal pads might want larger teardrops; fine-pitch BGA pads might want none
- **Per-net-class:** power nets get larger teardrops (wider trace width ratio); signal nets get standard

**Sprint:** Sprint 4, WS-D (Router) for auto-generation during routing + WS-B (Validation) for DRC integration.

---

### Named Unions (G60m)

Altium has "unions" — groups of selected objects that move and operate as a unit. Signex extends this with **naming and hierarchy**.

```rust
pub struct NamedUnion {
    pub name: String,                // "USB_Hub_Circuit", "DDR_Decoupling_Bank"
    pub members: Vec<Uuid>,          // Component/wire/label UUIDs in this union
    pub color: Option<Color>,        // Optional highlight color for the group
    pub locked: bool,                // Prevent accidental dissolution
    pub nested: Vec<NamedUnion>,     // Unions can contain sub-unions
    pub tags: Vec<String>,           // User-defined tags: "power", "analog", "critical"
}
```

**Features:**
- **Named:** each union has a visible name (shown as a subtle label on canvas when hovered)
- **Hierarchical:** unions can nest (the "DDR" union contains "DDR_Data" and "DDR_Address" sub-unions)
- **Persistent:** unions survive save/load (stored in project file, not just runtime state)
- **Operations:** select all members, move as group, copy as group, delete as group
- **Cross-editor:** a union defined in schematic can map to a PCB room (G66)
- **Navigator:** unions appear in the Navigator panel tree alongside sheets and components
- **Snippets integration:** save a union as a reusable snippet with all internal connectivity
- **Signal AI:** "add the USB hub circuit" → Signal creates a named union with all placed components

**Sprint:** Sprint 3, WS-A (Editor). Extends the existing group system with naming + persistence.

---

### Grid Extends Beyond Board Outline (G60n)

In Altium, the grid only renders inside the board outline. This makes it hard to stage components outside the board before placement (a common workflow: dump all components outside the outline, then drag them in).

**Signex approach:** The grid renders across the entire visible canvas, not just within the board outline. The board outline is drawn as a distinct boundary, but the grid is a viewport feature independent of the board shape.

**Options:**
```
Preferences > PCB > Grid:
  Grid extent: [Board only / Canvas (extends beyond board) / Infinite]
  Outside-board grid opacity: [slider 0-100%, default 40%]
  Outside-board grid color: [color picker, default: dimmer than board grid]
```

Default: `Canvas` mode — grid visible everywhere, slightly dimmer outside the board. Engineers can stage components outside, align them on grid, then drag inside.

**Sprint:** Sprint 4, WS-E (PCB Core). Grid renderer checks board outline intersection and applies opacity.

---

## 11-pre-0. Routing Grid, Watermarking, Mechanical Clearance (G59u, G59v, G59w)

### Routing Grid (G59u)

A separate grid specifically for trace routing, independent of the placement/visible grid. Altium supports this — the routing grid can be finer or coarser than the snap grid and can differ per layer.

**Implementation:**
```rust
pub struct GridSystem {
    pub placement_grid: GridConfig,  // For component placement + schematic editing
    pub routing_grid: GridConfig,    // For PCB trace routing (can differ per layer)
    pub via_grid: GridConfig,        // For via placement
    pub visible_grid: GridConfig,    // What's drawn on screen (may differ from snap)
}

pub struct GridConfig {
    pub size: Coord,                 // Grid spacing in nm
    pub origin: Vec2,                // Grid origin offset
    pub per_layer: Option<HashMap<LayerId, Coord>>,  // Per-layer override
}
```

- During routing, traces snap to the routing grid (not the placement grid)
- Per-layer routing grids: inner layers may use a different grid pitch than outer layers (e.g., 5mil outer, 3.5mil inner for HDI)
- Via grid: vias snap to their own grid (often coarser than routing grid)
- `G` shortcut cycles routing grid presets during active routing
- Grid shown as subtle dots/lines only when routing mode is active

**Sprint:** Sprint 4, WS-D (Router).

### Schematic/PCB Watermarking (G59v)

A semi-transparent overlay text rendered across the entire canvas. Shows document status to prevent uncontrolled distribution of draft or confidential designs.

**Watermark types:**
- `DRAFT` — large gray diagonal text across every sheet
- `CONFIDENTIAL` — with company name
- `FOR REVIEW ONLY` — review copies
- `PRELIMINARY` — pre-release
- `RELEASED` — cleared for manufacturing (optionally with date)
- `OBSOLETE` — superseded design
- Custom text

**Implementation:**
```rust
pub struct Watermark {
    pub text: String,                // "DRAFT", "CONFIDENTIAL - Alp Lab", etc.
    pub enabled: bool,
    pub font_size: f32,              // Large: fills ~30% of sheet width
    pub angle_deg: f32,              // Typically -45° diagonal
    pub color: Color,                // Default: gray at 8% opacity
    pub opacity: f32,                // 0.05-0.15 typical
    pub repeat: bool,                // Tile across entire sheet vs single centered
}
```

- Renders above all schematic content but below selection overlay (z = 14)
- Included in PDF/print export (cannot be removed by the recipient)
- Configurable per project or per document
- Pro edition: watermark auto-set from collaboration review state (draft → in review → released)
- Does NOT affect KiCad file output — watermark is a Signex display/export feature only

**Sprint:** Sprint 3, WS-A (Editor). Simple rendering overlay.

### Mechanical Layer Clearance Rules (G59w)

Design rules that check clearance between objects on mechanical/assembly layers — not just copper layers. Altium supports this for component courtyard checking, but Signex should extend it to all mechanical layers.

**Use cases:**
- Courtyard-to-courtyard clearance (component spacing for assembly)
- Component-to-board-edge on assembly layer (pick-and-place head clearance)
- Keepout on mechanical layer (enclosure interference zones)
- Silkscreen-to-mechanical layer spacing (fiducial clearance zones)
- Mounting hardware clearance (screw head diameter + wrench access)

**Implementation:**
```rust
pub struct MechanicalClearanceRule {
    pub layer_a: LayerId,        // e.g., F.CrtYd (courtyard)
    pub layer_b: LayerId,        // e.g., F.CrtYd or Edge.Cuts
    pub min_clearance: Coord,    // Minimum distance between objects on these layers
    pub scope: RuleScope,        // Global, per-component, per-region
}
```

- DRC checks courtyard overlaps (component-to-component)
- DRC checks courtyard-to-board-edge
- DRC checks objects on user mechanical layers (Dwgs.User, Cmts.User) against each other
- 3D clearance checking (G29) is the full 3D version; this is the fast 2D projection check

**Sprint:** Sprint 4, WS-B (DRC). Extends the clearance rule engine to non-copper layers.

---

## 11-pre-A. Creepage & Clearance Measurement (G59r)

Safety-critical and high-voltage designs require measuring **creepage distance** (shortest path along a surface between two conductors) and **clearance distance** (shortest path through air). These differ because creepage follows the board surface contour around slots, cutouts, and board edges.

**Altium does NOT have a dedicated creepage measurement tool.** Engineers use the Ctrl+M measure tool and manually trace the surface path — tedious and error-prone for complex board shapes.

### Signex Implementation

**Measurement tool:** `Tools > Creepage/Clearance` or shortcut `Shift+M`
- Click two conductors (pads, traces, copper pours) on different nets
- Signex calculates both values:
  - **Clearance** (straight-line air distance): simple Euclidean between closest copper points
  - **Creepage** (surface path): shortest path along PCB surface, routing AROUND board edges, slots, cutouts, and through-holes

**Creepage algorithm:**
1. Build a 2D visibility graph from all board edge segments, slots, cutout edges, and NPTH hole circumferences
2. Source = closest point on conductor A's copper boundary
3. Destination = closest point on conductor B's copper boundary
4. Dijkstra shortest path through the visibility graph (path must stay on the board surface)
5. Display the path as a highlighted line on the canvas with distance annotation

**DRC rule:**
```rust
pub struct CreepageRule {
    pub min_creepage_mm: f64,      // IEC 60950/62368 requirement
    pub min_clearance_mm: f64,     // Air gap requirement
    pub voltage_class: VoltageClass, // Functional, Basic, Reinforced
    pub pollution_degree: u8,      // 1-4 (IEC 60664)
    pub material_group: MaterialGroup, // I, II, IIIa, IIIb
}

pub enum VoltageClass {
    Functional,      // No safety requirement
    Basic,           // Single insulation
    Supplementary,   // Additional insulation
    Reinforced,      // Double insulation (most stringent)
}
```

**Automated checking:** `Design > DRC > Creepage/Clearance Check` runs the creepage calculation for ALL high-voltage net pairs and reports violations. Engineer defines which nets are high-voltage via net class or voltage parameter.

**Visual overlay:** When the creepage tool is active, the shortest creepage path is drawn on the canvas as a colored line. Violations are shown in red with the measured distance vs required distance.

**Standards support:**
- IEC 60950-1 / IEC 62368-1 (IT equipment)
- IEC 60601-1 (medical)
- UL 60950 / UL 62368
- Lookup tables for required distances based on voltage, pollution degree, material group

**Signex advantage:** No other EDA tool does automated creepage measurement. This is critical for power supply, medical device, and industrial equipment design.

**Sprint:** Sprint 5, WS-B (Validation). The visibility graph algorithm goes in `pcb-geom`, the DRC rule in `signex-drc`.

---

## 11-pre-B. Metal Core PCB Stackup (G59s)

Metal Core PCBs (MCPCBs) use an aluminum or copper core substrate instead of FR4. The metal core provides dramatically better thermal conductivity — critical for LED lighting, power electronics, and motor drivers.

### Layer Stack for MCPCB

```
┌──────────────────────────┐
│  Copper (signal layer)   │  35-105µm
├──────────────────────────┤
│  Dielectric (thin)       │  75-200µm (thermally conductive: 1-3 W/mK)
├──────────────────────────┤
│  ████ METAL CORE ████    │  1.0-3.2mm (aluminum 6061 or copper C11000)
├──────────────────────────┤
│  (optional: dielectric)  │  For 2-layer MCPCB
├──────────────────────────┤
│  (optional: copper)      │  For 2-layer MCPCB
└──────────────────────────┘
```

### Implementation in Layer Stack Manager

```rust
pub enum SubstrateType {
    Fr4 { tg: f64 },                    // Standard: Tg 130-180°C
    RogersRf { model: String },          // RF: RO4003C, RO4350B, etc.
    Polyimide,                           // Flex
    MetalCore {
        metal: CoreMetal,
        thickness_mm: f64,               // 1.0-3.2mm
        thermal_conductivity: f64,       // W/(m·K): Al=150-200, Cu=380
    },
    Ceramic { material: String },        // Al₂O₃, AlN (future)
}

pub enum CoreMetal {
    Aluminum,   // 6061-T6, k=167 W/mK, cheap, common for LED
    Copper,     // C11000, k=385 W/mK, expensive, high-power
}
```

**Stackup templates (new):**
- MCPCB 1-Layer (single copper + dielectric + aluminum core)
- MCPCB 2-Layer (copper + dielectric + aluminum core + dielectric + copper)
- MCPCB COB (chip-on-board LED, thermal pad definitions)

**Thermal simulation integration:** MCPCB designs benefit heavily from Elmer thermal simulation — the metal core creates a primary heat path that dominates thermal behavior. The layer stack feeds directly into Elmer geometry generation with correct material properties.

**Sprint:** Sprint 4, WS-E (PCB Core). Extends the existing layer stack data model.

---

## 11-pre-C. Square and Tapered Track Ends (G59t)

Standard PCB traces have round endpoints (semicircle cap). Professional designs sometimes need:

### Square Track Ends
The trace terminates with a flat edge perpendicular to the trace direction (no semicircle). Used for:
- Precise copper geometry on RF structures (microstrip stubs, patch antennas)
- Controlled impedance terminations where round caps add unwanted capacitance
- Pad-to-trace transitions where round overlap is undesirable

### Tapered Track Ends
The trace width smoothly transitions from the normal width to a wider or narrower width at the endpoint. Used for:
- Impedance transitions (gradual width change = less reflection than abrupt)
- Pad necking (wide pad → narrow trace, with gradual taper instead of sharp step)
- RF matching networks (tapered microstrip = broadband impedance transformer)
- Power routing (widen trace at high-current junction points)

### Implementation

```rust
pub enum TrackEndStyle {
    Round,           // Default: semicircle cap (standard PCB trace)
    Square,          // Flat perpendicular edge (RF, precise geometry)
    Tapered {
        end_width: f64,  // Width at the endpoint (wider or narrower than trace)
        taper_length: f64, // Length of the taper region
    },
}

pub struct PcbTrack {
    pub start: Vec2,
    pub end: Vec2,
    pub width: f64,
    pub layer: LayerId,
    pub net: NetId,
    pub start_style: TrackEndStyle,  // NEW
    pub end_style: TrackEndStyle,    // NEW
}
```

**UI:** During routing, right-click → Track End Style submenu. Or set per net class in Constraint Manager.

**Rendering:** Square ends use a rectangle instead of rounded rect. Tapered ends use a trapezoid path.

**Gerber output:** Square ends use the rectangle aperture. Tapered ends decompose into polygon primitives.

**Sprint:** Sprint 4, WS-D (Router) for UI/routing integration + WS-E (PCB Core) for rendering.

---

## 11-pre-D. DRC Rule Profiles — Save/Load Rule Sets (G59n)

### The Problem

Engineers design boards for different fabricators (JLCPCB, PCBWay, Eurocircuits, advanced fabs). Each fab has different capabilities: min trace width, min drill, min annular ring, min spacing. Currently, engineers manually reconfigure DRC rules for each fab — tedious and error-prone.

### Signex Solution: Named Rule Profiles

```rust
pub struct DrcRuleProfile {
    pub name: String,                    // "JLCPCB Standard", "PCBWay 4-Layer", "Advanced HDI"
    pub description: String,
    pub manufacturer: Option<String>,    // "JLCPCB", "PCBWay", etc.
    pub rules: Vec<DrcRule>,             // All rule values for this profile
    pub last_updated: chrono::NaiveDate,
}
```

**Built-in profiles (ship with Signex):**

| Profile | Min Trace | Min Space | Min Drill | Min Annular | Min Via | Layers |
|---|---|---|---|---|---|---|
| JLCPCB Standard | 5mil | 5mil | 0.3mm | 0.13mm | 0.45mm | 1-6 |
| JLCPCB Advanced | 3.5mil | 3.5mil | 0.2mm | 0.1mm | 0.35mm | 1-14 |
| PCBWay Standard | 5mil | 5mil | 0.3mm | 0.15mm | 0.5mm | 1-6 |
| PCBWay HDI | 3mil | 3mil | 0.1mm (laser) | 0.075mm | 0.25mm | 4-20 |
| OSH Park 2-Layer | 6mil | 6mil | 0.254mm | 0.127mm | 0.508mm | 2 |
| Eurocircuits Standard | 150µm | 150µm | 0.3mm | 0.15mm | 0.5mm | 1-8 |
| IPC Class 2 (default) | 5mil | 5mil | 0.25mm | 0.125mm | 0.45mm | any |
| IPC Class 3 (aerospace) | 4mil | 4mil | 0.2mm | 0.1mm | 0.35mm | any |

**Features:**
- **Save current rules as profile:** `Design > DRC > Save Profile As...` — saves all current DRC rule values to a `.snxdrc` JSON file
- **Load profile:** `Design > DRC > Load Profile...` — replaces all rule values from a profile file
- **Profile selector in DRC panel:** Dropdown at top of DRC panel — switch profiles instantly
- **Profile diff:** When switching profiles, show what changes (highlight rules that differ)
- **Custom profiles:** Engineer creates profiles for their specific fabs and shares with team
- **Pro: Cloud profiles:** Pro edition syncs profiles via Supabase — team shares fab-specific rules
- **Import from fab:** Future: import capabilities directly from fab website/API (JLCPCB has a capabilities page)

**Sprint:** Sprint 4, WS-B (DRC). Rule profiles are serialization + UI on top of the existing rule engine.

---

## 11-pre-2. Native Altium Import (G59o — CRITICAL)

This is the most important import feature for Signex adoption. Engineers migrating from Altium ($10K+/seat) need to bring their existing designs.

### Altium File Formats

| Extension | Type | Format | Priority |
|---|---|---|---|
| `.SchDoc` | Schematic | OLE Compound Document (binary) | **P0 — must have** |
| `.PcbDoc` | PCB layout | OLE Compound Document (binary) | **P0 — must have** |
| `.SchLib` | Symbol library | OLE Compound Document (binary) | **P1 — important** |
| `.PcbLib` | Footprint library | OLE Compound Document (binary) | **P1 — important** |
| `.PrjPcb` | Project file | INI-like text | **P0 — must have** |
| `.IntLib` | Integrated library | ZIP archive of SchLib + PcbLib | P2 |
| `.OutJob` | Output job | XML | P3 |
| `.BomDoc` | BOM Studio | XML | P3 |
| `.RuleDoc` | Design rules | XML | P2 |
| `.Harness` | Harness definition | Text | P2 |

### Parsing Strategy

Altium's `.SchDoc` and `.PcbDoc` are **OLE Compound Documents** (same container format as old .doc/.xls). Inside the OLE container, the design data is stored as binary records. The record format is partially documented by community reverse-engineering efforts.

**Existing open-source parsers to reference:**
- `nicedrop/altium-pars` (Rust) — partial SchDoc parser
- `altium-rs` on crates.io — incomplete but has OLE record structures
- `PyAltium` (Python) — mature SchDoc + PcbDoc parser
- KiCad's Altium importer (`kicad/pcbnew/plugins/altium/`) — C++, most complete

**Implementation plan:**
1. Use `cfb` crate (Rust) to read OLE Compound Document container
2. Parse binary record stream inside each OLE storage
3. Map Altium objects to `signex-types` domain types
4. Handle Altium-specific features: multi-channel UIDs, variant states, harness definitions
5. Convert coordinate system: Altium IU (0.1mil = 2.54nm) → Signex IU (1nm)

**What must import correctly:**
- All component placements with designators, values, footprints, fields
- All wires, buses, net labels, power ports, junctions
- Sheet hierarchy (sheet symbols → child sheets)
- Multi-channel Repeat definitions
- Design variants (fitted/not-fitted/alternate states)
- PCB traces, vias, pads with correct geometry and net assignment
- Copper pours with net, priority, thermal settings
- Board outline and cutouts
- Layer stackup (copper weights, dielectric properties)
- Design rules (clearance, width, via, etc.)
- 3D model references (STEP paths)

**What may require manual fixup after import:**
- Font rendering differences (Altium uses different font metrics)
- Complex custom pad shapes (Altium has very flexible pad stacks)
- Embedded 3D models (need to re-link to STEP files)
- Scripting/automation (DelphiScript → WASM plugin migration)

**Sprint:** Sprint 2, WS-I (Import). The `cfb` crate handles OLE. Record parsing is the main effort.

---

## 11-pre-3. Plated Board Cutouts & Castellated Holes (G59p, G59q)

### Plated Board Cutouts (G59p)

Internal plated slots and cutouts with copper plating on the cut edge. Different from regular board cutouts (which are NPTH).

**Use cases:**
- RF shielding can fence (plated slot around RF section, soldered to shield can)
- Connector through-holes that are larger than a standard pad
- Internal USB-C receptacle cutouts with ground connection on edge
- Thermal vias in an array forming a plated trench

**Implementation:**
```rust
pub struct PlatedCutout {
    pub outline: Vec<Vec2>,     // Closed polygon defining the cutout shape
    pub net: Option<NetId>,     // Net connection for the plating (usually GND)
    pub plating_thickness: f64, // Copper thickness on cut wall (µm)
    pub layers: LayerRange,     // Through-all or partial (blind cutout)
}
```

- Drawn on a dedicated sublayer of Edge.Cuts with a "plated" flag
- DRC checks clearance to plated cutout edge as copper (not as board edge)
- Gerber output: cutout outline on Edge.Cuts layer + plating on copper layers
- Drill output: routed slot path with plated flag
- 3D viewer: cutout shows as copper-plated wall

### Castellated Holes (G59q)

Half-plated through-holes on the board edge — the board outline cuts through the center of a plated via, leaving a semicircular copper surface on the edge. Used for module-to-module soldering (e.g., WiFi modules, SoM connections).

**Implementation:**
```rust
pub struct CastellatedHole {
    pub position: Vec2,          // Center of the full hole (on board edge)
    pub drill_diameter: f64,     // Full hole diameter before cutting
    pub net: NetId,              // Net connection
    pub pad_on_layers: Vec<LayerId>, // Which layers have pads (usually all copper)
}
```

- Place a standard plated via/pad ON the board edge
- The board outline cuts through the pad center
- Signex automatically detects pads intersecting Edge.Cuts and marks them as castellated
- DRC: castellated holes exempt from board-edge clearance rule
- Gerber: pad appears on copper layers, drill file includes the hole, outline cuts through it
- 3D viewer: shows half-cylinder copper surface on board edge
- BOM/assembly: castellated holes listed in a special section for manufacturing notes

**Footprint library support:**
Castellated-edge footprints (e.g., for designing modules that WILL be soldered via castellations):
- Pads at board edge with `castellated: true` flag
- When footprint is placed, pads auto-align to Edge.Cuts

**Sprint:** Sprint 4, WS-E (PCB Core). These are geometry features in `pcb-geom` + rendering in `signex-render/pcb`.

---

## 11a. True Variants in Multi-Channel Designs (G59m)

### The Problem in Altium

Altium's variant system applies uniformly across all channel instances. If you have a 4-channel audio amplifier using `Repeat(CH, 1, 4)`, and you create a variant that marks the output filter capacitor as "Not Fitted", it applies to ALL 4 channels. You cannot make a variant where channels 1-2 are fully populated but channels 3-4 have the filter removed.

This is a real limitation. Engineers work around it by:
- Creating separate sheet symbols instead of Repeat (losing multi-channel benefits)
- Using compile masks to hide channels (hacky, error-prone)
- Maintaining separate projects for different channel configs

### Signex Solution: Per-Channel Variant State

Signex extends the variant system so that variant states can be scoped to individual channels:

```rust
pub enum VariantScope {
    /// Classic: applies to all instances of this component across all channels.
    /// "C12 is Not Fitted" means C12_CH1, C12_CH2, C12_CH3, C12_CH4 are all DNP.
    Global,

    /// Per-channel: variant state is independent per channel instance.
    /// "C12 in CH3 is Not Fitted" means only C12_CH3 is DNP; CH1, CH2, CH4 keep it.
    PerChannel {
        /// Map from channel identifier to component state.
        /// Channels not listed inherit the global state.
        channel_overrides: HashMap<String, ComponentVariantState>,
    },
}

pub struct ComponentVariantState {
    pub fitted: FitState,                    // Fitted, NotFitted
    pub alternate_value: Option<String>,     // Override value for this channel
    pub alternate_footprint: Option<String>, // Override footprint for this channel
    pub parameter_overrides: HashMap<String, String>,
}
```

### Use Cases

| Scenario | Classic Altium | Signex Per-Channel |
|---|---|---|
| 8-ch ADC, channels 5-8 are optional SKU | Cannot do — need 2 separate projects | One variant: CH5-CH8 not fitted |
| 4-ch audio amp, CH3 has different gain | Cannot vary per channel | CH3 gets alternate resistor values |
| MIMO antenna, 2x2 variant of 4x4 design | Manual workaround | Variant: antenna chains 3-4 not fitted |
| Motor controller, 3-phase vs single-phase | Separate designs | Variant: phases B/C not fitted |
| DDR dual-rank vs single-rank | Separate designs | Variant: rank 2 components not fitted |

### Variant Manager UI

The Variant Manager spreadsheet adds a channel dimension:

```
┌──────────────┬────────────┬──────────┬──────────┬──────────┬──────────┐
│ Component    │ Base       │ SKU-Full │ SKU-2CH  │ SKU-Lite │          │
├──────────────┼────────────┼──────────┼──────────┼──────────┼──────────┤
│ U1 (all CH)  │ Fitted     │ Fitted   │ Fitted   │ Fitted   │ Global   │
│ C12 (CH1)    │ Fitted     │ Fitted   │ Fitted   │ Fitted   │          │
│ C12 (CH2)    │ Fitted     │ Fitted   │ Fitted   │ Not Fit  │ Per-CH   │
│ C12 (CH3)    │ Fitted     │ Fitted   │ Not Fit  │ Not Fit  │ Per-CH   │
│ C12 (CH4)    │ Fitted     │ Fitted   │ Not Fit  │ Not Fit  │ Per-CH   │
│ R5 (CH3)     │ 10kΩ       │ 10kΩ     │ —        │ —        │ Per-CH   │
│ R5 (CH3 alt) │            │          │          │ 4.7kΩ    │ Alternate│
└──────────────┴────────────┴──────────┴──────────┴──────────┴──────────┘
```

Right-click a component row → "Apply per-channel" expands it into per-channel rows.

### BOM Output

Variant-specific BOM respects per-channel states:
- SKU-Full BOM: all components across all 4 channels
- SKU-2CH BOM: C12 × 2 (CH1-CH2 only), all other components × 2
- SKU-Lite BOM: C12 × 1 (CH1 only), R5 shows 4.7kΩ for CH3

### Canvas Visualization

See variant drawing styles below (G59m visual).

### Altium Competitive Position

**This feature does not exist in any EDA tool.** Altium, KiCad, OrCAD, PADS — none support per-channel variant states. It's the #1 feature request from multi-channel designers. Signex implementing this is a significant competitive advantage.

**Sprint:** Sprint 3, WS-A (Core Editing). Variant data model must support per-channel scoping from the start. The Variant Manager UI expands in Sprint 3.

---

## 11a-2. Variant Drawing Styles (Enhanced Visualization)

Altium shows not-fitted components with a simple red X overlay. Signex provides more options for clarity, especially in dense designs and when presenting to non-engineers (mechanical, manufacturing).

### Drawing Style Options

| Style | Visual | Best For |
|---|---|---|
| **Cross-out** (Altium default) | Red X over component body | Quick visual, familiar to Altium users |
| **Ghosted** | Component drawn at 20% opacity, grayed out | Dense designs — DNP parts fade into background |
| **Strikethrough** | Diagonal lines (hatching) over component area | Printable — visible on B&W prints |
| **Dotted outline** | Body outline changes to dotted line, fill removed | Subtle — shows the component is "optional" |
| **Color tint** | Entire component tinted with variant color (configurable) | Color-coded by variant — see multiple variants at once |
| **Hidden** | Component completely hidden from canvas | Clean schematic for manufacturing review — only fitted parts visible |
| **Badge** | Small colored dot/tag at component corner with variant code | Minimal — doesn't obscure the schematic |

### Configuration

```
Preferences > Schematic > Variants:
  ├── Not Fitted style:     [dropdown: Cross-out / Ghosted / Strikethrough / 
  │                          Dotted / Color tint / Hidden / Badge]
  ├── Not Fitted color:     [color picker, default: #FF3333]
  ├── Not Fitted opacity:   [slider 0-100%, default: 20% for Ghosted]
  ├── Alternate style:      [dropdown: same options]
  ├── Alternate color:      [color picker, default: #FF8800]
  ├── Show variant name:    [checkbox — show variant name badge on affected components]
  ├── Per-channel indicator: [checkbox — show channel identifier on per-channel overrides]
  └── Variant comparison:   [checkbox — side-by-side overlay of two variants]
```

### Variant Comparison Mode

A split or overlay view showing two variants simultaneously:

```
┌─────────────────────┬─────────────────────┐
│    SKU-Full          │    SKU-Lite          │
│                      │                      │
│  ┌──────┐  ┌──────┐ │  ┌──────┐  ┌ ─ ─ ┐ │
│  │  U1  │──│ C12  │ │  │  U1  │──  C12   │  ← Ghosted (DNP)
│  └──────┘  └──────┘ │  └──────┘  └ ─ ─ ┘ │
│       │              │       │              │
│  ┌──────┐            │  ┌──────┐            │
│  │ R5   │ 10kΩ       │  │ R5   │ 4.7kΩ     │  ← Alternate value (orange tint)
│  └──────┘            │  └──────┘            │
│                      │                      │
└─────────────────────┴─────────────────────┘
```

This lets the engineer see exactly what changes between variants without switching back and forth.

### PCB Variant Visualization

On the PCB, variant styles apply to footprints:

| Style | PCB Visual |
|---|---|
| Cross-out | Red X over courtyard |
| Ghosted | Pads at 15% opacity, silkscreen hidden |
| Hidden | Footprint completely hidden (copper remains for thermal pad) |
| Color tint | Pads/silk tinted with variant color |
| Badge | Small colored marker at component origin |

Assembly drawing output (PDF/SVG) respects the selected variant — only fitted components appear.

### Signex Advantage

| Feature | Altium | Signex |
|---|---|---|
| DNP visual style | Red X only | 7 styles (cross/ghost/stripe/dot/tint/hide/badge) |
| Configurable DNP color | No | Yes, per variant |
| Alternate value visual | Italic text | Color tint + badge + italic |
| Per-channel indication | N/A (no per-channel variants) | Channel identifier badge |
| Variant comparison | Manual toggle | Side-by-side or overlay view |
| Assembly drawing respects variant | Yes | Yes + style-aware rendering |

**Sprint:** Sprint 3, WS-A (Core Editing). The drawing styles are rendering options in `signex-render/schematic/symbol.rs` and `signex-render/pcb/footprint.rs`. Variant comparison mode is a canvas overlay feature.

---

## 11b. Native Metric and Native Inch Support (G59l)

This is a foundational architecture decision that must be made in WS-K (Parser & Types) during Sprint 1.

### The Problem

- **KiCad** stores coordinates in metric (1 internal unit = 1 nanometer)
- **Altium** stores coordinates in imperial (1 internal unit = 1/10000 mil = 0.1 mil)
- Engineers work in either mm or mil depending on region and industry
- Mixing systems causes rounding errors that accumulate over large boards
- Grid presets must snap cleanly in both systems: 0.1mm grid must land on exact values, 1mil grid must also land on exact values

### Signex Approach: Dual-Native Coordinates

Signex stores coordinates in a **high-resolution integer** that is exact in both metric and imperial:

```rust
/// Internal coordinate unit.
/// 1 IU = 1 nanometer (same as KiCad).
/// 1 mil = 25400 nm = 25400 IU (exact integer, no rounding).
/// 1 mm  = 1000000 nm = 1000000 IU (exact integer).
///
/// This means BOTH metric and imperial values are exact — no float drift.
pub type Coord = i64;  // nanometers, range ±9.2 million meters

pub const NM_PER_MIL: i64 = 25_400;
pub const NM_PER_MM: i64 = 1_000_000;
pub const NM_PER_INCH: i64 = 25_400_000;
```

This is the same approach KiCad uses, and it works because 1 mil = exactly 25,400 nm — an integer. No floating-point conversion needed.

### What This Means for the User

| Setting | Behavior |
|---|---|
| **Document unit mode** | Per-document: Metric (mm) or Imperial (mil). Affects display, grid presets, default values. |
| **Grid presets (metric)** | 0.1mm, 0.25mm, 0.5mm, 1mm, 2.54mm (100mil-compatible) |
| **Grid presets (imperial)** | 1mil, 5mil, 10mil, 25mil, 50mil, 100mil |
| **Mixed-unit entry** | Type `10mm` or `100mil` in any numeric field — auto-converts to document unit |
| **Status bar** | Shows cursor in document unit. Ctrl+Q cycles mm/mil/inch display. |
| **Rule entry** | Enter `6mil` for clearance even in a metric document — stored as 152400 nm exactly |
| **KiCad import** | Metric coordinates import without rounding (same nm base) |
| **Altium import** | Imperial coordinates convert exactly: 1 Altium IU = 2.54 nm |
| **Coordinate display** | Metric: 2 decimal places (0.01mm = 10000nm). Imperial: 1 decimal (0.1mil = 2540nm). |

### Grid System

```rust
pub enum GridUnit {
    Metric,   // grid values in mm
    Imperial, // grid values in mil
}

pub struct GridConfig {
    pub unit: GridUnit,
    pub size: Coord,          // grid spacing in nm
    pub visible: bool,
    pub snap_enabled: bool,
    pub electrical_snap: bool, // snap to pins regardless of grid
}

// Metric presets (all exact in nm)
pub const GRID_METRIC: &[Coord] = &[
    100_000,     // 0.1mm
    250_000,     // 0.25mm
    500_000,     // 0.5mm
    1_000_000,   // 1.0mm
    2_540_000,   // 2.54mm (= 100mil, compatible)
];

// Imperial presets (all exact in nm)
pub const GRID_IMPERIAL: &[Coord] = &[
    25_400,      // 1mil
    127_000,     // 5mil
    254_000,     // 10mil
    635_000,     // 25mil
    1_270_000,   // 50mil
    2_540_000,   // 100mil
];
```

### Why This Matters

Most EDA tools pick one system and convert the other with floating-point, causing:
- 100mil grid point at 2.54000000000000003mm (float drift)
- Accumulated error after many operations
- Coordinates that don't round-trip exactly between save/load

Signex avoids this entirely by using integer nanometers — both 1mm and 1mil are exact integer multiples of 1nm.

**Sprint:** Sprint 1, WS-K (Types). The `Coord` type and unit system must be defined before any parser or rendering code is written.

---

## 12. Schematic Tables & Table of Contents (G59j, G59k)

### Tables on Schematic Canvas

A placeable, editable table object directly on the schematic canvas. Altium has a basic "Table" object and Canvas Docs tables, but Signex can do this significantly better.

**Placement:** `Place > Table` or shortcut. Click to place, drag to size. The table is a first-class schematic object — selectable, movable, rotatable, copy/pasteable, undoable.

**Table types (built-in templates):**

| Template | Auto-populates from | Use case |
|---|---|---|
| **Pin Assignment** | Selected IC symbol | Shows all pins: number, name, function, net, direction. Drag from component → table auto-fills. |
| **Connector Pinout** | Selected connector symbol | Pin number, signal name, direction, voltage, notes. Essential for cable/harness docs. |
| **Register Map** | User-defined or imported CSV | Address, name, bit fields, reset value, R/W, description. Common for FPGA/MCU datasheets. |
| **Revision History** | Document parameters | Rev, date, author, description. Standard title block companion. |
| **Test Point List** | ERC/DRC data | TP designator, net, probe side, location. Auto-generated after annotation. |
| **BOM Summary** | Component data | Condensed on-sheet BOM for simple designs. Groups by value + footprint. |
| **Power Budget** | User-defined | Rail, voltage, current, source, load components. Manual or AI-assisted. |
| **Net Summary** | Net resolver | Net name, connected pins, net class, diff pair, impedance target. |
| **Custom** | Empty | User defines rows, columns, content. Free-form. |

**Table features:**
- **Cell editing:** Double-click cell to edit. Rich text support (bold, italic, subscript via markup).
- **Column resize:** Drag column borders. Auto-fit to content.
- **Row/column add/remove:** Right-click context menu.
- **Cell merge:** Select cells → merge (for headers).
- **Borders:** Configurable per cell (solid, dashed, none, thick). Outer border thicker by default.
- **Fill color:** Per-cell or per-row alternating (zebra striping) for readability.
- **Header row:** Bold, centered, with bottom border. Optionally frozen during scroll.
- **Font:** Inherits from schematic theme. Override per cell possible.
- **Auto-populate:** Pin Assignment and Connector Pinout tables link to a symbol — when the symbol changes, the table updates.
- **Sort:** Click column header to sort rows (ascending/descending/original).
- **CSV import/export:** Right-click table → Import CSV / Export CSV. Paste from spreadsheet.
- **Theming:** Table colors follow the active Signex theme (border, fill, text colors from theme tokens).

**Data binding (Signex advantage — Altium doesn't have this):**
A table can be **bound to a data source** — when the source changes, the table updates automatically:
- Bound to component → pin table updates when pins are added/removed
- Bound to net class → net summary updates when nets are assigned
- Bound to BOM → summary updates when components change
- Bound to ERC results → violation summary updates after each ERC run

**Rendering:**
- Tables render on the schematic canvas at the same z-level as text annotations (z=12)
- Table cells support the rich label markup system (`V_{CC}`, `~{CS}`, etc.)
- Tables are included in PDF export, print, and visual diff

**Storage:**
Saved in `.kicad_sch` as a `(signex_table ...)` extension node (same pattern as formula annotations):
```
(signex_table
  (at 120 80)
  (columns 4)
  (rows 6)
  (column_widths 15 30 20 40)
  (header_row yes)
  (template "pin_assignment")
  (bound_to "U1")
  (cells
    (cell (row 0) (col 0) (text "Pin") (bold yes))
    (cell (row 0) (col 1) (text "Name") (bold yes))
    (cell (row 0) (col 2) (text "Type") (bold yes))
    (cell (row 0) (col 3) (text "Net") (bold yes))
    (cell (row 1) (col 0) (text "1"))
    (cell (row 1) (col 1) (text "VCC"))
    (cell (row 1) (col 2) (text "Power"))
    (cell (row 1) (col 3) (text "3V3"))
    ...
  )
)
```

### Table of Contents (Auto-Generated)

An auto-generated index page for multi-sheet schematic designs. Placed as the first sheet or as a dedicated ToC sheet.

**Content:**
```
┌─────────────────────────────────────────────────────┐
│  TABLE OF CONTENTS                                  │
│                                                     │
│  Sheet  Title                              Page     │
│  ─────  ─────────────────────────────────  ────     │
│  1      Top Level                          1        │
│  2      Power Supply (3V3 LDO)             2        │
│  3      Power Supply (1V2 Buck)            3        │
│  4      MCU (STM32H743)                    4        │
│  5      DDR4 Memory (2x 4Gbit)             5        │
│  6      Ethernet PHY (KSZ9031)             6        │
│  7      USB Hub (USB2514B)                 7        │
│  8      Debug & Programming (SWD)          8        │
│  9      Connectors & I/O                   9        │
│                                                     │
│  Revision History                                   │
│  Rev  Date        Author    Description             │
│  ───  ──────────  ────────  ──────────────────────  │
│  A    2026-03-15  C.Alp     Initial release         │
│  B    2026-04-01  C.Alp     Added USB hub           │
│  C    2026-04-10  C.Alp     DDR4 routing fixes      │
└─────────────────────────────────────────────────────┘
```

**Features:**
- **Auto-generation:** `Design > Generate Table of Contents` creates/updates the ToC sheet
- **Live update:** ToC refreshes when sheets are added, removed, renamed, or reordered
- **Clickable navigation:** In the application, clicking a ToC row navigates to that sheet (like bookmarks)
- **Revision history table:** Optionally includes a revision history section from document/project parameters
- **Sheet numbering:** Sequential or hierarchical (1.1, 1.2 for sub-sheets)
- **Block diagram (optional):** Auto-generated simplified block diagram showing sheet hierarchy with connections. Each block is clickable for navigation.
- **PDF bookmarks:** When exporting PDF, the ToC entries become PDF bookmarks for navigation
- **Template:** ToC layout follows the active sheet template (title block, border, font)
- **Custom columns:** Add columns like "Engineer", "Status" (Draft/Review/Released), "Description"

**Block diagram on ToC (Signex advantage):**
```
┌──────────┐     ┌──────────┐     ┌──────────┐
│  Power   │────→│   MCU    │────→│   DDR4   │
│  Supply  │     │ STM32H7  │     │  Memory  │
│ (Sh 2-3) │     │  (Sh 4)  │     │  (Sh 5)  │
└──────────┘     └────┬─────┘     └──────────┘
                      │
              ┌───────┼───────┐
              │       │       │
         ┌────▼──┐ ┌──▼───┐ ┌▼────────┐
         │  ETH  │ │ USB  │ │  Debug  │
         │  PHY  │ │ Hub  │ │   SWD   │
         │(Sh 6) │ │(Sh 7)│ │ (Sh 8)  │
         └───────┘ └──────┘ └─────────┘
```

This is auto-generated from the sheet symbol hierarchy. Each block is a clickable navigation target. The diagram updates when the hierarchy changes. Altium has nothing like this — their ToC is a static list in Canvas Docs.

**Sprint:** Sprint 3 (WS-A Core Editing). Tables are a drawing primitive — they fit alongside text frames and notes. ToC is a project-level feature tied to multi-sheet navigation.

---

## 13. Signex Advantages — Features Altium Doesn't Have

These are value-adds that make Signex better than Altium, not just equivalent.

### Already Planned

| Feature | Why It Matters | Phase |
|---|---|---|
| **Signal AI copilot** (Claude) | No EDA tool has native LLM integration. Design review, ERC fix suggestions, component recommendations, circuit template generation, visual context reasoning. | Phase 12 (Pro) |
| **RF/EM FDTD simulation** (OpenEMS) | Altium has no built-in EM solver. S-parameters, TDR, crosstalk analysis on actual copper geometry. | Phase 11 |
| **Thermal FEM simulation** (Elmer) | Altium has no thermal solver. Temperature maps, DC IR drop, Joule heating, current density visualization on 3D PCB model. | Phase 11 |
| **DC IR drop analysis** (Elmer) | Altium has a simplified power plane analyzer. Elmer gives full FEM with voltage drop overlay and current density vectors. | Phase 11 |
| **WASM plugin system** (Extism) | Altium uses legacy DelphiScript/JScript. WASM lets plugins be written in Rust/C/Go/Python/JS, sandboxed, hot-loadable, with permission gateway. | Phase 13 |
| **6 built-in themes** + theme editor | Altium has 2 themes (light/dark) with no editor. Signex has Catppuccin Mocha, VS Code Dark, GitHub Dark, Altium Dark, Solarized Light, Nord + full custom editing. | Phase 0 |
| **Real-time co-editing** (Supabase) | Altium 365 is sequential handoff. Signex Pro has true real-time concurrent editing with per-user cursors, CRDT sync, region/layer/net locking. | Phase 15 (Pro) |
| **Open source core** (GPL-3.0) | Altium is $10K+/seat proprietary. Signex Community is free with full schematic/PCB/sim capabilities. | Always |
| **KiCad native format** | Altium uses proprietary binary formats. Signex reads/writes KiCad format natively — the world's most popular open EDA format. | Phase 1 |
| **Formula annotations** (LaTeX) | No EDA tool supports LaTeX math on the schematic canvas. Engineers can annotate transfer functions, impedance equations, filter responses directly on the design. | Phase 3 |

### New Ideas — Beyond Altium

| # | Feature | Description | Value | Workstream |
|---|---|---|---|---|
| **S1** | **AI design review on commit** | When committing to version control, Signal AI automatically reviews changes: checks for common mistakes, suggests improvements, flags potential SI/PI issues. Like a code review bot for hardware. | Catches errors before they reach fabrication. Altium has nothing like this. | Pro (Signal) |
| **S2** | **Component risk dashboard** | Live panel showing: single-source components, long lead-time parts, recently-EOL'd parts, price volatility alerts. Pulls from supplier APIs + Signal AI analysis. | Supply chain awareness without leaving the editor. Altium's BOM Studio is static — this is proactive. | Pro (Library) |
| **S3** | **Schematic diff with visual overlay** | Side-by-side or overlay diff of two schematic revisions. Green = added, red = removed, blue = modified. Works on canvas, not just text diff. Altium 365 has web-based diff; Signex does it in-app. | Faster design review. No need to open a browser. | WS-A (Editor) |
| **S4** | **PCB diff with visual overlay** | Same as S3 but for PCB. Overlay two board revisions showing moved components, changed traces, modified pours. | Critical for reviewing PCB changes before re-fabrication. | WS-E (PCB Core) |
| **S5** | **AI-guided routing suggestions** | Signal AI analyzes the ratsnest and suggests routing order, layer assignments, and via placement strategies for complex buses (DDR, PCIe, USB). Shows a suggested route path overlay before the engineer starts routing. | Reduces routing time for complex high-speed designs. No EDA tool does this. | Pro (Signal + Router) |
| **S6** | **Interactive impedance calculator on canvas** | Hover over a trace → see its impedance based on current width + layer stack. Adjust width in-place to hit target impedance. Real-time feedback, not a separate dialog. | Engineers currently switch between calculator and editor. This eliminates the context switch. | WS-E (PCB Core) |
| **S7** | **Thermal-aware routing** | After running Elmer thermal sim, highlight traces that carry high current and are near thermal hotspots. Signal AI suggests wider traces or additional vias for thermal relief. | Integrates thermal analysis into the routing workflow. No EDA tool connects thermal sims to routing decisions. | WS-J (Sim) + Pro (Signal) |
| **S8** | **BOM cost optimization** | Signal AI analyzes BOM and suggests: cheaper compatible alternatives, consolidation (fewer unique parts), multi-source alternatives for single-source risk. Shows projected cost savings. | Direct cost reduction. Altium shows prices but doesn't optimize. | Pro (Signal + Library) |
| **S9** | **Design intent documentation** | Signal AI reads the schematic and auto-generates a design description document: block diagram, signal flow description, power architecture summary, critical net identification. Exports as markdown/PDF. | Saves hours of documentation work. Useful for design reviews, handoffs, and regulatory submissions. | Pro (Signal) |
| **S10** | **Cross-probe to simulation** | Click a net in the schematic → see its SPICE waveform. Click a trace in PCB → see its S-parameter. Click a component → see its temperature from thermal sim. One-click from design to analysis. | Eliminates the "run sim, find result, correlate to design" cycle. | WS-J (Sim) |
| **S11** | **Smart snap points** | During routing, show snap indicators not just at grid points but at impedance-optimal widths, matched-length target points, and diff-pair symmetry points. The cursor "magnetizes" to good design positions. | Reduces errors from manual impedance/length calculations. | WS-D (Router) |
| **S12** | **Component placement heatmap** | After importing netlist, show a heatmap of connectivity density. Hot zones = many connections between components in that area. Guides initial placement before routing. | Better initial placement = easier routing. No EDA tool visualizes placement quality this way. | WS-E (PCB Core) |
| **S13** | **Collaborative design review with annotations** | Reviewers can draw freehand annotations (circles, arrows, text) on schematic/PCB that persist as a review layer. Like drawing on a printout but digital and version-tracked. | Replaces the "print, markup with red pen, scan" workflow. Altium 365 has comments but not freehand markup. | Pro (Collab) |
| **S14** | **One-click manufacturing package** | Single button generates: Gerber, drill, BOM, pick-and-place, assembly drawing, 3D STEP, stackup report — all in a ZIP with standardized naming. Pre-validated against common fab house requirements (JLCPCB, PCBWay, OSH Park). | Eliminates the "did I include everything?" problem. Altium's OutJob requires manual configuration. | WS-C (Output) |
| **S15** | **Live DRC during wire drawing** | In the schematic, show ERC violations in real-time as the engineer draws wires. Don't wait for a manual ERC run. A red indicator appears instantly when connecting incompatible pins. | Catches errors at the moment they're made, not after the fact. | WS-B (Validation) |
| **S16** | **Natural language constraint entry** | In the Constraint Manager, type "DDR data group should be matched within 50 mils on inner layers" and Signal AI translates it to the correct constraint configuration. | Eliminates the need to learn complex rule syntax. | Pro (Signal) |
| **S17** | **Stackup wizard with material recommendations** | Instead of manually building a stackup, describe the design requirements ("4-layer, 50Ω microstrip on outer, 100Ω diff pair, 1.6mm total") and the wizard suggests materials, thicknesses, and copper weights. | Eliminates guesswork in stackup design. Currently requires experience or a separate calculator. | WS-E (PCB Core) |
| **S18** | **Data-bound schematic tables** | Pin assignment, register map, connector pinout, power budget tables that auto-update when the linked component changes. CSV import/export. Altium tables are static. | Engineers maintain pin tables manually — data binding eliminates drift between table and design. | WS-A (Editor) |
| **S19** | **Auto-generated ToC with block diagram** | Multi-sheet designs get a clickable table of contents + auto-generated hierarchy block diagram. Click any block → navigate to that sheet. | No EDA tool auto-generates navigable block diagrams from the sheet hierarchy. | WS-A (Editor) |
| **S20** | **Register map import** (SVD/IP-XACT/CSV) | Import MCU/FPGA register maps from ARM CMSIS-SVD, IP-XACT XML, or CSV. Auto-generate pin assignment table and IO constraint documentation. | Eliminates manual transcription from datasheet to schematic. Huge time saver for complex ICs. | WS-A (Editor) |
| **S21** | **Via impedance + delay + stub resonance** | Hover via on PCB → see Z₀, propagation delay (ps), stub resonant frequency. No EDA tool shows this. Analytical formula for quick estimates, OpenEMS full-wave for accurate extraction. Feeds directly into length matching (via delay counted in matched length). | Altium counts via delay as a rough estimate from via height. Signex gives actual impedance, accurate delay, and warns when stub resonance limits bandwidth. | WS-J (Sim) + WS-F (HS) |

---

## GAP Summary — Items to Add to Plan

### Critical (must have for v1.0)

| # | Feature | Category | Workstream |
|---|---|---|---|
| G1 | Measure distance tool (Ctrl+M) | SCH/PCB | WS-A (Editor) |
| G2 | Rubber stamp paste (Ctrl+R) | SCH | WS-A (Editor) |
| G3 | Bezier curve drawing | SCH | WS-A (Editor) |
| G4 | Schematic dimension annotation | SCH | WS-A (Editor) |
| G5 | Project compilation pass (full multi-sheet) | SCH | WS-B (Validation) |
| G6 | Net scope rule enforcement | SCH | WS-B (Validation) |
| G7 | PCB-level annotation (by board position) | SCH/PCB | WS-B (Validation) |
| G8 | Variant-specific BOM export | SCH | WS-C (Output) |
| G9 | Variant visual indication on canvas | SCH | WS-A (Editor) |
| G10 | Route completion (loop removal) | PCB | WS-D (Router) |
| G11 | Glossing (post-route optimization, 3 efforts) | PCB | WS-D (Router) |
| G12 | Impedance-controlled routing (Z₀ display, width-from-impedance) | PCB | WS-D (Router) |
| G13 | Interactive length gauge during routing | PCB | WS-D (Router) |
| G14 | Split planes (negative plane layers) | PCB | WS-E (PCB Core) |
| G15 | Via-in-pad | PCB | WS-E (PCB Core) |
| G16 | Paste expansion rule (per pad/per rule) | PCB | WS-E (PCB Core) |
| G17 | Mask expansion rule (per pad/per rule) | PCB | WS-E (PCB Core) |
| G18 | Layer stack editor (full, with εr/tanδ/copper weight/material library) | PCB | WS-E (PCB Core) |
| G19 | Impedance profile per layer pair + built-in calculator | PCB | WS-E (PCB Core) |
| G20 | Stackup templates (2/4/6/8 layer, HDI) | PCB | WS-E (PCB Core) |
| G21 | xSignals (pad-to-pad signal path analysis through components) | PCB | WS-F (High-Speed) |
| G22 | Length matching group management UI + bar chart dashboard | PCB | WS-F (High-Speed) |
| G23 | Topology constraints (star/chain/fly-by/T) | PCB | WS-F (High-Speed) |
| G24 | Per-layer keepout regions | PCB | WS-E (PCB Core) |
| G25 | Via/routing/component keepout types (separate) | PCB | WS-E (PCB Core) |
| G26 | Board cutouts (arbitrary internal) | PCB | WS-E (PCB Core) |
| G27 | Component clearance rule (courtyard-to-courtyard) | PCB | WS-B (Validation) |
| G28 | Component height constraint (per board region) | PCB | WS-B (Validation) |
| G29 | 3D clearance checking (body-to-body) | PCB | WS-G (3D) |
| G30 | Panelization (step-and-repeat, rails, breakaway tabs, mouse bites) | PCB | WS-C (Output) |
| G31 | Drill table (auto-generated fabrication drawing element) | PCB | WS-C (Output) |
| G32 | Board stackup report (PDF with layer diagram) | PCB | WS-C (Output) |
| G33 | DXF import/export (board outline, mechanical) | PCB | WS-C (Output) |
| G34 | Formal ECO dialog (change list with review before apply) | SCH/PCB | WS-B (Validation) |
| G35 | Unified component model (sym+fp+3D+sim linked) | Library | WS-H (Library) |
| G36 | Lifecycle management (active/EOL/obsolete/last-time-buy) | Library | WS-H (Library) |
| G37 | Keyboard shortcut customization | UI | WS-A (Editor) |
| G38 | Workspace layout save/restore | UI | WS-A (Editor) |
| G39 | Altium file import (.SchDoc, .PcbDoc) | Import | WS-I (Import) |
| G40 | Eagle file import (.sch, .brd) | Import | WS-I (Import) |
| G41 | Query-based rule scoping (InNet, InNetClass, OnLayer, boolean expressions) | PCB | WS-B (Validation) |
| G42 | Design rule width: min/preferred/max (not just min) | PCB | WS-B (Validation) |
| G43 | Routing via style rule (via size per net class) | PCB | WS-D (Router) |
| G44 | Routing priority rule (per net, affects push/shove) | PCB | WS-D (Router) |
| G45 | Routing layer constraint (restrict net to specific layers) | PCB | WS-D (Router) |
| G46 | Routing corners constraint per net class | PCB | WS-D (Router) |
| G47 | Polygon connect style per pad (thermal/direct/none override) | PCB | WS-E (PCB Core) |
| G48 | Plane connect style (thermal/direct for internal planes) | PCB | WS-E (PCB Core) |
| G49 | Plane clearance (anti-pad size on plane layers) | PCB | WS-E (PCB Core) |
| G50 | Silk-to-silk clearance rule | PCB | WS-B (Validation) |
| G51 | Acute angle detection (trace angle < threshold) | PCB | WS-B (Validation) |
| G52 | Board Insight HUD (clearance, net name, designator under cursor) | PCB | WS-E (PCB Core) |
| G53 | Query-based PCB filter + mask mode (dim non-matching) | PCB | WS-E (PCB Core) |
| G54 | Retrace (re-route existing trace segment interactively) | PCB | WS-D (Router) |
| G55 | Trace dragging (drag segment, corners adjust) | PCB | WS-D (Router) |
| G56 | Snippets (save/reuse schematic/PCB fragments) for PCB | PCB | WS-E (PCB Core) |
| G57 | Output Job file (.OutJob) with containers (folder/PDF/ZIP) | Output | WS-C (Output) |
| G58 | Change component (swap part keeping connections) | SCH | WS-A (Editor) |
| G59a | Smart PDF with bookmarks (per-sheet, per-net, per-component) + PDF layers | Output | WS-C (Output) |
| G59b | BOM Studio document (.BomDoc equivalent — dedicated live BOM management view) | Output | WS-C (Output) |
| G59c | Component matching via UIDs (not just designators) for robust ECO sync | SCH/PCB | WS-B (Validation) |
| G59d | Canvas Docs: Board Section View (cross-section cut through any board view) | Output | WS-C (Output) |
| G59e | Canvas Docs: Drill Drawing View (auto-symbols at hole locations + drill table) | Output | WS-C (Output) |
| G59f | Parameter Set directives carrying PCB design rules from schematic to PCB | SCH/PCB | WS-B (Validation) |
| G59g | Multi-channel naming format keywords ($ChannelName, $ChannelIndex, $ChannelAlpha) | SCH | WS-A (Editor) |
| G59h | Room generation from multi-channel (auto-create PCB rooms per channel) + layout replication | PCB | WS-E (PCB Core) |
| G59i | **Constraint Manager** (spreadsheet rule editor accessible from schematic — see below) | SCH/PCB | WS-B (Validation) |
| G59j | **Schematic tables** (placeable formatted tables on canvas — see below) | SCH | WS-A (Editor) |
| G59k | **Table of Contents** (auto-generated index page for multi-sheet designs) | SCH | WS-A (Editor) |
| G59l | **Native metric AND native inch support** (dual coordinate systems — see below) | Core | WS-K (Types) |
| G59m | **True variants in multi-channel designs** (per-channel variant state — see below) | SCH/PCB | WS-A (Editor) |
| G59n | **DRC rule profiles** (save/load named rule sets — see below) | PCB | WS-B (Validation) |
| G59o | **Native Altium import** (.SchDoc, .PcbDoc, .SchLib, .PcbLib, .PrjPcb — CRITICAL, see below) | Import | WS-I (Import) |
| G59p | **Plated board cutouts** (internal plated slots/cutouts for shielding, connectors) | PCB | WS-E (PCB Core) |
| G59q | **Castellated holes** (half-plated holes on board edge for module soldering) | PCB | WS-E (PCB Core) |
| G59r | **Creepage/clearance measurement** (IPC/IEC 60950 creepage distance tool — see below) | PCB | WS-B (Validation) |
| G59s | **Metal core PCB stackup** (MCPCB: aluminum/copper core for LED/power — see below) | PCB | WS-E (PCB Core) |
| G59t | **Square and tapered track ends** (non-round track termination styles — see below) | PCB | WS-D (Router) |
| G59u | **Routing grid** (per-layer routing grid separate from placement grid, grid-aligned routing) | PCB | WS-D (Router) |
| G59v | **Schematic watermarking** (draft/confidential/review overlay — see below) | SCH/PCB | WS-A (Editor) |
| G59w | **Mechanical layer clearance rules** (clearance checks on mechanical/assembly layers) | PCB | WS-B (Validation) |
| G59x | **Built-in Git branching support** (branch/commit/merge/diff from within Signex — see below) | VCS | WS-A (Editor) |
| G59y | **Encrypted SPICE model support** (protect vendor IP in simulation models — see below) | Sim | WS-J (Sim) |
| G59z | **Teardrops as design rules + pad/via property** (not just a post-process tool — see below) | PCB | WS-D (Router) + WS-B (Validation) |
| G60m | **Named unions** (named groups of components with group-level operations — see below) | SCH/PCB | WS-A (Editor) |
| G60n | **Grid extends beyond board outline** (grid visible outside board edge for off-board component staging) | PCB | WS-E (PCB Core) |
| G60o | **Descriptive undo history** (undo menu shows action names: "Move U1", "Route CLK", "Delete R5") | UI | WS-A (Editor) |
| G60p | **Gerber X3 support** (IPC-2581C successor with component data + netlist in Gerber) | Output | WS-C (Output) |
| G60q | **Project-level "not fitted" display toggle** (show/hide DNP visual across all sheets) | SCH | WS-A (Editor) |
| G60r | **Convert 3D body group to single STEP** (merge multiple 3D bodies into one .step export) | 3D | WS-G (3D) |
| G60s | **Design rule variables** (use named variables in rule values: `$DDR_CLR = 5mil`, reuse across rules) | PCB | WS-B (Validation) |
| G60t | **Project lock** (read-only mode that prevents accidental edits, with lock indicator) | UI | WS-A (Editor) |
| G60u | **External program in OutJob** (run custom scripts/executables as part of output generation) | Output | WS-C (Output) |
| G60v | **Area-selective DRC** (select a region → run DRC only within that area for fast local checks) | PCB | WS-B (Validation) |
| G60w | **Git-derived parameters** (auto-populate =GitBranch, =GitCommit, =GitTag, =GitAuthor in title blocks) | VCS | WS-A (Editor) |
| G60x | **Polygon pour rounded edges around pads** (fillet/radius on pour-to-pad clearance boundary) | PCB | WS-E (PCB Core) |
| G60y | **Micrometer (µm) unit for PCB layout** (add µm to mm/mil/inch unit cycle — essential for HDI/MEMS) | Core | WS-K (Types) |
| G60a | **Eye diagram generation** in waveform viewer (from transient PRBS stimulus) | Sim | WS-J (Sim) |
| G60b | **DDR timing analysis** (setup/hold margins vs spec, skew report) | Sim | WS-J (Sim) |
| G60c | **Channel simulation** (Tx IBIS → S-param cascade → Rx IBIS) | Sim | WS-J (Sim) |
| G60d | **S-parameter import** (.s2p/.s4p as SPICE elements for connectors/vias/packages) | Sim | WS-J (Sim) |
| G60e | **PDN impedance analysis** Z(f) from VRM to IC with decap optimization | Sim | WS-J (Sim) |
| G60f | **Antenna simulation** (radiation pattern, gain, S11 via OpenEMS) | Sim | WS-J (Sim) |
| G60g | **Transient thermal analysis** (time-dependent temperature evolution) | Sim | WS-J (Sim) |
| G60h | **Component junction temp estimation** (Rjc from datasheet + thermal map) | Sim | WS-J (Sim) |
| G60i | **Power plane resonance detection** (cavity resonance via OpenEMS) | Sim | WS-J (Sim) |
| G60j | **Via impedance calculator** (Z₀ from barrel diameter, anti-pad, stackup εr — analytical + OpenEMS full-wave) | Sim/PCB | WS-J (Sim) |
| G60k | **Via delay calculator** (propagation delay from via height, εr — integrated into length matching) | PCB | WS-F (High-Speed) |
| G60l | **Via stub resonance estimator** (stub length → f_resonant = c/(4·L·√εr) → performance ceiling) | Sim/PCB | WS-J (Sim) |

### Important (target for v1.1-v1.2)

| # | Feature | Category | Workstream |
|---|---|---|---|
| G59 | Auto-router (topological, Situs-style) | PCB | WS-D (Router) |
| G60 | ActiveRoute (semi-automatic routing assist) | PCB | WS-D (Router) |
| G61 | Backdrilling (stub via removal marking) | PCB | WS-E (PCB Core) |
| G62 | Copper balancing (non-functional fill for warp prevention) | PCB | WS-E (PCB Core) |
| G63 | Copper thieving (dot/square/cross-hatch patterns) | PCB | WS-E (PCB Core) |
| G64 | Return path analyzer (reference plane continuity check) | PCB | WS-F (High-Speed) |
| G65 | Power plane analyzer (current distribution visualization) | PCB | WS-F (High-Speed) |
| G66 | Rooms (from multi-channel, with per-room rules) | PCB | WS-E (PCB Core) |
| G67 | Canvas Docs (fabrication/assembly drawing documents) | Output | WS-C (Output) |
| G68 | V-cut scoring / tab routing with mouse bites | Output | WS-C (Output) |
| G69 | Auto-fiducials / tooling holes / test coupons on panel | Output | WS-C (Output) |
| G70 | Part choices (multi-supplier pricing/stock, BOM Studio) | Library | WS-H (Library) |
| G71 | Where-used analysis (which designs use a component) | Library | WS-H (Library) |
| G72 | SPICE parameter sweeps + Monte Carlo in UI | Sim | WS-J (Sim) |
| G73 | Manufacturing rules (acid trap, slivers) | PCB | WS-B (Validation) |
| G74 | Test point rule + report | PCB | WS-B (Validation) |
| G75 | Net antennae detection (dead-end traces) | PCB | WS-B (Validation) |
| G76 | Preferences import/export | UI | WS-A (Editor) |
| G77 | Visual diff/compare for schematic revisions | VCS | WS-A (Editor) |
| G78 | Visual diff/compare for PCB revisions | VCS | WS-E (PCB Core) |
| G79 | Device sheets (reusable schematic blocks with ports) | SCH | WS-A (Editor) |
| G80 | Net Inspector panel (total length, via count, topology per net) | PCB | WS-E (PCB Core) |
| G81 | Via counting in matched length (delay from via height) | PCB | WS-F (High-Speed) |
| G82 | Diff pair via placement (two vias simultaneously) | PCB | WS-D (Router) |
| G83 | Pad-to-pad length measurement (from pad edge, not centerline) | PCB | WS-F (High-Speed) |
| G84 | Max via count rule per net | PCB | WS-B (Validation) |
| G85 | Fanout control rule (via at pad, via outside pad) | PCB | WS-D (Router) |

### Defer (post v1.2 / plugin-driven)

| # | Feature | Category |
|---|---|---|
| G86 | Rigid-flex board (flex regions, fold lines, stiffeners, coverlay) | PCB |
| G87 | Board bending simulation | PCB |
| G88 | Embedded components (cavity in laminate) | PCB |
| G89 | Castellated holes (half-plated board edge) | PCB |
| G90 | OrCAD/PADS/Mentor/Zuken import | Import |
| G91 | Toolbar customization (drag/drop toolbar buttons) | UI |
| G92 | Slice tool (cut crossing objects) | SCH |
| ~G93~ | ~DelphiScript/JS scripting API~ → **Replaced by WASM plugin system (Phase 13)** | Plugin |

**Note on scripting:** Altium uses DelphiScript (legacy Pascal), JScript, and VBScript — all outdated.
Signex replaces this with **Extism WASM plugins** (Phase 13), which is strictly superior:
- Plugins can be written in any language that compiles to WASM (Rust, C, Go, Python, JS, etc.)
- Sandboxed execution (memory-safe, no host process crashes)
- Permission gateway (plugins declare required capabilities, user approves)
- Hot-loadable (no restart required)
- 5 host function categories: Document, Mutation, UI, Query, Sim
- Undo stack integration (plugin mutations are undoable)
This is a **Signex competitive advantage**, not a gap.
| G94 | 3D PDF output | Output |
| G95 | Board shape from STEP (extract outline from enclosure) | PCB |
| G96 | Enclosure fit checking (import enclosure STEP, check clearance) | 3D |
| G97 | Managed libraries (cloud-hosted, version-controlled component vault) | Library/Pro |

---

## Parallel Workstream Map

The migration is restructured into **10 parallel workstreams** that can be worked on simultaneously by different engineers. Dependencies between workstreams are explicitly documented.

### Dependency Graph

```
WS-A (Editor Core) ────────────────────────────────────────────────────────────┐
  ├─ Phase 0: Scaffold (Iced shell, dock, themes)                              │
  ├─ Phase 2: Canvas (wgpu, pan/zoom/grid)                                     │
  ├─ Phase 4: Editor Foundation (select, move, wire, undo)                     │
  ├─ Phase 5: Core Editing (copy/paste, labels, components)                    │
  └─ Gaps: G1-G4, G9, G37, G38                                                │
                                                                               │
WS-B (Validation) ─── depends on WS-A (Phase 4+) ─────────────────────────────┤
  ├─ Phase 6: ERC (11 checks, annotation)                                      │
  ├─ Phase 9.3: DRC (15→30+ rules)                                            │
  ├─ Query-based rule scoping engine (G41)                                     │
  └─ Gaps: G5-G7, G27-G28, G34, G41-G42, G50-G51, G73-G75, G84               │
                                                                               │
WS-C (Output) ─── depends on WS-A (Phase 3+) ─────────────────────────────────┤
  ├─ Phase 7: Advanced SCH (PDF, BOM, templates, library editor)               │
  ├─ Phase 9.7: PCB export (Gerber, ODB++, STEP)                              │
  └─ Gaps: G8, G30-G33, G47-G49                                               │
                                                                               │
WS-D (Router) ─── depends on WS-E (PCB types) ────────────────────────────────┤
  ├─ Phase 9.1-9.2: Interactive routing (walkaround, push, diff pair)          │
  ├─ Phase 9.6: Advanced (multi-track, via stitch, BGA, teardrop)              │
  └─ Gaps: G10-G13, G43-G46, G54-G55, G59-G60, G82, G85                       │
                                                                               │
WS-E (PCB Core) ─── depends on WS-K (Parser) ─────────────────────────────────┤
  ├─ Phase 8: PCB viewer (render, layers, cross-probe)                         │
  ├─ Phase 9.4-9.5: Copper pour, component placement                          │
  ├─ Layer stack editor + impedance calculator (G18-G20)                       │
  ├─ Board Insight HUD + query filter (G52-G53)                                │
  └─ Gaps: G14-G20, G24-G26, G47-G49, G52-G53, G56, G61-G63, G66, G78, G80   │
                                                                               │
WS-F (High-Speed) ─── depends on WS-D + WS-E ─────────────────────────────────┤
  ├─ xSignals engine (through-component path analysis) (G21)                   │
  ├─ Length matching dashboard + interactive tuning (G22)                       │
  └─ Gaps: G21-G23, G64-G65, G81, G83 (xSignals, topology, return path)       │
                                                                               │
WS-G (3D Viewer) ─── depends on WS-E (PCB types) ─────────────────────────────┤
  ├─ Phase 10: 3D PCB (extrude, PBR, STEP)                                    │
  └─ Gaps: G29 (3D clearance)                                                  │
                                                                               │
WS-H (Library) ─── independent until integration ──────────────────────────────┤
  └─ Gaps: G35-G36, G50-G51 (unified model, lifecycle, suppliers)              │
                                                                               │
WS-I (Import) ─── depends on WS-K (Parser types) ─────────────────────────────┤
  └─ Gaps: G39-G40 (Altium import, Eagle import)                               │
                                                                               │
WS-J (Simulation) ─── depends on WS-K (types), WS-E (PCB for EM/thermal) ─────┤
  ├─ Phase 11.1: ngspice (SPICE, IBIS, parameter/temp sweep)                  │
  ├─ Phase 11.2: OpenEMS (S-params, TDR, crosstalk, antenna, GPU FDTD)        │
  ├─ Phase 11.3: Elmer (thermal steady+transient, DC IR, Joule heating)        │
  ├─ Phase 11.4: DDR SI (IBIS+S-param cascade, eye diagram, timing analysis)  │
  ├─ Phase 11.5: PDN analysis (impedance vs freq, decap optimization)          │
  └─ Gaps: G52, G60a-G60i                                                      │
                                                                               │
WS-K (Parser & Types) ─── FOUNDATION, start first ─────────────────────────────┘
  ├─ Phase 1: KiCad parser + writer
  └─ signex-types crate (all domain types)
```

### Workstream Assignments

Each workstream can be assigned to a different engineer (or pair). They work on feature branches simultaneously and merge to `dev`.

```
SPRINT 1 (Weeks 1-4) — Foundation [2 engineers]
├── Engineer A: WS-K (Parser + Types)          → v0.2.0
├── Engineer B: WS-A (Scaffold + Canvas)       → v0.1.0, v0.3.0
└── Both unblocked immediately, no dependencies

SPRINT 2 (Weeks 5-8) — Schematic Viewer + Library [3 engineers]
├── Engineer A: WS-A (Schematic Viewer)        → v0.4.0  (needs WS-K done)
├── Engineer B: WS-H (Library system, unified component model, lifecycle)
└── Engineer C: WS-I (Altium/Eagle import)     → needs WS-K types

SPRINT 3 (Weeks 9-14) — Schematic Editor + Validation [4 engineers]
├── Engineer A: WS-A (Editor Foundation + gaps G1-G4, G9, G37-G38, G58)  → v0.5.0
├── Engineer B: WS-A (Core Editing + snippets, device sheets)            → v0.6.0
├── Engineer C: WS-B (ERC + project compilation G5-G7 + query engine G41) → v0.7.0
└── Engineer D: WS-C (Output: PDF, BOM, variant BOM G8, OutJob G57)      → v0.8.0

SPRINT 4 (Weeks 15-22) — PCB [5 engineers]
├── Engineer A: WS-E (PCB Core: viewer, layers, layer stack G18-G20, keepout G24-G26,
│               split planes G14, Board Insight HUD G52, query filter G53)       → v0.9.0
├── Engineer B: WS-D (Router: routing + loop removal G10, glossing G11,
│               impedance routing G12, length gauge G13, retrace G54,
│               trace drag G55, via style G43, priority G44, layer constraint G45) → v0.10.0
├── Engineer C: WS-B (DRC: 30+ rules, paste/mask G16-G17, acute angle G51,
│               silk-silk G50, component clearance G27-G28, ECO dialog G34)
├── Engineer D: WS-C (Gerber, ODB++, panelization G30, drill table G31,
│               stackup report G32, DXF G33)
└── Engineer E: WS-G (3D Viewer: PBR, STEP, 3D clearance G29)           → v0.11.0

SPRINT 5 (Weeks 23-30) — Sim + High-Speed + Gaps [5 engineers]
├── Engineer A: WS-J Phase 11.1-11.3 (ngspice+IBIS, OpenEMS S-param/TDR/antenna,
│               Elmer thermal+IR, GPU FDTD, parameter/temp sweep G72)    → v0.12.0
├── Engineer B: WS-F (High-Speed: xSignals G21, length matching UI G22,
│               topology G23, return path G64, via count G81, diff pair via G82)
│               + WS-J Phase 11.4 (DDR SI: channel sim G60c, eye diagram G60a,
│               timing analysis G60b, S-param cascade G60d)
├── Engineer C: WS-E (Copper balance G62-G63, rooms G66, backdrill G61,
│               plane connect G47-G49, net inspector G80)
├── Engineer D: WS-D (Auto-router G59, ActiveRoute G60, fanout G85)
│               + WS-J Phase 11.5 (PDN impedance G60e, plane resonance G60i)
└── Engineer E: WS-B (Mfg rules G73, testpoint G74, antennae G75, max via G84)
│               + WS-J (Junction temp G60h, transient thermal G60g)

SPRINT 6 (Weeks 31-38) — Pro + Plugin + Release [5 engineers]
├── Engineer A: Phase 12 Signal AI (Pro, Alp Lab gateway)
├── Engineer B: Phase 13 Plugin System (Extism WASM — Signex advantage over DelphiScript)
├── Engineer C: Phase 14 Polish + visual diff G77-G78
├── Engineer D: Phase 15 Collaboration (Pro, Supabase)
└── Engineer E: WS-C (Canvas Docs G67, V-cut G68, fiducials G69)
All: Integration testing, QA
```

### Branch Strategy for Parallel Work

```
main
├── dev
│   ├── ws-a/scaffold              (Engineer B, Sprint 1)
│   ├── ws-k/parser                (Engineer A, Sprint 1)
│   ├── ws-a/canvas                (Engineer B, Sprint 1)
│   ├── ws-a/schematic-viewer      (Engineer A, Sprint 2)
│   ├── ws-h/library-system        (Engineer B, Sprint 2)
│   ├── ws-i/altium-import         (Engineer C, Sprint 2)
│   ├── ws-a/editor-foundation     (Engineer A, Sprint 3)
│   ├── ws-a/core-editing          (Engineer B, Sprint 3)
│   ├── ws-b/erc-validation        (Engineer C, Sprint 3)
│   ├── ws-c/pdf-bom-export        (Engineer D, Sprint 3)
│   ├── ws-e/pcb-core              (Engineer A, Sprint 4)
│   ├── ws-d/router                (Engineer B, Sprint 4)
│   ├── ws-b/drc-rules             (Engineer C, Sprint 4)
│   ├── ws-g/3d-viewer             (Engineer E, Sprint 4)
│   ├── ws-j/simulation            (Engineer A, Sprint 5)
│   ├── ws-f/high-speed            (Engineer B, Sprint 5)
│   ├── pro/signal-ai              (Engineer A, Sprint 6)
│   ├── pro/collaboration          (Engineer D, Sprint 6)
│   └── feature/plugin-system      (Engineer B, Sprint 6)
```

**Rules for parallel work:**
- Each workstream gets its own branch prefix (`ws-a/`, `ws-b/`, etc.)
- Engineers merge to `dev` via PR when their branch is ready
- CI runs on every push to any workstream branch
- `dev` is the integration branch — merge conflicts resolved here
- `main` only receives tagged releases from `dev`
- Each workstream owns specific crates — no two workstreams edit the same crate simultaneously
- Shared crate (`signex-types`) changes require PR review from all affected workstreams

### Crate Ownership

| Crate | Primary WS | May also touch |
|---|---|---|
| `signex-types` | WS-K | All (via PR review) |
| `kicad-parser` | WS-K | WS-I |
| `kicad-writer` | WS-K | WS-C |
| `signex-app` (shell/dock/theme) | WS-A | All (panels) |
| `signex-app` (canvas/schematic) | WS-A | — |
| `signex-app` (canvas/pcb) | WS-E | WS-D |
| `signex-app` (canvas/pcb_3d) | WS-G | — |
| `signex-app` (panels/) | WS-A | WS-B, WS-C, WS-E, WS-F |
| `signex-render` (schematic/) | WS-A | — |
| `signex-render` (pcb/) | WS-E | WS-D |
| `signex-render` (pcb_3d/) | WS-G | — |
| `signex-erc` | WS-B | — |
| `signex-drc` | WS-B | — |
| `pcb-geom` | WS-E | WS-D |
| `spice-gen` | WS-J | — |
| `openems-bridge` | WS-J | — |
| `elmer-bridge` | WS-J | — |
| `formula-render` | WS-A | — |
| `step-loader` | WS-G | — |
| `plugin-api` | Dedicated | — |
| `signex-signal` | Pro team | — |
| `signex-collab` | Pro team | — |
