# Tolerant ear clipper: clip order

`signex-sketch::geom::tolerant_clip` triangulates the ring that `holes` builds
by bridging cut-outs into the outline (#1006).

## Why the order matters

Clipping the first clippable vertex in ring order turns a long thin strip of
cut-outs into fans of long slivers. Each sliver is tested against the reflex
vertices under its triangle, and a sliver spans most of the strip, so every
ear costs O(n) and the clip is O(n^2). A cheaper query (box, then triangle,
then a cached blocker, #1032) only lowered the constant: a thin ear that is
not blocked still has to look at every vertex along its long edge.

## What it does now

- Convex vertices wait in a min-heap keyed by the squared length of the
  longest side of their ear. Small ears go first, so the many short ears
  around each cut-out are clipped before any long ear exists, and a long
  ear's triangle is tested only when little of the ring is left near it.
- An entry carries the vertex's version. A vertex's version changes when its
  neighbours change, so a stale entry is dropped on pop (lazy update).
- A popped ear is tested for a blocker. A blocked ear is parked under its
  blocker and re-queued only when that vertex (or the ear's own corner)
  changes, instead of being re-tested on every pass.
- If the heap runs dry while ears are parked, every convex vertex is queued
  afresh once; if all of them are blocked again, the old forced cut runs.

The ear order differs from the former ring-order scan, so the exact triangle
set can differ. The result is still a partition of the ring; the tests check
the exact area and coverage rather than a fixed triangle list, and the cost
test counts blocking-test candidates per ring vertex on 500 and 2000 hole
strips (it must stay flat, a quadratic clip grows 4x).

## Safety net

Clipping small ears first can take an ear that lies outside the region when
the ring has two coincident antiparallel edges (cut-outs that share a side).
A vertex at an ear corner blocks through its edges whatever its convexity,
but that pair is not caught by it. The triangles' signed areas are therefore
compared with the ring's area, and a mismatch reruns the clip in ring order,
the former scan, which peels such a pair off first. Strips and grids of
cut-outs that merely touch never take that path.
