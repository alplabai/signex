# Signex — Roadmap

> **Status:** Living document. Updated quarterly.
> **Audience:** Anyone planning work, allocating engineering time, or
> communicating timelines.
> **Companion to:** `MASTER_PLAN.md` (scope and principles), `ARCHITECTURE.md`
> (technical foundation), `REPOSITORY_AND_CODEBASE.md` (crate ownership).

This document describes *when* things ship and *who* builds them. It does not
describe *what* features do — that is in `MASTER_PLAN.md` and
`PRODUCT_AND_EDITIONS.md`. It does not describe *how* features are
implemented — that is in `ARCHITECTURE.md`.

The dates and durations in this document are working estimates based on a
small team. They are not commitments. They will move. The order and the gates
are firmer than the dates.

---

## 1. How to Read This Document

The roadmap is organized into:

- **Phase 0** — the foundation gate. No UI. Must succeed before anything else.
- **v1.0 phases** — schematic-only editor.
- **v2.0 phases** — PCB editor.
- **v3.0 phases** — Pro launch (Signal AI + collaboration).
- **Beyond v3.0** — simulation, advanced features, plugins.

Each phase has:

- A goal (what success looks like)
- Workstreams (what gets built in parallel)
- Deliverables (concrete outputs)
- Exit criteria (how we know the phase is complete)
- A duration estimate (rough, not committed)

Phases are sequential. A phase does not start until the previous phase's exit
criteria are met.

---

## 2. Versioning Strategy

Signex uses **semantic versioning** with the following meaning:

| Version | Meaning                                                          |
|---------|------------------------------------------------------------------|
| 0.x     | Pre-release. Foundation, no public users.                        |
| 1.0     | First public release. Community schematic editor is real.        |
| 1.x     | Schematic-editor refinement. PCB development happens in parallel.|
| 2.0     | PCB editor ships. Signex is now a complete EDA tool.             |
| 2.x     | PCB-editor refinement. Pro development happens in parallel.      |
| 3.0     | Pro launch. Signal AI and collaboration ship.                    |
| 3.x     | Pro refinement, simulation begins.                               |
| 4.0+    | Simulation, advanced features, plugins.                          |

**Major versions are gates, not arbitrary cuts.** v1.0 ships when the
schematic editor is production-ready — not when a date passes. v2.0 ships when
PCB is production-ready. We do not ship v1.0 to "claim" a 1.0 milestone; we
ship v1.0 because users can use it.

**Patch releases** (1.0.1, 1.0.2, etc.) ship as needed for bug fixes. Minor
releases (1.1, 1.2, etc.) bundle non-breaking feature additions.

---

## 3. Priority Tiers

Every feature falls into one of four tiers. Tiers determine when a feature
can ship, not whether it ships at all.

| Tier   | Meaning                                                        | Ships in       |
|:------:|----------------------------------------------------------------|----------------|
| **P0** | Required for v1.0. Cannot ship Community without it.           | v0.1–v1.0      |
| **P1** | Required for v2.0. Professional users expect it.               | v1.1–v2.0      |
| **P2** | Differentiator. Drives Pro adoption.                           | v2.1–v3.0      |
| **P3** | Nice-to-have. Stable core required first.                      | v3.x and later |

**Decision rule:** if a feature is proposed for an earlier version than its
tier allows, the answer is "after the current target ships." This rule has no
exceptions during Phase 0 or v1.0 development. It is the single most
important defense against scope creep.

---

## 4. Workstreams

Workstreams are parallel tracks of development. Each workstream owns a set of
crates (see `REPOSITORY_AND_CODEBASE.md`) and a set of features. Workstreams
are sized so that a workstream can be assigned to one engineer at a time
without overlapping with another workstream's crates.

Not every workstream is active in every phase. Workstreams activate and
complete on their own schedules.

### Workstream Catalog

