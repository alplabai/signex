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
use signex_sketch::attr::{ElectricalType, PadFeature, TestpointFlags};

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
    /// v0.20 — Altium-parity pad-stack overrides. Per-side paste +
    /// mask + tented + thermal-relief + corner radius. Each value
    /// surfaces in the right-dock Properties panel "Pad Stack"
    /// section and round-trips through `Pad`'s matching fields.
    pub stack: PadStackUi,
    /// v0.20 — top-side surface treatment (Solder Bumps / Glue Dots
    /// / Adhesive Beads). `PadFeature::None` = bare copper.
    pub feature_top: PadFeature,
    /// v0.20 — bottom-side surface treatment.
    pub feature_bottom: PadFeature,
    /// v0.20 — test-point participation flags (top/bottom × assembly
    /// /fab). All `false` = not a test point.
    pub testpoint: TestpointFlags,
    /// v0.20 — pad-template name. Empty = no template.
    pub template: String,
    /// v0.20 — pad-template library reference. Empty = local.
    pub template_library: String,
    /// v0.20 — Altium-parity electrical-type (Load/Source/Terminator).
    pub electrical_type: ElectricalType,
    /// v0.20 — net assignment. Empty = unassigned.
    pub net: String,
    /// v0.20 — locked flag. `true` resists drag/delete/move-by-arrow.
    pub locked: bool,
    /// v0.20 — Pad Hole tolerance ± (mm). Reporting only — drives
    /// IPC-356 / drill-table export, not DRC.
    pub hole_tolerance_plus_mm: Option<f64>,
    pub hole_tolerance_minus_mm: Option<f64>,
    /// v0.20 — Pad Hole rotation (Slot/Rectangular orientation).
    pub hole_rotation_deg: Option<f64>,
    /// v0.20 — Copper offset relative to hole centre.
    pub copper_offset_x_mm: Option<f64>,
    pub copper_offset_y_mm: Option<f64>,
    /// v0.24 Phase 1 (Track A stub) — Per-pad parametric handles for
    /// shape-driven attributes. Maps a feature key (e.g.
    /// `"corner_r"`, `"chamfer_len"`, `"diameter"`) to a sketch-
    /// parameter name. Phase 2 (Agent A) populates this on
    /// `mirror_add_pad_to_sketch` and writes the corresponding
    /// parameter into `sketch.parameters`. The `LinkedRadius` enum
    /// in `signex-sketch::attr` controls "shared by default,
    /// splittable on right-click" semantics. Empty (= no parametric
    /// handles bound) on existing footprints — the Phase 2 mirror
    /// builds it lazily on first sketch-mode visit.
    pub shape_params: ShapeParamMap,
}

/// v0.24 Phase 1 (Track A stub) — per-pad parametric handle map.
/// Type alias keeps the field flexible for Phase 2 to swap in a
/// dedicated struct if the linked/unlinked semantics get richer
/// (e.g. per-corner overrides for Chamfered).
pub type ShapeParamMap = std::collections::HashMap<String, String>;

/// v0.20 — UI-side mirror of `Pad`'s pad-stack override fields. All
/// values in mm (already evaluated). `None` on a margin override
/// means "use the rule-driven / global value"; `true` on a tented
/// flag means "skip the mask opening on that side".
///
/// `Default` matches Altium's "out-of-the-box" pad: paste enabled on
/// both sides, mask not tented, thermal relief on, no margin
/// overrides, no explicit corner-radius percentage.
#[derive(Debug, Clone, PartialEq)]
pub struct PadStackUi {
    pub paste_margin_top: Option<f64>,
    pub paste_margin_bottom: Option<f64>,
    pub paste_enabled_top: bool,
    pub paste_enabled_bottom: bool,
    pub mask_margin_top: Option<f64>,
    pub mask_margin_bottom: Option<f64>,
    pub mask_tented_top: bool,
    pub mask_tented_bottom: bool,
    pub thermal_relief: bool,
    pub corner_radius_pct: Option<f64>,
}

