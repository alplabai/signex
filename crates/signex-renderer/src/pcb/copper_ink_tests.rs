//! Copper draws in its layer's colour, and through-holes show their drill.

use super::*;
use signex_types::theme::{ThemeId, canvas_colors};

/// A top track, a top SMD pad, a bottom SMD pad, a through-hole pad and a
/// through via.
fn board() -> PcbBoard {
    serde_json::from_str(
        r#"{
          "uuid": "0b0a7c1e-5f0e-4d8e-9a1f-2a6c1d3e4f50", "version": 1,
          "generator": "test", "thickness": 1.6, "outline": [],
          "layers": [
            { "id": 0, "name": "TopCopper", "layer_type": "signal" },
            { "id": 31, "name": "BottomCopper", "layer_type": "signal" }
          ],
          "setup": null, "nets": [],
          "footprints": [
            {
              "uuid": "1c2d3e4f-5a6b-4c7d-8e9f-0a1b2c3d4e5f", "reference": "J1",
              "value": "CONN", "footprint_id": "MIX", "position": { "x": 10.0, "y": 10.0 },
              "rotation": 0.0, "layer": "TopCopper", "locked": false,
              "pads": [
                { "uuid": "2d3e4f5a-6b7c-4d8e-9f0a-1b2c3d4e5f6a", "number": "1",
                  "pad_type": "smd", "shape": "rect", "position": { "x": 0.0, "y": 0.0 },
                  "size": { "x": 1.0, "y": 0.6 }, "drill": null,
                  "layers": ["TopCopper"], "net": null, "roundrect_ratio": 0.0 },
                { "uuid": "3e4f5a6b-7c8d-4e9f-0a1b-2c3d4e5f6a7b", "number": "2",
                  "pad_type": "smd", "shape": "rect", "position": { "x": 3.0, "y": 0.0 },
                  "size": { "x": 1.0, "y": 0.6 }, "drill": null,
                  "layers": ["BottomCopper"], "net": null, "roundrect_ratio": 0.0 },
                { "uuid": "4f5a6b7c-8d9e-4f0a-1b2c-3d4e5f6a7b8c", "number": "3",
                  "pad_type": "thru", "shape": "circle", "position": { "x": 6.0, "y": 0.0 },
                  "size": { "x": 1.8, "y": 1.8 }, "drill": { "diameter": 1.0, "shape": "circle" },
                  "layers": ["TopCopper", "BottomCopper"], "net": null, "roundrect_ratio": 0.0 }
              ],
              "graphics": [], "properties": []
            }
          ],
          "segments": [
            { "uuid": "5a6b7c8d-9e0f-4a1b-2c3d-4e5f6a7b8c9d", "start": {"x":0.0,"y":0.0},
              "end": {"x":5.0,"y":0.0}, "width": 0.25, "layer": "TopCopper", "net": 1 }
          ],
          "vias": [
            { "uuid": "6b7c8d9e-0f1a-4b2c-3d4e-5f6a7b8c9d0e", "position": {"x":2.5,"y":5.0},
              "diameter": 0.8, "drill": 0.4, "layers": ["TopCopper","BottomCopper"], "net": 2 }
          ],
          "zones": [], "graphics": [], "texts": []
        }"#,
    )
    .expect("valid board")
}

fn board_theme() -> ResolvedTheme {
    ResolvedTheme::from_canvas_colors(canvas_colors(ThemeId::GraphiteDark))
}

fn scene_of(snapshot: &PcbSnapshot) -> Scene {
    let mut scene = Scene::default();
    PcbRenderer::build_scene(
        snapshot,
        &board_theme(),
        DirtyFlags::LINES | DirtyFlags::CIRCLES | DirtyFlags::POLYGONS,
        &mut scene,
    );
    scene
}

fn palette_rgba(c: signex_types::theme::Color) -> [f32; 4] {
    [c.r, c.g, c.b, c.a].map(|v| f32::from(v) / 255.0)
}

