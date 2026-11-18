use super::*;
use crate::pcb::{
    LayerGroup, LayerRow, board_copper_layers, board_layers, group_layers, layer_rows,
    signal_layers,
};
use crate::schematic::ViewRenderer;
use crate::theme::ResolvedTheme;
use signex_gfx::scene::{DirtyFlags, Scene};
use signex_types::pcb::LayerDef;
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
    let copper = [TOP, PcbLayer::Copper(inner), BOTTOM];
    let mut view = PcbLayerView::default();
    view.step_active(&copper, true);
    assert_eq!(view.active(), PcbLayer::Copper(inner));
    view.step_active(&copper, true);
    view.step_active(&copper, true);
    assert_eq!(view.active(), TOP, "wraps forward");
    view.step_active(&copper, false);
    assert_eq!(view.active(), BOTTOM, "wraps back");
    view.set_active(inner);
    view.step_active(&[TOP, BOTTOM], true);
    assert_eq!(view.active(), TOP, "a layer the board lacks");
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
    let in_group = |group| -> Vec<&LayerRow> { rows.iter().filter(|r| r.group == group).collect() };
    let signal = in_group(LayerGroup::Signal);
    assert_eq!(signal.len(), 2);
    assert_eq!(signal[0].layer, TOP);
    assert!(signal.iter().all(|r| r.user_name.is_none()));
    assert!(
        in_group(LayerGroup::Plane).is_empty(),
        "no plane in the stackup"
    );
    let technical = rows
        .iter()
        .filter(|r| matches!(r.layer, PcbLayer::Technical(_)))
        .count();
    assert_eq!(technical, TechnicalLayer::ALL.len());
    let mechanical: Vec<_> = in_group(LayerGroup::Mechanical)
        .into_iter()
        .filter(|r| matches!(r.layer, PcbLayer::Mechanical(_)))
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

/// #2359: the panel groups the technical layers as overlay, mask, drill
/// and mechanical (the board outline among them); a plane in the stackup
/// lists under Plane and is not a signal layer.
#[test]
fn technical_layers_and_planes_fall_in_their_groups() {
    let group_of = |rows: &[LayerRow], layer: PcbLayer| {
        rows.iter().find(|r| r.layer == layer).map(|r| r.group)
    };
    let rows = layer_rows(&board().layers);
    let tech = |t| PcbLayer::Technical(t);
    for (layer, group) in [
        (tech(TechnicalLayer::SilkscreenTop), LayerGroup::Overlay),
        (tech(TechnicalLayer::SolderMaskBottom), LayerGroup::Mask),
        (tech(TechnicalLayer::PasteTop), LayerGroup::Mask),
        (tech(TechnicalLayer::DrillDrawing), LayerGroup::Drill),
        (BOARD_OUTLINE, LayerGroup::Mechanical),
    ] {
        assert_eq!(group_of(&rows, layer), Some(group), "{layer:?}");
    }

    let mut layers = board().layers;
    layers.insert(
        1,
        LayerDef {
            id: 1,
            name: "Inner1Copper".to_owned(),
            layer_type: "plane".to_owned(),
            display_name: String::new(),
        },
    );
    let inner = PcbLayer::Copper(CopperLayer::Inner(InnerCopper::new(1).unwrap()));
    assert_eq!(
        group_of(&layer_rows(&layers), inner),
        Some(LayerGroup::Plane)
    );
    assert_eq!(signal_layers(&layers), vec![TOP, BOTTOM]);
    assert_eq!(group_layers(&layers, LayerGroup::Signal), vec![TOP, BOTTOM]);
}

/// #2357: the strip and `+`/`-` follow the board's own layer list; a board
/// that lists no copper still has Top and Bottom.
#[test]
fn board_layers_keep_the_boards_order() {
    let mechanical = PcbLayer::Mechanical(MechanicalLayer::new(3).unwrap());
    assert_eq!(board_layers(&board().layers), vec![TOP, BOTTOM, mechanical]);
    let only_overlay = [LayerDef {
        id: 0,
        name: "TopSilkscreen".to_owned(),
        layer_type: String::new(),
        display_name: String::new(),
    }];
    assert_eq!(
        board_layers(&only_overlay),
        vec![
            TOP,
            BOTTOM,
            PcbLayer::Technical(TechnicalLayer::SilkscreenTop)
        ]
    );
}

