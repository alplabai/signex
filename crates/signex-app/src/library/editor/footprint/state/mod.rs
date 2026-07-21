//! Footprint editor in-memory state.
//!
//! The canvas state derives from a typed
//! [`signex_library::Footprint`] primitive — pad geometry mirrors
//! `Footprint::pads: Vec<Pad>`. Two-way sync runs through
//! [`FootprintEditorState::sync_pads_to_primitive`] so the dispatcher
//! keeps the primitive authoritative.
//!
//! Dispatcher convention: every mutating op edits the canvas state,
//! then calls `sync_pads_to_primitive(&canvas_state, &mut footprint)`
//! to write the new pad list back onto the primitive.
//!
//! Submodules:
//! - [`pad`] — `EditorPad`, `PadStackUi`, `NextPadDefaults`, `PadSide`,
//!   `CourtyardRect`, `ShapeParamMap`, `AlignOp`.
//! - [`mode`] — `EditorMode`, `FpActiveBarMenu`, `PadStackTab`.
//! - [`context_menu`] — right-click menu state types.
//! - [`placement`] — `PlacementInput*`, `PlaceArcPending`.
//! - [`selection`] — `selected_pad_indices`, the shared selection union.
//! - [`tool`] — `PadsTool`, `SketchTool`, `ToolPending`.
//! - [`selection_filter`] — `SelectionFilter`, `SelectionFilterKind`.
//! - [`snap_options`] — `SnapOptions`, `GridDef`, `Guide*`, `SnapSubTab`,
//!   `SnappingMode`, `GridDisplay`.

pub mod context_menu;
pub mod mode;
pub mod pad;
pub mod placement;
pub mod selection;
pub mod selection_filter;
pub mod snap_options;
pub mod tool;

pub use context_menu::{
    FootprintContextAction, FootprintContextMenuState, FootprintContextSubmenu,
    FootprintContextTarget,
};
pub use mode::{EditorMode, FpActiveBarMenu, PadStackTab};
pub use pad::{
    AlignOp, CourtyardRect, EditorPad, NextPadDefaults, PadSide, PadStackUi, ShapeParamMap,
};
pub use placement::{PlaceArcPending, PlacementInput, PlacementInputKind};
pub use selection_filter::{FpSelectionMode, SelectionFilter, SelectionFilterKind};
pub use snap_options::{
    GridDef, GridDisplay, Guide, GuideAxis, SnapOptions, SnapSubTab, SnappingMode,
};
pub use tool::{PadsTool, SketchTool, ToolPending};

use signex_library::{Footprint, LayerId};

use super::layers::LayerVisibility;
use pad::NEW_PAD_SIZE_MM;

/// Slack on each side of the pad bounding box when auto-fitting the
/// courtyard polygon.
const COURTYARD_SLACK_MM: f64 = 0.25;

/// v0.14 — typed-delta "Move Selection By X, Y" modal. `None` on
/// `FootprintEditorState::move_by_modal` means the modal is closed.
/// Two erasable string buffers (same pattern as `dimension_input`) so
/// typing "-" / "." mid-entry doesn't fight an f64 binding.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MoveByModal {
    pub dx_buf: String,
    pub dy_buf: String,
}

impl MoveByModal {
    /// Parse both buffers as mm. `None` if either fails to parse.
    pub fn parsed(&self) -> Option<(f64, f64)> {
        Some((
            self.dx_buf.trim().parse().ok()?,
            self.dy_buf.trim().parse().ok()?,
        ))
    }
}

/// #370 — "Align…" dialog state. `None` on
/// [`FootprintEditorState::align_modal`] means the modal is closed.
///
/// The dialog is a pure composition shell over the existing
/// [`AlignOp`] variants — it introduces no new geometry. The user picks
/// at most one horizontal op and at most one vertical op (each
/// `None` = "leave that axis untouched"); Confirm applies both chosen
/// ops under a SINGLE undo snapshot (see `updates::active_bar`). The two
/// axes are independent — horizontal ops touch only X, vertical ops only
/// Y — so applying both in sequence equals picking the two concrete
/// dropdown rows one at a time.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AlignModal {
    /// Chosen horizontal op ([`AlignOp::Left`] / [`AlignOp::CenterH`] /
    /// [`AlignOp::Right`] / [`AlignOp::DistributeH`]), or `None` to
    /// leave the X axis untouched.
    pub horizontal: Option<AlignOp>,
    /// Chosen vertical op ([`AlignOp::Top`] / [`AlignOp::CenterV`] /
    /// [`AlignOp::Bottom`] / [`AlignOp::DistributeV`]), or `None` to
    /// leave the Y axis untouched.
    pub vertical: Option<AlignOp>,
}

