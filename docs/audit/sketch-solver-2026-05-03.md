# Sketch Solver — Audit Trail

**Started:** 2026-05-03
**Branch:** `feature/v0.13-sketch-mode`
**Base:** `99ee92bc` (= tip of `feature/v0.12-cleanroom-rewrite`; v0.12 not yet tagged)
**Spec authority:** `docs/internal/SKETCH_MODE_PLAN.md`, `docs/internal/SKETCH_MODE_v0.13_PLAN.md`
**Plan:** `docs/internal/SKETCH_MODE_v0.13_PLAN.md`

This document is the contemporaneous audit trail for the v0.13 Sketch
Mode solver work. It exists to make the working discipline visible:
every input the orchestrator and sub-agents consulted is logged here
with a timestamp and a reason. The PR description for v0.13 will be
this file verbatim plus a feature summary.

---

## Discipline checks at session start

| # | Check | Result |
|---|---|---|
| 1 | Skill audit | ✓ Grepped `~/.claude/skills/` and `.claude/skills/` for `solvespace|freecad|planegcs|sketcher|opencascade` — both empty. No archival required. |
| 2 | Memory audit | ✓ `MEMORY.md` lists `project_sketch_mode_plan.md` only as plan reference. No constraint-solver implementation memories present. |
| 3 | Branch state | ✓ Branched off `feature/v0.12-cleanroom-rewrite` at `99ee92bc`. `git status` clean before branching. v0.12 has not yet shipped (PR #79 still draft) so there is no `v0.12.0` tag to branch from; the in-flight v0.12 branch is the closest available base. Will rebase onto `v0.12.0` after v0.12 merges. |
| 4 | Spec doc presence | ✓ Read `docs/internal/SKETCH_MODE_v0.13_PLAN.md` head; will read full file before each phase. |
| 5 | Forbidden inputs | Acknowledged. The orchestrator will not read: any SolveSpace / FreeCAD / Sketcher / planegcs source code, headers, wiki, or blog post; any OpenCascade source; any third-party constraint-solver crate source. Algorithm references are limited to textbooks listed in the plan (Hearn & Baker §10–§12, *Numerical Recipes* Press et al. §15, plus dimensional-analysis sources cited inline). |
| 6 | Tooling | (to be filled at first cargo invocation) |

## References consulted

(append-only; format: `YYYY-MM-DD HH:MM` — title — URL/DOI/ISBN — reason)

---

## Phase log

### Phase 1 — Crate scaffolding + entity types — DONE 2026-05-03

Commits on `feature/v0.13-sketch-mode`:

| SHA | Subject | Tasks |
|---|---|---|
| `cff60f60` | chore(sketch): open cleanroom audit doc for v0.13 solver work | Pre-flight |
| `2bb3fb0c` | feat(sketch): scaffold signex-sketch crate + ID newtypes | 1.1 + 1.2 |
| `e5f20ace` | feat(sketch): Plane / PlaneKind types | 1.3 |
| `14b71eaf` | feat(sketch): Entity / EntityKind types | 1.4 |
| `10f4aec8` | feat(sketch): bake-attribute schema (Pad/Silk/Courtyard/Pour/Keepout/Cutout/V-score) | 1.5 |
| `636bcf3c` | feat(sketch): SketchData container + Array (Linear/Grid/Polar) + BGA numbering | 1.6 + 1.7 + cap |
| `addee00f` | docs(sketch): log Phase 1 completion in audit trail | — |
| `f338294a` | fix(sketch): use signex_types::SignexLayer instead of KiCad-style BoardLayer | post-review fix |
| `fe587fc2` | docs: scrub KiCad-style framing across roadmap, codebase guide, and UX docs | post-review fix |

Result:
- `cargo build -p signex-sketch` clean
- `cargo test -p signex-sketch` — 39 / 39 passing
- `cargo build --workspace` clean (existing signex-app warnings unchanged)
- No third-party constraint-solver code or wikis consulted in this phase.
  All schema decisions follow the plan verbatim; no algorithmic input
  was needed yet (Phase 2 opens the math).

### Post-Phase-1 review fixes (2026-05-03)

The first pass of Phase 1 introduced a private `BoardLayer` enum in
`crates/signex-sketch/src/attr.rs` with KiCad-style short names
(`FCu`/`BCu`/`FMask`/...). The user flagged this as a violation of the
canonical layer policy in `docs/internal/docs/PCB_LAYERS_PLAN.md` and
the issue #62 cleanroom invariants. Two fix-up commits address it:

1. `f338294a` — Code fix:
   - Drop `BoardLayer` from `attr.rs`.
   - Depend on `signex-types` from `signex-sketch`.
   - Use `signex_types::layer::SignexLayer` directly everywhere
     (`TopCopper` / `BottomCopper` / `TopSolderMask` / etc.).
   - Update tests to use the canonical variants.
   - 39 / 39 round-trip tests still pass.

2. `fe587fc2` (main repo) + `199c7b8` (`docs/internal` submodule) —
   Doc scrub:
   - Both v0.13 sketch-mode plans (`SKETCH_MODE_PLAN.md`,
     `SKETCH_MODE_v0.13_PLAN.md`) updated to use `SignexLayer` in all
     code snippets, prose, and inline test examples.
   - `docs/internal/docs/PCB_LAYERS_PLAN.md` reframed: §2 no longer
     credits the foreign EDA tool's layer model as the design source;
     §6 KiCad-import section reframed as handled by the GPL-3.0
     companion repo `signex-kicad-import`.
   - 11 other internal plan docs (PCB_ROUTER, PCB_3D_RENDER, OUTPUT,
     SIMULATION_VIEW, DESIGN_NOTEBOOK, MIGRATION_PLAN, COLLABORATION_
     PLAN, PLM_INTEGRATION, PRODUCT_AND_EDITIONS, altium-gap-analysis,
     "Agentic Hardware Design Assistant") scrubbed for KiCad-style
     layer names and pre-v0.9 "KiCad-native" framing.
   - 4 main-repo docs (ROADMAP, REPOSITORY_AND_CODEBASE,
     UX_REFERENCE_ALTIUM, UI_WORKFLOW) scrubbed similarly.
   - MASTER_PLAN.md and ARCHITECTURE.md flagged for follow-up
     strategic rewrite — their entire architectural premise (Layer 1
     = "Raw KiCad Document"; "KiCad files are canonical") is from
     before the v0.9 cutover and needs a coherent product-thesis
     rewrite, not a surgical scrub.

References consulted in Phase 1: only
`docs/internal/SKETCH_MODE_v0.13_PLAN.md`,
`docs/internal/docs/PCB_LAYERS_PLAN.md` (canonical layer enum),
`crates/signex-types/src/layer.rs` (existing `SignexLayer`
definition). No third-party constraint-solver code, no foreign EDA
source code, no foreign-format wiki/blog/file-format docs.

### Phase 2 — Constraint residuals — DONE 2026-05-03

Commits on `feature/v0.13-sketch-mode`:

| SHA | Subject | Tasks |
|---|---|---|
| `3ef32f61` | feat(sketch): Phase 2 foundation — Constraint enum + state-vector packing + canonical residuals | 2.1 + 2.2 + 2.3 |
| `3cb04db3` | feat(sketch): Phase 2 — residuals for all 18 constraint kinds + total aggregator | 2.4 + 2.5 + 2.6 + 2.7 + 2.8 |

Tasks 2.4–2.7 ran as four parallel agents writing independent
per-family modules (`solver/residuals/{parallel_perp_angle,
point_on, equal_tangent, symmetric_midpoint}.rs`). Each agent owned
exactly two files (one impl, one test file) and never touched the
shared dispatcher, the Constraint enum, or the state-vector module.
All four reported back without conflicts; the orchestrator wrote the
Task 2.8 aggregator and the Task 2.8 tests.

Result:
- `cargo test -p signex-sketch` — 107 / 107 passing
- `cargo build --workspace` clean (existing 65 signex-app warnings
  unchanged)
- All 18 constraint kinds have residual implementations
- Each constraint kind has at least one residual test (most have
  several covering edge cases like degenerate lines, branch-cut
  wrapping, sign convention, mixed Arc/Circle dispatch)

References consulted for Phase 2 residual derivations (cited in
module-level doc comments of each `solver/residuals/*.rs` file):

- Hearn & Baker, *Computer Graphics with OpenGL* — ch. 5, 2D
  vector geometry primitives (cross product as signed area / side-
  of-line, dot product as projection, signed perpendicular distance
  via cross-divided-by-length).
- Press et al., *Numerical Recipes in C* (3rd ed.) — §10.6 (`atan2`
  branch handling), §10 (linear algebra primitives — applies in
  Phase 3 when the Jacobian + LM linear solve land).

No third-party constraint-solver source code, headers, wikis, or
blog posts (SolveSpace, FreeCAD Sketcher, planegcs, OpenCascade,
etc.) were consulted by any agent or the orchestrator during
Phase 2.

### Phase 3 — Solver: Newton-LM + Jacobian + DOF — DONE 2026-05-03

Commits on `feature/v0.13-sketch-mode`:

| SHA | Subject | Tasks |
|---|---|---|
| `133ad62c` | feat(sketch): Phase 3 Stage A — numerical Jacobian + dense LU linear solver | 3.1 + 3.2 (parallel agents) |
| `ca37be55` | feat(sketch): in-house math primitives + LU benchmark + residual refactor | math.rs + bench + Phase 2 refactor |
| `a7c9fb38` | feat(sketch): Phase 3.3 — Levenberg–Marquardt iteration + LuDecomposition wrapper | 3.3 |
| `88c2a713` | feat(sketch): Phase 3 Stage C — canonical sketch corpus + DOF analysis (Householder QR) | 3.4 + 3.5 (parallel agents) |
| `f43a8be2` | feat(sketch): Phase 3.6 — Solver public API + AutoPauseState hysteresis | 3.6 |

Architecture decisions:

- **Stayed dependency-free.** Initial plan was to use `nalgebra`
  (Apache-2.0/MIT pure-Rust LA library) for the LM step, but the
  user reversed that choice mid-Phase-3 in favour of an in-house
  math library so signex-sketch has zero external numeric crates.
  The roll-our-own LU benchmark (`examples/bench_linalg.rs`) shows
  ~80 µs at n=100 unknowns on a 2024-class laptop — comfortably
  inside the 50 ms LM budget. nalgebra-style API ergonomics
  (`LuDecomposition`/`QrDecomposition` structs with `new()` +
  `solve()`/`rank()` methods) are adopted as inspiration only;
  the implementations are first-principles textbook code.

- **Math primitives factored into `solver/math.rs`.** 18 free
  functions cover 2D vector ops (sub/add/scale/dot/cross/norm/
  distance/wrap_to_pi) and dense vector+matrix ops (norm_sq/
  norm_vec/axpy/matvec/matvec_t/matmul_ata/add_diag). The Phase 2
  residual modules were refactored to compose from these primitives
  instead of inlining 2D arithmetic — same behaviour, cleaner code,
  single source of truth, easier to optimise via `#[inline]`.

- **DOF analysis uses a conservative coarse rule** (rank(J) == n →
  all free Points Full; otherwise all Under) plus residual-magnitude
  over-detection. Documented as intentional in `solver/dof.rs`. A
  future revision can swap to per-column rank-deficiency detection
  for finer granularity; the canonical under/full/over test cases
  pass under the coarse rule.

References consulted for Phase 3 algorithms (cited in module-level
doc comments):

- **Hearn & Baker, *Computer Graphics with OpenGL*** — ch. 5 (2D
  vector geometry primitives).
- **Press et al., *Numerical Recipes in C* (3rd ed.)** —
  - §2.1 (vector and matrix conventions),
  - §2.3 (LU decomposition with partial pivoting),
  - §2.10 (QR decomposition via Householder reflections),
  - §5.7 (numerical derivatives — central difference + step-size
    selection),
  - §15.5 (Levenberg–Marquardt method).

API inspiration only (Apache-2.0 license-compatible per user
authorisation 2026-05-03):

- **nalgebra** — `LuDecomposition` and `QrDecomposition` struct
  shapes (factor once + reuse). No nalgebra source code was read;
  the inspiration is the API ergonomics, not the implementation.

No third-party constraint-solver source code, header, wiki, or
blog post (SolveSpace, FreeCAD Sketcher, planegcs, OpenCascade,
etc.) was consulted by any agent or the orchestrator during
Phase 3.

Result:
- `cargo test -p signex-sketch` — 167 / 167 passing
- `cargo build --workspace` clean
- `examples/bench_linalg.rs` documents performance baseline:
  ~80 µs full LU solve at n=100, ~540 µs at n=200 (roll-our-own,
  no SIMD)
- All 5 canonical sketches (anchored line + rectangle +
  parallelogram + isosceles triangle + regular hexagon) solve
  within 1e-6 in 4–6 LM iterations
- DOF colouring works on the three canonical cases (under / full /
  over)
- AutoPauseState hysteresis tested (single overrun no-pause, 2
  consecutive pauses, good observation resets, unpause clears)

### Phase 4 — Expression parser + evaluator + units — DONE 2026-05-03

Commit `e465fce0`. 6 tasks (4.1 unit parser, 4.2 AST, 4.3
recursive-descent parser, 4.4 evaluator with unit type-checking, 4.5
parameter table + topo resolution, 4.6 DimTarget::Expr full eval).
Tasks 4.1–4.4 ran as four parallel agents on independent files.
Reference cited: Aho/Sethi/Ullman *Compilers: Principles,
Techniques, and Tools* (Dragon Book) for the recursive-descent
parser. Test count grew from 167 to 257.

### Phase 5 — Schema migration + library integration — DONE 2026-05-03

Commits `070153d3` (Tasks 5.1+5.2: Footprint schema bump v1→v2 with
optional `sketch: Option<SketchData>`, plus 3-fixture migration test
corpus) and `57811487` (Tasks 5.3+5.4+5.5: FootprintEditorState
gains mode/sketch_solver/last_solve/auto_pause/solve_warnings
fields; SketchEdit + SketchModeMsg enums; sketch_dispatch.rs with
solve-on-edit dispatcher).

### Phase 6 — UI mode switcher + tool palette — DEFERRED to v0.13.1

The dispatcher + state fields + message types (Phases 5.3–5.5 above)
expose all v0.13 functionality for programmatic + test-driven use.
The full iced view layer (tool palette, sketch render layer, DOF
overlay, constraint icons, inspector panel) is multi-day iced+canvas
integration work that wasn't safely automatable in the single
session this branch was authored in. Tracked for v0.13.1.

### Phase 7 — Pad-only bake pipeline — DONE 2026-05-03

Commit `ebe3c481`. New `signex-bake` crate (depends on both
signex-sketch and signex-library, breaking the unavoidable
dependency cycle from Phase 5.1). Tasks 7.1+7.2 (bake_pads +
LinearArray bake) ran as a parallel agent; Task 7.3 (wire bake into
solve-on-edit dispatcher) shipped with Phase 5.4 in commit
`57811487`. Layer-name strings come from
`signex_types::layer::SignexLayer::altium_label()` — no foreign-
tooling short names. 11 bake tests + 4 dispatcher tests.

### Phase 8 — End-to-end smoke + verification — DONE 2026-05-03

Task 8.1 — `crates/signex-app/tests/sketch_qfn16_smoke.rs` drives
the entire stack programmatically: parameter resolution + expression
evaluation + LM solver + DOF analysis + pad bake + sketch ↔ library
integration. 3 tests all pass: `qfn16_row_bakes_at_05mm_pitch` (4
SMD pads at correct positions to within 1 µm),
`qfn16_row_regenerates_when_pad_pitch_changes` (re-resolve + re-
bake on pitch parameter edit), `qfn16_solve_warnings_empty_on_clean_sketch`.

Task 8.2 — Schema migration corpus already covered by
`crates/signex-library/tests/migration_v1_to_v2.rs` (5 tests, all
green). No additional edge cases discovered.

Task 8.3 — `.github/workflows/license-guard.yml` extended with two
new jobs:
- `no-third-party-constraint-solver-substrings` — forbids
  `solvespace|freecad|planegcs|opencascade|sketcher` substrings
  under `crates/signex-sketch/` and `crates/signex-bake/`.
- `no-third-party-constraint-solver-attribution` — forbids "from
  SolveSpace" / "based on FreeCAD" / similar attribution comments
  anywhere in the repo (excluding audit trail and the workflow file
  itself).
`crates/signex-sketch/deny.toml` ships with the standard Apache-
clean license allow-list so cargo-deny can be run on the sketch
crate in isolation.

Task 8.4 — PR self-declaration block lives in the PR body (added at
push time, not committed). Template:
```
## Cleanroom self-declaration
- [ ] No SolveSpace source code was loaded into context
- [ ] No FreeCAD/planegcs source code was loaded
- [ ] No third-party EDA tool's sketcher source was consulted
- [ ] All algorithm references cite public textbooks
- [ ] Audit doc at docs/audit/sketch-solver-2026-05-03.md lists
      every reference consulted
```

### Final test count

cargo test workspace-wide: 290+ tests across signex-sketch,
signex-bake, signex-library, signex-app, signex-types.
- signex-sketch: 257 tests (12 lib + 39 round_trip + 7 solver_basics
  + 18 linalg + 4 dof + 3 lm_basic + 22 canonical + 6 solver_api
  + family-residual files + expression suite)
- signex-bake: 11 tests
- signex-library: 5 migration + pre-existing tests
- signex-app: 4 dispatcher + 3 QFN-16 smoke + pre-existing tests
- signex-types: pre-existing tests

cargo build --workspace: clean (only pre-existing 65 signex-app
warnings unchanged from before Phase 5).

### v0.13 ships with

- Apache-clean signex-sketch crate (sketcher schema + Phase-2
  residuals + Phase-3 LM solver + DOF + Phase-4 expressions)
- Apache-clean signex-bake crate (sketch → library Pad pipeline)
- Footprint::sketch field (signex-library), v1→v2 migration
- Solve-on-edit dispatcher (signex-app)
- 290+ tests; cleanroom audit trail; License Guard CI extension
- Phase 6 UI deferred to v0.13.1

References consulted overall (cumulative across Phases 1–8):
- Hearn & Baker, *Computer Graphics with OpenGL*, ch. 5
- Press et al., *Numerical Recipes in C* (3rd ed.) §§ 2.1, 2.3,
  2.10, 5.7, 15.5
- Aho/Sethi/Ullman, *Compilers: Principles, Techniques, and Tools*
  (Dragon Book) — recursive-descent parser

API inspiration only (Apache-2.0 license-compatible, no source
consulted): nalgebra (`LuDecomposition` / `QrDecomposition` struct
shapes).

No third-party constraint-solver source code, header, wiki, or blog
post (SolveSpace, FreeCAD Sketcher, planegcs, OpenCascade, etc.)
consulted at any phase.

---

## v0.13.4 — Code review fixes (2026-05-04)

Branch `feature/v0.13.4-review-fixes` off `feature/v0.13.3-sketch-ui-final`.
Six commits, one per review issue from the post-v0.13.3 `/code-review`
pass:

| SHA prefix | Subject | Fix |
|---|---|---|
| `8af68bf9` | fix(ci): word-boundary the v0.13 license-guard regex | CI BLOCKER — `\bsketcher\b` so `SketchError` doesn't false-positive; pathspec exclude legit wordmark uses |
| `22331dfd` | fix(sketch): thread Solver::tolerance + max_iters through to solve_lm | Solver fields were ignored; constants TOL_SQ/MAX_ITERS removed |
| `af5a87ee` | fix(sketch): div-by-zero in eval_div_mod returns Domain error | All 5 (family, family) Div/Mod branches now reject zero divisor |
| `7b0835d8` | fix(app): preserve literal pads when sketch is empty | bake gated on !sketch.entities.is_empty() |
| `2bb107f6` | fix(app): surface solver errors in inspector solve_warnings | New `apply_sketch_edit_with_warnings` helper; 10 dispatch sites |
| `9b632f68` | fix(bake): skip construction entities in closed-profile warning loop | Mirrors the bake loop's existing construction skip |

908 / 908 workspace tests pass post-fix; License Guard regex returns
zero hits. References consulted: none new — all fixes were corrective
on existing code.

---

## v0.14 — Sketch-mode bake extras (in flight from 2026-05-04)

Branch `feature/v0.14-sketch-bake-extras` off
`feature/v0.13.4-review-fixes`. Three commits so far:

| SHA prefix | Subject | Tasks |
|---|---|---|
| `cce65ce9` | feat(library): v3 schema — pour / keepout / cutout / v_score / mask / paste_aperture fields + Castellated/Fiducial/Chamfered pad variants | Stage 1: schema bump + lib variants |
| `c4b9c1e8` | feat(bake): closed-profile walker for v0.14 silk/courtyard/mask/pour bakes | Stage 2: walker (Lines only, Arc tessellation deferred to v0.14.1) |
| `20befcb5` | feat(bake): silk + courtyard + mask + pour bakes; native lib variants | Stage 3: 4 new modules + dispatcher wiring + drop v0.13 lib-variant fallback warnings |

Test count: signex-bake grew from 13 lib tests to 32; signex-library
gained 10 v3 schema tests. 67 / 67 workspace test runs green.

Cleanroom: walker is textbook DFS (Cormen *Introduction to
Algorithms* §22.3 conventions); no third-party CAD-tooling source
consulted.

References consulted in v0.14 so far:
- (no new external references — all module designs derive from the
  existing v0.13 schema + textbook graph traversal)

### v0.14 deferred to v0.14.1+

The original v0.14 plan covered eight sub-tasks (A-H). Stages 1-3
above closed A (silk), B (courtyard), C (mask × 3 attrs), D (pour),
plus the lib-variant native bakes from G. Remaining for v0.14.1+:

- E. Keepout / cutout / v-score bake — `pad.rs` warning loop now
  flags these as "v0.14.1 feature".
- F. 3D extrude — sketch profiles on `BodyTopPlane` enrich
  `body_3d.outline` with the closed sketch profile.
- G. Lib variants (partial) — `LibPadShape::Custom(SketchProfile)`
  still falls back to bbox Rect with a warning.
- H. Stock library — bundle ~50 common footprints as parametric
  sketches.
- Walker scope — Arc tessellation in profiles (currently any Arc
  errors with `TraceError::ArcInProfile`).
- UX deferred items from v0.13.3 (Shift+Click multi-select drag,
  drag-to-move Point, Angle / DistancePtLine inspector, per-
  constraint delete, dimension-edit-in-place, modal value entry
  with units).

---

## v0.14.1 — Bake completion (2026-05-04)

Branch `feature/v0.14.1-bake-completion` off
`feature/v0.14-sketch-bake-extras`. Five commits closing every v0.14
deferred item except the stock library:

| SHA prefix | Subject | Stage |
|---|---|---|
| `24e36137` | feat(bake): walker now tessellates Arc segments | Walker upgrade |
| `a6c137f3` | feat(bake): keepout + cutout + v_score bake modules | E |
| `985bd81f` | feat(bake): 3D extrude profile from BodyTop plane | F |
| `340a6f1a` | feat(bake): native LibPadShape::Custom(SketchProfile) bake | G (final) |

Walker upgrade — `crates/signex-bake/src/profile.rs`:
- Drop `TraceError::ArcInProfile` (no longer produced).
- Adjacency now includes both Lines and Arcs; Arcs traverse with
  `ARC_SAMPLES = 16` interior vertices via polar sampling around the
  arc's centre.
- Reverse-traversal flips effective sweep direction so tessellation
  always sits on the correct side of the chord.
- 3 new walker tests (D-shape CCW, D-shape CW, arc-seed walks back
  through line).

Keepout / cutout / v-score — three new modules in `signex-bake`:
- `keepout.rs` — KeepoutAttr profiles → FpKeepout. KeepoutKinds
  6-bit field maps to KeepoutForbid 5-variant enum (multiple bits →
  All; single bit → matching variant).
- `cutout.rs` — BoardCutoutAttr profiles → FpCutout. edge_radius and
  through=false warned as v0.15 lib-field gaps.
- `vscore.rs` — VScoreHintAttr-tagged Lines → FpVScore. Single-Line
  records (no walker). Uses `NOMINAL_BOARD_THICKNESS_MM = 1.6` (IPC-
  A-600 default) to convert depth_fraction → mm. Arc/Circle entities
  with VScore skip with a warning.

3D extrude — new `body3d.rs` module:
- Find first `PlaneKind::BodyTop` plane.
- Find first non-construction Line/Arc on that plane.
- Trace closed profile, set `body_3d.outline = Some(Polygon)` and
  `body_3d.offset_z_mm = eval(plane.offset_z_expr)`.
- height_mm preserved (caller's responsibility).
- 4 new lib tests (no plane / normal / bad expr / no edges).

Native SketchProfile bake — `pad.rs`:
- `bake_shape` signature gains `sketch`, `solve`, `pad_position`.
- Custom::SketchProfile traces the closed profile, translates
  vertices to pad-local mm (subtract pad position), emits
  `LibPadShape::Custom(LibPolygon)`.
- Empty source / trace failure → fallback Rect + warning.
- 1 new integration test (`bake_custom_sketch_profile_native_v0141`).

Dispatcher (`sketch_dispatch.rs`) now invokes 9 bake modules in
sequence: pads + arrays + silk + courtyard + 3 mask + pour + keepout
+ cutout + v_scores + body3d.

Test count: signex-bake lib tests grew from 21 to 36 (15 new); plus
1 new integration test (14 total). Workspace: 67/67 test runs green.

References consulted in v0.14.1 (cited in module-level doc comments):
- Hearn & Baker §3.13 ("Drawing Circular Arcs") — arc tessellation
  via polar sampling.
- IPC-A-600 — nominal board thickness 1.6 mm for FR-4 (used as the
  v-score depth scale factor).

Cleanroom: no new third-party CAD/EDA source consulted. All bake
algorithms derive from textbook references + the existing v0.13/v0.14
foundation.

---

## v0.14.2 — UX iteration (2026-05-04)

Branch `feature/v0.14.2-ux` off `feature/v0.14.1-bake-completion`.
Twelve commits closing UX feedback while testing the v0.14.1 bake
pipeline:

| SHA prefix | Subject |
|---|---|
| `deeb7785` | perf(footprint): batch grid lines + live pan/zoom updates |
| `a89c871f` | feat(footprint): mode-context-switch with Fusion-style + Altium-style active bars |
| `a5cc11a3` | feat(panels): context-aware Footprint editor Properties panel |
| `293c325a` | fix: enable Save / Save As for primitive editor tabs + drop stale dirty dot |
| `830a8839` | fix(save): also save dirty .snxprj when Ctrl+S fires on a primitive tab |
| `f870ef9c` | feat(footprint): live ghost previews + canvas-under-bar Stack + mode-in-bar |
| `7787552e` | feat(panels): Auto-fit Courtyard toggle on Properties panel |
| `443c0f91` | feat(footprint): remove Auto-fit Courtyard button from active bar |
| `1c59aa71` | feat(footprint): separate floating mode switch (top-right, Sketch·Pads·3D) |
| `394d56f2` | fix(sketch): live ghost preview + 1/2/3 mode shortcuts |
| `d83cff56` | feat(panels): Footprint Library panel — Altium PCB Library parity |
| `026ecd60` | feat(footprint): auto-mint sketch Points for literal pads on Sketch entry |

Highlights:
- Floating active bar over canvas with mode-specific items (Pads
  vs Sketch); separate mode-switch chip at top-right.
- Live ghost previews for Line / Circle / Arc tools.
- 1 / 2 / 3 keyboard shortcuts for mode switching.
- Footprint Library panel mirrors SCH Library — lists sibling
  `.snxfpt` files in the containing `.snxlib`.
- Auto-mint sketch Points for literal pads on first Sketch entry —
  foundation for v0.15 bidirectional sync.

---

## v0.15 — Bidirectional sketch ↔ pads + schema gaps (2026-05-04)

Branch `feature/v0.15` off `feature/v0.14.2-ux`. Two commits so far,
work in flight:

| SHA prefix | Subject |
|---|---|
| `8633d6c9` | feat(footprint): bidirectional Pads → Sketch live mirror |
| `4c2de82a` | feat(library, bake): close v0.14.1 schema gaps — FpCutout edge_radius + through; FpVScore side + min_web |

**Bidirectional Pads → Sketch live mirror (`8633d6c9`):**
- New `EditorPad.sketch_entity_id: Option<SketchEntityId>` field
  links each canvas pad to its backing sketch `Point` carrying
  `PadAttr`.
- `auto_mint_for_literal_pads` now writes the minted entity IDs
  back into each EditorPad.
- Three new mirror helpers: `mirror_add_pad_to_sketch`,
  `mirror_move_pad_in_sketch`, `mirror_delete_pad_from_sketch`.
  Wired into `FootprintAddPad` / `FootprintMovePad` /
  `FootprintDeleteSelected` dispatchers (gated on "sketch is
  active" so users who only ever work in Pads mode don't get a
  silent sketch).

**Schema gaps closure (`4c2de82a`):**
- `FpCutout.edge_radius_mm: f64` + `through: bool` — sourced from
  `BoardCutoutAttr.edge_radius_expr` / `through`.
- `FpVScore.side: VScoreSide` (Both / Top / Bottom enum) +
  `min_web_mm: f64` — sourced from `VScoreHintAttr.side` /
  `min_web_expr`.
- Bake modules drop the v0.14.1 "deferred" warnings; tests
  rewritten to assert the baked field values.

### v0.16.0.1 + v0.16.1 (2026-05-04) — snap, chained Line, construction, filled loops, ghost, TAB pause, always-live solver

**v0.16.0.1 hotfix:**
- Centre-Point drag in Sketch mode now also translates the pad's 4
  outline-corner Points so the construction outline tracks the pad
  during sketch-mode drags. Without this the corner outline was
  stranded at the previous centre after a drag.
- `FpEditorToggleAutoFitCourtyard` handler now refreshes the panel
  context after dispatching the toggle so the pill's pressed-state
  style updates immediately.
- New `library/editor/footprint/snap.rs` module — Fusion-style cursor
  snap with priority chain (Point hit > Horizontal/Vertical within
  5° of axis > 15° angle increments > 0.1 mm grid). Wired into the
  canvas's `ButtonPressed` + `CursorMoved` handlers; only fires in
  Sketch mode (Pads-mode literal pads stay raw).

**v0.16.1:**
- **Chained Line tool** — Line tool's second click commits a segment
  AND advances `ToolPending::LineFirst` with the just-clicked endpoint
  as the new anchor instead of resetting to Idle. Esc / right-click
  cancel back to Select.
- **Construction-mode toggle** on the sketch active bar (┄ glyph) —
  sticky pill. While ON, every newly-minted Line / Arc / Circle /
  Rectangle / RoundedRectangle / Point gets `construction = true`.
  Bake skips construction entities.
- **Filled closed-loop rendering** — `draw_filled_closed_loops` walks
  the line graph per-frame, finds simple cycles whose Lines aren't
  all construction-flagged, fills the polygon at low opacity. Pad-
  corner outlines (all-construction) are skipped.
- **Pad-placement ghost** — Pads mode + `PadsTool::PlacePad` renders a
  translucent 1×1 mm rectangle at the snapped cursor showing where
  the next pad will land. Desaturates to grey + dashed-stroke when
  `placement_paused`.
- **TAB pause/resume** for ALL primitive placement (PadsTool::PlacePad
  + every non-Select sketch tool). `placement_paused` flag gates
  click-publish; `tool_pending` survives a pause/resume cycle.
- **Always-live solver** — removed `solve_and_bake` early-return on
  `auto_pause.paused()`. Footprint sketches stay small (tens-to-low-
  hundreds of entities); the solver's per-frame cost is well below
  the per-frame budget. Active-bar pause-toggle button retired.
- **Footprint Library panel** for lone `.snxfpt` files (not inside a
  `.snxlib`) now shows the open footprint as a single-row list rather
  than the misleading "(0 footprints) — right-click the .snxlib"
  empty state.

Tests: 116 lib + 3 integration green; `auto_pause_skips_solve` →
`solver_always_runs_even_when_auto_pause_observed` (assertion
inverted).

**Deferred from v0.16.1:**
- **Role assignment UI** — inspector dropdown / button row to pick
  Unassigned / Pad / Silk-Top / Silk-Bottom / Courtyard / Keepout /
  Cutout / Mask Opening / Pour. Writes the appropriate `*HintAttr` to
  the selected entity (Entity already carries the fields). Bake
  auto-emits.
- **Drag-corner-to-resize-pad** — corner Points on the pad outline
  become resize handles. Cursor changes per-edge (↕ for top/bottom,
  ↔ for left/right, ↗/↘ for corners).
- **Unified active bar** (Altium parity) — split-button submenus for
  Move / Filter / Selection / Align / Line-Arc / Track-Solid;
  primitive parity between Sketch and Pads modes (Line/Arc/Rectangle/
  Circle/Fill/Region available in both).
- **Full pad-stack Properties** — Pad Stack tabs, per-layer copper,
  hole + paste + solder mask, Testpoint, Pad Features. Empty-canvas
  Properties parity (Selection Filter / Snap Options checklist /
  Grid Manager / Guide Manager / Units).

### v0.16 (2026-05-04) — drag Points + Rounded Rectangle + pad-as-rectangle

Three deliverables on top of v0.15:

1. **Drag-to-move sketch Points** — Sketch mode + Select tool now
   hit-tests Point entities on left-press; a successful hit starts a
   drag that publishes `FootprintSketchMovePoint { id, dx, dy }` per
   CursorMoved tick (delta from `last_world` so each tick advances by
   exactly the cursor delta). Press → select + start drag, Move →
   per-tick move publish, Release → consume drag. Bidirectional pad
   moves piggyback on the existing `bake_pads` re-run inside
   `apply_sketch_edit_with_warnings`: when a Point with a `PadAttr`
   moves, the next solve regenerates the pad at the new position.

2. **Rounded Rectangle sketch tool** — new `SketchTool::RoundedRectangle`
   variant + `ToolPending::RoundedRectangleFirst { first }`. Two-click
   flow (corner + opposite corner) reads the corner radius from
   `dimension_input` (default 0.5 mm, clamped to
   `[0.05, min(half_w, half_h)]`). Commits 12 Points (4 arc centres +
   8 arc-end / line-end), 4 axis-aligned Lines (shortened by `r`), and
   4 Arcs (one per corner, sweep CCW). Live ghost preview in
   `draw_sketch_tool_preview` traces all 4 lines + 4 arcs against the
   cursor as the user moves before clicking the second corner. Active
   bar gains a tooltip-labelled `▢` glyph between Rectangle and
   Circle.

3. **Pad-as-editable-rectangle in Sketch mode** —
   `EditorPad.corner_entity_ids: Option<[SketchEntityId; 4]>` plus 4
   construction-flagged corner Points + 4 construction-flagged
   outline Lines per pad. `auto_mint_for_literal_pads` /
   `mirror_add_pad_to_sketch` mint them; `mirror_move_pad_in_sketch`
   reposition them on Pads-mode pad drags;
   `mirror_delete_pad_from_sketch` drops the corners + their connecting
   Lines together with the centre. Bake unaffected — `signex-bake`
   already skips construction entities. Pad outlines now appear as
   first-class primitives the user can pick / hover in Sketch mode;
   resizing-by-dragging-a-corner is queued for v0.16.1.

Test updates: `pad_to_sketch::tests` entity-count assertions adjusted
(per-pad: 1 centre + 4 corners + 4 lines = 9; pre-v0.16 tests assumed
1). All 9 module tests pass; full `cargo test -p signex-app` green
(110 lib + 3 integration, all green).

### v0.15 deferred to v0.15.1+ / v0.16+

- **Stock library** — bundle ~5-10 reference parametric `.snxfpt`
  fixtures (SOIC-8 / QFN-16 / QFP-32 / mounting hole / fiducial /
  USB-C edge). Hand-authored TOML; highly parallelizable with one
  agent per ~5 footprints.
- **Multi-footprint per `.snxfpt`** — change file format from a
  single `Footprint` to a `FootprintFile { footprints: Vec<...>,
  active_idx: usize }` envelope (mirrors `.snxsym` pattern).
  Required for the Footprint Library panel to read more than one
  footprint per file.
- **PCB outline editor** — Sketch-mode-like editor for the
  top-level PCB board outline.
- **Pour fill generation** + **DRC enforcement** for keepouts —
  actual rendering / DRC consumers of the v0.14.1 bake fields.

### v0.14.1 deferred to v0.14.2+ / v0.15+

- **Stock library (H)** — 5–10 reference parametric .snxfpt
  footprints. The bake pipeline is complete; what's missing is the
  hand-authored TOML with constraints + parameters. Highly
  parallelizable (one agent per ~10 footprints).
- **Lib field gaps in cutouts** — `FpCutout` doesn't carry edge
  radius or partial-depth flag. Schema bump v3 → v4.
- **Lib v-score richer fields** — `FpVScore` doesn't carry side
  (Top/Bottom/Both) or min-web. Schema bump.
- **Multi-plane Body3D stack-up** — currently only the first
  BodyTop plane contributes. Multi-body packages (e.g. crystal can
  + leads) need plane stack-up.
- **Circle bake** — Circle entities (closed primitive without
  start/end endpoints) with closed-profile attrs are skipped with a
  "v0.14.2" warning. Bake should sample the circle into N=ARC_SAMPLES*2
  vertices.
- **UX deferred items from v0.13.3** — unchanged.

### v0.16.1.1 (2026-05-04) — drag-corner-to-resize-pad + paused ghost hidden

Hotfix on top of v0.16.1:

- **Drag-corner-to-resize-pad** — `FootprintSketchMovePoint` now
  detects when the dragged Point is a member of any pad's
  `corner_entity_ids`. When so it recomputes the pad's bbox from the
  4 corner positions, rewrites `EditorPad.position_mm` +
  `EditorPad.size_mm`, repositions the centre Point to the new bbox
  centre, and updates `PadAttr.size_x_expr` / `size_y_expr` so the
  next bake emits the pad at the new size. Resize is one-axis when
  the dragged corner stays on its parallel edge; both-axis when the
  user drags it diagonally.
- **Ghost hidden while paused** — the previous "grey-but-still-
  following-cursor" ghost during `PadsTool::PlacePad` +
  `placement_paused` confused users (the cursor implied a target
  even though clicks were gated). v0.16.1.1 hides the ghost entirely
  while paused and renders a solid fill (alpha 1.0 + white stroke)
  while live for better contrast.

### v0.16.2 (2026-05-04) — role-assignment UI + role-tinted closed loops

Top-priority deferred item from v0.16.1 lands as a Fusion-style
inspector dropdown:

- **`RoleTag` enum** in `library/messages.rs` — 15 variants:
  Unassigned, Pad, SilkTop, SilkBottom, Courtyard, Keepout, Cutout,
  MaskOpeningTop, MaskOpeningBottom, MaskExcludeTop,
  MaskExcludeBottom, PourTop, PourBottom, PasteApertureTop,
  PasteApertureBottom. Wraps in a `RoleTag::ALL` static slice +
  `RoleTag::label()` for the inspector pick_list.
- **`PrimitiveEditorMsg::FootprintSketchSetRole { id, role }`** +
  matching `EditorMsg` variant + standalone-converter arm.
- **`apply_sketch_role` / `apply_sketch_role_with_warnings` /
  `set_entity_role` / `current_role_of`** in `sketch_dispatch.rs` —
  clears every `*Attr` slot on the target entity, sets the matching
  one with sensible defaults (Pad: 1×1mm SMD on TopCopper, designator
  = max-existing+1; Silk: TopSilk/BottomSilk; Courtyard: CourtyardAttr;
  Keepout: TopCopper + NO_ROUTING; Cutout: BoardCutoutAttr through=true;
  Pour: thermal-relief defaults; Mask Opening / Exclude: matching
  Top/Bottom solder mask; Paste: matching Top/Bottom paste). Pad role
  on a non-Point is a silent no-op.
- **Inspector "Role" section** (4th cell on the strip below the
  canvas) — pick_list dropdown bound to the selected entity's role.
  Visible "(select a sketch entity in Sketch mode)" placeholder when
  nothing is selected. Inline hint when a non-Point is selected
  ("Pad role applies to Points only").
- **Closed-loop fill colour by role** — `draw_filled_closed_loops`
  picks the loop's fill colour from the first role attr it finds in
  the loop's lines or points. SilkTop → `FpLayer::FSilks`; Courtyard
  / Keepout / Cutout → `FpLayer::EdgeCuts`; PourTop / Pad →
  `FpLayer::FCu`; PourBottom → `FpLayer::BCu`; etc. Loops with no
  role attr keep the v0.16.1 neutral grey fallback. Alpha bumped to
  0.20 (vs 0.10 grey) so role assignments are clearly visible.
- **Dispatcher arm** in `app/dispatch/library.rs` — calls
  `apply_sketch_role_with_warnings`, then diffs `state.pads` against
  the target entity's new pad attr (per-entity diff preserves
  existing `sketch_entity_id` + `corner_entity_ids` on auto-minted
  pads).
- **7 new dispatcher tests** in `sketch_dispatch_tests.rs`:
  - `set_role_pad_on_point_attaches_pad_attr_and_bakes`
  - `set_role_pad_on_line_is_silent_noop`
  - `set_role_silk_top_attaches_silk_attr_with_top_layer`
  - `set_role_unassigned_clears_every_attr`
  - `set_role_replaces_existing_attr_atomically` (Pad → Silk swap)
  - `set_role_courtyard_attaches_courtyard_attr`
  - `set_role_pad_increments_designator_across_entities`

Test counts: signex-app lib 116 → 123; full workspace test sweep
green (all 38 test-binary `test result: ok` lines, zero failures).
No clippy regressions.

**Why:** user testing of v0.16 made the role-assignment gap the #1
missing UX item — users could draw a closed silkscreen square but
the bake had no way to know it was silk vs courtyard vs cutout.
Option A from the v0.16 brainstorm (per-entity dropdown, not
per-loop) chosen so a single attr lives in one place and bake walks
out from it; matches Fusion's "select edge → assign feature" flow.

**Deferred to v0.16.3+:**
- Per-pad layer dropdown (top-side default fixed for now).
- Keepout sub-form (which kinds: routing/components/copper/vias/
  drilling/pours).
- Pour sub-form (net + fill type + thermal relief overrides).
- BoardCutout edge-radius input.
- V-score role tag (for Line entities — BomLayer mounts).