/// #2357: a non-copper active layer (a tab like Top Overlay) dims no copper;
/// stepping walks every listed layer, hidden ones skipped.
#[test]
fn an_overlay_can_be_the_active_layer() {
    let board = board();
    let overlay = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);
    let mut view = PcbLayerView::default();
    view.set_active(overlay);
    assert_eq!(view.shown_active(), Some(overlay));
    assert_eq!(view.active_copper(), None);
    let snapshot = PcbSnapshot::from_board_with_view(&board, &view);
    assert!(
        snapshot.traces.iter().all(|t| !t.dimmed),
        "no copper dimmed"
    );

    let layers = [TOP, overlay, BOTTOM];
    view.set_active(TOP);
    view.step_active(&layers, true);
    assert_eq!(view.active(), overlay);
    view.toggle_visibility(overlay);
    view.set_active(TOP);
    view.step_active(&layers, true);
    assert_eq!(view.active(), BOTTOM, "the hidden overlay is skipped");
}

/// #2357/#2359: single-layer mode draws the active layer alone, with the
/// board outline and the vias that reach it; the eyes keep their state.
#[test]
fn single_layer_mode_draws_only_the_active_layer() {
    let board = board();
    let mut view = PcbLayerView::default();
    view.toggle_single_layer();
    assert!(view.single_layer());
    let snapshot = PcbSnapshot::from_board_with_view(&board, &view);
    let nets: Vec<u32> = snapshot.traces.iter().map(|t| t.net).collect();
    assert_eq!(nets, vec![3, 1], "the unknown layer's trace and Top's");
    assert_eq!(snapshot.vias.len(), 1, "the via reaches Top");
    assert!(view.is_visible(BOARD_OUTLINE));
    assert!(!view.is_visible(BOTTOM));
    assert!(!view.is_hidden(BOTTOM), "the eye is not touched");

    view.set_active(PcbLayer::Technical(TechnicalLayer::SilkscreenTop));
    let snapshot = PcbSnapshot::from_board_with_view(&board, &view);
    assert_eq!(snapshot.traces.len(), 1, "no copper but the unknown layer");
    assert!(snapshot.vias.is_empty(), "a via reaches no overlay");

    view.toggle_single_layer();
    let snapshot = PcbSnapshot::from_board_with_view(&board, &view);
    assert_eq!(snapshot.traces.len(), 3, "off again, everything is drawn");
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
    let copper: Vec<PcbLayer> = board_copper_layers(&board.layers)
        .into_iter()
        .map(PcbLayer::Copper)
        .collect();
    let mut view = PcbLayerView::default();
    view.toggle_visibility(TOP);
    view.settle_active(&copper);
    assert_eq!(view.active(), BOTTOM);
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
    let copper = [TOP, PcbLayer::Copper(inner), BOTTOM];
    let mut view = PcbLayerView::default();
    view.toggle_visibility(PcbLayer::Copper(inner));
    view.step_active(&copper, true);
    assert_eq!(view.active(), BOTTOM);
    view.step_active(&copper, false);
    assert_eq!(view.active(), TOP);
    view.toggle_visibility(BOTTOM);
    view.step_active(&copper, true);
    assert_eq!(view.active(), TOP, "the only shown layer stays");
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

#[test]
fn hiding_board_outline_removes_the_edge_overlay_and_showing_it_brings_it_back() {
    let mut board = board();
    board.outline = [(0.0, 0.0), (30.0, 0.0), (30.0, 20.0), (0.0, 20.0)]
        .into_iter()
        .map(|(x, y)| signex_types::pcb::Point { x, y })
        .collect();
    let theme = ResolvedTheme::builtin_default();
    let edge = theme.color(signex_gfx::style::ColorSlot::BoardEdge);
    let edge_lines = |view: &PcbLayerView| {
        let snapshot = PcbSnapshot::from_board_with_view(&board, view);
        let mut scene = Scene::default();
        super::super::PcbRenderer::build_scene(&snapshot, &theme, DirtyFlags::OVERLAY, &mut scene);
        scene
            .overlay_lines
            .iter()
            .filter(|l| l.color == edge)
            .count()
    };
    let mut view = PcbLayerView::default();
    assert_eq!(edge_lines(&view), 4, "the outline's four edges");
    view.toggle_visibility(BOARD_OUTLINE);
    assert!(!view.is_visible(BOARD_OUTLINE));
    assert_eq!(edge_lines(&view), 0, "a hidden Board Outline draws no edge");
    view.toggle_visibility(BOARD_OUTLINE);
    assert_eq!(edge_lines(&view), 4, "shown again, the edge is back");
}

#[test]
fn the_origin_marker_is_shown_by_default_and_toggles() {
    let mut view = PcbLayerView::default();
    assert!(view.origin_marker_visible());
    view.toggle_origin_marker();
    assert!(!view.origin_marker_visible());
    view.toggle_origin_marker();
    assert!(view.origin_marker_visible());
    view.set_origin_marker_visible(false);
    assert!(!view.origin_marker_visible());
}

/// #2379: the layer tab menu's Hide this layer and Show only this layer.
#[test]
fn hide_and_show_only_act_on_one_layer() {
    let board = board();
    let mut view = PcbLayerView::default();
    view.hide(BOTTOM);
    view.hide(BOTTOM);
    assert!(view.is_hidden(BOTTOM), "hiding twice keeps it hidden");

    let mechanical = PcbLayer::Mechanical(MechanicalLayer::new(3).unwrap());
    let layers: Vec<PcbLayer> = layer_rows(&board.layers)
        .into_iter()
        .map(|row| row.layer)
        .collect();
    view.show_only(&layers, BOTTOM);
    assert_eq!(view.active(), BOTTOM, "the shown layer is the active one");
    assert!(!view.is_hidden(BOTTOM));
    assert!(view.is_hidden(TOP) && view.is_hidden(mechanical));
    assert!(!view.is_hidden(BOARD_OUTLINE), "the outline stays drawn");
    assert!(
        view.multi_layer_visible(),
        "the Multi-Layer row is untouched"
    );

    view.show_all();
    assert!(!view.has_hidden());
}

/// #2394: a highlighted layer is drawn on top and every other layer dimmed,
/// whatever the active layer; highlighting it again ends it.
#[test]
fn a_highlighted_layer_dims_every_other_layer() {
    let board = board();
    let mut view = PcbLayerView::default();
    view.toggle_highlight(BOTTOM);
    assert_eq!(view.highlighted(), Some(BOTTOM));
    let snapshot = PcbSnapshot::from_board_with_view(&board, &view);
    let nets: Vec<(u32, bool)> = snapshot.traces.iter().map(|t| (t.net, t.dimmed)).collect();
    assert_eq!(nets, vec![(1, true), (3, true), (2, false)]);
    view.toggle_highlight(BOTTOM);
    assert_eq!(view.highlighted(), None);
}

/// #2394: seen from below, the bottom copper is drawn over the top copper.
#[test]
fn a_flipped_view_draws_the_bottom_copper_last() {
    let board = board();
    let mut view = PcbLayerView::default();
    // A non-copper active layer: no copper is dimmed or raised.
    view.set_active(PcbLayer::Technical(TechnicalLayer::SilkscreenTop));
    let order = |view: &PcbLayerView| -> Vec<u32> {
        PcbSnapshot::from_board_with_view(&board, view)
            .traces
            .iter()
            .map(|t| t.net)
            .collect()
    };
    assert_eq!(order(&view), vec![1, 2, 3], "source order from above");
    view.toggle_flipped();
    assert!(view.flipped());
    assert_eq!(order(&view), vec![3, 1, 2], "bottom copper last from below");
}

/// #2394: a layer set shows its layers alone and the strip lists only them;
/// Show all layers lists every layer again.
#[test]
fn a_layer_set_shows_its_layers_and_lists_them() {
    let board = board();
    let mut view = PcbLayerView::default();
    let listed = crate::pcb::listed_layers(&board.layers);
    let set = crate::pcb::ShownLayerSet {
        id: crate::pcb::LayerSetId::Builtin(crate::pcb::BuiltinLayerSet::Signal),
        layers: vec![TOP, BOTTOM],
    };
    view.show_set(&listed, set);
    assert!(!view.is_hidden(TOP) && !view.is_hidden(BOTTOM));
    let mechanical = PcbLayer::Mechanical(MechanicalLayer::new(3).unwrap());
    assert!(view.is_hidden(mechanical));
    assert!(!view.is_hidden(BOARD_OUTLINE), "the outline stays drawn");
    assert!(view.lists_in_strip(TOP) && !view.lists_in_strip(mechanical));
    view.show_all();
    assert!(view.layer_set().is_none() && view.lists_in_strip(mechanical));
}

#[test]
fn a_via_with_a_smaller_inner_size_draws_it_on_an_inner_layer() {
    let mut board = board();
    board.vias[0].layer_sizes = Some(signex_types::pcb::LayerSizes::TopMiddleBottom {
        top: 0.8,
        middle: 0.5,
        bottom: 0.8,
    });
    let mut view = PcbLayerView::default();
    view.set_active(CopperLayer::Inner(InnerCopper::new(1).unwrap()));
    let inner = PcbSnapshot::from_board_with_view(&board, &view);
    assert_eq!(inner.vias[0].diameter_mm, 0.5, "#2401: the inner ring");
    view.set_active(TOP);
    let top = PcbSnapshot::from_board_with_view(&board, &view);
    assert_eq!(top.vias[0].diameter_mm, 0.8, "the top ring");
}

/// The lines of `board` as `view` draws them.
fn lines_of(board: &PcbBoard, view: &PcbLayerView) -> (PcbSnapshot, Scene) {
    let snapshot = PcbSnapshot::from_board_with_view(board, view);
    let theme = ResolvedTheme::builtin_default();
    let mut scene = Scene::default();
    super::super::PcbRenderer::build_scene(&snapshot, &theme, DirtyFlags::LINES, &mut scene);
    (snapshot, scene)
}

/// #2363: hiding a kind of object drops it from the snapshot; the layer
/// stays shown and the other kinds stay.
#[test]
fn a_hidden_kind_of_object_is_not_drawn() {
    let board = board();
    let mut view = PcbLayerView::default();
    view.set_object_visible(BoardObject::Tracks, false);
    let snapshot = PcbSnapshot::from_board_with_view(&board, &view);
    assert!(snapshot.traces.is_empty());
    assert_eq!(snapshot.vias.len(), 1, "vias are another kind");
    view.set_object_visible(BoardObject::Tracks, true);
    view.set_object_visible(BoardObject::Vias, false);
    let snapshot = PcbSnapshot::from_board_with_view(&board, &view);
    assert_eq!(snapshot.traces.len(), 3);
    assert!(snapshot.vias.is_empty());
}

/// #2363: a colour the view gives a copper layer recolours its copper and
/// leaves the other layers in the palette colour.
#[test]
fn a_layer_colour_recolours_its_copper() {
    let board = board();
    let mut view = PcbLayerView::default();
    let (snapshot, before) = lines_of(&board, &view);
    let line = |snapshot: &PcbSnapshot, scene: &Scene, net: u32| {
        let i = snapshot.traces.iter().position(|t| t.net == net).unwrap();
        scene.lines[i].color
    };
    let top_before = line(&snapshot, &before, 1);
    let bottom_before = line(&snapshot, &before, 2);
    view.set_layer_colour(TOP, Some([255, 0, 255]));
    let (snapshot, after) = lines_of(&board, &view);
    let top = line(&snapshot, &after, 1);
    assert_eq!([top[0], top[1], top[2]], [1.0, 0.0, 1.0]);
    assert_eq!(top[3], top_before[3], "the palette's alpha is kept");
    assert_eq!(line(&snapshot, &after, 2), bottom_before);
    view.set_layer_colour(TOP, None);
    let (snapshot, reset) = lines_of(&board, &view);
    assert_eq!(line(&snapshot, &reset, 1), top_before);
}

/// #2363: a transparency for tracks fades every track by that much.
#[test]
fn a_transparency_fades_the_tracks() {
    let board = board();
    let mut view = PcbLayerView::default();
    let (_, before) = lines_of(&board, &view);
    view.set_object_transparency(BoardObject::Tracks, 50);
    let (_, after) = lines_of(&board, &view);
    for (a, b) in before.lines.iter().zip(&after.lines) {
        assert!((b.color[3] - a.color[3] * 0.5).abs() < 1e-6);
    }
}
