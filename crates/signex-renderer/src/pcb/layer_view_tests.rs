use super::*;
use crate::schematic::ViewRenderer;
use crate::theme::ResolvedTheme;
use signex_gfx::scene::{DirtyFlags, Scene};
use signex_types::pcb_layer::{InnerCopper, MechanicalLayer};

const TOP: PcbLayer = PcbLayer::Copper(CopperLayer::Top);
const BOTTOM: PcbLayer = PcbLayer::Copper(CopperLayer::Bottom);

fn board() -> PcbBoard {
    serde_json::from_str(
        r#"{
          "uuid": "8d15b2f9-8f86-41d7-9ec4-0f54a4f3a651", "version": 1,
          "generator": "test", "thickness": 1.6, "outline": [],
          "layers": [
            { "id": 0, "name": "TopCopper", "layer_type": "signal" },
            { "id": 31, "name": "BottomCopper", "layer_type": "signal" },
            { "id": 40, "name": "Mechanical3", "layer_type": "mechanical", "display_name": "Assembly notes" }
          ],
          "setup": null, "nets": [], "footprints": [],
          "segments": [
            { "uuid": "f5ea9948-4c2f-4501-b647-4047f6eefca9", "start": {"x":0.0,"y":0.0}, "end": {"x":5.0,"y":0.0}, "width": 0.25, "layer": "TopCopper", "net": 1 },
            { "uuid": "f5ea9948-4c2f-4501-b647-4047f6eefca8", "start": {"x":0.0,"y":2.0}, "end": {"x":5.0,"y":2.0}, "width": 0.25, "layer": "BottomCopper", "net": 2 },
            { "uuid": "f5ea9948-4c2f-4501-b647-4047f6eefca7", "start": {"x":0.0,"y":4.0}, "end": {"x":5.0,"y":4.0}, "width": 0.25, "layer": "Unknown.Layer", "net": 3 }
          ],
          "vias": [
            { "uuid": "7f2eb13d-a3d6-47e6-84f7-3b27240a95f3", "position": {"x":2.5,"y":1.0}, "diameter": 0.8, "drill": 0.4, "layers": ["TopCopper","BottomCopper"], "net": 2 }
          ],
          "zones": [], "graphics": [], "texts": []
        }"#,
    )
    .expect("valid board")
}

#[test]
fn a_hidden_layer_is_not_in_the_snapshot_and_comes_back() {
    let board = board();
    let mut view = PcbLayerView::default();
    assert_eq!(
        PcbSnapshot::from_board_with_view(&board, &view)
            .traces
            .len(),
        3
    );
    view.toggle_visibility(BOTTOM);
    let snapshot = PcbSnapshot::from_board_with_view(&board, &view);
    assert_eq!(snapshot.traces.len(), 2, "the bottom trace is gone");
    assert_eq!(snapshot.vias.len(), 1, "a via on a shown layer stays");
    view.toggle_visibility(TOP);
    let snapshot = PcbSnapshot::from_board_with_view(&board, &view);
    assert_eq!(snapshot.traces.len(), 1, "only the unnamed layer's trace");
    assert!(
        snapshot.vias.is_empty(),
        "a via with every layer hidden goes"
    );
    view.show_all();
    assert_eq!(
        PcbSnapshot::from_board_with_view(&board, &view)
            .traces
            .len(),
        3
    );
}

#[test]
fn the_active_layer_is_drawn_last_and_the_rest_are_dimmed() {
    let board = board();
    let mut view = PcbLayerView::default();
    let snapshot = PcbSnapshot::from_board_with_view(&board, &view);
    let nets: Vec<(u32, bool)> = snapshot.traces.iter().map(|t| (t.net, t.dimmed)).collect();
    assert_eq!(nets, vec![(2, true), (3, false), (1, false)]);

    view.set_active(CopperLayer::Bottom);
    let snapshot = PcbSnapshot::from_board_with_view(&board, &view);
    let nets: Vec<(u32, bool)> = snapshot.traces.iter().map(|t| (t.net, t.dimmed)).collect();
    assert_eq!(nets, vec![(1, true), (3, false), (2, false)]);
}