| ID      | Name                          | Owns                                         |
|---------|-------------------------------|----------------------------------------------|
| **WS-D**  | Document Layer              | `kicad-document`                             |
| **WS-M**  | Semantic Model              | `signex-model`                               |
| **WS-E**  | Engine                      | `signex-engine`                              |
| **WS-R**  | Render                      | `signex-render`                              |
| **WS-U**  | UI Shell + Schematic Canvas | `signex-app` (shell, panels, schematic)      |
| **WS-V**  | Validation (ERC/DRC)        | `signex-erc`, `signex-drc`                   |
| **WS-O**  | Output                      | export modules (PDF, BOM, Gerber, etc.)      |
| **WS-P**  | PCB Geometry                | `pcb-geom`                                   |
| **WS-3D** | 3D Viewer                   | `signex-render-3d`, `step-loader`            |
| **WS-S**  | Simulation                  | `spice-bridge`, `openems-bridge`, etc.       |
| **WS-AI** | Signal AI (Pro)             | `signex-signal`                              |
| **WS-C**  | Collaboration (Pro)         | `signex-collab`, Supabase backend            |
| **WS-X**  | Plugins                     | `signex-plugin`                              |

These workstream IDs are referenced throughout the rest of this document.

---

## 5. Phase 0 — The Foundation Gate

**Goal:** prove that the architectural foundation works on real KiCad files
before building anything that depends on it.

**Duration estimate:** 8–12 weeks with one engineer; 6–8 weeks with two.

**Active workstreams:** WS-D (primary), WS-M (secondary), WS-E (tertiary).

### What Phase 0 Builds

#### Phase 0.1 — Parser

- `kicad-document` crate scaffold
- S-expression tokenizer
- S-expression tree builder with arena allocation, `NodeHandle` identity, span
  tracking
- Parsers for `.kicad_sch`, `.kicad_pcb`, `.kicad_sym`, `.kicad_pro`
- Unknown-node preservation in the tree structure
- Round-trip test harness (parse → write → parse → assert equal)

#### Phase 0.2 — Writer

- Minimal-diff writer that preserves node order, spans, and unknown content
- Configurable formatting to target KiCad's own writer output where reasonable
- Diff comparison against KiCad's output on the fixture corpus

#### Phase 0.3 — Semantic Model Skeleton

- `signex-model` crate scaffold
- Core types: `Symbol`, `Wire`, `Junction`, `Label`, `Sheet`, `Pin` (read-only
  surface; mutation lives in WS-E)
- `ObjectId` type and identity assignment
- `IdentityMap` between `ObjectId` and `NodeHandle`
- Builder that constructs a semantic model from a `Document`

#### Phase 0.4 — Engine Skeleton

- `signex-engine` crate scaffold
- `Command`, `SemanticPatch`, `DocumentPatch`, `PatchPair` types
- `Engine` struct with `execute`, `undo`, `redo`, `save`, `open`
- A small set of test commands (move, update property, delete) sufficient to
  exercise the patch system end-to-end
- The Invariant check (Section 4.5 of `ARCHITECTURE.md`) running in debug
  builds after every operation

### Phase 0 Exit Criteria

Phase 0 is complete when **all** of the following are true:

1. **20+ real KiCad projects** parse without error. Fixture corpus is
   committed to the repository.
2. **Round-trip stability:** every fixture project, parsed and written
   without modification, produces output that KiCad opens with zero warnings.
3. **Diff stability:** the written output differs from the original by no
   more than whitespace and ordering of irrelevant constructs (defined by an
   automated diff classifier).
4. **Unknown-node preservation:** at least three fixtures contain
   intentionally-introduced vendor extensions or unknown constructs; these
   round-trip through the parser/writer without loss.
5. **Engine end-to-end:** a headless test exercises a sequence of 50+
   commands (place, move, update, delete, undo, redo) on a real schematic and
   asserts the Invariant after each step.
6. **Performance baseline:** parsing a 500-symbol schematic takes under 200 ms
   on a 2020-era laptop. Writing takes under 200 ms.

If any of these criteria fails, Phase 0 continues. We do not start Phase 1
with a half-working foundation.

### Phase 0 Decision Points

