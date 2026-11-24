use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use super::*;
use crate::pcb::BoardEngine;

/// A board with a net-3 track along y = 0 on the top layer and a setup
/// whose via is 0.8 mm with a 0.4 mm drill.
fn board() -> PcbBoard {
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": 1.0, "trace_width": 0.3, "via_diameter": 0.8,
                  "via_drill": 0.4, "clearance": 0.2},
        "nets": [{"number": 3, "name": "GND"}],
        "segments": [{"uuid": Uuid::from_u128(2), "start": {"x": 0.0, "y": 0.0},
                      "end": {"x": 10.0, "y": 0.0}, "width": 0.5,
                      "layer": "TopCopper", "net": 3}]
    }))
    .unwrap()
}

#[test]
fn a_via_takes_the_setup_size_and_the_net_it_lands_on_in_one_step() {
    let mut engine = BoardEngine::new(board(), 1);
    let before = serde_json::to_string(engine.board()).unwrap();
    let on_track = Uuid::from_u128(0x10);
    let bare = Uuid::from_u128(0x11);
    let command = place_via(engine.board(), on_track, Point::new(5.0, 0.0));
    engine.execute(command).unwrap();
    let command = place_via(engine.board(), bare, Point::new(5.0, 5.0));
    engine.execute(command).unwrap();

    let via = |id| engine.board().vias.iter().find(|v| v.uuid == id).unwrap();
    assert_eq!((via(on_track).diameter, via(on_track).drill), (0.8, 0.4));
    assert_eq!(via(on_track).net, 3, "the via joins the track's net");
    assert_eq!(via(bare).net, 0, "bare board has no net");
    assert_eq!(via(bare).layers, ["TopCopper", "BottomCopper"]);

    engine.undo().unwrap();
    engine.undo().unwrap();
    assert_eq!(serde_json::to_string(engine.board()).unwrap(), before);
}

#[test]
fn a_board_without_setup_sizes_takes_the_pcb_defaults() {
    let board: PcbBoard = serde_json::from_value(serde_json::json!({"uuid": Uuid::nil()})).unwrap();
    let via = new_via(&board, Uuid::from_u128(5), Point::new(1.0, 1.0));
    assert_eq!(via.diameter, signex_types::pcb::PCB_DEFAULT_VIA_DIAMETER_MM);
    assert_eq!(via.drill, signex_types::pcb::PCB_DEFAULT_VIA_DRILL_MM);
}

#[test]
fn a_track_run_bends_by_the_corner_rule_on_the_net_it_starts_on() {
    use signex_types::corner::{CornerMode, CornerRule};
    use signex_types::pcb_layer::CopperLayer;

    let board = board();
    let from = Point::new(2.0, 0.0);
    let style = TrackStyle::starting_at(&board, from, CopperLayer::Top);
    assert_eq!(
        (style.width_mm, style.net),
        (0.3, 3),
        "setup width, track net"
    );
    let bottom = TrackStyle::starting_at(&board, from, CopperLayer::Bottom);
    assert_eq!(bottom.net, 0, "no copper under it on the bottom layer");

    let mut next = 0x40;
    let mut ids = || {
        next += 1;
        Uuid::from_u128(next)
    };
    let ortho = CornerRule::default();
    let tracks = track_run(from, Point::new(5.0, 4.0), ortho, style, &mut ids);
    let legs: Vec<_> = tracks.iter().map(|t| (t.start, t.end)).collect();
    assert_eq!(
        legs,
        [
            (from, Point::new(5.0, 0.0)),
            (Point::new(5.0, 0.0), Point::new(5.0, 4.0))
        ],
        "horizontal first, then vertical"
    );
    assert!(tracks.iter().all(|t| t.layer == "TopCopper" && t.net == 3));
    let free = CornerRule {
        mode: CornerMode::FreeAngle,
        is_flipped: false,
    };
    assert_eq!(
        track_run(from, Point::new(5.0, 4.0), free, style, &mut ids).len(),
        1
    );
    assert!(track_run(from, from, ortho, style, &mut ids).is_empty());
    assert!(place_route(Vec::new(), None).is_none());
}

