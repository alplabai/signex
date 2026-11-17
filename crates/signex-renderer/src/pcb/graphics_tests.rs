use super::*;
use crate::pcb::{DIMMED_ALPHA, PcbRenderer, PcbSnapshot};
use crate::schematic::ViewRenderer;
use signex_gfx::scene::DirtyFlags;
use signex_types::pcb_layer::{CopperLayer, TechnicalLayer};

const TOP_OVERLAY: PcbLayer = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);
const BOTTOM_OVERLAY: PcbLayer = PcbLayer::Technical(TechnicalLayer::SilkscreenBottom);
const TOP_COURTYARD: PcbLayer = PcbLayer::Technical(TechnicalLayer::CourtyardTop);

/// R1 at (10, 20), turned 90 degrees: a silkscreen line along its local x
/// axis from 1 to 2 mm, a courtyard circle, a copper line and a silkscreen
/// text. The board has its own line on Top Fab and a text on the bottom
/// overlay.
fn board() -> PcbBoard {
    serde_json::from_str(
        r#"{
          "uuid": "8d15b2f9-8f86-41d7-9ec4-0f54a4f3a651",
          "layers": [
            { "id": 0, "name": "TopCopper", "layer_type": "signal" },
            { "id": 31, "name": "BottomCopper", "layer_type": "signal" }
          ],
          "footprints": [{
            "uuid": "8e98ba95-1d0f-4d9f-84e0-27ddf90e10b5",
            "reference": "R1", "value": "10k",
            "position": { "x": 10.0, "y": 20.0 }, "rotation": 90.0,
            "layer": "TopCopper",
            "graphics": [
              { "graphic_type": "line", "layer": "TopSilkscreen", "width": 0.12,
                "start": { "x": 1.0, "y": 0.0 }, "end": { "x": 2.0, "y": 0.0 } },
              { "graphic_type": "circle", "layer": "TopCourtyard", "width": 0.05,
                "center": { "x": 0.0, "y": 0.0 }, "radius": 3.0 },
              { "graphic_type": "line", "layer": "TopCopper", "width": 0.2,
                "start": { "x": 0.0, "y": 1.0 }, "end": { "x": 0.0, "y": 2.0 } },
              { "graphic_type": "text", "layer": "TopSilkscreen", "text": "+",
                "position": { "x": -1.0, "y": 0.0 }, "font_size": 0.6 }
            ]
          }],
          "graphics": [
            { "graphic_type": "line", "layer": "TopFab", "width": 0.1,
              "start": { "x": 0.0, "y": 0.0 }, "end": { "x": 5.0, "y": 0.0 } }
          ],
          "texts": [
            { "uuid": "2f7e4c1a-0d3b-4c55-9a51-6c3f0e8b7d21", "text": "REV A",
              "position": { "x": 30.0, "y": 5.0 }, "layer": "BottomSilkscreen",
              "font_size": 1.2, "rotation": 0.0 }
          ]
        }"#,
    )
    .expect("valid board")
}

fn snapshot(board: &PcbBoard, view: &PcbLayerView) -> PcbSnapshot {
    PcbSnapshot::from_board_with_view(board, view)
}

fn on(snapshot: &PcbSnapshot, layer: PcbLayer) -> Vec<&GraphicInput> {
    snapshot
        .graphics
        .iter()
        .filter(|g| g.layer == layer)
        .collect()
}

fn overlay(snapshot: &PcbSnapshot) -> Scene {
    let mut scene = Scene::default();
    let theme = ResolvedTheme::builtin_default();
    PcbRenderer::build_scene(snapshot, &theme, DirtyFlags::OVERLAY, &mut scene);
    scene
}

#[test]
fn a_silkscreen_line_lands_turned_and_moved_on_top_overlay() {
    let shown = snapshot(&board(), &PcbLayerView::default());
    let silk = on(&shown, TOP_OVERLAY);
    assert_eq!(silk.len(), 1);
    // Local (1, 0) and (2, 0) turned a quarter: (0, 1) and (0, 2) from (10, 20).
    assert_eq!(
        silk[0].shape,
        GraphicShapeInput::Line {
            p0: [10.0, 21.0],
            p1: [10.0, 22.0],
        }
    );
    assert_eq!(silk[0].width_mm, 0.12);
    // It reaches the scene in the Top Overlay colour of the board palette.
    let theme = ResolvedTheme::builtin_default();
    let scene = overlay(&shown);
    let line = scene
        .overlay_lines
        .iter()
        .find(|l| l.p0 == [10.0, 21.0] && l.p1 == [10.0, 22.0])
        .expect("the silkscreen line is drawn");
    assert_eq!(line.color, theme.board_layer_color(TOP_OVERLAY));
    assert_eq!(line.width, 0.12);
}