impl Default for PadStackUi {
    fn default() -> Self {
        Self {
            paste_margin_top: None,
            paste_margin_bottom: None,
            paste_enabled_top: true,
            paste_enabled_bottom: true,
            mask_margin_top: None,
            mask_margin_bottom: None,
            mask_tented_top: false,
            mask_tented_bottom: false,
            thermal_relief: true,
            corner_radius_pct: None,
        }
    }
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
            stack: PadStackUi::default(),
            feature_top: PadFeature::None,
            feature_bottom: PadFeature::None,
            testpoint: TestpointFlags::default(),
            template: String::new(),
            template_library: String::new(),
            electrical_type: ElectricalType::Load,
            net: String::new(),
            locked: false,
            hole_tolerance_plus_mm: None,
            hole_tolerance_minus_mm: None,
            hole_rotation_deg: None,
            copper_offset_x_mm: None,
            copper_offset_y_mm: None,
            shape_params: ShapeParamMap::new(),
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
            stack: PadStackUi::default(),
            feature_top: PadFeature::None,
            feature_bottom: PadFeature::None,
            testpoint: TestpointFlags::default(),
            template: String::new(),
            template_library: String::new(),
            electrical_type: ElectricalType::Load,
            net: String::new(),
            locked: false,
            hole_tolerance_plus_mm: None,
            hole_tolerance_minus_mm: None,
            hole_rotation_deg: None,
            copper_offset_x_mm: None,
            copper_offset_y_mm: None,
            shape_params: ShapeParamMap::new(),
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
            stack: PadStackUi {
                paste_margin_top: p.paste_margin_top,
                paste_margin_bottom: p.paste_margin_bottom,
                paste_enabled_top: p.paste_enabled_top,
                paste_enabled_bottom: p.paste_enabled_bottom,
                mask_margin_top: p.mask_margin_top,
                mask_margin_bottom: p.mask_margin_bottom,
                mask_tented_top: p.mask_tented_top,
                mask_tented_bottom: p.mask_tented_bottom,
                thermal_relief: p.thermal_relief,
                corner_radius_pct: p.corner_radius_pct,
            },
            feature_top: p.feature_top,
            feature_bottom: p.feature_bottom,
            testpoint: p.testpoint,
            template: p.template.clone(),
            template_library: p.template_library.clone(),
            electrical_type: p.electrical_type,
            net: p.net.clone(),
            locked: p.locked,
            hole_tolerance_plus_mm: p.hole_tolerance_plus_mm,
            hole_tolerance_minus_mm: p.hole_tolerance_minus_mm,
            hole_rotation_deg: p.hole_rotation_deg,
            copper_offset_x_mm: p.copper_offset_x_mm,
            copper_offset_y_mm: p.copper_offset_y_mm,
            shape_params: ShapeParamMap::new(),
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
            template: self.template.clone(),
            template_library: self.template_library.clone(),
            paste_margin_top: self.stack.paste_margin_top,
            paste_margin_bottom: self.stack.paste_margin_bottom,
            paste_enabled_top: self.stack.paste_enabled_top,
            paste_enabled_bottom: self.stack.paste_enabled_bottom,
            mask_margin_top: self.stack.mask_margin_top,
            mask_margin_bottom: self.stack.mask_margin_bottom,
            mask_tented_top: self.stack.mask_tented_top,
            mask_tented_bottom: self.stack.mask_tented_bottom,
            thermal_relief: self.stack.thermal_relief,
            corner_radius_pct: self.stack.corner_radius_pct,
            feature_top: self.feature_top,
            feature_bottom: self.feature_bottom,
            testpoint: self.testpoint,
            electrical_type: self.electrical_type,
            net: self.net.clone(),
            locked: self.locked,
            hole_tolerance_plus_mm: self.hole_tolerance_plus_mm,
            hole_tolerance_minus_mm: self.hole_tolerance_minus_mm,
            hole_rotation_deg: self.hole_rotation_deg,
            copper_offset_x_mm: self.copper_offset_x_mm,
            copper_offset_y_mm: self.copper_offset_y_mm,
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
/// `sketch_solver` / `last_solve` whose underlying
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
    /// Last solve's audit / over-constraint warnings. Cleared per
    /// solve. Surfaced by the inspector panel. v0.22 — solver
    /// timeouts are also surfaced here as a hard warning instead of
    /// being silently swallowed by the old auto-pause hysteresis.
    pub solve_warnings: Vec<String>,
    /// v0.22 Phase E3+E4 polish — `true` while the cursor is over
    /// any row in the Properties-panel "Conflicts (worst first)"
    /// list. The canvas's `draw_constraint_icons` reads this and
    /// dims every constraint icon EXCEPT the over-constrained ones,
    /// visually isolating the redundant set so the user can see at
    /// a glance which glyphs are conflicting. `false` = default
    /// rendering (over-constrained red, others muted at 0.85).
    /// v0.22 Phase E3+E4 → v0.23 per-row precision: `Some(id)` when
    /// the user is hovering a specific row in the Conflicts list;
    /// the canvas renders that row's constraint at full red and dims
    /// every other constraint glyph. `None` = no hover; default
    /// rendering applies.
    pub conflicts_row_hovered: Option<signex_sketch::id::ConstraintId>,
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
    /// v0.22 Phase A5 — Centerline-mode mint flag. Sister to
    /// `construction_mode`: while on, every newly-minted entity gets
    /// `centerline = true`, rendered as long-dash gold and skipped
    /// by the bake. Mutually exclusive with `construction_mode` —
    /// enabling one disables the other (Fusion convention).
    pub centerline_mode: bool,
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
    /// v0.18.15.1 — first click of an in-flight Place Track
    /// gesture. `Some((x, y))` after the first click; the second
    /// click commits `(track_first, second_click)` to silk_f and
    /// re-stashes the second click here for chained tracks. Esc /
    /// right-click clears.
    pub track_first: Option<(f64, f64)>,
    /// v0.18.15.3 — Place Arc 3-click gesture state.
    pub place_arc_pending: PlaceArcPending,
    /// v0.18.15.4 — Place Polygon vertex stash. Each click appends.
    /// Tool switch / Esc commits if ≥ 3 vertices, otherwise drops.
    pub place_polygon_vertices: Vec<(f64, f64)>,
    /// v0.18.18 — currently-selected silk-front graphic index (into
    /// `primitive.silk_f`). `None` until the user clicks one.
    /// Independent of `selected_pad` — pad selection clears silk
    /// selection and vice versa.
    pub selected_silk_f: Option<usize>,
    /// v0.18.20 — Altium-style guide lines (vertical / horizontal).
    /// Per-editor; the snap-to-guides hook follows in a v0.18.20+
    /// patch (today they're visual-only).
    pub guides: Vec<Guide>,
    /// v0.18.21 — Altium-style grid table (multiple named grids with
    /// independent step / display style / multiplier). The currently
    /// active row's step / display style / multiplier are mirrored to
    /// `snap_options` so the canvas + snap logic keep their existing
    /// single-grid code paths. Grids must contain at least one entry;
    /// `default_seeded` constructs the implicit "Global Snap Grid"
    /// row that mirrors `SnapOptions::default()`.
    pub grids: Vec<GridDef>,
    /// v0.18.21 — index into `grids` of the row whose step / display
    /// style currently drives the canvas + snap. Always valid: clamps
    /// to `grids.len() - 1` on delete.
    pub active_grid_idx: usize,
    /// v0.18.25.1 — mirror of the global `ui_state.snap_enabled`
    /// status-bar toggle. The footprint snap chain is otherwise
    /// driven by the per-editor `snapping_mode` 3-state; this field
    /// gives the global "Snap off" toggle a hook into the same
    /// short-circuit logic so guides + grid + point-hit all stop
    /// firing together. Synced by the StatusBar::ToggleSnap handler.
    pub global_snap_disabled: bool,
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
    /// v0.13 — Open active-bar dropdown menu. `None` = no menu open.
    /// Set by right-clicking (or clicking the chevron on) a tool button
    /// in the active bar; cleared on item-pick or click-outside.
    pub active_bar_menu: Option<FpActiveBarMenu>,
    /// v0.20 — Pad Stack panel tab (Simple / Top-Middle-Bottom /
    /// Full Stack). UI-only state; not persisted. Drives which Pad
    /// Stack body the right-dock Properties panel renders. Mirrors
    /// the Altium PCB Library tab strip on the same section.
    pub pad_stack_tab: PadStackTab,
    /// v0.24 Phase 1 (Track D) — live numeric input during sketch-tool
    /// placement. Set when the user types a digit while a tool is
    /// pending; the next click commits at the typed length / radius
    /// instead of the cursor position. `None` (default) preserves the
    /// v0.22 click-only flow.
    pub placement_input: Option<PlacementInput>,
}

