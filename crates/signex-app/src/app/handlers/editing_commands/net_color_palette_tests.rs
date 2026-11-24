//! #2528 — the F5 palette's rows (coloured first, filtered) and the custom
//! picker's Hex field.

use super::net_color_driver::{activate, add_sheet_with, color, colour_of, paint};
use super::net_color_palette::{hex_of, net_label_names, palette_rows, parse_hex};
use crate::app::state::NetColorsCleared;
use crate::app::{Message, NetColorMsg, Signex};
use signex_types::schematic::{Label, LabelType, Point, SchematicSheet, Wire};
use std::path::{Path, PathBuf};

/// A wire per net, each with its label at the wire's left end, and one
/// label on no wire.
fn sheet_with_nets(nets: &[&str]) -> SchematicSheet {
    let mut sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    for (i, net) in nets.iter().enumerate() {
        let y = 10.0 * i as f64;
        sheet.wires.push(Wire {
            uuid: uuid::Uuid::now_v7(),
            start: Point::new(0.0, y),
            end: Point::new(10.0, y),
            ..Default::default()
        });
        sheet
            .labels
            .push(Label::new(*net, Point::new(0.0, y), LabelType::Net));
    }
    sheet.labels.push(Label::new(
        "FLOATING",
        Point::new(50.0, 50.0),
        LabelType::Global,
    ));
    sheet
}

fn wire_of_net(sheet: &SchematicSheet, index: usize) -> uuid::Uuid {
    sheet.wires[index].uuid
}

fn app_with_nets(nets: &[&str]) -> (Signex, PathBuf) {
    let (mut app, _task) = Signex::new();
    let path = PathBuf::from("palette.snxsch");
    add_sheet_with(&mut app, &path, sheet_with_nets(nets));
    (app, path)
}

#[test]
fn coloured_nets_are_listed_first_then_by_name() {
    let mut sheet = sheet_with_nets(&["A", "B", "C"]);
    let c_wire = wire_of_net(&sheet, 2);
    sheet.net_colors.insert(c_wire, color(200));
    let rows = palette_rows(&sheet, "");
    let names: Vec<&str> = rows.iter().map(|r| r.net.as_str()).collect();
    assert_eq!(names, ["C", "A", "B", "FLOATING"]);
    assert_eq!(rows[0].color, Some(color(200)));
    assert!(rows[1..].iter().all(|r| r.color.is_none()));
}

#[test]
fn the_filter_narrows_the_rows_in_any_case() {
    let sheet = sheet_with_nets(&["VOUT_3V3", "VIN", "GND"]);
    let names = |filter: &str| -> Vec<String> {
        palette_rows(&sheet, filter)
            .into_iter()
            .map(|r| r.net)
            .collect()
    };
    assert_eq!(names("v"), ["VIN", "VOUT_3V3"]);
    assert_eq!(names(" gnd "), ["GND"]);
    assert!(names("nothing").is_empty());
    assert_eq!(names("").len(), 4, "an empty filter lists every net label");
}

#[test]
fn a_name_on_several_labels_is_listed_once() {
    let mut sheet = sheet_with_nets(&["A"]);
    sheet
        .labels
        .push(Label::new("A", Point::new(10.0, 0.0), LabelType::Net));
    assert_eq!(net_label_names(&sheet), ["A", "FLOATING"]);
}

#[test]
fn hex_round_trips_to_rgb() {
    let c = parse_hex("#14B8A6").expect("a valid code");
    assert_eq!(
        (
            (c.r * 255.0).round() as u8,
            (c.g * 255.0).round() as u8,
            (c.b * 255.0).round() as u8
        ),
        (0x14, 0xB8, 0xA6)
    );
    assert_eq!(hex_of(c), "#14B8A6");
    assert_eq!(
        parse_hex(" 0ea5e9 ").map(hex_of).as_deref(),
        Some("#0EA5E9")
    );
}

#[test]
fn invalid_hex_is_rejected_without_a_panic() {
    for bad in [
        "", "#", "#12345", "#1234567", "#GG0000", "+12345", "#+1234", "ÄÄÄ", "#ÿÿÿ",
    ] {
        assert_eq!(parse_hex(bad), None, "{bad:?}");
    }
}

