# Signex — Master Plan

> **Status:** Foundational document. High-level only.
> **Companions:** `ARCHITECTURE.md`, `PRODUCT_AND_EDITIONS.md`,
> `UX_REFERENCE_ALTIUM.md`, `ROADMAP.md`, `REPOSITORY_AND_CODEBASE.md`.

This document answers the *why* and the *what*. It does not describe *how* to
build Signex — that belongs in `ARCHITECTURE.md`. It does not describe editions,
pricing, or UX specifics — those live in their own documents. Read this first,
then follow the cross-references.

---

## 1. Product Thesis

Signex is a **KiCad-compatible EDA editor with a different interaction layer**.

It opens KiCad projects, edits them faithfully, and saves them back as KiCad
files. The differentiation is not the file format. The differentiation is the
editor experience: the interaction model, the visual language, the responsiveness,
and — in the Pro edition — AI assistance and live collaboration layered on top
of unchanged KiCad data.

A user should be able to:

- Open a KiCad project in Signex
- Edit it through a better UI
- Save it
- Open the same project in KiCad the next day without surprises

That loop is the product. Everything else is in service of that loop.

---

## 2. Why This Exists

KiCad is an excellent engineering tool with a dated editor. Altium is an
excellent editor with a hostile license. Between the two sits a real gap:

- Engineers who use KiCad because it is free and open, but who would pay for a
  better editor experience on top of the same files
- Teams who want Altium-quality UX without Altium's cost structure and vendor
  lock-in
- Users who want AI assistance and real-time collaboration integrated into their
  EDA workflow, not bolted on through PDF exports and screen shares

Signex is a bet that the interaction layer can be rebuilt without abandoning the
data layer that the open hardware community already trusts.

We are not trying to replace KiCad. We are trying to be the editor people reach
for when they want to edit a KiCad project.

---

## 3. KiCad Compatibility Stance

**KiCad files are the canonical persisted representation. Signex does not have
a native format.**

This is a deliberate, load-bearing decision. It has several consequences:

- The parser and writer are core platform infrastructure, not import/export
  adapters
- Round-trip fidelity is a first-class requirement, not a nice-to-have
- Unknown or unsupported KiCad constructs are preserved, not discarded
- Opening a project in Signex and then in KiCad must not produce noisy diffs or
  lose data
- Any format evolution in KiCad is a compatibility event for Signex, not an
  opportunity to diverge

We will not ship a `.snxsch` or `.snxpcb` format in v1.0, v2.0, or any planned
release. If there is ever a compelling reason to introduce one — and we have not
found one yet — it will happen only after we have shipped a product that people
already use, and it will be a deliberate separate conversation.

**The compatibility policy in one sentence:** If it opens in KiCad, it opens in
Signex; if Signex saves it, KiCad opens it without complaint.

See `ARCHITECTURE.md` for how this is enforced at the code level.

---

## 4. What Differentiates Signex

Signex competes on four axes, in priority order:

### 4.1. Interaction quality

The editor is built around Altium-class interaction patterns: right-click pan,
context-aware Properties panel, dockable panels, keyboard-first workflow,
predictable selection semantics. This is the baseline. KiCad users regularly
cite editor ergonomics as their largest pain point, and Signex exists to address
that directly.

### 4.2. Modern rendering

Signex renders through `wgpu`. The schematic canvas uses a cached 2D pipeline;
the PCB canvas uses instanced GPU primitives. The goal is smooth interaction on
designs that are large enough to make KiCad's current renderer struggle.
Performance is a feature, not an optimization concern to revisit later.

### 4.3. AI assistance (Pro)

Signal AI is a design copilot integrated directly into the editor — not a
chatbot sidebar, but a tool-using agent that can read the design, run simulations,
suggest fixes, and place circuits. This is Pro-only and is one of two features
that justify the subscription.

### 4.4. Real-time collaboration (Pro)

Multiple engineers edit the same schematic or PCB at once, with live cursors,
region locks, and review workflows. This is Pro-only and is the second feature
that justifies the subscription.

Notably absent from this list: a new file format, a proprietary library ecosystem,
or lock-in of any kind. Compatibility is the opposite of moat, and we are
choosing it deliberately.

---

## 5. v1.0 Scope