/// v0.24 Phase 1 (Track D stub) — numeric-input overlay state for
/// sketch-tool placement. Phase 2 (Agent D) wires the keypress
/// handler + the cursor overlay; Phase 1 just declares the field
/// so other agents don't compete for the insertion point on
/// `FootprintEditorState`.
#[derive(Debug, Clone)]
pub struct PlacementInput {
    /// User-typed digits (and optional decimal point / minus).
    pub buffer: String,
    /// Which dimension the buffer represents. Drives unit interpretation
    /// + commit math at click time.
    pub kind: PlacementInputKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementInputKind {
    /// Line tool — second click commits at exactly `buffer` mm from
    /// the first endpoint, along the cursor's azimuth.
    LineLength,
    /// Circle tool — radius commit; second click ignores cursor delta.
    CircleRadius,
    /// Arc tool radius — second click ignores cursor delta from centre.
    ArcRadius,
    /// Arc tool sweep angle (degrees) — third click commits at the
    /// typed sweep relative to start.
    ArcSweep,
}

/// v0.20 — Pad Stack section's tab strip. Matches Altium's three
/// tabs verbatim:
/// - `Simple`: one row per stack family (COPPER / HOLE / PASTE /
///   SOLDER). Default.
/// - `TopMiddleBottom`: COPPER splits into Top / Middle / Bottom
///   rows. Middle is a placeholder for inner-copper pad-stack
///   overrides; surfaces in v0.21+ once the schema lands.
/// - `FullStack`: enumerates the pad's `layers` list verbatim, one
///   row per layer with the matching margin/expansion field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PadStackTab {
    #[default]
    Simple,
    TopMiddleBottom,
    FullStack,
}