/// Live, in-memory state of the Footprint canvas — drives interaction
/// and rendering. The authoritative pad list lives on
/// `ComponentEditorState.footprint.pads`; this struct mirrors it for
/// the canvas's hit-test + draw layer.
///
/// `PartialEq` is intentionally NOT derived: Phase 5.3 added
/// `sketch_solver` / `last_solve` whose underlying types in
/// `signex-sketch` don't implement `PartialEq`.
#[derive(Debug, Clone)]
pub struct FootprintEditorState {
    pub pads: Vec<EditorPad>,
    pub layer_visibility: LayerVisibility,
    /// `Some(idx)` while a pad is selected. With multi-select, this
    /// is the **primary** selection — the pad whose properties drive
    /// the right-dock Properties panel form. Extra rubber-band /
    /// ctrl-click selections live in `selected_pads_extra`.
    pub selected_pad: Option<usize>,
    /// v0.27 — Altium-style additional pads in the selection.
    /// Highlighted on canvas like `selected_pad` but don't drive the
    /// Properties form. Cleared whenever `selected_pad` is replaced
    /// via a non-multi action (single click on a different pad,
    /// click on empty canvas, Esc).
    pub selected_pads_extra: Vec<usize>,
    /// v0.27 — `true` while the Lasso Select tool is armed. Each
    /// canvas left-click appends a world-mm vertex to
    /// `lasso_vertices`. Right-click / Esc commits the polygon: any
    /// pad whose centre falls inside selects via
    /// `FootprintSelectPads`. After commit / cancel, both this flag
    /// and the vertex list reset.
    pub lasso_mode_active: bool,
    /// v0.27 — vertex stash for the Lasso Select tool. World mm.
    pub lasso_vertices: Vec<(f64, f64)>,
    /// v0.27 — `true` while the Touching Line tool is armed. Each
    /// click is one of two endpoints; after the second click, the
    /// line segment is intersected against every pad bbox and all
    /// hits are multi-selected.
    pub touching_line_active: bool,
    /// v0.27 — first endpoint of the in-flight touching line, in
    /// world mm. `None` before the first click; reset after commit
    /// / cancel.
    pub touching_line_first: Option<(f64, f64)>,
    /// v0.27 — most recent left-click world position on a pad.
    /// Drives Select overlapped / Select next dropdown items so
    /// they can find the stack of pads at that location and cycle
    /// through them.
    pub last_click_world_mm: Option<(f64, f64)>,
    /// `true` when the courtyard polygon should track the pad bbox.
    pub auto_fit_courtyard: bool,
    pub courtyard_mm: Option<CourtyardRect>,
    /// v0.27 — outline-following courtyard polygon, world mm. Set
    /// by [`FootprintEditorState::recompute_courtyard_outline`]
    /// which runs the union-of-pad-bboxes through
    /// `signex_sketch::geom::polygon_op` (Union) then offsets the
    /// result by `COURTYARD_SLACK_MM` via `offset_polygon`. Drawn
    /// in preference to the bbox-based `courtyard_mm` when present.
    /// Cleared whenever pads are added / moved / removed so the
    /// stale outline doesn't survive the next mutation; the user
    /// re-runs the action to refresh.
    pub courtyard_outline_mm: Option<Vec<(f64, f64)>>,
    /// Last known cursor world position in mm.
    pub cursor_mm: Option<(f64, f64)>,
    /// Phase 5.3: which editor mode the user has switched to.
    pub mode: EditorMode,
    /// Phase 5.3: shared LM solver config.
    pub sketch_solver: signex_sketch::solver::Solver,
    /// Output of the most recent solve.
    pub last_solve: Option<signex_sketch::solver::FullSolveOutput>,
    /// Last solve's audit / over-constraint warnings.
    pub solve_warnings: Vec<String>,
    /// v0.22 Phase E3+E4 → v0.23 — `Some(id)` when the user is hovering
    /// a specific row in the Conflicts list.
    pub conflicts_row_hovered: Option<signex_sketch::id::ConstraintId>,
    /// v0.13.2 — currently-active sketch tool.
    pub active_tool: SketchTool,
    /// v0.13.2 — transient state for multi-click tools.
    pub tool_pending: ToolPending,
    /// v0.13.3 — currently-selected sketch entity.
    pub selected_sketch: Option<signex_sketch::id::SketchEntityId>,
    /// v0.13.3 — secondary selected sketch entity.
    pub selected_sketch_secondary: Option<signex_sketch::id::SketchEntityId>,
    /// v0.27 — Altium / Fusion-style multi-select for sketch
    /// entities. Populated by the sketch-mode rubber-band release
    /// + Ctrl/Shift modifier clicks. Drawn with the same selection
    /// highlight as `selected_sketch`. Cleared whenever a single-
    /// click select fires without a modifier.
    pub selected_sketch_extra: Vec<signex_sketch::id::SketchEntityId>,
    /// v0.13.3 — Dimension tool's pending value (text input).
    pub dimension_input: String,
    /// v0.14 — typed-delta "Move Selection By" modal. `None` = closed.
    /// Two erasable string buffers (same pattern as `dimension_input`)
    /// so typing "-" / "." mid-entry doesn't fight an f64 binding.
    pub move_by_modal: Option<MoveByModal>,
    /// #370 — "Align…" dialog. `None` = closed. Holds the chosen
    /// per-axis ops; Confirm composes the existing [`AlignOp`] variants
    /// under one history snapshot (see [`AlignModal`]).
    pub align_modal: Option<AlignModal>,
    /// v0.15 — Pads-mode tool.
    pub pads_tool: PadsTool,
    /// v0.16.1 — sticky construction-mode toggle.
    pub construction_mode: bool,
    /// v0.22 Phase A5 — Centerline-mode mint flag.
    pub centerline_mode: bool,
    /// v0.16.1 — TAB pause-during-placement.
    pub placement_paused: bool,
    /// v0.16.3 — defaults applied to the next pad created via
    /// `PadsTool::PlacePad`.
    pub next_pad_defaults: NextPadDefaults,
    /// v0.17.0 — empty-canvas Snap Options.
    pub snap_options: SnapOptions,
    /// v0.18.15.1 — first click of an in-flight Place Track gesture.
    pub track_first: Option<(f64, f64)>,
    /// v0.18.15.3 — Place Arc 3-click gesture state.
    pub place_arc_pending: PlaceArcPending,
    /// v0.18.15.4 — Place Polygon vertex stash.
    pub place_polygon_vertices: Vec<(f64, f64)>,
    /// v0.18.18 — currently-selected silk-front graphic index.
    pub selected_silk_f: Option<usize>,
    /// v0.18.20 — Altium-style guide lines.
    pub guides: Vec<Guide>,
    /// v0.18.21 — Altium-style grid table.
    pub grids: Vec<GridDef>,
    /// v0.18.21 — index into `grids` of the active row.
    pub active_grid_idx: usize,
    /// v0.18.25.1 — mirror of the global `ui_state.snap_enabled`
    /// status-bar toggle.
    pub global_snap_disabled: bool,
    /// v0.18.13 — Altium Selection Filter pill row state.
    pub selection_filter: SelectionFilter,
    /// v0.27 — rubber-band selection mode (Inside / Touching /
    /// Outside) sourced from the active-bar Selection Mode picker.
    /// Drives the `box_select` release picker in `canvas/mod.rs`.
    pub selection_mode_2d: FpSelectionMode,
    /// v0.18.13 — active sub-tab in the Snap Options section.
    pub snap_subtab: SnapSubTab,
    /// v0.18.13 — Snapping 3-state.
    pub snapping_mode: SnappingMode,
    /// v0.13 — Open active-bar dropdown menu. `None` = no menu open.
    pub active_bar_menu: Option<FpActiveBarMenu>,
    /// v0.20 — Pad Stack panel tab.
    pub pad_stack_tab: PadStackTab,
    /// v0.24 Phase 1 (Track D) — live numeric input during sketch-tool
    /// placement.
    pub placement_input: Option<PlacementInput>,
    /// v0.14-footprint — the *inactive* (stashed) placement-input
    /// fields while the user Tab-cycles a multi-dimension tool. The
    /// focused field always lives in `placement_input` (so the
    /// existing char-append / consume logic needs no rework); Tab
    /// rotates the focused field out to here and pulls the next one
    /// in. Holds 0..N entries: empty for single-field tools, one for
    /// Line (len/angle) & Rectangle (w/h), two for Rounded-Rect
    /// (w/h/radius).
    pub placement_input_others: Vec<PlacementInput>,
    /// v0.25 polish — per-input verbatim buffers for Properties-panel
    /// numeric fields.
    pub numeric_buffers: std::collections::HashMap<String, String>,
    /// v0.26 — right-click canvas context menu.
    pub context_menu: Option<FootprintContextMenuState>,
    /// v0.26-C — one-shot "fit canvas to content" request.
    pub fit_pending: bool,
}

