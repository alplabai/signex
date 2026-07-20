# Signex — Roadmap

> **Status:** Living document. Updated quarterly. Last reconciled against
> reality 2026-07-15.
> **Audience:** Anyone planning work, allocating engineering time, or
> communicating timelines.
> **Owns:** the version axis. Every version number Signex ships is defined
> here and nowhere else.

This document describes *when* things ship. It does not describe *what*
features do — that is `MASTER_PLAN.md` and `PRODUCT_AND_EDITIONS.md`. It
does not describe *how* they are implemented — that is `ARCHITECTURE.md`.

The dates and durations here are working estimates. They are not
commitments. They will move. The order and the gates are firmer than the
dates.

---

## 1. Version Axis Ownership

**This document is the single source of truth for the version axis.**
Nothing else defines what ships in a version.

| Source | Owns | Does not own |
|--------|------|--------------|
| `docs/ROADMAP.md` (this file) | Version numbers, ordering, gates, exit criteria | Feature semantics |
| `MASTER_PLAN.md` | Scope, principles, product thesis, format stance | Version numbers |
| `.github/milestones.yml` | The GitHub projection of this file | Anything not listed here |
| `signex-internal` → `docs/ROADMAP_DETAIL.md` | Near-term per-version scope decomposition | Gate definitions |

This rule exists because it was broken. Before 2026-07-15 the version
axis was defined in three places at once — this file, `MASTER_PLAN.md`
§7, and a set of hand-made GitHub milestones — and all three disagreed.
Two of them placed different features at v1.4 and v1.5. A fourth axis
(v0.15–v0.26) lived only in commit messages. See §9.

**If you need a version that this file does not list, add it here first.**

---

## 2. Where We Are Now

Honesty first, because the previous revision of this document did not
have this section and drifted six versions away from reality as a result.

| Fact | State (2026-07-18) |
|------|--------------------|
| Latest tagged release | **v0.14.0** (2026-07-18) |
| `workspace.package.version` | **0.14.0** |
| Latest CHANGELOG section | **0.14.0** — shipped 2026-07-18 |
| Current work | Editor fixes (v0.15.0), command registry + symbol multi-unit tail (v0.16.0) |

Phases 0–6 below (the original v0.1 → v0.8 schematic-editor plan) are
**shipped**. The work since v0.9 — the Apache-clean native-format
cutover, the library browser, the cleanroom renderer rewrite, and the
parametric sketch/footprint editor — was scope added *after* the original
phase model was written, and is not described by any phase. That is why
this document read as fiction until it was reconciled.

**Known gaps, tracked but not resolved by this document:**

- **v0.12.0 never existed** as a release. The cleanroom renderer rewrite
  planned for it merged into the long-running library branch; no tag, no
  CHANGELOG section. `CLEANROOM_REWRITE_PLAN.md` still says "Pending"
  and is wrong.

---

## 3. Versioning Strategy

Signex uses **semantic versioning**.

| Version | Meaning                                                          |
|---------|------------------------------------------------------------------|
| 0.x     | Pre-release. Foundation and editor build-out. No public users.    |
| 1.0     | First public release. Community schematic editor is real.         |
| 1.x     | Schematic-editor refinement. PCB development happens in parallel. |
| 2.0     | PCB viewer ships.                                                 |
| 2.x     | PCB build-out: routing, output, 3D, advanced, high-speed.         |
| 3.0     | Pro launch. Signal AI, plugins, and collaboration ship.           |
| 3.x     | Pro refinement.                                                   |
| 4.x     | Simulation, advanced output, foreign-format import.               |
| 5.x     | Signex 365 — cloud PLM platform.                                  |

**Major versions are gates, not arbitrary cuts.** v1.0 ships when the
schematic editor is production-ready — not when a date passes. We do not
ship v1.0 to "claim" a 1.0 milestone; we ship v1.0 because users can use
it.

**Version numbers are not work labels.** A version number means "this was
or will be tagged and released". It does not mean "the branch I am on".
This rule exists because ~100 commits shipped carrying labels v0.13
through v0.26-G, of which exactly one (v0.13.0) was ever released — which
is how this document and the milestone set drifted apart in the first
place. Use issue numbers and milestones to track in-flight work.

---

## 4. Priority Tiers

Every feature falls into one of four tiers. Tiers determine when a
feature *can* ship, not whether it ships at all.

| Tier   | Meaning                                              | Ships in       | Label |
|:------:|------------------------------------------------------|----------------|-------|
| **P0** | Required for v1.0. Cannot ship Community without it.  | v0.x–v1.0      | `tier: p0` |
| **P1** | Required for v2.0. Professional users expect it.      | v1.1–v2.0      | `tier: p1` |
| **P2** | Differentiator. Drives Pro adoption.                  | v2.1–v3.0      | `tier: p2` |
| **P3** | Nice-to-have. Stable core required first.             | v3.x and later | `tier: p3` |

**Decision rule:** if a feature is proposed for an earlier version than
its tier allows, the answer is "after the current target ships." This is
the single most important defense against scope creep.