#[test]
fn top_and_bottom_copper_draw_in_their_layer_colours() {
    let layers = canvas_colors(ThemeId::GraphiteDark).board_layer_colors();
    let top = palette_rgba(layers.top_copper);
    let bottom = palette_rgba(layers.bottom_copper);
    let multi = palette_rgba(layers.multi_layer);
    let scene = scene_of(&PcbSnapshot::from_board(&board()));

    assert_eq!(scene.lines[0].color, top, "top track");
    // Pads in footprint order: top SMD, bottom SMD, THT pad then its drill.
    assert_eq!(scene.top_polygons[0].fill_color, top, "top SMD pad");
    assert_eq!(scene.top_polygons[1].fill_color, bottom, "bottom SMD pad");
    assert_eq!(scene.top_polygons[2].fill_color, multi, "THT pad");
    assert_eq!(scene.circles[0].color, multi, "through via");
}

#[test]
fn copper_off_the_active_layer_keeps_its_colour_but_dims() {
    let layers = canvas_colors(ThemeId::GraphiteDark).board_layer_colors();
    let bottom = palette_rgba(layers.bottom_copper);
    let snapshot = PcbSnapshot::from_board_with_view(&board(), &PcbLayerView::default());
    let scene = scene_of(&snapshot);

    let dimmed: Vec<_> = scene
        .top_polygons
        .iter()
        .filter(|p| p.fill_color[3] < 1.0)
        .collect();
    assert_eq!(dimmed.len(), 1, "only the bottom pad is off Top Layer");
    assert_eq!(dimmed[0].fill_color[..3], bottom[..3]);
    assert!((dimmed[0].fill_color[3] - DIMMED_ALPHA).abs() < 1e-6);
}

/// Caner (2026-11-21): vias and through-hole pads show the copper of the
/// active layer, and follow it.
#[test]
fn through_items_take_the_colour_of_the_active_copper_layer() {
    let layers = canvas_colors(ThemeId::GraphiteDark).board_layer_colors();
    for (active, colour) in [
        (CopperLayer::Top, palette_rgba(layers.top_copper)),
        (CopperLayer::Bottom, palette_rgba(layers.bottom_copper)),
    ] {
        let mut view = PcbLayerView::default();
        view.set_active(active);
        let scene = scene_of(&PcbSnapshot::from_board_with_view(&board(), &view));
        let via = scene.circles.iter().find(|c| !c.is_filled()).expect("via");
        assert_eq!(via.color, colour, "{active:?}");
        assert!(
            scene
                .top_polygons
                .iter()
                .any(|p| p.fill_color == colour && p.vertices.iter().any(|v| v[0] > 15.0)),
            "the THT pad (x 16 mm), {active:?}"
        );
    }
}

/// A non-copper active layer leaves vias and through-hole pads neutral, and
/// their own ink stays the Multi-Layer whatever is active.
#[test]
fn through_items_stay_neutral_on_a_non_copper_active_layer() {
    use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};
    let layers = canvas_colors(ThemeId::GraphiteDark).board_layer_colors();
    let multi = palette_rgba(layers.multi_layer);
    let mut view = PcbLayerView::default();
    view.set_active(PcbLayer::Technical(TechnicalLayer::SilkscreenTop));
    let snapshot = PcbSnapshot::from_board_with_view(&board(), &view);
    assert_eq!(snapshot.vias[0].shown_on, None);
    assert_eq!(snapshot.vias[0].ink, CopperInk::MultiLayer);
    let scene = scene_of(&snapshot);
    let via = scene.circles.iter().find(|c| !c.is_filled()).expect("via");
    assert_eq!(via.color[..3], multi[..3]);
    let through = snapshot
        .pads
        .iter()
        .find(|p| p.pad_type == PadType::Thru)
        .expect("the THT pad");
    assert_eq!(through.shown_on, None);
}

