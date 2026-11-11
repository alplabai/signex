//! #2172: a command that names its document acts on that document alone. Run
//! by id or by path it never touches what a person is doing: the active tab,
//! focus, the selection, the tool, a drag, a text edit, the wire being drawn,
//! the palette.

use super::interaction_snapshot_tests::interaction_snapshot;
use super::queries::tests::{WIRE_A, answer, app_with, path, sheet, uid};
use super::{CommandArgs, CommandOutcome};
use crate::app::Signex;
use crate::app::handlers::editing_commands::net_color_driver as fixture;
use crate::keymap::{AppCommandId, OperandKind, all_command_ids, metadata_for_str};

/// The sheet ids of the two open sheets: `a` first, `b` the active tab.
const A: u128 = 100;
const B: u128 = 101;

fn id(name: &str) -> AppCommandId {
    AppCommandId::new(name).expect("a valid id")
}

fn run(app: &mut Signex, name: &str, operands: &[&str]) -> CommandOutcome {
    let command = id(name);
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    app.run_operands(&command, &args).0
}

fn refusal(app: &mut Signex, name: &str, operands: &[&str]) -> String {
    match run(app, name, operands) {
        CommandOutcome::Disabled { reason } => reason,
        other => panic!("{name} {operands:?} was not refused: {other:?}"),
    }
}

fn by_id(n: u128) -> String {
    format!("document={}", uid(n))
}

fn by_path(file: &str) -> String {
    format!("document={}", path(file).display())
}

fn colours(app: &Signex, file: &str) -> usize {
    app.document_state.engines[&path(file)]
        .document()
        .net_colors
        .len()
}

fn revision(app: &Signex, file: &str) -> u64 {
    app.document_state.engines[&path(file)].revision()
}

/// Two clean sheets, `b` the active tab, with a selection, a tool and a wire
/// in progress on it: a person at work.
fn app_at_work() -> Signex {
    let mut app = app_with(&[("a.snxsch", A), ("b.snxsch", B)]);
    app.ui_state.snap_enabled = false;
    fixture::activate(&mut app, &path("b.snxsch"));
    let canvas = app.interaction_state.active_canvas_mut();
    canvas.selected = vec![signex_types::schematic::SelectedItem::new(
        uid(WIRE_A),
        signex_types::schematic::SelectedKind::Wire,
    )];
    canvas.wire_preview = vec![signex_types::schematic::Point::new(1.0, 2.0)];
    canvas.drawing_mode = true;
    app.interaction_state.current_tool = crate::app::Tool::Wire;
    app
}

#[test]
fn color_net_by_id_colours_that_sheet_and_leaves_the_person_alone() {
    let mut app = app_at_work();
    let before = interaction_snapshot(&app);
    let b_revision = revision(&app, "b.snxsch");

    let outcome = run(
        &mut app,
        "color_net",
        &["at=5,0", "color=#336699", &by_id(A)],
    );

    assert_eq!(outcome, CommandOutcome::Ran);
    assert_eq!(colours(&app, "a.snxsch"), 3, "the flood colours the net");
    assert_eq!(colours(&app, "b.snxsch"), 0);
    assert_eq!(revision(&app, "b.snxsch"), b_revision, "b is untouched");
    assert_eq!(interaction_snapshot(&app), before);
    assert!(app.document_state.is_document_dirty(&path("a.snxsch")));
    assert!(!app.document_state.is_document_dirty(&path("b.snxsch")));
}

/// The named sheet is enough: the active tab being a board (nothing a sheet
/// command is enabled for) does not stop the command.
#[test]
fn a_command_by_id_does_not_depend_on_what_the_active_tab_can_do() {
    let mut app = app_with(&[("a.snxsch", A)]);
    let board = signex_types::pcb::PcbBoard {
        sketch: None,
        unknown: Default::default(),
        uuid: uid(500),
        version: 1,
        generator: "test".into(),
        thickness: 1.6,
        outline: vec![],
        layers: vec![],
        setup: None,
        nets: vec![],
        footprints: vec![],
        segments: vec![],
        vias: vec![],
        zones: vec![],
        graphics: vec![],
        texts: vec![],
    };
    app.open_pcb_tab(path("board.snxpcb"), "board".into(), board);
    assert!(
        app.document_state.active_path.is_none(),
        "a board is active"
    );
    let outcome = run(
        &mut app,
        "color_net",
        &["at=5,0", "color=#336699", &by_id(A)],
    );
    assert_eq!(outcome, CommandOutcome::Ran);
    assert_eq!(colours(&app, "a.snxsch"), 3);
    // Undo is enabled by the active tab's history, which a board has none of.
    assert_eq!(run(&mut app, "undo", &[&by_id(A)]), CommandOutcome::Ran);
    assert_eq!(colours(&app, "a.snxsch"), 0);
}

