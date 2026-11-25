//! #2797 — Length Matching through `Signex::update` and the headless path:
//! Length Matching... opens one tab after the board's and focuses it again;
//! a group added, edited, given a net and deleted in the tab is one board
//! undo step each; the dock panel and the tab read the same rows; an agent
//! makes, reads and removes groups with the commands.

use std::path::{Path, PathBuf};

use serde_json::json;
use signex_types::pcb::PcbBoard;
use signex_types::pcb_length::LengthTopology;
use uuid::Uuid;

use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, Message, Signex};
use crate::dock::DockMessage;
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;
use crate::panels::PanelMsg;
use crate::panels::length_matching::{LengthField, LengthMatchingMsg};

/// DQ0 28.40 mm, DQ6 28.91 mm, CLK 10 mm; DQ7 has no copper.
fn app() -> (Signex, PathBuf, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let track = |n: u128, y: f64, length: f64, net: u32| {
        json!({
            "uuid": Uuid::from_u128(n), "start": {"x": 0.0, "y": y},
            "end": {"x": length, "y": y}, "width": 0.2, "layer": "TopCopper", "net": net
        })
    };
    let board: PcbBoard = serde_json::from_value(json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": 0.5, "trace_width": 0.2, "clearance": 0.2},
        "nets": [
            {"number": 1, "name": "DDR_DQ0"},
            {"number": 2, "name": "DDR_DQ6"},
            {"number": 3, "name": "DDR_DQ7"},
            {"number": 4, "name": "CLK"}
        ],
        "segments": [track(11, 0.0, 28.40, 1), track(12, 2.0, 28.91, 2), track(13, 4.0, 10.0, 4)]
    }))
    .unwrap();
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    let path = dir.path().join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board);
    (app, path, dir)
}

fn board(app: &Signex, path: &Path) -> PcbBoard {
    app.document_state.pcb_board(path).expect("board").clone()
}

fn send(app: &mut Signex, msg: LengthMatchingMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::LengthMatching(
        msg,
    ))));
}

fn type_in(app: &mut Signex, field: LengthField, text: &str) {
    send(
        app,
        LengthMatchingMsg::FieldTyped {
            field,
            text: text.into(),
        },
    );
}

fn tabs_of_kind(app: &Signex) -> usize {
    app.document_state
        .tabs
        .iter()
        .filter(|t| t.kind.is_length_matching())
        .count()
}

/// Add the DDR group in the tab: name, target 28.5, tolerance 0.25.
fn add_ddr(app: &mut Signex) -> Uuid {
    type_in(app, LengthField::NewName, "DDR_DQ");
    type_in(app, LengthField::NewTarget, "28.5");
    type_in(app, LengthField::NewTolerance, "0.25");
    send(app, LengthMatchingMsg::GroupAdded);
    app.document_state
        .panel_ctx
        .length_matching_ui
        .current
        .expect("the new group is picked")
}

#[test]
fn length_matching_opens_one_tab_after_the_board_and_focuses_it_again() {
    let (mut app, _path, _dir) = app();
    let _ = app.update(Message::Menu(MenuMessage::command("length_matching")));
    assert_eq!(tabs_of_kind(&app), 1);
    let docs = &app.document_state;
    assert!(docs.tabs[docs.active_tab].kind.is_length_matching());
    assert!(matches!(
        docs.tabs[docs.active_tab - 1].kind,
        crate::app::TabKind::Pcb
    ));
    send(&mut app, LengthMatchingMsg::TabOpened);
    assert_eq!(tabs_of_kind(&app), 1, "a second open focuses it");
    let facts = app.document_state.panel_ctx.length_matching.as_ref();
    assert!(facts.is_some_and(|f| f.groups.is_empty()), "{facts:?}");
}

#[test]
fn a_group_added_in_the_tab_reports_its_members_and_undoes_in_one_step() {
    let (mut app, path, _dir) = app();
    send(&mut app, LengthMatchingMsg::TabOpened);
    let before = board(&app, &path).options.clone();
    let group = add_ddr(&mut app);
    assert_eq!(board(&app, &path).options.length_groups.len(), 1);
    for net in ["DDR_DQ0", "DDR_DQ6", "DDR_DQ7"] {
        send(
            &mut app,
            LengthMatchingMsg::NetAdded {
                group,
                net: net.into(),
            },
        );
    }
    let facts = app
        .document_state
        .panel_ctx
        .length_matching
        .clone()
        .expect("facts");
    let row = &facts.groups[0];
    assert_eq!(row.rule, "MatchedLengths_DDR_DQ");
    let nets: Vec<&str> = row.members.iter().map(|m| m.net.as_str()).collect();
    assert_eq!(nets, ["DDR_DQ0", "DDR_DQ6", "DDR_DQ7"]);
    assert_eq!(row.out_of_band(), 1, "DDR_DQ6 is 0.41 mm over");
    assert!(
        app.document_state
            .panel_ctx
            .length_matching_ui
            .drafts
            .is_empty(),
        "the New group row is cleared"
    );
    // Four steps: the group and three nets. Undo all four.
    for _ in 0..4 {
        let _ = app.update(Message::Edit(EditMsg::Undo));
    }
    assert_eq!(board(&app, &path).options, before);
}