/// A via with no copper on the active layer keeps the neutral colour.
#[test]
fn a_via_off_the_active_layer_stays_neutral() {
    let layers = canvas_colors(ThemeId::GraphiteDark).board_layer_colors();
    let multi = palette_rgba(layers.multi_layer);
    let mut board = board();
    board.vias[0].layers = vec!["TopCopper".to_owned()];
    let mut view = PcbLayerView::default();
    view.set_active(CopperLayer::Bottom);
    let snapshot = PcbSnapshot::from_board_with_view(&board, &view);
    assert_eq!(snapshot.vias[0].shown_on, None);
    let scene = scene_of(&snapshot);
    let via = scene.circles.iter().find(|c| !c.is_filled()).expect("via");
    assert_eq!(via.color[..3], multi[..3]);
}

/// The ring and the pad take the active layer's own size with its colour.
#[test]
fn through_items_take_the_size_of_the_active_layer() {
    use signex_types::pcb::{LayerSizes, Point};
    let mut board = board();
    board.vias[0].layer_sizes = Some(LayerSizes::TopMiddleBottom {
        top: 0.8,
        middle: 0.5,
        bottom: 0.6,
    });
    board.footprints[0].pads[2].layer_sizes = Some(LayerSizes::TopMiddleBottom {
        top: Point::new(1.8, 1.8),
        middle: Point::new(1.5, 1.5),
        bottom: Point::new(2.4, 2.4),
    });
    let mut view = PcbLayerView::default();
    for (active, ring, pad) in [
        (CopperLayer::Top, 0.8, 1.8),
        (CopperLayer::Bottom, 0.6, 2.4),
    ] {
        view.set_active(active);
        let snapshot = PcbSnapshot::from_board_with_view(&board, &view);
        assert_eq!(snapshot.vias[0].diameter_mm, ring, "{active:?}");
        assert_eq!(snapshot.vias[0].shown_on, Some(active));
        let through = snapshot
            .pads
            .iter()
            .find(|p| p.pad_type == PadType::Thru)
            .expect("the THT pad");
        assert_eq!(through.size_mm, [pad, pad], "{active:?}");
        assert_eq!(through.shown_on, Some(active));
    }
}

/// An SMD pad keeps its own layer's colour with another layer active.
#[test]
fn an_smd_pad_keeps_its_own_colour() {
    let layers = canvas_colors(ThemeId::GraphiteDark).board_layer_colors();
    let top = palette_rgba(layers.top_copper);
    let mut view = PcbLayerView::default();
    view.set_active(CopperLayer::Bottom);
    let snapshot = PcbSnapshot::from_board_with_view(&board(), &view);
    assert!(
        snapshot
            .pads
            .iter()
            .filter(|p| p.pad_type == PadType::Smd)
            .all(|p| p.shown_on.is_none())
    );
    let scene = scene_of(&snapshot);
    assert!(
        scene
            .top_polygons
            .iter()
            .any(|p| p.fill_color[..3] == top[..3]),
        "the top SMD pad, dimmed in its own colour"
    );
}

#[test]
fn hiding_the_multi_layer_hides_vias_and_through_hole_pads() {
    let mut view = PcbLayerView::default();
    view.toggle_multi_layer();
    assert!(view.has_hidden());
    let snapshot = PcbSnapshot::from_board_with_view(&board(), &view);
    assert!(snapshot.vias.is_empty());
    assert_eq!(snapshot.pads.len(), 2, "the two SMD pads stay");
    assert!(snapshot.pads.iter().all(|p| p.drill_mm == 0.0));
    view.show_all();
    assert!(view.multi_layer_visible() && !view.has_hidden());
    assert_eq!(
        PcbSnapshot::from_board_with_view(&board(), &view)
            .vias
            .len(),
        1
    );
}

