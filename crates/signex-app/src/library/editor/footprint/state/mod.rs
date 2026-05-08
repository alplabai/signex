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
//!   `CourtyardRect`, `ShapeParamMap`.
//! - [`mode`] — `EditorMode`, `FpActiveBarMenu`, `PadStackTab`.
//! - [`context_menu`] — right-click menu state types.
//! - [`placement`] — `PlacementInput*`, `PlaceArcPending`.
//! - [`tool`] — `PadsTool`, `SketchTool`, `ToolPending`.
//! - [`selection_filter`] — `SelectionFilter`, `SelectionFilterKind`.
//! - [`snap_options`] — `SnapOptions`, `GridDef`, `Guide*`, `SnapSubTab`,
//!   `SnappingMode`, `GridDisplay`.

pub mod context_menu;
pub mod mode;
pub mod pad;
pub mod placement;
pub mod selection_filter;
pub mod snap_options;
pub mod tool;

pub use context_menu::{
    FootprintContextAction, FootprintContextMenuState, FootprintContextSubmenu,
    FootprintContextTarget,
};
pub use mode::{EditorMode, FpActiveBarMenu, PadStackTab};
pub use pad::{CourtyardRect, EditorPad, NextPadDefaults, PadSide, PadStackUi, ShapeParamMap};
pub use placement::{PlaceArcPending, PlacementInput, PlacementInputKind};
pub use selection_filter::{FpSelectionMode, SelectionFilter, SelectionFilterKind};
pub use snap_options::{GridDef, GridDisplay, Guide, GuideAxis, SnapOptions, SnapSubTab, SnappingMode};
pub use tool::{PadsTool, SketchTool, ToolPending};

use signex_library::{Footprint, LayerId};

use super::layers::LayerVisibility;
use pad::NEW_PAD_SIZE_MM;

/// Slack on each side of the pad bounding box when auto-fitting the
/// courtyard polygon.
const COURTYARD_SLACK_MM: f64 = 0.25;

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
    /// `Some(idx)` while a pad is selected.
    pub selected_pad: Option<usize>,
    /// `true` when the courtyard polygon should track the pad bbox.
    pub auto_fit_courtyard: bool,
    pub courtyard_mm: Option<CourtyardRect>,
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
    /// v0.13.3 — Dimension tool's pending value (text input).
    pub dimension_input: String,
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
        let pads = fp.pads.iter().map(EditorPad::from_pad).collect();
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
            // v0.26-I — auto-courtyard mode removed; courtyard is
            // authored explicitly via silk graphic / sketch entity.
            auto_fit_courtyard: false,
            courtyard_mm: None,
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
            dimension_input: String::new(),
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
            let (x0, y0, x1, y1) = pad.bbox_mm();
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
            let (x0, y0, x1, y1) = pad.bbox_mm();
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

    /// v0.22 Phase D2 — Inverse of `sync_pads_to_primitive`. After a
    /// Sketch-mode solve+bake regenerates `Footprint::pads` from the
    /// sketch source-of-truth, refresh the editor-side `pads` cache.
    ///
    /// `sketch_entity_id`, `corner_entity_ids`, and `shape_params` (v0.24
    /// Track A4) don't round-trip through `Pad`, so we re-attach them
    /// by matching `pad.number`.
    pub fn refresh_pads_from_primitive(&mut self, fp: &Footprint) {
        use std::collections::HashMap;
        type Link = (
            Option<signex_sketch::id::SketchEntityId>,
            Option<[signex_sketch::id::SketchEntityId; 4]>,
            ShapeParamMap,
        );
        let old_links: HashMap<String, Link> = self
            .pads
            .iter()
            .map(|p| {
                (
                    p.number.clone(),
                    (
                        p.sketch_entity_id,
                        p.corner_entity_ids,
                        p.shape_params.clone(),
                    ),
                )
            })
            .collect();
        let mut new_pads: Vec<EditorPad> = fp.pads.iter().map(EditorPad::from_pad).collect();
        for p in &mut new_pads {
            if let Some((sid, cids, params)) = old_links.get(&p.number) {
                p.sketch_entity_id = *sid;
                p.corner_entity_ids = *cids;
                p.shape_params = params.clone();
            }
        }
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
        // v0.22 Phase D1 — Pads-mode → Sketch attribute mirror.
        if let Some(sketch) = fp.sketch.as_mut() {
            for pad in &canvas.pads {
                let Some(id) = pad.sketch_entity_id else {
                    continue;
                };
                let Some(entity) = sketch.entities.iter_mut().find(|e| e.id == id) else {
                    continue;
                };
                let Some(attr) = entity.pad.as_mut() else {
                    continue;
                };
                attr.number = pad.number.clone();
                attr.net = pad.net.clone();
                attr.locked = pad.locked;
                attr.electrical_type = pad.electrical_type;
                attr.template = pad.template.clone();
                attr.library = pad.template_library.clone();
                attr.feature_top = pad.feature_top;
                attr.feature_bottom = pad.feature_bottom;
                attr.testpoint = pad.testpoint;
                attr.hole_tolerance_plus_mm = pad.hole_tolerance_plus_mm;
                attr.hole_tolerance_minus_mm = pad.hole_tolerance_minus_mm;
                attr.hole_rotation_deg = pad.hole_rotation_deg;
                attr.copper_offset_x_mm = pad.copper_offset_x_mm;
                attr.copper_offset_y_mm = pad.copper_offset_y_mm;
            }
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
}