/// #2770: Cancel discards the pick, so the next open does not start from it.
#[test]
fn cancel_puts_the_draft_back_as_it_was_before_the_open() {
    let (mut app, _path) = app_with_nets(&["A", "B"]);
    let before = hex_of(app.ui_state.net_color_custom.draft);
    let send = |app: &mut Signex, msg: NetColorMsg| {
        let _ = app.update(Message::NetColor(msg));
    };
    send(&mut app, NetColorMsg::CustomOpenedFor("A".into()));
    send(&mut app, NetColorMsg::CustomHexChanged("#112233".into()));
    send(&mut app, NetColorMsg::CustomShow(false));
    assert_eq!(hex_of(app.ui_state.net_color_custom.draft), before);

    send(&mut app, NetColorMsg::CustomOpenedFor("B".into()));
    assert_eq!(hex_of(app.ui_state.net_color_custom.draft), before);
}

/// Typing in Hex moves the draft only when the code parses, and the RGB
/// fields follow; an RGB edit hands the field back to the draft.
#[test]
fn hex_and_rgb_stay_in_sync() {
    let (mut app, _path) = app_with_nets(&["A"]);
    let send = |app: &mut Signex, msg: NetColorMsg| {
        let _ = app.update(Message::NetColor(msg));
    };
    send(&mut app, NetColorMsg::CustomHexChanged("#14B8A6".into()));
    assert_eq!(hex_of(app.ui_state.net_color_custom.draft), "#14B8A6");

    send(&mut app, NetColorMsg::CustomHexChanged("#14B8".into()));
    assert_eq!(
        hex_of(app.ui_state.net_color_custom.draft),
        "#14B8A6",
        "a half-typed code keeps the last colour"
    );
    assert_eq!(app.ui_state.net_color_custom.hex.as_deref(), Some("#14B8"));

    send(
        &mut app,
        NetColorMsg::CustomChannel(crate::app::contracts::Channel::R, "255".into()),
    );
    assert_eq!(app.ui_state.net_color_custom.hex, None);
    assert_eq!(hex_of(app.ui_state.net_color_custom.draft), "#FFB8A6");
}

/// A chooser colour colours the net's wires on the sheet, and Clear in the
/// chooser takes it off; the chooser closes either way.
#[test]
fn the_chooser_colours_and_clears_a_net() {
    let (mut app, path) = app_with_nets(&["A", "B"]);
    let wire = app.document_state.engines[&path].document().wires[0].uuid;
    let _ = app.update(Message::NetColor(NetColorMsg::RowOpened("A".into())));
    assert_eq!(app.ui_state.net_color_palette.chooser.as_deref(), Some("A"));
    let _ = app.update(Message::NetColor(NetColorMsg::Set {
        net: "A".into(),
        color: Some(color(120)),
    }));
    assert_eq!(colour_of(&app, wire), Some(color(120)));
    assert_eq!(app.ui_state.net_color_palette.chooser, None);
    assert_eq!(
        app.palette_rows()[0].net,
        "A",
        "the coloured net comes first"
    );

    let _ = app.update(Message::NetColor(NetColorMsg::Set {
        net: "A".into(),
        color: None,
    }));
    assert_eq!(colour_of(&app, wire), None);
}

/// Custom… from a row opens the picker for that net, seeded with its
/// colour; Use Color colours that net instead of arming the pen.
#[test]
fn custom_from_a_row_colours_that_net() {
    let (mut app, path) = app_with_nets(&["A"]);
    let wire = app.document_state.engines[&path].document().wires[0].uuid;
    let _ = app.update(Message::NetColor(NetColorMsg::CustomOpenedFor("A".into())));
    let custom = &app.ui_state.net_color_custom;
    assert!(custom.show);
    assert_eq!(custom.net.as_deref(), Some("A"));

    let pick = iced::Color::from_rgb8(0x14, 0xB8, 0xA6);
    let _ = app.update(Message::NetColor(NetColorMsg::CustomSubmit(pick)));
    assert_eq!(
        colour_of(&app, wire),
        Some(signex_types::theme::Color::new(0x14, 0xB8, 0xA6, 255))
    );
    assert_eq!(
        app.ui_state.pending_net_color, None,
        "the pen stays unarmed"
    );
    assert_eq!(app.ui_state.net_color_custom.net, None);
}