#[test]
fn the_picked_groups_fields_edit_it_and_delete_removes_it() {
    let (mut app, path, _dir) = app();
    send(&mut app, LengthMatchingMsg::TabOpened);
    let group = add_ddr(&mut app);
    type_in(&mut app, LengthField::Tolerance, "0.5");
    send(
        &mut app,
        LengthMatchingMsg::FieldSubmitted(LengthField::Tolerance),
    );
    send(
        &mut app,
        LengthMatchingMsg::TopologyPicked {
            group,
            topology: LengthTopology::FlyBy,
        },
    );
    type_in(&mut app, LengthField::Name, "DDR_BYTE0");
    send(
        &mut app,
        LengthMatchingMsg::FieldSubmitted(LengthField::Name),
    );
    let stored = board(&app, &path).options.length_groups[0].clone();
    assert_eq!(stored.tolerance_mm, 0.5);
    assert_eq!(stored.topology, LengthTopology::FlyBy);
    assert_eq!(stored.name, "DDR_BYTE0");
    assert_eq!(stored.uuid, group, "the same group throughout");

    // A typed text that is no number changes nothing.
    type_in(&mut app, LengthField::Target, "long");
    send(
        &mut app,
        LengthMatchingMsg::FieldSubmitted(LengthField::Target),
    );
    assert_eq!(board(&app, &path).options.length_groups[0], stored);

    send(&mut app, LengthMatchingMsg::GroupDeleted(group));
    assert!(board(&app, &path).options.length_groups.is_empty());
    assert_eq!(board(&app, &path).nets.len(), 4, "the nets stay");
    assert_eq!(
        app.document_state.panel_ctx.length_matching_ui.current,
        None
    );
}

#[test]
fn the_dock_panel_folds_a_group_and_the_filter_hides_groups_in_band() {
    let (mut app, path, _dir) = app();
    send(&mut app, LengthMatchingMsg::TabOpened);
    let group = add_ddr(&mut app);
    send(
        &mut app,
        LengthMatchingMsg::NetAdded {
            group,
            net: "DDR_DQ0".into(),
        },
    );
    send(&mut app, LengthMatchingMsg::GroupToggled(group));
    let ui = &app.document_state.panel_ctx.length_matching_ui;
    assert!(ui.closed.contains(&group));
    assert!(ui.is_for(&path));
    send(
        &mut app,
        LengthMatchingMsg::FilterPicked(crate::panels::length_matching::LengthFilter::OutOfBand),
    );
    let facts = app
        .document_state
        .panel_ctx
        .length_matching
        .clone()
        .expect("facts");
    let filter = app.document_state.panel_ctx.length_matching_ui.filter;
    assert!(!filter.shows(&facts.groups[0]), "DDR_DQ0 is in band");
}

fn headless(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new(id).unwrap();
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    app.run_headless_command_in_test(&command, args).0
}

fn answered(outcome: CommandOutcome) -> serde_json::Value {
    match outcome {
        CommandOutcome::Answered(answer) => answer,
        other => panic!("not answered: {other:?}"),
    }
}

#[test]
fn an_agent_makes_reads_and_removes_a_group() {
    let (mut app, path, _dir) = app();
    let document = format!("document={}", path.display());
    let made = answered(headless(
        &mut app,
        "set_length_group",
        &[
            &document,
            "name=DDR_DQ",
            "target=28.5",
            "tolerance=0.25mm",
            "topology=fly_by",
            "members=DDR_DQ0,DDR_DQ6,DDR_DQ7",
        ],
    ));
    let group = &made["groups"][0];
    assert_eq!(group["name"], "DDR_DQ");
    assert_eq!(group["rule"], "MatchedLengths_DDR_DQ");
    assert_eq!(group["topology"], "fly_by");
    assert_eq!(group["out_of_band"], 1, "{made}");
    assert_eq!(group["unrouted"], 1, "{made}");
    assert_eq!(group["members"][1]["status"], "over");
    assert_eq!(group["members"][2]["status"], "unrouted");

    let read = answered(headless(&mut app, "length_matching", &[&document]));
    assert_eq!(read["groups"].as_array().map(Vec::len), Some(1), "{read}");

    let refused = headless(
        &mut app,
        "set_length_group",
        &[&document, "name=CLK", "target=10"],
    );
    assert!(
        matches!(&refused, CommandOutcome::Disabled { reason } if signex_i18n::RenderTranslatable::render_en_us(reason).contains("target and a tolerance")),
        "{refused:?}"
    );
    let refused = headless(
        &mut app,
        "set_length_group",
        &[&document, "name=DDR_DQ", "members=NOPE"],
    );
    assert!(
        matches!(refused, CommandOutcome::Disabled { .. }),
        "{refused:?}"
    );

    let left = answered(headless(
        &mut app,
        "remove_length_group",
        &[&document, "name=DDR_DQ"],
    ));
    assert_eq!(left["removed"], "DDR_DQ");
    assert_eq!(left["groups"].as_array().map(Vec::len), Some(0));
    assert!(board(&app, &path).options.length_groups.is_empty());
}
