//! Footprint-editor panel context and its summary view-models.


/// Context handed to the Properties panel when a `.snxfpt` editor
/// tab is active. Mirrors a small read-only slice of the live
/// `FootprintEditorState` — the panel never mutates this, edits flow
/// back through `LibraryMessage::PrimitiveEditorEvent` like every
/// other primitive editor.
#[derive(Debug, Clone)]
pub struct FootprintEditorPanelContext {
    /// Open `.snxfpt` path (the tab key).
    pub path: std::path::PathBuf,
    /// `Footprint::name` — surfaced as the panel header.
    pub footprint_name: String,
    /// `Footprint::version` — semver-style revision string.
    pub version: String,
    /// Current editor mode — drives the Properties panel body branch.
    pub mode_kind: FootprintModeKind,
    /// Number of baked pads on the footprint primitive.
    pub pad_count: usize,
    /// Number of sketch entities (Points + Lines + Arcs + Circles)
    /// when a sketch is present.
    pub sketch_entity_count: usize,
    /// Number of sketch constraints when a sketch is present.
    pub sketch_constraint_count: usize,
    /// Number of free DoF the most recent solve reported, plus
    /// elapsed_ms. `None` if no solve has run yet.
    pub last_solve: Option<FootprintSolveSummary>,
    /// Read-only summary of the selected pad — populated when in
    /// Pads mode and a pad is selected.
    pub selected_pad: Option<FootprintPadSummary>,
    /// v0.27 — total number of selected pads (primary + extras).
    /// Drives the multi-select "(N selected)" indicator in the
    /// Properties panel header. `1` for a single-select; `> 1` when
    /// the rubber band picked multiple pads or All-on-Layer / All
    /// fired.
    pub selected_pad_count: usize,
    /// Read-only summary of the primary selected sketch entity —
    /// populated when in Sketch mode and an entity is selected.
    pub selected_sketch_entity: Option<FootprintSketchEntitySummary>,
    /// Mirrors `editor.state.auto_fit_courtyard`. v0.14.2 — drives
    /// the Properties panel's Auto-fit Courtyard toggle (the
    /// equivalent active-bar button stays for quick access).
    pub auto_fit_courtyard: bool,
    /// v0.14.2 — every `.snxfpt` sibling inside the containing
    /// `.snxlib`'s `footprints/` directory. Drives the new Footprint
    /// Library panel (mirror of SCH Library). Empty when the active
    /// footprint lives outside any mounted library.
    pub library_siblings: Vec<FootprintLibSibling>,
    /// Display name of the containing `.snxlib` (file stem) — used
    /// in the Footprint Library panel breadcrumb. `None` for
    /// lone-file edits.
    pub library_stem: Option<String>,
    /// v0.18.8 — every footprint inside the active multi-footprint
    /// `.snxfpt` envelope. Mirror of one row in Altium's PCB Library
    /// panel. Drives the Footprint Library panel's primary list.
    pub internal_footprints: Vec<FootprintLibInternalRow>,
    /// v0.18.8 — single-click selection within the panel's internal
    /// footprint list. `None` until the user clicks a row. Drives
    /// the Place / Delete / Edit button enable state.
    pub internal_selected_idx: Option<usize>,
    /// v0.16.2 — Sketch parameter table (name → expression). Drives
    /// the Properties panel's Parameters section in Sketch mode.
    /// Empty when the footprint has no `SketchData` yet.
    pub sketch_parameters: Vec<(String, String)>,
    /// v0.16.2 — solver warnings from the most recent solve + bake.
    /// Drives the Properties panel's "Solve warnings" section.
    pub solve_warnings: Vec<String>,
    /// v0.16.2 — sketch-entity ID of the primary selection. Wired
    /// through so the Properties panel's Role pick_list can emit
    /// `FootprintSketchSetRole` with the right id.
    pub selected_sketch_entity_id: Option<signex_sketch::id::SketchEntityId>,
    /// v0.16.2 — current role of the primary selected sketch entity
    /// (or `Unassigned` when no entity is selected). Inspected via
    /// `current_role_of` against the entity's `*Attr` slots.
    pub selected_sketch_role: crate::library::messages::RoleTag,
    /// v0.16.2 — `true` when the primary selected sketch entity is a
    /// Point. Drives the "Pad role applies to Points only" hint on
    /// the Role pick_list.
    pub selected_sketch_is_point: bool,
    /// v0.16.3 — `true` when the Pads-mode tool is `PlacePad`. Drives
    /// visibility of the "Pad placement defaults" form on the
    /// Properties panel (designator override + size_x / size_y / side).
    pub placement_active: bool,
    /// v0.16.3 — `true` when TAB has paused click-publish during pad
    /// placement. Adds a "PAUSED — TAB to resume" hint to the form.
    pub placement_paused: bool,
    /// v0.16.3 — designator override for the next placed pad. `None`
    /// = use the auto-incrementing numeric designator.
    pub next_pad_designator_override: Option<String>,
    /// v0.16.3 — size_x of the next placed pad in mm.
    pub next_pad_size_x_mm: f64,
    /// v0.16.3 — size_y of the next placed pad in mm.
    pub next_pad_size_y_mm: f64,
    /// v0.16.3 — copper side for the next placed pad.
    pub next_pad_side: crate::library::editor::footprint::state::PadSide,
    /// v0.16.6 — rotation in degrees (CCW positive) for the next
    /// placed pad. Persists through `EditorPad.rotation_deg` →
    /// `Pad::rotation` so the saved file carries the value.
    pub next_pad_rotation_deg: f64,
    /// v0.20 — Altium-parity pad-stack defaults for the next placed
    /// pad. Mirrors `editor.state.next_pad_defaults.stack` for the
    /// Properties panel "Pad Stack" section. UI mutates these via
    /// `FpEditorSetNextPad*` messages; the dispatcher writes the
    /// value back so `add_pad_at` picks it up on the next click.
    pub next_pad_stack: crate::library::editor::footprint::state::PadStackUi,
    /// v0.20 — pad shape for the next placed pad (read out of
    /// `EditorPad::shape` after mint). Defaults to `Rect`.
    pub next_pad_shape: signex_library::PadShape,
    /// v0.20 — drill diameter for the next placed pad in mm. `None`
    /// = SMD pad (no hole). Drives the HOLE → Hole size row.
    pub next_pad_drill_diameter_mm: Option<f64>,
    /// v0.20 — drill slot length for the next placed pad in mm.
    /// `None` = round drill; `Some(l)` = oval slot of length l.
    /// Drives the HOLE → Shape pick_list (Round vs Slot) and the
    /// Slot length input visibility.
    pub next_pad_drill_slot_length_mm: Option<f64>,
    /// v0.20 — pad-template name for the next placed pad. Empty =
    /// no template.
    pub next_pad_template: String,
    /// v0.20 — pad-template library reference for the next placed
    /// pad. Empty = local.
    pub next_pad_template_library: String,
    /// v0.20 — top-side surface feature for the next placed pad.
    pub next_pad_feature_top: signex_sketch::attr::PadFeature,
    /// v0.20 — bottom-side surface feature for the next placed pad.
    pub next_pad_feature_bottom: signex_sketch::attr::PadFeature,
    /// v0.20 — test-point participation flags for the next placed
    /// pad. All `false` = not a test point.
    pub next_pad_testpoint: signex_sketch::attr::TestpointFlags,
    /// v0.20 — currently-active Pad Stack tab (Simple / Top-Middle-
    /// Bottom / Full Stack). Drives which body the Pad Stack section
    /// renders. UI-only state; not persisted to disk.
    pub pad_stack_tab: crate::library::editor::footprint::state::PadStackTab,
    /// v0.21 — Altium-parity electrical-type for the next placed pad.
    pub next_pad_electrical_type: signex_sketch::attr::ElectricalType,
    /// v0.21 — net assignment for the next placed pad.
    pub next_pad_net: String,
    /// v0.21 — locked flag for the next placed pad.
    pub next_pad_locked: bool,
    /// v0.21 — Pad mounting kind for the next placed pad.
    pub next_pad_kind: signex_library::PadKind,
    /// v0.21 — Altium-parity component-level fields. Surface in the
    /// empty-canvas Footprint summary form.
    pub footprint_description: String,
    pub footprint_default_designator: String,
    pub footprint_component_type: signex_library::primitive::footprint::ComponentType,
    pub footprint_height_mm: Option<f64>,
    /// v0.21 — Pad Hole detail fields surfaced for the Multi-Layer
    /// pad placement form.
    pub next_pad_hole_tolerance_plus_mm: Option<f64>,
    pub next_pad_hole_tolerance_minus_mm: Option<f64>,
    pub next_pad_hole_rotation_deg: Option<f64>,
    pub next_pad_copper_offset_x_mm: Option<f64>,
    pub next_pad_copper_offset_y_mm: Option<f64>,
    /// v0.16.4 — Pour-role sub-form values for the selected entity.
    /// `None` when the entity isn't a pour. Carries the net string,
    /// fill type, and a snapshot of thermal-relief defaults so the
    /// panel can show + edit each field.
    pub selected_pour: Option<PourSummary>,
    /// v0.16.4 — Keepout-role sub-form values for the selected
    /// entity. `None` when the entity isn't a keepout.
    pub selected_keepout: Option<KeepoutSummary>,
    /// v0.16.4 — BoardCutout-role sub-form values for the selected
    /// entity. `None` when the entity isn't a board cutout.
    pub selected_cutout: Option<CutoutSummary>,
    /// v0.21 — pad-role sub-form values for the selected sketch
    /// entity. `Some` when the entity carries a `PadAttr`.
    pub selected_sketch_pad: Option<SketchPadAttrSummary>,
    /// v0.17.0 — empty-canvas Snap Options. Surfaced on the
    /// Properties panel default branch (no selection) so the user
    /// can toggle each priority chain step.
    pub snap_options: crate::library::editor::footprint::state::SnapOptions,
    /// v0.18.13 — Altium 5-section Properties layout state.
    pub selection_filter: crate::library::editor::footprint::state::SelectionFilter,
    pub snap_subtab: crate::library::editor::footprint::state::SnapSubTab,
    pub snapping_mode: crate::library::editor::footprint::state::SnappingMode,
    /// v0.18.20 — Altium-style guide lines visible to the Guide
    /// Manager UI table. Cloned out of `editor.state.guides` so the
    /// panel can iterate them without holding a borrow into the
    /// document state.
    pub guides: Vec<crate::library::editor::footprint::state::Guide>,
    /// v0.18.21 — Altium grid table rows. Cloned out of
    /// `editor.state.grids`; the active row is `active_grid_idx`.
    pub grids: Vec<crate::library::editor::footprint::state::GridDef>,
    /// v0.18.21 — index into `grids` of the active row (mirror of
    /// `editor.state.active_grid_idx`).
    pub active_grid_idx: usize,
    /// v0.18.24 — selected silk-front graphic summary. `None` when
    /// no silk graphic is selected; the Properties panel only renders
    /// the silk-selection branch when this is `Some`.
    pub selected_silk_summary: Option<FootprintSelectedSilkSummary>,
    /// v0.25 polish — verbatim per-input buffers for Properties-panel
    /// numeric fields. Renderer reads `numeric_buffers.get(key)` and
    /// uses the literal buffer if present; falls back to formatting
    /// the canonical f64 otherwise. See
    /// [`crate::library::editor::footprint::state::FootprintEditorState::numeric_buffers`]
    /// for the buffer contract.
    pub numeric_buffers: std::collections::HashMap<String, String>,
    /// v0.23 — Array (Pattern) summary surfaced when the selected
    /// sketch entity is the `source` of an array. Drives the
    /// Properties panel "Pattern" sub-section.
    pub selected_array: Option<ArraySummary>,
    /// v0.24 Phase 3 (Track A2) — parametric handle summary for the
    /// selected pad. Empty when the pad has no `shape_params` bindings
    /// (e.g. legacy pads minted before v0.24, or pads whose shape has
    /// no parametric handles like Rect / Oval). One entry per
    /// (feature_key → parameter) binding; the Properties panel
    /// renders one editable row per entry so the user can edit the
    /// shared sketch parameter without entering Sketch mode.
    pub selected_pad_shape_params: Vec<PadShapeParamSummary>,
}

