# PCB ratsnest (#2192)

`pcb_geom::ratsnest(&PcbBoard) -> Vec<RatLine>`: the shortest unrouted
connections per net, as nm endpoints, sorted by `(net, from, to)`.

## Where it lives

In `pcb-geom`, not `signex-net`. `signex-net` derives schematic
connectivity from `signex-types` alone. The ratsnest needs the board's
copper shapes, the R-tree, `UnionFind` and the spanning trees, all of which
are `pcb-geom` (ADR-0005). A second copy of the pad transform in another
crate would be the "kept in step" pair ADR-0005 bans.

## Rules

- Copper joins when its shape overlaps (touching counts) other copper of
  the same net, on a shared copper layer. Pads, tracks and vias all join
  by shape: a track across a pad joins it, and two overlapping pads join,
  with no end or centre inside the other. A pad is its real outline
  as `pad_shape` gives it (round-rect, trapezoid and custom pads are their
  bounding rectangle); an oval
  ellipse is read as a 32-point polygon, so it is under-read by its
  sagitta (about 0.5% of the radius). Through-hole pads and through vias
  reach every layer; blind and micro vias the layers between the ones they
  name.
- Per net, each connected group of copper holding a pad is an island. Up to
  about 4 million pad pairs across the islands (counted in pairs, so a big
  mostly-routed net with a stray pad stays exact) the islands join at their
  closest pads (Kruskal, `spanning_forest`); beyond that each island is one
  representative pad and the join is `euclidean_mst`.
- Net 0 is no net. Pads with no net are ignored.
- Crossing tracks: two same-net tracks on a shared copper layer join
  when their capsules (centre segment plus half the width) overlap, so a
  cross mid-segment counts with no end on the other. Parallel tracks join
  only when the edges touch; a 1 nm gap does not.
- Zones: a same-net zone is copper on its one layer. A pad, track or via
  joins it when its shape overlaps the zone polygon (touching counts), and
  everything that joins one zone is one island. The polygon is the zone's
  OUTLINE, because the model stores no filled polygon (the fill is not
  computed or saved); it may be concave. The test is integer nm
  (`Shape::overlaps_polygon`); the only float is the 32-point ellipse
  outline above. A via joins a zone on any copper layer of its span that
  the zone is on; a through pad or via reaches every layer.
- Zone to zone: two same-net zones on a shared copper layer whose
  outlines meet (an edge crossing or touching, or one wholly inside the
  other) are one copper, found through an R-tree of outline boxes. Zones 1 nm
  apart do not join.
- Not modelled: the zone fill itself (#2216 tracks using the filled polygon,
  holes and clearance, once fills are computed). Until then zone holes and
  the clearance the fill keeps from other nets are ignored: a pad inside a
  zone hole counts as joined and gets no ratsnest line (a false negative),
  as does one the real fill would leave isolated; drill holes (a
  through-hole pad is solid copper, so a track through its hole counts as
  touching); and the clearance a track needs to a pad of another net (that
  is DRC).

## Cost

The app computes the lines in `sync_pcb_canvas_from_visible_board`, keyed
by `(tab path, pcb revision)`, so a camera move or a net toggle does not
recompute. `crates/pcb-geom/tests/ratsnest_corpus.rs` times a 10,000-track,
100-net board and asserts a loose bound (5 s in debug CI); release runs
print the figure with `--nocapture`.

## Showing and hiding

`PcbRatsnest` (app `InteractionState.pcb_ratsnest`) holds the hidden nets, per board path (net numbers are per board).
`Message::Pcb(PcbMsg::RatsnestNetVisibilityToggled(net))` toggles one;
there is no panel control yet (follow-up: the nets panel is Graphite-owned). Drawing is the existing overlay in
`signex-renderer` (`pcb/ratsnest.rs` converts lines to `RatsnestInput`;
`pcb/emit.rs` draws them in `ColorSlot::Ratsnest`).
