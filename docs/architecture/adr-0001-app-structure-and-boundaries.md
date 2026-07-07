# ADR-0001 — App structure and boundaries

- **Status:** Proposed
- **Scope:** `crates/signex-app` internals, message design, and crate-boundary discipline
- **Framing:** This document records *what we do* (target decisions), not a retrospective. It was written after auditing the workspace against a reference "Cargo workspace + Clean Architecture + Modular Monolith + MVU" layout. The reference is CRUD/database-shaped and only partially transfers; the decisions below keep the parts that fit a single-process, Iced/MVU, canvas-heavy EDA tool and drop the parts that do not.

Every rule cites the concrete signex reality it is responding to. Evidence is collected in the appendix.

---

## Decisions — what we do

### D1. Colocate `update` with the `state` and `view` it mutates (vertical slices)

A surface that owns its state and view owns its message-handling too. Message-handling logic is **not** hoisted into a central dispatcher; it lives inside the surface's own slice.

- **Target:** dissolve the central `app/dispatch/library.rs` (10,861 lines) by moving each surface's handling into that surface's slice.
- **Model to copy:** `library/editor/footprint/` — the best-decomposed area in the tree (state split across 8 files, canvas split by drawing concern). New and refactored surfaces follow this shape.
- **Why:** every top-N god-file sits on the seam where a slice owns `state + view` but its `update` was pulled out into the horizontal `app/` spine. Colocation removes the seam.

### D2. A slice's message-handling module is named `update/`, split by concern

Canonical slice layout:

```
library/<surface>/
├── mod.rs          # slice's narrow public API (pub(crate) state + update entry)
├── state.rs        # the slice Model
├── view.rs         # rendering (grows into view/ when large)
└── update/         # message-handling, split by concern
    ├── mod.rs      # thin router — an exhaustive match, NO `_` wildcard
    ├── <concern>.rs  # pub(super) fns, named object → action (e.g. datasheet::set_url)
    └── ...
```

- Use **`update/`**, not `reducer/`. `update` is the Elm/MVU term this codebase already runs on (`iced::daemon(new, update, view)`); `reducer` is borrowed Redux/JS vocabulary used nowhere else here.
- `mod.rs` is a router only. All logic lives in the concern files. The router match stays exhaustive (no wildcard) so a new message variant is a **compile error** until it is deliberately routed.
- Naming is **object → action**: the file is the field group it owns, the function is the action (`supply::add_alternate`, `sim::set_pin_node`).

### D3. Namespace messages hierarchically — the root wraps per-surface sub-enums

Each surface owns its own message enum. The root `Message` and the library `EditorMsg` wrap those sub-enums; they do not carry a flat leaf tail.

- **Target shape:** `Message::<Surface>(<Surface>Msg)`, and one level down `EditorMsg::<Surface>(<Surface>Msg)` — mirroring the per-slice `update/` split in D2.
- **Migrate away from:** the ~223-variant root `Message` (only ~20 namespaced, ~200 flat leaves like `PrintPreview*`, `BomPreview*`, `Grid*`, `NetColor*`) and the ~140-variant **flat** `EditorMsg`.
- **Collapse the duplicate:** `EditorMsg` carries dead `Symbol*`/`Footprint*` variants (routed to an inert no-op arm) while real symbol/footprint editing goes through a separate, parallel `PrimitiveEditorMsg` (~134 variants). Merge these onto one namespaced path; do not maintain two enums for the same surfaces.
- **Why:** a flat mega-enum is the message-shaped equivalent of the dispatch monolith. Namespacing per surface is what lets D1/D2 stay local.

### D4. Domain logic lives in domain crates; the app crate orchestrates

`signex-app` is presentation + orchestration. It wires UI state to domain work by *calling* the domain crates; it does not *implement* domain algorithms inline.

- **Rule:** if it is pure computation over domain types (connectivity, geometry topology, netlist, DRC/ERC, BOM math, parsing), it belongs in a domain crate (`signex-engine`, `signex-sketch`, `signex-bake`, `signex-output`, `signex-erc`, `signex-bom`), not in a UI handler or draw file.
- **Model to copy:** `library/editor/footprint/sketch_dispatch.rs` — solves via `signex_sketch`, then delegates *all* geometry baking to `signex_bake::bake_*`. The app only wires results into fields. This is the standard.
- **Fix the two known leaks:**
  - `app/handlers/canvas.rs` (~L500–565) hand-rolls a union-find net-connectivity algorithm inside a click handler → move to a shared `net_connectivity` in `signex-engine`/`signex-output` so net-flood and netlist export share one source of truth.
  - `library/editor/footprint/canvas/draw_sketch.rs:949` `find_closed_loops` duplicates `signex-bake/src/profile.rs::trace_closed_profile` → reuse the domain function instead of re-implementing the walk.
  - (Minor) move world-space primitives `point_in_polygon` / `point_to_segment_dist` from `canvas/geometry.rs` into `signex_sketch::geom`.

### D5. Enforce boundaries with visibility + CI guards, not just crate edges

Cargo blocks the big violations (a domain crate physically cannot import the UI). Everything *inside* `signex-app` needs explicit discipline.

- Slice internals are `pub(super)` / `pub(crate)`. A slice **never** imports a sibling slice's internals or its `view`.
- Add a CI arch-guard (same mechanism as the existing KiCad-surface guards): grep-fail on (a) cross-slice imports between `library/<a>/` and `library/<b>/`, and (b) domain-shaped algorithms appearing in the app crate. A boundary that is not machine-checked will erode.