/// v0.24 Phase 3 (Track A2) — surface entry for one parametric pad
/// handle. Carries the feature key (e.g. `"corner_r"`, `"diameter"`),
/// the resolved sketch parameter name, the current expression string
/// (read out of `sketch.parameters`), and a UI label so the
/// Properties panel can render a localised row label without each
/// editor having to repeat the mapping.
#[derive(Debug, Clone)]
pub struct PadShapeParamSummary {
    /// Feature key as stored on `EditorPad.shape_params`
    /// (e.g. `"corner_r"`, `"diameter"`).
    pub key: String,
    /// Display label for the Properties panel row.
    pub label: String,
    /// Resolved parameter name in `sketch.parameters` (the value
    /// stored under `key` in `pad.shape_params`).
    pub parameter_name: String,
    /// Current expression string (e.g. `"0.25mm"`). Empty when the
    /// parameter is missing from `sketch.parameters` (defensive — the
    /// row still renders so the user can re-bind via direct edit).
    pub current_expr: String,
}

/// v0.16.4 — Pour role properties surfaced on the Properties panel.
#[derive(Debug, Clone)]
pub struct PourSummary {
    pub net: Option<String>,
    pub fill_type: signex_sketch::attr::PourFillType,
    pub priority: u32,
}

/// v0.16.4 — Keepout role properties surfaced on the Properties panel.
#[derive(Debug, Clone)]
pub struct KeepoutSummary {
    pub no_routing: bool,
    pub no_components: bool,
    pub no_copper: bool,
    pub no_vias: bool,
    pub no_drilling: bool,
    pub no_pours: bool,
}

