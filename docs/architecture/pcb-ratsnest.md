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
- Not modelled: zone fills (a pad inside a same-net pour still gets a
  line); drill holes (a through-hole pad is solid copper, so a track
  through its hole counts as touching); and the clearance a track needs
  to a pad of another net (that is DRC). Two tracks that cross, or a track
  that crosses a via, do join: they are copper overlapping copper.

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
