//! Advanced Schematic exit criterion 3, the board half: Update PCB on
//! `net-class-directives` puts the classes its Net Blanket (POWER) and Net
//! Parameters directive (HS) give into the board's net classes, the
//! project's net-class table, with the nets the netlist puts in them and the
//! rules the directives set, beside the rules it writes into the DRC rule
//! text (`exit_fixtures::the_directive_classes_and_rules_reach_update_pcb`).
//! DRC classes a board net by its name through that table
//! (`signex_types::net_class::first_matching_class`).

use signex_types::format::SnxPcb;
use signex_types::net_class::first_matching_class;

use super::exit_fixtures::{CLASSES, fixture_dir, open_fixture};
use crate::app::project_sheets::plan_project_sheets_for;
use crate::app::saved_channel_naming::SavedChannelNaming;
use crate::app::update_pcb::design::{DesignRequest, compute};

#[test]
fn update_pcb_puts_the_directive_classes_and_their_rules_on_the_board() {
    let mut app = open_fixture(CLASSES);
    let dir = fixture_dir(CLASSES);
    let board_path = dir.join(format!("{CLASSES}.snxpcb"));
    let text = std::fs::read_to_string(&board_path).expect("the board file");
    let board = SnxPcb::parse(&text).expect("the board reads").board;
    let computed = compute(DesignRequest {
        plan: plan_project_sheets_for(&app.document_state, &dir.join("main.snxsch")),
        project_dir: dir.clone(),
        board,
        revision: 0,
        libraries: Vec::new(),
        open_revisions: Vec::new(),
        previous: None,
        naming: SavedChannelNaming::of(&app.document_state.projects[0].data),
    })
    .expect("the design reads");
    let snapshot = &computed.snapshot;

    app.apply_update_pcb_classes(&board_path, snapshot)
        .expect("Execute stores the classes");

    let table = &app.document_state.projects[0].data.net_classes;
    let class_of = |net: &str| first_matching_class(table, net).map(|c| c.name.clone());
    let mut classed = 0;
    for net in &snapshot.netlist.nets {
        let wanted = net.class.as_ref().map(|c| c.0.clone());
        assert_eq!(class_of(&net.name), wanted, "the board net {}", net.name);
        classed += usize::from(wanted.is_some());
    }
    assert_eq!(classed, 2, "VIN and CLK are classed");
    assert_eq!(class_of("VIN").as_deref(), Some("POWER"));
    assert_eq!(class_of("CLK").as_deref(), Some("HS"));
    assert_eq!(class_of("SIG"), None);

    let rules = |name: &str| {
        (table.iter().find(|c| c.name == name))
            .unwrap_or_else(|| panic!("the class {name}"))
            .board_rules()
    };
    assert_eq!(rules("HS").trace_width, 0.2, "the Net Parameters width");
    assert_eq!(rules("HS").clearance, 0.15, "the Net Parameters clearance");
    assert_eq!(
        rules("POWER").trace_width,
        0.5,
        "the Net Blanket track width"
    );

    assert!(
        app.update_pcb_class_edit(&board_path, snapshot).is_none(),
        "a second Update PCB has nothing more to put"
    );
}

/// The GUI path with only classes to apply: the rules are already stored
/// and no board row is ticked, so the classes alone turn Execute on; the
/// dialog shows them as one summary line, and Execute puts them on the
/// board (exit criterion 3; no mockup row, a summary line).
#[test]
fn execute_in_the_dialog_applies_classes_alone() {
    use crate::app::Message;
    use crate::app::update_pcb::{Computed, Phase, UpdatePcbMsg, UpdatePcbState};

    let mut app = open_fixture(CLASSES);
    let dir = fixture_dir(CLASSES);
    let board_path = dir.join(format!("{CLASSES}.snxpcb"));
    let text = std::fs::read_to_string(&board_path).expect("the board file");
    let board = SnxPcb::parse(&text).expect("the board reads").board;
    app.open_pcb_tab(board_path.clone(), format!("{CLASSES}.snxpcb"), board);
    let engine = app.document_state.board_engine(&board_path).expect("open");
    let (board, revision) = (engine.board().clone(), engine.revision());
    let open_revisions = (app.document_state.engines.iter())
        .map(|(path, engine)| (path.clone(), engine.revision()))
        .collect();
    let computed = compute(DesignRequest {
        plan: plan_project_sheets_for(&app.document_state, &dir.join("main.snxsch")),
        project_dir: dir.clone(),
        board,
        revision,
        libraries: Vec::new(),
        open_revisions,
        previous: None,
        naming: SavedChannelNaming::of(&app.document_state.projects[0].data),
    })
    .expect("the design reads");

    // The rules are stored already, and no board row is ticked.
    let project = app.document_state.projects[0].id;
    let rows = signex_drc_dsl::schematic_rules::rule_rows(&computed.snapshot.schematic_rules, "");
    app.apply_update_pcb_rules(project, &rows)
        .expect("the rules are stored");
    let mut list = computed.list.clone();
    for row in &mut list.rows {
        row.on = false;
    }
    let computed = std::sync::Arc::new(Computed {
        snapshot: std::sync::Arc::clone(&computed.snapshot),
        list,
    });
    let mut state = UpdatePcbState::new(
        project,
        CLASSES.to_string(),
        Some(board_path.clone()),
        false,
    );
    state.job = 1;
    state.phase = Phase::Computing;
    app.ui_state.update_pcb = Some(Box::new(state));
    let _ = app.update(Message::from(UpdatePcbMsg::Computed {
        job: 1,
        result: Ok(computed),
    }));

    let state = app.ui_state.update_pcb.as_deref().expect("the dialog");
    assert!(state.ready_list().is_some(), "the list is ready");
    assert_eq!(state.execute_count(), 0, "no board row is ticked");
    assert_eq!(app.update_pcb_class_count(state), 2, "POWER and HS");
    assert_eq!(
        app.update_pcb_execute_count(state),
        2,
        "Execute counts the classes"
    );
    assert_eq!(app.update_pcb_execute_block(), None, "Execute is on");

    let _ = app.update(Message::from(UpdatePcbMsg::ExecuteRequested));
    assert!(
        app.ui_state.update_pcb.is_none(),
        "Execute closed the dialog"
    );
    let table = &app.document_state.projects[0].data.net_classes;
    let class_of = |net: &str| first_matching_class(table, net).map(|c| c.name.clone());
    assert_eq!(class_of("VIN").as_deref(), Some("POWER"));
    assert_eq!(class_of("CLK").as_deref(), Some("HS"));
}