/// Custom… opened on one sheet, Use Color pressed on another that has a net
/// of the same name: refused, and neither sheet changes.
#[test]
fn custom_refuses_when_the_active_sheet_changed() {
    let (mut app, first) = app_with_nets(&["A"]);
    let first_wire = app.document_state.engines[&first].document().wires[0].uuid;
    let _ = app.update(Message::NetColor(NetColorMsg::CustomOpenedFor("A".into())));
    assert_eq!(app.ui_state.net_color_custom.sheet.as_ref(), Some(&first));

    let second = PathBuf::from("second.snxsch");
    add_sheet_with(&mut app, &second, sheet_with_nets(&["A"]));
    let second_wire = app.document_state.engines[&second].document().wires[0].uuid;
    activate(&mut app, &second);

    let pick = iced::Color::from_rgb8(0x14, 0xB8, 0xA6);
    let _ = app.update(Message::NetColor(NetColorMsg::CustomSubmit(pick)));
    assert_eq!(colour_of(&app, second_wire), None, "the other sheet's A");
    activate(&mut app, &first);
    assert_eq!(colour_of(&app, first_wire), None, "nothing applied late");
    assert!(app.ui_state.net_colors.is_empty());
    assert_eq!(app.ui_state.net_color_custom.net, None);
    assert_eq!(app.ui_state.net_color_custom.sheet, None);
}

/// A palette colour refused by a read-only sheet leaves the per-name
/// record alone, so the Active Bar offers no clear for it.
#[test]
fn a_refused_palette_colour_is_not_recorded() {
    let (mut app, _task) = Signex::new();
    let path = PathBuf::from("read-only.snxsch");
    let mut sheet = sheet_with_nets(&["A"]);
    sheet.unknown.newer_minor_source = Some("snxsch/9.9".into());
    add_sheet_with(&mut app, &path, sheet);
    let wire = app.document_state.engines[&path].document().wires[0].uuid;

    let _ = app.update(Message::NetColor(NetColorMsg::Set {
        net: "A".into(),
        color: Some(color(120)),
    }));
    assert_eq!(colour_of(&app, wire), None, "the sheet refused");
    assert!(app.ui_state.net_colors.is_empty(), "nothing recorded");
}

/// A net with no wire under its label is no edit and is not recorded.
#[test]
fn an_empty_palette_colour_is_not_recorded() {
    let (mut app, _path) = app_with_nets(&["A"]);
    let _ = app.update(Message::NetColor(NetColorMsg::Set {
        net: "FLOATING".into(),
        color: Some(color(120)),
    }));
    assert!(app.ui_state.net_colors.is_empty());
}

/// An applied palette colour is recorded under its net name.
#[test]
fn an_applied_palette_colour_is_recorded() {
    let (mut app, _path) = app_with_nets(&["A"]);
    let _ = app.update(Message::NetColor(NetColorMsg::Set {
        net: "A".into(),
        color: Some(color(120)),
    }));
    assert_eq!(app.ui_state.net_colors.get("A"), Some(&color(120)));
}

// Mockup row 71: Clear all colours and a chooser's Clear show a toast with
// what was cleared and Undo.

fn wires_of(app: &Signex, path: &Path) -> Vec<uuid::Uuid> {
    let sheet = app.document_state.engines[path].document();
    sheet.wires.iter().map(|w| w.uuid).collect()
}

fn send(app: &mut Signex, msg: NetColorMsg) {
    let _ = app.update(Message::NetColor(msg));
}

fn cleared(app: &Signex) -> Option<NetColorsCleared> {
    let toast = app.ui_state.net_color_palette.toast.as_ref();
    toast.map(|t| t.cleared.clone())
}

fn clear_net(app: &mut Signex, net: &str) {
    send(
        app,
        NetColorMsg::Set {
            net: net.into(),
            color: None,
        },
    );
}