impl FootprintEditorState {
    /// Build canvas state from the primitive's pad list.
    pub fn from_footprint(fp: &Footprint) -> Self {
        let mut pads: Vec<EditorPad> = fp.pads.iter().map(EditorPad::from_pad).collect();
        // Rebuild the sketch link from the sketch — `from_pad` cannot,
        // and without it every mirror early-returns after a reopen.
        pad::relink_pads_to_sketch(&mut pads, fp);
        let mut s = Self::with_pads(pads);
        s.recompute_courtyard();
        s
    }

    /// Empty state — used for brand-new components and as the fallback
    /// when the binding has no footprint primitive yet.
    #[allow(dead_code)]
    pub fn empty() -> Self {
        let mut s = Self::with_pads(Vec::new());
        s.recompute_courtyard();
        s
    }

    /// Internal constructor shared by `from_footprint` + `empty`.
    /// Centralising the field-by-field defaulting kills the giant
    /// duplicated builder block that used to live in both call sites.
    fn with_pads(pads: Vec<EditorPad>) -> Self {
        Self {
            pads,
            layer_visibility: LayerVisibility::default(),
            selected_pad: None,
            selected_pads_extra: Vec::new(),
            lasso_mode_active: false,
            lasso_vertices: Vec::new(),
            touching_line_active: false,
            touching_line_first: None,
            last_click_world_mm: None,
            // v0.26-I — auto-courtyard mode removed; courtyard is
            // authored explicitly via silk graphic / sketch entity.
            auto_fit_courtyard: false,
            courtyard_mm: None,
            courtyard_outline_mm: None,
            cursor_mm: None,
            mode: EditorMode::Normal,
            sketch_solver: signex_sketch::solver::Solver::default(),
            last_solve: None,
            solve_warnings: Vec::new(),
            conflicts_row_hovered: None,
            active_tool: SketchTool::default(),
            tool_pending: ToolPending::default(),
            selected_sketch: None,
            selected_sketch_secondary: None,
            selected_sketch_extra: Vec::new(),
            dimension_input: String::new(),
            move_by_modal: None,
            align_modal: None,
            pads_tool: PadsTool::default(),
            construction_mode: false,
            centerline_mode: false,
            placement_paused: false,
            next_pad_defaults: NextPadDefaults::default(),
            snap_options: SnapOptions::default(),
            track_first: None,
            place_arc_pending: PlaceArcPending::default(),
            place_polygon_vertices: Vec::new(),
            selected_silk_f: None,
            guides: Vec::new(),
            grids: vec![GridDef::from_snap_options(&SnapOptions::default())],
            active_grid_idx: 0,
            global_snap_disabled: false,
            selection_filter: SelectionFilter::default(),
            selection_mode_2d: FpSelectionMode::default(),
            snap_subtab: SnapSubTab::default(),
            snapping_mode: SnappingMode::default(),
            active_bar_menu: None,
            pad_stack_tab: PadStackTab::default(),
            placement_input: None,
            placement_input_others: Vec::new(),
            numeric_buffers: std::collections::HashMap::new(),
            context_menu: None,
            fit_pending: false,
        }
    }