v1.0 is a **schematic-only editor**. PCB is deferred to v2.0.

This is a deliberate reduction from earlier scope drafts. The reasoning:

- A credible schematic editor is roughly 12–18 months of work for a small team
- A credible PCB editor is roughly another 12–18 months
- Shipping both at once means shipping neither well
- Schematic capture has lower bars for "credible" than PCB routing does, so it
  is the correct place to start earning user trust

### 5.1. v1.0 must do

- Open and render any KiCad schematic project without visual regression against
  KiCad itself
- Edit: select, move, wire, label, place symbol, delete, rotate, mirror,
  copy/paste, undo/redo
- Save back as `.kicad_sch` with stable, minimal-diff round-tripping
- Annotate and run ERC against the standard KiCad rule set
- Export PDF and BOM
- Handle hierarchical multi-sheet designs correctly
- Library browsing for schematic symbols (the KiCad libraries the user already
  has installed)
- Preserve unknown KiCad constructs on save
- Run on Windows, macOS, and Linux

### 5.2. v1.0 explicitly does not do

- PCB editing, viewing, or export (v2.0)
- 3D viewing (post v2.0)
- SPICE simulation (post v2.0)
- EM or thermal simulation (post v2.0)
- AI assistance (v2.0, Pro)
- Real-time collaboration (v2.0, Pro)
- Plugin system (post v2.0)
- Altium or Eagle import (post v2.0)
- Auto-router (not planned)
- Native Signex file format (not planned)

This list is as important as the "must do" list. If a feature is not on the
"must do" list, it is not in v1.0, regardless of how tempting it is during
development.

---

## 6. Post-v1.0 Direction

The direction after v1.0 is, in order:

1. **v2.0 — PCB editor.** Full PCB editing with routing, DRC, copper pour,
   Gerber/ODB++ export. This is the second half of a credible EDA tool and the
   gate to being taken seriously by professional users.

2. **v3.0 — Pro launch.** Signal AI and live collaboration. These require the
   core editor to be stable first; building AI tool-use against an unstable
   editor means debugging two moving targets at once.

3. **v4.0+ — Simulation, 3D, advanced features.** SPICE, OpenEMS, Elmer FEM,
   3D PCB viewer, high-speed design tools, plugin system. Each of these is a
   major undertaking and should not be confused with v1.0 scope.

The release cadence is **not time-boxed**. v1.0 ships when v1.0 is ready.
Shipping a broken schematic editor to hit a date destroys the trust that the
entire product depends on.

---

## 7. Top Architectural Principles

These are the principles that constrain every architectural decision. They are
elaborated in `ARCHITECTURE.md` but listed here so they cannot be lost.

1. **KiCad files are canonical.** The persisted document is a KiCad S-expression
   file. Everything else is derived.

2. **Four distinct layers.** Raw document, semantic model, UI state, render
   cache. Each has a single responsibility and a defined interface with the
   others. They do not bleed into each other.

3. **The engine is a first-class crate.** All editing goes through
   `signex-engine`. The UI never mutates the document or the semantic model
   directly. This rule has no exceptions.

4. **Two-level patches.** Every edit produces a `SemanticPatch` (user intent)
   and a `DocumentPatch` (raw file change). Undo/redo operates on the semantic
   level; persistence operates on the document level.

5. **Stable identity.** Every semantic object has a stable UUID. Every raw
   document node has a stable handle. Positional identity is never trusted across
   edits.

6. **Unknown content is preserved.** The parser preserves constructs it does
   not understand. The writer emits them unchanged. The semantic model ignores
   them. This is how round-tripping survives KiCad version drift.

7. **Minimal rewrite on save.** The writer targets byte-identical output to
   KiCad's own writer where possible, and stable, structural minimal rewrite
   otherwise. Unchanged content is not reformatted.

8. **Rendering derives from semantics.** The canvas never reads raw S-expression
   trees. It reads the semantic model, which feeds a cached render layer.

9. **UI state is transient.** Selection, hover, active tool, viewport, and tool
   previews live only in UI state. They are never persisted and never influence
   the semantic model.

10. **Compatibility is not a feature.** It is a platform constraint. Every
    feature is designed with "what does this do to round-tripping?" as a required
    question before implementation begins.

