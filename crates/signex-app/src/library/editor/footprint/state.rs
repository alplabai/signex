//! Footprint editor in-memory state.
//!
//! `PcbSide.footprint.sexpr` is the on-disk ground truth. We parse it
//! into typed structs on first display and re-serialize on every
//! mutation so the draft round-trips cleanly through Save Draft /
//! Commit.

use standard_parser::pcb as kp;
use signex_types::pcb::{Pad, PadShape, PadType};

use super::layers::{FpLayer, LayerVisibility};

/// Default new-pad size in mm — 1.0 mm × 1.0 mm rect, matching
/// `PCB_DEFAULT_PAD_SIZE_MM`. The user resizes via the Properties
/// panel in v0.9.x; the MVP only exposes pad placement.
const NEW_PAD_SIZE_MM: f64 = 1.0;

/// Slack on each side of the pad bounding box when auto-fitting the
/// courtyard polygon. Altium's default footprint courtyard expansion
/// is 0.25 mm — we match it.
const COURTYARD_SLACK_MM: f64 = 0.25;

/// Footprint name written into the `(footprint <name>)` envelope when
/// the part is brand new. The user re-names this from the Component
/// Editor's Overview tab in Phase 2; the MVP just keeps the existing
/// id intact during round-trips.
const DEFAULT_FOOTPRINT_ID: &str = "snx-footprint";

/// One pad in the in-memory footprint. A subset of
/// [`signex_types::pcb::Pad`] — we only carry what the MVP renders or
/// edits. Re-serialization fills in defaults for the rest.
#[derive(Debug, Clone, PartialEq)]
pub struct EditorPad {
    /// Pad number / name (Standard uses a free-form string here so
    /// "1", "GND", "A1" are all valid).
    pub number: String,
    /// Centre position in mm relative to the footprint origin.
    pub position_mm: (f64, f64),
    /// Pad width × height in mm.
    pub size_mm: (f64, f64),
    pub pad_type: PadType,
    pub shape: PadShape,
    /// Layers the pad lives on. SMD pads on F.Cu use
    /// `["F.Cu", "F.Mask", "F.Paste"]`; we only render the first
    /// entry but preserve the rest for round-trips.
    pub layers: Vec<String>,
}

impl EditorPad {
    /// Default new-pad: rect SMD on F.Cu, 1mm square.
    pub fn new_default(number: String, position_mm: (f64, f64)) -> Self {
        Self {
            number,
            position_mm,
            size_mm: (NEW_PAD_SIZE_MM, NEW_PAD_SIZE_MM),
            pad_type: PadType::Smd,
            shape: PadShape::Rect,
            layers: vec![
                "F.Cu".to_string(),
                "F.Mask".to_string(),
                "F.Paste".to_string(),
            ],
        }
    }

    /// Layer the pad lives on for hit-testing / toggle gating —
    /// derived from the first entry in `layers`.
    pub fn primary_layer(&self) -> FpLayer {
        self.layers
            .first()
            .and_then(|name| FpLayer::from_standard_name(name))
            .unwrap_or(FpLayer::FCu)
    }

    /// Bounding box (min_x, min_y, max_x, max_y) in mm — used for
    /// hit-testing and auto-fit.
    pub fn bbox_mm(&self) -> (f64, f64, f64, f64) {
        let (cx, cy) = self.position_mm;
        let (w, h) = self.size_mm;
        (cx - w / 2.0, cy - h / 2.0, cx + w / 2.0, cy + h / 2.0)
    }

    /// Hit test against a world-space point in mm. Pads are
    /// rendered as axis-aligned rectangles in the MVP, so the test
    /// is the obvious AABB containment.
    pub fn contains_mm(&self, x: f64, y: f64) -> bool {
        let (xmin, ymin, xmax, ymax) = self.bbox_mm();
        x >= xmin && x <= xmax && y >= ymin && y <= ymax
    }
}

