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
  as `pad_shape` gives it, turned by the footprint's rotation and the pad's
  own: an oval is a stadium along its longer side, a round-rect keeps its
  corner radius and is read exactly, as its rectangle inset by the radius
  plus that radius. Through-hole pads and through vias
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
- Zones: a same-net zone is copper on its one layer, and a pad, track or via
  joins it when its shape meets the zone's copper (touching counts). A
  zone's copper (#2216):
  - a polygon pour (`fill_type = "pour"`): its fill, from
    `pcb_geom::zone_fills` (see `docs/PCB_TOOLS.md`, "Polygon Pour"), with
    its holes and the clearance from other nets cut out. Each separate piece
    of the fill is its own copper, so pads on two pieces of one pour stay
    apart; a pour with nothing left to fill is no copper.
  - any other zone (a Fill or Solid Region): its outline less its usable
    holes (`Zone::usable_holes`); it may be concave.

  An area's holes are open: copper wholly inside a hole, touching none of
  its edges, does not meet the area, so a pad in a hole gets a ratsnest
  line. The tests are integer nm (`Shape::overlaps_polygon` and
  `area_touch`); the only float is the round-rect corner-arc outline above.
  A via joins a zone on any copper layer of its span that the zone is on; a
  through pad or via reaches every layer. `ratsnest` computes the fills
  itself; a caller that holds them passes them to `ratsnest_with_fills`.
- Zone to zone: two same-net zone areas on a shared copper layer that meet
  (an edge crossing or touching, or one wholly inside the other and not
  inside one of its holes) are one copper, found through an R-tree of
  outline boxes. Areas 1 nm apart do not join.
- A zone outline with no area (fewer than 3 distinct points, all collinear,
  or one point repeated) is a zero-width segment or a point, like a
  zero-width pad: it joins only what touches that segment or point, and
  never the plane around it. It never "holds" a corner of another zone or
  is held by one. An empty outline is no copper. A pour with such an
  outline has no fill.
- Not modelled: drill holes (a through-hole pad is solid copper, so a track
  through its hole counts as touching), and the clearance a track needs to a
  pad of another net (that is DRC).

## Cost

The app computes the pour fills and then the lines in
`sync_pcb_canvas_from_visible_board` (`PcbRatsnest::refresh`), keyed
by `(tab path, pcb revision)`, so a camera move or a net toggle does not
recompute; the canvas draws the same fills. `crates/pcb-geom/tests/ratsnest_corpus.rs` times a 10,000-track,
100-net board and asserts a loose bound (5 s in debug CI); release runs
print the figure with `--nocapture`.

## Showing and hiding

`PcbRatsnest` (app `InteractionState.pcb_ratsnest`) holds the hidden nets, per board path (net numbers are per board).
`Message::Pcb(PcbMsg::RatsnestNetVisibilityToggled(net))` toggles one;
there is no panel control yet (follow-up: the nets panel is Graphite-owned). Drawing is the existing overlay in
`signex-renderer` (`pcb/ratsnest.rs` converts lines to `RatsnestInput`;
`pcb/emit.rs` draws them in `ColorSlot::Ratsnest`).