The following architectural decisions are made or finalized during Phase 0:

- **Byte-identical vs. structural minimal rewrite** (see `ARCHITECTURE.md`
  Section 9.1)
- **Lossless trivia preservation** in the parser (whitespace, comments)
- **UI stack lock-in** (iced + wgpu, or a switch if iced proves insufficient
  during early prototyping at the end of Phase 0)
- **First-class KiCad version target** (currently 9.x; revisit if usage data
  during Phase 0 suggests otherwise)

After Phase 0 ends, these decisions are locked.

---

## 6. v1.0 — Schematic Editor (Community)

**Goal:** ship a Community schematic editor that a KiCad user can use as a
daily driver.

**Total duration estimate:** 10–14 months from end of Phase 0, with a small
team (2–3 engineers).

v1.0 is broken into six phases. Each phase is roughly 6–8 weeks with two
engineers.

### Phase 1 — UI Foundation (v0.1 → v0.2)

**Active workstreams:** WS-U (primary), WS-R (start).

**Duration:** 6–8 weeks.

**Goal:** Empty editor shell with iced, panel docking system, themes, status
bar, and a working canvas (no design rendering yet).

**Deliverables:**

- iced application that launches and shows a window
- Custom panel docking system (left/right/bottom regions, tabs, collapse)
- Custom tree view widget (for Projects panel)
- Menu bar, toolbar, status bar
- Document tab bar
- All six themes implemented and switchable
- Empty `iced::widget::Canvas` in the center area with pan/zoom/grid
- Right-click pan, scroll-wheel zoom (cursor-centered)
- Coordinate display in status bar with unit cycling

**Exit criteria:**

- Application launches reliably on Windows, macOS, Linux
- All six themes render without artifacts
- Pan/zoom is smooth at 60 fps on an empty canvas
- Panel system supports all `UX_REFERENCE_ALTIUM.md` Section 2 behaviors
  except floating panels

### Phase 2 — Schematic Viewer (v0.3 → v0.4)

**Active workstreams:** WS-U (primary), WS-R (primary), WS-M (extending
support for full schematic types).

**Duration:** 6–8 weeks.

**Goal:** Open a KiCad project and render every schematic element correctly.

**Deliverables:**

- File → Open Project dialog
- Parse `.kicad_pro`, populate Projects panel with sheet hierarchy
- Render all schematic element types: wires, symbols, pins, labels (all four
  kinds), junctions, no-connects, buses, bus entries, drawings, text
- Sheet borders, title blocks, multiple paper sizes
- Multi-sheet navigation via Projects panel and Ctrl+double-click
- Rich text markup (subscript, superscript, overbar)
- Theme-aware rendering (theme switch re-renders correctly)

**Exit criteria:**

- Five real KiCad demo projects render correctly with no visible difference
  from KiCad's own rendering
- 500-symbol schematic pans and zooms at 60 fps
- All six themes render every element type correctly

### Phase 3 — Schematic Editing Core (v0.5)

**Active workstreams:** WS-E (primary), WS-U (primary), WS-V (start with ERC
prep).

**Duration:** 8–10 weeks. *(This is the longest single phase — the engine
contract is established here and everything that follows depends on it.)*

**Goal:** Edit schematics. Select, move, wire, delete, rotate, mirror,
undo/redo, save. The engine becomes real.

**Deliverables:**

- Selection system (click, shift-click, box select with direction-sensitive
  semantics)
- Selection filter
- Move with rubber-banding; Ctrl+drag for stiff move; Ctrl+arrow nudge
- Wire drawing (W key) with three routing modes (Shift+Space cycle)
- Auto-junction at T-intersections
- Delete, rotate (Space/R), mirror (X/Y)
- Engine `Command` set covering all editing operations above
- `SemanticPatch` and `DocumentPatch` generation for all commands
- Undo/redo (50+ levels)
- Save (Ctrl+S) with minimal-diff writer
- Properties panel (F11) with context-aware content

**Exit criteria:**

