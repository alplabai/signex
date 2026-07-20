//! Footprint canvas editor messages — the `Footprint*` family split out of
//! the former flat `PrimitiveEditorMsg` (ADR-0001 D3). Reached through
//! [`super::PrimitiveEdit::Footprint`]; matched by `apply_footprint_primitive_edit`.

use super::*;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum FootprintEditorMsg {
    // ── Footprint ──────────────────────────────────────────
    /// v0.18.7 — switch which footprint inside the multi-footprint
    /// `.snxfpt` envelope is being edited. Wraps `active_idx` on the
    /// `FootprintEditorState` wrapper. The dispatcher refreshes the
    /// canvas pad list + camera fit on switch.
    SelectActiveIdx(usize),

    /// v0.18.7 — append a new empty footprint to the active
    /// `.snxfpt` envelope and switch the editor onto it. Names the
    /// new footprint `Footprint N` where N is the next free index.
    AddNewSibling,

    /// Click-to-place a pad at the given world position.
    AddPad {
        x_mm: f64,
        y_mm: f64,
    },

    /// v0.18.12 — Click-to-place a non-plated through hole (NPT) at
    /// the given world position. Mints a `Pad` with `kind = NptHole`,
    /// no copper / mask / paste, drill diameter from
    /// `next_pad_defaults.size_x_mm`. The active bar's "Place Hole"
    /// tool fires this on empty-canvas click.
    AddHole {
        x_mm: f64,
        y_mm: f64,
    },

    /// v0.18.15 — Click-to-place a silk-layer text label. Appends an
    /// `FpGraphic { kind: Text { ... } }` to `footprint.silk_f`
    /// with placeholder content "TEXT" + 1mm size. The user edits
    /// the content via the Properties panel later.
    AddText {
        x_mm: f64,
        y_mm: f64,
    },

    /// v0.14 — commit a dragged text frame (anchor + size, mm).
    AddTextFrame {
        x_mm: f64,
        y_mm: f64,
        w_mm: f64,
        h_mm: f64,
    },

    /// v0.18.15.1 — click during a Place Track 2-click gesture.
    TrackClick {
        x_mm: f64,
        y_mm: f64,
    },

    /// v0.18.15.1 — Esc / right-click during Place Track.
    TrackCancel,

    /// v0.18.15.3 — click during a Place Arc 3-click gesture.
    ArcClick {
        x_mm: f64,
        y_mm: f64,
    },

    /// v0.18.15.3 — Esc / right-click during Place Arc.
    ArcCancel,

    /// v0.18.15.4 — Place Polygon click (appends a vertex).
    PolygonClick {
        x_mm: f64,
        y_mm: f64,
    },

    /// v0.18.15.4 — explicit polygon commit.
    PolygonCommit,

    /// v0.18.15.4 — Esc / right-click during Place Polygon.
    PolygonCancel,

    /// v0.18.18 — silk-front graphic selection.
    SelectSilkF(Option<usize>),

    /// v0.18.18 — delete the selected silk-front graphic.
    DeleteSilkF,

    /// v0.18.14 — Selection Filter pill toggle from the unified
    /// active bar. Mirrors the panel-side
    /// `PanelMsg::FpEditorToggleSelectionFilter` but flows through
    /// the per-tab editor dispatch so the active bar can mutate
    /// `editor.state.selection_filter` directly.
    ToggleSelectionFilter(crate::library::editor::footprint::state::SelectionFilterKind),

    /// Drag the pad at `idx` to a new world position.
    MovePad {
        idx: usize,
        x_mm: f64,
        y_mm: f64,
    },

    /// Cursor moved over the canvas — drives the footer X/Y readout.
    CursorAt {
        x_mm: f64,
        y_mm: f64,
    },

    /// Select / deselect a pad. `None` deselects everything.
    SelectPad(Option<usize>),

    /// v0.27 — Multi-select replacement. Replaces the entire
    /// selection with `pads`. First entry becomes primary (drives
    /// the Properties form); rest go to `selected_pads_extra`.
    /// Empty Vec deselects all.
    SelectPads(Vec<usize>),

    /// v0.27 — Multi-select for sketch entities. Replaces the
    /// sketch selection. First → primary, second → secondary,
    /// rest → `selected_sketch_extra`. Empty = clear.
    SketchSelectMany(Vec<signex_sketch::id::SketchEntityId>),

    /// Delete-key — remove the currently-selected pad.
    DeleteSelected,

    /// Toolbar — toggle a layer's visibility. Carries the Standard layer
    /// name string; the dispatcher maps to `FpLayer`.
    ToggleLayer(String),

    /// Toolbar — toggle the auto-fit-courtyard flag.
    ToggleAutoFit,

    // ── v0.13.1 — Sketch mode (Phase 6) ────────────────────
    /// Toolbar — switch the editor mode. `Normal` is the existing
    /// pad-list authoring; `Sketch` opens the parametric sketcher;
    /// `View3d` is the body 3D preview.
    SetMode(crate::library::editor::footprint::state::EditorMode),

    /// Sketch tool — place a Point at the given world-mm position.
    /// Triggers a solve + bake via the dispatcher.
    SketchPlacePoint {
        x_mm: f64,
        y_mm: f64,
    },

    /// Sketch inspector — edit / insert a parameter source string.
    /// Triggers a solve + bake.
    SketchEditParameter {
        name: String,
        expr: String,
    },

    /// v0.13.2 — Tool palette: switch the active drawing tool.
    /// Clears any in-flight multi-click gesture (`tool_pending`) so
    /// switching tools mid-gesture doesn't leave dangling anchors.
    SketchSetTool(crate::library::editor::footprint::state::SketchTool),

    /// v0.16.1 — toggle construction-mode (sticky).
    SketchToggleConstruction,

    /// v0.22 Phase A5 — toggle centerline-mode (sticky). Mutually
    /// exclusive with construction-mode.
    SketchToggleCenterline,

    /// v0.16.1 — TAB pause/resume during pad placement.
    TogglePlacementPause,

    /// v0.26 — open the canvas right-click context menu at the given
    /// **window-absolute** screen position.
    ShowContextMenu {
        x: f32,
        y: f32,
        target: crate::library::editor::footprint::state::FootprintContextTarget,
    },

    /// v0.26 — dismiss the context menu.
    CloseContextMenu,

    /// v0.26 — hover-expand a context-menu submenu. `None` collapses.
    ContextMenuOpenSubmenu(
        Option<crate::library::editor::footprint::state::FootprintContextSubmenu>,
    ),

    /// v0.26 — execute one of the context-menu lightweight actions.
    ContextMenuAction(crate::library::editor::footprint::state::FootprintContextAction),

    /// v0.26-C — canvas signals that the pending Fit-to-Window
    /// request has been honoured. See EditorMsg::Footprint(FootprintEditorMsg::FitConsumed).
    FitConsumed,

    /// v0.26-E — clipboard ops on the selected pad.
    CopyPad,

    CutPad,

    PastePad,

    /// v0.16.2 — set the role attr on a sketch entity. Inspector
    /// emits this when the user picks a value from the Role dropdown;
    /// dispatcher routes through
    /// `apply_sketch_role_with_warnings`.
    SketchSetRole {
        id: signex_sketch::id::SketchEntityId,
        role: RoleTag,
    },

    /// v0.22 Phase D4 — convert the closed-loop profile that includes
    /// the currently-selected Line into a `PadShape::Custom(SketchProfile)`
    /// pad. Mints a centre `Point` at the loop's centroid + a
    /// `PadAttr` with a `SketchProfile` shape pointing at the seed
    /// Line. Bake re-walks the loop on the next solve. No-op (with a
    /// warning surfaced via `solve_warnings`) when the selection is
    /// not a Line, the line is not part of a closed loop, or no
    /// solve has run yet.
    SketchMakePadFromProfile,

    /// v0.24 Phase 3 (Track A3) — Right-click action on an Arc that
    /// belongs to a RoundRect pad's corner outline. Mints a fresh
    /// per-corner sketch parameter (`corner_r_<slug>_<corner>`),
    /// copies the current shared parameter's value into it, and
    /// records the per-corner override on the owning pad's
    /// `shape_params` (e.g. `"corner_r_ne" -> "<new_param>"`). The
    /// other three corners stay on the shared `corner_r` parameter
    /// so the user can edit one corner independently while leaving
    /// the rest in lockstep. No-op (with a `tracing::warn`) when the
    /// arc doesn't belong to any pad's `shape_params` graph.
    SketchUnlinkCornerRadius {
        arc_entity_id: signex_sketch::id::SketchEntityId,
    },

    /// v0.15 — Pads-mode tool switch (Select / PlacePad). Right-
    /// click cancels back to Select via the same dispatch.
    SetPadsTool(crate::library::editor::footprint::state::PadsTool),

    /// v0.15 — global tool-cancel (Esc). Resets both `pads_tool`
    /// AND `active_tool` (sketch) to Select + clears
    /// `tool_pending`. Mode-agnostic, so the same Esc dispatch
    /// works whichever mode the user is in.
    ToolEscape,

    // ── v0.13 — Active bar dropdowns ──────────────────────
    /// Toggle the active-bar dropdown menu. Click the chevron once to
    /// open; click again (or click-outside / pick item) to close.
    ToggleActiveBarMenu(crate::library::editor::footprint::state::FpActiveBarMenu),

    /// Close any open active-bar dropdown (item picked / click-outside).
    CloseActiveBarMenu,

    /// Stub for "coming soon" Place / Move / Drag / Selection / 3D
    /// Body / Text / Shapes dropdown items. Carries the label so the
    /// dispatcher can log a single warn() per click without minting
    /// a separate variant per item.
    ActiveBarStub(&'static str),

    /// v0.14 (Task 6) — apply footprint filter preset `idx` from the
    /// persisted `footprint_filter_presets` list. No-op if `idx` is
    /// out of range.
    ApplyFilterPreset(usize),

    /// v0.14 (Task 6) — flip every footprint selection filter on/off
    /// (the Filter dropdown's "All - On / All - Off" chip).
    ToggleAllFilters,

    /// v0.14 (Task 6) — snapshot the current selection filter as a
    /// new named preset (default name `Filter {n}`) and persist it,
    /// capped at `CUSTOM_FILTER_PRESET_LIMIT`. Source: the Filter
    /// dropdown's "Save current filter as preset…" button.
    CaptureFilterPreset,

    /// Snap-options toggle from the active-bar Snap dropdown.
    /// Equivalent to `PanelMsg::FpEditorToggleSnapOption` but flows
    /// through the editor-event path so the dropdown overlay stays
    /// in the LibraryMessage envelope.
    ActiveBarToggleSnap(crate::panels::SnapOptionFlag),

    /// Snapping-mode pick from the active-bar Snap dropdown
    /// (All Layers / Current Layer / Off).
    ActiveBarSetSnappingMode(crate::library::editor::footprint::state::SnappingMode),

    /// Snap sub-tab pick from the active-bar Snap dropdown
    /// (Grids / Guides / Axes).
    ActiveBarSetSnapSubTab(crate::library::editor::footprint::state::SnapSubTab),

    /// Active-bar Place → Rotate Selection. 90° CCW rotation on the
    /// currently-selected pad's `rotation_deg`.
    ActiveBarRotateSelection,

    /// Active-bar Place → Flip Selection. Swap Top ↔ Bottom layer
    /// (and the paste/mask siblings) on the currently-selected pad.
    ActiveBarFlipSelection,

    /// Active-bar Place → one-step nudge. Nudges the whole selection
    /// (`selected_pad` + `selected_pads_extra`) by one active grid step
    /// in +X and +Y. The step derives from `snap_options.grid_step_mm`
    /// — no hardcoded size. No-op when nothing is selected. Superseded
    /// as the active-bar's primary "Move Selection by X, Y…" item by
    /// the typed-delta Move-By modal (`FootprintMoveByOpen` and its
    /// siblings below); this one-step nudge is still reachable and
    /// shares its geometry with the modal via the
    /// `footprint_nudge_selection` dispatcher helper.
    ActiveBarNudgeSelection,

    /// Active-bar Place → "Move Selection by X, Y…". Opens the typed-
    /// delta modal (`FootprintEditorState::move_by_modal`).
    MoveByOpen,

    /// Move-By modal X buffer edit (erasable string, mm).
    MoveBySetX(String),

    /// Move-By modal Y buffer edit (erasable string, mm).
    MoveBySetY(String),

    /// Confirm the Move-By modal: nudge the selection by the parsed
    /// (dx, dy) mm delta, then close the modal. No-op (but still
    /// closes) if either buffer fails to parse.
    MoveByConfirm,

    /// Cancel the Move-By modal without moving anything.
    MoveByCancel,

    /// Active-bar Body → "3D Body". Extrude the courtyard into a solid.
    MintBody3d,

    /// Active-bar Body → "Extruded 3D Body". Extrude the fab outline.
    MintExtrudedBody3d,

    /// Active-bar Align → Align Selection To Grid. Snap the currently-
    /// selected pad's centre to the nearest active-grid step.
    ActiveBarAlignSelectionToGrid,

    /// Active-bar Align → Move All Components Origin To Grid. Snap
    /// every pad's centre to the nearest active-grid step.
    ActiveBarMoveOriginToGrid,

    /// Active-bar Align → align / distribute / re-space the current
    /// pad selection. Operates on `selected_pad` + `selected_pads_extra`
    /// (the combined set); a no-op when fewer than two pads are
    /// selected (fewer than three for the distribute ops). See
    /// [`AlignOp`](crate::library::editor::footprint::state::AlignOp)
    /// for the per-operation geometry.
    AlignPads(crate::library::editor::footprint::state::AlignOp),

    /// #370 — Active-bar Align ▸ "Align…". Opens the per-axis Align
    /// dialog ([`FootprintEditorState::align_modal`]). Composes the
    /// existing [`AlignOp`](crate::library::editor::footprint::state::AlignOp)
    /// variants; adds no new geometry.
    AlignOpen,

    /// #370 — Align dialog: set (or clear, with `None`) the chosen
    /// horizontal op. `Some(op)` restricted to the X-axis variants
    /// (`Left` / `CenterH` / `Right` / `DistributeH`) by the view.
    AlignSetHorizontal(Option<crate::library::editor::footprint::state::AlignOp>),

    /// #370 — Align dialog: set (or clear, with `None`) the chosen
    /// vertical op. `Some(op)` restricted to the Y-axis variants
    /// (`Top` / `CenterV` / `Bottom` / `DistributeV`) by the view.
    AlignSetVertical(Option<crate::library::editor::footprint::state::AlignOp>),

    /// #370 — Confirm the Align dialog: apply both chosen axes under a
    /// SINGLE undo snapshot, then close. Choosing neither axis (or a
    /// selection too small for the chosen ops) is a clean no-op that
    /// pushes no history and does not dirty the document.
    AlignConfirm,

    /// #370 — Cancel/dismiss the Align dialog without touching the
    /// selection. Reached by the Cancel button, the close ✕, the
    /// backdrop click, and Esc.
    AlignCancel,

    /// Active-bar Selection → Select All. Pads mode picks the first
    /// pad; Sketch mode picks the first sketch entity.
    ActiveBarSelectAll,

    /// Active-bar Selection → Toggle Selection. Clears the selection
    /// slot if anything is selected.
    ActiveBarClearSelection,

    /// Active-bar Shapes → arm a sketch tool. Switches the editor to
    /// Sketch mode if it isn't already and sets `state.active_tool`.
    ActiveBarSetSketchTool(crate::library::editor::footprint::state::SketchTool),

    /// Properties panel — rename the active internal footprint. Writes
    /// `editor.primitive_mut().name` so the rename mirrors into the
    /// .snxfpt envelope on next save.
    SetName(String),

    /// v0.13.2 — Canvas left-click in Sketch mode while a multi-click
    /// drawing tool is active. The dispatcher advances the per-tool
    /// state machine on `tool_pending` and emits the appropriate
    /// `SketchEdit` (AddEntity Line / Circle / Arc) when the gesture
    /// completes. `snap_id` carries the sketch Point under the cursor
    /// (within `SNAP_RADIUS_PX`) for auto-Coincident snap.
    SketchToolClick {
        x_mm: f64,
        y_mm: f64,
        snap_id: Option<signex_sketch::id::SketchEntityId>,
    },

    /// v0.13.2 — Escape during a multi-click gesture: discard
    /// `tool_pending` without emitting a SketchEdit.
    SketchToolEscape,

    /// v0.24 Track D — append a typed character to
    /// `state.placement_input.buffer`. Mints a fresh `PlacementInput`
    /// keyed off the active sketch tool when the field is `None`.
    /// Validation (one decimal point, leading minus only for
    /// `ArcSweep`) lives in the dispatcher; the canvas filters out
    /// non-numeric characters before publishing.
    SketchPlacementInputChar(char),

    /// v0.24 Track D — pop the trailing character from
    /// `state.placement_input.buffer`. Clears `placement_input` to
    /// `None` once the buffer empties so the next keypress mints a
    /// fresh entry.
    SketchPlacementInputBackspace,

    /// v0.24 Track D — Enter while the placement-input overlay is
    /// open. No-op on state — the buffer waits for the next click to
    /// consume it. Surfaced as a distinct message so the canvas can
    /// capture the keypress and prevent it from triggering global
    /// shortcuts (Search, Run, …).
    SketchPlacementInputEnter,

    /// v0.24 Track D — Escape while the placement-input overlay is
    /// open. Clears `state.placement_input = None`; the next click
    /// commits at the cursor position as if no buffer had been typed.
    SketchPlacementInputEscape,

    /// v0.14-footprint — Tab toggles the active Line placement-input
    /// field between length and angle (Fusion convention). No-op for
    /// any other tool's placement input.
    SketchPlacementInputTab,

    // ── v0.13.3 — selection / constraint submenu / dimension ──
    /// v0.13.3 — Select a sketch entity. `None` clears the selection;
    /// `Some(id, false)` replaces the primary selection;
    /// `Some(id, true)` adds to the secondary selection slot.
    SketchSelect {
        id: Option<signex_sketch::id::SketchEntityId>,
        shift: bool,
    },

    /// v0.13.3 — Drag-move a Point entity by `(dx, dy)` in mm. Fires
    /// from the canvas while the user drags a selected Point in
    /// Sketch mode. Emits `SketchEdit::MovePoint`.
    SketchMovePoint {
        id: signex_sketch::id::SketchEntityId,
        dx: f64,
        dy: f64,
    },

    /// v0.27 — Drag-move a Line entity by translating both its
    /// endpoints. Per-tick `(dx, dy)` delta in mm.
    SketchMoveLine {
        id: signex_sketch::id::SketchEntityId,
        dx: f64,
        dy: f64,
    },

    /// v0.27 — Resize a Round pad's diameter via the east-edge
    /// handle drag in Sketch mode. The dispatcher updates pad.size_mm
    /// and the matching Circle entity + diameter parameter.
    SketchResizeRoundPad {
        pad_idx: usize,
        diameter_mm: f64,
    },

    /// v0.27 — pick the rubber-band selection mode (Inside /
    /// Touching / Outside) from the active-bar Selection picker.
    SetSelectionMode2d(crate::library::editor::footprint::state::FpSelectionMode),

    /// v0.27 — select every pad on the active primary layer.
    SelectAllOnLayer,

    /// v0.27 — drop a via at the cursor (Round, 0.6 mm copper,
    /// 0.3 mm drill, Multi-Layer plated). Bypasses Pads-mode
    /// `next_pad_defaults` so the via geometry is canonical.
    AddVia {
        x_mm: f64,
        y_mm: f64,
    },

    /// v0.27 — Rebuild the outline-following courtyard polygon
    /// from the current pad layout (union + offset). Stores the
    /// result on `state.courtyard_outline_mm`.
    RecomputeCourtyardOutline,

    /// v0.27 — multi-select every pad off the current snap grid.
    SelectOffGridPads,

    /// v0.27 — Lasso tool lifecycle.
    LassoArm,

    LassoAddVertex {
        x_mm: f64,
        y_mm: f64,
    },

    LassoCommit,

    LassoCancel,

    /// v0.27 — Touching Line tool lifecycle.
    TouchingLineArm,

    TouchingLineFirst {
        x_mm: f64,
        y_mm: f64,
    },

    TouchingLineCommit {
        x_mm: f64,
        y_mm: f64,
    },

    TouchingLineCancel,

    /// v0.27 — Z-order cycle on the last-clicked stacked pads.
    SelectOverlapped,

    SelectNextOverlapped,

    /// v0.13.3 — Add a constraint based on the current selection.
    /// The inspector's selection-aware submenu emits a `Tag` that
    /// the dispatcher maps into the appropriate `ConstraintKind`
    /// using `selected_sketch` + `selected_sketch_secondary`.
    SketchAddConstraintForSelection(SketchConstraintTag),

    /// v0.13.3 — Inline numeric input for the Dimension tool /
    /// editable Distance value. Updates `state.dimension_input`.
    SketchDimensionInput(String),
}
