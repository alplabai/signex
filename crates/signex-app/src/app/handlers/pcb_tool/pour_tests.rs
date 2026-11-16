//! #2216 — a placed polygon pour through `Signex::update`: a click inside it
//! picks it and opens the area panel with its priority, island rules and
//! copper; each panel edit is one undo step; Repour Selected and Repour All.

use pcb_geom::{ItemRef, PickKind};

use super::form_tests::{form, panel};
use super::pour_cut_tests::draw_pour;
use super::tests::{app_with_board, board_of, click, escape, run, undo};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::{Message, Signex};
use crate::dock::DockMessage;
use crate::keymap::AppCommandId;
use crate::panels::pcb_primitive_properties::{
    PcbPrimitiveFacts, PrimitiveField, PrimitiveTarget, ZoneFacts, ZoneKind,
};
use crate::panels::{PanelMsg, PcbBoardMsg, PcbPrimitiveMsg};

/// The pour of `draw_pour` is cut by the net-3 track along y = 0, grown by
/// the board's 0.2 mm clearance: two pieces of 8 x 3.55 mm.
const PIECE_MM2: f64 = 8.0 * 3.55;

fn area_panel(app: &Signex) -> ZoneFacts {
    match form(app) {
        PcbPrimitiveFacts::Zone(f) => f,
        other => panic!("the area panel, not {other:?}"),
    }
}

/// Draws the pour and picks it with a click inside it, off the track.
fn picked_pour(app: &mut Signex) -> PrimitiveTarget {
    draw_pour(app);
    click(app, 5.0, 3.0);
    area_panel(app).common.target
}

fn typed_and_entered(
    app: &mut Signex,
    target: &PrimitiveTarget,
    field: PrimitiveField,
    text: &str,
) {
    panel(
        app,
        PcbPrimitiveMsg::FieldTyped {
            target: target.clone(),
            field,
            text: text.into(),
        },
    );
    panel(
        app,
        PcbPrimitiveMsg::FieldSubmitted {
            target: target.clone(),
            field,
        },
    );
}

#[test]
fn a_click_inside_a_pour_picks_it_and_opens_the_area_panel() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let target = picked_pour(&mut app);
    let pour = board_of(&app, &path).zones[0].uuid;
    assert_eq!(
        target.items,
        vec![ItemRef::Zone {
            zone: pour,
            kind: PickKind::Polygons,
        }]
    );
    let f = area_panel(&app);
    assert_eq!(f.kind, Some(ZoneKind::Pour));
    assert!(f.is_pour);
    assert_eq!(
        (f.priority, f.min_island_area, f.remove_dead_islands),
        (Some(0), Some(0.0), Some(false))
    );
    let (area, pieces) = f.copper.expect("the fill is computed");
    assert_eq!(pieces, 2, "the track cuts it in two");
    assert!((area - 2.0 * PIECE_MM2).abs() < 1e-6, "{area}");

    // The Polygons chip of the Selection Filter is live: turned off, a
    // click inside the pour picks nothing.
    escape(&mut app);
    let toggled = PcbBoardMsg::FilterKindToggled(PickKind::Polygons);
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::PcbBoard(
        toggled,
    ))));
    assert!(!app.ui_state.pcb_pick_filter.is_on(PickKind::Polygons));
    click(&mut app, 5.0, 3.0);
    let picked: Vec<ItemRef> = app
        .interaction_state
        .pcb_selection
        .selection()
        .items()
        .collect();
    assert!(picked.is_empty(), "{picked:?}");
}

#[test]
fn the_area_panel_sets_a_pours_priority_and_island_rules_one_step_each() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let target = picked_pour(&mut app);

    typed_and_entered(&mut app, &target, PrimitiveField::Priority, "2");
    assert_eq!(board_of(&app, &path).zones[0].priority, 2);
    typed_and_entered(&mut app, &target, PrimitiveField::Priority, "1.5");
    assert_eq!(
        board_of(&app, &path).zones[0].priority,
        2,
        "a priority is a whole number"
    );

    panel(
        &mut app,
        PcbPrimitiveMsg::DeadIslandsToggled {
            target: target.clone(),
            on: true,
        },
    );
    assert!(board_of(&app, &path).zones[0].remove_dead_islands);
    assert_eq!(area_panel(&app).remove_dead_islands, Some(true));

    // Both pieces are below 30 mm²: the fill drops them.
    typed_and_entered(&mut app, &target, PrimitiveField::MinIslandArea, "30");
    assert_eq!(board_of(&app, &path).zones[0].min_island_area, 30.0);
    let (_, pieces) = area_panel(&app).copper.expect("the fill is computed");
    assert_eq!(pieces, 0, "every island is below the minimum");

    undo(&mut app);
    assert_eq!(board_of(&app, &path).zones[0].min_island_area, 0.0);
    undo(&mut app);
    assert!(!board_of(&app, &path).zones[0].remove_dead_islands);
    undo(&mut app);
    assert_eq!(board_of(&app, &path).zones[0].priority, 0);
}

#[test]
fn repour_selected_needs_a_selected_pour_and_reports_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    draw_pour(&mut app);
    let before = serde_json::to_string(board_of(&app, &path)).unwrap();
    let refused = "Repour Selected: select a polygon pour first";
    let reported = "areas of copper";
    let refused_before = crate::diagnostics::logged_count(refused);
    run(&mut app, "repour_selected");
    assert!(crate::diagnostics::logged_count(refused) > refused_before);

    click(&mut app, 5.0, 3.0);
    let reported_before = crate::diagnostics::logged_count(reported);
    run(&mut app, "repour_selected");
    assert!(crate::diagnostics::logged_count(reported) > reported_before);
    let fills = app
        .current_pour_fills(&path)
        .expect("the fills of the board in view");
    assert_eq!(fills.of(0).expect("filled").pieces.len(), 2);

    escape(&mut app);
    let reported_before = crate::diagnostics::logged_count(reported);
    run(&mut app, "repour_all");
    assert!(crate::diagnostics::logged_count(reported) > reported_before);
    assert_eq!(
        serde_json::to_string(board_of(&app, &path)).unwrap(),
        before,
        "a repour changes nothing in the document"
    );
}

#[test]
fn repour_all_named_a_board_answers_with_each_pours_fill() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    app.ui_state.first_run_tour_open = false;
    app.document_state.projects[0].data.pcb_file = Some("board.snxpcb".into());
    draw_pour(&mut app);
    let pour = board_of(&app, &path).zones[0].uuid;

    let command = AppCommandId::new("repour_all").unwrap();
    let document = format!("document={}", path.display());
    let args = CommandArgs::parse_operands(&command, &[&document]).expect("operands parse");
    let (outcome, _) = app.run_headless_command_in_test(&command, args);
    let CommandOutcome::Answered(answer) = outcome else {
        panic!("not answered: {outcome:?}");
    };
    let pours = answer["pours"].as_array().expect("pours");
    assert_eq!(pours.len(), 1);
    assert_eq!(pours[0]["zone"], pour.to_string());
    assert_eq!(pours[0]["filled"], true);
    assert_eq!(pours[0]["pieces"], 2);
    assert_eq!(pours[0]["islands_removed"], 0);
    let area = pours[0]["area_mm2"].as_f64().expect("area");
    assert!((area - 2.0 * PIECE_MM2).abs() < 1e-6, "{area}");

    let (outcome, _) = app.run_headless_command_in_test(
        &command,
        CommandArgs::parse_operands(&command, &[]).expect("parse"),
    );
    assert!(
        matches!(outcome, CommandOutcome::Disabled { .. }),
        "no board named: {outcome:?}"
    );
}
