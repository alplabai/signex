# Schematic GPU mount

How the schematic is drawn on the GPU when
`feature_flags::SCHEMATIC_GPU_RENDER_ENABLED` is on (#199). Code:
`crates/signex-app/src/canvas/gpu_mount.rs` and `gpu_drag.rs`. The flag ships
off; the CPU canvas is what users see until the default-flip checklist on #199
is done.

## Layers

The mount is a `stack!` of five layers, bottom to top:

1. **Backdrop** (`SchematicBackdrop`) - paper, grid and sheet frame. It has no
   `update`: `stack!` hands every event to every child, so a second handler
   would place two symbols per click.
2. **Content shader** (`SchematicShaderProgram`) - the sheet content on the
   GPU. Its scene is built in `draw`, because only `draw` knows the cull
   bounds. During a move-drag it carries the static layer, everything the drag
   leaves alone.
3. **Static text layer** (`SchematicStaticTextLayer`) - the rotated texts of
   everything not moving, which the GPU glyph atlas cannot draw
   (`TextItem::is_axis_aligned`).
4. **Drag shader** (`SchematicDragShaderProgram`) - the moving items of a
   move-drag. It is a second primitive type (`SchematicDragSurface`) because
   iced prepares every primitive of one type before rendering any of them, so
   two of one type in a window would overwrite each other. Empty when idle.
5. **Interactive overlay** (`SchematicCanvas` in `ContentMode::GpuOverlay`) -
   the only layer with `update`. During a drag its content layer draws the
   rotated texts of the moving items; then dim, selection, ghosts, previews and
   the HUD as on the CPU path.

The PCB's order (interactive canvas at the bottom) is not reused: it would bury
selection and ghosts under the content. A moving item draws above every unmoved
one, as in the CPU preview.

## The move-drag

The drag is split the way the CPU preview splits it (`drag_layers`, #1335). The
**static scene** is built once per drag and again on zoom, on a pan past its
margin (one viewport on every side), on a new document revision or a new skip
set, and stays resident on the GPU under one generation. The **moving scene**
is rebuilt each cursor step and uploaded every frame (generation `None`); it is
small. An idle drag layer carries one constant generation, so the GPU uploads
its empty scene once.

The drag state lives in the overlay's `Program::State`, which the shaders
cannot reach. `update` therefore leaves the frame it computed in a
`PublishedDrag` on the slot. A click-click float can be ended by an app message
(Delete, Undo) that no canvas event announces, but iced forwards
`RedrawRequested` to `Program::update` before every draw, so `update` has
re-judged the gesture (`drag_delta` drops a stale float) by the time a shader
reads the mark. The mark carries no staleness check of its own. Replacing the
render cache clears it, because a new document at the same known revision would
otherwise keep the old frame.

## Tests

`gpu_mount_tests.rs` renders the real stack headlessly through iced's wgpu
renderer (never tiny-skia, which swaps red and blue and cannot run shader
primitives) and pins coverage, CPU parity, z-order, HiDPI and padding.
`gpu_mount_drag_tests.rs` drives the overlay's `update` and reads the programs'
primitives with no GPU.
