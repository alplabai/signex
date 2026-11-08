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

- Copper joins when a track end (within the track's half width), a via
  centre or a pad centre lies on other copper of the same net, on a shared
  copper layer. Through-hole pads and through vias reach every layer;
  blind and micro vias the layers between the ones they name.
- Per net, each connected group of copper holding a pad is an island. Up to
  256 pads the islands join at their closest pads (Kruskal,
  `spanning_forest`); beyond that each island is one representative pad and
  the join is `euclidean_mst`.
- Net 0 is no net. Pads with no net are ignored.
- Not modelled yet: zone fills (a pad inside a same-net pour still gets a
  line) and two tracks that cross without an end on the other.

## Cost

The app computes the lines in `sync_pcb_canvas_from_visible_board`, keyed
by `(tab path, pcb revision)`, so a camera move or a net toggle does not
recompute. `crates/pcb-geom/tests/ratsnest_corpus.rs` times a 10,000-track,
100-net board and asserts a loose bound (5 s in debug CI); release runs
print the figure with `--nocapture`.

## Showing and hiding

`PcbRatsnest` (app `InteractionState.pcb_ratsnest`) holds the hidden nets.
`Message::Pcb(PcbMsg::RatsnestNetVisibilityToggled(net))` toggles one;
there is no panel control yet. Drawing is the existing overlay in
`signex-renderer` (`pcb/ratsnest.rs` converts lines to `RatsnestInput`;
`pcb/emit.rs` draws them in `ColorSlot::Ratsnest`).
