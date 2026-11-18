//! Read-only footprint model for previews outside the editor (Components
//! panel details, Component Preview dialog). Built once when a footprint
//! is loaded, borrowed on every draw.

use signex_gfx::scene::{DirtyFlags, Scene};
use signex_library::primitive::Footprint;
use signex_library::primitive::footprint::{FpGraphic, FpGraphicKind};
use signex_renderer::pcb::{PcbRenderer, PcbSnapshot};
use signex_renderer::schematic::ViewRenderer;
use signex_renderer::theme::ResolvedTheme;

use super::draw::{LayerInput, PadLabels};
use crate::library::editor::footprint::layers::{LayerPalette, LayerVisibility};
use crate::library::editor::footprint::state::{CourtyardRect, EditorPad, FootprintEditorState};

/// A preview labels a pad once it is this wide on screen.
const MIN_LABELLED_PAD_PX: f32 = 9.0;

/// The editor's view of a saved footprint: pad models, silk, the courtyard
/// the editor would draw, the pads as the board draws them, and the bbox the
/// preview fits to.
#[derive(Debug, Clone)]
pub struct PreviewFootprint {
    pub name: String,
    key: (uuid::Uuid, i64),
    pads: Vec<EditorPad>,
    /// The pads lowered as the PCB canvas lowers them, in the board palette.
    board_pads: Scene,
    /// The pads the board cannot hold (custom or chamfered shape, slot,
    /// copper offset), drawn by the editor's pad renderer instead.
    unheld_pads: Vec<EditorPad>,
    silk_f: Vec<FpGraphic>,
    silk_b: Vec<FpGraphic>,
    fab_f: Vec<FpGraphic>,
    fab_b: Vec<FpGraphic>,
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
        for g in fp
            .silk_f
            .iter()
            .chain(&fp.fab_f)
            .chain(&fp.silk_b)
            .chain(&fp.fab_b)
        {
            let mut pts = Vec::new();
            graphic_points(g, &mut pts);
            for p in pts {
                grow(p[0], p[1]);
            }
        }
        let drawn = signex_library::drawn_board_footprint(fp);
        let unheld_pads = drawn
            .unheld_pads
            .iter()
            .filter_map(|&index| state.pads.get(index).cloned())
            .collect();
        Self {
            name: fp.name.clone(),
            key: (fp.uuid, fp.updated.timestamp_millis()),
            courtyard_outline,
            courtyard_rect: state.courtyard_mm,
            bbox_mm: bbox,
            board_pads: board_pad_scene(&drawn.footprint),
            unheld_pads,
            pads: state.pads,
            silk_f: fp.silk_f.clone(),
            silk_b: fp.silk_b.clone(),
            fab_f: fp.fab_f.clone(),
            fab_b: fp.fab_b.clone(),
        }
    }

    /// Bounding box `(min_x, min_y, max_x, max_y)` in mm of pads,
    /// courtyard, silk and fab of both sides; `None` for an empty footprint.
    pub fn bbox_mm(&self) -> Option<(f64, f64, f64, f64)> {
        self.bbox_mm
    }

    /// Pads, in the editor's pad model.
    pub fn pads(&self) -> &[EditorPad] {
        &self.pads
    }

    /// The pads as the PCB canvas draws them: copper in the board palette's
    /// layer colours, drills as openings.
    pub(crate) fn board_pads(&self) -> &Scene {
        &self.board_pads
    }

    /// The pads the board cannot hold, for the editor's pad renderer.
    pub(crate) fn unheld_pads(&self) -> &[EditorPad] {
        &self.unheld_pads
    }

    /// Front fab graphics.
    pub(crate) fn fab_f(&self) -> &[FpGraphic] {
        &self.fab_f
    }

    /// Back silk graphics.
    pub(crate) fn silk_b(&self) -> &[FpGraphic] {
        &self.silk_b
    }

    /// Back fab graphics.
    pub(crate) fn fab_b(&self) -> &[FpGraphic] {
        &self.fab_b
    }

    /// Default layers (front side on, plus bottom copper, whose pads the
    /// board draws too), nothing selected.
    pub(crate) fn layer_input(&self) -> LayerInput<'_> {
        LayerInput {
            pads: &self.pads,
            silk_f: &self.silk_f,
            silk_b: &self.silk_b,
            visibility: LayerVisibility {
                b_cu: true,
                ..LayerVisibility::default()
            },
            courtyard_outline: self.courtyard_outline.as_deref(),
            courtyard_rect: self.courtyard_rect,
            selected_pad: None,
            selected_pads_extra: &[],
            selected_silk_f: None,
            pad_labels: PadLabels::MinPadPx(MIN_LABELLED_PAD_PX),
            palette: LayerPalette::Board,
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

/// `footprint`'s pads lowered through the PCB canvas's own path, in the board
/// palette (the same in every theme).
fn board_pad_scene(footprint: &signex_types::pcb::Footprint) -> Scene {
    let theme = ResolvedTheme::from_canvas_colors(crate::styles::board_canvas_colors());
    let mut scene = Scene::default();
    PcbRenderer::build_scene(
        &PcbSnapshot::from_footprint(footprint),
        &theme,
        DirtyFlags::POLYGONS,
        &mut scene,
    );
    scene
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