- A KiCad schematic can be loaded, edited (place, move, wire, delete,
  property change), saved, and reopened in KiCad with zero unexpected diffs
- The Invariant holds after every operation in a 200-command stress test
- Undo/redo works correctly across all command types

### Phase 4 — Full Editing Workflow (v0.6)

**Active workstreams:** WS-U (primary), WS-E (extending command set), WS-V
(continuing).

**Duration:** 6–8 weeks.

**Goal:** All the editing operations a user actually does, beyond the basics.

**Deliverables:**

- Copy/Cut/Paste (`Ctrl+C/X/V`)
- Smart paste (`Shift+Ctrl+V`)
- Duplicate (`Ctrl+D`)
- Label placement (L key) for all four label types
- Bus drawing (B key), bus entries
- Component placement (P key) with library browser
- In-place text editing (F2 or click-pause-click)
- Context menu (right-click no-drag)
- Find / Find and Replace (`Ctrl+F`, `Ctrl+H`)
- Selection memory (`Ctrl+1-8`, `Alt+1-8`)
- Measure tool (`Ctrl+M`)

**Exit criteria:**

- All shortcuts in `UX_REFERENCE_ALTIUM.md` Section 4 work as documented
- A user can build a non-trivial schematic from scratch using only the
  editor and library browser
- Library browser displays all installed KiCad symbol libraries

### Phase 5 — Validation and Annotation (v0.7)

**Active workstreams:** WS-V (primary), WS-U (Messages panel and integration),
WS-E (annotation commands).

**Duration:** 6–8 weeks.

**Goal:** ERC, annotation, net-aware visualization.

**Deliverables:**

- ERC engine in `signex-erc` covering 11 standard rule types (duplicate
  designators, unconnected pins, floating wires, no driver, single-pin nets,
  output conflicts, multiple net names, unannotated components, pin matrix
  conflicts, undriven power pins, unlabeled nets)
- 12×12 pin connection matrix (configurable per cell)
- Messages panel with click-to-zoom-and-highlight
- Annotation system with four modes, preview, lock/unlock per designator
- Net color override (F5)
- AutoFocus (dim unrelated objects on hover/select)

**Exit criteria:**

- ERC catches all 11 violation types correctly on a fixture set of
  intentionally-broken schematics
- Click on a violation in the Messages panel zooms to the source on the canvas
- Annotation produces correct results across all four modes

### Phase 6 — Output and Polish (v0.8 → v1.0)

**Active workstreams:** WS-O (primary), WS-U (polish), WS-E (final commands).

**Duration:** 8–10 weeks.

**Goal:** Export everything users need, and polish the editor to v1.0 quality.

**Deliverables:**

- PDF export (single sheet, multi-sheet, configurable DPI and color mode)
- BOM export (CSV, TSV, HTML, Excel)
- Netlist export (KiCad S-expression)
- Print via system dialog
- Sheet templates (ISO A4, ANSI A)
- Title block field substitution (=Title, =Date, =Rev, etc.)
- Drawing tools (line, rectangle, circle, arc, polyline, polygon)
- Watermarking (optional DRAFT/CONFIDENTIAL overlay)
- Symbol library editor (basic — create/edit symbols, pins, graphics)
- Performance pass: profile and fix any frame-rate regression on the fixture
  corpus
- Stability pass: fix all P0 bugs from the issue tracker
- Installer for Windows (.msi), macOS (.dmg), Linux (.AppImage)
- Native file associations for `.kicad_sch`, `.kicad_pcb`, `.kicad_pro`
- Documentation: user guide, keyboard reference, getting-started tutorial

**Exit criteria — v1.0 RELEASE:**

- All v1.0 must-do items from `MASTER_PLAN.md` Section 5.1 are complete
- All exit criteria of all previous phases still pass
- Fixture corpus has grown to 50+ real KiCad projects, all round-tripping
  cleanly
- A non-developer beta tester can install Signex, open their KiCad project,
  edit it, and save it without consulting a developer
- The issue tracker has zero P0 bugs

---

## 7. v2.0 — PCB Editor (Community)

