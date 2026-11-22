//! Row m164: the board status bar counters and their popovers.

use pcb_geom::ItemRef;
use signex_app::app::{InputMsg, Message, Signex, StatusBarRequest, UiMsg};
use signex_app::panels::PanelKind;
use signex_app::status_bar::{BoardCounter, BoardPopoverEvent, BoardPopoverKind};
use signex_types::pcb::PcbBoard;
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use uuid::Uuid;

use super::support::{BoardBench, open_board};
use crate::settle_support::settle;

const R1: Uuid = Uuid::from_u128(0xC641);

/// Two copper layers, R1 at (10, 10) and R2 at (30, 10), each with one pad.
fn board() -> PcbBoard {
    let part = |id: Uuid, pad: u128, reference: &str, x: f64| {
        serde_json::json!({
            "uuid": id, "reference": reference,
            "position": {"x": x, "y": 10.0}, "layer": "TopCopper",
            "pads": [{"uuid": Uuid::from_u128(pad), "number": "1", "pad_type": "smd",
                      "shape": "rect", "position": {"x": 0.0, "y": 0.0},
                      "size": {"x": 1.0, "y": 1.0}, "drill": null,
                      "layers": ["TopCopper"], "net": null}]
        })
    };
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(0xC640),
        "setup": {"grid_size": 0.5},
        "layers": [
            {"id": 0, "name": "TopCopper", "layer_type": "signal"},
            {"id": 31, "name": "BottomCopper", "layer_type": "signal"}
        ],
        "footprints": [
            part(R1, 0xC651, "R1", 10.0),
            part(Uuid::from_u128(0xC642), 0xC652, "R2", 30.0)
        ]
    }))
    .expect("the audit board")
}

fn counter(b: &mut BoardBench, which: BoardCounter) {
    let request = StatusBarRequest::BoardCounterClicked {
        window: None,
        counter: which,
    };
    let message = Signex::status_bar_message(b.main, None, request);
    settle(&mut b.app, message);
}

fn popover_message(event: BoardPopoverEvent) -> Message {
    Message::Ui(UiMsg::StatusBar(StatusBarRequest::BoardPopover(event)))
}

fn event(b: &mut BoardBench, event: BoardPopoverEvent) {
    settle(&mut b.app, popover_message(event));
}

fn open_kind(b: &BoardBench) -> Option<BoardPopoverKind> {
    b.app
        .ui_state
        .board_status
        .popover
        .as_ref()
        .map(|popover| popover.kind)
}

fn grid(b: &BoardBench) -> f32 {
    b.app.status_grid(b.main).size_mm
}

#[test]
fn each_counter_opens_its_own_popover_and_a_second_click_closes_it() {
    let mut b = open_board(board());
    for which in [
        BoardCounter::ActiveLayer,
        BoardCounter::Grid,
        BoardCounter::Routed,
        BoardCounter::Unrouted,
        BoardCounter::Drc,
        BoardCounter::Selected,
    ] {
        counter(&mut b, which);
        assert_eq!(open_kind(&b), Some(which.popover()), "{which:?}");
        counter(&mut b, which);
        assert_eq!(open_kind(&b), None, "{which:?} closes");
    }
    assert!(!b.dirty(), "opening a popover edits nothing");
}

#[test]
fn routed_and_unrouted_share_one_popover() {
    assert_eq!(
        BoardCounter::Routed.popover(),
        BoardCounter::Unrouted.popover()
    );
}

#[test]
fn opening_another_counter_replaces_the_open_popover() {
    let mut b = open_board(board());
    counter(&mut b, BoardCounter::Drc);
    counter(&mut b, BoardCounter::Grid);
    assert_eq!(open_kind(&b), Some(BoardPopoverKind::Grid));
}

#[test]
fn esc_closes_the_popover_and_keeps_the_selection() {
    let mut b = open_board(board());
    b.click(10.0, 10.0);
    let selected = b.app.interaction_state.pcb_selection.selection().len();
    assert_eq!(selected, 1);
    counter(&mut b, BoardCounter::Selected);
    settle(
        &mut b.app,
        Message::Input(InputMsg::EscapePressed {
            window: Some(b.main),
        }),
    );
    assert_eq!(open_kind(&b), None);
    assert_eq!(
        b.app.interaction_state.pcb_selection.selection().len(),
        1,
        "Esc closed only the popover"
    );
}

#[test]
fn dismissing_the_popover_closes_it() {
    let mut b = open_board(board());
    counter(&mut b, BoardCounter::Unrouted);
    event(&mut b, BoardPopoverEvent::Dismissed);
    assert_eq!(open_kind(&b), None);
}