impl PadStackTab {
    pub const ALL: &'static [PadStackTab] = &[
        PadStackTab::Simple,
        PadStackTab::TopMiddleBottom,
        PadStackTab::FullStack,
    ];
    pub fn label(self) -> &'static str {
        match self {
            PadStackTab::Simple => "Simple",
            PadStackTab::TopMiddleBottom => "Top-Middle-Bottom",
            PadStackTab::FullStack => "Full Stack",
        }
    }
}

/// v0.13 — Altium-style footprint active bar dropdown menus. One per
/// chevron-bearing button in `pads_active_bar`. The dropdown overlay
/// reads this enum to render the matching menu body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FpActiveBarMenu {
    /// Selection Filter — 10 footprint-kind toggle pills.
    Filter,
    /// Snap Options — Grids/Guides/Axes pills + 12 snap-object pills.
    Snap,
    /// Place / Move / Drag — gesture menu (Move, Drag, Break Track,
    /// Drag Track End, Move Selection, Move Selection by X Y,
    /// Rotate Selection, Flip Selection).
    Place,
    /// Selection — selection-mode picker (Select overlapped, Select
    /// next, Lasso Select, Inside Area, Outside Area, Touching
    /// Rectangle, Touching Line, All on Layer, All, Off Grid Pads,
    /// Toggle Selection).
    Select,
    /// Align — full Altium align/distribute menu.
    Align,
    /// 3D Body — 3D Body, Extruded 3D Body.
    Body3d,
    /// Text — String, Text Frame.
    Text,
    /// Shapes — Line, Arc (Center), Arc (Edge), Arc (Any Angle),
    /// Full Circle, Fill, Solid Region, Rectangle. Sketch-mode only.
    Shapes,
}

/// v0.16.3 — author-controlled defaults for the next placed pad.
/// `designator_override = Some("U1A")` overrides the auto-incrementing
/// numeric designator; `None` means "use next-pad-number". Size is
/// always in mm. Side controls which copper layer the pad lands on.
/// v0.16.6 — `rotation_deg` controls the orientation of the next
/// pad in degrees (CCW positive).
/// v0.20 — also carries the Altium-parity Pad Stack (per-side mask /
/// paste / tented / thermal / corner-radius), Pad Features (top /
/// bottom surface treatment), Testpoint flags, and Template /
/// Library reference fields. Mirrors `EditorPad` so the Properties
/// panel renders the same form before-placement and after-selection.
#[derive(Debug, Clone, PartialEq)]
pub struct NextPadDefaults {
    pub designator_override: Option<String>,
    pub size_x_mm: f64,
    pub size_y_mm: f64,
    pub side: PadSide,
    pub rotation_deg: f64,
    /// Per-side pad-stack overrides. Mirror of `EditorPad.stack`.
    pub stack: PadStackUi,
    /// Top-side surface feature.
    pub feature_top: PadFeature,
    /// Bottom-side surface feature.
    pub feature_bottom: PadFeature,
    /// Test-point participation flags.
    pub testpoint: TestpointFlags,
    /// Pad-template name. Empty = no template.
    pub template: String,
    /// Pad-template library reference. Empty = local.
    pub template_library: String,
    /// v0.20 — Drill diameter (mm) for the next placed pad. `None`
    /// = SMD pad (no hole). `Some(d)` = THT/NPT pad with the given
    /// diameter. Surfaced in the Pad Stack → HOLE → Hole size row.
    pub drill_diameter_mm: Option<f64>,
    /// v0.20 — Drill slot length (mm). `None` = round drill;
    /// `Some(l)` = oval slot of length l along the slot's long axis.
    /// Drives the HOLE → Shape pick_list (Round vs Slot) and the
    /// "Slot length" input visibility.
    pub drill_slot_length_mm: Option<f64>,
    /// v0.20 — Copper shape for the next placed pad. Mirror of
    /// `EditorPad.shape`. Drives the Pad Stack → COPPER → Shape
    /// pick_list and the 3D preview.
    pub shape: signex_library::PadShape,
    /// v0.20 — Pad mounting kind (SMD / THT / NptHole / etc) for
    /// the next placed pad. Mirror of `EditorPad.kind`. Persisted
    /// to `Pad::kind` at bake time.
    pub kind: signex_library::PadKind,
    /// v0.20 — Altium-parity electrical-type (Load/Source/Terminator).
    pub electrical_type: ElectricalType,
    /// v0.20 — net assignment.
    pub net: String,
    /// v0.20 — locked flag.
    pub locked: bool,
    /// v0.20 — Pad Hole tolerance ±.
    pub hole_tolerance_plus_mm: Option<f64>,
    pub hole_tolerance_minus_mm: Option<f64>,
    pub hole_rotation_deg: Option<f64>,
    pub copper_offset_x_mm: Option<f64>,
    pub copper_offset_y_mm: Option<f64>,
}