**Goal:** Add PCB editing, making Signex a complete EDA tool.

**Total duration estimate:** 12–16 months from v1.0, with 2–3 engineers.

v2.0 development overlaps with v1.x maintenance. The schematic editor
continues to receive bug fixes and refinement during v2.0 development.

### Phase 7 — PCB Viewer (v1.1)

**Active workstreams:** WS-R (primary, learning instanced rendering), WS-U
(PCB canvas widget), WS-M (extending model with PCB types).

**Duration:** 8–10 weeks.

**Goal:** Open and render KiCad PCB files. No editing yet.

**Deliverables:**

- PCB types in `signex-model`: `Track`, `Pad`, `Via`, `Zone`, `Footprint`,
  `BoardOutline`
- PCB canvas widget using `iced::widget::Shader` with custom WGSL pipelines
- Instanced rendering for tracks, pads, vias
- Zone fill rendering (pre-tessellated polygons)
- 32 copper layers + technical layers, Altium default colors
- Layer Stack panel with visibility, color, active layer
- Single-layer mode (Shift+S cycle)
- Board flip (Ctrl+F)
- Cross-probe between schematic and PCB (Ctrl+double-click)
- Ratsnest (MST + UnionFind)

**Exit criteria:**

- Five real KiCad PCB projects render correctly
- 10,000-track PCB pans and zooms at 60 fps
- Cross-probe works bidirectionally

### Phase 8 — PCB Editing Core (v1.2)

**Active workstreams:** WS-E (PCB commands), WS-U (PCB tools), WS-P (geometry).

**Duration:** 10–12 weeks.

**Goal:** Move components, edit zones, modify board outline. Not yet routing.

**Deliverables:**

- Component placement (move, rotate any angle, flip to other side)
- Board outline editing
- Zone (copper pour) drawing and editing
- Polygon clipping (Clipper2 integration)
- Hit testing for PCB element types
- PCB commands in the engine (move, rotate, flip, edit zone, edit outline)

**Exit criteria:**

- A PCB can be modified (components moved, zones edited, outline changed)
- All edits round-trip cleanly through KiCad

### Phase 9 — Routing (v1.3)

**Active workstreams:** WS-P (primary, router algorithms), WS-E (routing
commands), WS-U (routing tools).

**Duration:** 12–16 weeks. *(Routing is the hardest single feature in EDA.)*

**Goal:** Interactive routing comparable to KiCad 9's interactive router.

**Deliverables:**

- Walkaround routing
- Push-and-shove routing
- 45° / 90° / arc corner styles
- Via placement during routing
- Track width from net class rules
- Differential pair routing with gap control
- Length tuning (accordion, sawtooth, trombone meanders)
- BGA fanout, via stitching
- Multi-track routing

**Exit criteria:**

- A complete PCB can be routed using only the interactive router
- Push-and-shove behavior matches KiCad's quality on standard test cases

### Phase 10 — DRC and Design Rules (v1.4)

**Active workstreams:** WS-V (primary, DRC engine), WS-U (DRC panel,
constraint dialogs), WS-E (rule application).

**Duration:** 8–10 weeks.

**Goal:** DRC engine with the standard rule set, and the constraint editing
that drives it.

**Deliverables:**

- DRC engine in `signex-drc` covering 15 base rules (clearance, min track
  width, min via, annular ring, drill, mask, paste, courtyard, hole-to-hole,
  edge clearance, silk-silk, copper edge, zone fill, unconnected, short)
- DRC panel with click-to-zoom on violations
- Net class management (assign nets, define rules per class)
- Constraint editing dialogs

**Exit criteria:**

- DRC catches all 15 violation types on intentionally-broken fixtures
- Net class rules are applied correctly during routing and DRC
- A real PCB design passes DRC with zero false positives on the fixture corpus

### Phase 11 — PCB Output and v2.0 Polish (v1.5 → v2.0)

**Active workstreams:** WS-O (primary), all others (polish).

**Duration:** 10–12 weeks.