#[test]
fn a_flipped_footprint_draws_its_silk_mirrored_on_bottom_overlay() {
    let mut flipped = board();
    pcb_geom::flip_footprint(&mut flipped.footprints[0]);
    let shown = snapshot(&flipped, &PcbLayerView::default());
    assert!(on(&shown, TOP_OVERLAY).is_empty());
    let silk = on(&shown, BOTTOM_OVERLAY);
    assert_eq!(silk.len(), 1);
    // Rotation 270 after the flip; local x mirrored: (-1, 0) and (-2, 0)
    // turned 270 degrees are (0, 1) and (0, 2), the same spots the top side
    // had, as a physical turn over puts them.
    assert_eq!(
        silk[0].shape,
        GraphicShapeInput::Line {
            p0: [10.0, 21.0],
            p1: [10.0, 22.0],
        }
    );
    // The courtyard went to the bottom too.
    assert!(on(&shown, TOP_COURTYARD).is_empty());
}

#[test]
fn a_bottom_footprint_mirrors_its_silk_about_its_own_y_axis() {
    let mut bottom = board();
    bottom.footprints[0].rotation = 0.0;
    bottom.footprints[0].layer = "BottomCopper".into();
    bottom.footprints[0].graphics[0].layer = "BottomSilkscreen".into();
    let shown = snapshot(&bottom, &PcbLayerView::default());
    let silk = on(&shown, BOTTOM_OVERLAY);
    assert_eq!(silk.len(), 1);
    assert_eq!(
        silk[0].shape,
        GraphicShapeInput::Line {
            p0: [9.0, 20.0],
            p1: [8.0, 20.0],
        }
    );
}

#[test]
fn hiding_top_overlay_removes_its_drawings_from_the_scene() {
    let board = board();
    let mut view = PcbLayerView::default();
    let silk_colour = ResolvedTheme::builtin_default().board_layer_color(TOP_OVERLAY);
    let silk_lines = |view: &PcbLayerView| {
        overlay(&snapshot(&board, view))
            .overlay_lines
            .iter()
            .filter(|l| l.color == silk_colour)
            .count()
    };
    assert_eq!(silk_lines(&view), 1);
    view.toggle_visibility(TOP_OVERLAY);
    let hidden = snapshot(&board, &view);
    assert!(on(&hidden, TOP_OVERLAY).is_empty());
    assert!(hidden.texts.iter().all(|t| t.layer != TOP_OVERLAY));
    assert_eq!(silk_lines(&view), 0);
    // The other layers stay.
    assert_eq!(on(&hidden, TOP_COURTYARD).len(), 1);
}

#[test]
fn single_layer_mode_draws_only_the_active_layers_drawings() {
    let mut view = PcbLayerView::default();
    view.set_active(TOP_OVERLAY);
    view.toggle_single_layer();
    let shown = snapshot(&board(), &view);
    assert!(!shown.graphics.is_empty());
    assert!(shown.graphics.iter().all(|g| g.layer == TOP_OVERLAY));
}

#[test]
fn copper_drawings_off_the_active_layer_are_dimmed_and_the_rest_are_not() {
    let mut view = PcbLayerView::default();
    view.set_active(CopperLayer::Bottom);
    let shown = snapshot(&board(), &view);
    let copper = on(&shown, PcbLayer::Copper(CopperLayer::Top));
    assert_eq!(copper.len(), 1);
    assert!(copper[0].dimmed);
    assert!(on(&shown, TOP_OVERLAY).iter().all(|g| !g.dimmed));
    let scene = overlay(&shown);
    let theme = ResolvedTheme::builtin_default();
    let faint = theme.board_layer_color(PcbLayer::Copper(CopperLayer::Top))[3] * DIMMED_ALPHA;
    assert!(
        scene
            .overlay_lines
            .iter()
            .any(|l| l.p0 == [9.0, 20.0] && (l.color[3] - faint).abs() < 1e-6)
    );
}

#[test]
fn a_board_text_and_a_text_drawing_draw_on_their_layers() {
    let shown = snapshot(&board(), &PcbLayerView::default());
    let rev = shown
        .texts
        .iter()
        .find(|t| t.content == "REV A")
        .expect("the board text is drawn");
    assert_eq!(rev.layer, BOTTOM_OVERLAY);
    assert_eq!(rev.position, [30.0, 5.0]);
    assert_eq!(rev.size_mm, 1.2);
    assert!(rev.mirrored, "read from below");
    let plus = shown
        .texts
        .iter()
        .find(|t| t.content == "+")
        .expect("the text drawing is drawn");
    assert_eq!(plus.layer, TOP_OVERLAY);
    // Local (-1, 0) turned a quarter: (0, -1) from (10, 20).
    assert_eq!(plus.position, [10.0, 19.0]);
    assert_eq!(plus.rotation_deg, 90.0);

    let mut view = PcbLayerView::default();
    view.toggle_visibility(BOTTOM_OVERLAY);
    let hidden = snapshot(&board(), &view);
    assert!(hidden.texts.iter().all(|t| t.content != "REV A"));
}

