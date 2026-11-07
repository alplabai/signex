# Signex — Roadmap

> **Status:** Living document. Updated quarterly. Last reconciled against
> reality 2026-11-07 (AI Design moved first, §9).
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

| Fact | State (2026-11-07) |
|------|--------------------|
| Latest tagged release | **v1.0.0-rc.1** (2026-10-29), the first release candidate of the Community Preview and the **last SemVer tag**. It published as a GitHub pre-release (`release.yml` marks any tag with a `-` suffix as one), so `releases/latest` still points at v0.15.0 (2026-07-23) until the first stable CalVer release |
| Next tag | **v26.11.0-rc.1**, the first CalVer tag and the second Community Preview release candidate, cut from this release-prep PR. The final `v26.11.0` follows a tester's pass of the user guide on a fresh install (#322). `1.0.0` final is never cut |
| `workspace.package.version` | **26.11.0-rc.1** |
| Latest CHANGELOG section | **26.11.0-rc.1** — dated 2026-11-07; the summary is the release page, the full list sits under "Detailed changes in 26.11.0-rc.1" |
| Current work | Command Registry & Symbol Units milestone: 0 open issues; it shipped in 1.0.0-rc.1, and its test follow-up #1727 landed after the tag and ships in the next release. Community Preview milestone: open issues are tracked on [the milestone](https://github.com/alplabai/signex/milestone/12) (a dated issue list here goes stale). The first CalVer release candidate, v26.11.0-rc.1, is being cut |

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
Community Preview, AI Design, Community Release, Pro Release, Simulation
and Signex 365 (§8). The stages, in order:

| Stage | Meaning |
|-------|---------|
| Pre-1.0 (shipped, §6) | Foundation and editor build-out. No public users. |
| Community Preview | First public release. Community schematic editor is real. |
| AI Design | Schematic and PCB design with Signal AI, as an assistant and as a generator: PCB layout from the schematic, routing and the AI tool layer, built together. |
| Community Release | Manufacturing output for the boards AI Design produces. |
| Schematic Refinement | Schematic-editor refinement. Deferred until AI Design ships. |
| PCB build-out | 3D, advanced PCB, high-speed, the rest of the router. |
| Pro Release | Pro launch: licensing and billing, plugins, live collaboration. |
| Pro refinement | Plugin system, live collaboration, the rest of Signal AI. |
| Simulation band | Simulation, advanced output, foreign-format import. |
| Signex 365 | Cloud PLM platform. |

**Version numbers are not work labels, and not plan labels.** A version
number means "this was tagged and released". It does not mean "the branch
I am on", and it does not name a future release: which release carries a
feature is known only when it ships. Plans are identified by **name**
(Community Preview, AI Design, PCB Layout, …). This rule exists
because ~100 commits shipped carrying labels v0.13 through v0.26-G, of
which exactly one (v0.13.0) was ever released — which is how this document
and the milestone set drifted apart in the first place. Use issue numbers
and milestones to track in-flight work.

---

## 4. Priority Tiers

Every feature falls into one of four tiers. Tiers determine when a
feature *can* ship, not whether it ships at all.

| Tier   | Meaning                                              | Ships in       | Label |
|:------:|------------------------------------------------------|----------------|-------|
| **P0** | Required for the Community Preview. Cannot ship Community without it. | Up to and including Community Preview | `tier: p0` |
| **P1** | Required for AI Design, or a professional schematic feature users expect. | AI Design, then Schematic Refinement | `tier: p1` |
| **P2** | Differentiator. Drives Pro adoption.                  | PCB Routing through Pro Release | `tier: p2` |
| **P3** | Nice-to-have. Stable core required first.             | After the Pro Release (Signal AI Core onward) | `tier: p3` |

**AI Design first (2026-11-07):** the plans of the AI Design gate (§8) are
P1 and are scheduled first. Schematic Refinement issues keep their
`tier: p1` label but wait until AI Design ships; they are not re-triaged.

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

## 8. Gates and Planned Releases — Community Preview → PLM Advanced

Strategic. Undated by policy. A gate ships when its exit criteria pass.