#[test]
fn color_net_by_path_does_the_same() {
    let mut by_id_app = app_at_work();
    let mut by_path_app = app_at_work();
    run(
        &mut by_id_app,
        "color_net",
        &["at=5,0", "color=#336699", &by_id(A)],
    );
    let outcome = run(
        &mut by_path_app,
        "color_net",
        &["at=5,0", "color=#336699", &by_path("a.snxsch")],
    );
    assert_eq!(outcome, CommandOutcome::Ran);
    assert_eq!(
        by_id_app.document_state.engines[&path("a.snxsch")]
            .document()
            .net_colors,
        by_path_app.document_state.engines[&path("a.snxsch")]
            .document()
            .net_colors
    );
}

/// One command id, one behaviour: naming the sheet a person has in front of
/// them is the click that sheet takes.
#[test]
fn color_net_by_id_on_the_active_sheet_is_the_pen_click() {
    let mut clicked = app_at_work();
    clicked.ui_state.pending_net_color =
        Some(signex_types::theme::Color::new(0x33, 0x66, 0x99, 255));
    let _ = clicked.update(crate::app::Message::CanvasEvent(
        crate::canvas::CanvasEvent::Clicked {
            world_x: 5.0,
            world_y: 0.0,
        },
    ));
    let mut by_command = app_at_work();
    let outcome = run(
        &mut by_command,
        "color_net",
        &["at=5,0", "color=#336699", &by_id(B)],
    );
    assert_eq!(outcome, CommandOutcome::Ran);
    assert_eq!(
        by_command.document_state.engines[&path("b.snxsch")]
            .document()
            .net_colors,
        clicked.document_state.engines[&path("b.snxsch")]
            .document()
            .net_colors
    );
    assert_eq!(
        by_command.document_state.engines[&path("b.snxsch")].undo_position(),
        clicked.document_state.engines[&path("b.snxsch")].undo_position()
    );
}

#[test]
fn undo_and_redo_by_id_step_that_documents_history_only() {
    let mut app = app_at_work();
    run(
        &mut app,
        "color_net",
        &["at=5,0", "color=#336699", &by_id(A)],
    );
    assert!(
        app.document_state.is_document_dirty(&path("a.snxsch")),
        "an edit"
    );
    let before = interaction_snapshot(&app);

    assert_eq!(run(&mut app, "undo", &[&by_id(A)]), CommandOutcome::Ran);
    assert_eq!(colours(&app, "a.snxsch"), 0);
    assert!(
        !app.document_state.is_document_dirty(&path("a.snxsch")),
        "back at the save"
    );
    assert_eq!(
        run(&mut app, "redo", &[&by_path("a.snxsch")]),
        CommandOutcome::Ran
    );
    assert_eq!(colours(&app, "a.snxsch"), 3);
    assert!(
        app.document_state.is_document_dirty(&path("a.snxsch")),
        "redone, an edit again"
    );
    assert_eq!(interaction_snapshot(&app), before);

    let nothing = refusal(&mut app, "redo", &[&by_id(A)]);
    assert!(nothing.contains("Nothing to redo"), "{nothing}");
    let other = refusal(&mut app, "undo", &[&by_id(B)]);
    assert!(
        other.contains("Nothing to undo"),
        "b has no history: {other}"
    );
}

#[test]
fn clear_all_net_colors_by_id_clears_that_sheet_alone() {
    let mut app = app_at_work();
    for target in [by_id(A), by_id(B)] {
        run(&mut app, "color_net", &["at=5,0", "color=#336699", &target]);
    }
    let before = interaction_snapshot(&app);
    assert_eq!(
        run(&mut app, "clear_all_net_colors", &[&by_id(A)]),
        CommandOutcome::Ran
    );
    assert_eq!(colours(&app, "a.snxsch"), 0);
    assert_eq!(colours(&app, "b.snxsch"), 3, "b kept its colours");
    assert_eq!(interaction_snapshot(&app), before);
    let again = refusal(&mut app, "clear_all_net_colors", &[&by_id(A)]);
    assert!(again.contains("no net colours"), "{again}");
}