#[test]
fn the_board_area_fills_the_outline_under_every_zone() {
    use signex_types::pcb::Point;
    let mut board = board();
    board.outline = vec![
        Point::new(0.0, -5.0),
        Point::new(30.0, -5.0),
        Point::new(30.0, 10.0),
        Point::new(0.0, 10.0),
    ];
    let scene = scene_of(&PcbSnapshot::from_board(&board));
    let area = &scene.polygons[0];
    assert_eq!(area.fill_color, board_theme().color(ColorSlot::BoardArea));
    assert_eq!(area.vertices.len(), 4);
    assert!(area.holes.is_empty() && area.stroke_color.is_none());
    let canvas = canvas_colors(ThemeId::GraphiteDark);
    assert_eq!(
        area.fill_color,
        palette_rgba(
            canvas
                .board_area
                .expect("the board palette sets a board area")
        )
    );
}

#[test]
fn a_tht_pad_and_a_via_draw_their_drill_as_an_opening() {
    let opening = board_theme().color(ColorSlot::BoardOpening);
    let snapshot = PcbSnapshot::from_board(&board());
    let scene = scene_of(&snapshot);

    // Three pads, plus one drill for the THT pad, right after it.
    assert_eq!(scene.top_polygons.len(), 4);
    let drill = &scene.top_polygons[3];
    assert_eq!(drill.fill_color, opening);
    assert!(drill.stroke_color.is_none());
    let tht = &snapshot.pads[2];
    for v in &drill.vertices {
        let r = ((v[0] - tht.center[0]).powi(2) + (v[1] - tht.center[1]).powi(2)).sqrt();
        assert!((r - 0.5).abs() < 1e-4, "drill radius {r}");
    }
    // SMD pads have no drill.
    assert!(
        scene.top_polygons[..3]
            .iter()
            .all(|p| p.fill_color != opening)
    );

    // The via: its copper ring, then a disc of the drill's radius.
    assert_eq!(scene.circles.len(), 2);
    let hole = scene.circles[1];
    assert!(hole.is_filled());
    assert_eq!(hole.color, opening);
    assert!((hole.radius - 0.2).abs() < 1e-6);
}

#[test]
fn copper_slots_follow_the_layer_cycle_for_mid_layers() {
    use signex_types::pcb_layer::InnerCopper;
    assert_eq!(copper_slot(CopperLayer::Top), ColorSlot::TopCopper);
    assert_eq!(copper_slot(CopperLayer::Bottom), ColorSlot::BottomCopper);
    let mid = |n| CopperLayer::Inner(InnerCopper::new(n).expect("inner"));
    assert_eq!(copper_slot(mid(1)), ColorSlot::LayerCycle1);
    assert_eq!(copper_slot(mid(6)), ColorSlot::LayerCycle0);
    assert_eq!(copper_slot(mid(13)), ColorSlot::LayerCycle1);
}

/// A non-plated hole has no copper: it draws as its opening alone, the
/// drill circle, or its own shape when it gives no drill.
#[test]
fn an_npth_pad_draws_only_its_opening() {
    let opening = board_theme().color(ColorSlot::BoardOpening);
    let mut board = board();
    let pad = &mut board.footprints[0].pads[2];
    pad.pad_type = PadType::NpThru;
    let snapshot = PcbSnapshot::from_board(&board);
    let scene = scene_of(&snapshot);

    // Two SMD pads and the hole: no copper annulus, no second polygon.
    assert_eq!(scene.top_polygons.len(), 3);
    let hole = &scene.top_polygons[2];
    assert_eq!(hole.fill_color, opening);
    let npth = &snapshot.pads[2];
    for v in &hole.vertices {
        let r = ((v[0] - npth.center[0]).powi(2) + (v[1] - npth.center[1]).powi(2)).sqrt();
        assert!((r - 0.5).abs() < 1e-4, "drill radius {r}");
    }

    board.footprints[0].pads[2].drill = None;
    let snapshot = PcbSnapshot::from_board(&board);
    let scene = scene_of(&snapshot);
    assert_eq!(scene.top_polygons.len(), 3);
    assert_eq!(scene.top_polygons[2].fill_color, opening);
    assert_eq!(scene.top_polygons[2].vertices, snapshot.pads[2].outline);
}
