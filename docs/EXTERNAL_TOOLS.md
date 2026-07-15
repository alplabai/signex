# Signex — External Tools

> **Status:** Living document. Updated when a choice changes.
> **Audience:** Anyone about to build, propose, or vendor a dependency in
> one of the domains below.
> **Owns:** what we build on. For each domain Signex depends on, this file
> names the tool we chose, why, and how it is reached.
> **Does not own:** version numbers (`docs/ROADMAP.md` owns the version
> axis) or feature semantics (the internal plans own those). The roadmap
> bands quoted here are pointers, not commitments — if this file and
> `docs/ROADMAP.md` disagree about a version, the roadmap is right.

---

## 1. The Rule

**Before proposing or building an alternative stack in one of these
domains, open an issue.** If our choice is wrong we want to hear it —
that's a conversation, not a PR.

This is not a request for permission to contribute. It is a request to
spend thirty seconds before spending a weekend.

## 2. Why This File Exists

Signex's planning documents live in a private submodule at
`docs/internal/`. That means the stack decisions in this section were
made, written down, and then made invisible to everyone outside the
repository. We never published them, and we never told anyone we hadn't.

The cost landed on a contributor. PR #304 proposed a ~13k-line
transmission-line/Smith-chart implementation that hand-rolled complex
arithmetic, hand-rolled a Touchstone parser, and pulled `boa_engine` — a
complete JavaScript interpreter — into `build.rs` to render formulas. All
three problems were already solved by tools we had chosen: scikit-rf for
the RF math and Touchstone, typst for the formulas. None of that was
knowable from the public repository. The same contributor opened #233
(Topola for routing, where we have our own router plan) and #303
(IPC-2221 calculator) against the same blind spot.

That was our failure, not theirs. Reviewing work that could not have been
aimed correctly is not a code-quality problem; it is a documentation
problem, and this file is the fix. It is tracked by issue #306.

---

## 3. Domain → Chosen Tool

| Domain | Tool | Licence | How it is reached | Roadmap band |
|--------|------|---------|-------------------|--------------|
| Circuit simulation | **ngspice** | Mixed — Berkeley SPICE3 lineage plus other terms; consult upstream `COPYING` | FFI against `libngspice` | v4.x |
| EM / S-parameter extraction | **OpenEMS** + **CSXCAD** | GPL-3.0 / LGPL-3.0 | **Subprocess bridge** — see §4 | v4.x |
| FEM / thermal | **Elmer** + **GMSH** | Mixed / GPL-family; consult upstream | **Subprocess bridge** — see §4 | v4.x |
| RF math — Touchstone, S-params, Smith chart | **scikit-rf** | BSD-3-Clause | Embedded CPython via `pyo3`, in the Design Notebook compute layer | v1.4 |
| Control / systems math | **python-control** | BSD-3-Clause | Same compute layer | v1.4 |
| Formula + document rendering | **typst** | Apache-2.0 | Rust crate, linked | v1.4 |
| 3D kernel / STEP import | **OpenCascade** + **truck-modeling** | LGPL / Apache-2.0 | OpenCascade via a `step-to-gltf` conversion step; `truck-modeling` linked | v2.3 |
| Router geometry | **rstar**, **spade**, **kurbo**, **parry2d** | MIT / Apache-2.0 | Cargo dependencies of `pcb-geom` | v2.1 |
| 2D constraint solving | **our own Newton-LM solver** (`crates/signex-sketch`) | Apache-2.0 | In-tree | shipped (v0.13.0) |

### Notes per domain

