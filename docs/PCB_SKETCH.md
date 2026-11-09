# PCB sketch: board geometry from the parametric sketch (#2234)

A board can carry a sketch, the same parametric sketch the footprint editor
uses (`signex_types::sketch::SketchData`, solved by `signex-sketch`). Entity
attributes say what each part of it becomes on the board. The sketch is the
source of truth; the bake turns it into board geometry, and one board
command stores both.

This page covers the domain half (storage, bake, engine command, DRC). The
sketch-mode UI is a separate step of #2234.

## What a board sketch defines

| Entity attribute | Entity | Bakes to |
|---|---|---|
| `board_outline = true` | a loop of lines and arcs, or one circle | the board outline (`PcbBoard::outline`) |
| `keepout` (`KeepoutAttr`: layer, forbidden kinds) | a loop or a circle | a `PcbKeepout` |
| `board_cutout` (`BoardCutoutAttr`: edge radius, through) | a loop or a circle | a `PcbCutout`, an opening in the board |
| `mounting_hole` (`MountingHoleAttr`: drill, plated, net name) | a circle | a `PcbHole` at the circle's centre |
| `v_score` (`VScoreHintAttr`) | a line | a `PcbVScore` |

A loop needs only one tagged edge: the bake walks the closed profile through
it (`trace_closed_profile`), and edges tagged alike in one loop bake it once.
Arcs are tessellated with `ARC_SAMPLES` points, a circle with
`4 * ARC_SAMPLES` corners. Construction and centerline entities never bake
(`Entity::bake_skipped`), as in footprints.

A mounting hole's drill is its `drill_expr` (a length expression over the
sketch parameters), or the circle's diameter when unset. Its net is a net
**name**, turned into the board's net number at bake time, so renumbering the
net table does not detach it. A V-score keeps its depth as a fraction of the
board thickness, not in mm, so a thickness change needs no re-bake.

## The bake

`signex_bake::bake_board(&SketchData, &[NetDef]) -> Result<BakedBoard,
BoardBakeError>` is pure: it resolves the parameters, solves the sketch and
reads the solved positions. Every baked item takes the uuid of the entity it
came from (the first tagged edge of its loop, or the circle), so baking an
unchanged sketch again gives an equal result.

Unlike the footprint bake, which skips a broken feature with a warning, the
board bake refuses: a keepout or hole dropped without a word would pass DRC
unseen. `BoardBakeError` names the problem:

| Error | When |
|---|---|
| `Parameters` | the parameter table does not resolve (a cycle, a bad expression) |
| `Solve` | the sketch does not solve (`SolveError`: over-constrained, dangling reference, ...) |
| `OpenProfile` / `BranchingProfile` / `UntraceableProfile` | a tagged loop is not closed, branches, or cannot be walked; names the feature and the edge |
| `NotAnEdge` | a point carries an outline, keepout or cutout attribute |
| `MissingPoint` | a profile point has no solved position |
| `OutlineCount` | the outline tags more than one closed profile |
| `HoleNotACircle`, `HoleDrill`, `UnknownNet` | a hole on a non-circle, a drill not above 0, a net name the board lacks |
| `VScoreNotALine` | a V-score on an arc or circle |
| `Expression` | a cutout radius, drill or V-score depth that does not evaluate to a finite number |

**The outline is owned like a footprint's baked fields.** With an edge
tagged `board_outline`, the bake returns the profile. With none tagged, it
returns `None` (the board keeps its own outline), unless an earlier bake of
this sketch produced one (`baked_kinds` holds `board_outline`): then it
returns an empty outline, which clears the board's. `BoardSketch::from_bake`
records that token.

## The board command

`BoardCommand::SetBoardSketch { sketch, outline }` is the only way the sketch,
its keepouts, cutouts, holes and V-scores, or a sketch-owned outline change.
Build it with `BoardCommand::set_board_sketch(sketch, baked)`;
`BoardCommand::remove_board_sketch()` takes the sketch off and leaves the
outline as the last bake drew it, the board's own again.

- One undo step. Values move by `mem::replace`, so undo and redo restore the
  board bit for bit. Its inverse is another `SetBoardSketch`.
- Checked like every command (`check_batch`, all-or-nothing batches). A NaN
  or infinity anywhere in the sketch or its geometry is refused
  (`NonFiniteSketch`), as is a non-finite outline point.
- `SetBoardOutline` is refused (`OutlineOwnedBySketch`) while the sketch owns
  the outline.

The baked outline is written to `PcbBoard::outline`, so every reader of the
board area (the renderer, Update PCB staging, export) uses it with no change.

## In the `.snxpcb`

A new optional section, written only for a board that has a sketch. A board
without one writes no byte of it and re-saves byte-identical; a file written
before it loads unchanged. The format token stays `snxpcb/1.3`: a build that
predates the section keeps it as a foreign top-level key and writes it back.

```toml
[board_sketch.sketch]
# SketchData in the serde form `.snxfpt` uses: planes, entities,
# constraints, arrays, parameters, baked_kinds. Lengths in f64 mm.

[[board_sketch.keepouts]]
uuid = "..."
boundary = [{ x = 10.0, y = 10.0 }, ...]   # f64 mm, closing vertex not repeated
layers = ["TopCopper"]                     # board-layer tokens

[board_sketch.keepouts.forbids]
no_routing = true
...

[[board_sketch.cutouts]]                    # uuid, boundary, edge_radius (mm), through
[[board_sketch.v_scores]]                   # uuid, from, to, depth_fraction, min_web (mm), side

[board_sketch.holes]
content = """
uuid  pos_x  pos_y  drill  plated  net
...   4000000  4000000  3200000  true  7
"""
```

- `keepouts`, `cutouts`, `v_scores` and `holes` are left out when empty.
- The holes are a TSV block like `[vias]`: positions and drill in integer nm,
  `plated` is `true` or `false`, `net` is a net number or empty. Unknown
  columns are kept per row.
- Polygons are TOML tables in f64 mm, like `[[zones]]` and the outline in
  `[extras.board]`. The baked outline stays in `[extras.board] outline`.
- Foreign keys inside `[board_sketch]` are kept (`BoardUnknown::board_sketch`).
- A non-finite number fails the save, naming its key path.

## DRC

`run_drc` reports a `Keepout` violation for each item inside a keepout that
forbids it; see `docs/architecture/pcb-drc.md`, "Keepouts".

## Not done yet

- The sketch-mode UI (enter and leave, Active Bar sketch tools, DOF row,
  snap glyphs), and calling `bake_board` + `SetBoardSketch` on commit.
- Drawing keepouts, cutouts, holes and V-scores on the board, and exporting
  cutouts, holes and V-scores (Gerber edge cuts, drill file).
- DRC: `no_pours` keepouts, holes in `no_drilling` keepouts, and
  copper-to-edge and copper-to-cutout clearance (the rules have no edge
  clearance yet).
- The ratsnest does not read plated holes: a hole with a net is not yet
  copper of that net. It reads no board area today, so the outline does not
  affect it.
- A corpus golden for a board with a sketch (`SIGNEX_BLESS_CORPUS=1`).