    /// Bounding box of the entire footprint (pads + courtyard) in mm.
    pub fn content_bbox_mm(&self) -> Option<(f64, f64, f64, f64)> {
        let mut bbox: Option<(f64, f64, f64, f64)> = None;
        let mut expand = |x0: f64, y0: f64, x1: f64, y1: f64| {
            bbox = Some(match bbox {
                Some((a, b, c, d)) => (a.min(x0), b.min(y0), c.max(x1), d.max(y1)),
                None => (x0, y0, x1, y1),
            });
        };
        for pad in &self.pads {
            let (x0, y0, x1, y1) = pad.rotated_aabb_mm();
            expand(x0, y0, x1, y1);
        }
        if let Some(c) = self.courtyard_mm {
            expand(c.min_x, c.min_y, c.max_x, c.max_y);
        }
        bbox
    }

    /// Auto-incremented pad number — picks the next integer above the
    /// current max, or "1" if none of the pads parse as integers.
    pub fn next_pad_number(&self) -> String {
        let max_int = self
            .pads
            .iter()
            .filter_map(|p| p.number.parse::<u32>().ok())
            .max()
            .unwrap_or(0);
        format!("{}", max_int + 1)
    }

    /// Click-add a new pad at the given world position.
    pub fn add_pad_at(&mut self, x_mm: f64, y_mm: f64) -> usize {
        let defaults = self.next_pad_defaults.clone();
        let number = defaults
            .designator_override
            .clone()
            .unwrap_or_else(|| self.next_pad_number());
        let layers = match defaults.side {
            PadSide::Top => vec![
                LayerId::new("F.Cu"),
                LayerId::new("F.Mask"),
                LayerId::new("F.Paste"),
            ],
            PadSide::Bottom => vec![
                LayerId::new("B.Cu"),
                LayerId::new("B.Mask"),
                LayerId::new("B.Paste"),
            ],
            PadSide::All => vec![
                LayerId::new("*.Cu"),
                LayerId::new("F.Mask"),
                LayerId::new("B.Mask"),
            ],
        };
        let mut pad = EditorPad::new_default(number, (x_mm, y_mm));
        pad.size_mm = (defaults.size_x_mm.max(0.05), defaults.size_y_mm.max(0.05));
        pad.shape = defaults.shape.clone();
        pad.kind = defaults.kind;
        pad.layers = layers;
        pad.rotation_deg = defaults.rotation_deg;
        pad.stack = defaults.stack.clone();
        pad.feature_top = defaults.feature_top;
        pad.feature_bottom = defaults.feature_bottom;
        pad.testpoint = defaults.testpoint;
        pad.template = defaults.template.clone();
        pad.template_library = defaults.template_library.clone();
        if matches!(defaults.side, PadSide::All) {
            pad.drill_diameter_mm = defaults.drill_diameter_mm;
        }
        pad.electrical_type = defaults.electrical_type;
        pad.net = defaults.net.clone();
        pad.locked = defaults.locked;
        pad.hole_tolerance_plus_mm = defaults.hole_tolerance_plus_mm;
        pad.hole_tolerance_minus_mm = defaults.hole_tolerance_minus_mm;
        pad.hole_rotation_deg = defaults.hole_rotation_deg;
        pad.copper_offset_x_mm = defaults.copper_offset_x_mm;
        pad.copper_offset_y_mm = defaults.copper_offset_y_mm;
        self.pads.push(pad);
        let idx = self.pads.len() - 1;
        self.selected_pad = Some(idx);
        self.recompute_courtyard();
        idx
    }

