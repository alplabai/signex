# Signex — Roadmap

> **Status:** Living document. Updated quarterly. Last reconciled against
> reality 2026-11-07 (PCB design moved first, §9); Pro planning moved to
> the private Pro repository 2026-11-17 (§9); Community goal decisions
> 2026-11-17 (§9).
> **Audience:** Anyone planning work, allocating engineering time, or
> communicating timelines.
> **Owns:** the release axis. The ordering, names and gates of every planned
> Signex release, and the versioning policy (§3). Forward-looking plans are
> identified by name, never by version number.

This document describes *when* things ship. It does not describe *what*
features do — that is the product plan in the private `signex-internal`
repo (`docs/internal`, not in this checkout). It does not describe *how*
they are implemented — that is `docs/architecture/` and
`docs/REPOSITORY_AND_CODEBASE.md`.

This roadmap covers Signex Community only. Signex Pro is planned
separately; see §3, "Signex Pro".

The dates and durations here are working estimates. They are not
commitments. They will move. The order and the gates are firmer than the
dates.

---

## 1. Release Axis Ownership

**This document is the single source of truth for the release axis.**
Nothing else defines what ships in a release.

| Source | Owns | Does not own |
|--------|------|--------------|
| `docs/ROADMAP.md` (this file) | Plan names, ordering, gates, exit criteria, the versioning policy | Feature semantics |
| Product plan (`signex-internal`) | Scope, principles, product thesis, format stance | Plan names, version numbers |
| `.github/milestones.yml` | The GitHub projection of this file | Anything not listed here |
| `signex-internal` → `docs/ROADMAP_DETAIL.md` | Near-term per-plan scope decomposition | Gate definitions |

This rule exists because it was broken. Before 2026-07-15 the release
axis was defined in three places at once — this file, `MASTER_PLAN.md`
§7, and a set of hand-made GitHub milestones — and all three disagreed.
Two of them placed different features at v1.4 and v1.5. A fourth axis
(v0.15–v0.26) lived only in commit messages. See §9.

**If you need a plan that this file does not list, add it here first.**

---

## 2. Where We Are Now

