//! Symbol canvas editor messages — the `Symbol*` family split out of the
//! former flat `PrimitiveEditorMsg` (ADR-0001 D3). Reached through
//! [`super::PrimitiveEdit::Symbol`]; matched by `apply_symbol_primitive_edit`.

use super::*;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum SymbolEditorMsg {
    // ── Symbol ─────────────────────────────────────────────
    /// Set the active drawing tool on the Symbol canvas.
    SetTool(SymbolToolMsg),

    /// Click-to-place a pin on the standalone Symbol canvas at the
    /// given grid-snapped (mm) world position.
    AddPin { x: f64, y: f64 },

    /// Place an axis-aligned rectangle spanning the two opposite corners
    /// (both grid-snapped mm world positions). Emitted on the second
    /// click of the two-click canvas draw flow; the handler normalizes
    /// the corners.
    AddRectangle {
        from_x: f64,
        from_y: f64,
        to_x: f64,
        to_y: f64,
    },

    /// Place a line segment from `from` to `to` (both grid-snapped mm world positions).
    AddLine {
        from_x: f64,
        from_y: f64,
        to_x: f64,
        to_y: f64,
    },

    /// Place a circle with center `(cx, cy)` and the given radius.
    /// Emitted on the second click of the two-click canvas draw flow.
    AddCircle { cx: f64, cy: f64, radius: f64 },

    /// Place an arc with center, radius, and start/end angles in degrees
    /// (0° = right, 90° = up). Emitted on the third click of the
    /// three-click canvas draw flow.
    AddArc {
        cx: f64,
        cy: f64,
        radius: f64,
        start_deg: f64,
        end_deg: f64,
    },

    /// The Place Arc gesture's third click swept >= 360° — rejected
    /// rather than committed (see `CanvasAction::ArcSweepRejected`'s
    /// doc comment). Sets `SymbolEditorState::status_message`; no
    /// graphic is pushed and no undo snapshot is recorded.
    ArcSweepRejected,

    /// Stamp a default "Text" label anchored at `(x, y)`. Edit the
    /// content via the Properties panel after placement.
    AddText { x: f64, y: f64 },

    /// Append one grid-snapped vertex to `SymbolEditorState::
    /// polygon_vertices` (Place Polygon click-collect stash).
    PolygonClick { x: f64, y: f64 },

    /// Commit the Place Polygon stash: pushes a closed polygon
    /// (implicitly closed — see `SymbolGraphicKind::Polygon`) through
    /// `push_graphic` when the stash holds a valid ring (>= 3
    /// vertices after normalising), else silently discards it.
    PolygonCommit,

    /// Discard the Place Polygon stash with no commit (Esc /
    /// right-click while a placement is in flight).
    PolygonCancel,

    /// Join the selected Line/Arc graphics end-to-end into one closed
    /// Polygon (`signex_library::chain_into_closed_contour`). No-op
    /// when the selection is empty, contains fewer than one eligible
    /// graphic, or contains any non-Line/Arc graphic. An open chain is
    /// auto-closed once by synthesizing the missing edge between its
    /// two loose ends before retrying.
    JoinSelectionIntoPolygon,

    /// Select a symbol element (pin index / field key).
    Select(SymbolSelectionMsg),

    /// Click landed on empty canvas — drop the current selection.
    Deselect,

    /// Drag the currently-selected element to a new grid-snapped
    /// world position.
    MoveSelected { x: f64, y: f64 },

    /// Shift every pin and graphic by `(dx, dy)` mm (All selection drag).
    MoveAll { dx: f64, dy: f64 },

    /// Drag-to-resize: move one resize handle of the graphic at
    /// `idx` to grid-snapped world coordinates `(x, y)`. Fires
    /// continuously while the user holds and drags a graphic handle
    /// in the Select tool. The dispatcher mutates the matching field
    /// on `SymbolGraphic.kind` (rect corner / line endpoint / circle
    /// radius / arc angle / text anchor).
    MoveGraphicHandle {
        idx: usize,
        handle: GraphicHandleMsg,
        x: f64,
        y: f64,
    },

    /// Rotate the current Symbol-canvas selection by 90°.
    ///
    /// `clockwise=true` (Space), `clockwise=false` (Shift+Space).
    /// `pivot=GeometryCenter` when Option/Alt is held.
    RotateSelected {
        clockwise: bool,
        pivot: SymbolRotatePivotMsg,
    },

    /// Delete-key — drop the currently-selected element.
    DeleteSelected,

    /// Active-bar Align ▸ "Align To Grid" — snap every pin/graphic
    /// named by the current selection onto the symbol-canvas snap
    /// grid, in place. No-op (no undo snapshot, no dirty flag) for
    /// `None`/`Field` selections — see `state::selected_is_alignable`.
    /// #426.
    AlignSelectedToGrid,

    /// Properties pane — overwrite the pin number string at index.
    SetPinNumber { idx: usize, number: String },

    /// Properties pane — overwrite the pin name string at index.
    SetPinName { idx: usize, name: String },

    // ── View / camera (Altium-style pan/zoom/grid) ─────────
    /// Right- or middle-button pan delta in screen pixels.
    /// Updates `editor.camera.offset`.
    Pan { dx: f32, dy: f32 },

    /// Mouse-wheel zoom centred on the cursor screen position
    /// `(sx, sy)` in canvas-local pixels. Positive `delta` zooms
    /// in, negative zooms out. Updates `editor.camera`.
    Zoom { sx: f32, sy: f32, delta: f32 },

    /// Fit the active symbol's bounding box into the viewport.
    /// Bound to the Home key + the Fit button on the toolbar.
    Fit,

    /// Cursor world position in mm — drives the status footer X/Y
    /// readout. `None` clears the readout when the cursor leaves.
    CursorAt {
        x_mm: Option<f64>,
        y_mm: Option<f64>,
    },

    /// Toolbar / context-menu — pick the sheet background colour
    /// preset (Black / White / Dark Gray / Light Gray / Cream),
    /// matching Altium's per-document Sheet Color. Applies to the
    /// `.snxlib` containing this `.snxsym` so every primitive
    /// editor opened from the same library shares the colour.
    SetSheetColor(crate::panels::SheetColor),

    /// Status-footer click on the `Grid` label — toggles whether
    /// the dot grid renders. Applies to the containing `.snxlib`.
    ToggleGrid,

    /// Status-footer click on the grid spacing — cycles through
    /// `crate::canvas::grid::GRID_SIZES_MM`. Applies to the
    /// containing `.snxlib`.
    CycleGridSize,

    /// Status-footer click on the unit label — cycles
    /// mm → mil → inch → um → mm. Applies to the containing
    /// `.snxlib`.
    CycleUnit,

    // ── Multi-part component ───────────────────────────────
    /// Toolbar — step the active sub-part down one (Altium "←
    /// Part" arrow). Clamps at `1`. Drives the canvas pin filter +
    /// the active-part badge in the toolbar.
    PrevPart,

    /// Toolbar — step the active sub-part up one (Altium "Part →"
    /// arrow). Clamps at the symbol's max declared `part_number`
    /// (i.e. doesn't auto-create new parts; that's the Tools ▸
    /// New Part flow's job).
    NextPart,

    /// Tools ▸ New Part — bumps the symbol's max `part_number` by
    /// one and switches `active_part` to the new value. The new
    /// part starts with no pins; the user adds pins with the
    /// active_part selected.
    NewPart,

    /// Tools ▸ Remove Part / toolbar "−" — deletes the active sub-
    /// part outright: its pins are dropped, every higher part
    /// renumbers down by one, and the declared `part_count`
    /// decrements. The active part clamps to the new top unit. No-op
    /// when only one part exists or the active part is part 1.
    RemovePart,

    /// Undo the last symbol mutation. Pops from `undo_snapshots`
    /// and restores the previous `Symbol` state.
    Undo,

    /// Redo — reapply an undone symbol mutation. Pops from
    /// `redo_snapshots`.
    Redo,

    /// Drag committed — clears `mid_drag` so the next drag starts a
    /// fresh undo snapshot group. Emitted on `ButtonReleased(Left)`
    /// when a move drag was in progress.
    DragCommit,

    // ── v0.13 — Symbol library editor active bar ─────────
    /// Toggle a symbol-editor active-bar dropdown menu.
    ToggleActiveBarMenu(crate::library::editor::symbol::state::SymActiveBarMenu),

    /// Close any open symbol-editor active-bar dropdown.
    CloseActiveBarMenu,

    /// "Coming soon" stub for symbol-editor active-bar items.
    ActiveBarStub(&'static str),

    /// Toggle a kind on the symbol-editor selection filter.
    ToggleSelectionFilter(crate::library::editor::symbol::state::SymbolFilterKind),

    // ── Right-click context menu ─────────────────────────────
    /// Right-release-without-pan opens the context menu at
    /// window-absolute `(x, y)`; `target` is what the cursor was
    /// over, so the handler can select-first (Altium parity) before
    /// showing the menu.
    ShowContextMenu {
        x: f32,
        y: f32,
        target: SymbolContextTargetMsg,
    },

    /// Close the open context menu (Esc / click outside / any
    /// non-context-menu action fired through it).
    CloseContextMenu,

    /// Toggle which submenu row is accordion-expanded in place.
    /// `None` collapses whichever submenu was open.
    ContextMenuOpenSubmenu(Option<SymbolContextSubmenuMsg>),

    /// A context-menu row's real action, boxed so this enum doesn't
    /// grow for every other variant. The dispatcher applies the
    /// boxed message via itself, then closes the menu — the "any
    /// click on a real action closes the popover" behaviour every
    /// row wants, expressed once instead of per-row.
    ContextMenuAction(Box<SymbolEditorMsg>),
}