    /// v0.18.12 — click-add a non-plated through hole at the given
    /// world position.
    pub fn add_hole_at(&mut self, x_mm: f64, y_mm: f64) -> usize {
        let defaults = self.next_pad_defaults.clone();
        let number = defaults
            .designator_override
            .clone()
            .unwrap_or_else(|| self.next_pad_number());
        let drill_mm = defaults.size_x_mm.max(0.05);
        let mut pad = EditorPad::new_npt_hole(number, (x_mm, y_mm), drill_mm);
        pad.stack = defaults.stack.clone();
        pad.feature_top = defaults.feature_top;
        pad.feature_bottom = defaults.feature_bottom;
        pad.testpoint = defaults.testpoint;
        pad.template = defaults.template.clone();
        pad.template_library = defaults.template_library.clone();
        self.pads.push(pad);
        let idx = self.pads.len() - 1;
        self.selected_pad = Some(idx);
        self.recompute_courtyard();
        idx
    }

    /// Move the pad at `idx` to a new world position.
    pub fn move_pad(&mut self, idx: usize, x_mm: f64, y_mm: f64) {
        if let Some(pad) = self.pads.get_mut(idx) {
            pad.position_mm = (x_mm, y_mm);
            self.recompute_courtyard();
        }
    }

    /// v0.14 — translate every pad in `indices` by `(dx, dy)` mm.
    /// Backs the active-bar "Move Selection by X, Y…" nudge. Out-of-
    /// range indices are skipped; the courtyard is recomputed once at
    /// the end. Returns the moved pad indices (in the order given,
    /// minus any out-of-range entries) so the caller can mirror exactly
    /// those pads into the backing sketch.
    pub fn nudge_pads(&mut self, indices: &[usize], dx: f64, dy: f64) -> Vec<usize> {
        let mut moved = Vec::with_capacity(indices.len());
        for &i in indices {
            if let Some(pad) = self.pads.get_mut(i) {
                let (x, y) = pad.position_mm;
                pad.position_mm = (x + dx, y + dy);
                moved.push(i);
            }
        }
        if !moved.is_empty() {
            self.recompute_courtyard();
        }
        moved
    }