**Goal:** Manufacturing-ready output and v2.0 release polish.

**Deliverables:**

- Gerber RS-274X with X2 attributes
- Excellon drill files
- ODB++ export
- Pick-and-place CSV
- IPC-2581 export
- STEP 3D export (board body)
- Assembly drawings (SVG)
- Performance pass on PCB editor
- Stability pass
- Installer updates
- Updated documentation

**Exit criteria — v2.0 RELEASE:**

- A complete project (schematic + PCB) can be designed, validated, and
  exported to manufacturing files
- A real PCB designed in Signex has been successfully fabricated by at least
  one beta user
- Issue tracker has zero P0 bugs

---

## 8. v3.0 — Pro Launch

**Goal:** Add Signal AI and live collaboration, launch the Pro subscription.

**Total duration estimate:** 8–12 months from v2.0.

Pro development happens in feature-gated crates (see `PRODUCT_AND_EDITIONS.md`
Section 5). The Community editor continues to ship, refined and bug-fixed.

### Phase 12 — Signal AI (v2.1 → v2.5)

**Active workstreams:** WS-AI (primary), WS-U (Signal panel UI in `pro` cfg).

**Duration:** 16–20 weeks.

**Goal:** A working Signal AI integration that justifies a subscription.

**Deliverables:**

- Alp Lab API gateway (managed Claude API access)
- License validation system
- Signal panel in the bottom dock (Pro builds only)
- Streaming chat with markdown rendering
- Tool use definitions for: simulation, query, edit, analysis, review
- Design context injection (component list, nets, ERC/DRC results)
- Visual context (canvas screenshot to vision API)
- Circuit template library (6+ templates)
- Design review mode (structured analysis with severity ranking)
- Usage metering and reasonable fair-use limits

**Exit criteria:**

- A Pro user can hold a productive design conversation with Signal AI
- Tool use successfully executes design changes that are correctly undoable
- Beta testers report Signal AI is worth the subscription

### Phase 13 — Live Collaboration Backend (v2.6 → v2.8)

**Active workstreams:** WS-C (primary), backend (Supabase setup).

**Duration:** 12–16 weeks.

**Goal:** Backend infrastructure for collaboration. Client work begins in
parallel toward end of phase.

**Deliverables:**

- Supabase project provisioning and schema (see `PRODUCT_AND_EDITIONS.md`
  for tables)
- Edge Functions for CRDT merge, lock acquisition, notifications, cleanup
- Row-Level Security policies for owner/editor/viewer roles
- Authentication flows (email, GitHub, Google OAuth)
- Storage layer for project files
- Local SQLite offline operation queue (client side)
- Client connection layer (WebSocket Realtime channels)

**Exit criteria:**

- Two test clients can connect to a shared project and exchange CRDT
  operations
- Authentication and RLS are enforced correctly
- Offline operation queueing and replay work correctly

### Phase 14 — Live Collaboration Client (v2.9 → v3.0)

**Active workstreams:** WS-C (primary), WS-U (collaboration UI in `pro` cfg).

**Duration:** 12–16 weeks.

**Goal:** Full collaboration experience visible in the editor.

**Deliverables:**

- Per-user cursors rendered on canvas with name/color
- Presence panel (online users, follow mode)
- Sheet/region/layer/net locking
- Comments pinned to canvas locations
- Review workflow (request, diff overlay, approve, reject)
- Version history browser
- Activity feed
- Conflict resolution UI

**Exit criteria — v3.0 RELEASE:**

- Three test users can edit the same schematic and PCB simultaneously
  without conflicts
- All Pro features in `PRODUCT_AND_EDITIONS.md` Section 4.4.2 work as
  documented
- Pricing is set, subscription billing is operational
- Pro and Community binaries both build clean from a single workspace

---

## 9. Beyond v3.0

Post-v3.0 development happens in parallel feature streams. Order is
flexible and driven by user demand.