#[test]
fn a_board_drawing_is_drawn_where_it_is_saved() {
    let shown = snapshot(&board(), &PcbLayerView::default());
    let fab = on(&shown, PcbLayer::Technical(TechnicalLayer::FabTop));
    assert_eq!(fab.len(), 1);
    assert_eq!(
        fab[0].shape,
        GraphicShapeInput::Line {
            p0: [0.0, 0.0],
            p1: [5.0, 0.0],
        }
    );
}

#[test]
fn circles_draw_as_rings_or_discs_and_arcs_as_chords() {
    let ring = GraphicInput {
        shape: GraphicShapeInput::Circle {
            center: [0.0, 0.0],
            radius_mm: 1.0,
        },
        layer: TOP_COURTYARD,
        width_mm: 0.1,
        filled: false,
        dimmed: false,
    };
    let disc = GraphicInput {
        filled: true,
        ..ring.clone()
    };
    let arc = GraphicInput {
        shape: GraphicShapeInput::Arc {
            center: [0.0, 0.0],
            radius_mm: 1.0,
            start_deg: 0.0,
            sweep_deg: 90.0,
        },
        ..ring.clone()
    };
    let mut scene = Scene::default();
    emit(
        &[ring, disc, arc],
        &ResolvedTheme::builtin_default(),
        &mut scene,
    );
    assert_eq!(scene.overlay_circles.len(), 2);
    assert!(!scene.overlay_circles[0].is_filled());
    assert_eq!(scene.overlay_circles[0].radius, 1.0);
    assert!(scene.overlay_circles[1].is_filled());
    assert!((scene.overlay_circles[1].radius - 1.05).abs() < 1e-6);
    let chords = &scene.overlay_lines;
    assert_eq!(chords.len(), 15, "90 degrees in 6 degree chords");
    assert_eq!(chords[0].p0, [1.0, 0.0]);
    let end = chords[chords.len() - 1].p1;
    assert!(
        end[0].abs() < 1e-6 && (end[1] - 1.0).abs() < 1e-6,
        "{end:?}"
    );
}

#[test]
fn the_carried_footprint_takes_its_drawings_and_the_library_preview_does_not() {
    let board = board();
    assert!(
        PcbSnapshot::from_footprint(&board.footprints[0])
            .graphics
            .is_empty()
    );
    let drawn = snapshot(&board, &PcbLayerView::default());
    let footprint_only = PcbBoard {
        graphics: Vec::new(),
        texts: Vec::new(),
        free: Default::default(),
        ..board.clone()
    };
    let carried = snapshot(&footprint_only, &PcbLayerView::default());
    assert_eq!(carried.graphics.len(), drawn.graphics.len() - 1);
}

/// #2424: a text frame draws its box as an outline and its lines as board
/// texts, inside the box, on the frame's layer.
#[test]
fn a_text_frame_draws_its_box_and_its_lines() {
    let mut board = board();
    board.graphics = vec![
        serde_json::from_str(
            r#"{ "graphic_type": "text_frame", "layer": "TopSilkscreen", "width": 0.1,
                 "start": { "x": 0.0, "y": 0.0 }, "end": { "x": 10.0, "y": 4.0 },
                 "text": "LINE ONE\nLINE TWO", "font_size": 1.0 }"#,
        )
        .expect("a drawing"),
    ];
    board.texts.clear();
    let shown = snapshot(&board, &PcbLayerView::default());
    let boxes: Vec<&GraphicInput> = on(&shown, TOP_OVERLAY)
        .into_iter()
        .filter(|g| matches!(&g.shape, GraphicShapeInput::Polygon { points } if points.len() == 4))
        .collect();
    assert_eq!(boxes.len(), 1, "the box");
    assert!(!boxes[0].filled);
    let lines: Vec<&FootprintTextInput> = shown
        .texts
        .iter()
        .filter(|t| t.content.starts_with("LINE"))
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].content, "LINE ONE");
    assert_eq!(lines[0].layer, TOP_OVERLAY);
    assert!(lines[0].position[1] < lines[1].position[1], "top down");
    assert!(
        lines
            .iter()
            .all(|t| t.position[1] < 4.0 && t.position[0] == 5.0)
    );
}
