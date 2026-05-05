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

use signex_library::{Footprint, LayerId, Pad, PadKind, PadShape};

use super::layers::{FpLayer, LayerVisibility};

/// Default new-pad size in mm.
const NEW_PAD_SIZE_MM: f64 = 1.0;
/// Slack on each side of the pad bounding box when auto-fitting the
/// courtyard polygon.
const COURTYARD_SLACK_MM: f64 = 0.25;

/// One pad in the editor canvas. A subset of [`signex_library::Pad`] —
/// we only carry the fields the canvas renders or hit-tests. Extra
/// fields on `Pad` (drill, mask/paste margins, etc.) round-trip via
/// [`FootprintEditorState::sync_pads_to_primitive`] without a UI yet.
#[derive(Debug, Clone, PartialEq)]
pub struct EditorPad {
    pub number: String,
    pub position_mm: (f64, f64),
    pub size_mm: (f64, f64),
    pub kind: PadKind,
    pub shape: PadShape,
    /// Layers the pad lives on — first entry is treated as the
    /// primary layer for hit-test/visibility gating.
    pub layers: Vec<LayerId>,
    /// v0.15 — bidirectional sketch ↔ pads link. `Some(id)` when
    /// the pad has a backing sketch entity (a `Point` carrying a
    /// `PadAttr`); edits in either mode mirror through this ID.
    /// `None` for pads created in Pads mode before v0.15 and not
    /// yet mirrored, or footprints opened from disk on v0.14 or
    /// earlier (the auto-mint on first Sketch entry will populate
    /// these IDs going forward).
    pub sketch_entity_id: Option<signex_sketch::id::SketchEntityId>,
    /// v0.16 — outline-corner Points minted when the pad enters
    /// Sketch mode so the user sees / can pick the four corners
    /// directly. Order: `[ne, se, sw, nw]`. Construction-flagged so
    /// they're visual-only and don't affect `bake_pads`.
    pub corner_entity_ids: Option<[signex_sketch::id::SketchEntityId; 4]>,
    /// v0.16.6 — pad rotation in degrees. Round-trips via
    /// `Pad::rotation`. Editor canvas renders unrotated pads in
    /// v0.16.6 (rendering rotated pads is a v0.17 follow-up); the
    /// bake honours the value so saved files carry the correct
    /// rotation regardless.
    pub rotation_deg: f64,
    /// v0.18.12 — drill diameter (mm) for through-hole / NPT pads.
    /// `None` for SMD pads. Round-trips via `Pad::drill`. Mints as
    /// `Some(default_drill_mm)` for `Place Hole` clicks.
    pub drill_diameter_mm: Option<f64>,
}

impl EditorPad {
    pub fn new_default(number: String, position_mm: (f64, f64)) -> Self {
        Self {
            number,
            position_mm,
            size_mm: (NEW_PAD_SIZE_MM, NEW_PAD_SIZE_MM),
            kind: PadKind::Smd,
            shape: PadShape::Rect,
            layers: vec![
                LayerId::new("F.Cu"),
                LayerId::new("F.Mask"),
                LayerId::new("F.Paste"),
            ],
            sketch_entity_id: None,
            corner_entity_ids: None,
            rotation_deg: 0.0,
            drill_diameter_mm: None,
        }
    }

    /// v0.18.12 — non-plated through hole. No copper / mask / paste
    /// layers; the drill is the visible footprint feature. Default
    /// outer diameter equals the drill diameter so the hole renders
    /// as a circle of that size in the editor.
    pub fn new_npt_hole(number: String, position_mm: (f64, f64), drill_mm: f64) -> Self {
        let d = drill_mm.max(0.05);
        Self {
            number,
            position_mm,
            size_mm: (d, d),
            kind: PadKind::NptHole,
            shape: PadShape::Round,
            // No copper / mask / paste — bare drilled hole.
            layers: Vec::new(),
            sketch_entity_id: None,
            corner_entity_ids: None,
            rotation_deg: 0.0,
            drill_diameter_mm: Some(d),
        }
    }

    /// Layer the pad lives on for hit-testing / toggle gating.
    pub fn primary_layer(&self) -> FpLayer {
        self.layers
            .first()
            .and_then(|name| FpLayer::from_standard_name(name.as_str()))
            .unwrap_or(FpLayer::FCu)
    }

