//! Show only unrouted (mockup row 164, #2821): the built scene goes faint
//! except its ratsnest lines. The board area and its openings stay as the
//! backdrop the lines are read against, so a hole never fills in.

use signex_gfx::primitive::polygon::GpuPolygon;
use signex_gfx::scene::{DirtyFlags, Scene};
use signex_gfx::style::ColorSlot;

use super::emit::with_alpha_mul;
use super::{DIMMED_ALPHA, RATSNEST_STYLE_DASHED};
use crate::theme::ResolvedTheme;

/// Fade the buckets of `scene` that `dirty` says were just rebuilt.
pub(super) fn dim_board(theme: &ResolvedTheme, dirty: DirtyFlags, scene: &mut Scene) {
    let backdrop = [
        theme.color(ColorSlot::BoardArea),
        theme.color(ColorSlot::BoardOpening),
    ];
    let fade = |color: &mut [f32; 4]| {
        if !backdrop.contains(&*color) {
            *color = with_alpha_mul(*color, DIMMED_ALPHA);
        }
    };
    if dirty.contains(DirtyFlags::LINES) {
        scene.lines.iter_mut().for_each(|l| fade(&mut l.color));
    }
    if dirty.contains(DirtyFlags::CIRCLES) {
        scene.circles.iter_mut().for_each(|c| fade(&mut c.color));
    }
    if dirty.contains(DirtyFlags::POLYGONS) {
        fade_polygons(&mut scene.polygons, &fade);
        fade_polygons(&mut scene.top_polygons, &fade);
    }
    if dirty.contains(DirtyFlags::OVERLAY) {
        scene
            .overlay_lines
            .iter_mut()
            .filter(|l| l.style & RATSNEST_STYLE_DASHED == 0)
            .for_each(|l| fade(&mut l.color));
        scene
            .overlay_circles
            .iter_mut()
            .for_each(|c| fade(&mut c.color));
        fade_polygons(&mut scene.overlay_polygons, &fade);
    }
}

fn fade_polygons(polygons: &mut [GpuPolygon], fade: &impl Fn(&mut [f32; 4])) {
    for polygon in polygons {
        fade(&mut polygon.fill_color);
        if let Some(stroke) = polygon.stroke_color.as_mut() {
            fade(stroke);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pcb::{PcbLayerView, PcbRenderer, PcbSnapshot, RatsnestInput};
    use crate::schematic::ViewRenderer;
    use signex_types::pcb::PcbBoard;
    use signex_types::theme::{ThemeId, canvas_colors};

    fn board() -> PcbBoard {
        serde_json::from_str(
            r#"{
              "uuid": "8d15b2f9-8f86-41d7-9ec4-0f54a4f3a651", "version": 1,
              "generator": "test", "thickness": 1.6,
              "outline": [{"x":0.0,"y":0.0},{"x":9.0,"y":0.0},{"x":9.0,"y":9.0}],
              "layers": [
                { "id": 0, "name": "TopCopper", "layer_type": "signal" },
                { "id": 31, "name": "BottomCopper", "layer_type": "signal" }
              ],
              "setup": null, "nets": [], "footprints": [],
              "segments": [
                { "uuid": "f5ea9948-4c2f-4501-b647-4047f6eefca9", "start": {"x":0.0,"y":0.0}, "end": {"x":5.0,"y":0.0}, "width": 0.25, "layer": "TopCopper", "net": 1 }
              ],
              "vias": [
                { "uuid": "7f2eb13d-a3d6-47e6-84f7-3b27240a95f3", "position": {"x":2.5,"y":1.0}, "diameter": 0.8, "drill": 0.4, "layers": ["TopCopper","BottomCopper"], "net": 1 }
              ],
              "zones": [], "graphics": [], "texts": []
            }"#,
        )
        .expect("valid board")
    }

    fn drawn(only_unrouted: bool, theme: &ResolvedTheme) -> Scene {
        let mut view = PcbLayerView::default();
        view.set_only_unrouted(only_unrouted);
        let snapshot =
            PcbSnapshot::from_board_with_view(&board(), &view).with_ratsnest_lines(vec![
                RatsnestInput {
                    p0: [0.0, 0.0],
                    p1: [5.0, 5.0],
                    net: 7,
                },
            ]);
        let mut scene = Scene::default();
        let dirty =
            DirtyFlags::LINES | DirtyFlags::CIRCLES | DirtyFlags::POLYGONS | DirtyFlags::OVERLAY;
        PcbRenderer::build_scene(&snapshot, theme, dirty, &mut scene);
        scene
    }

    fn ratsnest_colour(scene: &Scene) -> [f32; 4] {
        scene
            .overlay_lines
            .iter()
            .find(|l| l.style & RATSNEST_STYLE_DASHED != 0)
            .expect("a ratsnest line")
            .color
    }

    /// DECISIONS 2026-11-22 (#2821), in both Graphite themes.
    #[test]
    fn show_only_unrouted_dims_the_board_and_leaves_the_ratsnest_lines() {
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let theme = ResolvedTheme::from_canvas_colors(canvas_colors(id));
            let (plain, dimmed) = (drawn(false, &theme), drawn(true, &theme));
            let faint = plain.lines[0].color[3] * DIMMED_ALPHA;
            assert!((dimmed.lines[0].color[3] - faint).abs() < 1e-6, "{id:?}");
            let ring = plain.circles[0].color[3] * DIMMED_ALPHA;
            assert!((dimmed.circles[0].color[3] - ring).abs() < 1e-6, "{id:?}");
            let edge = dimmed
                .overlay_lines
                .iter()
                .zip(&plain.overlay_lines)
                .find(|(l, _)| l.style & RATSNEST_STYLE_DASHED == 0)
                .expect("a board edge");
            assert!((edge.0.color[3] - edge.1.color[3] * DIMMED_ALPHA).abs() < 1e-6);
            assert_eq!(ratsnest_colour(&dimmed), ratsnest_colour(&plain), "{id:?}");
            // The backdrop: the board area, and the via's drill.
            assert_eq!(dimmed.polygons[0].fill_color, plain.polygons[0].fill_color);
            assert_eq!(dimmed.circles[1].color, plain.circles[1].color, "{id:?}");
        }
    }

    /// The flag is the snapshot's: the Layers panel's paint never carries it.
    #[test]
    fn only_a_snapshot_of_the_view_is_dimmed() {
        let board = board();
        let mut view = PcbLayerView::default();
        assert!(
            !PcbSnapshot::from_board_with_view(&board, &view)
                .paint
                .is_board_dimmed()
        );
        view.set_only_unrouted(true);
        assert!(
            PcbSnapshot::from_board_with_view(&board, &view)
                .paint
                .is_board_dimmed()
        );
        assert!(!view.paint_of(&board).is_board_dimmed());
        assert!(!view.paint().is_board_dimmed());
    }
}