    /// Delete the pad at `idx`.
    pub fn delete_pad(&mut self, idx: usize) {
        if idx >= self.pads.len() {
            return;
        }
        self.pads.remove(idx);
        self.selected_pad = adjust_selection_after_remove(self.selected_pad, idx);
        self.recompute_courtyard();
    }

    /// Hit-test pads in reverse z-order (last-drawn = topmost).
    /// Skips pads on hidden layers.
    pub fn pad_at(&self, x_mm: f64, y_mm: f64) -> Option<usize> {
        for (idx, pad) in self.pads.iter().enumerate().rev() {
            if !self.layer_visibility.get(pad.primary_layer()) {
                continue;
            }
            if pad.contains_mm(x_mm, y_mm) {
                return Some(idx);
            }
        }
        None
    }

    /// Recompute the courtyard polygon when auto-fit is enabled.
    pub fn recompute_courtyard(&mut self) {
        if !self.auto_fit_courtyard {
            return;
        }
        if self.pads.is_empty() {
            self.courtyard_mm = None;
            return;
        }
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for pad in &self.pads {
            let (x0, y0, x1, y1) = pad.rotated_aabb_mm();
            if x0 < min_x {
                min_x = x0;
            }
            if y0 < min_y {
                min_y = y0;
            }
            if x1 > max_x {
                max_x = x1;
            }
            if y1 > max_y {
                max_y = y1;
            }
        }
        self.courtyard_mm = Some(CourtyardRect {
            min_x: min_x - COURTYARD_SLACK_MM,
            min_y: min_y - COURTYARD_SLACK_MM,
            max_x: max_x + COURTYARD_SLACK_MM,
            max_y: max_y + COURTYARD_SLACK_MM,
        });
    }

    /// Toggle the auto-fit courtyard flag.
    pub fn toggle_auto_fit(&mut self) {
        self.auto_fit_courtyard = !self.auto_fit_courtyard;
        self.recompute_courtyard();
    }

    /// v0.27 — outline-following courtyard. Builds a polygon for
    /// each pad's bbox, unions the lot via the sketch geom module,
    /// offsets the union by `COURTYARD_SLACK_MM`, and stores the
    /// result on `courtyard_outline_mm`. Replaces the bbox-based
    /// `recompute_courtyard` for users who want the courtyard to
    /// hug the actual pad cluster rather than its enclosing
    /// rectangle. Returns `true` when a polygon was produced;
    /// `false` when there are no pads or the boolean union failed.
    pub fn recompute_courtyard_outline(&mut self) -> bool {
        use signex_sketch::geom::{BoolOp, CornerStyle, Point2, offset_polygon, polygon_op};

        if self.pads.is_empty() {
            self.courtyard_outline_mm = None;
            return false;
        }

        // Per-pad quad from the ROTATED corners, so a turned pad's
        // courtyard hugs the copper instead of the box it would
        // occupy unrotated. Round/Oval/RoundRect/etc. still fall back
        // to the enclosing quad; a follow-up can mint the shape-
        // accurate outline (sampled circle for Round, arc-anchor for
        // RoundRect, etc.).
        let pad_polys: Vec<Vec<Point2>> = self
            .pads
            .iter()
            .map(|p| {
                p.rotated_corners_mm()
                    .iter()
                    .map(|&(x, y)| Point2::new(x, y))
                    .collect()
            })
            .collect();

        // Union pads pairwise. For disjoint pads polygon_op returns
        // both rings; we keep them all and offset each individually.
        // Connected pads collapse into a single ring through the
        // accumulating union.
        let mut accumulated: Vec<Vec<Point2>> = vec![pad_polys[0].clone()];
        for next in &pad_polys[1..] {
            let mut new_acc: Vec<Vec<Point2>> = Vec::new();
            let mut consumed = false;
            for ring in accumulated.drain(..) {
                let unioned = polygon_op(&ring, next, BoolOp::Union);
                if unioned.is_empty() {
                    new_acc.push(ring);
                } else {
                    new_acc.extend(unioned);
                    consumed = true;
                }
            }
            if !consumed {
                new_acc.push(next.clone());
            }
            accumulated = new_acc;
        }

        // Offset each ring outward by COURTYARD_SLACK_MM with
        // round corners (matches the Altium-PCB convention of an
        // arc-cornered courtyard) sampled at 4 segments per
        // corner — enough to read smooth at typical zooms.
        let mut offsetted: Vec<Vec<Point2>> = Vec::new();
        for ring in &accumulated {
            let off = offset_polygon(
                ring,
                COURTYARD_SLACK_MM,
                CornerStyle::Round { arc_segments: 4 },
            );
            if off.len() >= 3 {
                offsetted.push(off);
            }
        }

        if offsetted.is_empty() {
            self.courtyard_outline_mm = None;
            return false;
        }
        // For now we surface only the first ring — the bake's
        // `Footprint::courtyard` field carries one polygon. Multi-
        // ring courtyards (separate pad islands) need a schema
        // bump; queued for v0.28.
        let chosen = &offsetted[0];
        self.courtyard_outline_mm = Some(chosen.iter().map(|p| (p.x, p.y)).collect());
        true
    }