| Stream                          | Tier | Tentative Window |
|---------------------------------|:----:|------------------|
| Plugin system (WASM, Extism)    | P1   | v3.x             |
| 3D PCB viewer                   | P1   | v3.x             |
| SPICE simulation (ngspice)      | P2   | v3.x             |
| EM simulation (OpenEMS)         | P2   | v4.x             |
| Thermal simulation (Elmer)      | P2   | v4.x             |
| Advanced schematic (variants, multi-channel, harnesses) | P2 | v3.x |
| Advanced PCB (impedance, length matching, xSignals)     | P2 | v3.x |
| High-speed design (DDR SI, eye diagrams, PDN)            | P3 | v4.x+ |
| Altium/Eagle import                                      | P3 | v4.x  |
| Git integration                                          | P2 | v3.x  |
| Auto-router                                              | P3 | v4.x+ |
| Rigid-flex, embedded components                          | P3 | v5.x+ |

These are not commitments. They are recognition that work continues after
v3.0 and these are the candidates.

---

## 10. Staffing Assumptions

This roadmap is planned for a **small team**: 2–3 engineers during
v0.x–v1.0, scaling to 3–5 during v2.0 and v3.0.

### What That Implies

- Phases overlap less than they would with a larger team
- A single engineer typically owns one workstream at a time
- Specialists may rotate between workstreams as their expertise becomes
  needed (e.g., the WS-D parser engineer may move to WS-O writer/exporter
  work after Phase 0)
- The roadmap is paced for sustainability, not for sprint-to-failure

### Scaling Up

If the team grows, parallelism increases:

- WS-V (validation) can be developed in parallel with WS-U (UI) once the
  engine API is stable
- WS-O (output) can be parallel with WS-V once the model is stable
- WS-3D and WS-S are independent of the editor's main path and can be done
  by separate engineers without coordination overhead
- WS-AI and WS-C are largely independent of each other in v3.0

### Scaling Down

If the team shrinks to one engineer (which is realistic at certain points):

- Phase durations approximately double from the estimates above
- The roadmap order remains the same; only velocity changes
- v1.0 in particular is achievable solo if the engineer is committed and the
  scope discipline holds

### What Doesn't Scale

- Phase 0 always requires one focused engineer for 2–3 months. More people
  do not make parser/writer development faster; they make it slower.
- Routing (Phase 9) is fundamentally one engineer's deep work for 3+ months.
  It is the canonical example of a feature where adding people slows things
  down.
- Architectural decisions (the kind documented in `ARCHITECTURE.md`) are made
  by a small group and then communicated. They are not decided in committee.

---

## 11. What This Roadmap Does Not Commit To

Listed honestly to manage expectations:

- **Calendar dates.** Estimates are working numbers. They will move. We do
  not announce ship dates publicly until a release candidate exists.
- **Feature ordering within a phase.** Within a phase, the order of
  individual deliverables may shift based on dependencies and engineer
  availability.
- **The post-v3.0 list (Section 9).** Order and timing are speculative.
  User feedback and operational realities will reshape this list.
- **Pricing (Pro).** Set when v3.0 is in beta, not earlier.
- **Promised performance numbers.** The performance targets in this document
  are exit criteria, not user-facing promises. Real-world performance varies
  with hardware and design complexity.

---

## 12. Rules for Changing This Document

- **Phase order does not change** without an architectural reason. "We
  changed our minds" is not sufficient.
- **Phase 0 cannot be skipped, abbreviated, or merged into Phase 1.** It is
  a gate.
- **Adding scope to a phase requires removing equivalent scope from the
  same phase.** The phase duration does not silently grow.
- **Pulling a feature forward** (from v2.x to v1.x, for example) requires
  explicit justification and an updated tier classification.
- **Pushing a feature back** is always allowed and does not require
  justification beyond "we underestimated the scope."
- **Quarterly review:** this document is reviewed and updated every quarter.
  Estimates are refreshed against actual progress. Phases that are running
  significantly long or short are re-planned.
- **Major version exit criteria** (the lists at the end of v1.0, v2.0, v3.0)
  are very stable. Changing them requires a strong product reason and is
  treated like changing the master plan.
