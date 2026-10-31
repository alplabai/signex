//! Every command id the net navigation work wired reaches its handler through
//! the command dispatcher (#1270, #1271, #1272).

use signex_types::schematic::SelectedKind;

use super::fixture::*;
use crate::app::command::CommandArgs;
use crate::app::command::bridge::{core_to_message, is_dispatchable};
use crate::app::{Message, NetNavMsg, UiMsg};
use crate::keymap::AppCommandId;

fn run(p: &mut Project, id: &str) {
    let command = AppCommandId::new(id).expect("a catalog id");
    assert!(is_dispatchable(&command), "'{id}' has no dispatch arm");
    let _task = p.app.dispatch_command(&command, CommandArgs::none(), None);
    p.app.settle_net_index_blocking();
}

fn resolves_to(id: &str) -> NetNavMsg {
    let command = AppCommandId::new(id).expect("a catalog id");
    match core_to_message(&command) {
        Some(Message::Ui(UiMsg::NetNav(msg))) => msg,
        other => panic!("'{id}' resolved to {other:?}"),
    }
}

#[test]
fn each_new_id_resolves_to_its_net_nav_message() {
    assert!(matches!(
        resolves_to("sheet_navigation_back"),
        NetNavMsg::SheetBackRequested
    ));
    assert!(matches!(
        resolves_to("sheet_navigation_forward"),
        NetNavMsg::SheetForwardRequested
    ));
    assert!(matches!(
        resolves_to("go_to_net_partner"),
        NetNavMsg::PartnerJumpRequested
    ));
    assert!(matches!(
        resolves_to("highlight_net_under_cursor"),
        NetNavMsg::NetHighlightRequested
    ));
    assert!(matches!(
        resolves_to("highlight_related_net_objects"),
        NetNavMsg::RelatedNetHighlightRequested
    ));
    assert!(matches!(
        resolves_to("next_highlighted_net_item"),
        NetNavMsg::HighlightStepRequested { forward: true }
    ));
    assert!(matches!(
        resolves_to("previous_highlighted_net_item"),
        NetNavMsg::HighlightStepRequested { forward: false }
    ));
    assert!(matches!(
        resolves_to("clear_net_highlighting"),
        NetNavMsg::HighlightClearRequested
    ));
}

#[test]
fn the_sheet_history_commands_move_between_sheets() {
    let mut p = project();
    run(&mut p, "sheet_navigation_back");
    assert_eq!(p.active(), Some(p.mid.clone()));
    run(&mut p, "sheet_navigation_forward");
    assert_eq!(p.active(), Some(p.root.clone()));
}

#[test]
fn the_up_commands_go_to_the_parent_sheet() {
    for id in ["leave_sheet", "navigate_up_hierarchy"] {
        let mut p = project();
        p.focus(&p.mid.clone());
        // Up Hierarchy is enabled from the Navigator's hierarchy (#1772).
        p.app.refresh_panel_ctx();
        p.app.settle_navigator();
        run(&mut p, id);
        assert_eq!(p.active(), Some(p.root.clone()), "{id}");
    }
}

#[test]
fn the_partner_command_jumps_from_the_selected_port() {
    let mut p = project();
    p.select(ROOT_VCC, SelectedKind::Label);
    run(&mut p, "go_to_net_partner");
    assert_eq!(p.active(), Some(p.mid.clone()));
}

#[test]
fn the_highlight_commands_set_step_and_clear_the_highlight() {
    let mut p = project();
    p.select(ROOT_VCC, SelectedKind::Label);

    run(&mut p, "highlight_net_under_cursor");
    assert!(p.app.document_state.net_nav.highlight.is_some());

    run(&mut p, "next_highlighted_net_item");
    assert_eq!(p.selected().first().map(|i| i.uuid), Some(id(ROOT_R1)));
    run(&mut p, "previous_highlighted_net_item");
    assert_eq!(
        p.active(),
        Some(p.third.clone()),
        "previous wraps to the end"
    );

    run(&mut p, "highlight_related_net_objects");
    assert!(p.app.document_state.net_nav.highlight.is_some());

    run(&mut p, "clear_net_highlighting");
    assert!(p.app.document_state.net_nav.highlight.is_none());
}
