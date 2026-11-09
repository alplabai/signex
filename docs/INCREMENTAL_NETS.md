# Incremental net derive (#2077)

A drag step runs the engine's move pass on the pre-move sheet, and the net
guard inside it derives nets for the plain-moved sheet, for each candidate
round and, with a kept power port, for a reference sheet. `signex_net::NetBase`
makes each of those cost what the move changed, not the size of the sheet.
The answer is `net_signatures` / `net_signatures_at` exactly; the tests hold
every derive inside a ghost to the full derive.

## Two levels

The full derive is one union-find over wire ends, dots, labels, pins and ports.
`NetBase` splits it where the geometry stops and the names start:

1. **Physical clusters** (`incremental/level1.rs`): wires and dots, labels and
   child-sheet pins anchored to the wire they sit on, with the component pins
   (those with something on the tip), labels and power ports that land on each
   cluster. A cluster only depends on elements at its own points.
2. **Nets** (`incremental/merge.rs`): clusters joined by name, with the
   netlist's rules (`Net` / `Global` / `Power` labels of one text; `Global` /
   `Power` labels with power ports of one value), then named by the best label
   or port value. This runs over every cluster on each derive: it costs the
   number of clusters, not the geometry.

`NetBase::new` runs level one over the whole pre-move sheet and keeps the
clusters by union-find root, the node to root map, the elements by the root of
each point they sit on, and indexes of wire segments and of the points a wire
can pass through.

## One derive

1. Diff the sheet against the base by element id: wires, dots, labels,
   no-connects, symbols (position, rotation, mirror, unit, body, library id,
   power value), child-sheet pins. A match that differs counts as removed and
   added.
2. The *dirty keys* are the points of everything removed (where it was) and
   added (where it is): wire ends, dot / label / no-connect / pin positions,
   every pin tip of a symbol (connected or not), and the queried points.
3. The *region* is the set of base clusters those keys reach: the cluster of
   each key, the cluster of every wire passing through a key, and, for every
   wire added or dropped, the cluster of each label, dot or child-sheet pin
   lying on it (taken through its own key the same way).
4. Level one is rerun on a small sheet made of the unchanged elements in the
   region's clusters plus everything added. A symbol that lies only partly in
   the region reports just the pins in it.
5. Level two runs over the base clusters outside the region and the rebuilt
   ones.

Why the region is enough: elements interact only by sharing a point, by a
label, dot or child-sheet pin lying inside a wire (anchoring, T-merges, and the
choice among several wires under one point), by something on a pin tip
(connected or not), and by power ports on one tip. A change can alter only the
clusters whose points it touches; a point inside a wire is covered by taking
the wires through every dirty key and the items under every dirty wire. Name
joins are not physical, so they are not part of the region: level two redoes
them for all clusters.

## Queried points

`derive_at` also answers where queried points sit. A point flagged as on a
wire's interior that lies inside a wire (not on an endpoint) takes the net of
that wire's smaller endpoint; any other point takes its own node's net.
Nothing is joined, so a point that is already a node of another net (a pin tip
held by a no-connect, a power port's tip) never merges two nets, and no query
changes another one's answer (#2144). `NetBase::locate` answers the same way
through the region's own segment index: every queried key is a dirty key, so
every wire through it is in the region.

## When it derives in full

`NetBase::derive` returns the full derive's answer for any sheet, and runs the
full derive itself when:

- the sheet's library symbols are not the base's (`LibSymbolTable` storage
  differs): pin positions could differ for any symbol; or
- the base has two elements of one kind with one id: the diff cannot tell them
  apart.

A sheet with a repeated id that is *not* in the base is handled: the second
copy counts as added.

## Where it is used

The guard (`transform/rubber_band/guard.rs`) takes an optional `NetBase`
(`Guard::with_base`). `MovePreview` builds one per drag and gives it to the
throwaway engine. The commit path has no base and keeps the full derive, so the
ghost-equals-commit tests compare the two derives end to end.

## Junction dots

Each round of the move pass also reconciles junction dots: it mints the dots
the touched wires need and drops every minted dot no wire meeting justifies
any more. Whether a dot is justified reads only the wires within the junction
tolerance of its point. `MovePreview` therefore judges every minted dot of the
pre-move sheet once (`transform/dot_verdicts.rs`); a step diffs the wires by
id and judges again only the dots that moved, are new, or lie inside the
bounding box, grown by twice the tolerance, of a wire that was added, removed
or changed (old and new geometry). Every other dot keeps its pre-move verdict.
A sheet with two wires of one id, or a changed wire with a non-finite
coordinate, is judged in full.

The wire index behind those checks grids every wire of the sheet. With only a
few lookups to answer (a single dragged part, few dots to judge) it hands
every wire to each lookup instead (`WireIndex::for_lookups`): the answers are
the same, as every predicate still runs on its candidates, and the grid is
not built. This also speeds up the commit.

### Reconcile margin

Every command that changes wire geometry (the drop of a move, delete, rotate,
mirror, place wire, break wire) reconciles the dots against only the wires
whose bounding box meets a touched wire's box or a judged dot, each grown by
`RECONCILE_MARGIN_MM` (`transform/region_wires.rs`), five junction
tolerances (0.05 mm): two of reach plus three of headroom. That gives the same answers as every wire, because no test reads
a wire farther away:

1. Each test reads a wire within one tolerance of its query point, per axis.
   The endpoint test is `|d| < tol` on both axes; the interior test needs a
   perpendicular distance of at most `tol` with the foot inside the segment;
   the key-space test (`junction_is_honoured`) holds only within about 1 µm
   of the segment, as `pt_key` rounds to 1 µm.
2. Each query point is one of: a touched wire's end (inside its box); another
   wire's end lying on a touched wire's interior (within `tol` of it, and
   that wire's own box holds the point); a judged dot; a dot just minted,
   which is one of the first two.
3. So a wire any test reads has its box within `2 * tol` of a touched wire's
   box, or within `tol` of a judged dot. The other three tolerances are
   headroom; the float rounding of the grown boxes is many orders below `tol`.

The order of the wires is kept, so candidate lists, and with them the order
in which dots are minted, are the same. `commit_local_tests.rs` holds every
such command to the full scan on random, near-tolerance and corpus sheets.

## What it does not remove

The guard still turns each derived list into sets to compare, level two walks
every cluster, and the diffs read every element id, so a step is not free of
the sheet size; it is free of the geometry (the union-find, the segment index,
the pin transforms, the terminal gate, the junction checks) outside what the
move touches. With the bench below, one step on 480 symbols and 960 wires
takes about 4.8 ms, against 13.0 ms before the incremental derive (the
from-scratch path the commit takes now runs in about 8.3 ms).

## Tests

- `signex-net` `build::tests::incremental`: targeted edits and a random-edit
  property test against `net_signatures_at`, queried points included.
- `signex-engine` `transform::dot_verdicts::tests`: every kept verdict equals
  judging the dot again after random wire edits, and the scanning index
  judges as the gridded one.
- `signex-engine` `incremental_nets_tests`: every derive inside the guard is
  asserted equal to the full derive (`guard::derive_at`, tests only), ghost =
  commit = full-derive ghost over targeted moves and random cases, and an
  ignored timing bench.