/// A person editing sheet A while an agent colours and undoes on sheet B by
/// id: A and the whole UI are as they were.
#[test]
fn an_agent_on_sheet_b_leaves_the_person_on_sheet_a_untouched() {
    let mut app = app_with(&[("b.snxsch", B), ("a.snxsch", A)]);
    app.ui_state.snap_enabled = false;
    fixture::activate(&mut app, &path("a.snxsch"));
    app.interaction_state.active_canvas_mut().selected =
        vec![signex_types::schematic::SelectedItem::new(
            uid(WIRE_A),
            signex_types::schematic::SelectedKind::Wire,
        )];
    app.interaction_state.current_tool = crate::app::Tool::Wire;
    let snapshot = interaction_snapshot(&app);
    let a_revision = revision(&app, "a.snxsch");
    let encoded = |app: &Signex| {
        app.document_state.engines[&path("a.snxsch")]
            .serialize()
            .expect("encodes")
    };
    let a_before = encoded(&app);

    for step in ["color", "undo", "redo", "clear", "undo"] {
        match step {
            "color" => run(
                &mut app,
                "color_net",
                &["at=5,0", "color=#112233", &by_id(B)],
            ),
            "undo" => run(&mut app, "undo", &[&by_id(B)]),
            "redo" => run(&mut app, "redo", &[&by_id(B)]),
            _ => run(&mut app, "clear_all_net_colors", &[&by_id(B)]),
        };
        assert_eq!(interaction_snapshot(&app), snapshot, "after {step}");
        assert_eq!(revision(&app, "a.snxsch"), a_revision, "after {step}");
    }
    assert_eq!(encoded(&app), a_before, "sheet A is exactly as it was");
    assert!(!app.document_state.is_document_dirty(&path("a.snxsch")));
}

#[test]
fn a_document_that_cannot_take_the_command_is_refused_with_the_reason() {
    let mut app = app_at_work();
    let unknown = refusal(
        &mut app,
        "color_net",
        &["at=5,0", "color=#336699", &by_id(999)],
    );
    assert!(unknown.contains("no document has the id"), "{unknown}");
    let gone = refusal(&mut app, "undo", &[&by_path("never-opened.snxsch")]);
    assert!(gone.contains("not an open or referenced"), "{gone}");
    let no_wire = refusal(
        &mut app,
        "color_net",
        &["at=20,20", "color=#336699", &by_id(A)],
    );
    assert!(no_wire.contains("no wire"), "{no_wire}");
    assert_eq!(colours(&app, "a.snxsch"), 0, "a refusal changes nothing");

    // A board is not a sheet.
    let board = signex_types::pcb::PcbBoard {
        sketch: None,
        unknown: Default::default(),
        uuid: uid(500),
        version: 1,
        generator: "test".into(),
        thickness: 1.6,
        outline: vec![],
        layers: vec![],
        setup: None,
        nets: vec![],
        footprints: vec![],
        segments: vec![],
        vias: vec![],
        zones: vec![],
        graphics: vec![],
        texts: vec![],
    };
    app.open_pcb_tab(path("board.snxpcb"), "board".into(), board);
    let pcb = refusal(
        &mut app,
        "color_net",
        &["at=5,0", "color=#336699", &by_id(500)],
    );
    assert!(
        pcb.contains("acts on a sheet") && pcb.contains("pcb"),
        "{pcb}"
    );
}

#[test]
fn a_read_only_sheet_refuses_a_colour_by_id() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, _task) = Signex::new();
    let mut read_only = sheet(A);
    read_only.unknown.newer_minor_source = Some("snxsch/1.7".into());
    fixture::add_sheet_with(&mut app, &path("a.snxsch"), read_only);
    let reason = refusal(
        &mut app,
        "color_net",
        &["at=5,0", "color=#336699", &by_id(A)],
    );
    assert!(
        reason.contains("read-only") || reason.contains("newer"),
        "{reason}"
    );
    assert_eq!(colours(&app, "a.snxsch"), 0);
}

#[test]
fn close_by_id_closes_that_tab_and_the_active_one_stays() {
    let mut app = app_at_work();
    let (outcome, _task) = app.run_operands(
        &id("close_active_document"),
        &CommandArgs::parse_operands(&id("close_active_document"), &[&by_id(A)]).unwrap(),
    );
    assert_eq!(outcome, CommandOutcome::Ran);
    assert_eq!(app.document_state.tabs.len(), 1);
    assert_eq!(
        app.document_state.active_path.as_deref(),
        Some(path("b.snxsch").as_path())
    );
    assert!(app.document_state.active_tab < app.document_state.tabs.len());
    let again = refusal(&mut app, "close_active_document", &[&by_id(A)]);
    assert!(again.contains("no document has the id"), "{again}");
}

#[test]
fn a_person_keeps_the_active_document_as_the_default() {
    let mut app = app_at_work();
    let _ = run(
        &mut app,
        "color_net",
        &["at=5,0", "color=#336699", &by_id(B)],
    );
    // The palette and the keybinding give no operands: Undo steps the sheet
    // the person is looking at.
    let (outcome, _task) = app.dispatch_command_reporting(&id("undo"), CommandArgs::none(), None);
    assert_eq!(outcome, CommandOutcome::Ran);
    assert_eq!(colours(&app, "b.snxsch"), 0);
}

