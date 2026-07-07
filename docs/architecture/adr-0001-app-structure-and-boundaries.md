# ADR-0001 — App structure, MVU, and domain boundaries

- **Status:** Proposed
- **Scope:** `crates/signex-app` internals, the MVU contract, message design, the domain model, and crate-boundary discipline
- **Framing:** This records *what we do* (target decisions) and the *principles they rest on*. It was written after auditing the workspace against a reference "Cargo workspace + Clean Architecture + Modular Monolith + MVU" layout. That reference is CRUD/database-shaped and only partially transfers; the decisions below keep what fits a single-process, Iced/MVU, canvas-heavy EDA tool and drop what does not.

The document has three parts: **A. Foundations** (the MVU and domain-model principles), **B. Decisions** (what we do), **C. Anti-patterns** (what we stay away from). Every decision cites the concrete signex reality it responds to; evidence is in the appendix.

---

## Part A — Foundations

### A1. The MVU pattern we build on

signex is an [Iced](https://iced.rs) `0.14` application. Iced implements **The Elm Architecture (TEA)**, also called **MVU (Model–View–Update)**. There are three moving parts and one hard rule.

- **Model** — all application state, in one place. `Signex` is the Model; slices own sub-models (`state.rs`).
- **View** — a *pure function* of the Model: `view(&Model) -> Element<Message>`. It produces a description of the UI; it does not mutate state and it does not perform IO.
- **Update** — the *only* place state changes: `update(&mut Model, Message) -> Task<Message>`. It reacts to a `Message`, mutates the Model, and returns a `Task` describing any side effects (async IO, file dialogs) to run next.
- **The hard rule — single source of truth.** State lives in the Model and nowhere else. The view is derived from it every frame; nothing keeps a private shadow copy of state.

signex already runs exactly this loop: `iced::daemon(Signex::new, Signex::update, Signex::view)` (`main.rs:23`), with `update(&mut self, Message) -> Task<Message>`, `subscription() -> Subscription<Message>`, and `view(&self, window) -> Element<Message>`.

**Composition in MVU — how a big app is built from parts.** A parent owns its children's sub-models and calls their `view`/`update`. A child speaks its own message type; the parent embeds it with an enum variant and remaps with `.map()`:

```rust
// parent view: draw the child, remap its messages into the parent's namespace
child::view(&model.child).map(Message::Child)

// parent update: delegate the child's messages back to the child's update
Message::Child(msg) => child::update(&mut model.child, msg),
```

This *is* the vertical-slice pattern: each surface owns `{ state, message, view, update }`, and the root only wraps and delegates. signex already does this for `Menu`, `Tool`, `Dock`, `ActiveBar`, `Library`, `Preferences`, `FindReplace` (dozens of `.map(Message::…)` call sites in `app/view/mod.rs`).

**Reuse in MVU is view functions — NOT stateful components.** This is a load-bearing decision, not a style preference:

- Iced **deprecated the `Component` trait in 0.13** with the stated reason that *"components introduce encapsulated state and hamper the use of a single source of truth."* The official replacement is: use the Elm Architecture directly, or implement a custom stateless `Widget`. ([Iced `Component` docs](https://docs.iced.rs/iced/widget/trait.Component.html), [Iced 0.14 notes](https://news.ycombinator.com/item?id=46185323))
- Elm's own guidance is blunter: *"Actively trying to make components is a recipe for disaster."* Reuse is achieved with **composable view functions** that take state and message-constructors as arguments — not objects with private state. ([Reusable views in Elm](https://dev.to/dwayne/stateless-and-stateful-components-no-reusable-views-in-elm-2kg0))

So when this document says **"widget"** it means a **stateless, reusable view function or custom `Widget`** — a piece of `view` that takes `&state` and returns an `Element`, with all mutable state kept in the owning slice's Model. It never means a self-contained object with its own encapsulated state and update loop (the deprecated `Component`). That older meaning is the exact thing Iced removed, and we do not reintroduce it.

**Side effects live in `Task`, not in `view` or the domain.** The app crate performs IO by returning `Task`s from `update` (file dialogs, exports, network). The domain crates stay pure and synchronous; they never touch Iced or async runtimes.

### A2. The domain model

The **domain model** is the set of types and rules that model the EDA problem — schematics, nets, PCBs, footprints, parametric sketches — independent of any UI or IO. In this workspace it is not one crate but a layer of them: the data lives in `signex-types`, and the behavior lives in the engines that operate on it (`signex-engine`, `signex-sketch`, `signex-bake`, `signex-erc`, `signex-output`, `signex-bom`).

**Data-oriented model + command engine (signex's deliberate style).** Classic DDD favors *rich* aggregates — entities that carry their own behavior as methods — and warns against the *anemic* model (plain data with logic elsewhere). ([Xebia — Functional Domain Modeling in Rust](https://xebia.com/blog/functional-domain-modeling-in-rust-part-1/), [rust-cqrs DDD theory](https://doc.rust-cqrs.org/theory_ddd.html)). signex deliberately splits the two:

- `signex-types` holds the model **data** as plain, serializable structs (`Schematic`, `Pcb`, `Net`, `Project`, `coord`, `anchor2d`, `rotation2d`, `layer`, …).
- The **behavior** is applied by engines — most importantly `signex-engine` applying `Command::*` variants (a command/use-case layer), the `signex-sketch` constraint solver, `signex-bake` geometry baking, and the `signex-erc` rule engine.

This is a considered trade-off, not laziness: plain-data models serialize cleanly, diff well, and make **command-based undo/redo** and atomic file IO natural — which matter more for an interactive editor than method-carrying aggregates do. The command engine *is* our "application layer"; we do not need a separate `application` crate to have one.

**Value objects — where we do carry behavior.** Small immutable, validated types with intrinsic behavior are proper value objects and keep their behavior with them: `coord`, `rotation2d` (with the `Rotatable2d` trait), `anchor2d`. New geometry/units primitives follow this shape — validate on construction, stay immutable, own their operations.

**Consistency boundary = the document, mutated through the engine.** DDD's aggregate rule ("modify the whole through the root, not its parts piecemeal") maps here to: a document (`Schematic` / `Pcb` / `Footprint`) is the unit of consistency, and edits flow through the engine's command/patch path (`DocumentPatch` via the mutation gateway) rather than ad-hoc field pokes. This is what keeps undo/redo and dirty-tracking correct.

**The authoritative-model rule (the one that governs the app crate).** The domain model is the single source of truth for *domain algorithms*. Any computation over domain types — connectivity, geometry topology, netlist building, DRC/ERC, BOM math, parsing — belongs with the model, in a domain crate. The presentation crate may *call* these; it must not *re-implement* them. Re-implementing domain logic in the UI is the anemic-model failure in reverse: behavior that drifts away from the model it belongs to (see the two known leaks in the appendix).

---

## Part B — Decisions (what we do)

### D1. Colocate `update` with the `state` and `views` it mutates (vertical slices)

A surface that owns its state and views owns its message-handling too. Handling logic is **not** hoisted into a central dispatcher; it lives inside the surface's own slice. This is A1's composition rule applied structurally.

- **Target:** dissolve the central `app/dispatch/library.rs` (10,861 lines) by moving each surface's handling into that surface's slice.
- **Model to copy:** `library/editor/footprint/` — the best-decomposed area in the tree.

### D2. Canonical slice layout

```
library/<surface>/            # one vertical slice = one MVU part
├── mod.rs                    # narrow public API: pub(crate) state + update + view entry
├── state.rs                  # the slice Model (the ONLY home of this surface's mutable state)
├── message.rs                # the slice's Message sub-enum (see D3)
├── views/                    # view FUNCTIONS (plural) — the slice's screens/panels
│   ├── mod.rs
│   └── <screen>.rs           # pub(super) fn ...(&State, ...) -> Element<Msg>
├── widgets/                  # slice-local REUSABLE view functions / custom Widgets
│   └── <piece>.rs            #   stateless building blocks; state stays in state.rs (see A1)
└── update/                   # message handling, split by concern
    ├── mod.rs                # thin router — an exhaustive match, NO `_` wildcard
    └── <concern>.rs          # pub(super) fns named object → action (e.g. datasheet::set_url)
```

Naming rules:
- **`views/` is plural** — a slice usually has more than one screen/panel; each is a view function.
- **`update/`, not `reducer/`.** `update` is the MVU term the codebase already runs on; `reducer` is borrowed Redux/JS vocabulary used nowhere else here. The router is a thin exhaustive match (no `_`), so a new message variant is a compile error until deliberately routed. Concern files are named object → action.
- **`widgets/` = stateless reusable views only.** Per A1, these are reusable view functions / custom `Widget`s, never stateful Iced `Component`s. If a widget needs mutable state, that state lives in the slice's `state.rs` and is passed in — the widget stays a pure function of it.

  > Naming note: we use **`widgets/`** consistently across all three tiers — slice-local `<slice>/widgets/`, domain-shared `library/shared/widgets/`, and the app-wide `signex-widgets` crate — matching Iced's own vocabulary. The tier is the scope; the name stays the same.

### D3. Namespace messages hierarchically — the root wraps per-surface sub-enums

Each slice owns its own `Message` sub-enum (`message.rs`). The root `Message` and the library `EditorMsg` wrap those sub-enums; they do not carry a flat leaf tail. This is A1's `.map(Message::Surface)` composition made real in the type system.

- **Target shape:** `Message::<Surface>(<Surface>Msg)`, and one level down `EditorMsg::<Surface>(<Surface>Msg)`.
- **Migrate away from:** the ~223-variant root `Message` (~20 namespaced, ~200 flat leaves like `PrintPreview*`, `BomPreview*`, `Grid*`, `NetColor*`) and the ~140-variant **flat** `EditorMsg`.
- **Collapse the duplicate:** `EditorMsg` carries dead `Symbol*`/`Footprint*` variants (inert no-op arm) while real symbol/footprint editing runs through a separate, parallel `PrimitiveEditorMsg` (~134 variants). Merge onto one namespaced path; do not maintain two enums for the same surfaces.

### D4. Domain logic lives in the domain model; the app crate orchestrates

Direct application of A2's authoritative-model rule. `signex-app` wires UI state to domain work by *calling* the domain crates; it does not *implement* domain algorithms inline.

- **Rule:** pure computation over domain types belongs in a domain crate (`signex-engine`, `signex-sketch`, `signex-bake`, `signex-output`, `signex-erc`, `signex-bom`), not in a UI handler or draw file.
- **Model to copy:** `library/editor/footprint/sketch_dispatch.rs` — solves via `signex_sketch`, delegates *all* geometry baking to `signex_bake::bake_*`, and only wires results into fields.
- **Fix the two known leaks:**
  - `app/handlers/canvas.rs` (~L500–565) hand-rolls a union-find net-connectivity algorithm in a click handler → move to `signex-engine`/`signex-output` so net-flood and netlist export share one source of truth.
  - `library/editor/footprint/canvas/draw_sketch.rs:949` `find_closed_loops` duplicates `signex-bake/src/profile.rs::trace_closed_profile` → reuse the domain function.
  - (Minor) move `point_in_polygon` / `point_to_segment_dist` from `canvas/geometry.rs` into `signex_sketch::geom`.

### D5. Enforce boundaries with visibility + CI guards, not just crate edges

Cargo blocks the big violations (a domain crate physically cannot import Iced — verified: zero `iced` imports in domain crates). Everything *inside* `signex-app` needs explicit discipline.

- Slice internals are `pub(super)` / `pub(crate)`. A slice **never** imports a sibling slice's internals, `views`, or `widgets`.
- Add a CI arch-guard (same mechanism as the existing KiCad-surface guards) that grep-fails on (a) cross-slice imports between `library/<a>/` and `library/<b>/`, and (b) domain-shaped algorithms surfacing in the app crate. A boundary that is not machine-checked erodes.

### D6. Reusable views/widgets use three tiers with promotion-by-need

Reusable pieces are never copied into each slice's `views/`. They live at the lowest tier that covers their reuse:

- **Tier 1 — `signex-widgets` crate:** app-wide, domain-agnostic custom widgets (`icon_button`, `tab_pill`, `tree_view`, …). Already consumed by `signex-app` and `chrome-catalog`.
- **Tier 2 — `library/shared/widgets/`:** reused by ≥2 library slices (today scattered: `editor/datasheet_picker.rs`, `editor/preview.rs`, `editor/params.rs`). Collect here; never duplicate per slice.
- **Tier 3 — slice-local `<slice>/widgets/`:** used by exactly one slice; stays private.
- **Promotion rule (rule of three):** a piece starts Tier 3; on the second real consumer it is promoted (not copied) to Tier 2; when broadly reusable and domain-agnostic it moves to Tier 1. Promote on real reuse, not speculation. Dependency direction is one-way (slice → Tier 2 → Tier 1) and compile-enforced. All tiers hold *stateless* views per A1.

### D7. Respect the crate DAG; resolve the two domain-to-domain couplings

Keep the acyclic `apps → modules → shared` flow (Cargo enforces acyclicity). The graph is clean apart from two peer-domain edges to review:

- `signex-library → signex-sketch` and `signex-bake → {signex-sketch, signex-library}` are genuine domain-to-domain coupling. Per case: accept as legitimate lower-tier layering, or lift a shared contract/type into `signex-types` so the peer dependency disappears.
- `signex-erc-dsl → signex-erc` and `signex-renderer → signex-gfx` are legitimate layering (parser on engine; renderer on a pure-GPU foundation) and are left as-is.

---

## Part C — Anti-patterns to avoid (scenarios we stay away from)

CRUD/microservice- or OO-shaped patterns that do **not** fit signex. Noted so they are not reintroduced by analogy to generic templates.

- **No stateful widgets (the deprecated `Component`).** No self-contained widget with its own private mutable state. It breaks the single source of truth (A1). Reusable pieces are stateless view functions; state lives in the slice Model.
- **No domain logic in the app crate.** No connectivity/geometry/netlist/DRC/BOM algorithm implemented inline in `signex-app` (A2, D4). That is behavior drifting away from the model it belongs to.
- **No cross-crate event bus.** Iced's `update` loop is already the message bus, centralized in one `Message` enum. A second in-memory/event bus between crates is redundant indirection for a single-process app.
- **No per-layer crates** (`*_domain`, `*_application`, `*_infrastructure`). Layer *inside* a crate with folders; the command/engine pattern already gives us an application layer.
- **No repository / DB-per-module ceremony.** signex is not CRUD; persistence is atomic file IO + git.
- **No flat mega-enums.** A message enum with hundreds of sibling leaf variants is the message-shaped monolith (D3).
- **No hoisting `update` out of its slice.** Centralizing a surface's handling is what produced the 10,861-line god-file (D1).

---

## Appendix — current state (evidence)

**Framework:** Iced `0.14` (retained-mode Elm/MVU) — `iced::daemon(Signex::new, Signex::update, Signex::view)` (`main.rs:23`). No egui/eframe anywhere. Custom GPU rendering via `signex-gfx`/`signex-renderer` + `canvas::Program` impls renders *through* Iced, complementing MVU.

**Crate graph (16 members, acyclic):** apex `signex-app` (out-degree 11, in-degree 0); foundation `signex-types` (in-degree 11, out-degree 0). Layers: L0 `{types, bom, gfx, 3d-model-importer}` → L1 `{engine, erc, sketch, widgets, erc-dsl, output, renderer}` → L2 `{library}` → L3 `{bake, library-server}` → apex `{app, chrome-catalog}`. Domain crates are UI-pure (zero `iced` imports; the 4 hits in `signex-library` are comments). `signex-types` = 15 modules (~7,415 LoC), data types plus the `Rotatable2d` and `SnxTable` contract traits.

**`signex-app`:** 179 files, ~109,324 lines. Hybrid — `app/` is horizontal (`dispatch/`, `view/`, `handlers/`), `library/` is vertical (per-surface slices). Largest files:

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

**Known domain leaks:** union-find connectivity in `app/handlers/canvas.rs` (~L500–565); `find_closed_loops` in `draw_sketch.rs:949` duplicating `signex-bake/src/profile.rs::trace_closed_profile`; world-space geometry primitives in `library/editor/footprint/canvas/geometry.rs`.

**Best-decomposed reference in-tree:** `library/editor/footprint/` (state in 8 files, canvas split by concern). Least: `library/editor/symbol/` (`state.rs` 1,522, `canvas.rs` 1,980 — still flat).

---

## References

- Iced — `Component` trait (deprecated 0.13): <https://docs.iced.rs/iced/widget/trait.Component.html>
- Iced 0.14 release discussion: <https://news.ycombinator.com/item?id=46185323>
- "Reusable views in Elm" (view functions over components): <https://dev.to/dwayne/stateless-and-stateful-components-no-reusable-views-in-elm-2kg0>
- Functional Domain Modeling in Rust (Xebia): <https://xebia.com/blog/functional-domain-modeling-in-rust-part-1/>
- DDD theory — aggregates, entities, value objects (rust-cqrs): <https://doc.rust-cqrs.org/theory_ddd.html>