**Not every plan here is a gate.** The gates are Community Preview, AI
Design, Community Release, Pro Release, Simulation, and Signex 365, in that
order — they block what follows. (AI Design took the place of the PCB
Viewer, the former v2.0, on 2026-11-07; the others are the former v1.0,
v2.2, v3.0, v4.0 and v5.0.) The first four carry exit criteria below; the exit criteria of
the Simulation and Signex 365 gates are written when their band is planned. Everything else is a planned
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
  #271, #278, #115).

A release candidate is a tag of the form `vYY.MM.MICRO-rc.N`; `release.yml`
builds the same packages as a final release and publishes them as a
pre-release. (`v1.0.0-rc.1`, the last SemVer tag, was the first candidate.)

### AI Design (gate) — schematic and PCB with Signal AI

Maintainer decision, 2026-11-07: designing schematics and boards with AI
comes first after the Community Preview. Work starts now, alongside the
last Community Preview issues, behind the `pro` feature so none of it is in
the Community Preview build; it ships in the first release after the
Community Preview. The plans below are built in parallel. Every other band on this page waits until this gate ships:
Schematic Refinement, the 3D, Advanced PCB, High-Speed, HQ Render and
Animation bands, the Plugin System, Live Collaboration, Simulation and
Signex 365. The PCB Viewer gate is folded into the PCB Layout plan here: a
viewer for boards that nothing in Signex can create is not a milestone
worth stopping at.