/// v0.16.4 — BoardCutout role properties surfaced on the Properties panel.
#[derive(Debug, Clone)]
pub struct CutoutSummary {
    pub edge_radius_expr: Option<String>,
    pub through: bool,
}

/// v0.23 — Array (Pattern) properties surfaced on the Properties panel
/// when the selected sketch entity is the source of an
/// [`signex_sketch::array::Array`]. The handler resolves the array by
/// `array_id`, mutates the matching field, then runs solve+bake.
#[derive(Debug, Clone)]
pub struct ArraySummary {
    pub array_id: signex_sketch::array::ArrayId,
    pub kind: ArrayKindSummary,
    pub numbering: NumberingSchemeKindUi,
    /// `true` when the polar centre re-pick is active — the next
    /// sketch click on a Point sets `array.center`.
    pub repicking_polar_center: bool,
    /// v0.25 polish — when `numbering == BgaRowCol`, this carries the
    /// BGA-specific config (skip_letters / start_row / start_col) so
    /// the Properties panel can surface editable rows for each. `None`
    /// for Linear / Explicit numbering schemes.
    pub bga_config: Option<BgaConfigSummary>,
}

/// v0.25 polish — surface for BGA numbering scheme parameters.
/// Mirror of [`signex_sketch::array::NumberingScheme::BgaRowCol`].
#[derive(Debug, Clone)]
pub struct BgaConfigSummary {
    /// IPC-7351 letter-skip convention (omits I/O/Q/S/X/Z to avoid
    /// confusion with numerals). Default `true` matches Altium.
    pub skip_letters: bool,
    /// First row letter (e.g. `'A'`). Drives the row labels in the
    /// baked replicas.
    pub start_row: char,
    /// First column number (e.g. `1`).
    pub start_col: u32,
}

