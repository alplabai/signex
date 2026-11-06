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

`derive_at` also answers where queried points sit. The full derive anchors a
point flagged as on a wire's interior in the union-find that already holds the
name joins, one point after the other, and then looks the net up by the root
the nets were numbered with before those joins. Normally the point is a fresh
node and nothing changes; but a point that is already a node of another net
(a pin tip held by a no-connect, inside a wire with no dot) merges two whole
classes, for the later queries too, and a net numbered by a root that the merge
changed is not found. `NetBase::locate` replays exactly that over the class
roots, not over the clusters alone (the first version anchored in the
cluster-level union-find and missed the merge; found by the random-edit test).
The region argument above is unaffected: every queried key is a dirty key, so
the wires through it are in the region.

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

## What it does not remove

The guard still turns each derived list into sets to compare, and level two
walks every cluster, so a step is not free of the sheet size; it is free of the
geometry (the union-find, the segment index, the pin transforms, the terminal
gate) outside the region. The junction reconcile is separate work.

## Tests

- `signex-net` `build::tests::incremental`: targeted edits and a random-edit
  property test against `net_signatures_at`, queried points included.
- `signex-engine` `incremental_nets_tests`: every derive inside the guard is
  asserted equal to the full derive (`guard::derive_at`, tests only), ghost =
  commit = full-derive ghost over targeted moves and random cases, and an
  ignored timing bench.