    /// Bounding box (min_x, min_y, max_x, max_y) in mm.
    pub fn bbox_mm(&self) -> (f64, f64, f64, f64) {
        let (cx, cy) = self.position_mm;
        let (w, h) = self.size_mm;
        (cx - w / 2.0, cy - h / 2.0, cx + w / 2.0, cy + h / 2.0)
    }

    /// AABB containment check.
    pub fn contains_mm(&self, x: f64, y: f64) -> bool {
        let (xmin, ymin, xmax, ymax) = self.bbox_mm();
        x >= xmin && x <= xmax && y >= ymin && y <= ymax
    }

    fn from_pad(p: &Pad) -> Self {
        Self {
            number: p.number.clone(),
            position_mm: (p.position[0], p.position[1]),
            size_mm: (p.size[0], p.size[1]),
            kind: p.kind,
            shape: p.shape.clone(),
            layers: p.layers.clone(),
            sketch_entity_id: None,
            corner_entity_ids: None,
            rotation_deg: p.rotation,
            drill_diameter_mm: p.drill.as_ref().map(|d| d.diameter),
        }
    }

    fn to_pad(&self) -> Pad {
        let drill = self.drill_diameter_mm.map(|d| signex_library::Drill {
            diameter: d,
            slot_length: None,
        });
        Pad {
            number: self.number.clone(),
            kind: self.kind,
            shape: self.shape.clone(),
            size: [self.size_mm.0, self.size_mm.1],
            position: [self.position_mm.0, self.position_mm.1],
            rotation: self.rotation_deg,
            layers: self.layers.clone(),
            drill,
            solder_mask_margin: None,
            paste_margin: None,
        }
    }
}

/// Footprint editor mode — gate sketch tooling on / off without
/// rewriting the canvas state machine. Phase 5.3 of the v0.13 sketch-
/// mode plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorMode {
    /// Direct pad-list editing (the existing Phase 0–v0.10 surface).
    #[default]
    Normal,
    /// Parametric sketch mode — Phase 6 UI lives behind this.
    Sketch,
    /// 3D body preview (existing v0.10 viewer).
    View3d,
}