#[derive(Debug, Clone)]
pub enum ArrayKindSummary {
    Linear {
        count_expr: String,
        dx_expr: String,
        dy_expr: String,
    },
    Grid {
        nx_expr: String,
        ny_expr: String,
        dx_expr: String,
        dy_expr: String,
        /// Empty string when the array has no `GridDepopulation`.
        mask_expr: String,
        /// Per-instance suppression carried alongside `mask_expr`. The
        /// bake honours both — a (i, j) pair in this list skips the
        /// instance regardless of the mask predicate. Stored as
        /// `(i, j)` 0-based row/column indices. Drives the v0.23 B5
        /// per-instance checkbox grid in the Properties panel.
        suppressed_instances: Vec<(u32, u32)>,
        /// Snapshot of `nx` after evaluation — drives the checkbox
        /// grid's column count. `None` when the expression isn't
        /// numeric (e.g. references a parameter that isn't bound).
        nx_value: Option<u32>,
        /// Snapshot of `ny` — checkbox grid's row count.
        ny_value: Option<u32>,
    },
    Polar {
        count_expr: String,
        sweep_angle_expr: String,
        center_position_mm: Option<[f64; 2]>,
        mask_expr: String,
        /// Per-instance suppression for Polar; the j coordinate is
        /// always 0 (Polar arrays are 1-D).
        suppressed_instances: Vec<u32>,
        /// Snapshot of the evaluated `count` — drives the checkbox row
        /// length. `None` when the expression isn't numeric.
        count_value: Option<u32>,
    },
}

