//! #2172: a command that names its document acts on that document alone. Run
//! by id or by path it never touches what a person is doing: the active tab,
//! focus, the selection, the tool, a drag, a text edit, the wire being drawn,
//! the palette.

use signex_i18n::RenderTranslatable;

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
        CommandOutcome::Disabled { reason } => reason.render_en_us(),
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
        stackup: None,
        rooms: Vec::new(),
        options: Default::default(),
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
        free: Default::default(),
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
        stackup: None,
        rooms: Vec::new(),
        options: Default::default(),
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
        free: Default::default(),
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
        "place_blanket",
        "edit_blanket",
        "edit_no_erc_rules",
        "place_compile_mask",
        "place_diff_pair",
        "place_parameter_set",
        "place_component",
        "place_sheet_symbol",
        "place_sheet_entry",
        "sync_sheet_pins",
        "create_sheet_symbol_from_sheet",
        "sync_sheet_entries",
        "set_object_position",
        "set_object_rotation",
        "set_sheet_symbol_size",
        "set_symbol_parameter",
        "set_sheet_variable",
        "repair_pin_junctions",
        "place_design_block",
        "import_graphics",
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
                    "autoplace_fields",
                    "undo",
                    "redo",
                    "save_document",
                    "save_document_as",
                    "close_active_document",
                    "document_info",
                    "read_document",
                    "query_pins",
                    "check_erc",
                    "check_drc",
                    "check_requirements",
                    "find_items",
                    "apply_edits",
                    "revert",
                    "rules_list",
                    "rules_get",
                    "rules_add",
                    "rules_set",
                    "rules_remove",
                    "rules_enable",
                    "rules_effective",
                    "rules_explain",
                    "rules_conflicts",
                    // #2401, #2407: board edits an agent names its board for.
                    "set_layer_stack",
                    "lock_board_items",
                    "unlock_board_items",
                    "set_via_span",
                    "set_layer_sizes",
                    "set_pad_shape",
                    // The board queries (#2403).
                    "measure_board",
                    "query_board_nets",
                    "find_similar_items",
                    // #2408, #2409: the board Tools edits name their board.
                    "stitch_vias",
                    "add_teardrops",
                    "remove_teardrops",
                    "gloss_selected_routes",
                    "board_shape_from_selection",
                    "keepout_from_selection",
                    "arrange_in_box",
                    "rules_matrix_get",
                    "rules_matrix_set",
                    "rules_set_create",
                    "rules_set_edit",
                    "rules_set_apply",
                    "rules_preset_diff",
                    "rules_preset_apply",
                    "add_board_graphic",
                    "set_board_graphic",
                    "set_footprint_text",
                    "panelize_board",
                    "remove_board_panel",
                    "set_board_panel",
                    "board_panel",
                    // #2410: they act on the board they name.
                    "swap_pins",
                    "swap_gates",
                    "save_board_reuse_block",
                    "place_board_reuse_block",
                    "project_settings",
                    "project_move_sheet",
                    "project_add_variant",
                    "project_rename_variant",
                    "project_remove_variant",
                    "project_set_active_variant",
                    "project_move_library",
                    "project_set_annotate_undo",
                    "project_set_version_control",
                    "project_add_requirement",
                    "project_set_requirement",
                    "project_remove_requirement",
                    "project_enable_requirement",
                    "project_run_requirements",
                    // #2394: the board view's layer commands.
                    "hide_layer",
                    "highlight_layer",
                    "hide_layer_group",
                    "show_layer_group",
                    "show_layer_set",
                    "save_layer_set",
                    // #2363: the Layers panel's board view configuration.
                    "set_layer_visible",
                    "set_active_layer",
                    "set_layer_color",
                    // Headless forms of the Reset rows, on the named board.
                    "reset_layer_colors",
                    "reset_board_origin",
                    "show_used_layers_only",
                    "set_object_visible",
                    "set_object_transparency",
                    // #2399, #2400: DRC results and the board net classes.
                    "show_drc_violation",
                    "filter_drc_results",
                    "waive_drc_violation",
                    "unwaive_drc_violation",
                    // #2940: the DRC fixes.
                    "fix_drc_violation",
                    "fix_drc",
                    "add_net_class",
                    "rename_net_class",
                    "delete_net_class",
                    "set_net_class_rules",
                    "assign_board_net_class",
                    // #2402, #2404: board reports and Board Print / PDF.
                    "board_report_bom",
                    "board_report_netlist",
                    "board_report_net_length",
                    "board_report_components",
                    "export_board_report",
                    "export_board_pdf",
                    // #2486: the assembly drawings.
                    "export_assembly_svg",
                    "board_assembly_drawing",
                    // #2488: the board STEP model.
                    "export_board_step",
                    // #2410: DXF import onto a board.
                    "import_dxf",
                    "import_dxf_outline",
                    // #2216: Repour All on the named board.
                    "repour_all",
                    // #2389: length tuning on the named board.
                    "tune_net_length",
                    "tune_diff_pair",
                    // #2326 slice 6.P2b: the drawing order and Delete by uuid.
                    "bring_drawings_forward",
                    "bring_drawings_to_front",
                    "send_drawings_backward",
                    "send_drawings_to_back",
                    "delete_selection",
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
    // Done drawing: the sheet takes its step. The canvas's own wire in
    // progress (`app_at_work`) ends too; the busy check reads it (#2548).
    app.interaction_state.wire_drawing = false;
    app.interaction_state.wire_points.clear();
    let canvas = app.interaction_state.active_canvas_mut();
    canvas.drawing_mode = false;
    canvas.wire_preview.clear();
    assert_eq!(run(&mut app, "undo", &[&by_id(B)]), CommandOutcome::Ran);
    assert_eq!(colours(&app, "b.snxsch"), 0);
}

