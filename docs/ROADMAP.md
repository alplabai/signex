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

| Fact | State (2026-07-15) |
|------|--------------------|
| Latest tagged release | **v0.13.0** (2026-05-31) |
| `workspace.package.version` | **0.14.0** |
| Latest CHANGELOG section | **0.14.0** — written, not yet tagged |
| Current work | Footprint editor, sketch mode, symbol multi-unit, command registry |

Phases 0–6 below (the original v0.1 → v0.8 schematic-editor plan) are
**shipped**. The work since v0.9 — the Apache-clean native-format
cutover, the library browser, the cleanroom renderer rewrite, and the
parametric sketch/footprint editor — was scope added *after* the original
phase model was written, and is not described by any phase. That is why
this document read as fiction until it was reconciled.

**Known gaps, tracked but not resolved by this document:**

- CHANGELOG `[Unreleased]` is empty while trunk carries merged work
  (symbol multi-unit, command registry, god-file decomposition).
- v0.14.0 has a CHANGELOG section but no tag.
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

## 6. Shipped — v0.1 → v0.13

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

This covers Phases 0–6 of the original plan. Those phases are closed.

---

## 7. Release Train — v0.14 → v1.0

Near-term and concrete. **Scope decomposition for each version lives in
the private companion repo** (`signex-internal`, `docs/ROADMAP_DETAIL.md`).
Only the coarse shape is public.

| Version | Scope | Milestone |
|---------|-------|-----------|
| **v0.14.0** | Footprint editor enabled — sketch constraints, active-bar tooling, closed-profile bakes | `v0.14.0 — Footprint Editor` |
| **v0.15.0** | Footprint editor parity — selection filter, units, full pad stack, unified active bar | `v0.15.0 — Footprint Editor Parity` |
| **v0.16.0** | PCB outline editor, pour fill + DRC enforcement, TOML+TSV primitive migration | `v0.16.0 — PCB Outline & Pours` |
| v0.17+ | Properties-panel parity, sketch Fusion parity, parametric pads | *(not yet gated; see internal detail)* |

The v0.17+ band is deliberately not enumerated here. It has been
re-planned repeatedly under labels that never shipped, and publishing a
sequence that has never survived contact with reality is how the last
revision of this document lost credibility. It gets versions when it gets
milestones.

---

## 8. Version Gates — v1.0 → v5.0

Strategic. Undated by policy. A gate ships when its exit criteria pass.

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

| Version | Scope |
|---------|-------|
| v1.1.0 | Advanced schematic — variants, multi-channel, harness, parameter manager |
| v1.2.0 | Schematic tables, ToC, drawing tools, named unions |
| v1.3.0 | Enhanced output — smart PDF, variant BOM, watermark, output jobs |
| v1.4.0 | Design Notebook — Typst editor, component-linked annotations |
| v1.5.0 | Block Diagram — system-level functional blocks, signal flow |

### v2.0.0 — PCB Viewer

PCB rendering via `iced::widget::Shader`, 32 copper layers + technical
layers, layer stack panel, cross-probe with schematic, ratsnest. No
editing.

**Exit criteria:** five real `.snxpcb` fixtures render correctly;
10,000-track PCB pans and zooms at 60 fps; cross-probe works
bidirectionally.

### v2.1.0 — PCB Routing

Professional-grade interactive routing, implemented clean-room under
Apache-2.0 with no reference to other EDA tools' source or format docs.
Ships as five staged sub-releases: greedy (v2.1.0), walkaround (v2.1.1),
push-and-shove (v2.1.2), diff-pair + length tuning (v2.1.3), copper pour
(v2.1.4). DRC (15 base rules) and net-class management land in this band.

Routing is the hardest single feature in EDA and is fundamentally one
engineer's deep work for 3+ months. See `PCB_ROUTER_PLAN.md` for the
authoritative plan.

**Exit criteria:** 50-board fixture corpus routes end-to-end with no
panic, stuck state, or geometry corruption across 10,000 random routing
actions per board; shove converges in ≤8 iterations on 95% of actions;
median action latency ≤16 ms, 95th percentile ≤33 ms.

**Deferred:** multi-track routing, BGA fanout, via stitching → v2.2
candidates. Autorouting → not scoped.

### v2.2.0 — Community Release

Manufacturing output: Gerber X2, Excellon, ODB++, pick-and-place,
IPC-2581, STEP export. Full schematic + PCB editor.

**Exit criteria:** a complete project can be designed, validated, and
exported to manufacturing files; a real PCB designed in Signex has been
successfully fabricated by at least one beta user; zero `tier: p0` bugs.

### v2.3.0 – v2.5.0 — PCB Build-out

| Version | Scope |
|---------|-------|
| v2.3.0 | 3D viewer — 3D PCB, PBR materials, STEP model loading |
| v2.4.0 | Advanced PCB — layer stack editor, impedance, constraints, keepout |
| v2.5.0 | High-speed design — xSignals, DDR SI, eye diagram, PDN analysis |

### v3.0.0 — Pro Release

Signal AI, plugin system, and live collaboration. Pro development happens
in feature-gated crates; the Community editor continues to ship.

Sub-releases: Signal AI core (v3.1), Signal AI tools (v3.2), plugin
system (v3.3), collaboration (v3.4).

**Exit criteria:** a Pro user can hold a productive design conversation
with Signal AI and its tool use is correctly undoable; three test users
can edit the same schematic and PCB simultaneously without conflicts;
pricing is set and subscription billing is operational; Pro and Community
binaries both build clean from a single workspace.

### v4.0.0 — Simulation

Unified simulation view with block-diagram composition, SPICE (ngspice),
simulation wizards. Then PCB sim blocks, parameter sweep / Monte Carlo,
OpenEMS and Elmer bridges, advanced output, and foreign-format import
(Altium, Eagle) plus built-in Git.

### v5.0.0 — Signex 365

Cloud PLM platform: BOM Studio with live pricing and lifecycle status,
ECO workflow, document linking, ERP bridge (Odoo / ERPNext), compliance
dashboard.

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
  speculative; user feedback will reshape them.
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