/// HI-25 helper: when an item is removed at `removed_idx` from a Vec,
/// fold the change into a `selected: Option<usize>` so it still points
/// at the right element (or clears to `None` if the selection is what
/// got deleted). Used by the pad / silk / drawing deletion paths so
/// the "selection became dangling after delete" bug class can't recur.
///
/// - `None`                 → `None`
/// - `Some(sel)` if `sel == removed_idx` → `None` (was selected; gone)
/// - `Some(sel)` if `sel < removed_idx`  → `Some(sel)` (unaffected)
/// - `Some(sel)` if `sel > removed_idx`  → `Some(sel - 1)` (shifted left)
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

/// Pad copper side mirror — UI-side label-bearing enum. The sketch
/// crate has the same shape at `signex_sketch::attr::PadSide`; this
/// type wraps it for the app's panel/dispatcher boundary so the panel
/// doesn't pull in the sketch crate's constraint-residual surface.
///
/// HI-24: variants MUST stay in lockstep with `signex_sketch::attr::PadSide`.
/// The `From`/`Into` impls below force a compile error if either side
/// adds a variant without updating the other. Adding `Inner` for buried
/// copper would require:
///   1. add `Inner` here AND in `signex_sketch::attr::PadSide`
///   2. extend the conversion match arms below
///   3. extend `pad_to_sketch.rs` mirror logic
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
            PadSide::Top => "Top Layer",
            PadSide::Bottom => "Bottom Layer",
            PadSide::All => "Multi-Layer",
        }
    }
}

impl std::fmt::Display for PadSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

impl From<signex_sketch::attr::PadSide> for PadSide {
    fn from(value: signex_sketch::attr::PadSide) -> Self {
        match value {
            signex_sketch::attr::PadSide::Top => PadSide::Top,
            signex_sketch::attr::PadSide::Bottom => PadSide::Bottom,
            signex_sketch::attr::PadSide::All => PadSide::All,
        }
    }
}

impl From<PadSide> for signex_sketch::attr::PadSide {
    fn from(value: PadSide) -> Self {
        match value {
            PadSide::Top => signex_sketch::attr::PadSide::Top,
            PadSide::Bottom => signex_sketch::attr::PadSide::Bottom,
            PadSide::All => signex_sketch::attr::PadSide::All,
        }
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
    /// v0.18.19 — fine grid display style.
    pub fine_grid_display: GridDisplay,
    /// v0.18.19 — coarse grid display style. The coarse grid is
    /// drawn on top of the fine one at `grid_step_mm * coarse_multiplier`
    /// spacing.
    pub coarse_grid_display: GridDisplay,
    /// v0.18.19 — coarse-grid multiplier (typically 5 or 10).
    /// Renders an overlay grid at this multiple of the snap step.
    pub coarse_multiplier: u32,
    // v0.13 — Altium "Objects for snapping" set. Each flag gates a
    // priority in the snap pipeline. Defaults follow Altium's "common
    // ON" group (Track Vertices, Pad Centers, Via Centers).
    pub snap_track_vertices: bool,
    pub snap_track_lines: bool,
    pub snap_arc_centers: bool,
    pub snap_intersections: bool,
    pub snap_pad_centers: bool,
    pub snap_pad_vertices: bool,
    pub snap_pad_edges: bool,
    pub snap_via_centers: bool,
    pub snap_texts: bool,
    pub snap_regions: bool,
    pub snap_footprint_origins: bool,
    pub snap_3d_body_points: bool,
    /// v0.13 — Altium "Snap Distance" (mm). Hit-test radius for the
    /// per-kind snap targets above.
    pub snap_distance_mm: f64,
    /// v0.13 — Altium "Axis Snap Range" (mm). Lateral tolerance for
    /// horizontal/vertical axis snapping.
    pub axis_snap_range_mm: f64,
    /// v0.13 — Altium-style independent target toggles: each gates
    /// one CATEGORY of snap target (grids / guides / axes). Match
    /// Altium's PCB Library editor where these can be combined freely
    /// (you can have grid-snap AND axis-snap on simultaneously).
    /// Replaces the v0.18.13 `snap_subtab` mutex enum.
    pub snap_to_grids: bool,
    pub snap_to_guides: bool,
    pub snap_to_axes: bool,
}

/// v0.18.19 — Altium grid display style for the Cartesian Grid
/// Editor's Fine + Coarse pickers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GridDisplay {
    #[default]
    Lines,
    Dots,
    Hidden,
}

/// v0.18.20 — Altium-style guide line. One of `x` / `y` is set,
/// representing a vertical (X = const) or horizontal (Y = const)
/// guide. Stored on the per-editor `FootprintEditorState`; the
/// snap-to-guides hook lands in a follow-up (today they're
/// visual-only).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Guide {
    pub axis: GuideAxis,
    pub position_mm: f64,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GuideAxis {
    #[default]
    Vertical,
    Horizontal,
}