/// Live, in-memory state of the Footprint canvas — drives interaction
/// and rendering. The authoritative pad list lives on
/// `ComponentEditorState.footprint.pads`; this struct mirrors it for
/// the canvas's hit-test + draw layer.
///
/// `PartialEq` is intentionally NOT derived: Phase 5.3 added
/// `sketch_solver` / `last_solve` / `auto_pause` whose underlying
/// types in `signex-sketch` don't implement `PartialEq`. The editor
/// uses pointer-equality / dirty-flag patterns elsewhere; no test or
/// production call site compared two `FootprintEditorState` values
/// for structural equality.
#[derive(Debug, Clone)]
pub struct FootprintEditorState {
    pub pads: Vec<EditorPad>,
    pub layer_visibility: LayerVisibility,
    /// `Some(idx)` while a pad is selected.
    pub selected_pad: Option<usize>,
    /// `true` when the courtyard polygon should track the pad bbox.
    pub auto_fit_courtyard: bool,
    pub courtyard_mm: Option<CourtyardRect>,
    /// Last known cursor world position in mm — drives the footer
    /// readout.
    pub cursor_mm: Option<(f64, f64)>,
    /// Phase 5.3: which editor mode the user has switched to.
    pub mode: EditorMode,
    /// Phase 5.3: shared LM solver config + last solution. Cloned
    /// per-edit by the dispatcher so the editor's clone-per-frame iced
    /// flow keeps working without `Arc`. The Solver struct is small
    /// (three numbers); `last_solve` is replaced wholesale on every
    /// solve.
    pub sketch_solver: signex_sketch::solver::Solver,
    /// Output of the most recent solve — `None` until first solve.
    /// Carried so the canvas DOF overlay + render layer can read the
    /// solved entity coordinates without rerunning the LM iteration.
    pub last_solve: Option<signex_sketch::solver::FullSolveOutput>,
    /// Hysteresis state for live-solve auto-pause. Phase 3.6 ships
    /// `AutoPauseState`; the dispatcher feeds elapsed_ms into it on
    /// every solve.
    pub auto_pause: signex_sketch::solver::timeout::AutoPauseState,
    /// Last solve's audit / over-constraint warnings. Cleared per
    /// solve. Surfaced by the inspector panel in Phase 6.
    pub solve_warnings: Vec<String>,
    /// v0.13.2 — currently-active sketch tool. The inspector tool
    /// palette emits `FootprintSketchSetTool(...)`; the canvas
    /// Program reads this to interpret pointer events. `Select`
    /// is the default and means "no drawing tool active".
    pub active_tool: SketchTool,
    /// v0.13.2 — transient state for multi-click tools (Line / Arc /
    /// Circle). Stashes the first / second clicks until the gesture
    /// completes; cleared on commit or Esc.
    pub tool_pending: ToolPending,
    /// v0.13.3 — currently-selected sketch entity. `None` means no
    /// selection. Drives the inspector's selection-aware constraint
    /// submenu; also drives the canvas drag-to-move gesture for
    /// Point entities.
    pub selected_sketch: Option<signex_sketch::id::SketchEntityId>,
    /// v0.13.3 — secondary selected sketch entity. Used so the
    /// inspector's constraint submenu can detect "two entities
    /// selected" cases (Coincident, Distance, Parallel, etc.).
    /// `Shift+Click` adds to the selection; clicking empty canvas
    /// clears both.
    pub selected_sketch_secondary: Option<signex_sketch::id::SketchEntityId>,
    /// v0.13.3 — Dimension tool's pending value (text input). The
    /// user picks a number, the inspector emits the AddConstraint
    /// for the active selection.
    pub dimension_input: String,
    /// v0.15 — Pads-mode tool. Default `Select`; switching to
    /// `PlacePad` makes empty-canvas clicks drop a new pad at the
    /// cursor instead of clearing the selection. Right-click / Esc
    /// returns to Select. Mirrors the sketch-mode `active_tool`
    /// state machine.
    pub pads_tool: PadsTool,
    /// v0.16.1 — sticky construction-mode toggle on the sketch
    /// active bar. When `true`, every newly-minted Line / Arc /
    /// Circle / Rectangle / RoundedRectangle / Point gets
    /// `entity.construction = true`, which the bake skips and the
    /// canvas renders as dashed grey. Construction geometry is
    /// useful for guides + symmetry without affecting the baked
    /// pad / silk / courtyard output.
    pub construction_mode: bool,
    /// v0.16.1 — TAB pause-during-placement. When `true`, the canvas
    /// ignores empty-canvas clicks during PadsTool::PlacePad so the
    /// user can adjust pad-stack defaults before resuming. TAB
    /// toggles. Mirrors the schematic's pre-placement / Resume
    /// pattern; the in-flight `pads_tool` survives a pause/resume.
    pub placement_paused: bool,
    /// v0.16.3 — defaults applied to the next pad created via
    /// `PadsTool::PlacePad`. Surfaced as input fields in the right-
    /// dock Properties panel under the "Pad placement" section so
    /// the user can pick a custom designator + size BEFORE the click
    /// (TAB pause/resume gives them time to type). Without this
    /// the canvas always minted 1×1 mm "1"-numbered pads.
    pub next_pad_defaults: NextPadDefaults,
    /// v0.17.0 — empty-canvas Snap Options. Each flag gates one of
    /// the four priorities in `snap::snap_cursor`. UI-toggled via
    /// the Properties panel default branch.
    pub snap_options: SnapOptions,
    /// v0.18.13 — Altium Selection Filter pill row state. Per-
    /// editor so flipping pads off in one tab doesn't follow the
    /// user into another footprint.
    pub selection_filter: SelectionFilter,
    /// v0.18.13 — active sub-tab in the Snap Options section
    /// (Grids / Guides / Axes).
    pub snap_subtab: SnapSubTab,
    /// v0.18.13 — Snapping 3-state (All Layers / Current Layer /
    /// Off). Stored for forward compatibility; the actual layer-
    /// aware enforcement lands with the PCB layer system. `Off`
    /// short-circuits all snap priorities to raw cursor today.
    pub snapping_mode: SnappingMode,
}

/// v0.16.3 — author-controlled defaults for the next placed pad.
/// `designator_override = Some("U1A")` overrides the auto-incrementing
/// numeric designator; `None` means "use next-pad-number". Size is
/// always in mm. Side controls which copper layer the pad lands on.
/// v0.16.6 — `rotation_deg` controls the orientation of the next
/// pad in degrees (CCW positive).
#[derive(Debug, Clone, PartialEq)]
pub struct NextPadDefaults {
    pub designator_override: Option<String>,
    pub size_x_mm: f64,
    pub size_y_mm: f64,
    pub side: PadSide,
    pub rotation_deg: f64,
}