#[test]
fn a_plain_snapshot_dims_nothing() {
    let snapshot = PcbSnapshot::from_board(&board());
    assert!(snapshot.traces.iter().all(|t| !t.dimmed));
}

#[test]
fn dimmed_copper_is_emitted_fainter() {
    let board = board();
    let snapshot = PcbSnapshot::from_board_with_view(&board, &PcbLayerView::default());
    let theme = ResolvedTheme::builtin_default();
    let mut scene = Scene::default();
    super::super::PcbRenderer::build_scene(&snapshot, &theme, DirtyFlags::LINES, &mut scene);
    let alpha = |net: u32| {
        let i = snapshot.traces.iter().position(|t| t.net == net).unwrap();
        scene.lines[i].color[3]
    };
    assert!((alpha(2) - alpha(1) * DIMMED_ALPHA).abs() < 1e-6);
}

#[test]
fn stepping_wraps_through_the_copper_layers() {
    let inner = CopperLayer::Inner(InnerCopper::new(1).unwrap());
    let copper = [CopperLayer::Top, inner, CopperLayer::Bottom];
    let mut view = PcbLayerView::default();
    view.step_active(&copper, true);
    assert_eq!(view.active(), inner);
    view.step_active(&copper, true);
    view.step_active(&copper, true);
    assert_eq!(view.active(), CopperLayer::Top, "wraps forward");
    view.step_active(&copper, false);
    assert_eq!(view.active(), CopperLayer::Bottom, "wraps back");
    view.set_active(inner);
    view.step_active(&[CopperLayer::Top, CopperLayer::Bottom], true);
    assert_eq!(view.active(), CopperLayer::Top, "a layer the board lacks");
}

#[test]
fn a_name_this_build_does_not_know_cannot_be_hidden() {
    let mut view = PcbLayerView::default();
    for layer in PcbLayer::all() {
        view.toggle_visibility(layer);
    }
    assert!(view.shows_any(&["F.Cu".to_owned()]));
    assert!(view.shows_any(&[]));
    assert!(!view.shows_any(&["TopCopper".to_owned()]));
}

#[test]
fn rows_list_the_boards_copper_every_technical_layer_and_named_mechanical_layers() {
    let rows = layer_rows(&board().layers);
    let copper: Vec<_> = rows
        .iter()
        .filter(|r| r.group == LayerGroup::Copper)
        .collect();
    assert_eq!(copper.len(), 2);
    assert_eq!(copper[0].layer, TOP);
    assert!(copper.iter().all(|r| r.user_name.is_none()));
    let technical = rows
        .iter()
        .filter(|r| r.group == LayerGroup::Technical)
        .count();
    assert_eq!(technical, TechnicalLayer::ALL.len());
    let mechanical: Vec<_> = rows
        .iter()
        .filter(|r| r.group == LayerGroup::Mechanical)
        .collect();
    assert_eq!(mechanical.len(), 1);
    assert_eq!(mechanical[0].user_name.as_deref(), Some("Assembly notes"));
    assert_eq!(
        mechanical[0].layer,
        PcbLayer::Mechanical(MechanicalLayer::new(3).unwrap())
    );
    // No stackup at all still lists Top and Bottom.
    assert_eq!(
        board_copper_layers(&[]),
        vec![CopperLayer::Top, CopperLayer::Bottom]
    );
}

fn smd_pad_board() -> PcbBoard {
    let mut board = board();
    board.footprints = serde_json::from_str(
        r#"[{
          "uuid": "8e98ba95-1d0f-4d9f-84e0-27ddf90e10b5", "reference": "U1", "value": "",
          "footprint_id": "", "position": {"x": 0.0, "y": 0.0}, "rotation": 0.0,
          "layer": "TopCopper", "locked": false,
          "pads": [{
            "uuid": "47ceef20-d496-4e62-b8d8-7c34195c6818", "number": "1", "pad_type": "smd",
            "shape": "rect", "position": {"x": 1.0, "y": 0.0}, "size": {"x": 1.0, "y": 1.0},
            "drill": null, "layers": ["TopCopper", "TopSolderMask", "TopPaste"],
            "net": null, "roundrect_ratio": 0.0
          }],
          "graphics": [], "properties": []
        }]"#,
    )
    .expect("valid footprint");
    board
}