/// v0.18.21 — One row in the Cartesian Grid Manager. Each grid is a
/// named (step / fine_display / coarse_display / multiplier) bundle.
/// The currently-active row mirrors its values onto
/// `SnapOptions::{grid_step_mm, fine_grid_display, coarse_grid_display,
/// coarse_multiplier}` so the canvas + snap logic keep operating on a
/// single source of truth. The Properties panel's Grid Manager
/// section is the multi-row CRUD view; the legacy "Grid step (mm)"
/// numeric input edits the active row in place.
#[derive(Debug, Clone, PartialEq)]
pub struct GridDef {
    pub name: String,
    pub step_mm: f64,
    pub fine_display: GridDisplay,
    pub coarse_display: GridDisplay,
    pub coarse_multiplier: u32,
}

impl Default for GridDef {
    fn default() -> Self {
        Self {
            name: "Grid".into(),
            step_mm: 1.0,
            fine_display: GridDisplay::Lines,
            coarse_display: GridDisplay::Lines,
            coarse_multiplier: 5,
        }
    }
}

impl GridDef {
    /// Seed the implicit "Global Snap Grid" row from a `SnapOptions`
    /// snapshot. Used when the FootprintEditorState first materialises
    /// to keep the legacy single-grid behaviour intact.
    pub fn from_snap_options(opts: &SnapOptions) -> Self {
        Self {
            name: "Global Snap Grid".into(),
            step_mm: opts.grid_step_mm,
            fine_display: opts.fine_grid_display,
            coarse_display: opts.coarse_grid_display,
            coarse_multiplier: opts.coarse_multiplier,
        }
    }
}

impl Default for SnapOptions {
    fn default() -> Self {
        Self {
            point_hit: true,
            horizontal_vertical: true,
            angle: true,
            grid: true,
            grid_step_mm: 1.0,
            fine_grid_display: GridDisplay::Lines,
            coarse_grid_display: GridDisplay::Lines,
            coarse_multiplier: 5,
            // Altium "common ON" defaults: Track Vertices, Track Lines,
            // Arc Centers, Pad Centers, Via Centers, Footprint Origins.
            snap_track_vertices: true,
            snap_track_lines: false,
            snap_arc_centers: true,
            snap_intersections: false,
            snap_pad_centers: true,
            snap_pad_vertices: false,
            snap_pad_edges: false,
            snap_via_centers: true,
            snap_texts: false,
            snap_regions: false,
            snap_footprint_origins: true,
            snap_3d_body_points: false,
            snap_distance_mm: 0.203,
            axis_snap_range_mm: 5.08,
            snap_to_grids: true,
            snap_to_guides: false,
            snap_to_axes: false,
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
            stack: PadStackUi::default(),
            feature_top: PadFeature::None,
            feature_bottom: PadFeature::None,
            testpoint: TestpointFlags::default(),
            template: String::new(),
            template_library: String::new(),
            drill_diameter_mm: None,
            drill_slot_length_mm: None,
            shape: signex_library::PadShape::Rect,
            kind: signex_library::PadKind::Smd,
            electrical_type: ElectricalType::Load,
            net: String::new(),
            locked: false,
            hole_tolerance_plus_mm: None,
            hole_tolerance_minus_mm: None,
            hole_rotation_deg: None,
            copper_offset_x_mm: None,
            copper_offset_y_mm: None,
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
    pub vias: bool,
    pub regions: bool,
    pub fills: bool,
    pub other: bool,
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
            vias: true,
            regions: true,
            fills: true,
            other: true,
        }
    }
}

/// Selection-filter pill identifier — drives the panel pill row +
/// the dispatcher's mutation. Order matches Altium's PCB Library
/// editor: 3D Bodies, Keepouts, Tracks, Arcs, Pads, Vias, Regions,
/// Fills, Texts, Other (+ our two internal: Pours, Cutouts).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionFilterKind {
    Bodies3d,
    Keepouts,
    Tracks,
    Arcs,
    Pads,
    Vias,
    Regions,
    Fills,
    Texts,
    Other,
    Pours,
    Cutouts,
}