#[test]
fn clear_all_counts_the_nets_and_its_undo_brings_them_back() {
    let (mut app, path) = app_with_nets(&["A", "B", "C"]);
    let wires = wires_of(&app, &path);
    paint(&mut app, &[wires[0]], color(10));
    paint(&mut app, &[wires[1]], color(20));

    send(&mut app, NetColorMsg::ClearedAll);
    assert_eq!(cleared(&app), Some(NetColorsCleared::Nets(2)));
    let toast = app.ui_state.net_color_palette.toast.clone();
    assert_eq!(toast.map(|t| t.sheet), Some(path));
    assert!(app.net_color_toast_can_undo());
    assert_eq!(colour_of(&app, wires[0]), None);

    send(&mut app, NetColorMsg::ToastUndone);
    assert_eq!(colour_of(&app, wires[0]), Some(color(10)));
    assert_eq!(colour_of(&app, wires[1]), Some(color(20)));
    assert_eq!(cleared(&app), None, "the toast goes with its Undo");
}

#[test]
fn a_chooser_clear_names_the_net_and_its_undo_brings_it_back() {
    let (mut app, path) = app_with_nets(&["A", "B"]);
    let wires = wires_of(&app, &path);
    paint(&mut app, &[wires[0]], color(10));

    clear_net(&mut app, "A");
    assert_eq!(colour_of(&app, wires[0]), None);
    assert_eq!(cleared(&app), Some(NetColorsCleared::Net("A".into())));

    send(&mut app, NetColorMsg::ToastUndone);
    assert_eq!(colour_of(&app, wires[0]), Some(color(10)));
}

#[test]
fn clear_all_of_unlabelled_nets_says_the_sheet_was_cleared() {
    let (mut app, _task) = Signex::new();
    let path = PathBuf::from("unlabelled.snxsch");
    let mut sheet = sheet_with_nets(&["A"]);
    let unlabelled = uuid::Uuid::now_v7();
    sheet.wires.push(Wire {
        uuid: unlabelled,
        start: Point::new(0.0, 30.0),
        end: Point::new(10.0, 30.0),
        ..Default::default()
    });
    add_sheet_with(&mut app, &path, sheet);
    paint(&mut app, &[unlabelled], color(30));

    send(&mut app, NetColorMsg::ClearedAll);
    assert_eq!(colour_of(&app, unlabelled), None);
    assert_eq!(cleared(&app), Some(NetColorsCleared::Unlabelled));
}

#[test]
fn nothing_cleared_shows_no_toast() {
    let (mut app, _path) = app_with_nets(&["A"]);
    send(&mut app, NetColorMsg::ClearedAll);
    assert_eq!(cleared(&app), None, "a clean sheet");
    clear_net(&mut app, "A");
    assert_eq!(cleared(&app), None, "a net with no colour");
    send(
        &mut app,
        NetColorMsg::Set {
            net: "A".into(),
            color: Some(color(40)),
        },
    );
    assert_eq!(cleared(&app), None, "a colour pick is no clear");
}

/// The toast's Undo takes back its clear only: after a newer edit it is
/// greyed, and a press undoes nothing.
#[test]
fn the_toast_undo_leaves_a_newer_edit_alone() {
    let (mut app, path) = app_with_nets(&["A", "B"]);
    let wires = wires_of(&app, &path);
    paint(&mut app, &[wires[0]], color(10));
    send(&mut app, NetColorMsg::ClearedAll);
    paint(&mut app, &[wires[1]], color(50));
    assert!(!app.net_color_toast_can_undo());

    send(&mut app, NetColorMsg::ToastUndone);
    assert_eq!(colour_of(&app, wires[1]), Some(color(50)));
    assert_eq!(colour_of(&app, wires[0]), None);
    assert_eq!(cleared(&app), None);
}

/// An older toast's timeout leaves the newer toast up.
#[test]
fn only_the_shown_toasts_timeout_hides_it() {
    let (mut app, path) = app_with_nets(&["A", "B"]);
    let wires = wires_of(&app, &path);
    let revision = |app: &Signex| {
        let toast = app.ui_state.net_color_palette.toast.as_ref();
        toast.expect("a toast").revision
    };
    paint(&mut app, &[wires[0]], color(10));
    clear_net(&mut app, "A");
    let first = revision(&app);
    paint(&mut app, &[wires[1]], color(20));
    clear_net(&mut app, "B");
    let second = revision(&app);
    assert_ne!(first, second);

    send(&mut app, NetColorMsg::ToastExpired(first));
    assert_eq!(cleared(&app), Some(NetColorsCleared::Net("B".into())));
    send(&mut app, NetColorMsg::ToastExpired(second));
    assert_eq!(cleared(&app), None);
}