/// Pad copper side mirror — kept here so the panel doesn't have to
/// import `signex_sketch::attr::PadSide`. v0.16.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PadSide {
    #[default]
    Top,
    Bottom,
    All,
}

impl PadSide {
    pub const ALL_OPTIONS: &'static [PadSide] = &[PadSide::Top, PadSide::Bottom, PadSide::All];
    pub fn label(self) -> &'static str {
        match self {
            PadSide::Top => "Top",
            PadSide::Bottom => "Bottom",
            PadSide::All => "All (THT)",
        }
    }
}

impl std::fmt::Display for PadSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// v0.17.0 — per-priority snap toggles surfaced on the empty-canvas
/// Properties panel. Mirrors Altium's "Snap Options" checklist. Each
/// flag gates one priority in `snap::snap_cursor`; defaults are all
/// `true` so existing behaviour is preserved when no toggling has
/// happened.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SnapOptions {
    /// Snap onto an existing sketch Point within `POINT_SNAP_RADIUS_PX`.
    pub point_hit: bool,
    /// Horizontal / Vertical inference within `AXIS_THRESHOLD_DEG`.
    pub horizontal_vertical: bool,
    /// Multi-of-`ANGLE_STEP_DEG` snap within `ANGLE_THRESHOLD_DEG`.
    pub angle: bool,
    /// Round to the nearest `grid_step_mm`. When `false` the cursor
    /// passes through raw — useful for free-hand authoring.
    pub grid: bool,
    /// v0.18.9 — author-controlled grid step in mm. Replaces the
    /// hardcoded `snap::GRID_STEP_MM`. The Properties panel exposes
    /// this as a numeric input; the v0.18.10 `G`-key popup picks
    /// from the Altium-standard ladder
    /// (1/5/10/20/25/50/100 mil + 0.025/0.1/0.25/0.5/1.0/2.5 mm).
    pub grid_step_mm: f64,
}

impl Default for SnapOptions {
    fn default() -> Self {
        Self {
            point_hit: true,
            horizontal_vertical: true,
            angle: true,
            grid: true,
            grid_step_mm: 1.0,
        }
    }
}

impl Default for NextPadDefaults {
    fn default() -> Self {
        Self {
            designator_override: None,
            size_x_mm: NEW_PAD_SIZE_MM,
            size_y_mm: NEW_PAD_SIZE_MM,
            side: PadSide::Top,
            rotation_deg: 0.0,
        }
    }
}

/// v0.18.13 — Altium-style Selection Filter. Each flag gates whether
/// the corresponding kind is selectable in the canvas. `Pads` is the
/// only one functionally wired today; the others are stored for
/// forward compatibility so the pill row reflects user intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionFilter {
    pub pads: bool,
    pub tracks: bool,
    pub arcs: bool,
    pub pours: bool,
    pub bodies_3d: bool,
    pub keepouts: bool,
    pub cutouts: bool,
    pub texts: bool,
}

impl Default for SelectionFilter {
    fn default() -> Self {
        Self {
            pads: true,
            tracks: true,
            arcs: true,
            pours: true,
            bodies_3d: true,
            keepouts: true,
            cutouts: true,
            texts: true,
        }
    }
}

/// Selection-filter pill identifier — drives the panel pill row +
/// the dispatcher's mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionFilterKind {
    Pads,
    Tracks,
    Arcs,
    Pours,
    Bodies3d,
    Keepouts,
    Cutouts,
    Texts,
}

impl SelectionFilter {
    pub fn get(&self, kind: SelectionFilterKind) -> bool {
        match kind {
            SelectionFilterKind::Pads => self.pads,
            SelectionFilterKind::Tracks => self.tracks,
            SelectionFilterKind::Arcs => self.arcs,
            SelectionFilterKind::Pours => self.pours,
            SelectionFilterKind::Bodies3d => self.bodies_3d,
            SelectionFilterKind::Keepouts => self.keepouts,
            SelectionFilterKind::Cutouts => self.cutouts,
            SelectionFilterKind::Texts => self.texts,
        }
    }