impl SelectionFilterKind {
    /// Altium's 10 user-visible pill kinds in display order.
    pub const ALTIUM_PILLS: &'static [SelectionFilterKind] = &[
        Self::Bodies3d,
        Self::Keepouts,
        Self::Tracks,
        Self::Arcs,
        Self::Pads,
        Self::Vias,
        Self::Regions,
        Self::Fills,
        Self::Texts,
        Self::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Bodies3d => "3D Bodies",
            Self::Keepouts => "Keepouts",
            Self::Tracks => "Tracks",
            Self::Arcs => "Arcs",
            Self::Pads => "Pads",
            Self::Vias => "Vias",
            Self::Regions => "Regions",
            Self::Fills => "Fills",
            Self::Texts => "Texts",
            Self::Other => "Other",
            Self::Pours => "Pours",
            Self::Cutouts => "Cutouts",
        }
    }
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
            SelectionFilterKind::Vias => self.vias,
            SelectionFilterKind::Regions => self.regions,
            SelectionFilterKind::Fills => self.fills,
            SelectionFilterKind::Other => self.other,
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
            SelectionFilterKind::Vias => self.vias = !self.vias,
            SelectionFilterKind::Regions => self.regions = !self.regions,
            SelectionFilterKind::Fills => self.fills = !self.fills,
            SelectionFilterKind::Other => self.other = !self.other,
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
    /// v0.18.15 — silk-layer text placeholder. 1-click drop appends
    /// an `FpGraphic { kind: Text { position, content: "TEXT", size:
    /// 1.0 } }` to `footprint.silk_f`. The user can later edit the
    /// content via the Properties panel (queued).
    PlaceString,
    /// v0.18.15.1 — silk-layer line. 2-click gesture — first click
    /// stashes the start position in `track_first`, second click
    /// emits `FootprintAddTrack { from, to }` and chains: the
    /// second click also becomes the next gesture's start so the
    /// user can stroke a polyline without re-clicking the tool.
    /// Esc / right-click clears `track_first`.
    PlaceTrack,
    /// v0.18.15.3 — silk-layer arc. 3-click gesture — first click
    /// stashes the centre, second the radius (= distance from
    /// centre), third the sweep end angle. Esc / right-click
    /// clears the in-flight gesture. State carried in
    /// `place_arc_pending`.
    PlaceArc,
    /// v0.18.15.4 — silk-layer closed-loop polygon. Each click
    /// appends a vertex to `place_polygon_vertices`. Switching
    /// away from the tool (or Esc) commits the polygon (one
    /// `FpGraphic::Polygon` entry, `filled: false`) when ≥ 3
    /// vertices have been captured. No preview today — the
    /// canvas-overlay shape preview lands with the silk-layer
    /// hover system.
    PlacePolygon,
    /// v0.18.17 — silk-layer filled region (Altium "Place Region").
    /// Same gesture as `PlacePolygon`; emits one
    /// `FpGraphic::Polygon` with `filled: true` so the renderer
    /// fills the shape instead of stroking the outline.
    PlaceRegion,
    /// v0.13 — through-hole via. 1-click drop, mints a Pad with
    /// `kind = ThroughHole` + paired drill. Hole functionality is
    /// covered by `PlacePad` with the via geometry.
    PlaceVia,
}

