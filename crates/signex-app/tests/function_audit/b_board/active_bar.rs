//! Rows m106 and m108: the PCB panel's net table and the PCB active bar.

use pcb_geom::PickKind;
use signex_app::app::{Message, PcbBarMenu, PcbBarMsg, PcbMsg};
use signex_app::keymap::AppCommandId;
use signex_app::panels::pcb_primitive_properties::PrimitiveField;
use signex_app::panels::{PanelKind, PcbBoardMsg, PcbNetsSummary, PcbPrimitiveMsg};

use super::support::{Bench, all_kinds, open};

// ---- m106 PCB panel ----

fn nets(b: &Bench) -> PcbNetsSummary {
    b.app
        .document_state
        .panel_ctx
        .pcb_nets
        .clone()
        .expect("the net table of the board")
}

fn net_length(b: &Bench, name: &str) -> f64 {
    nets(b)
        .nets
        .iter()
        .find(|row| row.name == name)
        .map(|row| row.length_mm)
        .expect("a row for the net")
}

#[test]
fn the_pcb_panel_is_docked_with_a_board_and_lists_its_nets() {
    let b = open(all_kinds());
    assert!(
        b.app
            .document_state
            .dock
            .locate(PanelKind::PcbNets)
            .is_some()
    );
    let table = nets(&b);
    let names: Vec<&str> = table.nets.iter().map(|row| row.name.as_str()).collect();
    assert_eq!(names, ["GND", "VCC"]);
    assert_eq!(table.pads, 2);
    assert_eq!(net_length(&b, "GND"), 10.0);
    assert_eq!(net_length(&b, "VCC"), 0.0);
}

#[test]
fn the_net_table_follows_an_edit_and_its_undo() {
    let mut b = open(all_kinds());
    let t = b.pick(5.0, 0.0);
    b.primitive(PcbPrimitiveMsg::NetPicked { target: t, net: 2 });
    assert_eq!(net_length(&b, "GND"), 0.0);
    assert_eq!(net_length(&b, "VCC"), 10.0);
    b.undo();
    assert_eq!(net_length(&b, "GND"), 10.0);
    assert_eq!(net_length(&b, "VCC"), 0.0);
}

#[test]
fn the_net_table_follows_a_track_length_edit() {
    let mut b = open(all_kinds());
    let t = b.pick(5.0, 0.0);
    b.enter(&t, PrimitiveField::EndX, "14");
    assert_eq!(net_length(&b, "GND"), 14.0);
}

// ---- m108 PCB active bar ----

fn bar(b: &mut Bench, msg: PcbBarMsg) {
    b.send(Message::Pcb(PcbMsg::Bar(msg)));
}

fn picked(b: &mut Bench, id: &str) {
    let id = AppCommandId::new(id).expect("a catalog command");
    bar(b, PcbBarMsg::CommandPicked(id));
}

#[test]
fn a_bar_dropdown_opens_and_a_second_toggle_closes_it() {
    let mut b = open(all_kinds());
    b.view_only(|b| bar(b, PcbBarMsg::MenuToggled(PcbBarMenu::Move)));
    assert_eq!(b.app.interaction_state.pcb_bar_menu, Some(PcbBarMenu::Move));
    bar(&mut b, PcbBarMsg::MenuToggled(PcbBarMenu::Move));
    assert_eq!(b.app.interaction_state.pcb_bar_menu, None);
}

#[test]
fn another_bar_dropdown_replaces_the_open_one() {
    let mut b = open(all_kinds());
    bar(&mut b, PcbBarMsg::MenuToggled(PcbBarMenu::Move));
    bar(&mut b, PcbBarMsg::MenuToggled(PcbBarMenu::View));
    assert_eq!(b.app.interaction_state.pcb_bar_menu, Some(PcbBarMenu::View));
}

#[test]
fn a_click_outside_closes_the_dropdown_and_changes_nothing() {
    let mut b = open(all_kinds());
    b.view_only(|b| {
        bar(b, PcbBarMsg::MenuToggled(PcbBarMenu::Align));
        bar(b, PcbBarMsg::MenuDismissed);
    });
    assert_eq!(b.app.interaction_state.pcb_bar_menu, None);
}

#[test]
fn a_right_press_outside_the_flyout_closes_it_and_leaves_the_armed_tool() {
    let mut b = open(all_kinds());
    b.command("place_board_via");
    bar(&mut b, PcbBarMsg::MenuToggled(PcbBarMenu::View));
    bar(&mut b, PcbBarMsg::FlyoutRightPressed);
    assert_eq!(b.app.interaction_state.pcb_bar_menu, None);
    assert!(b.app.interaction_state.pcb_tool.is_some(), "the tool stays");
}

#[test]
fn a_right_press_on_a_bar_part_with_no_menu_leaves_the_flyout_and_the_tool() {
    let mut b = open(all_kinds());
    b.command("place_board_via");
    bar(&mut b, PcbBarMsg::MenuToggled(PcbBarMenu::View));
    bar(&mut b, PcbBarMsg::RightPressTaken);
    assert_eq!(b.app.interaction_state.pcb_bar_menu, Some(PcbBarMenu::View));
    assert!(b.app.interaction_state.pcb_tool.is_some());
}

#[test]
fn a_picked_row_closes_the_dropdown_and_runs_its_command() {
    let mut b = open(all_kinds());
    bar(&mut b, PcbBarMsg::MenuToggled(PcbBarMenu::View));
    picked(&mut b, "place_board_via");
    assert_eq!(b.app.interaction_state.pcb_bar_menu, None);
    assert!(
        b.app.interaction_state.pcb_tool.is_some(),
        "the Via tool is armed"
    );
}

#[test]
fn the_view_rows_turn_the_view_without_touching_the_board() {
    let mut b = open(all_kinds());
    let angle = |b: &Bench| b.app.interaction_state.pcb_canvas.view_rotation_deg();
    b.view_only(|b| picked(b, "view_rotate_180"));
    assert_eq!(angle(&b), 180.0);
    picked(&mut b, "view_rotate_reset");
    assert_eq!(angle(&b), 0.0);
}

#[test]
fn the_filter_dropdowns_chip_stops_a_kind_being_picked() {
    let mut b = open(all_kinds());
    bar(&mut b, PcbBarMsg::MenuToggled(PcbBarMenu::Filter));
    b.board_panel(PcbBoardMsg::FilterKindToggled(PickKind::Vias));
    assert!(!b.app.ui_state.pcb_pick_filter.is_on(PickKind::Vias));
    b.click(20.0, 0.0);
    assert!(b.app.interaction_state.pcb_selection.selection().is_empty());
}

#[test]
fn repour_all_from_the_bar_leaves_the_document_unchanged() {
    let mut b = open(all_kinds());
    b.view_only(|b| picked(b, "repour_all"));
}