/// Every command that acts on a document declares the `Document` operand,
/// required, so a headless call cannot name none; every query answers.
#[test]
fn the_catalog_declares_the_document_operand_where_a_command_acts_on_one() {
    for name in [
        "color_net",
        "clear_all_net_colors",
        "assign_net_class",
        "undo",
        "redo",
        "save_document",
        "save_document_as",
        "close_active_document",
        "document_info",
        "read_document",
        "query_pins",
        "apply_edits",
        "revert",
    ] {
        let operands = metadata_for_str(name).expect("row").operands;
        let document = operands.iter().find(|op| op.name == "document");
        let document = document.unwrap_or_else(|| panic!("{name} has no `document` operand"));
        assert_eq!(document.kind, OperandKind::Document, "{name}");
        assert!(document.required, "{name}: a headless call must name it");
    }
    // The converse: only commands that act on a document declare one, `find`
    // (optional, a scope) and the placements (optional, #2199) apart.
    let placements = [
        "place_net_label",
        "place_local_net_label",
        "place_global_label",
        "place_hierarchical_label",
        "place_power_symbol",
        "place_no_connect",
        "place_junction",
        "place_no_erc",
        "place_wire",
        "draw_wire",
        "place_bus",
        "draw_bus",
        "place_text",
        "place_text_string",
        "draw_line",
        "draw_graphic_line",
        "draw_rectangle",
        "place_component",
        "place_sheet_symbol",
        "place_sheet_entry",
        "sync_sheet_pins",
    ];
    for name in all_command_ids() {
        let meta = metadata_for_str(name).expect("row");
        if placements.contains(&name) {
            let document = meta.operands.iter().find(|op| op.name == "document");
            assert!(
                document.is_some_and(|op| !op.required),
                "{name} takes an optional `document`"
            );
            continue;
        }
        if meta.operands.iter().any(|op| op.name == "document") {
            assert!(
                [
                    "color_net",
                    "clear_all_net_colors",
                    "assign_net_class",
                    "undo",
                    "redo",
                    "save_document",
                    "save_document_as",
                    "close_active_document",
                    "document_info",
                    "read_document",
                    "query_pins",
                    "find_items",
                    "apply_edits",
                    "revert",
                ]
                .contains(&name),
                "{name} declares a document operand"
            );
        }
    }
}

#[test]
fn every_query_command_answers_with_data() {
    let mut app = app_with(&[("a.snxsch", A)]);
    let doc = by_id(A);
    let wire = format!("id={}", uid(WIRE_A));
    for (name, operands) in [
        ("list_documents", vec![]),
        ("document_info", vec![doc.as_str()]),
        ("resolve", vec![wire.as_str()]),
        ("find_items", vec!["kind=wire"]),
        ("read_document", vec![doc.as_str()]),
    ] {
        let value = answer(&mut app, name, &operands);
        assert!(value.is_object(), "{name} answered {value}");
    }
}

#[test]
fn a_headless_run_that_names_no_document_is_asked_for_one() {
    for name in [
        "undo",
        "redo",
        "save_document",
        "save_document_as",
        "close_active_document",
        "clear_all_net_colors",
        "assign_net_class",
    ] {
        let missing = super::missing_headless_operand(&id(name));
        assert_eq!(missing, Some("document"), "{name}");
    }
}

/// The person is drawing a wire on the sheet an agent steps: refused with the
/// reason, nothing undone and the buffer kept; on a sheet the person is not on
/// the step runs as before.
#[test]
fn undo_by_id_is_refused_while_the_person_draws_on_that_sheet() {
    let mut app = app_at_work();
    run(
        &mut app,
        "color_net",
        &["at=5,0", "color=#336699", &by_id(B)],
    );
    app.interaction_state.wire_drawing = true;
    app.interaction_state.wire_points = vec![signex_types::schematic::Point::new(1.0, 2.0)];
    let drawing = interaction_snapshot(&app);
    let before = colours(&app, "b.snxsch");

    let reason = refusal(&mut app, "undo", &[&by_id(B)]);
    assert!(
        reason.contains("the person is drawing on this sheet"),
        "{reason}"
    );
    assert_eq!(colours(&app, "b.snxsch"), before, "nothing was undone");
    assert_eq!(interaction_snapshot(&app), drawing, "the buffer is kept");
    let redo = refusal(&mut app, "redo", &[&by_id(B)]);
    assert!(
        redo.contains("drawing") || redo.contains("Nothing to redo"),
        "{redo}"
    );

    // Another sheet is not the one being drawn on.
    run(
        &mut app,
        "color_net",
        &["at=5,0", "color=#336699", &by_id(A)],
    );
    assert_eq!(run(&mut app, "undo", &[&by_id(A)]), CommandOutcome::Ran);
    // Done drawing: the sheet takes its step.
    app.interaction_state.wire_drawing = false;
    app.interaction_state.wire_points.clear();
    assert_eq!(run(&mut app, "undo", &[&by_id(B)]), CommandOutcome::Ran);
    assert_eq!(colours(&app, "b.snxsch"), 0);
}