#[test]
fn a_layer_change_adds_its_via_and_tracks_as_one_step() {
    use signex_types::corner::CornerRule;
    use signex_types::pcb_layer::CopperLayer;

    let mut engine = BoardEngine::new(board(), 1);
    let before = serde_json::to_string(engine.board()).unwrap();
    let at = Point::new(5.0, 0.0);
    let via = new_via(engine.board(), Uuid::from_u128(0x50), at);
    let style = TrackStyle {
        layer: CopperLayer::Bottom,
        width_mm: 0.3,
        net: via.net,
    };
    let mut n = 0x60;
    let tracks = track_run(
        at,
        Point::new(5.0, 3.0),
        CornerRule::default(),
        style,
        || {
            n += 1;
            Uuid::from_u128(n)
        },
    );
    engine
        .execute(place_route(tracks, Some(via)).unwrap())
        .unwrap();
    assert_eq!(engine.board().vias.len(), 1);
    assert_eq!(
        engine.board().segments.last().unwrap().layer,
        "BottomCopper"
    );
    assert_eq!(engine.board().segments.last().unwrap().net, 3);
    engine.undo().unwrap();
    assert_eq!(serde_json::to_string(engine.board()).unwrap(), before);
}

#[test]
fn a_copper_area_is_solid_copper_on_the_net_under_its_first_point() {
    use signex_types::pcb_layer::CopperLayer;

    let mut engine = BoardEngine::new(board(), 1);
    let before = serde_json::to_string(engine.board()).unwrap();
    let outline = rectangle(Point::new(1.0, 0.0), Point::new(4.0, 3.0));
    assert_eq!(outline.len(), 4);
    assert_eq!(outline[2], Point::new(4.0, 3.0));
    let id = Uuid::from_u128(0x70);
    let zone = new_copper_area(engine.board(), id, outline, CopperLayer::Top).unwrap();
    assert_eq!((zone.net, zone.net_name.as_str()), (3, "GND"));
    assert_eq!(
        (zone.layer.as_str(), zone.fill_type.as_str()),
        ("TopCopper", "solid")
    );
    engine.execute(place_zone(zone)).unwrap();
    assert_eq!(engine.board().zones.len(), 1);
    engine.undo().unwrap();
    assert_eq!(serde_json::to_string(engine.board()).unwrap(), before);

    let bottom = rectangle(Point::new(1.0, 0.0), Point::new(4.0, 3.0));
    let zone = new_copper_area(engine.board(), id, bottom, CopperLayer::Bottom).unwrap();
    assert_eq!(zone.net, 0, "no bottom copper under it");
    let flat = rectangle(Point::new(1.0, 1.0), Point::new(4.0, 1.0));
    assert!(new_copper_area(engine.board(), id, flat, CopperLayer::Top).is_none());
    let two = vec![Point::new(0.0, 0.0), Point::new(1.0, 1.0)];
    assert!(new_copper_area(engine.board(), id, two, CopperLayer::Top).is_none());
}

#[test]
fn a_keepout_is_drawn_on_copper_or_a_keep_out_layer_only() {
    use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};

    let outline = rectangle(Point::new(1.0, 1.0), Point::new(4.0, 3.0));
    let id = Uuid::from_u128(0x80);
    let top = PcbLayer::Technical(TechnicalLayer::KeepoutTop);
    let zone = new_keepout(id, outline.clone(), top).unwrap();
    assert_eq!((zone.layer.as_str(), zone.net), ("TopKeepout", 0));
    let keepout = zone.as_keepout().expect("DRC reads it as a keepout");
    assert_eq!(keepout.layers, [PcbLayer::Copper(CopperLayer::Top)]);
    let inner = PcbLayer::Copper(CopperLayer::Bottom);
    assert!(new_keepout(id, outline.clone(), inner).is_some());
    let overlay = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);
    assert!(new_keepout(id, outline, overlay).is_none());
}

#[test]
fn a_pour_is_a_pour_zone_on_the_net_under_its_first_point_in_one_step() {
    use signex_types::pcb_layer::CopperLayer;

    let mut engine = BoardEngine::new(board(), 1);
    let before = serde_json::to_string(engine.board()).unwrap();
    let outline = rectangle(Point::new(1.0, 0.0), Point::new(4.0, 3.0));
    let id = Uuid::from_u128(0x90);
    let zone = new_pour(engine.board(), id, outline, CopperLayer::Top).unwrap();
    assert!(zone.is_pour());
    assert_eq!((zone.net, zone.layer.as_str()), (3, "TopCopper"));
    assert_eq!(zone.clearance, 0.0, "follows the board's clearance");
    assert_eq!(zone.pour_clearance_mm(engine.board()), 0.2);
    engine.execute(place_zone(zone)).unwrap();
    assert_eq!(engine.board().zones.len(), 1);
    engine.undo().unwrap();
    assert_eq!(serde_json::to_string(engine.board()).unwrap(), before);

    let flat = rectangle(Point::new(1.0, 1.0), Point::new(4.0, 1.0));
    assert!(new_pour(engine.board(), id, flat, CopperLayer::Top).is_none());
}