/// #2548: operand undo and redo use the person-busy check `revert` uses, so
/// they wait while the person draws on the sheet in an undocked window (the
/// global drawing flag is off) and while they type into an item of it.
#[test]
fn undo_and_redo_by_id_wait_while_the_person_draws_undocked_or_types_on_that_sheet() {
    let mut app = app_at_work();
    let a = path("a.snxsch");
    run(
        &mut app,
        "color_net",
        &["at=5,0", "color=#336699", &by_id(A)],
    );
    run(
        &mut app,
        "color_net",
        &["at=5,0", "color=#112233", &by_id(A)],
    );
    assert_eq!(run(&mut app, "undo", &[&by_id(A)]), CommandOutcome::Ran);
    let before = app.document_state.engines[&a].document().net_colors.clone();

    let window = iced::window::Id::unique();
    app.ui_state.windows.insert(
        window,
        crate::app::state::WindowKind::UndockedTab {
            path: a.clone(),
            title: "a".into(),
        },
    );
    let mut slot = crate::canvas::CanvasSlot::new();
    slot.drawing_mode = true;
    slot.wire_preview = vec![signex_types::schematic::Point::new(20.0, 20.0)];
    app.interaction_state.canvases.insert(window, slot);
    assert!(!app.interaction_state.wire_drawing);
    for step in ["undo", "redo"] {
        let reason = refusal(&mut app, step, &[&by_id(A)]);
        assert!(reason.contains("the person is drawing"), "{step}: {reason}");
    }
    app.interaction_state.canvases.remove(&window);
    app.ui_state.windows.remove(&window);

    let symbol = app.document_state.engines[&a].document().symbols[0].uuid;
    app.interaction_state.editing_text = Some(crate::app::TextEditState {
        uuid: symbol,
        kind: signex_types::schematic::SelectedKind::SymbolRefField,
        text: "PERSON".into(),
        original_text: "U1".into(),
        world_x: 0.0,
        world_y: 0.0,
        sheet: Some(a.clone()),
        placement: None,
    });
    for step in ["undo", "redo"] {
        let reason = refusal(&mut app, step, &[&by_id(A)]);
        assert!(reason.contains("typing"), "{step}: {reason}");
    }
    assert_eq!(
        app.document_state.engines[&a].document().net_colors,
        before,
        "nothing was stepped"
    );
    app.interaction_state.editing_text = None;
    assert_eq!(run(&mut app, "redo", &[&by_id(A)]), CommandOutcome::Ran);
}