**Two modes, one tool layer.** Signal AI works as an assistant or as a
generator. Both call the same undoable commands the menus and keys call
(the Command Registry, #278, which moves here from Advanced Schematic),
both act under the user's identity, and neither writes to disk without the
user's save.

- **Assistant:** the user drives. Chat with the open design as context;
  explanations, design reviews, and ERC / DRC fix suggestions. The AI edits
  only when asked, and every edit is shown as a preview and applied as one
  undoable step.
- **Generator:** the AI drives from a written specification (function,
  interfaces, power, size, cost, preferred parts), in stages. Each stage
  stops for the user's approval as a change list they can review entry by
  entry: (1) architecture, a block diagram and a power tree; (2) parts,
  from the user's libraries first and then distributors, as a BOM with
  alternates; (3) schematic, hierarchical, annotated and ERC-clean; (4)
  board, created from the schematic, with outline, placement, routing and a
  DRC-clean result. The user can stop at any stage and go on in Assistant
  mode or by hand.

| Plan | Scope |
|------|-------|
| **PCB Layout** | One part identity across library, schematic and board (#663); a new `.snxpcb` in a project; board outline; Update PCB from schematic with a reviewable change list (#1312); component placement (move, rotate, flip); ratsnest; cross-probe in both directions; PCB rendering via `iced::widget::Shader` with 32 copper and the technical layers, and the layer panel (the former PCB Viewer scope) |
| **PCB Routing** | As in the PCB Routing band below: single-trace routing, vias, net classes, live DRC with the 15 base rules |
| **Router: Walkaround** | As below: obstacle-aware A* routing, which the Generator's board stage routes with |
| **Chat & Context** | Signal panel (streaming chat, markdown rendering); design context (parts, nets, ERC / DRC results); local session history; the Alp Lab API gateway client and the `#[cfg(feature = "pro")]` gate |
| **Edit & Analysis Tools** | Assistant mode: undoable tools over the Command Registry for schematic and board (place a part, wire, set a value, delete, place a footprint, route a net); check_erc and check_drc; fix suggestions applied as reviewable changes |
| **Generator: Schematic** | Generator stages 1–3: specification to architecture, parts and an ERC-clean schematic, each stage a reviewable change list |
| **Generator: Board** | Generator stage 4: the board created from the schematic, outline, placement, routing with the Walkaround router, DRC-clean |

**Exit criteria:**

- A schematic drawn in Signex becomes a board: every footprint placed from
  the schematic and every net shown as ratsnest; five real `.snxpcb`
  fixtures render correctly; a 10,000-track board pans and zooms at 60 fps;
  cross-probe works in both directions.
- Assistant: a user holds a productive design conversation about an open
  schematic and board, and every change the AI makes is undoable.
- Generator: from a written specification for a small board (an MCU, a
  USB-C supply and a few sensors, say), Signex produces an ERC-clean
  schematic and a placed, routed, DRC-clean board, with the user approving
  each stage.
- No AI change reaches disk without the user's save.

Signal AI ships in the Pro build. Licensing and billing stay in the Pro
Release; until it ships, AI Design builds go to internal and invited
testers.

### Schematic Refinement

**Deferred until AI Design ships (2026-11-07).** The Command Registry
(#278) moved from Advanced Schematic to AI Design.

Sub-versions are enumerated because they are staged releases, not
milestones-of-convenience: each one ships on its own. Order within a band
is firmer than the boundaries — scope may move between adjacent
sub-versions of the same band without a roadmap change, but may not move
between bands.

#### Advanced Schematic (P1)

| Plan | Scope |
|------|-------|
| **Advanced Schematic** | Design variants (fitted / not-fitted / alternate) with 7 drawing styles; multi-channel design (Repeat keyword, channel naming) including per-channel variant state; signal harnesses (connectors, entries, nested); Parameter Manager; net classes + diff-pair classes; smart paste (rubber stamp, paste array); change component; Command Registry (#278: dispatch, command palette, args, CLI, plugin hooks) |

#### SCH Tables & Docs (P1)

| Plan | Scope |
|------|-------|
| **SCH Tables & Docs** | Schematic tables (pin assignment, register map, connector pinout); auto-generated Table of Contents for multi-sheet designs; drawing tools (bezier, dimension annotation); named unions (hierarchical groups with tags) |

#### Enhanced Output (P1)

| Plan | Scope |
|------|-------|
| **Enhanced Output** | Smart PDF with bookmarks + PDF layers; variant-specific BOM export; watermarking (DRAFT / CONFIDENTIAL overlay); Output Job file; workspace layout save/restore |

#### Design Notebook (P1)

A first-class document type alongside Schematic and PCB: design
rationale, calculations, measurement data, and debug logs attached to the
design rather than to a separate file. The notebook is a Typst document
with Signex extensions for component / pin / net references.

| Plan | Scope |
|------|-------|
| **Design Notebook** | Notebook tab (fourth document mode); `signex-notebook` crate — Typst source model, compile pipeline, annotation types; split-pane source + live preview editor; syntax highlighting and autocomplete; notebook file format (Typst source + metadata sidecar); PDF export |
| **Design References & Annotations** | Custom Typst functions `@component` / `@pin` / `@net`; reference resolution against the active schematic with stale warnings; bidirectional navigation notebook ↔ schematic; schematic badge overlay with hover preview; annotation types (Note, Calculation, Measurement, Issue, Decision); dockable Annotations panel |
| **Measurements & Signal AI Integration** | `#measurement()` function (value + unit + conditions + timestamp); per-pin / per-component measurement timeline; attachments (scope screenshots, thermal images, simulation plots); stale detection when referenced values change; Signal AI reads and writes notebook context (Pro) |
| **Computational Layer** | `#python()` inline calculation cells; embedded CPython (`pyo3`) with pruned NumPy / SciPy / Matplotlib / pint / python-control / scikit-rf; schematic-value bridge (`@component(R7).value` → units-aware Python variable); inline Matplotlib plot rendering; result caching; sandboxed execution (no subprocess / network / out-of-project FS); slim installer variant without the Python embed |

Python is the only notebook runtime. Octave / MATLAB paste-and-run is
explicitly deferred; `python-control` + `scikit-rf` are deliberate
MATLAB-API clones covering the legacy use cases without a subprocess
bridge. The `ComputeRuntime` trait is pluggable if users surface demand.

#### Block Diagram (P1)

System-level architectural view supporting top-down design: start with
functional blocks and signal flows, then refine into detailed schematics.

| Plan | Scope |
|------|-------|
| **Block Diagram** | Block Diagram tab (fifth document mode); block canvas with customizable shapes / colours; block properties (function, rails, key specs, interfaces); signal-flow connections with protocol labels; interface annotations (SPI, I2C, UART, USB, LVDS, power, analog); block diagram file format; SVG / PDF export |
| **Schematic Linking & Power Tree** | Link blocks to schematic sheets (click block → navigate); auto-generate a block diagram from hierarchical sheet structure; power-tree visualisation auto-detected from the schematic; power budget annotations per block; signal-chain visualisation; block-level net summary; Signal AI block-diagram generation from a natural-language system description (Pro) |

### PCB Viewer

**Folded into AI Design (2026-11-07)** as the PCB Layout plan, with board
creation from the schematic and placement added. Its exit criteria are part
of the AI Design gate's.

### PCB Routing

**PCB Routing and Router: Walkaround are built in AI Design** (2026-11-07);
the plans after them in this band wait until it ships.

Professional-grade interactive routing, implemented clean-room under
Apache-2.0 with no reference to other EDA tools' source or format docs.
Ships as five staged sub-releases. DRC (15 base rules) and net-class
management land in this band.

| Plan | Scope |
|------|-------|
| **PCB Routing** | Router stage 1, greedy single-trace. `pcb-geom` crate (polygon offset, R-tree, Delaunay, Minkowski); `pcb-router` skeleton (session model, preview / commit); single-trace routing with 45° / 90° / arc45 / arc90 corners; via placement (through / blind / buried); net-class editor (width, clearance, via size per class); live DRC feedback with the 15 base rules (`signex-drc`) |
| **Router: Walkaround** | Obstacle graph with clearance-inflated Minkowski offsets; A* pathfinder with corner / via / layer costs; corner-insertion pass preserving 45° / 90° / arc style; incremental debounced DRC on the proposed path; routing test-corpus tooling (50 open-source PCBs) |
| **Router: Push-and-Shove** | Topology-preserving shove solver with fixed-point iteration; rigidity heuristics (pad / via / edge proximity); via shove within a rigidity budget; preview buffer + commit barrier with Escape rollback; the whole shove cascade undone as one action |
| **Router: Diff Pairs & Length Tuning** | Coupled two-net router with gap control; length meander generator (accordion / trombone / sawtooth); length and skew reporting per net and per pair; teardrops as a design rule and a per-pad property |
| **Copper Pour** | Zone fill engine (polygon boolean via `pcb-geom`); thermal relief + island removal; fill priority and on-demand fill; back annotation / ECO |

Routing is the hardest single feature in EDA and is fundamentally one
engineer's deep work. See `PCB_ROUTER_PLAN.md` for the authoritative plan.

**Exit criteria:** 50-board fixture corpus routes end-to-end with no
panic, stuck state, or geometry corruption across 10,000 random routing
actions per board; shove converges in ≤8 iterations on 95% of actions;
median action latency ≤16 ms, 95th percentile ≤33 ms.

**Deferred:** multi-track routing, BGA fanout, via stitching → Community Release
candidates. Autorouting → not scoped.

### Community Release

Manufacturing output. AI Design and Community Release together are a complete schematic + PCB
editor: design, validate, route, DRC, and export to fabrication.

| Plan | Scope |
|------|-------|
| **Community Release** | Gerber RS-274X + X2 export; Excellon drill export; ODB++ export |
| **Assembly & 3D Export** | Pick-and-place CSV; IPC-2581 export; STEP 3D export (board body); assembly SVG |

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
| **Internationalisation** | `signex-i18n` crate (Fluent + ICU4X), `Localized` display-only type, runtime language switch with the locale persisted in preferences (#2055); API gate, key-consistency and locale-invariance tests (#2056); locale-aware formatting of numbers, dates, lists and case, engineering number input (#2063, library functions); then the Preferences language picker, the area migrations (#2057–#2062), pseudolocale layout and font coverage (#2064) and the first translation, `tr-TR`, with the translator workflow (#2065) |

**Not in this plan:** right-to-left layout, translated user guide
and sample projects, user content.

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
| **Impedance & DRC Rules** | Impedance profile per layer pair + built-in calculator; impedance-controlled routing (Z0 display, width-from-impedance); additional DRC rules (silk-silk, acute angle, component clearance, height) |
| **Routing Polish & Geometry** | Route completion (loop removal) and glossing; split planes (negative plane layers); via-in-pad, paste / mask expansion rules; per-layer keepout, board cutouts, castellated holes |
| **Constraint Manager & HUD** | Constraint Manager (schematic-side spreadsheet rule editor); query-based rule scoping (InNet, InNetClass, OnLayer, boolean); Board Insight HUD (clearance, net name under cursor) |

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
are the most likely to move — including past the Pro Release. Their only hard
dependency is the 3D Viewer band.

### Pro Release

Signal AI, plugin system, and live collaboration. Pro development happens
in feature-gated crates; the Community editor continues to ship.

| Plan | Scope |
|------|-------|
| **Pro Release** | Pro license validation with an offline cache; Community build hides the Signal panel and shows an upgrade prompt. (The API gateway client and the feature gate moved to AI Design, 2026-11-07.) |

**Exit criteria:** Signal AI as shipped in AI Design is available to Pro
users; three test users
can edit the same schematic and PCB simultaneously without conflicts;
pricing is set and subscription billing is operational; Pro and Community
binaries both build clean from a single workspace.

---

> **Everything below this line is a sketch, not a plan.** §11 applies with
> full force: the order and timing past the Pro Release are speculative and user
> feedback will reshape them. These plans are enumerated so that
> contributors can see what is already scoped and avoid duplicating it —
> **not** because they are committed. Sub-release boundaries here are
> working guesses; expect them to merge, split, and reorder. Nothing in
> Apart from the Simulation and Signex 365 gates, no plan below gates
> anything; those two gates' exit criteria are written when their band is
> planned.

### Signal AI Core (Pro)

Chat & Context and Edit & Analysis Tools moved to AI Design (2026-11-07);
what is left here comes after it.

| Plan | Scope |
|------|-------|
| **Visual & Polish** | Visual context (schematic / PCB screenshot to a vision model); usage meter; graceful offline degradation |

### Signal AI Tools (Pro)

| Plan | Scope |
|------|-------|
| **Sim & Templates** | Simulation tools (run_spice_sim, run_openems, run_thermal); notebook tools (read_notebook, write_notebook); design review mode (scored analysis, findings written to the notebook); circuit templates (buck, LDO, op-amp, filter, …); natural-language constraint entry and AI-guided routing suggestions |

### Plugin System (Pro)

Extism WASM runtime (`plugin-api` crate); five host function categories
(Document, Mutation, UI, Query, Sim); permission gateway (plugin manifest
+ user approval); undo-stack integration for plugin mutations;
hot-loading without restart.

**Not decomposed into sub-releases.** Unlike every other band on this
page, Plugin System has no sub-release breakdown yet — it is a single body of work
that has not been staged. It gets sub-releases when it gets a staging
plan, per §12.

### Live Collaboration (Pro)

| Plan | Scope |
|------|-------|
| **Realtime & CRDT** | Realtime WebSocket client (`signex-collab` crate); CRDT document model for conflict-free concurrent editing; auth (accounts, teams, roles) |
| **Presence & Cursors** | Per-user canvas cursors (coloured + name label); presence panel with online status and follow mode; sheet / region / layer / net locking |
| **Storage & Review** | Cloud project storage, version history + edit attribution; comments pinned to canvas locations with a review workflow; offline support (local SQLite op queue, replay on reconnect); server-side merge / notify / lock functions |

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
| **Thermal & Simulation Wizards** | Elmer FEM bridge (GMSH mesh, `.sif` writer, VTK reader); steady-state thermal analysis with component heat sources; DC IR drop (voltage distribution, current density); 3D thermal overlay on the PCB model; wizards for DDR SI, power supply, thermal, PDN impedance, and RF / antenna; wizard invocation from Signal AI |

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

### Signex 365 (gate) — PLM Core

Signex 365 is the cloud platform connecting the desktop editor to the
PLM. See `PLM_INTEGRATION.md`.

| Plan | Scope |
|------|-------|
| **Signex 365** | Signex 365 web platform; shared auth (PLM + collaboration + desktop on one account); part-link table (PLM parts ↔ native symbol / footprint IDs by `ObjectId`); project-link table (PLM assemblies ↔ Signex projects) |
| **Component Picker** | Component Picker — desktop queries the PLM for part placement; fast paginated component-search service; lifecycle alerts in the schematic editor (EOL / NRND / Obsolete badges) |

### BOM Studio

| Plan | Scope |
|------|-------|
| **BOM Panel & Pricing** | BOM Studio panel in the desktop editor; live pricing + availability from cached distributor data; BOM cost roll-up per assembly |
| **Lifecycle & Risk** | Lifecycle status (active / NRND / EOL) and part-choice ranking; supply-chain risk alerts (single-source, low stock, long lead time); BOM cost optimisation suggestions (Signal AI); unified component model (symbol + footprint + 3D + sim linked) |

### PLM Sync

| Plan | Scope |
|------|-------|
| **BOM & ECO Sync** | BOM push from Signex to the PLM; delta sync (only changed BOM lines updated); ECO creation from Signex |
| **Documents & Lifecycle** | Document publishing (design files → PLM document management); batch lifecycle status check on file open |

### ERP Bridge

| Plan | Scope |
|------|-------|
| **Odoo / ERPNext Sync** | Released BOMs flow Signex → PLM → Odoo / ERPNext; ERP pricing flows back to BOM Studio as real purchase prices |
| **ECO & Vendor Sync** | ECO traceability through manufacturing (PLM → ERP); vendor sync (PLM AVL → ERP supplier list) |

### PLM Advanced

| Plan | Scope |
|------|-------|
| **Compliance & Obsolescence** | Compliance dashboard integration (desktop shows RoHS / REACH status); obsolescence monitoring with alternative suggestions; AVL badge on the component picker (approved / conditional / disqualified) |
| **PLM-Aware AI** | PLM-aware Signal AI (design review includes supply-chain analysis); reusable testbench library stored in the PLM (`.snxsim` templates) |

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

### 2026-11-07 — AI Design moves first

Maintainer decision (Caner): schematic and PCB design with AI is the next
priority after the Community Preview; the other bands wait. Work starts
now behind the `pro` feature, and it ships in the first release after the
Community Preview. What changed:

- **A new gate, AI Design,** follows the Community Preview. It holds PCB
  Layout, PCB Routing, Router: Walkaround, Chat & Context, Edit & Analysis
  Tools, Generator: Schematic and Generator: Board.
- **Signal AI has two modes,** Assistant and Generator, on one tool layer
  (§8).
- **The PCB Viewer gate is folded into the PCB Layout plan,** which adds
  board creation from the schematic (#1312), board outline and placement.
  A viewer had no board to show: nothing in Signex could create one. The
  milestone was renamed through `aliases`.
- **Gate order changed,** which §12 allows only for an architectural
  reason: the Generator's board stage needs board creation, placement and
  routing, and both modes need the Command Registry, so the AI and the PCB
  work are one dependency chain and are scheduled together.
- **Pulled forward:** the Command Registry (#278, from Advanced Schematic),
  Chat & Context and Edit & Analysis Tools (from after the Pro Release),
  PCB Routing and Router: Walkaround. **Pushed back:** Schematic
  Refinement and the rest of the PCB build-out.
- The Pro Release keeps licensing, billing, plugins and collaboration.

### 2026-11-04 — SemVer to CalVer

Maintainers (Hakan, Caner) moved Signex from semantic versioning to
calendar versioning, `YY.MM.MICRO` (§3). What that changed:

- **The version axis became a name axis.** §7 and §8 identify plans by
  name (Community Preview, Advanced Schematic, PCB Viewer, …). Order,
  tiers, exit criteria and scope are unchanged; only the labels moved.
  The gates are the former v1.0, v2.0, v2.2, v3.0, v4.0 and v5.0.
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
3–5 from AI Design through the Pro Release.

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
- **Plans not yet enumerated and everything past the Pro Release.** Order and timing are
  speculative; user feedback will reshape them. §8 enumerates the
  sub-releases of those bands so that scoped work is visible to
  contributors. **Enumeration is not commitment.** A plan that has a
  heading and a milestone is still a sketch if it sits past the Pro Release.
- **Sub-release boundaries within a band.** Which sub-release a feature
  lands in may change without amending this document; which *band* it
  lands in may not.
- **Pricing (Pro).** Set when the Pro Release is in beta, not earlier.
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