#[test]
fn a_board_string_lands_on_its_layer_at_the_default_size_in_one_step() {
    use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};

    let mut engine = BoardEngine::new(board(), 1);
    let before = serde_json::to_string(engine.board()).unwrap();
    let overlay = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);
    let id = Uuid::from_u128(0x90);
    let text = new_text(id, Point::new(2.0, 3.0), overlay, "J1".into()).unwrap();
    assert_eq!(
        (text.layer.as_str(), text.text.as_str()),
        ("TopSilkscreen", "J1")
    );
    assert_eq!(text.font_size, signex_types::pcb::PCB_DEFAULT_TEXT_SIZE_MM);
    assert_eq!(text.rotation, 0.0);
    engine.execute(place_text(text.clone())).unwrap();
    assert_eq!(engine.board().texts, [text]);
    engine.undo().unwrap();
    assert_eq!(serde_json::to_string(engine.board()).unwrap(), before);
}

/// Was `a_board_string_lands_on_an_overlay_mechanical_or_documentation_layer_only`,
/// with copper refused; a string lands on copper too since Caner's
/// 2026-11-24 decision (#2883).
#[test]
fn a_board_string_lands_on_a_copper_overlay_mechanical_or_documentation_layer_only() {
    use signex_types::pcb_layer::{CopperLayer, MechanicalLayer, PcbLayer, TechnicalLayer};

    let at = Point::new(2.0, 3.0);
    let id = Uuid::from_u128(0x91);
    let bottom = PcbLayer::Technical(TechnicalLayer::SilkscreenBottom);
    let text = new_text(id, at, bottom, "J1".into()).unwrap();
    assert_eq!(text.layer, "BottomSilkscreen");
    let mech = PcbLayer::Mechanical(MechanicalLayer::new(2).unwrap());
    assert_eq!(
        new_text(id, at, mech, "J1".into()).unwrap().layer,
        "Mechanical2"
    );
    let dims = PcbLayer::Technical(TechnicalLayer::Dimensions);
    assert!(new_text(id, at, dims, "J1".into()).is_some(), "#2373");
    for (layer, token) in [
        (PcbLayer::Copper(CopperLayer::Top), "TopCopper"),
        (PcbLayer::Copper(CopperLayer::Bottom), "BottomCopper"),
    ] {
        assert_eq!(new_text(id, at, layer, "J1".into()).unwrap().layer, token);
    }
    for layer in [
        PcbLayer::Technical(TechnicalLayer::EdgeCuts),
        PcbLayer::Technical(TechnicalLayer::SolderMaskTop),
    ] {
        assert!(new_text(id, at, layer, "J1".into()).is_none(), "{layer:?}");
    }
}

/// #2400: a run on a net whose class sets a track width is drawn that wide;
/// a net in no class, or a class without a width, keeps the board's.
#[test]
fn a_track_takes_the_width_of_its_net_class() {
    use signex_types::net_class::NetClass;
    use signex_types::pcb_layer::CopperLayer;

    let board = board();
    let start = Point::new(2.0, 0.0);
    let style = |classes: &[NetClass]| {
        TrackStyle::starting_at(&board, start, CopperLayer::Top).with_class_width(&board, classes)
    };
    let power = NetClass {
        nets: vec!["GND".into()],
        trace_width: 0.6,
        ..NetClass::named("Power")
    };
    assert_eq!(style(std::slice::from_ref(&power)).width_mm, 0.6);
    assert_eq!(
        class_track_width(&board, 3, std::slice::from_ref(&power)),
        Some(0.6)
    );
    let unset = NetClass {
        trace_width: 0.0,
        ..power.clone()
    };
    assert_eq!(style(&[unset]).width_mm, 0.3, "no width: the board's");
    let other = NetClass {
        nets: vec!["VCC".into()],
        ..power.clone()
    };
    assert_eq!(style(&[other]).width_mm, 0.3, "GND is in no class");
    assert_eq!(class_track_width(&board, 0, &[power]), None, "no net");
}

#[test]
fn a_pour_takes_the_net_of_a_later_corner_when_the_first_is_off_copper() {
    use signex_types::pcb_layer::CopperLayer;

    let engine = BoardEngine::new(board(), 1);
    // (1, 5) and (4, 5) are off copper; (4, 0) lies on GND's track.
    let outline = vec![
        Point::new(1.0, 5.0),
        Point::new(4.0, 5.0),
        Point::new(4.0, 0.0),
    ];
    let id = Uuid::from_u128(0x91);
    let zone = new_pour(engine.board(), id, outline, CopperLayer::Top).unwrap();
    assert_eq!(zone.net, 3);
}