    /// v0.22 Phase D2 — Inverse of `sync_pads_to_primitive`. After a
    /// Sketch-mode solve+bake regenerates `Footprint::pads` from the
    /// sketch source-of-truth, refresh the editor-side `pads` cache.
    ///
    /// `sketch_entity_id`, `corner_entity_ids`, and `shape_params` (v0.24
    /// Track A4) don't round-trip through `Pad`, so we re-attach them
    /// by matching `pad.number`.
    ///
    /// A pad matched in `old_links` keeps its full editor-side link. A
    /// pad NOT in `old_links` — one that first appears from the sketch
    /// side (e.g. "Make Pad from Profile", which mints a `PadAttr`-
    /// carrying entity directly on the sketch and only reaches
    /// `state.pads` after the next bake) — is relinked from the
    /// authoritative source: the sketch entity whose `PadAttr.number`
    /// matches. Without this fallback such a pad stays permanently
    /// `sketch_entity_id: None`, so a Pads-mode move can't mirror into
    /// the sketch and the pad snaps back to its original position on the
    /// next bake.
    ///
    /// The number match is only applied where the number identifies ONE
    /// pad on each side. Pad numbers are not unique in signex (a
    /// shared-designator row / thermal / shield set is normal), and a
    /// last-wins number map hands several pads the same
    /// `sketch_entity_id` — after which a Pads-mode delete of one runs
    /// the delete mirror over another pad's geometry and its copper
    /// silently disappears from the bake. Ambiguous numbers are left
    /// unlinked here and offered to `relink_pads_to_sketch`, which
    /// disambiguates by exact position and refuses if even that ties.
    pub fn refresh_pads_from_primitive(&mut self, fp: &Footprint) {
        let mut new_pads: Vec<EditorPad> = fp.pads.iter().map(EditorPad::from_pad).collect();
        pad::carry_links_by_unique_number(&self.pads, &mut new_pads);
        // Anything the number carry could not supply a link for — a pad
        // that first appears from the sketch side, one whose old link
        // was itself `None` after a reopen, or one whose number is
        // ambiguous — is relinked from the sketch.
        pad::relink_pads_to_sketch(&mut new_pads, fp);
        self.pads = new_pads;
        if let Some(idx) = self.selected_pad {
            if idx >= self.pads.len() {
                self.selected_pad = None;
            }
        }
    }

    /// Write the canvas-side pad list back onto the primitive. Called
    /// after every mutation so the saved row sees the current pad
    /// layout. Other Footprint fields (graphics, body_3d, etc.) are
    /// left untouched — they're edited by their own panes.
    pub fn sync_pads_to_primitive(canvas: &Self, fp: &mut Footprint) {
        fp.pads = canvas.pads.iter().map(EditorPad::to_pad).collect();
        if let Some(c) = canvas.courtyard_mm {
            fp.courtyard = signex_library::Polygon::new(vec![
                [c.min_x, c.min_y],
                [c.max_x, c.min_y],
                [c.max_x, c.max_y],
                [c.min_x, c.max_y],
            ]);
        }
        // v0.22 Phase D1 — Pads-mode → Sketch attribute mirror. The
        // mapping itself is `pad_to_sketch` policy and lives there.
        if let Some(sketch) = fp.sketch.as_mut() {
            super::pad_to_sketch::mirror_pad_attrs_into_sketch(&canvas.pads, sketch);
        }
    }
}

