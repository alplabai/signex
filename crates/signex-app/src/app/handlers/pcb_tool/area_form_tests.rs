//! #2393 — the area tools show the area they place next in Properties, the
//! same panel a placed area shows when selected: Tab pauses to edit it,
//! Enter or Tab places with the values. Driven through `Signex::update`.

use std::path::Path;

use signex_types::format::pcb_layer_token;
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};

use super::form_tests::{enter, form, panel, tab, typed};
use super::tests::{app_with_board, board_of, click, escape, hover, right_click, run, undo};
use crate::app::Signex;
use crate::panels::PcbPrimitiveMsg;
use crate::panels::pcb_primitive_properties::{
    PcbPrimitiveFacts, PrimitiveField, PrimitiveTarget, ZoneFacts, ZoneKind,
};

fn zone_form(app: &Signex) -> ZoneFacts {
    match form(app) {
        PcbPrimitiveFacts::Zone(f) => f,
        other => panic!("the area form, not {other:?}"),
    }
}

fn placement(path: &Path) -> PrimitiveTarget {
    PrimitiveTarget::placement(path.to_path_buf())
}

fn active_layer(app: &Signex, path: &Path) -> PcbLayer {
    app.pcb_layer_view(path).active()
}

/// A square of corners from (20, 20), closed with a right-click.
fn draw_square(app: &mut Signex) {
    for (x, y) in [(20.0, 20.0), (24.0, 20.0), (24.0, 24.0), (20.0, 24.0)] {
        click(app, x, y);
    }
    right_click(app);
}

#[test]
fn a_polygon_pour_takes_the_clearance_set_in_its_form() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    run(&mut app, "place_board_polygon_pour");
    let f = zone_form(&app);
    assert_eq!(f.kind, Some(ZoneKind::Pour));
    assert!(f.is_pour && f.is_copper);
    assert!(f.common.target.is_placement());
    assert_eq!(f.clearance, Some(0.0), "0 follows the board's clearance");

    tab(&mut app);
    assert!(app.is_board_tool_paused());
    typed(&mut app, &path, PrimitiveField::Clearance, "0.4");
    enter(&mut app, &path, PrimitiveField::Clearance);
    assert!(!app.is_board_tool_paused(), "Enter goes on placing");
    assert_eq!(zone_form(&app).clearance, Some(0.4));

    draw_square(&mut app);
    let zones = &board_of(&app, &path).zones;
    assert_eq!(zones.len(), 1, "one area, one undo step");
    assert!(zones[0].is_pour());
    assert_eq!(zones[0].clearance, 0.4);
    draw_square(&mut app);
    assert_eq!(
        board_of(&app, &path).zones[1].clearance,
        0.4,
        "the value stays"
    );
    undo(&mut app);
    assert_eq!(board_of(&app, &path).zones.len(), 1);
}

#[test]
fn a_fill_takes_the_net_picked_in_its_form() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    run(&mut app, "place_board_fill");
    // Off the GND track, which runs from the origin: an area takes the net
    // under its first corner (#2216).
    hover(&mut app, 20.0, 20.0);
    let f = zone_form(&app);
    assert_eq!(f.kind, Some(ZoneKind::Fill));
    assert_eq!(
        f.common.net_info.net,
        Some(0),
        "bare board under the pointer"
    );

    tab(&mut app);
    panel(
        &mut app,
        PcbPrimitiveMsg::NetPicked {
            target: placement(&path),
            net: 3,
        },
    );
    assert_eq!(zone_form(&app).common.net_info.net, Some(3));
    tab(&mut app);
    assert!(!app.is_board_tool_paused(), "Tab goes on placing");

    click(&mut app, 20.0, 20.0);
    click(&mut app, 24.0, 23.0);
    let zone = &board_of(&app, &path).zones[0];
    assert_eq!((zone.net, zone.net_name.as_str()), (3, "GND"));
}

#[test]
fn a_keepout_form_moves_the_drawing_to_a_keep_out_layer_and_esc_puts_it_back() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    run(&mut app, "place_board_keepout");
    let f = zone_form(&app);
    assert_eq!(f.kind, Some(ZoneKind::Keepout));
    assert!(!f.is_copper, "a keepout has no net");
    let keep_out = PcbLayer::Technical(TechnicalLayer::KeepoutTop);
    assert!(f.layers.iter().any(|l| l.layer == keep_out));
    let top = PcbLayer::Copper(CopperLayer::Top);
    assert_eq!(active_layer(&app, &path), top);

    tab(&mut app);
    let pick = PcbPrimitiveMsg::AreaLayerPicked {
        target: placement(&path),
        layer: keep_out,
    };
    panel(&mut app, pick.clone());
    assert_eq!(active_layer(&app, &path), keep_out);
    escape(&mut app);
    assert_eq!(active_layer(&app, &path), top, "Esc puts the layer back");
    assert!(app.interaction_state.pcb_tool.is_some(), "still placing");

    tab(&mut app);
    panel(&mut app, pick);
    tab(&mut app);
    draw_square(&mut app);
    let zone = &board_of(&app, &path).zones[0];
    assert_eq!(zone.layer, pcb_layer_token(keep_out));
}

#[test]
fn a_copper_area_refuses_a_keep_out_layer() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    run(&mut app, "place_board_region");
    assert_eq!(zone_form(&app).kind, Some(ZoneKind::Region));
    panel(
        &mut app,
        PcbPrimitiveMsg::AreaLayerPicked {
            target: placement(&path),
            layer: PcbLayer::Technical(TechnicalLayer::KeepoutTop),
        },
    );
    assert_eq!(
        active_layer(&app, &path),
        PcbLayer::Copper(CopperLayer::Top)
    );
}

#[test]
fn a_selected_fill_shows_the_area_panel_and_its_edits_are_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    run(&mut app, "place_board_fill");
    click(&mut app, 20.0, 20.0);
    click(&mut app, 24.0, 23.0);
    escape(&mut app);
    assert!(app.interaction_state.pcb_tool.is_none());

    click(&mut app, 22.0, 21.0);
    let f = zone_form(&app);
    assert_eq!(f.kind, Some(ZoneKind::Fill));
    assert!(!f.common.target.is_placement(), "the placed fill");
    panel(
        &mut app,
        PcbPrimitiveMsg::NetPicked {
            target: f.common.target.clone(),
            net: 3,
        },
    );
    assert_eq!(board_of(&app, &path).zones[0].net, 3);
    undo(&mut app);
    assert_eq!(board_of(&app, &path).zones[0].net, 0, "one undo step");
}