    pub fn toggle(&mut self, kind: SelectionFilterKind) {
        match kind {
            SelectionFilterKind::Pads => self.pads = !self.pads,
            SelectionFilterKind::Tracks => self.tracks = !self.tracks,
            SelectionFilterKind::Arcs => self.arcs = !self.arcs,
            SelectionFilterKind::Pours => self.pours = !self.pours,
            SelectionFilterKind::Bodies3d => self.bodies_3d = !self.bodies_3d,
            SelectionFilterKind::Keepouts => self.keepouts = !self.keepouts,
            SelectionFilterKind::Cutouts => self.cutouts = !self.cutouts,
            SelectionFilterKind::Texts => self.texts = !self.texts,
        }
    }
}

/// v0.18.13 — Altium Snap Options sub-tabs (Grids / Guides / Axes).
/// `Grids` is active by default; the other two are visual placeholders
/// for the v0.18.14 Guide Manager / Axes Manager content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SnapSubTab {
    #[default]
    Grids,
    Guides,
    Axes,
}

/// v0.18.13 — Altium Snapping mode (3-state segment). Visual today;
/// the actual layer-aware filtering will land alongside the PCB
/// layer system. Defaults to `AllLayers` (current behaviour).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SnappingMode {
    #[default]
    AllLayers,
    CurrentLayer,
    Off,
}

/// Pads-mode drawing tool — v0.15. The Pads-mode active bar's
/// "Place Pad" button switches to `PlacePad`; right-click cancels
/// back to `Select`. Selecting a pad uses the existing pad-click
/// hit-test and works regardless of `pads_tool`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PadsTool {
    #[default]
    Select,
    /// Click empty canvas → adds a new pad at the cursor.
    PlacePad,
    /// v0.18.12 — non-plated through hole. 1-click drop. Mints a
    /// `Pad` with `kind = NptHole`, no copper / mask / paste, drill
    /// at `next_pad_defaults.size_x_mm` (default 1mm).
    PlaceHole,
}

/// Sketch-mode drawing tool. Phase 6.3 (v0.13.1) shipped Place Point
/// only; v0.13.2 adds Line, Circle, Arc; v0.15 adds Rectangle (two-
/// click corner-to-corner; emits 4 Lines + corner Points). v0.16 adds
/// RoundedRectangle (two-click corner-to-corner with corner radius
/// from `dimension_input`; emits 4 Points + 4 Lines + 4 Arcs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SketchTool {
    #[default]
    Select,
    Point,
    Line,
    Rectangle,
    RoundedRectangle,
    Circle,
    Arc,
}