/// v0.18.15.3 — Place Arc 3-click gesture state machine.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum PlaceArcPending {
    #[default]
    Idle,
    /// First click — centre stashed.
    Center { center: (f64, f64) },
    /// Second click — start point stashed; the third click closes
    /// the sweep.
    Start {
        center: (f64, f64),
        start: (f64, f64),
    },
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
    /// v0.22 Phase B1 — Mirror tool. Click 1 picks the mirror line
    /// (a `Line` entity), click 2 picks any entity to mirror. The
    /// dispatcher generates a symmetric copy and adds the
    /// `SymmetricAboutLine` constraint linking each Point of the
    /// original to its mirror counterpart. Esc / right-click cancels.
    Mirror,
    /// v0.22 Phase B2 — Offset tool. Pre-condition: a Line / Arc /
    /// Circle is selected via Select tool. Click determines which
    /// side of the source curve the offset is generated on. Offset
    /// distance comes from `state.dimension_input` (default 0.5 mm).
    /// Lines: emits a parallel Line at perpendicular distance + a
    /// `Parallel` and `DistancePtLine` constraint pair so the
    /// distance survives source edits. Circles / Arcs: emits a
    /// concentric copy sharing the source's centre Point so the
    /// centres stay locked; the radius literal is set to source ±
    /// dist. Esc / right-click cancels.
    Offset,
    /// v0.22 Phase B3 — Rectangular Pattern tool. Click 1 picks the
    /// source entity; the dispatcher mints a default `ArrayKind::Grid`
    /// with `nx=2`, `ny=2`, `dx=5mm`, `dy=5mm`. The user edits
    /// per-instance parameters via the sketch JSON until a Properties
    /// sub-form lands.
    RectPattern,
    /// v0.22 Phase B4 — Circular (Polar) Pattern tool. Click 1 picks
    /// the source entity; the dispatcher mints a default
    /// `ArrayKind::Polar` with `count=4`, `sweep_angle=360°`, and a
    /// fresh centre Point offset 5 mm from the source position.
    /// User adjusts the centre + parameters via JSON or the (future)
    /// Pattern Properties sub-form.
    CircularPattern,
    /// v0.24 Track C — Tangent Arc tool. Two-click chained arc
    /// segment: click 1 stashes the first endpoint; click 2 mints an
    /// `Arc` entity tangent to the most recently committed `Line`
    /// whose end Point matches the first click. The dispatcher also
    /// emits a `TangentLineArc` constraint so the tangency survives
    /// further edits. If no incident Line is found at the first
    /// click, a placeholder centre is computed (perpendicular to the
    /// chord midpoint) and a warning is published. Esc / right-click
    /// cancels back to Select. Per the project's no-canvas-gestures
    /// rule, this tool is invoked explicitly via the active-bar
    /// button — it's not an implicit drag-mode of the Line tool.
    TangentArc,
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
    /// v0.23 — "Re-pick centre" affordance from the Pattern
    /// Properties sub-form. The next sketch click on a Point
    /// overwrites `array.center` for the array identified by
    /// `array_id`. Cancels with Esc.
    RepickPolarCenter {
        array_id: signex_sketch::array::ArrayId,
    },
    /// v0.24 Track C — Tangent Arc, first endpoint placed; awaiting
    /// the second endpoint click. The dispatcher mints an `Arc`
    /// tangent to whatever `Line` ends at `first` (or a placeholder
    /// arc with a warning when no incident Line exists).
    TangentArcFirst {
        first: signex_sketch::id::SketchEntityId,
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
            snap_subtab: SnapSubTab::default(),
            snapping_mode: SnappingMode::default(),
            active_bar_menu: None,
            pad_stack_tab: PadStackTab::default(),
            placement_input: None,
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
            snap_subtab: SnapSubTab::default(),
            snapping_mode: SnappingMode::default(),
            active_bar_menu: None,
            pad_stack_tab: PadStackTab::default(),
            placement_input: None,
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
        pad.shape = defaults.shape.clone();
        pad.kind = defaults.kind;
        pad.layers = layers;
        pad.rotation_deg = defaults.rotation_deg;
        // v0.20 — apply Altium-parity Pad Stack / Pad Features /
        // Testpoint / Template defaults so the placed pad inherits
        // the user-edited values from the Properties panel.
        pad.stack = defaults.stack.clone();
        pad.feature_top = defaults.feature_top;
        pad.feature_bottom = defaults.feature_bottom;
        pad.testpoint = defaults.testpoint;
        pad.template = defaults.template.clone();
        pad.template_library = defaults.template_library.clone();
        // For Multi-Layer pads, propagate drill diameter from the
        // defaults. Single-layer pads stay SMD (no drill).
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
        let mut pad = EditorPad::new_npt_hole(number, (x_mm, y_mm), drill_mm);
        // v0.20 — propagate the Pad Properties defaults onto the hole
        // so testpoint / template / feature flags survive placement.
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
    /// sketch source-of-truth, refresh the editor-side `pads` cache so
    /// the canvas / Pads-mode Properties panel reads the same values
    /// that just got baked.
    ///
    /// `sketch_entity_id` and `corner_entity_ids` don't round-trip
    /// through `Pad`, so we re-attach them by matching `pad.number`.
    /// Brand-new pads (e.g., array expansions) keep `None` for these
    /// fields; the next Sketch-mode entry's auto-mint can populate.
    pub fn refresh_pads_from_primitive(&mut self, fp: &Footprint) {
        use std::collections::HashMap;
        type Link = (
            Option<signex_sketch::id::SketchEntityId>,
            Option<[signex_sketch::id::SketchEntityId; 4]>,
        );
        let old_links: HashMap<String, Link> = self
            .pads
            .iter()
            .map(|p| {
                (
                    p.number.clone(),
                    (p.sketch_entity_id, p.corner_entity_ids),
                )
            })
            .collect();
        let mut new_pads: Vec<EditorPad> = fp.pads.iter().map(EditorPad::from_pad).collect();
        for p in &mut new_pads {
            if let Some((sid, cids)) = old_links.get(&p.number) {
                p.sketch_entity_id = *sid;
                p.corner_entity_ids = *cids;
            }
        }
        self.pads = new_pads;
        // selected_pad index might now point past the new vec — clamp.
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
        // v0.22 Phase D1 — Pads-mode → Sketch attribute mirror. For
        // every editor-side pad backed by a sketch entity, copy the
        // enum/bool/Option<f64> attribute fields onto the entity's
        // PadAttr so a subsequent Sketch-mode session reads the same
        // Net / Locked / ElectricalType / Template / Library /
        // Feature / Testpoint / Hole-Details state. Geometry-shaping
        // expressions (size_x_expr / mask_margin_expr / paste_*_expr
        // / drill spec) intentionally stay sketch-parameterised — only
        // attribute fields cross. Designator (number) also mirrors
        // so a Pads-mode rename propagates to the entity.
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
