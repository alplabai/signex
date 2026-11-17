//! #2393 — an armed board placement tool shows what it places next in
//! Properties; Tab pauses placing to edit it, Enter or Tab goes on with the
//! values, Esc puts the previous ones back. Driven through `Signex::update`.

use std::path::Path;

use crate::app::{Message, Signex};
use crate::dock::DockMessage;
use crate::panels::pcb_primitive_properties::{PcbPrimitiveFacts, PrimitiveField, PrimitiveTarget};
use crate::panels::{PanelMsg, PcbPrimitiveMsg};

use super::tests::{app_with_board, board_of, click, escape, run, undo};

pub(super) fn form(app: &Signex) -> PcbPrimitiveFacts {
    app.document_state
        .panel_ctx
        .pcb_board
        .as_ref()
        .and_then(|b| b.primitive.clone())
        .expect("the placement form")
}

pub(super) fn panel(app: &mut Signex, msg: PcbPrimitiveMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::PcbPrimitive(
        msg,
    ))));
}

pub(super) fn typed(app: &mut Signex, path: &Path, field: PrimitiveField, text: &str) {
    panel(
        app,
        PcbPrimitiveMsg::FieldTyped {
            target: PrimitiveTarget::placement(path.to_path_buf()),
            field,
            text: text.into(),
        },
    );
}

pub(super) fn enter(app: &mut Signex, path: &Path, field: PrimitiveField) {
    panel(
        app,
        PcbPrimitiveMsg::FieldSubmitted {
            target: PrimitiveTarget::placement(path.to_path_buf()),
            field,
        },
    );
}

pub(super) fn tab(app: &mut Signex) {
    run(app, "placement_properties");
}

fn diameters(app: &Signex, path: &Path) -> Vec<f64> {
    board_of(app, path)
        .vias
        .iter()
        .map(|v| v.diameter)
        .collect()
}

#[test]
fn arming_via_shows_the_via_it_places_and_tab_edits_its_diameter() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    run(&mut app, "place_board_via");
    let PcbPrimitiveFacts::Via(via) = form(&app) else {
        panic!("arming Via shows the via form");
    };
    assert!(via.common.target.is_placement());
    assert_eq!(
        (via.diameter, via.hole),
        (Some(0.8), Some(0.4)),
        "the board's via size"
    );

    tab(&mut app);
    assert!(app.is_board_tool_paused(), "Tab pauses placing");
    click(&mut app, 7.4, 6.2);
    assert!(
        diameters(&app, &path).is_empty(),
        "a paused tool places nothing"
    );

    typed(&mut app, &path, PrimitiveField::Diameter, "1.2");
    enter(&mut app, &path, PrimitiveField::Diameter);
    assert!(!app.is_board_tool_paused(), "Enter goes on placing");
    let PcbPrimitiveFacts::Via(via) = form(&app) else {
        panic!("the via form stays");
    };
    assert_eq!(via.diameter, Some(1.2));

    click(&mut app, 4.2, 0.3);
    assert_eq!(
        diameters(&app, &path),
        [1.2],
        "the via takes the form's value"
    );
    click(&mut app, 7.4, 6.2);
    assert_eq!(
        diameters(&app, &path),
        [1.2, 1.2],
        "and so does the next one"
    );
    assert_eq!(board_of(&app, &path).vias[0].net, 3, "on the net under it");
    undo(&mut app);
    assert_eq!(diameters(&app, &path), [1.2], "one undo step per via");

    escape(&mut app);
    run(&mut app, "place_board_via");
    let PcbPrimitiveFacts::Via(via) = form(&app) else {
        panic!("the via form");
    };
    assert_eq!(via.diameter, Some(1.2), "the value stays the tool's");
}

#[test]
fn esc_during_the_pause_keeps_the_previous_values_and_goes_on_placing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    run(&mut app, "place_board_via");
    tab(&mut app);
    typed(&mut app, &path, PrimitiveField::Diameter, "1.0");
    enter(&mut app, &path, PrimitiveField::Diameter);

    tab(&mut app);
    panel(
        &mut app,
        PcbPrimitiveMsg::NetPicked {
            target: PrimitiveTarget::placement(path.clone()),
            net: 3,
        },
    );
    typed(&mut app, &path, PrimitiveField::Diameter, "2.0");
    escape(&mut app);
    assert!(!app.is_board_tool_paused(), "Esc ends the pause");
    assert!(
        app.interaction_state.pcb_tool.is_some(),
        "and keeps the tool armed"
    );

    click(&mut app, 7.4, 6.2);
    let vias = &board_of(&app, &path).vias;
    assert_eq!(vias.len(), 1);
    assert_eq!(vias[0].diameter, 1.0, "the value before the pause");
    assert_eq!(vias[0].net, 0, "the net picked in the pause is undone");
}

#[test]
fn tab_again_applies_the_typed_value_and_goes_on_placing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    run(&mut app, "place_board_via");
    tab(&mut app);
    typed(&mut app, &path, PrimitiveField::HoleSize, "0.3");
    tab(&mut app);
    assert!(!app.is_board_tool_paused());
    click(&mut app, 7.4, 6.2);
    assert_eq!(board_of(&app, &path).vias[0].drill, 0.3);
}

#[test]
fn a_size_that_is_not_above_zero_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    run(&mut app, "place_board_via");
    tab(&mut app);
    typed(&mut app, &path, PrimitiveField::Diameter, "-1");
    enter(&mut app, &path, PrimitiveField::Diameter);
    click(&mut app, 7.4, 6.2);
    assert_eq!(diameters(&app, &path), [0.8]);
}

#[test]
fn the_track_tool_shows_its_width_and_places_with_the_edited_one() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    run(&mut app, "place_board_track");
    let PcbPrimitiveFacts::Track(track) = form(&app) else {
        panic!("arming Track shows the track form");
    };
    assert_eq!(track.width, Some(0.3), "the board's track width");

    tab(&mut app);
    typed(&mut app, &path, PrimitiveField::Width, "0.6");
    enter(&mut app, &path, PrimitiveField::Width);
    click(&mut app, 2.0, 5.0);
    click(&mut app, 6.0, 5.0);
    let added = &board_of(&app, &path).segments[1];
    assert_eq!(added.width, 0.6);
}
