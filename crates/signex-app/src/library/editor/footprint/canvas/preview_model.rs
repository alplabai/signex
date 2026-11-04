//! Read-only footprint model for previews outside the editor (Components
//! panel details, Component Preview dialog). Built once when a footprint
//! is loaded, borrowed on every draw.

use signex_library::primitive::Footprint;
use signex_library::primitive::footprint::{FpGraphic, FpGraphicKind};

use super::draw::LayerInput;
use crate::library::editor::footprint::layers::LayerVisibility;
use crate::library::editor::footprint::state::{CourtyardRect, EditorPad, FootprintEditorState};

/// The editor's view of a saved footprint: pad models, silk, the courtyard
/// the editor would draw, and the bbox the preview fits to.
#[derive(Debug, Clone)]
pub struct PreviewFootprint {
    pub name: String,
    key: (uuid::Uuid, i64),
    pads: Vec<EditorPad>,
    silk_f: Vec<FpGraphic>,
    silk_b: Vec<FpGraphic>,
    courtyard_outline: Option<Vec<(f64, f64)>>,
    courtyard_rect: Option<CourtyardRect>,
    bbox_mm: Option<(f64, f64, f64, f64)>,
}

impl PreviewFootprint {
    /// Build the pad models and courtyard through the editor's own loader,
    /// so the preview cannot disagree with what the editor opens.
    pub fn build(fp: &Footprint) -> Self {
        let state = FootprintEditorState::from_footprint(fp);
        let mut courtyard_outline = state.courtyard_outline().map(<[_]>::to_vec);
        // The loader computes a courtyard only when auto-fit is on; a saved
        // courtyard polygon is what the part actually carries.
        if courtyard_outline.is_none() && state.courtyard_mm.is_none() {
            let saved = &fp.courtyard.points;
            if saved.len() >= 3 {
                courtyard_outline = Some(saved.iter().map(|p| (p[0], p[1])).collect());
            }
        }
        let mut bbox = state.content_bbox_mm();
        let mut grow = |x: f64, y: f64| {
            bbox = Some(match bbox {
                Some((a, b, c, d)) => (a.min(x), b.min(y), c.max(x), d.max(y)),
                None => (x, y, x, y),
            });
        };
        for (x, y) in courtyard_outline.iter().flatten() {
            grow(*x, *y);
        }
        for g in &fp.silk_f {
            let mut pts = Vec::new();
            graphic_points(g, &mut pts);
            for p in pts {
                grow(p[0], p[1]);
            }
        }
        Self {
            name: fp.name.clone(),
            key: (fp.uuid, fp.updated.timestamp_millis()),
            courtyard_outline,
            courtyard_rect: state.courtyard_mm,
            bbox_mm: bbox,
            pads: state.pads,
            silk_f: fp.silk_f.clone(),
            silk_b: fp.silk_b.clone(),
        }
    }

    /// Bounding box `(min_x, min_y, max_x, max_y)` in mm of pads,
    /// courtyard and front silk; `None` for an empty footprint.
    pub fn bbox_mm(&self) -> Option<(f64, f64, f64, f64)> {
        self.bbox_mm
    }

    /// Pads, in the editor's pad model.
    pub fn pads(&self) -> &[EditorPad] {
        &self.pads
    }

    /// Default layers (front side on), nothing selected.
    pub(crate) fn layer_input(&self) -> LayerInput<'_> {
        LayerInput {
            pads: &self.pads,
            silk_f: &self.silk_f,
            silk_b: &self.silk_b,
            visibility: LayerVisibility::default(),
            courtyard_outline: self.courtyard_outline.as_deref(),
            courtyard_rect: self.courtyard_rect,
            selected_pad: None,
            selected_pads_extra: &[],
            selected_silk_f: None,
        }
    }
}

impl PreviewFootprint {
    /// Identifies the saved state this was built from: a preview keeps its
    /// camera while the key is unchanged and refits when it changes.
    pub fn key(&self) -> (uuid::Uuid, i64) {
        self.key
    }
}

fn graphic_points(g: &FpGraphic, out: &mut Vec<[f64; 2]>) {
    match &g.kind {
        FpGraphicKind::Line { from, to } | FpGraphicKind::Rectangle { from, to } => {
            out.extend([*from, *to]);
        }
        FpGraphicKind::Circle { center, radius } | FpGraphicKind::Arc { center, radius, .. } => {
            out.push([center[0] - radius, center[1] - radius]);
            out.push([center[0] + radius, center[1] + radius]);
        }
        FpGraphicKind::Polygon { vertices } => out.extend(vertices.iter().copied()),
        FpGraphicKind::Text { position, .. } => out.push(*position),
    }
}