#[test]
fn a_row_of_the_active_layer_popover_makes_that_layer_active() {
    let mut b = open_board(board());
    let bottom = PcbLayer::Copper(CopperLayer::Bottom);
    counter(&mut b, BoardCounter::ActiveLayer);
    event(&mut b, BoardPopoverEvent::LayerPicked(bottom));
    let active = b
        .app
        .ui_state
        .pcb_layer_views
        .get(&b.path)
        .map(|view| view.active());
    assert_eq!(active, Some(bottom));
    assert_eq!(
        open_kind(&b),
        Some(BoardPopoverKind::ActiveLayer),
        "the Active layer popover stays open"
    );
    assert!(!b.dirty(), "the active layer is view state");
}

#[test]
fn a_grid_preset_sets_the_grid_the_bar_reads() {
    let mut b = open_board(board());
    counter(&mut b, BoardCounter::Grid);
    event(&mut b, BoardPopoverEvent::GridPresetPicked(1.27));
    assert_eq!(grid(&b), 1.27);
}

#[test]
fn a_custom_pitch_sets_the_grid_on_enter_only() {
    let mut b = open_board(board());
    counter(&mut b, BoardCounter::Grid);
    event(&mut b, BoardPopoverEvent::GridPresetPicked(0.25));
    event(
        &mut b,
        BoardPopoverEvent::CustomGridEdited("0.2".to_string()),
    );
    assert_eq!(grid(&b), 0.25, "typing alone changes nothing");
    event(&mut b, BoardPopoverEvent::CustomGridSubmitted);
    assert_eq!(grid(&b), 0.2);
}

#[test]
fn a_bad_custom_pitch_is_refused_and_keeps_the_text() {
    let mut b = open_board(board());
    counter(&mut b, BoardCounter::Grid);
    event(&mut b, BoardPopoverEvent::GridPresetPicked(0.25));
    let before = b.json();
    for bad in ["-3", "0", "abc", ""] {
        event(&mut b, BoardPopoverEvent::CustomGridEdited(bad.to_string()));
        event(&mut b, BoardPopoverEvent::CustomGridSubmitted);
        assert_eq!(grid(&b), 0.25, "{bad:?} changes no grid");
        let popover = b
            .app
            .ui_state
            .board_status
            .popover
            .as_ref()
            .expect("the popover stays open");
        assert!(popover.custom_refused, "{bad:?} is marked refused");
        assert_eq!(popover.custom_grid, bad, "the typed text is kept");
    }
    assert_eq!(b.json(), before, "the board is unchanged");
}

#[test]
fn editing_a_refused_pitch_clears_the_error() {
    let mut b = open_board(board());
    counter(&mut b, BoardCounter::Grid);
    event(&mut b, BoardPopoverEvent::CustomGridEdited("x".to_string()));
    event(&mut b, BoardPopoverEvent::CustomGridSubmitted);
    event(
        &mut b,
        BoardPopoverEvent::CustomGridEdited("0.5".to_string()),
    );
    let popover = b.app.ui_state.board_status.popover.as_ref().expect("open");
    assert!(!popover.custom_refused);
}

#[test]
fn snap_to_grid_in_the_popover_flips_snapping() {
    let mut b = open_board(board());
    counter(&mut b, BoardCounter::Grid);
    let before = b.app.ui_state.snap_enabled;
    // Its task writes the preference file, which a test must not reach.
    let _ = b
        .app
        .update(popover_message(BoardPopoverEvent::SnapToggled));
    assert_eq!(b.app.ui_state.snap_enabled, !before);
}

#[test]
fn run_drc_from_the_popover_lands_a_result_and_edits_nothing() {
    let mut b = open_board(board());
    assert!(b.app.interaction_state.pcb_drc.done_for(&b.path).is_none());
    let before = b.json();
    counter(&mut b, BoardCounter::Drc);
    event(&mut b, BoardPopoverEvent::DrcRunRequested);
    assert!(b.app.interaction_state.pcb_drc.done_for(&b.path).is_some());
    assert_eq!(b.json(), before);
    assert!(!b.dirty());
}

#[test]
fn open_messages_closes_the_popover_and_shows_the_panel() {
    let mut b = open_board(board());
    counter(&mut b, BoardCounter::Drc);
    let _ = b
        .app
        .update(popover_message(BoardPopoverEvent::MessagesOpened));
    assert_eq!(open_kind(&b), None);
    assert!(
        b.app
            .document_state
            .dock
            .locate(PanelKind::Messages)
            .is_some()
    );
}

#[test]
fn clear_in_the_selection_popover_deselects_and_closes_it() {
    let mut b = open_board(board());
    b.click(10.0, 10.0);
    assert!(
        b.app
            .interaction_state
            .pcb_selection
            .selection()
            .items()
            .any(|item| item == ItemRef::Footprint(R1))
    );
    counter(&mut b, BoardCounter::Selected);
    event(&mut b, BoardPopoverEvent::SelectionCleared);
    assert_eq!(b.app.interaction_state.pcb_selection.selection().len(), 0);
    assert_eq!(open_kind(&b), None);
    assert!(!b.dirty());
}
