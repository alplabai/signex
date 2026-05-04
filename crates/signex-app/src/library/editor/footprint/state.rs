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
        }
    }

    fn to_pad(&self) -> Pad {
        Pad {
            number: self.number.clone(),
            kind: self.kind,
            shape: self.shape.clone(),
            size: [self.size_mm.0, self.size_mm.1],
            position: [self.position_mm.0, self.position_mm.1],
            rotation: 0.0,
            layers: self.layers.clone(),
            drill: None,
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
}

/// Sketch-mode drawing tool. Phase 6.3 (v0.13.1) shipped Place Point
/// only; v0.13.2 adds Line, Circle, Arc; v0.15 adds Rectangle (two-
/// click corner-to-corner; emits 4 Lines + corner Points).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SketchTool {
    #[default]
    Select,
    Point,
    Line,
    Rectangle,
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

    /// Click-add a new default pad at the given world position.
    pub fn add_pad_at(&mut self, x_mm: f64, y_mm: f64) -> usize {
        let number = self.next_pad_number();
        self.pads.push(EditorPad::new_default(number, (x_mm, y_mm)));
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
