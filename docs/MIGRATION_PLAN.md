# Signex — Architecture Migration Plan

> **Status:** Active migration plan.
> **Audience:** Engineers moving the current codebase toward the target
> architecture in `ARCHITECTURE.md`.

This document defines the staged migration from the current `signex-app`-heavy
editor implementation to the target architecture built around a dedicated
engine, semantic model, and raw KiCad document layer.

The goal is not a rewrite. The goal is controlled extraction with green builds
after every step.

---

## 1. Migration Principles

- Migrate in small, reversible steps.
- Preserve current editor behavior unless a step explicitly changes behavior.
- Prefer introducing seams before introducing new crates.
- Keep the workspace buildable after each phase.
- Treat `signex-app` as the temporary host for transition code until ownership
  can move cleanly.

---

## 2. Current Reality

Today, `signex-app` still owns too much of the edit lifecycle:

- command interpretation
- direct mutation of schematic state
- undo/redo orchestration
- file open/save orchestration
- canvas invalidation and state sync

This is acceptable only as an intermediate state.

---

## 3. Target End State

The intended end state remains the one described in `ARCHITECTURE.md`:

- `kicad-document` owns raw KiCad persistence state
- `signex-model` owns semantic objects and identity mapping
- `signex-engine` owns all mutations, undo/redo, and patch generation
- `signex-app` owns only UI state and presentation flow
- `signex-render` derives render cache state from semantic objects

---

## 4. Phases

### Phase 0 — Stabilize migration seams

Goal: reduce repeated direct mutations inside `signex-app` and centralize them
behind explicit helper entry points.

Deliverables:

- central mutation gateway in `signex-app`
- update handlers stop hand-rolling the same mutation/sync path repeatedly
- file I/O and edit flows become easier to lift into a future engine crate

Exit criteria:

- repeated `undo_stack.execute + canvas sync + dirty + commit` patterns are
  funneled through shared methods

### Phase 1 — Introduce engine vocabulary

Goal: define `Command`, `CommandResult`, and engine-facing errors without moving
all behavior at once.

Deliverables:

- `signex-engine` crate skeleton
- command enums for user-intent-level operations
- app updates translate UI messages into commands at a small number of seams

Exit criteria:

- new editing features add commands first, not ad hoc UI mutation paths

### Phase 2 — Extract semantic ownership

Goal: introduce a semantic model layer separated from the UI.

Deliverables:

- `signex-model` crate
- stable object identity and read-only model access from UI
- renderer consumes model-derived data instead of app-owned schematic state

Exit criteria:

- `signex-app` stops owning the primary mutable semantic document

### Phase 3 — Extract raw document ownership

Goal: formalize the raw KiCad document layer.

Deliverables:

- `kicad-document` crate
- stable raw node identity
- preservation path for unknown KiCad content

Exit criteria:

- parse/write flows stop treating typed app structs as the only persisted state

### Phase 4 — Move execution into `signex-engine`

Goal: make the engine the only mutation path.

Deliverables:

- engine executes commands and owns undo/redo
- app delegates edit, save, and load workflows through engine APIs
- render invalidation is driven by engine results

Exit criteria:

- `signex-app` no longer mutates Layer 1 or Layer 2 directly

---

## 5. Immediate Next Steps

## 6. Progress Snapshot

Phase 0 / Step 1 status: complete.

Phase 1 status: started.

Completed in the current migration slice:

- app-local edit execution now flows through `mutation_gateway`
- extracted edit handlers use shared mutation finalization instead of open-coded
  schematic sync
- direct `undo_stack.execute(...)` calls were removed from `app.rs` and the
  extracted edit/update modules
- file-load and sheet-swap flows now route through a dedicated `load_gateway`
- `signex-engine` crate skeleton exists with initial `Command`,
  `CommandResult`, `PatchPair`, and `EngineError` types
- the in-place text submit flow is the first real UI edit path that now emits
  `signex-engine::Command::UpdateText` and executes it through the engine
- symbol property edits for designator, value, and footprint now emit
  `signex-engine::Command::UpdateSymbolFields` and execute through the engine
- delete-selection now emits `signex-engine::Command::DeleteSelection` and
  executes through the engine
- direct canvas drag-move now emits `signex-engine::Command::MoveSelection`
  and executes through the engine
- align/distribute batch move flows now execute through repeated
  `signex-engine::Command::MoveSelection` calls in a shared engine session
- `signex-app` now keeps a persistent active-tab `Engine` instance instead of
  rebuilding a fresh engine for every engine-backed command
- engine-backed commands now record snapshot history inside the persistent
  engine, and app undo/redo dispatch begins routing by command origin
- app undo history now stores lightweight engine markers instead of mirrored
  legacy commands for engine-backed actions
- rotate, mirror, and simple placement flows (bus, power symbol, no-connect,
  bus entry, text note) now execute through `signex-engine`
- clipboard paste now emits engine placement commands instead of legacy undo
  batches
- wire placement and its automatic junction insertion now execute inside
  `signex-engine`
- label and text-note property edits now emit `signex_engine::Command::UpdateText`
  instead of app-local legacy edit commands
- read-only UI flows have started switching from direct `self.schematic` reads
  to engine-derived `active_schematic()` access
- app-local undo history is now engine-marker-only; the legacy undo execution
  branch has been removed from `signex-app`
- the `Signex` app state no longer owns a `self.schematic` field; visible
  document reads now resolve from the active engine first and the active tab
  cache second
- render sync now feeds `canvas.render_cache` through a dedicated setter from
  the engine-derived visible schematic; that cache shares one
  `SchematicRenderSnapshot` across draw, hit-test, and selection-overlay paths
- `TabInfo` now exposes `cached_document` explicitly as the inactive-tab and
  tab-switch cache boundary around the active engine-owned document

Still intentionally left for the next slice:

- the render cache still rebuilds a full `SchematicRenderSnapshot` after each
  document mutation instead of applying finer-grained incremental updates
- `cached_document` still stores a full inactive-tab document; that boundary is
  now explicit, but only the schematic variant exists today

Bridge note:

- app-level undo history now acts only as a lightweight step-grouping layer over
  persistent engine history
- grouped engine-backed actions still use marker step counts so one app-level
  history entry can drive multiple engine undo/redo operations

This means both local seams are now in place and the engine vocabulary exists,
but command execution and persistence orchestration still need to move out of
`signex-app`.

Current active step:

1. Start translating one UI mutation path to `signex-engine::Command`
2. Teach `signex-engine::execute` to handle one real edit flow end-to-end
3. Move save/load orchestration behind engine APIs
4. Retire the temporary app-local gateways once engine ownership is proven

This keeps the migration incremental while moving execution ownership toward the
target architecture.