/// Transient per-tool gesture state. The canvas Program reads + writes
/// it through editor messages so the iced update loop can persist it
/// across renders without coupling the canvas's internal `cstate`
/// (which is local to the canvas program) to the editor's serialised
/// state.
#[derive(Debug, Clone, Default)]
pub enum ToolPending {
    #[default]
    Idle,
    /// Line tool, first click landed (anchor point exists in sketch).
    LineFirst {
        first: signex_sketch::id::SketchEntityId,
    },
    /// Rectangle tool, first corner click landed (anchor point in sketch).
    /// v0.15. Click 2 commits the opposite corner; the dispatcher
    /// adds 4 Lines + 2 new corner Points (opposite + 2 mid-axis).
    RectangleFirst {
        first: signex_sketch::id::SketchEntityId,
    },
    /// Rounded-Rectangle tool, first corner click landed. v0.16.
    /// Click 2 commits the opposite corner; the dispatcher emits 4
    /// corner-of-rect Points + 4 Lines (axis-aligned, shortened by
    /// the corner radius) + 4 Arcs (one per corner). Radius reads
    /// from `dimension_input` (defaults to 0.5 mm if blank).
    RoundedRectangleFirst {
        first: signex_sketch::id::SketchEntityId,
    },
    /// Circle tool, centre click landed.
    CircleCenter {
        center: signex_sketch::id::SketchEntityId,
    },
    /// Arc tool, centre click landed.
    ArcCenter {
        center: signex_sketch::id::SketchEntityId,
    },
    /// Arc tool, centre + start clicks landed; awaiting end click.
    ArcStart {
        center: signex_sketch::id::SketchEntityId,
        start: signex_sketch::id::SketchEntityId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CourtyardRect {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl FootprintEditorState {
    /// Build canvas state from the primitive's pad list.
    pub fn from_footprint(fp: &Footprint) -> Self {
        let pads = fp.pads.iter().map(EditorPad::from_pad).collect();
        let mut s = Self {
            pads,
            layer_visibility: LayerVisibility::default(),
            selected_pad: None,
            auto_fit_courtyard: true,
            courtyard_mm: None,
            cursor_mm: None,
            mode: EditorMode::Normal,
            sketch_solver: signex_sketch::solver::Solver::default(),
            last_solve: None,
            auto_pause: signex_sketch::solver::timeout::AutoPauseState::default(),
            solve_warnings: Vec::new(),
            active_tool: SketchTool::default(),
            tool_pending: ToolPending::default(),
            selected_sketch: None,
            selected_sketch_secondary: None,
            dimension_input: String::new(),
            pads_tool: PadsTool::default(),
            construction_mode: false,
            placement_paused: false,
            next_pad_defaults: NextPadDefaults::default(),
            snap_options: SnapOptions::default(),
            selection_filter: SelectionFilter::default(),
            snap_subtab: SnapSubTab::default(),
            snapping_mode: SnappingMode::default(),
        };
        s.recompute_courtyard();
        s
    }

    /// Empty state — used for brand-new components and as the fallback
    /// when the binding has no footprint primitive yet.
    #[allow(dead_code)]
    pub fn empty() -> Self {
        let mut s = Self {
            pads: Vec::new(),
            layer_visibility: LayerVisibility::default(),
            selected_pad: None,
            auto_fit_courtyard: true,
            courtyard_mm: None,
            cursor_mm: None,
            mode: EditorMode::Normal,
            sketch_solver: signex_sketch::solver::Solver::default(),
            last_solve: None,
            auto_pause: signex_sketch::solver::timeout::AutoPauseState::default(),
            solve_warnings: Vec::new(),
            active_tool: SketchTool::default(),
            tool_pending: ToolPending::default(),
            selected_sketch: None,
            selected_sketch_secondary: None,
            dimension_input: String::new(),
            pads_tool: PadsTool::default(),
            construction_mode: false,
            placement_paused: false,
            next_pad_defaults: NextPadDefaults::default(),
            snap_options: SnapOptions::default(),
            selection_filter: SelectionFilter::default(),
            snap_subtab: SnapSubTab::default(),
            snapping_mode: SnappingMode::default(),
        };
        s.recompute_courtyard();
        s
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

    /// Click-add a new pad at the given world position. v0.16.3 — the
    /// new pad applies the user-controlled `next_pad_defaults` so the
    /// canvas mints whatever designator + size + side the Properties
    /// panel form has captured. Falls back to auto-incrementing
    /// designator when `designator_override` is `None` (default).
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
        pad.layers = layers;
        pad.rotation_deg = defaults.rotation_deg;
        self.pads.push(pad);
        let idx = self.pads.len() - 1;
        self.selected_pad = Some(idx);
        self.recompute_courtyard();
        idx
    }

    /// v0.18.12 — click-add a non-plated through hole at the given
    /// world position. Drill diameter inherits the
    /// `next_pad_defaults.size_x_mm` setting so the user can dial it
    /// in via the Properties panel before placing.
    pub fn add_hole_at(&mut self, x_mm: f64, y_mm: f64) -> usize {
        let defaults = self.next_pad_defaults.clone();
        let number = defaults
            .designator_override
            .clone()
            .unwrap_or_else(|| self.next_pad_number());
        let drill_mm = defaults.size_x_mm.max(0.05);
        let pad = EditorPad::new_npt_hole(number, (x_mm, y_mm), drill_mm);
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
        self.selected_pad = match self.selected_pad {
            Some(sel) if sel == idx => None,
            Some(sel) if sel > idx => Some(sel - 1),
            other => other,
        };
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

    /// Write the canvas-side pad list back onto the primitive. Called
    /// after every mutation so the saved row sees the current pad
    /// layout. Other Footprint fields (graphics, body_3d, etc.) are
    /// left untouched — they're edited by their own panes.
    pub fn sync_pads_to_primitive(canvas: &Self, fp: &mut Footprint) {
        fp.pads = canvas.pads.iter().map(EditorPad::to_pad).collect();
        // Auto-fit courtyard is mirrored as a Polygon for downstream
        // PCB renderers.
        if let Some(c) = canvas.courtyard_mm {
            fp.courtyard = signex_library::Polygon::new(vec![
                [c.min_x, c.min_y],
                [c.max_x, c.min_y],
                [c.max_x, c.max_y],
                [c.min_x, c.max_y],
            ]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_library::Footprint;

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