/// v0.23 — Numbering scheme kind for the Properties panel pick_list.
/// Mirrors [`signex_sketch::array::NumberingScheme`]'s tag. The handler
/// preserves the inner expression fields when flipping kinds where
/// possible (e.g. switching to LinearIncrement keeps any prior
/// start/step expressions; switching to Explicit clears them).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberingSchemeKindUi {
    LinearIncrement,
    BgaRowCol,
    Explicit,
}

impl NumberingSchemeKindUi {
    pub const ALL: [Self; 3] = [Self::LinearIncrement, Self::BgaRowCol, Self::Explicit];
}

impl std::fmt::Display for NumberingSchemeKindUi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::LinearIncrement => "Linear (1, 2, 3, …)",
            Self::BgaRowCol => "BGA (A1, A2, …)",
            Self::Explicit => "Explicit list",
        })
    }
}

/// v0.23 — Field discriminator for [`PanelMsg::FpEditorEditArrayParam`].
/// Each variant maps to a single text-input on one [`ArrayKindSummary`]
/// branch; the handler uses the variant to disambiguate the target
/// field when mutating the array in place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrayParamField {
    LinearCountExpr,
    LinearDxExpr,
    LinearDyExpr,
    GridNxExpr,
    GridNyExpr,
    GridDxExpr,
    GridDyExpr,
    PolarCountExpr,
    PolarSweepAngleExpr,
    /// Maps to `GridDepopulation.mask_expr` for both Grid and Polar
    /// arrays. Empty string clears the depopulation entirely (set
    /// `Option::None` on the array).
    MaskExpr,
}

/// v0.16.4 — discrete bit identifier for [`KeepoutSummary`] flags.
/// PanelMsg-friendly so the Keepout sub-form's checklist can carry
/// "which flag" without smuggling a closure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeepoutKindFlag {
    NoRouting,
    NoComponents,
    NoCopper,
    NoVias,
    NoDrilling,
    NoPours,
}

/// v0.17.0 — discrete identifier for the four `SnapOptions` flags.
/// Used by the Properties-panel snap checklist to carry "which
/// flag" through `PanelMsg`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapOptionFlag {
    PointHit,
    HorizontalVertical,
    Angle,
    Grid,
    // v0.13 — Altium "Objects for snapping" set.
    TrackVertices,
    TrackLines,
    ArcCenters,
    Intersections,
    PadCenters,
    PadVertices,
    PadEdges,
    ViaCenters,
    Texts,
    Regions,
    FootprintOrigins,
    Body3dPoints,
    /// Independent target-category toggles (Altium PCB Library
    /// editor): each can be on simultaneously.
    SnapToGrids,
    SnapToGuides,
    SnapToAxes,
}

/// One row in the Footprint Library panel — a sibling `.snxfpt`
/// living next to the active footprint inside the same `.snxlib`'s
/// `footprints/` directory.
#[derive(Debug, Clone)]
pub struct FootprintLibSibling {
    /// Absolute path to the `.snxfpt` on disk.
    pub path: std::path::PathBuf,
    /// File stem ("SOIC-8") — falls back to filename when the stem
    /// can't be resolved.
    pub display_name: String,
    /// `true` when this sibling is the currently-open footprint
    /// (the active tab). The panel renders this row with the
    /// selection highlight.
    pub is_active: bool,
}