Honesty first, because the previous revision of this document did not
have this section and drifted six versions away from reality as a result.
| Fact | State (2026-11-17) |
|------|--------------------|
| Latest tagged release | **v26.11.0** (2026-11-17), the Community Preview release and the first full CalVer release; the schematic and PCB editors are the supported surface. **v26.11.0-rc.1** (2026-11-07) was its release candidate; **v1.0.0-rc.1** (2026-10-29) was the **last SemVer tag**, and `1.0.0` final is never cut |
| `workspace.package.version` | **26.11.0** |
| Latest CHANGELOG section | **26.11.0** — dated 2026-11-17 |
| Current work | Community Preview milestone: open issues are tracked on [the milestone](https://github.com/alplabai/signex/milestone/12) (a dated issue list here goes stale). AI Design work runs alongside (§8) |

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

Signex uses **calendar versioning** ([CalVer](https://calver.org)) since
2026-11. Releases up to and including `v1.0.0-rc.1` (2026-10-29) used
semantic versioning; `v1.0.0-rc.1` is the **last SemVer tag** and `1.0.0`
final is never cut. All already-shipped versions (v0.1.0 … v0.15.0,
v1.0.0-rc.1) keep their numbers as history.

**Format: `YY.MM.MICRO`**

| Part | Rule |
|------|------|
| `YY` | Two-digit year of the release (calver.org `YY`: year − 2000, no padding): 2026 is `26`. |
| `MM` | Month of the release, **without zero padding** (calver.org `MM`): January 2027 is `27.1.0`, never `27.01.0`. Cargo and semver reject leading zeros, so a padded month would not parse. |
| `MICRO` | Starts at `0` each month and increments for every release in that month, patch releases included. |
| Release candidate | `YY.MM.MICRO-rc.N`, e.g. `26.11.0-rc.1`. A release's version is fixed when its first release candidate is cut; the final keeps it even if it ships in a later month (`26.11.0-rc.1` → `26.11.0`). |
| Git tag | The `v` prefix stays: `v26.11.0`, `v26.11.0-rc.1`. The `release.yml` trigger (`v[0-9]+.[0-9]+.[0-9]+` / `-*`) and `scripts/check_release_version.sh` accept this shape unchanged. |

Ordering stays monotonic because every CalVer major (26 and up) is above
the last SemVer major (1).

The version is dated by the month its first release candidate is cut, or,
for a release with no candidate, the month it ships; never by the month the
work started. The first CalVer release-prep PR sets
`workspace.package.version` (still `1.0.0-rc.1`).

**File-format compatibility is not carried by the app version.** It is
carried by the format tokens (`snxsch/1` and the other `.snx*` tokens). A newer
app reads older files by token, and an older app refuses a newer token; the
calendar version says nothing about either.

**Gates are named, not numbered.** A gate ships when the product is ready,
not when a date passes. The Community Preview ships because the schematic editor is
production-ready and users can use it, not to claim a round number. The gates are the
Community Preview, PCB Design, Community Release and Simulation (§8). The
stages, in order:

| Stage | Meaning |
|-------|---------|
| Pre-1.0 (shipped, §6) | Foundation and editor build-out. No public users. |
| Community Preview | First public release. Community schematic editor is real. |
| PCB Design | PCB layout from the schematic and routing, built together. |
| Schematic Refinement | Schematic-editor refinement, Design Notebook and Block Diagram. Ships before the Community Release. |
| Community Release | Manufacturing output for the boards PCB Design produces. |
| PCB build-out | 3D, advanced PCB, high-speed, the rest of the router. |
| Plugin System | The plugin system. |
| Simulation band | Simulation, advanced output, foreign-format import. |

**Version numbers are not work labels, and not plan labels.** A version
number means "this was tagged and released". It does not mean "the branch
I am on", and it does not name a future release: which release carries a
feature is known only when it ships. Plans are identified by **name**
(Community Preview, PCB Design, PCB Layout, …). This rule exists
because ~100 commits shipped carrying labels v0.13 through v0.26-G, of
which exactly one (v0.13.0) was ever released — which is how this document
and the milestone set drifted apart in the first place. Use issue numbers
and milestones to track in-flight work.

**Community** (FSL-1.1-ALv2, offline forever; maintainer decision,
2026-11-08) is the full editor — schematic, PCB layout, routing, DRC, the
plugin system and the headless CLI. Nothing is locked, and there are no
upgrade prompts.

### Signex Pro

Signex Pro is a separate, private, proprietary product built on Signex
Community. It adds Signex AI (agent mode, the generator and hosted AI),
team collaboration and PLM. Signex Community stays complete and works
offline without it. Pro is planned in its own private roadmap, so this
document gives no Pro dates, gates, prices or design details.

---

## 4. Priority Tiers

Every feature falls into one of four tiers. Tiers determine when a
feature *can* ship, not whether it ships at all.

| Tier   | Meaning                                              | Ships in       | Label |
|:------:|------------------------------------------------------|----------------|-------|
| **P0** | Required for the Community Preview. Cannot ship Community without it. | Up to and including Community Preview | `tier: p0` |
| **P1** | Required for PCB Design, or a professional schematic feature users expect. | PCB Design, then Schematic Refinement, both before the Community Release | `tier: p1` |
| **P2** | Differentiator.                                       | PCB Routing through the Community Release | `tier: p2` |
| **P3** | Nice-to-have. Stable core required first.             | After the Community Release (the P3 bands of §8) | `tier: p3` |

**PCB Design first (2026-11-07):** the plans of the PCB Design gate (§8) are
P1 and are scheduled first. Schematic Refinement issues keep their
`tier: p1` label; they are not re-triaged.

**Schematic Refinement before the Community Release (2026-11-17):** all
five Schematic Refinement bands (Advanced Schematic, SCH Tables & Docs,
Enhanced Output, Design Notebook, Block Diagram) ship before the Community
Release. Their tier stays P1.

**Decision rule:** if a feature is proposed for an earlier plan than
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
| **WS-P**  | PCB Geometry + Router       | `pcb-geom` (routing in `pcb-geom::route`)    |
| **WS-3D** | 3D Viewer                   | `signex-model-import`, `signex-render-3d`    |
| **WS-MC** | MCAD                        | `signex-mcad`                                |
| **WS-S**  | Simulation                  | `spice-bridge`, `openems-bridge`, etc.       |
| **WS-X**  | Plugins                     | `signex-plugin`                              |

Workstreams are deliberately **not** GitHub labels. They overlap the
`area:` family almost exactly, and two labels meaning the same thing is
how taxonomies rot.

---

## 6. Shipped — v0.1 → v0.15

Recorded so the phase model below is readable as history rather than
plan. Detail lives in `CHANGELOG.md`.

| Version | Shipped | Scope |
|---------|---------|-------|
| v0.1.0 – v0.6.4 | ✅ | Shell, canvas, schematic viewer, schematic editor, installers |
| v0.7.x | ✅ | ERC (11 rules), annotation, pin matrix, multi-window |
| v0.8.0 | ✅ | PDF / BOM / netlist export, multi-project workspaces |
| v0.9.x | ✅ | **Apache-clean cutover (#62)** — native `.snx*`, external-format I/O → companion repo |
| v0.10.0 – v0.11.0 | ✅ | Library browser, `.snxlib` classes, master-detail browser |
| v0.12.0 | ⚠️ | Cleanroom renderer rewrite — merged, **never released** (see §2) |
| v0.13.0 | ✅ | Sketch mode α — Newton-LM constraint solver, footprint pad-bake |
| v0.14.0 | ✅ | Footprint editor parity, symbol multi-unit + polygon, `signex-net` netlist contract, keyboard-shortcut profiles, schematic GPU render, ADR-0001 decomposition |
| v0.15.0 | ✅ | Editor fixes: footprint/symbol editor correctness, order-independent netlist with ERC/BOM on one topology, crash-safe persistence, pinned Rust 1.97.0 + enforced rustfmt and god-file gates |

This covers Phases 0–6 of the original plan. Those phases are closed.

---

## 7. Release Train — toward the Community Preview

Near-term and concrete. **The train is derived from the issue tracker,
not from the internal specs.** See §9 for why that distinction is
load-bearing. (v0.15.0 shipped 2026-07-23 — see §6.)

| Plan | Scope | Milestone |
|------|-------|-----------|
| **Command Registry & Symbol Units** | Command catalog groundwork (menus read one label/shortcut table, #271), root `Message` namespacing (#162), table/settings write cost (#416), and the symbol multi-unit tail. The full Command Registry epic (#278: dispatch, palette, CLI, plugins) moved to Advanced Schematic. **Shipped in 1.0.0-rc.1; test follow-up #1727 landed after the tag and ships in the next release.** The milestone is closed. | `Command Registry & Symbol Units` |
| Later plans | Not enumerated | *(get names when they get milestones)* |

**v0.14.0 is one release, not two.** Its CHANGELOG section was written
2026-05-31 and never tagged; 170 commits landed past it. Rather than
invent a phantom (v0.12 is already one — planned, merged, never tagged),
v0.14.0 claims everything since v0.13.0.

**The band after the Command Registry plan is deliberately empty.** It has been re-planned
repeatedly under labels that never shipped, and publishing a sequence
that has never survived contact with reality is how the previous revision
of this document lost credibility.

**Where near-term scope comes from.** The issue tracker: every open issue
carries a `tier:` label and, where scheduled, a milestone. The internal
specs (`signex-internal`) are **not** a source of forward scope — they
describe work that has largely already shipped. They remain useful as
implementation detail for whatever is genuinely outstanding.

---

## 8. Gates and Planned Releases — Community Preview → Polish

Strategic. Undated by policy. A gate ships when its exit criteria pass.

**Not every plan here is a gate.** The gates are Community Preview, PCB
Design, Community Release and Simulation, in that order — they block what
follows. (PCB Design took the place of the PCB Viewer, the former v2.0, on
2026-11-07; the others are the former v1.0, v2.2 and v4.0.) The first three
carry exit criteria below; the exit criteria of the Simulation gate are
written when its band is planned. Everything else is a planned
release: real, scoped, and publishable, but it gates nothing. Certainty
decreases as the list goes on. Which release number carries a plan is known
only when it ships (§3). Read the tier
markers and §11 before treating anything here as a promise.

### Community Preview

Schematic-only early access. First shipped executables for Windows,
macOS, Linux.

**Exit criteria:**

- All Community Preview must-do items from `MASTER_PLAN.md` §5.1 complete
- Fixture corpus of 50+ native `.snx*` projects, all round-tripping cleanly
- A non-developer beta tester can install Signex, create or open a project,
  edit it, and save it without consulting a developer
- Issue tracker has zero `tier: p0` bugs

**Status against the criteria (2026-10-29), for the rc.1 decision:**

- *Must-do items:* `MASTER_PLAN.md` is in the private `signex-internal` repo
  and is not checked here. The open Community Preview milestone issues are listed in §2.
- *Fixture corpus:* **open question.** `crates/signex-types/tests/corpus/`
  holds 43 `.snxsch`, 36 `.snxpcb` and 8 `.snxprj` files, each a small
  single-feature case, plus the bundled sample project. Whether those count as
  "50+ native projects" is a product decision; as whole projects there are
  8 `.snxprj`.
- *Non-developer beta tester:* **not yet exercised.** The install-to-PDF
  walkthrough in `docs/USER_GUIDE.md` has not been followed end to end by a
  person with a fresh install. The release packages are unsigned
  (`docs/RELEASE_SIGNING.md`), so Windows and macOS first-launch warnings apply.
- *Zero `tier: p0` bugs:* no open issue carries both `tier: p0` and
  `type: bug`. Open `tier: p0` issues exist that are not bugs (#859, #322,
  #271, #278). #115 moved to the Pro repository on 2026-11-17.

A release candidate is a tag of the form `vYY.MM.MICRO-rc.N`; `release.yml`
builds the same packages as a final release and publishes them as a
pre-release. (`v1.0.0-rc.1`, the last SemVer tag, was the first candidate.)

### PCB Design (gate) — board layout and routing from the schematic

Maintainer decision, 2026-11-07: designing boards from the schematic comes
first after the Community Preview, and it ships in the first release after
it. The plans below are built in parallel. Schematic Refinement, all five
of its bands, follows and ships before the Community Release (2026-11-17).
Every other band on this page waits until this gate ships: the 3D,
Advanced PCB,
High-Speed, HQ Render and Animation bands, the Plugin System and
Simulation. The PCB Viewer gate is folded into the PCB Layout plan here: a
viewer for boards that nothing in Signex can create is not a milestone
worth stopping at. The Command Registry (#278) moves here from Advanced
Schematic, so every edit is one undoable command reachable from the menus,
the keys and the CLI. The GitHub milestone for this gate keeps its earlier
title, `AI Design`, so existing issues keep their milestone.

| Plan | Scope |
|------|-------|
| **PCB Layout** | One part identity across library, schematic and board (#663); a new `.snxpcb` in a project; board outline; Update PCB from schematic with a reviewable change list (#1312); component placement (move, rotate, flip); ratsnest; cross-probe in both directions; PCB rendering via `iced::widget::Shader` with 32 copper and the technical layers, and the layer panel (the former PCB Viewer scope) |
| **PCB Routing** | As in the PCB Routing band below: single-trace routing, vias, net classes, live DRC with the 15 base rules |
| **Router: Walkaround** | As below: obstacle-aware A* routing |
| **Requirements & Verification** | Can ship on its own, ahead of the rest of this gate. The project requirements list (ERC / DRC rules, net classes, rail budgets, board size, BOM cost and approved parts) as deterministic checks that run like ERC and DRC; pass and fail with the evidence for each requirement |

**Exit criteria:**

- A schematic drawn in Signex becomes a board: every footprint placed from
  the schematic and every net shown as ratsnest; five real `.snxpcb`
  fixtures render correctly; a 10,000-track board pans and zooms at 60 fps;
  cross-probe works in both directions.

### Schematic Refinement

**Ships before the Community Release (2026-11-17):** Advanced Schematic,
SCH Tables & Docs, Enhanced Output, Design Notebook and Block Diagram.
Only the Community scope written here is pulled in; Signex AI parts are
Pro and planned in the private Pro roadmap. The Command Registry (#278)
moved from Advanced Schematic to PCB Design.

**Rules for the whole band (2026-11-17):**

- **Format versioning lands first.** Before any Schematic Refinement
  addition to a file format, every `.snx*` file carries a version stamp,
  Signex refuses to save over a file written by a newer version, and the
  user gets a clear error saying so (#2207).
- **Signex names.** Plan text and the UI use Signex's own names: Object
  List, PDF Export, Export Job, Net Blanket and the Net Parameters
  directive. Harnesses keep the generic name.
- **Dock panels.** The Object List, the compiled design view and the
  Parameter Manager are dock panels.
- **Rich notes.** Typst (Apache-2.0) is allowed as a dependency for rich
  text notes (#1114). A project carries a font (#1178) only when the
  font's licence permits redistribution.

Sub-versions are enumerated because they are staged releases, not
milestones-of-convenience: each one ships on its own. Order within a band
is firmer than the boundaries — scope may move between adjacent
sub-versions of the same band without a roadmap change, but may not move
between bands.

#### Advanced Schematic (P1)

| Plan | Scope |
|------|-------|
| **Advanced Schematic** | Design variants (fitted / not-fitted / alternate) with 7 drawing styles; multi-channel design (Repeat keyword, channel naming) including per-channel variant state; signal harnesses (connectors, entries, nested); Parameter Manager, Object List and compiled design view as dock panels; net classes + diff-pair classes, assigned with a Net Blanket or a Net Parameters directive; smart paste (rubber stamp, paste array); change component; Command Registry (#278: dispatch, command palette, args, CLI, plugin hooks) |

**Exit criteria (draft, 2026-11-17, for maintainer review):**

- A 4-channel design with two variants exports a correct per-variant BOM
  and PDF; the project is a fixture with a golden BOM per variant.
- A nested harness connects through its entries, and the netlist of a
  harness fixture matches its golden netlist.
- Net classes and diff-pair classes assigned with a Net Blanket and with a
  Net Parameters directive appear in the netlist and reach the board through
  Update PCB.
- A Parameter Manager edit across several sheets is one undo step; the
  Object List and the Parameter Manager show the same values as the canvas.
- Every new feature is a catalog command with tests, and every new file
  row round-trips in the fixture corpus.

#### SCH Tables & Docs (P1)

| Plan | Scope |
|------|-------|
| **SCH Tables & Docs** | Schematic tables (pin assignment, register map, connector pinout); auto-generated Table of Contents for multi-sheet designs; drawing tools (bezier, dimension annotation); named unions (hierarchical groups with tags) |

**Exit criteria (draft, 2026-11-17, for maintainer review):**

- Tables of contents and pin tables render on the canvas and export to PDF
  and match their golden output.
- A table of contents follows a sheet that is added, renamed or removed,
  and a pin table follows a pin edit on its part.
- A rich text note renders the same on the canvas and in the PDF; a
  project-carried font is used by both.
- Bezier and dimension objects round-trip in the fixture corpus.
- Every new feature is a catalog command with tests.

#### Enhanced Output (P1)

| Plan | Scope |
|------|-------|
| **Enhanced Output** | PDF Export with bookmarks + PDF layers; variant-specific BOM export; watermarking (DRAFT / CONFIDENTIAL overlay); Export Job file; workspace layout save/restore |

**Exit criteria (draft, 2026-11-17, for maintainer review):**

- An Export Job reproduces a full PDF, BOM and netlist set headless, and
  two runs on the same project give the same files.
- The exported PDF has a bookmark per sheet and per part, and its PDF
  layers and watermark show in a standard PDF reader.
- A variant BOM export of a fixture project matches its golden BOM.
- Saved workspace layouts restore the same panels and documents.
- Every new feature is a catalog command with tests.

#### Design Notebook (P1)

A first-class document type alongside Schematic and PCB: design
rationale, calculations, measurement data, and debug logs attached to the
design rather than to a separate file. The notebook is a Typst document
with Signex extensions for component / pin / net references.

| Plan | Scope |
|------|-------|
| **Design Notebook** | Notebook tab (fourth document mode); `signex-notebook` crate — Typst source model, compile pipeline, annotation types; split-pane source + live preview editor; syntax highlighting and autocomplete; notebook file format (Typst source + metadata sidecar); PDF export |
| **Design References & Annotations** | Custom Typst functions `@component` / `@pin` / `@net`; reference resolution against the active schematic with stale warnings; bidirectional navigation notebook ↔ schematic; schematic badge overlay with hover preview; annotation types (Note, Calculation, Measurement, Issue, Decision); dockable Annotations panel |
| **Measurements & Signex AI Integration** | `#measurement()` function (value + unit + conditions + timestamp); per-pin / per-component measurement timeline; attachments (scope screenshots, thermal images, simulation plots); stale detection when referenced values change |
| **Computational Layer** | `#python()` inline calculation cells; embedded CPython (`pyo3`) with pruned NumPy / SciPy / Matplotlib / pint / python-control / scikit-rf; schematic-value bridge (`@component(R7).value` → units-aware Python variable); inline Matplotlib plot rendering; result caching; sandboxed execution (no subprocess / network / out-of-project FS); slim installer variant without the Python embed |

Python is the only notebook runtime. Octave / MATLAB paste-and-run is
explicitly deferred; `python-control` + `scikit-rf` are deliberate
MATLAB-API clones covering the legacy use cases without a subprocess
bridge. The `ComputeRuntime` trait is pluggable if users surface demand.

The Measurements & Signex AI Integration plan is pulled in for the
measurement scope listed above only; its Signex AI part is Pro.

**Exit criteria (draft, 2026-11-17, for maintainer review):**

- A notebook with `@component`, `@pin` and `@net` references opens beside
  its project, every reference resolves, and a renamed or deleted target
  shows a stale warning.
- Navigating from a notebook reference to the schematic and back works in
  both directions.
- A notebook fixture exports to PDF, and two exports of it give the same
  file.
- A `#measurement()` entry shows in the timeline of its pin or part and is
  marked stale when the referenced value changes.
- A `#python()` cell computes from a schematic value with units, and the
  sandbox refuses subprocess, network and out-of-project file access, each
  pinned by a test.
- The notebook file round-trips in the fixture corpus; every new feature is
  a catalog command with tests.

#### Block Diagram (P1)

System-level architectural view supporting top-down design: start with
functional blocks and signal flows, then refine into detailed schematics.

| Plan | Scope |
|------|-------|
| **Block Diagram** | Block Diagram tab (fifth document mode); block canvas with customizable shapes / colours; block properties (function, rails, key specs, interfaces); signal-flow connections with protocol labels; interface annotations (SPI, I2C, UART, USB, LVDS, power, analog); block diagram file format; SVG / PDF export |
| **Schematic Linking & Power Tree** | Link blocks to schematic sheets (click block → navigate); auto-generate a block diagram from hierarchical sheet structure; power-tree visualisation auto-detected from the schematic; power budget annotations per block; signal-chain visualisation; block-level net summary |

**Exit criteria (draft, 2026-11-17, for maintainer review):**

- A block diagram generated from a hierarchical fixture project has one
  block per sheet and matches its golden diagram.
- Clicking a linked block opens its sheet, and the link follows a sheet
  rename.
- The power tree of a fixture project lists every rail from the schematic,
  with the budget per block.
- A block diagram exports to SVG and PDF, and the file round-trips in the
  fixture corpus.
- Every new feature is a catalog command with tests.

### PCB Viewer

**Folded into PCB Design (2026-11-07)** as the PCB Layout plan, with board
creation from the schematic and placement added. Its exit criteria are part
of the PCB Design gate's.

### PCB Routing

**PCB Routing and Router: Walkaround are built in PCB Design** (2026-11-07);
the plans after them in this band wait until it ships.

Professional-grade interactive routing, implemented clean-room under
FSL-1.1-ALv2 with no reference to other EDA tools' source or format docs.
Ships as five staged sub-releases. DRC (15 base rules) and net-class
management land in this band.

| Plan | Scope |
|------|-------|
| **PCB Routing** | Router stage 1, greedy single-trace. `pcb-geom` crate (polygon offset, R-tree, Delaunay, Minkowski); router session (preview / commit) in `pcb-geom::route`, driven by `signex-engine` (`pcb::route`); there is no separate router crate; single-trace routing with 45° / 90° / arc45 / arc90 corners; via placement (through / blind / buried); net-class editor (width, clearance, via size per class); live DRC feedback with the 15 base rules (`signex-drc`) |
| **Router: Walkaround** | Obstacle graph with clearance-inflated Minkowski offsets; A* pathfinder with corner / via / layer costs; corner-insertion pass preserving 45° / 90° / arc style; incremental debounced DRC on the proposed path; routing test-corpus tooling (50 boards under permissive licences, see below) |
| **Router: Push-and-Shove** | Topology-preserving shove solver with fixed-point iteration; rigidity heuristics (pad / via / edge proximity); via shove within a rigidity budget; preview buffer + commit barrier with Escape rollback; the whole shove cascade undone as one action |
| **Router: Diff Pairs & Length Tuning** | Coupled two-net router with gap control; length meander generator (accordion / trombone / sawtooth); length and skew reporting per net and per pair; teardrops as a design rule and a per-pad property |
| **Copper Pour** | Zone fill engine (polygon boolean via `pcb-geom`); thermal relief + island removal; fill priority and on-demand fill; back annotation / ECO |

Routing is the hardest single feature in EDA and is fundamentally one
engineer's deep work. See `PCB_ROUTER_PLAN.md` for the authoritative plan.

**Routing test corpus (2026-11-17):** the corpus boards come only from
permissive licences (MIT, CC-BY, CERN-OHL-P), imported through the import
companion. Each file records its source and licence in a manifest. The
corpus lives outside `crates/`, in `testdata/routing-corpus/`.

**Exit criteria:** 50-board fixture corpus routes end-to-end with no
panic, stuck state, or geometry corruption across 10,000 random routing
actions per board; shove converges in ≤8 iterations on 95% of actions;
median action latency ≤16 ms, 95th percentile ≤33 ms.

**Deferred:** multi-track routing, BGA fanout, via stitching → Community Release
candidates. Autorouting → not scoped.

### Community Release

Manufacturing output. PCB Design and Community Release together are a complete schematic + PCB
editor: design, validate, route, DRC, and export to fabrication.

| Plan | Scope |
|------|-------|
| **Community Release** | Gerber RS-274X + X2 export; Excellon drill export; ODB++ export; added 2026-11-17: PCB rooms from schematic sheets (#2457); Command Log panel (#2458); PCB status-bar counters (#2460); compare board with schematic (#2462); agent-socket coverage of the existing catalog commands (#2470) |
| **Assembly & 3D Export** | Pick-and-place CSV; IPC-2581 export; STEP 3D export (board body), written with Signex's own STEP translator from MCAD Kernel, pulled forward (2026-11-17); assembly SVG |
| **Scripted Demo Videos** | Scripts that run catalog commands through the existing agent socket (or the CLI), with no second control port; one story script per feature (schematic to board, rules, routing); screen capture with a drawn pointer, click ripples and captions; idle stretches cut; MP4 and GIF output for the README, the docs and the website, re-recorded on every release so the videos never go stale |

**Exit criteria:** a complete project can be designed, validated, and
exported to manufacturing files; a real PCB designed in Signex has been
successfully fabricated by at least one beta user; zero `tier: p0` bugs.

### Internationalisation (tier not decided)

Every user-facing string comes from a message catalog and the UI language
switches at runtime, while everything Signex writes stays byte-identical
whatever the language. Epic #2053; decisions in `DECISIONS.md` (2026-11-07,
2026-11-08). No milestone and no tier yet; its place in this list implies no
order.

Infrastructure first, UI binding last: the Preferences language picker and
the area migrations come after the infrastructure.

| Plan | Scope |
|------|-------|
| **Internationalisation** | `signex-i18n` crate (Fluent + ICU4X), `Localized` display-only type, runtime language switch with the locale persisted in preferences (#2055); API gate, key-consistency and locale-invariance tests (#2056); locale-aware formatting of non-engineering numbers, dates, lists and case (#2063: library functions and UI wiring done; Turkish acceptance waits for the `tr` catalog; engineering values stay `.`); then the Preferences language picker, the area migrations (#2057–#2062), pseudolocale layout and font coverage (#2064) and the first translation, `tr-TR`, with the translator workflow (#2065) |

**Not in this plan:** right-to-left layout (a later phase of #2053), translated user guide
and sample projects, user content.

### Application Core (architecture, no tier)

One iced-free application core shared by every front end: the GUI and
`signex-cli` run the same command through the same core, and the CLI stops
building and booting the GUI. Epic #2326; the design is
[ADR-0006](architecture/adr-0006-application-core.md). The slices ship one
PR at a time across releases, each a valid stopping point; this plan carries
no release of its own.

| Plan | Scope |
|------|-------|
| **Application Core** | The #2325 CLI stack fix; the `signex-i18n-ids` split and the `signex-commands` crate; the command catalog in `signex-commands`; the safety net (the parity table, the test adapter and the safety-net tests), with the core's event and effect types added by the slice that moves each behaviour; the event seam and projector; the `load_gateway` split; the state partition; persistence and git IO as effects run by a host; command handlers in the core; queries; lifting the core into `signex-workspace` so `signex-cli` drops `signex-app` (ADR-0006, "Slices") |

**Not in this plan:** a web front end and the wasm32 gate (epic D10: not
now), the plugin host (Plugin System).

### 3D Viewer (P3)

The in-canvas realistic 3D view mode — Tier 1 of the two-tier 3D stack.
Keyboard `2` / `3` toggles the PCB canvas between flat-layered 2D and
orbitable 3D in place, no separate window. See `PCB_3D_RENDER_PLAN.md`.

| Plan | Scope |
|------|-------|
| **3D Viewer** | `signex-scene3d` Scene IR crate (camera, materials, lights, units); `signex-board3d` board mesh + CSG cutouts + drill / via holes; board surface texture bake (albedo / normal / roughness / metallic); `BoardStackMaterials` + preset library; flat-2D view consumes the baked textures |
| **Realistic 3D View Mode** | `Realistic3D` view mode + `2` / `3` toggle; orbit camera rig (yaw / pitch / distance, middle-drag orbit, wheel zoom); `signex-render-wgpu-3d` PBR-lite pipeline; board rendered with baked textures, components as extruded courtyard blocks |
| **3D Selection & Cross-Probe** | Ray-cast hit-test with parity to 2D selection; cross-probe from schematic orbits the 3D camera to the selected component; layer visibility honoured in 3D |
| **On-Canvas 3D Toolbar** | Floating toolbar in 3D mode; camera presets (Top, Iso, Front, Side, Back, Reset); board flip and component explode slider; HDRI preset cycle, screenshot, send-to-render-manager |
| **STEP → glTF Cache Pipeline** | `signex-3d-models` crate (STEP import, tessellation, glTF write); hashed on-disk model cache; family-heuristic material fallback; fallback extrusion for components with no model |
| **Material Sidecars & Controls** | `.snxmat` parser (face-attribute matching → PBR presets); `.snxmat` applied during STEP → glTF conversion; Properties-panel Board context (mask colour, silkscreen, surface finish); PBR preset library v1 (~30 presets) |
| **Curated Footprint Library** | Curated `.glb` models for the top 50 stock-library footprints (IPC-7351 standard package set); CI per-footprint golden-image render gate; missing-model on-demand download flow |
| **Parametric Generators: R / C / L** | Family generators for R_* (0201 … 2512, cement) and C_* / L_*; BOM-driven value / tolerance label bake; CI regenerates the full family glTF set on schema change |
| **Parametric Generators: IC Packages** | Family generators for SOIC / TSSOP / SSOP / QFP / QFN and BGA / LGA / DIP; pin-1 dot decals and part-number labels from glTF metadata |
| **Board-Scope Material Overrides** | Project-scoped `.snxmat` for custom silkscreen art / logos; per-board texture channel overrides; auto-bake vs. user-authored toggle per board |
| **Advanced Board Controls** | Mask misregistration; via tenting modes (Tented / Plugged / Open) with geometry response; edge bevel geometry; wear / weathering overlay |

### Advanced PCB (P3)

| Plan | Scope |
|------|-------|
| **Advanced PCB** | Full layer stack editor (εr, tan δ, copper weight, material library); stackup templates (2 / 4 / 6 / 8 layer, HDI, MCPCB); DRC rule profiles (fab-house and IPC presets, save / load); formal ECO dialog with change review |
| **Partial & Embedded Heavy Copper** | Copper weight per region, not only per layer: heavy-copper areas (power, high current, heat spreading) beside standard-weight fine-pitch routing on the same layer, and heavy-copper inlays or bars embedded in the stackup. Region copper weight in the stack editor and the `.snxpcb` format; DRC clearance, width and spacing rules that follow the local copper weight (fab-house profiles); current capacity and temperature rise per trace and region; region thickness in the 3D view; fab outputs that carry the regions (separate region layer, stackup report and fab notes). Needs the Advanced PCB stack editor |
| **Impedance & DRC Rules** | Impedance profile per layer pair + built-in calculator; impedance-controlled routing (Z0 display, width-from-impedance); additional DRC rules (silk-silk, acute angle, component clearance, height) |
| **Routing Polish & Geometry** | Route completion (loop removal) and glossing; split planes (negative plane layers); via-in-pad, paste / mask expansion rules; per-layer keepout, board cutouts, castellated holes |
| **Constraint Manager & HUD** | Constraint Manager (schematic-side spreadsheet rule editor); query-based rule scoping (InNet, InNetClass, OnLayer, boolean); Board Insight HUD (clearance, net name under cursor). **Pulled forward by #2398:** the Rules manager (a table rule editor over the rule text, with net, class and pattern scopes in one Scope cell, "Why this value?", shadow and conflict flags, and Required / Actual / Margin from the last DRC) ships ahead of this band; the same issue later brings the net-class editor of PCB Routing, the DRC rule profiles of Advanced PCB, and rule rows as Requirements & Verification checks |

### High-Speed Design (P3)

| Plan | Scope |
|------|-------|
| **High-Speed Design** | xSignals (pad-to-pad through-component path analysis); length-matching group management UI + bar-chart dashboard; topology constraints (star, chain, fly-by, T) |
| **DDR SI & PDN** | Eye diagram generation; DDR timing analysis (setup / hold vs. spec); channel simulation (Tx IBIS → S-parameter cascade → Rx IBIS); PDN impedance analysis Z(f); return-path and power-plane analysers |

### HQ Render / Blender Export (P3)

Offline path-traced renders via a Blender subprocess — Tier 2 of the 3D
stack. Tier 1 (the 3D Viewer band) is preview; Tier 2 is export: product-shot stills,
marketing images, assembly renders. **Blender is detected, never
bundled.** See `PCB_3D_RENDER_PLAN.md` §4.

| Plan | Scope |
|------|-------|
| **Render Manager & Scene Export** | Render Manager dock panel (Appearance / Environment / Output / Queue); `signex-render-blender` crate — scene-bundle export (glTF + materials + board textures + render script); Blender auto-detect with a Preferences override |
| **Blender Addon & Single-Frame Render** | Signex Blender addon, auto-installed on first render; script generation (import glTF, wire HDRI, camera, lights, samples); single-frame render to PNG / JPG / EXR; "Install Blender" helper on macOS / Windows |
| **HDRI Presets & Lighting Rigs** | HDRI library (Studio / Workbench / Daylight / Pure White); lighting rig presets (key / fill / rim, product-shot, flat); sample count presets (Draft / Good / Final) |
| **Render Queue** | Multiple queued jobs with per-job progress and cancel; queue survives app restart; open-output-folder action and per-job log viewer |
| **Preview-Parity CI Gate** | CI renders a reference scene through Tier 1 and Tier 2 and diffs them; regression fails CI on material drift between preview and render; golden-image corpus covering common component families |

### Animations (P3)

Time-based renders: turntable demos, marketing videos, assembly
animations. Builds on the HQ Render / Blender Export pipeline — same scene, same
materials, temporal camera / state.

| Plan | Scope |
|------|-------|
| **Turntable Animation** | Turntable camera path (360° yaw around target, configurable duration); MP4 / WebM export via Blender's ffmpeg encoder; background render thread so editing continues while a render runs |
| **Camera Path Editor** | Bezier camera tracks with keyframe editing in the Render Manager; timeline scrubber with per-keyframe camera preview in Tier 1; per-segment easing curves |
| **Assembly Animation** | Components drop in along BOM order with per-part easing; per-component delay / duration overrides; solder-reflow / appear-to-sit animation mode |
| **Simulation Overlay Bake-In** | Render-time thermal simulation heatmap overlay; render-time EM / signal-integrity overlay; per-frame interpolation for time-varying simulation data |

**HQ Render / Blender Export and Animations are P3 and are not gates.** They are published because
they are planned and scoped, not because they are committed. They carry
no exit criteria, they gate nothing, and of every band on this page they
are the most likely to move — including below the sketch line. Their only hard
dependency is the 3D Viewer band.

### MCAD (P3)

Mechanical design inside Signex, so the board and the part it goes into are
designed, checked and changed in one place. Solid
modelling uses an existing B-rep kernel crate, wrapped by a Signex crate. The
kernel must be Apache-2.0 or MIT, pure Rust, wasm-compatible and an exact B-rep.
`solidcore` is the current choice against these criteria; it is re-checked when
MCAD Kernel starts, since the crate is still being ported. The kernel is a
normal dependency, credited in the third-party licence list, with its NOTICE
file carried in the distribution as Apache-2.0 §4(d) requires. Signex's own
sketch solver (`signex-sketch`) drives every sketch, so the sketch mode users
already know is the one they model with. Every operation is a command with an
inverse (one undo step), a typed agent command with a JSON result, and reachable
from menus, the Active Bar, context menus, keys and the palette. Board and
enclosure stay linked: a board change (outline, mounting hole, connector move)
is offered as a reviewable change to the mechanical design, never applied
silently.

`signex-mcad` does no IO and builds for `wasm32`, because a web front end is
possible under the shared application core (#2326, ADR-0001 A4). STEP and the
native mechanical document are read from and written to bytes (`&[u8]` in,
`Vec<u8>` out); opening and saving files is the host's job (native, browser or
test), reached through an `Effect`, never through `std::fs` inside the crate.
The crate uses no `std::fs`, `std::net`, threads or tokio; long kernel work runs
as a compute job the host schedules. The kernel dependency must also build for
`wasm32`. `signex-mcad` joins the `DOMAIN` set in `scripts/check_arch_guard.sh`
and the `wasm32` CI build when it is created.

| Plan | Scope |
|------|-------|
| **MCAD Kernel** | `signex-mcad` crate wrapping the kernel: bodies, faces, edges, persistent face/edge names; a STEP AP214/AP242 B-rep translator (read and write) written in Signex on the kernel's topology, since the kernel provides no STEP support; tessellation for the 3D view; a native mechanical document type in the `.snx*` family; kernel calls guarded so a failed operation is a message, never a crash |
| **Enclosure Fit Check** | Insert an enclosure or mechanical STEP next to the board in 3D; DRC rules for component-to-enclosure clearance, collisions, component height under a lid, keepouts and mounting-hole-to-standoff alignment; results in the DRC panel with cross-probe; a `mcad.fit_check` agent command |
| **Part Modelling** | Sketch on a plane or a face, then extrude, revolve, cut, fillet, chamfer, shell, draft, holes, patterns and mirror; named parameters and an editable feature history that rebuilds; measure and section views |
| **Board-Driven Enclosure** | An enclosure started from the board: outline offset, wall and lid, standoffs placed on the mounting holes, port cut-outs from connector bodies, vents; parameters follow board changes through a reviewable change list |
| **Assemblies & Interference** | Components and occurrences, joints with limits, ground, interference between parts shown on the bodies, exploded views; the board is one component of the assembly |
| **Sheet Metal & Plastic Parts** | Flanges, hems, bends, flat pattern with DXF export; bosses, ribs, lip/groove and snap fits for moulded enclosures |

**MCAD is P3 and is not a gate.** It is published because it is planned and
scoped. Work on it starts once the schematic and PCB editors are complete
and the release that ships them, with its demo videos and website, is out.
The goal is one tool for
every electronic and mechanical design. Its hard dependencies are the 3D Viewer
band and the STEP 3D export in Assembly & 3D Export. The STEP translator of
MCAD Kernel is pulled forward into the Community Release (2026-11-17):
Assembly & 3D Export writes the board's STEP with it, so it is built before
the rest of MCAD Kernel, which stays P3. Enclosure Fit Check needs
no modelling and comes first; the other plans follow in table order.

---

> **Everything below this line is a sketch, not a plan.** §11 applies with
> full force: the order and timing of the plans below are speculative and
> user feedback will reshape them. These plans are enumerated so that
> contributors can see what is already scoped and avoid duplicating it —
> **not** because they are committed. Sub-release boundaries here are
> working guesses; expect them to merge, split, and reorder. Apart from
> the Simulation gate, no plan below gates anything; that gate's exit
> criteria are written when its band is planned.

### Plugin System (Community)

Extism WASM runtime (`plugin-api` crate); five host function categories
(Document, Mutation, UI, Query, Sim); permission gateway (plugin manifest
+ user approval); undo-stack integration for plugin mutations;
hot-loading without restart.

**Not decomposed into sub-releases.** Unlike every other band on this
page, Plugin System has no sub-release breakdown yet — it is a single body of work
that has not been staged. It gets sub-releases when it gets a staging
plan, per §12.

### Simulation (gate) — Simulation View + SPICE

Simulation uses a **dedicated Simulation View** — a block-diagram
composition workspace separate from the schematic editor. See
`SIMULATION_VIEW.md`.

| Plan | Scope |
|------|-------|
| **Simulation** | Simulation view tab (third editor mode); stimulus block palette (V_DC, V_Pulse, V_Sine, V_PWL, I_DC, I_Pulse); load blocks (R_Load, RC_Load, RLC_Load) + GND reference; Sheet Block (import a schematic sheet as a sim block with auto-detected ports); port auto-detection from hierarchical labels and power pins; Manhattan wire routing between ports; analysis directive editor (DC op, DC sweep, AC, Transient); `.snxsim` file format; ngspice FFI (`spice-gen` crate) |
| **Waveform & Probes** | Netlist generation from the block diagram; subcircuit extraction from schematic sheets; voltage / current probe placement; run → ngspice → results; waveform panel (multi-trace, dual cursor, PNG / CSV export); `.raw` parser; DC, AC, Transient, Noise, Fourier analyses; probe ↔ trace cross-probe |

### Advanced Simulation

| Plan | Scope |
|------|-------|
| **PCB Geometry Blocks & Vendor Models** | PCB Trace Block (select net → auto-extract S-params via OpenEMS); PCB Via Block; PCB Region Block (area → multi-port network); IBIS Block (`.ibs` import, pin / model selection); S-Parameter Block (`.s2p` / `.s4p`) and Package Block; PRBS generator, clock source, eye diagram probe, TDR probe |
| **Advanced SPICE & EM** | Parameterized blocks (override values without editing the schematic); parameter sweep with multi-trace overlay; Monte Carlo and temperature sweep; corner analysis for passive tolerances; OpenEMS bridge (CSX writer, FDTD runner, HDF5 reader); S-parameter extraction, Smith chart, TDR; sim job queue with progress and cancel |
| **Thermal & Simulation Wizards** | Elmer FEM bridge (GMSH mesh, `.sif` writer, VTK reader); steady-state thermal analysis with component heat sources; DC IR drop (voltage distribution, current density); 3D thermal overlay on the PCB model; wizards for DDR SI, power supply, thermal, PDN impedance, and RF / antenna |

### Advanced Output

| Plan | Scope |
|------|-------|
| **Panelization** | Panelization (step-and-repeat, rails, tabs, mouse bites); V-cut scoring / tab routing; auto-fiducials, tooling holes, test coupons |
| **Fab Docs & Export** | Drill table and board stackup report (PDF); DXF import / export; one-click validated manufacturing package; Gerber X3 |

### Import + Git

Each foreign-format importer is a **separate companion repo / binary**,
licensed appropriately for its source-format constraints and distributed
independently of the main workspace — the same architectural pattern as
the [import companion repository](https://github.com/alplabai/signex-kicad-import) shipped at v0.9.0. One-way conversion to native
`.snx*`, no compile-time dependency from the main repo.

| Plan | Scope |
|------|-------|
| **Altium & Eagle Import** | `signex-altium-import` companion (`.SchDoc`, `.PcbDoc`, `.SchLib`, `.PcbLib`, `.PrjPcb` → native, one-way); `signex-eagle-import` companion (`.sch`, `.brd` → native, one-way); creepage / clearance measurement + DRC (IEC 60950), in the main repo |
| **Built-in Git** | Built-in Git (branch, commit, merge, visual diff, blame); visual schematic diff as a canvas overlay; visual PCB diff as a canvas overlay |

### Polish

| Plan | Scope |
|------|-------|
| **Auto-Router & Advanced Routing** | Auto-router (topological) and semi-automatic ActiveRoute; copper balancing / thieving, backdrilling; thermal-aware routing, placement heatmap |
| **Advanced Geometry & Import** | Rigid-flex board, embedded components (cavity); OrCAD / PADS / Mentor import companions (separate repos, one-way); 3D clearance checking (body-to-body); antenna simulation (pattern, gain); interactive on-canvas impedance calculator |

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
- **Non-monotonic phase versions removed.** Phases previously ran
  v1.1 → v1.2 → **v2.1** → **v1.4** → v1.5. Phases no longer carry
  version numbers; they are narrative, and the version axis is §7–§8.
- **PCB viewer renumbered** v1.1 → v2.0, and "v2.0 = complete EDA tool"
  retired in favour of v2.2 = Community Release.
- **Ghost versions acknowledged.** v0.15–v0.26 existed only as work
  labels on commits and internal plans, never as releases. The train
  restarts from the real anchor (v0.14) in §7.
- **v4.0 promoted to a gate.** Previously only half-covered by a
  "Beyond v3.0" table; it has a dedicated internal plan
  (`SIMULATION_VIEW.md`) and milestones.

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

### 2026-11-17 — Community goal decisions

Maintainer decisions (Caner), each also a row in `docs/DECISIONS.md`:

- **Pulled forward:** all of Schematic Refinement (Advanced Schematic,
  SCH Tables & Docs, Enhanced Output, Design Notebook and Block Diagram)
  now ships before the Community Release; it was deferred until PCB Design
  shipped, with no order against the Community Release. The tier stays P1.
  Each band gets draft exit criteria (§8). Only the Community scope is
  pulled in; the Signex AI part of Measurements & Signex AI Integration is
  Pro.
- **Scope added to the Community Release plan:** PCB rooms from schematic
  sheets (#2457), the Command Log panel (#2458), PCB status-bar counters
  (#2460), compare board with schematic (#2462) and agent-socket coverage
  of the existing catalog commands (#2470). Nothing moved out of the plan;
  the maintainer chose to grow it.
- **Pulled forward from MCAD (P3):** the STEP translator of MCAD Kernel, so
  the Community Release writes the board's STEP with Signex's own
  translator.
- **Routing test corpus:** boards under permissive licences only, with
  per-file source and licence, outside `crates/`.
- **Schematic Refinement rules:** format versioning lands first (#2207);
  Signex names in the plan text; the Object List, the compiled design view
  and the Parameter Manager are dock panels; Typst is allowed for rich
  notes.
- **New milestone:** `Requirements & Verification` (#2339).
- **Moved out of this repository:** #115 (library-server hardening) to the
  Pro repository. #1707 (library packages from a git URL) stays Community.

### 2026-11-17 — Pro roadmap moved to the private Pro repository

Maintainer decision (Caner): Signex Pro is a separate, private product, so
its planning moved out of this file into the private Pro repository. This
file now covers Community only, with a one-paragraph summary of Pro in §3,
"Signex Pro". What changed:

- **Moved out:** the Signex AI parts of the former AI Design gate, the Pro
  Release, Signex AI Core, Signex AI Tools, Live Collaboration, the PLM
  bands (the former v5.0 gate and the bands after it), the paid editions,
  two workstreams, the 2026-11-08 commercial-model
  entry and the AI parts of other log entries.
- **Renamed:** the Community part of the former AI Design gate is the **PCB
  Design** gate (PCB Layout, PCB Routing, Router: Walkaround and
  Requirements & Verification); its exit criterion is the board one it
  already had. Its milestone keeps the title `AI Design`, because existing
  issues use it.
- **Tiers:** P2 ships from PCB Routing through the Community Release, P3
  after it, since the Pro Release no longer bounds them here.
- No Community scope was moved, added or removed. The milestones of moved
  plans stay in `.github/milestones.yml` with a neutral description, so
  their issues keep them.

### 2026-11-16 — MCAD and scripted demo videos

Maintainer request (Caner): Signex should have full MCAD capabilities, so
electronics and mechanics are designed in one place. Added as a new band, **MCAD (P3)**, with workstream WS-MC. The
maintainer chose to build on an existing permissive (Apache-2.0 / MIT) Rust
B-rep kernel crate rather than write a kernel; features are written in Signex,
from published standards and Signex's own design, not copied from another
application. Also added **Scripted Demo Videos** as its own plan in Community
Release: every feature gets a scripted, re-recordable video for the README,
the docs and the website. No scope was moved out of another plan. On
2026-11-17 the maintainer set MCAD's start: once the schematic and PCB editors
are complete and their release is out. The maintainer (Hakan) also
required that `signex-mcad` does no IO and builds for `wasm32`: STEP and
the native document go through bytes, file access goes through the host's
`Effect`, and long kernel work is a host-scheduled compute job.

### 2026-11-13 — Partial and embedded heavy copper

Maintainer request (Caner): some fabs offer partial or embedded heavy
copper in the stackup, and Signex should support designing for it. Added
as its own plan, **Partial & Embedded Heavy Copper**, under Advanced PCB
(P3), after the stack editor it builds on. No scope was moved out of
another plan.

### 2026-11-07 — PCB design moves first

Maintainer decision (Caner): board design from the schematic is the next
priority after the Community Preview; the other bands wait. The PCB Viewer
gate was folded into the PCB Layout plan, which adds board creation from
the schematic (#1312), board outline and placement, because a viewer had no
board to show. **Pulled forward:** the Command Registry (#278, from Advanced
Schematic), PCB Routing and Router: Walkaround. **Pushed back:** Schematic
Refinement and the rest of the PCB build-out. The full entry, which also
covers Pro scope, is in the private Pro roadmap (2026-11-17 entry).

### 2026-11-04 — SemVer to CalVer

Maintainers (Hakan, Caner) moved Signex from semantic versioning to
calendar versioning, `YY.MM.MICRO` (§3). What that changed:

- **The version axis became a name axis.** §7 and §8 identify plans by
  name (Community Preview, Advanced Schematic, PCB Viewer, …). Order,
  tiers, exit criteria and scope are unchanged; only the labels moved.
  The Community gates are the former v1.0, v2.0, v2.2 and v4.0.
- **`v1.0.0-rc.1` (2026-10-29) is the last SemVer tag**; `1.0.0` final is
  never cut. The next release is CalVer, dated per §3.
- **GitHub milestones were renamed** to the plan names via `aliases` in
  `.github/milestones.yml`, so issues kept their milestone. Milestones of
  shipped versions keep their version titles.
- **`workspace.package.version` was not bumped** (still `1.0.0-rc.1`); the
  first CalVer release-prep PR sets it.
- The full old version → plan name map is the `aliases` field of
  `.github/milestones.yml`.
- Everything in §6 and in this log keeps its original version numbers as
  history.

---

## 10. Staffing Assumptions

Planned for a **small team**: 2–3 engineers up to the Community Preview, scaling to
3–5 from PCB Design onward.

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
- **Feature ordering within a plan.** May shift on dependencies and
  availability.
- **Plans not yet enumerated and everything below the sketch line in §8.**
  Order and timing are speculative; user feedback will reshape them. §8
  enumerates the sub-releases of those bands so that scoped work is
  visible to contributors. **Enumeration is not commitment.** A plan that
  has a heading and a milestone is still a sketch if it sits below that
  line.
- **Sub-release boundaries within a band.** Which sub-release a feature
  lands in may change without amending this document; which *band* it
  lands in may not.
- **Anything about Signex Pro.** Pro is planned separately (§3, "Signex
  Pro").
- **Performance numbers.** The targets here are exit criteria, not
  user-facing promises. Real-world performance varies with hardware and
  design complexity.

---

## 12. Rules for Changing This Document

- **Add the plan here, by name, before using it anywhere.** Not in a commit
  message, not in a milestone, not in an internal plan. Here first.
  Never put a future version number on a plan.
- **Gate order does not change** without an architectural reason. "We
  changed our minds" is not sufficient.
- **Adding scope to a plan requires removing equivalent scope from the
  same plan.** The plan does not silently grow.
- **Pulling a feature forward** requires explicit justification and an
  updated tier classification. **Pushing a feature back** is always
  allowed and needs no justification beyond "we underestimated."
- **Milestones follow this file, never lead it.** `.github/milestones.yml`
  is a projection; edit the roadmap first.
- **Quarterly review**, and reconcile against `CHANGELOG.md` + `git tag`
  every time. If this document and the tags disagree, the tags are right
  and this document is broken — fix it the same day and add a line to §9.

### Milestone convention

- **Title = plan name only:** Title Case, unique, no version number, no date.
  Sub-plans are named by their own content; prefix "<Parent>: " only when
  the name alone would be ambiguous (e.g. "Router: Walkaround").
- **Roadmap first:** a new milestone is added here (row or heading with the
  exact same name), then to `.github/milestones.yml` in the same PR.
- **Description:** one user-facing line of what it delivers; a gate's starts
  with "Gate:" (applies to new milestones).
- **Rename** = change the title and append the old title to `aliases`; never
  edit on GitHub by hand.
- **No due dates:** milestones are scope, not schedule; the CalVer version
  records when it shipped.
- **Shipping:** when its last issue closes and the work is in a release, set
  `state: closed` and end the description with "Shipped in YY.MM.MICRO." The
  title keeps the plan name; milestones closed before 2026-11 keep their
  version titles as history.
- **Pro plans:** a milestone for a plan planned in the private Pro roadmap
  stays in `.github/milestones.yml` so its issues keep it, with a neutral
  one-line description and no Pro detail. New Pro plans get no public
  milestone. `AI Design` is the milestone of the PCB Design gate (§8).
- **Plans of a gate** have their own milestones when they ship on their own:
  `Requirements & Verification` (#2339) is a plan of the PCB Design gate
  with its own milestone, since it can ship ahead of the gate.