/// Review #2233: an SMD pad on Top Copper, Top Solder Mask and Top Paste
/// goes with Top Copper, not with the mask layer that is still shown.
#[test]
fn a_mask_and_paste_pad_follows_its_copper_layer() {
    let board = smd_pad_board();
    let (footprint, pad) = (&board.footprints[0], &board.footprints[0].pads[0]);
    let mut view = PcbLayerView::default();
    assert!(view.shows_pad(footprint, pad));
    assert_eq!(
        PcbSnapshot::from_board_with_view(&board, &view).pads.len(),
        1
    );
    view.toggle_visibility(TOP);
    assert!(!view.shows_pad(footprint, pad));
    assert!(
        PcbSnapshot::from_board_with_view(&board, &view)
            .pads
            .is_empty()
    );
    // Hiding the mask alone leaves the copper pad drawn.
    view.show_all();
    view.toggle_visibility(PcbLayer::Technical(TechnicalLayer::SolderMaskTop));
    assert!(view.shows_pad(footprint, pad));
}

/// Review #2233: with the active layer hidden and no shown copper left,
/// nothing is dimmed; settling moves the active layer to a shown one.
#[test]
fn a_hidden_active_layer_moves_on_or_lifts_the_dimming() {
    let board = board();
    let copper = board_copper_layers(&board.layers);
    let mut view = PcbLayerView::default();
    view.toggle_visibility(TOP);
    view.settle_active(&copper);
    assert_eq!(view.active(), CopperLayer::Bottom);
    let snapshot = PcbSnapshot::from_board_with_view(&board, &view);
    assert!(
        snapshot.traces.iter().all(|t| !t.dimmed),
        "bottom is active now"
    );

    view.toggle_visibility(BOTTOM);
    view.settle_active(&copper);
    assert_eq!(view.shown_active(), None, "no copper is shown");
    let snapshot = PcbSnapshot::from_board_with_view(&board, &view);
    assert!(
        snapshot.traces.iter().all(|t| !t.dimmed),
        "nothing is dimmed"
    );
}

#[test]
fn stepping_skips_hidden_layers() {
    let inner = CopperLayer::Inner(InnerCopper::new(1).unwrap());
    let copper = [CopperLayer::Top, inner, CopperLayer::Bottom];
    let mut view = PcbLayerView::default();
    view.toggle_visibility(PcbLayer::Copper(inner));
    view.step_active(&copper, true);
    assert_eq!(view.active(), CopperLayer::Bottom);
    view.step_active(&copper, false);
    assert_eq!(view.active(), CopperLayer::Top);
    view.toggle_visibility(BOTTOM);
    view.step_active(&copper, true);
    assert_eq!(
        view.active(),
        CopperLayer::Top,
        "the only shown layer stays"
    );
}

/// Review #2233: a through via listed as Top and Bottom passes through an
/// active inner layer, so it is not dimmed there.
#[test]
fn a_via_spanning_the_active_layer_is_on_it() {
    let inner = CopperLayer::Inner(InnerCopper::new(1).unwrap());
    let mut view = PcbLayerView::default();
    view.set_active(inner);
    let through = ["TopCopper".to_owned(), "BottomCopper".to_owned()];
    assert_eq!(view.emphasis(&through), Emphasis::Active);
    assert_eq!(view.emphasis(&["TopCopper".to_owned()]), Emphasis::Dimmed);
    assert_eq!(
        view.emphasis(&["TopSilkscreen".to_owned()]),
        Emphasis::Normal
    );
}