`tier:` is not `priority:`. Tier is a scope gate ("which release can this
ship in"); priority is urgency ("how soon should someone look"). A
`tier: p2` issue can legitimately be `priority: critical`.

---

## 5. Workstreams

Parallel tracks of development. Each owns a set of crates (see
`REPOSITORY_AND_CODEBASE.md`). Workstreams are sized so one can be
assigned to one engineer without overlapping another's crates. Not every
workstream is active in every phase.

| ID      | Name                          | Owns                                         |
|---------|-------------------------------|----------------------------------------------|
| **WS-D**  | Document Layer              | `signex-document` (native `.snx*` raw doc)   |
| **WS-M**  | Semantic Model              | `signex-model`                               |
| **WS-E**  | Engine                      | `signex-engine`                              |
| **WS-R**  | Render                      | `signex-render`                              |
| **WS-U**  | UI Shell + Schematic Canvas | `signex-app` (shell, panels, schematic)      |
| **WS-V**  | Validation (ERC/DRC)        | `signex-erc`, `signex-drc`                   |
| **WS-O**  | Output                      | export modules (PDF, BOM, Gerber, etc.)      |
| **WS-P**  | PCB Geometry + Router       | `pcb-geom`, `pcb-router`                     |
| **WS-3D** | 3D Viewer                   | `signex-model-import`, `signex-render-3d`    |
| **WS-S**  | Simulation                  | `spice-bridge`, `openems-bridge`, etc.       |
| **WS-AI** | Signal AI (Pro)             | `signex-signal`                              |
| **WS-C**  | Collaboration (Pro)         | `signex-collab`, Supabase backend            |
| **WS-X**  | Plugins                     | `signex-plugin`                              |

Workstreams are deliberately **not** GitHub labels. They overlap the
`area:` family almost exactly, and two labels meaning the same thing is
how taxonomies rot.

---

## 6. Shipped — v0.1 → v0.14

Recorded so the phase model below is readable as history rather than
plan. Detail lives in `CHANGELOG.md`.

| Version | Shipped | Scope |
|---------|---------|-------|
| v0.1.0 – v0.6.4 | ✅ | Shell, canvas, schematic viewer, schematic editor, installers |
| v0.7.x | ✅ | ERC (11 rules), annotation, pin matrix, multi-window |
| v0.8.0 | ✅ | PDF / BOM / netlist export, multi-project workspaces |
| v0.9.x | ✅ | **Apache-clean cutover (#62)** — native `.snx*`, KiCad I/O → companion repo |
| v0.10.0 – v0.11.0 | ✅ | Library browser, `.snxlib` classes, master-detail browser |
| v0.12.0 | ⚠️ | Cleanroom renderer rewrite — merged, **never released** (see §2) |
| v0.13.0 | ✅ | Sketch mode α — Newton-LM constraint solver, footprint pad-bake |
| v0.14.0 | ✅ | Footprint editor parity, symbol multi-unit + polygon, `signex-net` netlist contract, keyboard-shortcut profiles, schematic GPU render, ADR-0001 decomposition |

This covers Phases 0–6 of the original plan. Those phases are closed.

---

## 7. Release Train — v0.15 → v1.0

Near-term and concrete. **The train is derived from the issue tracker,
not from the internal specs.** See §9 for why that distinction is
load-bearing. (v0.14.0 shipped 2026-07-18 — see §6.)

| Version | Scope | Milestone |
|---------|-------|-----------|
| **v0.15.0** | Editor fixes carrying `tier: p0`, plus Break Track / Drag Track End (the one item v0.14.0 actually deferred) | `v0.15.0 — Editor Fixes` |
| **v0.16.0** | Command Registry — one addressable action system (menus, keybindings, CLI, plugins) — and the symbol multi-unit tail | `v0.16.0 — Command Registry & Symbol Units` |
| v0.17+ | Not enumerated | *(gets versions when it gets milestones)* |

**v0.14.0 is one release, not two.** Its CHANGELOG section was written
2026-05-31 and never tagged; 170 commits landed past it. Rather than
invent a phantom (v0.12 is already one — planned, merged, never tagged),
v0.14.0 claims everything since v0.13.0.

**The v0.17+ band is deliberately empty.** It has been re-planned
repeatedly under labels that never shipped, and publishing a sequence
that has never survived contact with reality is how the previous revision
of this document lost credibility.

**Where near-term scope comes from.** The issue tracker: every open issue
carries a `tier:` label and, where scheduled, a milestone. The internal
specs (`signex-internal`) are **not** a source of forward scope — they
describe work that has largely already shipped. They remain useful as
implementation detail for whatever is genuinely outstanding.

---

## 8. Version Gates and Planned Versions — v1.0 → v5.4

Strategic. Undated by policy. A gate ships when its exit criteria pass.

**Not every version here is a gate.** The gates are v1.0, v2.0, v2.2,
v3.0, v4.0, and v5.0 — they carry exit criteria and block what follows.
Everything else is a planned release: real, scoped, and publishable, but
it gates nothing. Certainty decreases as the numbers grow. Read the tier
markers and §11 before treating anything here as a promise.

### v1.0.0 — Community Preview

Schematic-only early access. First shipped executables for Windows,
macOS, Linux.

**Exit criteria:**

- All v1.0 must-do items from `MASTER_PLAN.md` §5.1 complete
- Fixture corpus of 50+ native `.snx*` projects, all round-tripping cleanly
- A non-developer beta tester can install Signex, create or open a project,
  edit it, and save it without consulting a developer
- Issue tracker has zero `tier: p0` bugs

### v1.x — Schematic Refinement

Sub-versions are enumerated because they are staged releases, not
milestones-of-convenience: each one ships on its own. Order within a band
is firmer than the boundaries — scope may move between adjacent
sub-versions of the same band without a roadmap change, but may not move
between bands.

#### v1.1 — Advanced Schematic (P1)

| Version | Scope |
|---------|-------|
| **v1.1.0 — Advanced Schematic** | Design variants (fitted / not-fitted / alternate) with 7 drawing styles; multi-channel design (Repeat keyword, channel naming) including per-channel variant state; signal harnesses (connectors, entries, nested); Parameter Manager; net classes + diff-pair classes; smart paste (rubber stamp, paste array); change component |

#### v1.2 — SCH Tables & Docs (P1)

| Version | Scope |
|---------|-------|
| **v1.2.0 — SCH Tables & Docs** | Schematic tables (pin assignment, register map, connector pinout); auto-generated Table of Contents for multi-sheet designs; drawing tools (bezier, dimension annotation); named unions (hierarchical groups with tags) |

#### v1.3 — Enhanced Output (P1)

| Version | Scope |
|---------|-------|
| **v1.3.0 — Enhanced Output** | Smart PDF with bookmarks + PDF layers; variant-specific BOM export; watermarking (DRAFT / CONFIDENTIAL overlay); Output Job file; workspace layout save/restore |

#### v1.4 — Design Notebook (P1)

A first-class document type alongside Schematic and PCB: design
rationale, calculations, measurement data, and debug logs attached to the
design rather than to a separate file. The notebook is a Typst document
with Signex extensions for component / pin / net references.

| Version | Scope |
|---------|-------|
| **v1.4.0 — Design Notebook** | Notebook tab (fourth document mode); `signex-notebook` crate — Typst source model, compile pipeline, annotation types; split-pane source + live preview editor; syntax highlighting and autocomplete; notebook file format (Typst source + metadata sidecar); PDF export |
| **v1.4.1 — Design References & Annotations** | Custom Typst functions `@component` / `@pin` / `@net`; reference resolution against the active schematic with stale warnings; bidirectional navigation notebook ↔ schematic; schematic badge overlay with hover preview; annotation types (Note, Calculation, Measurement, Issue, Decision); dockable Annotations panel |
| **v1.4.2 — Measurements & Signal AI Integration** | `#measurement()` function (value + unit + conditions + timestamp); per-pin / per-component measurement timeline; attachments (scope screenshots, thermal images, simulation plots); stale detection when referenced values change; Signal AI reads and writes notebook context (Pro) |
| **v1.4.3 — Computational Layer** | `#python()` inline calculation cells; embedded CPython (`pyo3`) with pruned NumPy / SciPy / Matplotlib / pint / python-control / scikit-rf; schematic-value bridge (`@component(R7).value` → units-aware Python variable); inline Matplotlib plot rendering; result caching; sandboxed execution (no subprocess / network / out-of-project FS); slim installer variant without the Python embed |

Python is the only notebook runtime. Octave / MATLAB paste-and-run is
explicitly deferred; `python-control` + `scikit-rf` are deliberate
MATLAB-API clones covering the legacy use cases without a subprocess
bridge. The `ComputeRuntime` trait is pluggable if users surface demand.

#### v1.5 — Block Diagram (P1)

System-level architectural view supporting top-down design: start with
functional blocks and signal flows, then refine into detailed schematics.

| Version | Scope |
|---------|-------|
| **v1.5.0 — Block Diagram** | Block Diagram tab (fifth document mode); block canvas with customizable shapes / colours; block properties (function, rails, key specs, interfaces); signal-flow connections with protocol labels; interface annotations (SPI, I2C, UART, USB, LVDS, power, analog); block diagram file format; SVG / PDF export |
| **v1.5.1 — Schematic Linking & Power Tree** | Link blocks to schematic sheets (click block → navigate); auto-generate a block diagram from hierarchical sheet structure; power-tree visualisation auto-detected from the schematic; power budget annotations per block; signal-chain visualisation; block-level net summary; Signal AI block-diagram generation from a natural-language system description (Pro) |

### v2.0.0 — PCB Viewer

PCB rendering via `iced::widget::Shader`, 32 copper layers + technical
layers, layer stack panel, cross-probe with schematic, ratsnest. No
editing.

**Exit criteria:** five real `.snxpcb` fixtures render correctly;
10,000-track PCB pans and zooms at 60 fps; cross-probe works
bidirectionally.

### v2.1 — PCB Routing

Professional-grade interactive routing, implemented clean-room under
Apache-2.0 with no reference to other EDA tools' source or format docs.
Ships as five staged sub-releases. DRC (15 base rules) and net-class
management land in this band.

| Version | Scope |
|---------|-------|
| **v2.1.0 — PCB Routing** | Router stage 1, greedy single-trace. `pcb-geom` crate (polygon offset, R-tree, Delaunay, Minkowski); `pcb-router` skeleton (session model, preview / commit); single-trace routing with 45° / 90° / arc45 / arc90 corners; via placement (through / blind / buried); net-class editor (width, clearance, via size per class); live DRC feedback with the 15 base rules (`signex-drc`) |
| **v2.1.1 — Router: Walkaround** | Obstacle graph with clearance-inflated Minkowski offsets; A* pathfinder with corner / via / layer costs; corner-insertion pass preserving 45° / 90° / arc style; incremental debounced DRC on the proposed path; routing test-corpus tooling (50 open-source PCBs) |
| **v2.1.2 — Router: Push-and-Shove** | Topology-preserving shove solver with fixed-point iteration; rigidity heuristics (pad / via / edge proximity); via shove within a rigidity budget; preview buffer + commit barrier with Escape rollback; the whole shove cascade undone as one action |
| **v2.1.3 — Router: Diff Pairs & Length Tuning** | Coupled two-net router with gap control; length meander generator (accordion / trombone / sawtooth); length and skew reporting per net and per pair; teardrops as a design rule and a per-pad property |
| **v2.1.4 — Copper Pour** | Zone fill engine (polygon boolean via `pcb-geom`); thermal relief + island removal; fill priority and on-demand fill; back annotation / ECO |

Routing is the hardest single feature in EDA and is fundamentally one
engineer's deep work. See `PCB_ROUTER_PLAN.md` for the authoritative plan.

**Exit criteria:** 50-board fixture corpus routes end-to-end with no
panic, stuck state, or geometry corruption across 10,000 random routing
actions per board; shove converges in ≤8 iterations on 95% of actions;
median action latency ≤16 ms, 95th percentile ≤33 ms.

**Deferred:** multi-track routing, BGA fanout, via stitching → v2.2
candidates. Autorouting → not scoped.

### v2.2 — Community Release

Manufacturing output. v2.0–v2.2 together are a complete schematic + PCB
editor: design, validate, route, DRC, and export to fabrication.

| Version | Scope |
|---------|-------|
| **v2.2.0 — Community Release** | Gerber RS-274X + X2 export; Excellon drill export; ODB++ export |
| **v2.2.1 — Assembly & 3D Export** | Pick-and-place CSV; IPC-2581 export; STEP 3D export (board body); assembly SVG |

**Exit criteria:** a complete project can be designed, validated, and
exported to manufacturing files; a real PCB designed in Signex has been
successfully fabricated by at least one beta user; zero `tier: p0` bugs.

### v2.3 — 3D Viewer (P3)

The in-canvas realistic 3D view mode — Tier 1 of the two-tier 3D stack.
Keyboard `2` / `3` toggles the PCB canvas between flat-layered 2D and
orbitable 3D in place, no separate window. See `PCB_3D_RENDER_PLAN.md`.

| Version | Scope |
|---------|-------|
| **v2.3.0 — 3D Viewer** | `signex-scene3d` Scene IR crate (camera, materials, lights, units); `signex-board3d` board mesh + CSG cutouts + drill / via holes; board surface texture bake (albedo / normal / roughness / metallic); `BoardStackMaterials` + preset library; flat-2D view consumes the baked textures |
| **v2.3.1 — Realistic 3D View Mode** | `Realistic3D` view mode + `2` / `3` toggle; orbit camera rig (yaw / pitch / distance, middle-drag orbit, wheel zoom); `signex-render-wgpu-3d` PBR-lite pipeline; board rendered with baked textures, components as extruded courtyard blocks |
| **v2.3.2 — 3D Selection & Cross-Probe** | Ray-cast hit-test with parity to 2D selection; cross-probe from schematic orbits the 3D camera to the selected component; layer visibility honoured in 3D |
| **v2.3.3 — On-Canvas 3D Toolbar** | Floating toolbar in 3D mode; camera presets (Top, Iso, Front, Side, Back, Reset); board flip and component explode slider; HDRI preset cycle, screenshot, send-to-render-manager |
| **v2.3.4 — STEP → glTF Cache Pipeline** | `signex-3d-models` crate (STEP import, tessellation, glTF write); hashed on-disk model cache; family-heuristic material fallback; fallback extrusion for components with no model |
| **v2.3.5 — Material Sidecars & Controls** | `.snxmat` parser (face-attribute matching → PBR presets); `.snxmat` applied during STEP → glTF conversion; Properties-panel Board context (mask colour, silkscreen, surface finish); PBR preset library v1 (~30 presets) |
| **v2.3.6 — Curated Footprint Library** | Curated `.glb` models for the top 50 stock-library footprints (IPC-7351 standard package set); CI per-footprint golden-image render gate; missing-model on-demand download flow |
| **v2.3.7 — Parametric Generators: R / C / L** | Family generators for R_* (0201 … 2512, cement) and C_* / L_*; BOM-driven value / tolerance label bake; CI regenerates the full family glTF set on schema change |
| **v2.3.8 — Parametric Generators: IC Packages** | Family generators for SOIC / TSSOP / SSOP / QFP / QFN and BGA / LGA / DIP; pin-1 dot decals and part-number labels from glTF metadata |
| **v2.3.9 — Board-Scope Material Overrides** | Project-scoped `.snxmat` for custom silkscreen art / logos; per-board texture channel overrides; auto-bake vs. user-authored toggle per board |
| **v2.3.10 — Advanced Board Controls** | Mask misregistration; via tenting modes (Tented / Plugged / Open) with geometry response; edge bevel geometry; wear / weathering overlay |

### v2.4 — Advanced PCB (P3)

| Version | Scope |
|---------|-------|
| **v2.4.0 — Advanced PCB** | Full layer stack editor (εr, tan δ, copper weight, material library); stackup templates (2 / 4 / 6 / 8 layer, HDI, MCPCB); DRC rule profiles (fab-house and IPC presets, save / load); formal ECO dialog with change review |
| **v2.4.1 — Impedance & DRC Rules** | Impedance profile per layer pair + built-in calculator; impedance-controlled routing (Z0 display, width-from-impedance); additional DRC rules (silk-silk, acute angle, component clearance, height) |
| **v2.4.2 — Routing Polish & Geometry** | Route completion (loop removal) and glossing; split planes (negative plane layers); via-in-pad, paste / mask expansion rules; per-layer keepout, board cutouts, castellated holes |
| **v2.4.3 — Constraint Manager & HUD** | Constraint Manager (schematic-side spreadsheet rule editor); query-based rule scoping (InNet, InNetClass, OnLayer, boolean); Board Insight HUD (clearance, net name under cursor) |

### v2.5 — High-Speed Design (P3)

| Version | Scope |
|---------|-------|
| **v2.5.0 — High-Speed Design** | xSignals (pad-to-pad through-component path analysis); length-matching group management UI + bar-chart dashboard; topology constraints (star, chain, fly-by, T) |
| **v2.5.1 — DDR SI & PDN** | Eye diagram generation; DDR timing analysis (setup / hold vs. spec); channel simulation (Tx IBIS → S-parameter cascade → Rx IBIS); PDN impedance analysis Z(f); return-path and power-plane analysers |

### v2.6 — HQ Render / Blender Export (P3)

Offline path-traced renders via a Blender subprocess — Tier 2 of the 3D
stack. Tier 1 (v2.3.x) is preview; Tier 2 is export: product-shot stills,
marketing images, assembly renders. **Blender is detected, never
bundled.** See `PCB_3D_RENDER_PLAN.md` §4.

| Version | Scope |
|---------|-------|
| **v2.6.0 — Render Manager & Scene Export** | Render Manager dock panel (Appearance / Environment / Output / Queue); `signex-render-blender` crate — scene-bundle export (glTF + materials + board textures + render script); Blender auto-detect with a Preferences override |
| **v2.6.1 — Blender Addon & Single-Frame Render** | Signex Blender addon, auto-installed on first render; script generation (import glTF, wire HDRI, camera, lights, samples); single-frame render to PNG / JPG / EXR; "Install Blender" helper on macOS / Windows |
| **v2.6.2 — HDRI Presets & Lighting Rigs** | HDRI library (Studio / Workbench / Daylight / Pure White); lighting rig presets (key / fill / rim, product-shot, flat); sample count presets (Draft / Good / Final) |
| **v2.6.3 — Render Queue** | Multiple queued jobs with per-job progress and cancel; queue survives app restart; open-output-folder action and per-job log viewer |
| **v2.6.4 — Preview-Parity CI Gate** | CI renders a reference scene through Tier 1 and Tier 2 and diffs them; regression fails CI on material drift between preview and render; golden-image corpus covering common component families |

### v2.7 — Animations (P3)

Time-based renders: turntable demos, marketing videos, assembly
animations. Builds on the v2.6 render pipeline — same scene, same
materials, temporal camera / state.

| Version | Scope |
|---------|-------|
| **v2.7.0 — Turntable Animation** | Turntable camera path (360° yaw around target, configurable duration); MP4 / WebM export via Blender's ffmpeg encoder; background render thread so editing continues while a render runs |
| **v2.7.1 — Camera Path Editor** | Bezier camera tracks with keyframe editing in the Render Manager; timeline scrubber with per-keyframe camera preview in Tier 1; per-segment easing curves |
| **v2.7.2 — Assembly Animation** | Components drop in along BOM order with per-part easing; per-component delay / duration overrides; solder-reflow / appear-to-sit animation mode |
| **v2.7.3 — Simulation Overlay Bake-In** | Render-time thermal simulation heatmap overlay; render-time EM / signal-integrity overlay; per-frame interpolation for time-varying simulation data |

**v2.6 and v2.7 are P3 and are not gates.** They are published because
they are planned and scoped, not because they are committed. They carry
no exit criteria, they gate nothing, and of every band on this page they
are the most likely to move — including past v3.0. Their only hard
dependency is v2.3.

### v3.0.0 — Pro Release

Signal AI, plugin system, and live collaboration. Pro development happens
in feature-gated crates; the Community editor continues to ship.

| Version | Scope |
|---------|-------|
| **v3.0.0 — Pro Release** | Alp Lab API gateway client (streaming SSE); Pro license validation with an offline cache; compile-time feature gate (`#[cfg(feature = "pro")]`); Community build hides the Signal panel and shows an upgrade prompt |

**Exit criteria:** a Pro user can hold a productive design conversation
with Signal AI and its tool use is correctly undoable; three test users
can edit the same schematic and PCB simultaneously without conflicts;
pricing is set and subscription billing is operational; Pro and Community
binaries both build clean from a single workspace.

---

> **Everything below this line is a sketch, not a plan.** §11 applies with
> full force: the order and timing past v3.0 are speculative and user
> feedback will reshape them. These versions are enumerated so that
> contributors can see what is already scoped and avoid duplicating it —
> **not** because they are committed. Sub-version boundaries here are
> working guesses; expect them to merge, split, and reorder. Nothing in
> v3.1 → v5.4 carries exit criteria, and no version below gates anything.

### v3.1 — Signal AI Core (Pro)

| Version | Scope |
|---------|-------|
| **v3.1.0 — Chat & Context** | Signal panel (streaming chat, markdown rendering); design context injection (components, nets, ERC / DRC results, notebook annotations); locally persisted session history |
| **v3.1.1 — Visual & Polish** | Visual context (schematic / PCB screenshot to a vision model); usage meter; graceful offline degradation |

### v3.2 — Signal AI Tools (Pro)

| Version | Scope |
|---------|-------|
| **v3.2.0 — Edit & Analysis Tools** | Undoable edit tools (add_component, draw_wire, set_value, delete_element); analysis tools (check_erc, check_drc, check_si); ERC / DRC fix suggestions with auto-apply |
| **v3.2.1 — Sim & Templates** | Simulation tools (run_spice_sim, run_openems, run_thermal); notebook tools (read_notebook, write_notebook); design review mode (scored analysis, findings written to the notebook); circuit templates (buck, LDO, op-amp, filter, …); natural-language constraint entry and AI-guided routing suggestions |

### v3.3 — Plugin System (Pro)

Extism WASM runtime (`plugin-api` crate); five host function categories
(Document, Mutation, UI, Query, Sim); permission gateway (plugin manifest
+ user approval); undo-stack integration for plugin mutations;
hot-loading without restart.

**Not decomposed into sub-versions.** Unlike every other band on this
page, v3.3 has no sub-version breakdown yet — it is a single body of work
that has not been staged. It gets sub-versions when it gets a staging
plan, per §12.

### v3.4 — Live Collaboration (Pro)

| Version | Scope |
|---------|-------|
| **v3.4.0 — Realtime & CRDT** | Realtime WebSocket client (`signex-collab` crate); CRDT document model for conflict-free concurrent editing; auth (accounts, teams, roles) |
| **v3.4.1 — Presence & Cursors** | Per-user canvas cursors (coloured + name label); presence panel with online status and follow mode; sheet / region / layer / net locking |
| **v3.4.2 — Storage & Review** | Cloud project storage, version history + edit attribution; comments pinned to canvas locations with a review workflow; offline support (local SQLite op queue, replay on reconnect); server-side merge / notify / lock functions |

### v4.0 — Simulation View + SPICE

Simulation uses a **dedicated Simulation View** — a block-diagram
composition workspace separate from the schematic editor. See
`SIMULATION_VIEW.md`.

| Version | Scope |
|---------|-------|
| **v4.0.0 — Simulation** | Simulation view tab (third editor mode); stimulus block palette (V_DC, V_Pulse, V_Sine, V_PWL, I_DC, I_Pulse); load blocks (R_Load, RC_Load, RLC_Load) + GND reference; Sheet Block (import a schematic sheet as a sim block with auto-detected ports); port auto-detection from hierarchical labels and power pins; Manhattan wire routing between ports; analysis directive editor (DC op, DC sweep, AC, Transient); `.snxsim` file format; ngspice FFI (`spice-gen` crate) |
| **v4.0.1 — Waveform & Probes** | Netlist generation from the block diagram; subcircuit extraction from schematic sheets; voltage / current probe placement; run → ngspice → results; waveform panel (multi-trace, dual cursor, PNG / CSV export); `.raw` parser; DC, AC, Transient, Noise, Fourier analyses; probe ↔ trace cross-probe |

### v4.1 — Advanced Simulation

| Version | Scope |
|---------|-------|
| **v4.1.0 — PCB Geometry Blocks & Vendor Models** | PCB Trace Block (select net → auto-extract S-params via OpenEMS); PCB Via Block; PCB Region Block (area → multi-port network); IBIS Block (`.ibs` import, pin / model selection); S-Parameter Block (`.s2p` / `.s4p`) and Package Block; PRBS generator, clock source, eye diagram probe, TDR probe |
| **v4.1.1 — Advanced SPICE & EM** | Parameterized blocks (override values without editing the schematic); parameter sweep with multi-trace overlay; Monte Carlo and temperature sweep; corner analysis for passive tolerances; OpenEMS bridge (CSX writer, FDTD runner, HDF5 reader); S-parameter extraction, Smith chart, TDR; sim job queue with progress and cancel |
| **v4.1.2 — Thermal & Simulation Wizards** | Elmer FEM bridge (GMSH mesh, `.sif` writer, VTK reader); steady-state thermal analysis with component heat sources; DC IR drop (voltage distribution, current density); 3D thermal overlay on the PCB model; wizards for DDR SI, power supply, thermal, PDN impedance, and RF / antenna; wizard invocation from Signal AI |

### v4.2 — Advanced Output

| Version | Scope |
|---------|-------|
| **v4.2.0 — Panelization** | Panelization (step-and-repeat, rails, tabs, mouse bites); V-cut scoring / tab routing; auto-fiducials, tooling holes, test coupons |
| **v4.2.1 — Fab Docs & Export** | Drill table and board stackup report (PDF); DXF import / export; one-click validated manufacturing package; Gerber X3 |

### v4.3 — Import + Git

Each foreign-format importer is a **separate companion repo / binary**,
licensed appropriately for its source-format constraints and distributed
independently of the main workspace — the same architectural pattern as
`signex-kicad-import` shipped at v0.9.0. One-way conversion to native
`.snx*`, no compile-time dependency from the main repo.

| Version | Scope |
|---------|-------|
| **v4.3.0 — Altium & Eagle Import** | `signex-altium-import` companion (`.SchDoc`, `.PcbDoc`, `.SchLib`, `.PcbLib`, `.PrjPcb` → native, one-way); `signex-eagle-import` companion (`.sch`, `.brd` → native, one-way); creepage / clearance measurement + DRC (IEC 60950), in the main repo |
| **v4.3.1 — Built-in Git** | Built-in Git (branch, commit, merge, visual diff, blame); visual schematic diff as a canvas overlay; visual PCB diff as a canvas overlay |

### v4.4 — Polish

| Version | Scope |
|---------|-------|
| **v4.4.0 — Auto-Router & Advanced Routing** | Auto-router (topological) and semi-automatic ActiveRoute; copper balancing / thieving, backdrilling; thermal-aware routing, placement heatmap |
| **v4.4.1 — Advanced Geometry & Import** | Rigid-flex board, embedded components (cavity); OrCAD / PADS / Mentor import companions (separate repos, one-way); 3D clearance checking (body-to-body); antenna simulation (pattern, gain); interactive on-canvas impedance calculator |

### v5.0 — PLM Core

Signex 365 is the cloud platform connecting the desktop editor to the
PLM. See `PLM_INTEGRATION.md`.

| Version | Scope |
|---------|-------|
| **v5.0.0 — Signex 365** | Signex 365 web platform; shared auth (PLM + collaboration + desktop on one account); part-link table (PLM parts ↔ native symbol / footprint IDs by `ObjectId`); project-link table (PLM assemblies ↔ Signex projects) |
| **v5.0.1 — Component Picker** | Component Picker — desktop queries the PLM for part placement; fast paginated component-search service; lifecycle alerts in the schematic editor (EOL / NRND / Obsolete badges) |

### v5.1 — BOM Studio

| Version | Scope |
|---------|-------|
| **v5.1.0 — BOM Panel & Pricing** | BOM Studio panel in the desktop editor; live pricing + availability from cached distributor data; BOM cost roll-up per assembly |
| **v5.1.1 — Lifecycle & Risk** | Lifecycle status (active / NRND / EOL) and part-choice ranking; supply-chain risk alerts (single-source, low stock, long lead time); BOM cost optimisation suggestions (Signal AI); unified component model (symbol + footprint + 3D + sim linked) |

### v5.2 — PLM Sync

| Version | Scope |
|---------|-------|
| **v5.2.0 — BOM & ECO Sync** | BOM push from Signex to the PLM; delta sync (only changed BOM lines updated); ECO creation from Signex |
| **v5.2.1 — Documents & Lifecycle** | Document publishing (design files → PLM document management); batch lifecycle status check on file open |

### v5.3 — ERP Bridge

| Version | Scope |
|---------|-------|
| **v5.3.0 — Odoo / ERPNext Sync** | Released BOMs flow Signex → PLM → Odoo / ERPNext; ERP pricing flows back to BOM Studio as real purchase prices |
| **v5.3.1 — ECO & Vendor Sync** | ECO traceability through manufacturing (PLM → ERP); vendor sync (PLM AVL → ERP supplier list) |

### v5.4 — PLM Advanced

| Version | Scope |
|---------|-------|
| **v5.4.0 — Compliance & Obsolescence** | Compliance dashboard integration (desktop shows RoHS / REACH status); obsolescence monitoring with alternative suggestions; AVL badge on the component picker (approved / conditional / disqualified) |
| **v5.4.1 — PLM-Aware AI** | PLM-aware Signal AI (design review includes supply-chain analysis); reusable testbench library stored in the PLM (`.snxsim` templates) |

---

## 9. Reconciliation Log — 2026-07-15

What this revision changed, so the next reader knows why the old version
numbers do not match older documents:

- **Version axis consolidated here.** `MASTER_PLAN.md` §7 previously
  declared itself authoritative for versioning and carried the release
  table that generated the GitHub milestones. It now defers to this file.
- **v1.4 / v1.5 collision resolved.** This document said v1.4 = DRC and
  v1.5 = PCB output; `MASTER_PLAN.md` §7 said v1.4 = Design Notebook and
  v1.5 = Block Diagram. MASTER_PLAN's mapping wins — it was the one
  already reflected in the milestone set. **DRC moved into the v2.1
  routing band**, where it is actually built.
- **Router / Signal AI collision resolved.** Phase 9 claimed v2.1 for
  routing while Phase 12 claimed "v2.1 → v2.5" for Signal AI. Routing
  keeps v2.1.x; Signal AI moves to v3.1–v3.2.
- **Non-monotonic phase versions removed.** Phases previously ran
  v1.1 → v1.2 → **v2.1** → **v1.4** → v1.5. Phases no longer carry
  version numbers; they are narrative, and the version axis is §7–§8.
- **PCB viewer renumbered** v1.1 → v2.0, and "v2.0 = complete EDA tool"
  retired in favour of v2.2 = Community Release.
- **Ghost versions acknowledged.** v0.15–v0.26 existed only as work
  labels on commits and internal plans, never as releases. The train
  restarts from the real anchor (v0.14) in §7.
- **v4.0 / v5.0 promoted to gates.** Previously only half-covered by a
  "Beyond v3.0" table; they have dedicated internal plans
  (`SIMULATION_VIEW.md`, `PLM_INTEGRATION.md`) and milestones.

### Correction, same day

The first cut of §7 gated **v0.15.0 as "Footprint Editor Parity"**
(selection filter, units toggle, full pad stack), renumbered from the
internal specs' v0.17 band. **That work was already on `trunk`** —
`stack.rs` implements `Simple` / `TopMiddleBottom` / `FullStack`, and the
selection filter and units toggle exist. The roadmap gated work that had
already shipped.

The tell was in `CHANGELOG.md` the whole time: v0.14.0's own "Deferred to
v0.15" section lists exactly one item — *Break Track / Drag Track End* —
not parity.

The mistake is worth recording because it is the *same* mistake this
document was rewritten to fix. Renumbering the specs onto a real anchor
is not enough: **the specs describe work that has already shipped.** The
~100 commits of the `feature/library` branch carried labels v0.13→v0.26
and implemented most of the spec content; only v0.13.0 was ever tagged,
so the specs still read as forward plans.

**The rule that follows:** near-term scope comes from the **issue
tracker**, never from the specs. An issue is open because the work is
outstanding. A spec section is written because someone once intended it —
which says nothing about whether it exists today. Before gating a version
on spec content, grep the tree for it.

### Sub-version promotion, same day

§8 previously listed one row per band (v1.1.0 … v2.5.0) and stopped. The
internal plan decomposes those bands into staged sub-releases, and two
whole bands — **v2.6 HQ Render / Blender Export** and **v2.7 Animations**
— were absent from this document entirely. Contributors could not see
what was already scoped, and proposed work that was. All sub-versions
through v5.4 are now published here.

What that promotion does and does not mean:

- **It does not change any scope.** Every version below was already
  defined internally. Publishing it changes who can read it, not what
  ships.
- **It does not upgrade a sketch to a commitment.** v2.6 and v2.7 are P3.
  v3.1 → v5.4 remain speculative under §11 and are marked as such in §8.
  A milestone existing on GitHub is not a promise that the version will;
  it is a place to hang an issue.
- **v3.3 is published at band level only.** The internal plan has a
  feature list for the plugin system but no sub-version staging for it,
  and inventing `v3.3.0` here to make the table symmetric would violate
  the first rule in §12. It gets sub-versions when someone stages it.
- **Sub-version boundaries within a band are the softest thing on this
  page.** Scope may move between adjacent sub-versions of the same band
  without amending this document. It may not move between bands.

---

## 10. Staffing Assumptions

Planned for a **small team**: 2–3 engineers during v0.x–v1.0, scaling to
3–5 during v2.0 and v3.0.

- A single engineer typically owns one workstream at a time
- If the team shrinks to one engineer, phase durations approximately
  double; the order stays the same, only velocity changes
- **What does not scale:** routing is one engineer's deep work for 3+
  months — the canonical example of a feature where adding people slows
  things down. Architectural decisions are made by a small group and then
  communicated; they are not decided in committee.

---

## 11. What This Roadmap Does Not Commit To

- **Calendar dates.** Estimates are working numbers. They will move. We
  do not announce ship dates publicly until a release candidate exists.
- **Feature ordering within a version.** May shift on dependencies and
  availability.
- **The v0.17+ band and everything past v3.0.** Order and timing are
  speculative; user feedback will reshape them. §8 enumerates the
  sub-versions of those bands so that scoped work is visible to
  contributors. **Enumeration is not commitment.** A version that has a
  heading and a milestone is still a sketch if it sits past v3.0.
- **Sub-version boundaries within a band.** Which sub-release a feature
  lands in may change without amending this document; which *band* it
  lands in may not.
- **Pricing (Pro).** Set when v3.0 is in beta, not earlier.
- **Performance numbers.** The targets here are exit criteria, not
  user-facing promises. Real-world performance varies with hardware and
  design complexity.

---

## 12. Rules for Changing This Document

- **Add the version here before using it anywhere.** Not in a commit
  message, not in a milestone, not in an internal plan. Here first.
- **Gate order does not change** without an architectural reason. "We
  changed our minds" is not sufficient.
- **Adding scope to a version requires removing equivalent scope from the
  same version.** The version does not silently grow.
- **Pulling a feature forward** requires explicit justification and an
  updated tier classification. **Pushing a feature back** is always
  allowed and needs no justification beyond "we underestimated."
- **Milestones follow this file, never lead it.** `.github/milestones.yml`
  is a projection; edit the roadmap first.
- **Quarterly review**, and reconcile against `CHANGELOG.md` + `git tag`
  every time. If this document and the tags disagree, the tags are right
  and this document is broken — fix it the same day and add a line to §9.