/// One graphic stroke / poly preserved verbatim from the source
/// footprint. Phase 2 (router stages) graduates this into a typed
/// drawing model; the MVP only renders.
#[derive(Debug, Clone, PartialEq)]
pub struct EditorGraphic {
    pub layer: FpLayer,
    pub kind: GraphicKind,
    /// Standard-format layer name, kept so round-tripping unknown layers
    /// (which `FpLayer::from_standard_name` collapses) preserves the
    /// original string.
    pub raw_layer_name: String,
    /// Stroke width in mm.
    pub width: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GraphicKind {
    Line {
        start: (f64, f64),
        end: (f64, f64),
    },
    Circle {
        center: (f64, f64),
        radius: f64,
    },
    Polygon {
        points: Vec<(f64, f64)>,
    },
}

/// Live, in-memory state of the Footprint tab.
#[derive(Debug, Clone, PartialEq)]
pub struct FootprintEditorState {
    /// Display id retained from the parsed footprint envelope.
    pub footprint_id: String,
    pub pads: Vec<EditorPad>,
    /// Decorative graphics (silk / fab / edge-cuts polylines).
    /// Pure-display in the MVP — Phase 2 surfaces an editing UI.
    pub graphics: Vec<EditorGraphic>,
    pub layer_visibility: LayerVisibility,
    /// `Some(idx)` while a pad is selected. Cleared on background
    /// click or Delete.
    pub selected_pad: Option<usize>,
    /// `true` when the courtyard polygon should track the pad
    /// bounding box. Toggled by the canvas footer button.
    pub auto_fit_courtyard: bool,
    /// Courtyard polygon. When `auto_fit_courtyard` is true this is
    /// derived from `pads`; otherwise it's user-controlled (Phase 2
    /// drawing tools).
    pub courtyard_mm: Option<CourtyardRect>,
    /// Last known cursor world position in mm — drives the footer
    /// readout.
    pub cursor_mm: Option<(f64, f64)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CourtyardRect {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl FootprintEditorState {
    /// Parse a Standard footprint S-expression into editable state. An
    /// empty / blank `sexpr` yields a fresh empty footprint so the
    /// "new component" flow can drop the user straight into the
    /// editor.
    pub fn from_sexpr(sexpr: &str) -> Self {
        let trimmed = sexpr.trim();
        if trimmed.is_empty() {
            return Self::empty();
        }

        match kp::parse_footprint_file(trimmed) {
            Ok(fp) => Self::from_parsed(fp),
            Err(e) => {
                tracing::warn!(
                    target: "signex::library::footprint",
                    error = %e,
                    "footprint sexpr parse failed; opening blank canvas",
                );
                Self::empty()
            }
        }
    }

    /// Empty footprint — used for brand-new components and as the
    /// fallback when parsing fails.
    pub fn empty() -> Self {
        let mut s = Self {
            footprint_id: DEFAULT_FOOTPRINT_ID.to_string(),
            pads: Vec::new(),
            graphics: Vec::new(),
            layer_visibility: LayerVisibility::default(),
            selected_pad: None,
            auto_fit_courtyard: true,
            courtyard_mm: None,
            cursor_mm: None,
        };
        s.recompute_courtyard();
        s
    }

    fn from_parsed(fp: signex_types::pcb::Footprint) -> Self {
        let pads = fp
            .pads
            .into_iter()
            .map(|p: Pad| EditorPad {
                number: p.number,
                position_mm: (p.position.x, p.position.y),
                size_mm: (p.size.x, p.size.y),
                pad_type: p.pad_type,
                shape: p.shape,
                layers: p.layers,
            })
            .collect();

        let graphics = fp
            .graphics
            .into_iter()
            .filter_map(graphic_from_fp)
            .collect();

        let mut s = Self {
            footprint_id: if fp.footprint_id.is_empty() {
                DEFAULT_FOOTPRINT_ID.to_string()
            } else {
                fp.footprint_id
            },
            pads,
            graphics,
            layer_visibility: LayerVisibility::default(),
            selected_pad: None,
            auto_fit_courtyard: true,
            courtyard_mm: None,
            cursor_mm: None,
        };
        s.recompute_courtyard();
        s
    }

    /// Bounding box of the entire footprint (pads + courtyard) in mm.
    /// Used by the canvas to fit-to-content. Returns `None` for an
    /// empty footprint so the caller can apply its own default rect.
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
    /// Returns the new pad's index for downstream selection.
    pub fn add_pad_at(&mut self, x_mm: f64, y_mm: f64) -> usize {
        let number = self.next_pad_number();
        self.pads.push(EditorPad::new_default(number, (x_mm, y_mm)));
        let idx = self.pads.len() - 1;
        self.selected_pad = Some(idx);
        self.recompute_courtyard();
        idx
    }

    /// Move the pad at `idx` to a new world position. No-op if the
    /// index is out of range.
    pub fn move_pad(&mut self, idx: usize, x_mm: f64, y_mm: f64) {
        if let Some(pad) = self.pads.get_mut(idx) {
            pad.position_mm = (x_mm, y_mm);
            self.recompute_courtyard();
        }
    }

    /// Delete the pad at `idx`. No-op if out of range.
    /// Adjusts `selected_pad` so it doesn't dangle.
    pub fn delete_pad(&mut self, idx: usize) {
        if idx >= self.pads.len() {
            return;
        }
        self.pads.remove(idx);
        // Selection cleared whenever the deleted pad was selected;
        // otherwise shift down so it still points at the same pad.
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
    /// Called after every pad mutation.
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

    /// Toggle the auto-fit courtyard flag. Recomputes when re-enabled.
    pub fn toggle_auto_fit(&mut self) {
        self.auto_fit_courtyard = !self.auto_fit_courtyard;
        self.recompute_courtyard();
    }

    /// Re-emit the editor state as a Standard-format S-expression that
    /// round-trips through `parse_footprint_file`. Pad fields not
    /// edited by the MVP (drill, net, properties) are written with
    /// safe defaults; preserved-but-unedited graphics are written
    /// back unchanged.
    ///
    /// Output is whitespace-friendly so a future diff against
    /// `parse_footprint_file` can rely on stable line breaks.
    pub fn to_sexpr(&self) -> String {
        let mut out = String::new();
        out.push_str("(footprint \"");
        out.push_str(&escape(&self.footprint_id));
        out.push_str("\"\n  (layer \"F.Cu\")\n");

        // Pads.
        for pad in &self.pads {
            let kind = pad_type_str(pad.pad_type);
            let shape = pad_shape_str(pad.shape);
            out.push_str(&format!(
                "  (pad \"{}\" {} {}\n",
                escape(&pad.number),
                kind,
                shape,
            ));
            out.push_str(&format!(
                "    (at {:.4} {:.4})\n",
                pad.position_mm.0, pad.position_mm.1
            ));
            out.push_str(&format!(
                "    (size {:.4} {:.4})\n",
                pad.size_mm.0, pad.size_mm.1
            ));
            out.push_str("    (layers");
            for layer in &pad.layers {
                out.push(' ');
                out.push('"');
                out.push_str(&escape(layer));
                out.push('"');
            }
            out.push_str(")\n  )\n");
        }

        // Courtyard polygon — drawn as a rectangle of fp_lines on
        // Edge.Cuts. Rendered conditionally on auto-fit + presence.
        if let Some(c) = self.courtyard_mm {
            let layer_name = "Edge.Cuts";
            let corners = [
                (c.min_x, c.min_y),
                (c.max_x, c.min_y),
                (c.max_x, c.max_y),
                (c.min_x, c.max_y),
            ];
            for i in 0..4 {
                let (x0, y0) = corners[i];
                let (x1, y1) = corners[(i + 1) % 4];
                out.push_str(&format!(
                    "  (fp_line (start {:.4} {:.4}) (end {:.4} {:.4}) (layer \"{}\") (stroke (width 0.05)))\n",
                    x0, y0, x1, y1, layer_name,
                ));
            }
        }

        // Round-trip the silk/fab/etc. graphics we parsed but don't edit.
        for g in &self.graphics {
            // Skip Edge.Cuts strokes — they're regenerated from the
            // courtyard each save so we don't double-write.
            if g.raw_layer_name == "Edge.Cuts" {
                continue;
            }
            match &g.kind {
                GraphicKind::Line { start, end } => {
                    out.push_str(&format!(
                        "  (fp_line (start {:.4} {:.4}) (end {:.4} {:.4}) (layer \"{}\") (stroke (width {:.4})))\n",
                        start.0, start.1, end.0, end.1, escape(&g.raw_layer_name), g.width,
                    ));
                }
                GraphicKind::Circle { center, radius } => {
                    let edge = (center.0 + radius, center.1);
                    out.push_str(&format!(
                        "  (fp_circle (center {:.4} {:.4}) (end {:.4} {:.4}) (layer \"{}\") (stroke (width {:.4})))\n",
                        center.0, center.1, edge.0, edge.1, escape(&g.raw_layer_name), g.width,
                    ));
                }
                GraphicKind::Polygon { points } => {
                    out.push_str("  (fp_poly (pts");
                    for (x, y) in points {
                        out.push_str(&format!(" (xy {:.4} {:.4})", x, y));
                    }
                    out.push_str(&format!(
                        ") (layer \"{}\") (stroke (width {:.4})))\n",
                        escape(&g.raw_layer_name),
                        g.width
                    ));
                }
            }
        }

        out.push(')');
        out
    }
}

fn graphic_from_fp(g: signex_types::pcb::FpGraphic) -> Option<EditorGraphic> {
    let layer = FpLayer::from_standard_name(&g.layer).unwrap_or(FpLayer::FFab);
    let raw_layer_name = if g.layer.is_empty() {
        layer.standard_name().to_string()
    } else {
        g.layer
    };
    let kind = match g.graphic_type.as_str() {
        "line" => {
            let start = g.start?;
            let end = g.end?;
            GraphicKind::Line {
                start: (start.x, start.y),
                end: (end.x, end.y),
            }
        }
        "circle" => {
            let center = g.center?;
            GraphicKind::Circle {
                center: (center.x, center.y),
                radius: g.radius,
            }
        }
        "poly" => GraphicKind::Polygon {
            points: g.points.into_iter().map(|p| (p.x, p.y)).collect(),
        },
        // Drop fp_text and fp_arc in the MVP — they're preserved as raw
        // sexpr in `draft.pcb.footprint.sexpr`'s pre-edit value if the
        // editor is never opened, and re-introduced once Phase 2 adds
        // text/arc tooling.
        _ => return None,
    };
    Some(EditorGraphic {
        layer,
        kind,
        raw_layer_name,
        width: if g.width <= 0.0 { 0.1 } else { g.width },
    })
}

fn pad_type_str(t: PadType) -> &'static str {
    match t {
        PadType::Thru => "thru_hole",
        PadType::Smd => "smd",
        PadType::Connect => "connect",
        PadType::NpThru => "np_thru_hole",
    }
}

fn pad_shape_str(s: PadShape) -> &'static str {
    match s {
        PadShape::Circle => "circle",
        PadShape::Rect => "rect",
        PadShape::Oval => "oval",
        PadShape::Trapezoid => "trapezoid",
        PadShape::RoundRect => "roundrect",
        PadShape::Custom => "custom",
    }
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