/// v0.18.8 — one row in the Footprint Library panel representing a
/// footprint *inside* the active `.snxfpt` envelope. Mirror of one
/// component row in Altium's PCB Library panel.
#[derive(Debug, Clone)]
pub struct FootprintLibInternalRow {
    /// `Footprint::name` — primary display label.
    pub name: String,
    /// Pad count, surfaced in a secondary column.
    pub pad_count: usize,
    /// `true` when this index matches the editor's `active_idx` —
    /// the row that the canvas / Properties panel currently shows.
    pub is_active: bool,
}

/// Editor mode mirror — kept in this crate so the panel doesn't need
/// to import `library::editor::footprint::state::EditorMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FootprintModeKind {
    Pads,
    Sketch,
    View3d,
}

#[derive(Debug, Clone)]
pub struct FootprintSolveSummary {
    pub iterations: usize,
    pub elapsed_ms: u64,
    pub final_residual_norm: f64,
    pub over_constraint_count: usize,
    /// v0.22 Phase E3+E4 — per-over-constraint summary so the
    /// Properties panel can list the conflicts and the user can
    /// click each to focus the canvas on the offending geometry.
    /// Sorted descending by `residual_magnitude` so the worst
    /// offender is first. Empty when `over_constraint_count == 0`.
    pub over_constraints: Vec<OverConstraintSummary>,
}

#[derive(Debug, Clone)]
pub struct OverConstraintSummary {
    /// v0.23 — Constraint ID used by the canvas to render the
    /// specific row at full red while everything else (including
    /// other over-constraints) dims. Drives per-row hover precision
    /// in the Properties panel "Conflicts" list.
    pub constraint_id: signex_sketch::id::ConstraintId,
    /// Kind label — "Coincident", "DistancePtPt", "Horizontal", etc.
    /// Static string avoids allocating per-row.
    pub kind_label: &'static str,
    /// Post-solve residual magnitude. The LM iteration drives this
    /// below `Solver::tolerance` for satisfiable constraints; values
    /// above `RANK_TOL` indicate the constraint couldn't be met
    /// alongside the others.
    pub residual_magnitude: f64,
    /// First Point entity the constraint touches. Click → select
    /// this Point so the canvas pans and the constraint icon
    /// rendered in red sits in view. `None` for constraints with
    /// no Point endpoints (rare — Fixed pseudo-rows).
    pub focus_entity_id: Option<signex_sketch::id::SketchEntityId>,
}

#[derive(Debug, Clone)]
pub struct FootprintPadSummary {
    pub idx: usize,
    pub number: String,
    pub kind_label: &'static str,
    pub shape_label: &'static str,
    pub size_mm: [f64; 2],
    pub position_mm: [f64; 2],
    pub rotation_deg: f64,
    pub layer_count: usize,
    pub has_drill: bool,
    /// v0.20 — full snapshot of editable pad state surfaced for the
    /// selected-pad Properties panel. Mirrors the same fields the
    /// next-pad form binds to so the selected-pad branch can render
    /// the same Properties / Pad Stack / Pad Features sections.
    pub side: crate::library::editor::footprint::state::PadSide,
    pub shape: signex_library::PadShape,
    pub kind: signex_library::PadKind,
    pub drill_diameter_mm: Option<f64>,
    pub stack: crate::library::editor::footprint::state::PadStackUi,
    pub feature_top: signex_sketch::attr::PadFeature,
    pub feature_bottom: signex_sketch::attr::PadFeature,
    pub testpoint: signex_sketch::attr::TestpointFlags,
    pub template: String,
    pub template_library: String,
    /// v0.21 — Altium-parity electrical-type.
    pub electrical_type: signex_sketch::attr::ElectricalType,
    /// v0.21 — net assignment.
    pub net: String,
    /// v0.21 — locked flag.
    pub locked: bool,
    /// v0.21 — Pad Hole detail fields for the selected pad.
    pub hole_tolerance_plus_mm: Option<f64>,
    pub hole_tolerance_minus_mm: Option<f64>,
    pub hole_rotation_deg: Option<f64>,
    pub copper_offset_x_mm: Option<f64>,
    pub copper_offset_y_mm: Option<f64>,
}

