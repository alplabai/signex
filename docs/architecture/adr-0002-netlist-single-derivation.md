# ADR-0002 — Netlist increment 2: one derivation, a complete contract, wired consumers

- **Status:** Accepted (2026-07-08) — D8 amended with the approved cross-sheet stitching design after a three-lens design round with adversarial risk review
- **Scope:** `signex-net`, `signex-erc`, `signex-types::net`, the UI net-flood, PCB net assignment, netlist export
- **Extends / amends:** ADR-0001 §A3.1 (the Netlist contract) and §D7 (crate DAG)
- **Trigger:** PR #137 landed increment 1 (the `Netlist` contract type + `signex_net::build_netlist`). This ADR records the decisions increment 2 needs — found by an adversarially-verified review of #137 (2026-07-07; evidence in the appendix).
- **Tracking:** D3+D5 → #157 · D2+D4 → #158 · D7 (app wiring) → #159 · D8 (cross-sheet, inc2c) → #156. Landed since first draft: net-flood consumer (#138, D7.2), same-name label merge incl. interior anchoring (#154, part of D5.4).

## Context

\#137 did what it set out to do: the contract types (`Netlist`, `Net`, `Terminal`) live in `signex-types`, `build_netlist` is deterministic and unit-tested, and the raw union-find primitive is now shared (`signex_net::uf`, HI-17). But until consumers are wired, the *duplication surface got bigger, not smaller*:

- **Four** independent connectivity derivations now exist: `signex-net::build_netlist`, `signex-erc::context::derive_nets`, the inline topology pass in `signex-erc::rules::missing_power_flag`, and the hand-rolled flood in `app/handlers/canvas.rs`. The first two agree only via copy-paste plus "kept in step" comments — no shared fixture or cross-check pins them together.
- **Three** copies of `SymbolTransform` exist (`signex-types` canonical, private copies in `signex-erc` and `signex-net`). The two private copies are character-identical to the canonical one and could be replaced today — exactly the divergence HI-19 was written to prevent.
- The contract as shaped **cannot feed the UI net-flood** — the one leak §A3.1 explicitly names. The flood colors wires and junctions by `Uuid`; `Net` carries only `Terminal { reference, pin }` strings, and `build_netlist` discards the parent map that membership would come from.
- The review confirmed boundary-condition bugs shared by both domain derivations (junctions missing from the connectivity gate, phantom one-terminal bus nets, mixed distance metrics, mid-span labels ignored for naming, no net-name uniqueness) — details in the appendix.

## Decisions

### D1. `signex-net` is the home of connectivity — amend ADR-0001 A3.1(a)

A3.1(a) said the producer lives in `signex-engine`. We keep it in `signex-net` instead and amend the ADR rather than move the code. Rationale: `signex-engine` is the *mutation* engine (commands, patches, undo history); connectivity is a *pure derivation* over the document. A leaf L1 crate lets `signex-erc` (and later the app, output, PCB) depend on connectivity without dragging in command/history machinery. The `signex-erc → signex-net` edge is added to D7's reviewed list as legitimate layering (rule engine on connectivity).

### D2. One derivation — everything else is a view

`signex-net` exposes the union-find result as a first-class value (working name `Connectivity`: point-key → net index, plus per-net wire/junction/pin membership). `build_netlist` becomes a projection of it, and every other consumer reads it instead of rebuilding it:

- `ErcContext` consumes `Connectivity`; `derive_nets` is **deleted**.
- `missing_power_flag` reads the same topology instead of running its own union pass (its junction loop today calls `find` without `union` — the exact pre-#107 bug pattern `derive_nets` documents fixing).
- The canvas net-flood reads net→wire/junction membership (D7 below).

A "kept in step" comment is not a synchronization mechanism; shared code is.

### D3. Complete the contract before wiring consumers

Three gaps must close first, because consumers shape the types:

1. **Membership.** `Net` gains wire/junction membership (`wires: Vec<Uuid>`, `junctions: Vec<Uuid>` — or a `Netlist`-level membership table). Required by the net-flood and the future ratsnest.
2. **Terminal identity.** `Terminal` gains `symbol: Uuid`. A bare reference-designator string collapses unannotated (`R?`) and duplicate refdes; the `Uuid` links a terminal back to the placed symbol. `reference`/`pin` stay for exporters and display.
3. **Typed class, no heuristic.** `Net.class` becomes `Option<NetClassId>` (the newtype already exists) and the name-prefix heuristic moves out of the builder — class resolution is a project-rules concern layered on top of connectivity, not part of deriving it.

### D4. One `SymbolTransform`

`signex-erc` and `signex-net` use `signex_types::schematic::SymbolTransform` (verified character-identical math). The two private copies are deleted; rotation/mirror unit tests live at the canonical site, which today has none.

### D5. Correctness fixes ship with the unification

Each with a regression test, fixed once in the single derivation:

1. **Junctions join the connectivity gate.** `point_is_connected` omits junctions, so a pin tip on a junction mid-wire yields no `Terminal` *and* a spurious ERC unconnected-pin warning. (Pin-to-pin contact stays "not connected" — that is a deliberate, ERC-consistent policy; record it in the rustdoc.)
2. **Bus policy.** Buses currently gate pin connectivity but are never unioned, producing one-terminal phantom nets. Either union bus segments or remove buses from the gate — not the current asymmetry.
3. **One distance metric.** The 1 µm integer bucket is the single definition of "same point"; the float `EPS` check may pre-filter but must not disagree with bucketing on any decision path. Decide the diagonal-wire tolerance (exact integer collinearity fails after quantization on non-axis-aligned wires): widen to a ±1-bucket band or reject with an ERC diagnostic.
4. **Label semantics.** Mid-span labels silently fail to name their net today. Either attach them (union at the projected point) or keep endpoint-only attachment — but document and test the choice. Enforce net-name uniqueness (dedup suffix or ERC error); auto-names `N$k` must not collide with user labels.

### D6. Agreement is tested, not commented

A shared fixture corpus lives in `signex-net` (`tests/fixtures/`) covering: T-junction, X-crossing (no junction), rotated/mirrored symbols, buses, power ports, multi-unit symbols, same-name labels on disjoint nets, mid-span labels, pin-on-junction. While `derive_nets` still exists, a cross-check test asserts both derivations produce identical net membership on every fixture; after D2 deletes it, the corpus remains as the single regression net. (Today: zero cross-checks, and rotation/mirror projection is untested everywhere.)

### D7. Consumers wire in dependency order

1. **`ErcContext`** (D2) — proves the derivation on the richest existing consumer.
2. **Canvas net-flood** — replaces the hand-rolled flood in `app/handlers/canvas.rs` (10 µm bucket, no interior T-merge, recursive `find`). This *changes flood behavior* on T-junction and near-coincident-point topologies; that is the intended A3.1 outcome — note it in the changelog.
3. **PCB net assignment** — `Netlist → Pcb.nets: Vec<NetDef>` (`id → number`, `name → name`); today nothing populates `NetDef`. Class needs a landing field or a `NetClass` lookup on the PCB side (D3.3). The future ratsnest is a GPU-instanced draw over this same data — another reason membership (D3.1) is plain tables of ids, not app types.
4. **Netlist exporter** — future emitters read `Netlist` through `ExportContext`; the byte-blob-only shape is retired.

**Freshness — the netlist is derived state, held like any other Model state (MVU).** The `Netlist`/`Connectivity` for a document is computed in `update` and cached in the Model, invalidated by the same `DocumentPatch` bits that already drive dirty-tracking (`WIRES | JUNCTIONS | LABELS | SYMBOLS`), exactly like the existing `canvas_cache` idiom. It is **never** computed inside `view` or a canvas `draw` — a derivation over every wire on the sheet has no place on the render path. Consumers read the cached value; a stale-read is impossible because the only writer is `update`, which is also the only place patches land.

### D8. Multi-sheet follows the ERC precedent — amended with the approved stitching design (2026-07-08, issue #156)

*Amendment note: the first draft deferred stitching semantics to a later increment and returned a bare `Netlist`. The design round settled the semantics, so inc2c ships them; the return type is `ProjectNetlist` (netlist + in-band issues).*

```rust
pub struct ProjectNetlist { pub netlist: Netlist, pub issues: Vec<StitchIssue> }

pub fn build_project_netlist(
    root: &SchematicSheet,
    children: &HashMap<String, SchematicSheet>,   // keyed by the EXACT ChildSheet.filename
    root_filename: Option<&str>,                  // enables cycle-through-root detection
) -> ProjectNetlist
```

**Decided semantics (full spec in #156):**

- **Two-level union-find.** Level 1 per sheet *occurrence* = today's per-sheet derivation plus `SheetPin` anchoring; **all level-1 unions complete before roots are sampled** into level-2 nodes. Level 2 merges across occurrences by exactly three rules: (1) Global/Power labels join by bare name project-wide; (2) power-port *symbols* (`is_power && !value.is_empty()`) are implicit global name carriers — matching the ERC rule that already reads them; (3) a named `SheetPin` binds to child labels of `text == pin.name` with `label_type ∈ {Hierarchical, Global}` — the codebase's own port model (the engine auto-generates pins from child Global labels; ERC accepts both). Local `Net` labels never cross sheets.
- **Multi-instance = per-occurrence expansion.** One child file instantiated N times yields N independent occurrence analyses — instances are never aliased onto one copy (that would short them together). Colliding refdes across occurrences are surfaced as `SharedReferenceAcrossInstances` until per-instance annotation exists.
- **Issues are in-band data, not errors** (`MissingChild`, `SheetCycle`, `DuplicateSheetUuid`, `SharedReferenceAcrossInstances`, `NameCollision`): the netlist is always produced, deterministically; degradation is reported, never silent.
- **Identity & cycles:** sheet identity is the children-map key (the filename string) — never the sheet uuid (copy-as-template duplicates it; detected as `DuplicateSheetUuid`). Cycle = key already on the DFS stack or equal to `root_filename`.
- **Naming:** name selection reuses `best_label_name` verbatim (priority and document-order tie-break); Global/Power names always bare; Hierarchical/Net names chosen in a child occurrence are qualified by the `ChildSheet.name` chain, root names stay bare; `class` derives from the *bare* label text before qualification; `N$k` scheme unchanged; collisions get a deterministic suffix + `NameCollision`.
- **Equivalence gate:** `build_project_netlist(root, &{}, None).netlist == build_netlist(root)` byte-for-byte, achieved by construction — both entry points call the same extracted per-sheet helpers (no parallel pipeline, per D2).

The app's ERC handler already assembles the project sheet map (parsing unopened sheets from disk); hoisting it into a shared helper — fixing its basename-collision keying — is the app-wiring increment (#159), so ERC and netlist read one project view.

## Consequences and deferred root cause

**The 1 µm bucket (D5.3) is a mitigation, not the fix.** Every metric ambiguity in the appendix (float `EPS` vs integer bucket, diagonal-wire collinearity after quantization) exists because the in-memory schematic model stores `f64` millimetres while the on-disk format and `signex-types::coord` already speak `i64` nanometres. The reference EDA discipline is the inverse of what we do today: integer-nm coordinates *internally*, floats only at the render boundary — under which "same point" is exact equality and D5.3 dissolves. Migrating `schematic::Point` to integer nm is deliberately **out of scope** here (it touches every crate) but is recorded as the candidate subject of a future coordinate ADR; D5.3's single-metric rule keeps increment 2 correct until then.

## Anti-patterns (extends ADR-0001 Part C)

- **No new connectivity derivations.** Any `union`/`find` over schematic geometry outside `signex-net` is a regression. Add a CI grep-guard (the D5 mechanism from ADR-0001) once the canvas flood is migrated.
- **No "kept in step" comments as a substitute for shared code.** If two functions must agree, they must be one function — or be pinned by a shared fixture test.
- **No consumer re-parses wires to rebuild adjacency.** Consumers read `Connectivity`/`Netlist`; if a consumer needs data the contract lacks, the contract grows (D3), the consumer does not fork the derivation.

## Appendix — review evidence (2026-07-07, adversarially verified)

| # | Finding | Where |
|---|---------|-------|
| 1 | `point_is_connected` omits junctions → pin on junction mid-wire loses its terminal + false ERC unconnected-pin | `signex-net/src/build.rs:91`, same in `signex-erc/src/context.rs:412` |
| 2 | Buses gate connectivity but are never unioned → one-terminal phantom nets | `signex-net/src/build.rs:96` |
| 3 | Float `EPS` (0.1 µm) vs 1 µm bucket disagree near half-µm boundaries; pin path depends on both | `signex-net/src/build.rs:62–72` |
| 4 | Junction on a diagonal wire interior can fail exact integer collinearity after quantization | `signex-net/src/build.rs:77` |
| 5 | Mid-span label never names its net (auto `N$x` despite visible label); untested | `signex-net/src/build.rs:144–162` |
| 6 | No net-name uniqueness; user label can collide with auto `N$k` | `signex-net/src/build.rs:209` |
| 7 | Four connectivity derivations, three `SymbolTransform` copies (was 3 / 2 before #137) | `build.rs:26`, `context.rs:437`, `rules.rs:459`, `handlers/canvas.rs:509` |
| 8 | ERC depends on `signex-net` only for `uf`; keeps full parallel `derive_nets` | `signex-erc/src/context.rs:433` |
| 9 | `Netlist` lacks wire/junction membership the net-flood needs; parent map discarded | `signex-types/src/net.rs:52` |
| 10 | UI flood: 10 µm bucket, no interior T-merge, recursive `find` (HI-17 pattern) | `app/handlers/canvas.rs:509–563` |
| 11 | `Net.class` untyped `String` from a name-prefix heuristic; `NetClassId`/`NetClass` unused; no PCB landing field | `signex-net/src/build.rs:122` |
| 12 | Zero production callers of `build_netlist`; ratsnest / `NetDef` assignment / exporter body absent | `signex-net/src/lib.rs:15` |

## References

- ADR-0001 — App structure, MVU, and domain boundaries (§A3.1 amended by D1 here; §D7 crate DAG extended)
- Iced 0.14 — The Elm Architecture, `Task`, canvas cache invalidation: <https://book.iced.rs> / <https://docs.iced.rs>
- Iced `shader` widget (custom wgpu pipelines run inside iced's own wgpu context — relevant to the future GPU ratsnest and to issue #100's version-alignment constraint): <https://docs.iced.rs/iced/widget/shader/index.html>