### D6. Reusable views use three tiers with promotion-by-need

Reusable views are never copied into each slice's `view.rs`. They live at the lowest tier that covers their reuse:

- **Tier 1 — `signex-widgets` crate:** app-wide, domain-agnostic primitives (`icon_button`, `tab_pill`, `tree_view`, …). Already exists and is consumed by `signex-app` and `chrome-catalog`.
- **Tier 2 — `library/shared/view/`:** views reused by ≥2 library slices (today scattered: `editor/datasheet_picker.rs`, `editor/preview.rs`, `editor/params.rs`). Collect here; never duplicate per slice.
- **Tier 3 — slice-local `<slice>/view.rs`:** used by exactly one slice; stays private.
- **Promotion rule (rule of three):** a view starts Tier 3; on the second real consumer it is promoted (not copied) to Tier 2; when broadly reusable and domain-agnostic it moves to Tier 1. Promote on real reuse, not speculation. Dependency direction is one-way (slice → Tier 2 → Tier 1) and compile-enforced.

### D7. Respect the crate DAG; resolve the two domain-to-domain couplings

Keep the acyclic `apps → modules → shared` flow (Cargo already enforces acyclicity). The dependency graph is clean apart from two peer-domain edges to review:

- `signex-library → signex-sketch` and `signex-bake → {signex-sketch, signex-library}` are genuine domain-to-domain coupling. Decide per case: accept as legitimate lower-tier layering, or introduce a shared contract/type in `signex-types` so the peer dependency disappears.
- Edges like `signex-erc-dsl → signex-erc` and `signex-renderer → signex-gfx` are legitimate layering (a parser on its engine; a renderer on a pure-GPU foundation) and are left as-is.

---

## Anti-patterns to avoid (scenarios we stay away from)

These are CRUD/microservice-shaped patterns that do **not** fit signex. Noted here so they are not reintroduced by analogy to generic "modular monolith" templates.

- **No cross-crate event bus.** Iced's `update` loop is already the message bus, centralized in one `Message` enum. A second in-memory/event bus between crates is redundant indirection for a single-process app.
- **No per-layer crates** (`*_domain`, `*_application`, `*_infrastructure`). Layer *inside* a crate with folders/modules; splitting layers into separate crates multiplies compile units and dependency management for no boundary we don't already get from module visibility.
- **No repository / DB-per-module ceremony.** signex is not CRUD; persistence is atomic file IO + git, and the "application/use-case" role is already covered by the command/engine pattern (`signex-engine` applying `Command::*`).
- **No flat mega-enums.** A message enum that accumulates hundreds of sibling leaf variants is the message-shaped monolith (see D3).
- **No hoisting `update` out of its slice.** Pulling a surface's message-handling into a central dispatcher is exactly what produced the 10,861-line god-file (see D1).

---

## Appendix — current state (evidence)

**Framework:** Iced `0.14` (retained-mode Elm/MVU) — `iced::daemon(Signex::new, Signex::update, Signex::view)` (`main.rs:23`). No egui/eframe anywhere. Custom GPU rendering via `signex-gfx`/`signex-renderer` + `canvas::Program` impls renders *through* Iced, complementing MVU.

**Crate graph (16 members, acyclic):** apex `signex-app` (out-degree 11, in-degree 0); foundation `signex-types` (in-degree 11, out-degree 0). Layers: L0 `{types, bom, gfx, 3d-model-importer}` → L1 `{engine, erc, sketch, widgets, erc-dsl, output, renderer}` → L2 `{library}` → L3 `{bake, library-server}` → apex `{app, chrome-catalog}`. Domain crates are UI-pure (zero `iced` imports; the 4 hits in `signex-library` are comments).

**`signex-app`:** 179 files, ~109,324 lines. Hybrid organization — `app/` is horizontal (`dispatch/`, `view/`, `handlers/`), `library/` is vertical (per-surface slices). Largest files:

| File | Lines |
|---|---|
| `app/dispatch/library.rs` | 10,861 |
| `app/view/mod.rs` | 5,828 |
| `panels/mod.rs` | 4,583 |
| `app/view/dialogs.rs` | 3,525 |
| `app/handlers/dock/sch_library.rs` | 3,088 |
| `library/editor/footprint/canvas/mod.rs` | 3,073 |
| `canvas/mod.rs` | 2,144 |
| `library/browser.rs` | 2,065 |
| `panels/element_properties.rs` | 2,020 |
| `library/editor/footprint/canvas/draw_sketch.rs` | 2,016 |

**Messages:** root `Message` (`app/contracts.rs`) ~223 variants, ~20 namespaced + ~200 flat leaves. `EditorMsg` (`library/messages.rs`) ~140 variants, flat, prefix-namespaced only; parallel `PrimitiveEditorMsg` ~134 variants owns the real symbol/footprint editing while `EditorMsg::Symbol*/Footprint*` are inert.

**Known leaks:** union-find connectivity in `app/handlers/canvas.rs` (~L500–565); `find_closed_loops` in `draw_sketch.rs:949` duplicating `signex-bake/src/profile.rs::trace_closed_profile`; world-space geometry primitives in `library/editor/footprint/canvas/geometry.rs`.

**Best-decomposed reference in-tree:** `library/editor/footprint/` (state in 8 files, canvas split by concern). Least: `library/editor/symbol/` (`state.rs` 1,522, `canvas.rs` 1,980 — still flat).