---

## 8. Top Execution Priorities

These are the priorities that govern how we build, not what we build.

### 8.1. Phase 0 must succeed before Phase 1 starts

Phase 0 is a two-to-three-month effort that ships no UI. It produces:

- A faithful KiCad parser
- A round-trip-stable writer
- A raw document model with unknown-node preservation
- A validation suite that round-trips 20+ real KiCad projects byte-identically
  or with acceptable minimal-diff behavior

If Phase 0 cannot demonstrate stable round-tripping on real projects, the
project does not continue. This is the single hardest technical risk and must
be retired first.

### 8.2. The engine comes before the canvas

`signex-engine` exists before any editing UI is built. Editing commands, patch
generation, and undo/redo are all working on a headless test suite before the
first wire is drawn on a real canvas. This prevents the UI from growing into
domain logic.

### 8.3. Tests against real KiCad projects, always

Every merge to `dev` runs the round-trip suite against a growing fixture set of
real KiCad projects. Synthetic fixtures are not enough. We maintain fixtures
from common sources: the KiCad demo projects, open hardware projects on GitHub,
and projects submitted by early users.

### 8.4. No new scope until v1.0 ships

Scope creep is the most common way EDA projects die. Every feature proposed
between now and v1.0 is evaluated against the v1.0 "must do" list. If it is not
on that list, the answer is "after v1.0." This is not negotiable.

### 8.5. Performance is verified, not assumed

Performance targets are written into the test suite. A schematic with 500
symbols must pan and zoom at 60 fps. If a merge breaks this, the merge is
reverted. Performance regressions that accumulate quietly are how good editors
become unusable editors.

### 8.6. Stack decisions are reversible until Phase 0 ends

The working stack is Rust + iced + wgpu. We treat this as the default and
build toward it, but Phase 0 is also the window during which we are allowed to
reconsider. If iced's docking, text editing, or widget ecosystem proves
insufficient during the parser/writer phase, we change the UI stack before
building the UI, not after.

After Phase 0 ends, the UI stack is locked.

---

## 9. What Could Kill This Project

Listed honestly so we can avoid them:

- **Shipping native format too early.** Breaks the compatibility story, which
  is the product's only real differentiator from Horizon EDA and others.
- **Building UI before engine.** Domain logic metastasizes into the UI and the
  codebase becomes unfixable.
- **Pretending the scope is smaller than it is.** Signex is years of work. A
  sprint plan that says otherwise is lying.
- **Treating round-tripping as an edge case.** Lose one KiCad user's project
  metadata and the trust is gone.
- **Building Pro features before Community is stable.** AI and collaboration
  are exciting and distracting. They do not matter if the editor is broken.
- **Maintaining too many forks.** `alplabai/ngspice`, `alplabai/elmerfem`, etc.
  Each fork is a long-term tax. Minimize them.

---

## 10. Signals of Success

These are the markers that tell us the project is working:

- **Phase 0 complete:** 20+ real KiCad projects round-trip cleanly
- **v0.5:** An engineer can edit a real schematic in Signex, save it, open in
  KiCad, and see no unexpected diffs
- **v1.0:** A KiCad user reports that Signex has replaced KiCad as their
  schematic editor for at least one project
- **v2.0:** The same for PCB
- **v3.0:** A team pays for Signex Pro because Signal AI or collaboration
  changed their workflow

If we hit these markers, the product is real. If we do not hit them, no amount
of additional scope will rescue it.

---

## 11. What to Read Next

- **`ARCHITECTURE.md`** — How the four layers, the engine, and the patch system
  are structured in code. Start here if you are writing any Signex code.
- **`PRODUCT_AND_EDITIONS.md`** — Community vs. Pro, pricing, feature gating.
  Start here if you are making business or packaging decisions.
- **`UX_REFERENCE_ALTIUM.md`** — Canonical UX specification. Start here if you
  are building any user-facing surface.
- **`ROADMAP.md`** — Sprint plan, workstreams, version tiers, staffing. Start
  here if you are planning work.
- **`REPOSITORY_AND_CODEBASE.md`** — Workspace layout, branch rules, crate
  ownership. Start here if you are setting up the repo or onboarding an engineer.