/// v0.18.24 — Read-only summary of the currently-selected silk-front
/// v0.21 — sketch-mode pad attribute snapshot. Mirrors the new
/// fields we added to `PadAttr` so the sketch-entity Properties
/// branch can render an editable Pad Attributes section.
#[derive(Debug, Clone)]
pub struct SketchPadAttrSummary {
    pub id: signex_sketch::id::SketchEntityId,
    pub electrical_type: signex_sketch::attr::ElectricalType,
    pub net: String,
    pub locked: bool,
    pub template: String,
    pub template_library: String,
    pub feature_top: signex_sketch::attr::PadFeature,
    pub feature_bottom: signex_sketch::attr::PadFeature,
    pub testpoint: signex_sketch::attr::TestpointFlags,
    pub thermal_relief: bool,
    pub mask_top_tented: bool,
    pub mask_bottom_tented: bool,
    pub paste_top_enabled: bool,
    pub paste_bottom_enabled: bool,
    pub corner_radius_pct: Option<f64>,
    pub hole_tolerance_plus_mm: Option<f64>,
    pub hole_tolerance_minus_mm: Option<f64>,
    pub hole_rotation_deg: Option<f64>,
    pub copper_offset_x_mm: Option<f64>,
    pub copper_offset_y_mm: Option<f64>,
    /// `true` when the pad has a drill spec (i.e. THT / NPT). Used
    /// to gate Hole-detail UI rows.
    pub has_drill: bool,
}

/// graphic (`FpGraphic` in `silk_f`). Drives the Properties panel's
/// silk-selection branch with full per-kind editable fields.
#[derive(Debug, Clone)]
pub struct FootprintSelectedSilkSummary {
    pub idx: usize,
    pub kind_label: &'static str,
    /// Stroke width in mm.
    pub stroke_width_mm: f64,
    /// `true` = solid fill; `false` = outline only.
    pub filled: bool,
    /// Per-kind geometry — only the field set matching `kind_label`
    /// is meaningful at any given time. `None` slots stay None.
    pub kind: SilkKindGeometry,
}

/// v0.21 — per-`FpGraphicKind` editable geometry. We surface
/// dedicated editable forms only for Line (Track) and Text (String);
/// Arc / Rectangle / Circle / Polygon collapse to `Other` and get a
/// minimal banner pointing the user at sketch mode for parametric
/// editing.
#[derive(Debug, Clone)]
pub enum SilkKindGeometry {
    Line {
        from_mm: [f64; 2],
        to_mm: [f64; 2],
    },
    Text {
        position_mm: [f64; 2],
        content: String,
        size_mm: f64,
    },
    /// Fallback for Arc / Rectangle / Circle / Polygon — the
    /// Properties panel surfaces stroke width / filled / locked /
    /// delete and a hint to switch to Sketch mode for full editing.
    Other,
}

#[derive(Debug, Clone)]
pub struct FootprintSketchEntitySummary {
    /// Display label for the entity kind ("Point", "Line", "Arc",
    /// "Circle"). Wrapper avoids importing the sketch enum.
    pub kind_label: &'static str,
    /// Coordinates in mm — for Points only; None for Lines/Arcs/Circles.
    pub position_mm: Option<[f64; 2]>,
    /// Number of attached constraints touching this entity.
    pub attached_constraint_count: usize,
    /// `true` if this is a construction entity (solver scaffolding
    /// only, no baked geometry).
    pub construction: bool,
    /// v0.22 Phase A3 — solver-state colour for the entity. `Some` for
    /// Points (looked up in `last_solve.colours`); `None` for other
    /// entity kinds whose DOF state is implicitly the min of their
    /// endpoints'. Drives the "DOF" row in the Properties panel.
    pub dof_state: Option<signex_sketch::solver::dof::DofColor>,
}