**Circuit simulation — ngspice.** The mature open SPICE engine, reached
through `libngspice`'s C API rather than by shelling out, because
interactive simulation needs incremental control of the run. We maintain a
fork carrying fixes and performance work; **it is not public yet**, so
there is nothing to link here. Work against
[upstream ngspice](https://ngspice.sourceforge.io/) and we will reconcile.
We deliberately do not state an SPDX identifier for ngspice: its licensing
is a mix rooted in the Berkeley SPICE3 lineage. Read upstream `COPYING`
before you assume anything about it.

**EM / S-parameters — OpenEMS + CSXCAD.** OpenEMS is an EC-FDTD solver;
CSXCAD is the geometry library it consumes. Signex writes a CSX geometry
description, runs the solver as a separate process, and reads HDF5 results
back. This is the path for extracting S-parameters from real PCB
geometry — trace, via, and region blocks. It is the reason a hand-rolled
Touchstone parser is not needed: the numbers come from the solver and the
post-processing comes from scikit-rf.

**FEM / thermal — Elmer + GMSH.** GMSH meshes the geometry, Elmer solves
it, Signex reads VTK back. Same subprocess shape as OpenEMS. GMSH is used
straight from upstream — no fork.

**RF math — scikit-rf.** Touchstone parsing, network algebra, cascading,
de-embedding, Smith charts, TDR. This is a mature, well-tested, BSD-3
library, and reimplementing it in Rust is a multi-year project that ends
with a worse version of it. It is reached through the embedded CPython
runtime (`pyo3`) that the Design Notebook compute layer already carries
for NumPy / SciPy / Matplotlib — so it costs us no new runtime, no new
build dependency, and no new FFI surface.

**Control / systems math — python-control.** Same runtime, same
reasoning. Together with scikit-rf it also covers the legacy-MATLAB use
cases without a second interpreter: both are deliberate MATLAB-API
clones.

**Formula + document rendering — typst.** Rust-native, real math
typesetting, compiles directly to PDF, faster than LaTeX, and already
proven in our own tooling. It is a normal Cargo dependency. See §6 for
why the JavaScript-based alternatives are not on the table.

**3D kernel — OpenCascade + truck-modeling.** OpenCascade reads STEP and
tessellates; `truck-modeling` is the Rust-side geometry. OpenCascade is
LGPL and is kept at arm's length as a conversion step producing cached
glTF, not as a hard link inside the editor binary. Blender, where used for
high-quality offline rendering, is likewise GPL and is invoked as a
subprocess — we do not link `libbpy`.

**Router geometry — rstar, spade, kurbo, parry2d.** R-tree spatial index,
constrained Delaunay, curve/arc operations, collision primitives.
Permissive, maintained, and boring, which is what a router's foundation
should be. The router itself is ours, written clean-room. See #233: we
read the Topola proposal and the answer is that routing is the feature we
are least willing to outsource — it is the core of the product's
differentiation and it is planned in detail.

**2D constraint solving — ours.** `crates/signex-sketch` implements a
Newton–Levenberg-Marquardt constraint solver, written from textbook and
published-paper descriptions. See §6.

---

## 4. The GPL / LGPL Bridge Boundary

**This is the most important section in this file.** The forks listed in
§5 are copyleft. Linking any of them into Signex would relicense Signex.
It will not happen.

The rules, imperatively:

- **No GPL or LGPL code in `crates/`.** Not vendored, not adapted, not
  "just this one function".
- **No GPL or LGPL Cargo dependency, ever** — direct or transitive.
  `deny.toml` rejects them and `cargo-deny` runs in CI.
- **No `*-sys` shim** wrapping a copyleft library. A binding is a link.
- **Copyleft solvers are reached across a process boundary, and only
  across a process boundary.** The bridge writes input files, spawns the
  solver as a subprocess, and reads results back:

```
signex (Apache-2.0)                        ‖   solver (GPL / LGPL)
                                           ‖
  write .csx / .sif / mesh input   ────────╫──►  openEMS | ElmerSolver
                                           ‖         (separate process,
  read HDF5 / VTK results          ◄───────╫──       separate binary)
                                           ‖
        no linking, no FFI, no shared address space
```

The bridge crates (`openems-bridge`, `elmer-bridge`, and their kin) are
Apache-2.0. They contain file writers, file readers, and a process
spawner. They contain no solver code. If a patch to a bridge crate adds a
`links =` key, a `-sys` dependency, or a copied solver source file, it is
wrong regardless of how well it works.

ngspice is the one FFI case, and it is FFI precisely because its licensing
is not GPL-style copyleft. That is a licence-specific exception, not a
precedent. Do not generalise it to OpenEMS or Elmer.

The Apache-2.0 surface of this repository is a promise to downstream
users and redistributors. See [`docs/LICENSING.md`](LICENSING.md) for the
full statement and the audit trail behind it.

---

## 5. Why We Fork At All

We fork when a tool we depend on needs fixes we cannot wait for, and we
carry those fixes on a named branch. Public forks:

| Fork | Upstream | Licence | What we carry |
|------|----------|---------|---------------|
| [alplabai/openEMS](https://github.com/alplabai/openEMS) | thliebig/openEMS | GPL-3.0 | `exit()` → exceptions, `volatile` → atomics, O(log N) `SnapToMeshLine`, memory-leak fixes. A `feature/cuda-engine` branch explores GPU FDTD. |
| [alplabai/openEMS-Project](https://github.com/alplabai/openEMS-Project) | thliebig/openEMS-Project | see submodules | Superproject pinning the submodule set |
| [alplabai/CSXCAD](https://github.com/alplabai/CSXCAD) | thliebig/CSXCAD | LGPL-3.0 | Iterator UB after `erase`, `UpdateIDs` logic bug |
| [alplabai/fparser](https://github.com/alplabai/fparser) | thliebig/fparser | consult upstream | Function parser for openEMS expressions |
| [alplabai/elmerfem](https://github.com/alplabai/elmerfem) | ElmerCSC/elmerfem | mixed; consult upstream | Branch `signex-pcb`: VectorHelmholtz segfault plus four HIGH-severity fixes in `Load.c` / `cholmod.c` |
| [alplabai/AppCSXCAD](https://github.com/alplabai/AppCSXCAD) | thliebig/AppCSXCAD | GPL-3.0 | Qt GUI for CSXCAD |
| [alplabai/QCSXCAD](https://github.com/alplabai/QCSXCAD) | thliebig/QCSXCAD | LGPL-3.0 | Qt GUI library for CSXCAD |
| [alplabai/signex-kicad-import](https://github.com/alplabai/signex-kicad-import) | — (ours) | GPL-3.0-or-later | The one-way KiCad → Signex migration companion. Not a fork; a deliberately separate GPL repo. See [`docs/LICENSING.md`](LICENSING.md). |

GMSH is used from upstream unmodified. Our ngspice fork is not public;
see §3.

**And now the counterweight, which matters more than the list.** Our own
internal plan names "maintaining too many forks" as a way this project
fails: *each fork is a long-term tax. Minimize them.* Every fork is a
merge conflict scheduled for a date we don't control.

So:

- **Forks exist to carry fixes we need**, not to hold opinions.
- **We upstream where we can.** A fix that lands upstream is a fix we stop
  paying for.
- **A new fork needs justification.** "It would be convenient" is not one.
- **If you have a fix for one of these tools, send it to the tool's
  upstream** where it is appropriate for upstream — `thliebig/openEMS`,
  `thliebig/CSXCAD`, `ElmerCSC/elmerfem`. That helps more people than a
  patch to our fork does, and it shortens our fork. If the fix is
  Signex-specific, then send it to our fork.

---

## 6. Why We Didn't Use The Obvious Thing

The highest-value section for anyone about to start work.

**Constraint solving — not SolveSpace, not planegcs.** SolveSpace's
`slvs` is GPL-3.0-or-later; FreeCAD's `planegcs` is LGPL-2.1-or-later.
Both are good. Both are unusable here: a constraint solver lives *inside*
the editor, in-process, on the hot path — the §4 subprocess escape hatch
does not apply to something called on every mouse-move. So we wrote our
own Newton–Levenberg-Marquardt solver in `crates/signex-sketch`, from
textbook and published-paper descriptions, under clean-room discipline
with a contemporaneous audit trail. Sources are cited in the source
comments. If you are improving the solver, cite the maths, never another
tool's implementation.

**KiCad — no code, in either direction.** This repository is Apache-clean
and CI enforces it. KiCad interoperability ships as a separate
GPL-3.0-or-later companion, `signex-kicad-import`. The full history is in
[`docs/LICENSING.md`](LICENSING.md); read it before proposing anything
that touches KiCad formats.

**Formulas — not MathJax, not KaTeX.** Both are JavaScript. Using either
means a JS engine in the build graph or in the runtime, and neither is
acceptable in a Rust desktop application whose install size and build
determinism we care about. #304 demonstrated the failure mode exactly: it
reached for `boa_engine` in `build.rs` to render formulas. typst is
Rust-native, does real math typesetting, and emits PDF directly — which is
also the output the Design Notebook needs anyway.

**Touchstone / S-parameters — not hand-rolled.** Touchstone looks like a
weekend parser and is not: `.s2p` through `.snp`, option lines, mixed
formats, noise data, reference-impedance renormalisation, and the network
algebra that has to be correct afterwards. scikit-rf has been getting this
right for years. Same reasoning for complex arithmetic: NumPy exists.

**Routing — not Topola, not any external router.** See §3 and #233.

**3D kernel — not hand-rolled STEP.** STEP is an enormous standard.
OpenCascade reads it; we keep OpenCascade behind a conversion step for
licence hygiene rather than replacing it.

The pattern: we hand-roll when linking would relicense us and no process
boundary is available (the sketch solver). We use the mature thing
everywhere else.

---

## 7. Not Yet Chosen

Honestly open. Input is genuinely wanted here — open an issue.

- **PCB thermal / IPC-2221-style engineering calculators.** #303 proposes
  one. We have no chosen implementation. The open questions are where the
  results live (Design Notebook cell? panel? both?) and whether the maths
  belongs in Rust or in the Python compute layer, which already has the
  numerical stack. Decide that before writing the maths.
- **Signal-integrity post-processing beyond scikit-rf's coverage** — eye
  diagrams, jitter decomposition. scikit-rf is the assumed foundation; the
  layer above it is unchosen.
- **IBIS parsing.** No tool chosen. Candidates exist; none evaluated.
- **Gerber / ODB++ / IPC-2581 writers.** Currently assumed in-tree and
  Apache-2.0. No external library has been ruled in.
- **Plot rendering inside the compute layer.** Matplotlib → SVG is the
  current assumption. It works; it may not be the best answer for
  interactive plots.
- **Blender bundling versus detection** for high-quality offline 3D
  renders. Detection is the current lean; it is not settled.

If a domain is not on this page at all, it is not chosen — ask.

---

## 8. Rules For Changing This Document

- **A new dependency in one of these domains updates this file in the
  same PR.** A choice that is not written here is not a choice; it is a
  surprise for the next contributor.
- **Licence claims are verified, not remembered.** Check the repository's
  actual licence before writing an SPDX identifier here. Where licensing
  is genuinely mixed, say "consult upstream" and mean it — a wrong SPDX
  identifier in a licensing document is worse than no identifier.
- **Never link a repository you have not confirmed is public.** A 404 in
  a document like this is worse than an omission.
- **Version bands here follow `docs/ROADMAP.md`.** Edit the roadmap first.
- **Adding a fork requires the justification from §5** in the PR
  description: what fix, why not upstream, when it retires.