/// HI-25 helper: when an item is removed at `removed_idx` from a Vec,
/// fold the change into a `selected: Option<usize>` so it still points
/// at the right element (or clears to `None` if the selection is what
/// got deleted). Used by the pad / silk / drawing deletion paths so
/// the "selection became dangling after delete" bug class can't recur.
///
/// - `None`                                 → `None`
/// - `Some(sel)` if `sel == removed_idx`    → `None`
/// - `Some(sel)` if `sel < removed_idx`     → `Some(sel)`
/// - `Some(sel)` if `sel > removed_idx`     → `Some(sel - 1)`
pub(crate) fn adjust_selection_after_remove(
    selected: Option<usize>,
    removed_idx: usize,
) -> Option<usize> {
    match selected {
        Some(sel) if sel == removed_idx => None,
        Some(sel) if sel > removed_idx => Some(sel - 1),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_library::{LayerId, Pad, PadKind, PadShape};

    #[test]
    fn from_footprint_round_trips_pads() {
        let mut fp = Footprint::empty("test");
        fp.pads.push(Pad {
            number: "1".into(),
            kind: PadKind::Smd,
            shape: PadShape::Rect,
            size: [1.0, 1.5],
            position: [-2.0, 0.0],
            rotation: 0.0,
            layers: vec![LayerId::new("F.Cu")],
            drill: None,
            solder_mask_margin: None,
            paste_margin: None,
            ..Pad::default()
        });
        let s = FootprintEditorState::from_footprint(&fp);
        assert_eq!(s.pads.len(), 1);
        assert_eq!(s.pads[0].number, "1");
        assert_eq!(s.pads[0].size_mm, (1.0, 1.5));
    }

    #[test]
    fn add_pad_assigns_next_number() {
        let mut s = FootprintEditorState::empty();
        let i = s.add_pad_at(0.0, 0.0);
        assert_eq!(i, 0);
        assert_eq!(s.pads[0].number, "1");
        s.add_pad_at(1.0, 0.0);
        assert_eq!(s.pads[1].number, "2");
    }

    #[test]
    fn sync_pads_to_primitive_writes_back() {
        let mut s = FootprintEditorState::empty();
        s.add_pad_at(0.0, 0.0);
        let mut fp = Footprint::empty("test");
        FootprintEditorState::sync_pads_to_primitive(&s, &mut fp);
        assert_eq!(fp.pads.len(), 1);
        assert_eq!(fp.pads[0].number, "1");
    }

    /// A pad that first appears from the SKETCH side — "Make Pad from
    /// Profile" mints a centre Point carrying a `PadAttr` directly on the
    /// sketch, and the pad only reaches `state.pads` after the next
    /// bake. Its number never existed in the prior `state.pads`, so the
    /// number-vs-old-pads relink misses and it would stay permanently
    /// unlinked (`sketch_entity_id: None`). An unlinked pad can't
    /// mirror a move into the sketch, so dragging it in Pads mode leaves
    /// the profile behind and the pad snaps back on the next bake.
    ///
    /// The relink must fall back to the authoritative link: the sketch
    /// entity whose `PadAttr.number` matches the pad.
    #[test]
    fn refresh_relinks_pad_from_sketch_pad_attr() {
        use signex_sketch::attr::PadAttr;
        use signex_sketch::entity::{Entity, EntityKind};
        use signex_sketch::id::SketchEntityId;
        use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
        use signex_sketch::sketch::SketchData;

        let mut fp = Footprint::empty("test");
        let plane_id = PlaneId::new();
        let mut sketch = SketchData::default();
        sketch.planes.push(Plane {
            id: plane_id,
            kind: PlaneKind::BoardTop,
        });
        let centre_id = SketchEntityId::new();
        let mut centre = Entity::new(centre_id, plane_id, EntityKind::Point { x: 1.0, y: 0.5 });
        centre.pad = Some(PadAttr {
            number: "1".into(),
            ..PadAttr::default()
        });
        sketch.entities.push(centre);
        fp.sketch = Some(sketch);
        fp.pads.push(Pad {
            number: "1".into(),
            ..Pad::default()
        });

        // state.pads is empty — the pad first appears from the sketch.
        let mut s = FootprintEditorState::empty();
        s.refresh_pads_from_primitive(&fp);

        assert_eq!(s.pads.len(), 1);
        assert_eq!(
            s.pads[0].sketch_entity_id,
            Some(centre_id),
            "pad appearing from the sketch must relink to its \
             PadAttr-carrying entity by number"
        );
    }
}
