//! Panel message type -- the `PanelMsg` enum every panel surface emits.

use super::*;

/// Panel-level message wrapping widget messages.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum PanelMsg {
    Tree(TreeMsg),
    SetUnit(Unit),
    RunErc,
    /// v0.14.2 — Properties panel toggle for the active footprint
    /// editor's Auto-fit Courtyard setting. Routes through
    /// `handle_dock_sch_library_message` (same dispatcher every other
    /// `Sym*` / `FpEditor*` panel msg uses) which resolves the active
    /// footprint editor by tab and flips its `auto_fit_courtyard`
    /// flag via the existing `FootprintToggleAutoFit` path.
    FpEditorToggleAutoFitCourtyard,
    /// v0.16.2 — Properties-panel Role pick_list emit. Routed
    /// through the dock handler which forwards to
    /// `LibraryMessage::PrimitiveEditorEvent { ... FootprintSketchSetRole }`
    /// keyed on the active footprint editor tab.
    FpEditorSetRole {
        id: signex_sketch::id::SketchEntityId,
        role: crate::library::messages::RoleTag,
    },
    /// v0.16.2 — Properties-panel Parameter row text input. Routed
    /// through the dock handler which forwards to
    /// `LibraryMessage::PrimitiveEditorEvent { ... FootprintSketchEditParameter }`.
    FpEditorEditParameter {
        name: String,
        expr: String,
    },
    /// v0.16.3 — Properties-panel "Pad placement defaults" form
    /// updates. The handler mutates `editor.state.next_pad_defaults`
    /// directly so the next `add_pad_at` picks up the new values.
    FpEditorSetNextPadDesignator(String),
    FpEditorSetNextPadSizeX(String),
    FpEditorSetNextPadSizeY(String),
    FpEditorSetNextPadSide(crate::library::editor::footprint::state::PadSide),
    /// v0.16.6 — Properties-panel rotation input for the next placed
    /// pad. String-typed so the user can erase / type freely.
    FpEditorSetNextPadRotation(String),
    /// v0.16.6 — Properties-panel rotation input for the SELECTED
    /// pad in Pads mode. Mutates `state.pads[idx].rotation_deg`
    /// directly + dirty-marks the tab.
    FpEditorSetSelectedPadRotation {
        idx: usize,
        value: String,
    },
    /// v0.20 — Altium-parity Pad Properties / Pad Stack / Pad Features
    /// form for the next placed pad. Each variant maps to one row in
    /// the right-dock Properties panel; the dispatcher writes the
    /// parsed value into `editor.state.next_pad_defaults` (or the
    /// matching sub-struct) so the next `add_pad_at` picks it up.
    /// String-typed inputs preserve the per-field typing buffer
    /// behaviour we use for size_x / size_y / rotation.
    FpEditorSetNextPadShape(signex_library::PadShape),
    FpEditorSetNextPadKind(signex_library::PadKind),
    FpEditorSetNextPadDrillDiameter(String),
    FpEditorSetNextPadDrillSlotLength(String),
    FpEditorSetNextPadCornerRadiusPct(String),
    FpEditorSetNextPadTemplate(String),
    FpEditorSetNextPadTemplateLibrary(String),
    FpEditorSetNextPadPasteMarginTop(String),
    FpEditorSetNextPadPasteMarginBottom(String),
    FpEditorToggleNextPadPasteEnabledTop(bool),
    FpEditorToggleNextPadPasteEnabledBottom(bool),
    FpEditorSetNextPadMaskMarginTop(String),
    FpEditorSetNextPadMaskMarginBottom(String),
    FpEditorToggleNextPadMaskTentedTop(bool),
    FpEditorToggleNextPadMaskTentedBottom(bool),
    FpEditorToggleNextPadThermalRelief(bool),
    FpEditorSetNextPadFeatureTop(signex_sketch::attr::PadFeature),
    FpEditorSetNextPadFeatureBottom(signex_sketch::attr::PadFeature),
    FpEditorToggleNextPadTestpointTopAssembly(bool),
    FpEditorToggleNextPadTestpointTopFab(bool),
    FpEditorToggleNextPadTestpointBottomAssembly(bool),
    FpEditorToggleNextPadTestpointBottomFab(bool),
    /// v0.20 — Altium-parity Pad Properties / Pad Stack / Pad Features
    /// editing for the SELECTED pad. Each handler mutates
    /// `state.pads[idx]` (and dirty-marks the editor + syncs the
    /// primitive). String-typed numeric inputs preserve the
    /// per-field typing buffer behaviour.
    FpEditorSetSelectedPadDesignator {
        idx: usize,
        value: String,
    },
    FpEditorSetSelectedPadSide {
        idx: usize,
        side: crate::library::editor::footprint::state::PadSide,
    },
    FpEditorSetSelectedPadShape {
        idx: usize,
        shape: signex_library::PadShape,
    },
    FpEditorSetSelectedPadKind {
        idx: usize,
        kind: signex_library::PadKind,
    },
    FpEditorSetSelectedPadSizeX {
        idx: usize,
        value: String,
    },
    FpEditorSetSelectedPadSizeY {
        idx: usize,
        value: String,
    },
    FpEditorSetSelectedPadDrillDiameter {
        idx: usize,
        value: String,
    },
    FpEditorSetSelectedPadDrillSlotLength {
        idx: usize,
        value: String,
    },
    FpEditorSetSelectedPadCornerRadiusPct {
        idx: usize,
        value: String,
    },
    FpEditorSetSelectedPadTemplate {
        idx: usize,
        value: String,
    },
    FpEditorSetSelectedPadTemplateLibrary {
        idx: usize,
        value: String,
    },
    FpEditorSetSelectedPadPasteMarginTop {
        idx: usize,
        value: String,
    },
    FpEditorSetSelectedPadPasteMarginBottom {
        idx: usize,
        value: String,
    },
    FpEditorToggleSelectedPadPasteEnabledTop {
        idx: usize,
        value: bool,
    },
    FpEditorToggleSelectedPadPasteEnabledBottom {
        idx: usize,
        value: bool,
    },
    FpEditorSetSelectedPadMaskMarginTop {
        idx: usize,
        value: String,
    },
    FpEditorSetSelectedPadMaskMarginBottom {
        idx: usize,
        value: String,
    },
    FpEditorToggleSelectedPadMaskTentedTop {
        idx: usize,
        value: bool,
    },
    FpEditorToggleSelectedPadMaskTentedBottom {
        idx: usize,
        value: bool,
    },
    FpEditorToggleSelectedPadThermalRelief {
        idx: usize,
        value: bool,
    },
    FpEditorSetSelectedPadFeatureTop {
        idx: usize,
        value: signex_sketch::attr::PadFeature,
    },
    FpEditorSetSelectedPadFeatureBottom {
        idx: usize,
        value: signex_sketch::attr::PadFeature,
    },
    FpEditorToggleSelectedPadTestpointTopAssembly {
        idx: usize,
        value: bool,
    },
    FpEditorToggleSelectedPadTestpointTopFab {
        idx: usize,
        value: bool,
    },
    FpEditorToggleSelectedPadTestpointBottomAssembly {
        idx: usize,
        value: bool,
    },
    FpEditorToggleSelectedPadTestpointBottomFab {
        idx: usize,
        value: bool,
    },
    /// v0.20 — switch the Pad Stack section's tab (Simple /
    /// Top-Middle-Bottom / Full Stack). UI-only; mutates
    /// `editor.state.pad_stack_tab`.
    FpEditorSetPadStackTab(crate::library::editor::footprint::state::PadStackTab),
    /// v0.21 — Altium-parity Net / Locked / Electrical Type fields
    /// for both placement-defaults and selected-pad targets.
    FpEditorSetNextPadElectricalType(signex_sketch::attr::ElectricalType),
    FpEditorSetNextPadNet(String),
    FpEditorToggleNextPadLocked(bool),
    FpEditorSetSelectedPadElectricalType {
        idx: usize,
        value: signex_sketch::attr::ElectricalType,
    },
    FpEditorSetSelectedPadNet {
        idx: usize,
        value: String,
    },
    FpEditorToggleSelectedPadLocked {
        idx: usize,
        value: bool,
    },
    /// v0.21 — Footprint (component-level) edits.
    FpEditorSetFootprintDescription(String),
    FpEditorSetFootprintDefaultDesignator(String),
    FpEditorSetFootprintComponentType(signex_library::primitive::footprint::ComponentType),
    FpEditorSetFootprintHeight(String),
    /// v0.21 — Selected silk graphic edits (Line + Text only;
    /// Arc/Region/Fill/etc are sketch-mode-authored).
    FpEditorSetSilkLineFromX(String),
    FpEditorSetSilkLineFromY(String),
    FpEditorSetSilkLineToX(String),
    FpEditorSetSilkLineToY(String),
    FpEditorSetSilkTextPositionX(String),
    FpEditorSetSilkTextPositionY(String),
    FpEditorSetSilkTextSize(String),
    FpEditorSetSilkStrokeWidth(String),
    FpEditorToggleSilkFilled(bool),
    /// v0.21 — Pad Hole detail fields (Multi-Layer only).
    FpEditorSetNextPadHoleTolerancePlus(String),
    FpEditorSetNextPadHoleToleranceMinus(String),
    FpEditorSetNextPadHoleRotation(String),
    FpEditorSetNextPadCopperOffsetX(String),
    FpEditorSetNextPadCopperOffsetY(String),
    /// v0.21 — Plated toggle on the Pad Hole row. `true` = THT
    /// (plated), `false` = NPT (non-plated).
    FpEditorToggleNextPadPlated(bool),
    /// v0.21 — Selected-pad hole-detail mirror.
    FpEditorSetSelectedPadHoleTolerancePlus {
        idx: usize,
        value: String,
    },
    FpEditorSetSelectedPadHoleToleranceMinus {
        idx: usize,
        value: String,
    },
    FpEditorSetSelectedPadHoleRotation {
        idx: usize,
        value: String,
    },
    FpEditorSetSelectedPadCopperOffsetX {
        idx: usize,
        value: String,
    },
    FpEditorSetSelectedPadCopperOffsetY {
        idx: usize,
        value: String,
    },
    FpEditorToggleSelectedPadPlated {
        idx: usize,
        value: bool,
    },
    /// v0.21 — Sketch-mode pad attribute edits. Mutate the `PadAttr`
    /// on the selected sketch entity (identified by SketchEntityId)
    /// and re-run solve+bake. Mirrors the new pad fields surfaced in
    /// Pads-mode but addressed by the sketch entity rather than the
    /// flat-pad index.
    FpEditorSetSketchPadElectricalType {
        id: signex_sketch::id::SketchEntityId,
        value: signex_sketch::attr::ElectricalType,
    },
    FpEditorSetSketchPadNet {
        id: signex_sketch::id::SketchEntityId,
        value: String,
    },
    FpEditorToggleSketchPadLocked {
        id: signex_sketch::id::SketchEntityId,
        value: bool,
    },
    FpEditorSetSketchPadTemplate {
        id: signex_sketch::id::SketchEntityId,
        value: String,
    },
    FpEditorSetSketchPadTemplateLibrary {
        id: signex_sketch::id::SketchEntityId,
        value: String,
    },
    FpEditorSetSketchPadFeatureTop {
        id: signex_sketch::id::SketchEntityId,
        value: signex_sketch::attr::PadFeature,
    },
    FpEditorSetSketchPadFeatureBottom {
        id: signex_sketch::id::SketchEntityId,
        value: signex_sketch::attr::PadFeature,
    },
    FpEditorToggleSketchPadTestpointTopAssembly {
        id: signex_sketch::id::SketchEntityId,
        value: bool,
    },
    FpEditorToggleSketchPadTestpointTopFab {
        id: signex_sketch::id::SketchEntityId,
        value: bool,
    },
    FpEditorToggleSketchPadTestpointBottomAssembly {
        id: signex_sketch::id::SketchEntityId,
        value: bool,
    },
    FpEditorToggleSketchPadTestpointBottomFab {
        id: signex_sketch::id::SketchEntityId,
        value: bool,
    },
    FpEditorToggleSketchPadThermalRelief {
        id: signex_sketch::id::SketchEntityId,
        value: bool,
    },
    FpEditorToggleSketchPadMaskTentedTop {
        id: signex_sketch::id::SketchEntityId,
        value: bool,
    },
    FpEditorToggleSketchPadMaskTentedBottom {
        id: signex_sketch::id::SketchEntityId,
        value: bool,
    },
    FpEditorToggleSketchPadPasteEnabledTop {
        id: signex_sketch::id::SketchEntityId,
        value: bool,
    },
    FpEditorToggleSketchPadPasteEnabledBottom {
        id: signex_sketch::id::SketchEntityId,
        value: bool,
    },
    FpEditorSetSketchPadHoleTolerancePlus {
        id: signex_sketch::id::SketchEntityId,
        value: String,
    },
    FpEditorSetSketchPadHoleToleranceMinus {
        id: signex_sketch::id::SketchEntityId,
        value: String,
    },
    FpEditorSetSketchPadHoleRotation {
        id: signex_sketch::id::SketchEntityId,
        value: String,
    },
    FpEditorSetSketchPadCopperOffsetX {
        id: signex_sketch::id::SketchEntityId,
        value: String,
    },
    FpEditorSetSketchPadCopperOffsetY {
        id: signex_sketch::id::SketchEntityId,
        value: String,
    },
    FpEditorSetSketchPadCornerRadiusPct {
        id: signex_sketch::id::SketchEntityId,
        value: String,
    },
    /// v0.21 — "Edit in Sketch" jump from a selected pad to its
    /// backing sketch entity. Switches editor mode to Sketch and
    /// selects the entity. No-op when the pad has no
    /// `sketch_entity_id` (placed before sketch-mode auto-mint).
    FpEditorEditPadInSketch {
        pad_idx: usize,
    },
    /// v0.24 Phase 3 (Track A2) — Properties-panel parametric handle
    /// row edit. The handler looks up `pad.shape_params[key]` to
    /// resolve the bound parameter name, writes `value` into
    /// `sketch.parameters[parameter_name]`, then dispatches a sketch
    /// `ForceRebuild` so the solver re-runs and every entity bound to
    /// that parameter (e.g. all 4 corner Arcs of a RoundRect) updates
    /// in lockstep.
    FpEditorEditPadShapeParam {
        pad_idx: usize,
        key: String,
        value: String,
    },
    /// v0.24 Phase 3 (Track A3) — sketch-canvas right-click action.
    /// "Unlink corner radius" mints a fresh per-corner parameter and
    /// rebinds the clicked Arc to it so the user can override that
    /// one corner independently. The other three corners stay on the
    /// shared `corner_r` parameter. No-op when the Arc isn't part of
    /// any pad's `shape_params` graph.
    FpEditorUnlinkCornerRadius {
        arc_entity_id: signex_sketch::id::SketchEntityId,
    },
    /// v0.22 Phase D6 — Mirror of `FpEditorEditPadInSketch` going the
    /// other direction. From a sketch entity carrying a `PadAttr`,
    /// switch to Pads mode and select the EditorPad whose
    /// `sketch_entity_id` matches this id. No-op when no pad has
    /// this entity as its backing point.
    FpEditorEditSketchPadInPads {
        id: signex_sketch::id::SketchEntityId,
    },
    /// v0.22 Phase E3+E4 — Properties-panel "Conflicts (worst first)"
    /// over-constrained constraint row. Click → select the row's
    /// focus entity in the sketch so the canvas re-renders with the
    /// constraint icon highlighted. The handler dispatches the
    /// equivalent `FootprintSketchSelect` library message.
    FpEditorSelectSketchEntity {
        id: signex_sketch::id::SketchEntityId,
    },
    /// v0.22 Phase 8.5 — Right-dock History panel "Restore this
    /// version" button. The handler resolves the active tab's
    /// owning project, opens `LocalGitProjectAdapter`, and runs
    /// `restore_at(rel_path, oid)` to overwrite the working-tree
    /// file with the historical blob. Marks the file dirty so the
    /// next save commits the restored content.
    HistoryRestoreClicked {
        sha: String,
    },
    /// v0.22 Phase E3+E4 polish — Hover state for the Properties
    /// panel's "Conflicts (worst first)" list. `true` on row
    /// `on_enter`, `false` on `on_exit`. The handler flips
    /// `editor.state.hovered_over_constraint` between
    /// v0.22 Phase E3+E4 → v0.23 — Per-row hover on a Properties
    /// panel "Conflicts" list row. `Some(constraint_id)` highlights
    /// the specific constraint at full red and dims everything else
    /// (including other over-constraints) so the user can isolate a
    /// single offender. `None` clears the isolation back to the
    /// default rendering.
    FpEditorHoverOverConstraint {
        constraint: Option<signex_sketch::id::ConstraintId>,
    },
    /// v0.16.4 — Pour-role sub-form. The handler mutates the
    /// selected entity's `pour` attr and runs solve+bake.
    FpEditorSetPourNet {
        id: signex_sketch::id::SketchEntityId,
        value: String,
    },
    FpEditorSetPourFillType {
        id: signex_sketch::id::SketchEntityId,
        value: signex_sketch::attr::PourFillType,
    },
    FpEditorSetPourPriority {
        id: signex_sketch::id::SketchEntityId,
        value: String,
    },
    /// v0.16.4 — Keepout-role kinds checklist. The handler mutates
    /// the matching `kinds.<flag>` and runs solve+bake.
    FpEditorSetKeepoutKind {
        id: signex_sketch::id::SketchEntityId,
        kind: KeepoutKindFlag,
        value: bool,
    },
    /// v0.16.4 — BoardCutout-role edge-radius expression input.
    FpEditorSetCutoutEdgeRadius {
        id: signex_sketch::id::SketchEntityId,
        value: String,
    },
    /// v0.16.4 — BoardCutout-role through-vs-partial-depth toggle.
    FpEditorSetCutoutThrough {
        id: signex_sketch::id::SketchEntityId,
        value: bool,
    },
    /// v0.23 — Pattern Properties sub-form text-input edit. The
    /// handler walks `sketch.arrays`, finds the array with `array_id`,
    /// mutates the field identified by `field`, then runs
    /// `SketchEdit::ForceRebuild` so the bake re-expands.
    FpEditorEditArrayParam {
        array_id: signex_sketch::array::ArrayId,
        field: ArrayParamField,
        value: String,
    },
    /// v0.23 — Switch the numbering scheme on an array. The handler
    /// preserves the existing inner fields when possible (LinearIncrement
    /// keeps prior start/step exprs; flipping to Explicit clears the
    /// names list).
    FpEditorSetArrayNumberingScheme {
        array_id: signex_sketch::array::ArrayId,
        scheme: NumberingSchemeKindUi,
    },
    /// v0.25 polish — toggle BGA `skip_letters`. Active only when the
    /// array's numbering is BgaRowCol; ignored for Linear / Explicit.
    FpEditorSetBgaSkipLetters {
        array_id: signex_sketch::array::ArrayId,
        value: bool,
    },
    /// v0.25 polish — set BGA `start_row` letter. Empty input no-ops;
    /// non-letter input no-ops; multi-char input takes the first
    /// letter. Uppercased before storage.
    FpEditorSetBgaStartRow {
        array_id: signex_sketch::array::ArrayId,
        value: String,
    },
    /// v0.25 polish — set BGA `start_col` integer. Empty input no-ops;
    /// non-numeric input no-ops; bounds are otherwise unconstrained.
    FpEditorSetBgaStartCol {
        array_id: signex_sketch::array::ArrayId,
        value: String,
    },
    /// v0.23 — Delete the array entirely. The source entity stays put.
    FpEditorDeleteArray {
        array_id: signex_sketch::array::ArrayId,
    },
    /// v0.23 — Begin re-picking the polar centre. Sets
    /// `ToolPending::RepickPolarCenter { array_id }` so the next sketch
    /// click on a Point overwrites `array.center`. Cancels with Esc.
    FpEditorBeginRepickPolarCenter {
        array_id: signex_sketch::array::ArrayId,
    },
    /// v0.23 — Toggle a single (i, j) instance in a Grid array's
    /// `GridDepopulation.suppressed_instances`. `value=true` re-enables
    /// the instance; `value=false` suppresses it.
    FpEditorToggleArrayInstance {
        array_id: signex_sketch::array::ArrayId,
        i: u32,
        j: u32,
        value: bool,
    },
    /// v0.17.0 — empty-canvas Snap Options toggles. The handler
    /// flips the matching `SnapOptions` flag.
    FpEditorToggleSnapOption(SnapOptionFlag),
    /// v0.18.9 — author-controlled snap grid step (mm). The handler
    /// parses the string and writes
    /// `state.snap_options.grid_step_mm`. Invalid / empty strings
    /// no-op so the input doesn't fight intermediate keystrokes.
    FpEditorSetSnapGridStep(String),
    /// v0.13 — Altium "Snap Distance" numeric input.
    FpEditorSetSnapDistance(String),
    /// v0.13 — Altium "Axis Snap Range" numeric input.
    FpEditorSetAxisSnapRange(String),
    /// v0.13 — Properties panel rename for the active internal
    /// footprint. Routes to `editor.primitive_mut().name`.
    FpEditorSetFootprintName(String),
    /// v0.18.13 — Altium Selection Filter pill toggle. The pills
    /// live on the v0.18.14 unified active bar; the Properties
    /// panel surfaces the toggle through this same message.
    FpEditorToggleSelectionFilter(crate::library::editor::footprint::state::SelectionFilterKind),
    /// v0.18.13 — open the Custom Selection Filter modal. Stubbed
    /// until v0.18.14 ships the modal body alongside the active
    /// bar pills.
    FpEditorOpenSelectionFilterCustom,
    /// v0.18.13 — Snap Options sub-tab switch (Grids / Guides / Axes).
    FpEditorSetSnapSubTab(crate::library::editor::footprint::state::SnapSubTab),
    /// v0.18.13 — Snapping mode 3-state toggle.
    FpEditorSetSnappingMode(crate::library::editor::footprint::state::SnappingMode),
    /// v0.18.13 — `Add` button on the Grid Manager table (placeholder
    /// for the multi-grid CRUD that lands with the v0.18.14 grid
    /// system). Dispatch logs a warn for now.
    FpEditorGridManagerAdd,
    /// v0.18.13 — `Properties` button on the Grid Manager row;
    /// reuses the Ctrl+G Cartesian Grid Editor modal.
    FpEditorGridManagerProperties,
    /// v0.18.13 — `Delete` button on the Grid Manager row
    /// (placeholder until multi-grid CRUD lands).
    FpEditorGridManagerDelete,
    /// v0.18.21 — Activate the row at the given index. Mirrors the
    /// row's step / display style / multiplier onto `snap_options` so
    /// the canvas + snap logic switch to the new grid.
    FpEditorGridSetActive(usize),
    /// v0.18.24 — Edit the `content` field of a selected silk-front
    /// `FpGraphicKind::Text { content, .. }` entry. The dispatcher
    /// finds `editor.state.selected_silk_f` and mutates the matching
    /// `silk_f[idx]` if it's a Text. No-op for non-Text selections.
    FpEditorSetSilkText(String),
    /// v0.18.24 — Delete the selected silk-front graphic. Mirrors the
    /// existing `FootprintEditorMsg::DeleteSilkF` surface but
    /// is emitted from the Properties panel's silk-selection branch.
    FpEditorDeleteSelectedSilk,
    /// v0.18.13 — `Add` button on the Guide Manager table
    /// (placeholder until guide system lands).
    FpEditorGuideManagerAdd,
    /// v0.18.20 — `Add Vertical` button on the Guide Manager footer.
    /// Appends a new vertical guide at world X = 0 mm.
    FpEditorGuideAddVertical,
    /// v0.18.20 — `Add Horizontal` button on the Guide Manager footer.
    /// Appends a new horizontal guide at world Y = 0 mm.
    FpEditorGuideAddHorizontal,
    /// v0.18.20 — Per-row delete button on the Guide Manager. Removes
    /// the guide at the given index.
    FpEditorGuideDelete(usize),
    /// v0.18.20 — Per-row enabled toggle on the Guide Manager. Flips
    /// `guides[idx].enabled` so the user can hide individual guides
    /// without deleting them.
    FpEditorGuideToggle(usize),
    /// v0.18.20 — Per-row position edit on the Guide Manager. The text
    /// input emits the raw string; the dispatcher parses to f64 and
    /// no-ops on invalid input so intermediate keystrokes don't fight
    /// the user.
    FpEditorGuideSetPosition(usize, String),
    /// v0.14.2 — open a sibling `.snxfpt` from the Footprint Library
    /// panel. The handler routes through the existing
    /// `handle_open_primitive` flow so the file gets a fresh tab + a
    /// `FootprintEditorState` (or activates an existing tab).
    FpLibraryOpenSibling(std::path::PathBuf),
    /// v0.18.8 — Footprint Library panel single-click on an internal
    /// footprint row. Sets `panel_selected_idx` (independent of
    /// `active_idx`) so the row highlights and the bottom button
    /// row gates Place / Delete / Edit on it.
    FpLibrarySelectInternal(usize),
    /// v0.18.8 — `+ Add` button: append an empty `Footprint` to the
    /// active envelope and switch onto it. Routes through the
    /// existing `FootprintAddNewSibling` handler.
    FpLibraryAddInternal,
    /// v0.18.8 — `Delete` button: remove the selected internal
    /// footprint from the envelope. The active footprint clamps to
    /// the new last index when the deleted row was the active one.
    FpLibraryDeleteInternal(usize),
    /// v0.18.8 — `Edit` button (also fires on row double-click):
    /// promote `panel_selected_idx` to `active_idx` so the canvas
    /// switches to the selected sibling.
    FpLibraryEditInternal(usize),
    /// v0.18.8 — `Place` button: place the selected internal
    /// footprint as a Component on the active PCB. Stubbed until
    /// the PCB integration lands; for now no-op + tracing-warn.
    FpLibraryPlaceInternal(usize),
    /// Clear the current ERC violations list and canvas markers.
    ClearErc,
    /// Focus a specific ERC diagnostic row from the global flattened list.
    FocusErcViolation(usize),
    /// Focus previous ERC diagnostic row in the global list.
    FocusPrevErcDiagnostic,
    /// Focus next ERC diagnostic row in the global list.
    FocusNextErcDiagnostic,
    /// User clicked the Quick Fix chip on an ERC violation row. Routes
    /// to a per-rule handler — UnusedPin places a NoConnect at the
    /// pin, every other rule falls back to "zoom + select" (same as
    /// clicking the row body).
    ErcQuickFix(usize),
    ToggleGrid,
    ToggleSnap,
    PropertiesTab(usize),
    SelectLibrary(String),
    SelectComponent(String),
    DragComponentsSplit,
    ComponentFilter(String),
    /// F15 — Properties panel: open the primitive picker (symbol /
    /// footprint) for the row described by the active
    /// `PanelContext.library_row_detail`. Routes to
    /// `LibraryMessage::OpenPrimitivePicker` with a
    /// `PrimitivePickerTarget::BrowserRow` so the pick applies +
    /// persists through the existing adapter path.
    LibraryRowPickSymbol,
    LibraryRowPickFootprint,
    /// Toggle a collapsible section (by section key).
    ToggleSection(String),
    /// Edit a symbol's designator (committed on submit).
    EditSymbolDesignator(uuid::Uuid, String),
    /// Edit a symbol's value (committed on submit).
    EditSymbolValue(uuid::Uuid, String),
    /// Edit a symbol's footprint (committed on submit).
    EditSymbolFootprint(uuid::Uuid, String),
    /// Toggle a symbol's mirror_x.
    ToggleSymbolMirrorX(uuid::Uuid),
    /// Toggle a symbol's mirror_y.
    ToggleSymbolMirrorY(uuid::Uuid),
    /// Toggle a symbol's locked state.
    ToggleSymbolLocked(uuid::Uuid),
    /// Toggle a symbol's DNP state.
    ToggleSymbolDnp(uuid::Uuid),
    /// Set absolute rotation on a symbol (degrees).
    EditSymbolRotation(uuid::Uuid, f64),
    /// Set font size (Altium pt) on a symbol's value text property.
    EditSymbolValueFontSizePt(uuid::Uuid, u32),
    /// Change the lib_id of a symbol (used by power-port Style dropdown).
    EditSymbolLibId(uuid::Uuid, String),
    /// Swap a power-port's style: change lib_id and preserve visual direction
    /// by setting rotation accordingly.
    EditPowerPortStyle {
        symbol_id: uuid::Uuid,
        new_lib_id: String,
        rotation_degrees: f64,
    },
    /// Edit a label's text (committed on submit).
    EditLabelText(uuid::Uuid, String),
    /// Edit a label's horizontal justification.
    EditLabelJustifyH(uuid::Uuid, signex_types::schematic::HAlign),
    /// Edit a label direction preset (rotation + horizontal justify).
    EditLabelDirection(uuid::Uuid, f64, signex_types::schematic::HAlign),
    /// Edit a label's rotation (degrees).
    EditLabelRotation(uuid::Uuid, f64),
    /// Edit a label's font size in Altium pt (10 = 2.54 mm).
    EditLabelFontSizePt(uuid::Uuid, u32),
    /// Edit a text note's text (committed on submit).
    EditTextNoteText(uuid::Uuid, String),
    /// Pre-placement: update label/text field.
    SetPrePlacementText(String),
    /// Pre-placement: update designator field.
    SetPrePlacementDesignator(String),
    /// Pre-placement: update rotation.
    SetPrePlacementRotation(f64),
    /// Pre-placement: update font family.
    SetPrePlacementFont(String),
    /// Pre-placement: update font size (pt).
    SetPrePlacementFontSize(u32),
    /// Pre-placement: set horizontal justification.
    SetPrePlacementJustifyH(signex_types::schematic::HAlign),
    /// Pre-placement: set vertical justification.
    SetPrePlacementJustifyV(signex_types::schematic::VAlign),
    /// Pre-placement: toggle bold / italic / underline.
    TogglePrePlacementBold,
    TogglePrePlacementItalic,
    TogglePrePlacementUnderline,
    SetPrePlacementShapeWidth(f64),
    SetPrePlacementShapeFill(signex_types::schematic::FillType),
    /// Properties panel — edit a pin's designator (number) on the
    /// active Symbol editor tab. Routed through `handle_dock_sch_library_message`
    /// because the symbol editor's lifecycle owns the pin edits.
    SymEditorSetPinNumber {
        pin_idx: usize,
        value: String,
    },
    /// Properties panel — edit a pin's display name.
    SymEditorSetPinName {
        pin_idx: usize,
        value: String,
    },
    /// Properties panel — edit a pin's stub length in mm.
    SymEditorSetPinLength {
        pin_idx: usize,
        value: f64,
    },
    /// Properties panel — set a pin's electrical type from the
    /// Altium-spec dropdown (Input / Output / Bidirectional / Power
    /// / Passive / Open Collector / Open Emitter / Tri-state /
    /// Not Connected / Unspecified).
    SymEditorSetPinElectrical {
        pin_idx: usize,
        value: signex_library::PinDirection,
    },
    /// Properties panel — set a pin's orientation (Right / Up /
    /// Left / Down). Also updates the canvas cache so the pin
    /// re-renders.
    SymEditorSetPinOrientation {
        pin_idx: usize,
        value: signex_library::PinOrientation,
    },
    /// Properties panel — set a pin's X coordinate in mm.
    SymEditorSetPinX {
        pin_idx: usize,
        value: f64,
    },
    /// Properties panel — set a pin's Y coordinate in mm.
    SymEditorSetPinY {
        pin_idx: usize,
        value: f64,
    },
    /// SCH Library panel — click on a row in the Pins sub-list.
    /// Selects the pin on the canvas (Properties panel switches
    /// to pin-mode automatically via the next refresh_panel_ctx).
    SymEditorSelectPin(usize),
    /// Properties panel — edit a pin's free-text Description.
    SymEditorSetPinDescription {
        pin_idx: usize,
        value: String,
    },
    /// Properties panel — edit a pin's Function list (alt-names) as
    /// a single comma-separated string. Persisted as `Vec<String>`
    /// after splitting + trimming on commit.
    SymEditorSetPinFunctionCsv {
        pin_idx: usize,
        value: String,
    },
    /// Properties panel — toggle a pin's designator visibility.
    SymEditorTogglePinDesignatorVisible(usize),
    /// Properties panel — toggle a pin's name visibility.
    SymEditorTogglePinNameVisible(usize),
    /// Properties panel — toggle Pin Hide.
    SymEditorTogglePinHidden(usize),
    /// Properties panel — toggle Pin Locked.
    SymEditorTogglePinLocked(usize),
    /// Properties panel — set one of the four IEEE-symbol slots on
    /// a pin. `slot` is 0=Inside, 1=InsideEdge, 2=OutsideEdge, 3=Outside.
    SymEditorSetPinSymbol {
        pin_idx: usize,
        slot: u8,
        value: signex_library::PinSymbolKind,
    },
    /// Properties panel — set a pin's multi-part scope (Altium
    /// "Part Number" spinner). `0` is the special Part Zero (pin
    /// appears on every part); `1..=N` scopes to a specific part.
    SymEditorSetPinPartNumber {
        pin_idx: usize,
        value: u8,
    },
    /// SCH Library panel: select a placed graphic from the active
    /// symbol's `graphics` vector. Fires the same selection state as
    /// a canvas click on a graphic body.
    SymEditorSelectGraphic(usize),
    /// SCH Library panel: switch the editor's `active_part` to the
    /// given value. `0` selects Part Zero (shared pins). The
    /// dispatcher clamps to `[0, active_max_part]` so a stale tree
    /// click can't park `active_part` outside the symbol's range.
    SymEditorSelectPart(u8),
    /// Properties panel — set one numeric field of the placed graphic
    /// at `idx`. The dispatcher routes `field` to the matching
    /// `SymbolGraphicKind` variant; mismatched (idx, field) pairs
    /// silently no-op so an out-of-date Properties panel can't
    /// corrupt geometry.
    SymEditorSetGraphicField {
        idx: usize,
        field: GraphicFieldId,
        value: f64,
    },
    /// Properties panel — set the text content of a placed
    /// `SymbolGraphicKind::Text` at `idx`. No-op for other kinds.
    SymEditorSetGraphicText {
        idx: usize,
        value: String,
    },
    /// Properties panel — open / close the fill colour-picker for the
    /// placed graphic at `idx` (swatch click). Opening one picker closes
    /// any other. UI-only; no dirty.
    SymEditorToggleGraphicFillPicker {
        idx: usize,
    },
    /// Properties panel — expand the open graphic-fill picker into the
    /// HSV / RGB overlay (`Custom…` click). UI-only; no dirty.
    SymEditorOpenGraphicFillAdvanced {
        idx: usize,
    },
    /// Properties panel — cancel / close the graphic-fill picker without
    /// committing. UI-only; no dirty.
    SymEditorCancelGraphicFillPicker,
    /// Properties panel — set the fill colour of the placed graphic at
    /// `idx` (preset cell or HSV submit) and close the picker. Dirties.
    SymEditorSetGraphicFill {
        idx: usize,
        color: [u8; 4],
    },
    /// Properties panel — clear the placed graphic's fill (back to
    /// unfilled) and close the picker. Dirties.
    SymEditorClearGraphicFill {
        idx: usize,
    },
    /// Properties panel — edit the active symbol's name (Altium
    /// "Design Item ID"). Affects the SCH Library panel row label
    /// + the on-disk container's `display_name` when the active
    /// symbol is the only one in the file.
    SymEditorSetSymbolName(String),
    /// Properties panel — edit the active symbol's designator
    /// template (Altium "Designator", e.g. `U?`).
    SymEditorSetSymbolDesignator(String),
    /// Properties panel — edit the active symbol's comment
    /// passthrough.
    SymEditorSetSymbolComment(String),
    /// Properties panel — edit the active symbol's free-text
    /// description.
    SymEditorSetSymbolDescription(String),
    /// Properties panel — pick the active symbol's Component Type.
    SymEditorSetSymbolType(signex_library::ComponentType),
    /// Properties panel — toggle the active symbol's mirrored flag.
    SymEditorToggleSymbolMirrored,
    /// Properties panel — open / close a symbol-level local-colour
    /// picker (Fills / Lines / Pins). Opening one closes any other.
    /// UI-only; no dirty.
    SymEditorToggleLocalColorPicker(crate::app::LocalColorSlot),
    /// Properties panel — expand the open local-colour picker into the
    /// HSV / RGB overlay (`Custom…` click). UI-only; no dirty.
    SymEditorOpenLocalColorAdvanced(crate::app::LocalColorSlot),
    /// Properties panel — cancel / close the local-colour picker
    /// without committing. UI-only; no dirty.
    SymEditorCancelLocalColorPicker,
    /// Properties panel — set a symbol-level local colour (preset cell
    /// or HSV submit) and close the picker. Dirties.
    SymEditorSetLocalColor {
        slot: crate::app::LocalColorSlot,
        color: [u8; 4],
    },
    /// Properties panel — clear a symbol-level local colour (back to
    /// inherit) and close the picker. Dirties.
    SymEditorClearLocalColor(crate::app::LocalColorSlot),
    /// Document Options (Properties pane when nothing is selected)
    /// — set the sheet background color preset on the containing
    /// `.snxlib`. All `.snxsym` tabs from the same library share.
    SymEditorSetDisplaySheetColor(SheetColor),
    /// Document Options — toggle the visible dot grid on the
    /// containing `.snxlib`.
    SymEditorToggleDisplayGrid,
    /// Document Options — cycle the visible grid spacing through
    /// `crate::canvas::grid::GRID_SIZES_MM`.
    SymEditorCycleDisplayGridSize,
    /// Document Options — cycle the coordinate display unit on
    /// the containing `.snxlib` (mm → mil → inch → um → mm).
    SymEditorCycleDisplayUnit,
    /// SCH Library panel: switch the active symbol within the open
    /// `.snxsym` container to the given index.
    SchLibrarySelectSymbol(usize),
    /// SCH Library panel: append a new empty symbol to the open
    /// container and make it active. Caller emits a default name —
    /// the user renames via the Properties panel.
    SchLibraryAddSymbol,
    /// SCH Library panel: delete the symbol at the given index from
    /// the open container. Refuses to delete the last remaining
    /// symbol — the file would be empty otherwise.
    SchLibraryDeleteSymbol(usize),
    UpdateDrawingEdit(crate::app::contracts::DrawingFieldEdit),
    /// Numeric text_input keystroke for a drawing field. The string
    /// is stored verbatim in panel_ctx.drawing_edit_buf so empty /
    /// partial input survives between frames; the handler parses
    /// best-effort and fires UpdateDrawingEdit when the value is a
    /// valid f64.
    DrawingFieldTyping(DrawingFieldId, String),
    /// Open / close the border-colour picker overlay for a child sheet.
    ToggleChildSheetBorderPicker(uuid::Uuid),
    /// Open / close the fill-colour picker overlay for a child sheet.
    ToggleChildSheetFillPicker(uuid::Uuid),
    /// Expand the currently-open child-sheet picker dropdown into the
    /// full HSV / RGB ColorPicker overlay. `is_border` selects which
    /// channel (border vs fill).
    OpenChildSheetAdvancedPicker(uuid::Uuid, bool),
    /// Cancel the currently-open child-sheet colour picker without
    /// committing a new value.
    CancelChildSheetColorPicker,
    /// Commit a new border colour for a child sheet (closes the picker).
    EditChildSheetBorderColor(uuid::Uuid, iced::Color),
    /// Commit a new fill colour for a child sheet (closes the picker).
    EditChildSheetFillColor(uuid::Uuid, iced::Color),
    /// Buffered keystroke for the child sheet stroke-width input.
    ChildSheetStrokeWidthTyping(uuid::Uuid, String),
    /// Commit the currently-buffered child sheet stroke width.
    CommitChildSheetStrokeWidth(uuid::Uuid),
    /// Reset child sheet styling (border / fill colour, line width)
    /// back to theme defaults.
    ResetChildSheetStyle(uuid::Uuid),
    /// Pre-placement: confirm and close.
    ConfirmPrePlacement,
    /// Set snap grid size (mm).
    SetGridSize(f32),
    /// Set visible grid size (mm) — independent of snap grid.
    SetVisibleGridSize(f32),
    /// Toggle snap to electrical object hotspots.
    ToggleSnapHotspots,
    /// Change the UI font (saved to prefs; applies on next restart).
    #[allow(dead_code)]
    SetUiFont(String),
    /// Change the canvas font (applied immediately to schematic/PCB text).
    SetCanvasFont(String),
    /// Change canvas font size (px) applied to canvas text rendering.
    SetCanvasFontSize(f32),
    /// Toggle canvas font bold style.
    SetCanvasFontBold(bool),
    /// Toggle canvas font italic style.
    SetCanvasFontItalic(bool),
    /// Open canvas font popup.
    OpenCanvasFontPopup,
    /// Close canvas font popup.
    CloseCanvasFontPopup,
    /// Set page margin vertical zones.
    SetMarginVertical(u32),
    /// Set page margin horizontal zones.
    SetMarginHorizontal(u32),
    /// Toggle a single selection filter — shared with the Active Bar.
    ToggleSelectionFilter(crate::active_bar::SelectionFilter),
    /// Toggle all selection filters on/off — shared with the Active Bar.
    ToggleAllSelectionFilters,
    /// Append a new empty custom filter preset (no-op when at the cap).
    AddCustomFilterPreset,
    /// Remove the preset at this index.
    RemoveCustomFilterPreset(usize),
    /// Rename the preset at this index.
    RenameCustomFilterPreset(usize, String),
    /// Toggle whether the preset at `idx` includes `filter`.
    ToggleCustomFilterPresetMember(usize, crate::active_bar::SelectionFilter),
    /// Snapshot the active selection filter set into the preset at `idx`.
    CaptureCustomFilterPreset(usize),
    /// Switch the Properties-panel preset editor to the given tab.
    SelectCustomFilterTab(usize),
    /// Page Options: choose formatting mode.
    SetPageFormatMode(PageFormatMode),
    /// Page Options: choose paper size.
    SetPaperSize(String),
    /// Page Options: choose origin corner.
    SetPageOrigin(PageOrigin),
    /// Page Options: set custom paper width (mm).
    SetCustomPaperWidth(f32),
    /// Page Options: set custom paper height (mm).
    SetCustomPaperHeight(f32),
    /// Page Options: choose sheet background colour.
    SetSheetColor(SheetColor),
    /// No-op placeholder for unimplemented UI controls.
    Noop,
}

