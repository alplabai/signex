//! The sheet right-click menu follows each command's own rule and carries the
//! edit and net rows (#2419). Every test presses the real row through
//! `Signex::update`, and the lock and Clear Filter cases also run the command
//! through the agent's dispatch, which answers with a structured outcome.

use std::collections::HashSet;

use super::row_effects::{
    app_with, assert_menu_closed, canvas_rows, doc, logged, one_symbol, open_menu, poses, press,
    row_is_live, select, select_symbols, settle_answering_clipboard, sheet_of, symbol,
};
use super::*;

use crate::app::command::{CommandArgs, CommandOutcome};
use crate::keymap::AppCommandId;
use signex_i18n::menu_ids;
use signex_types::schematic::{Point, SelectedItem, SelectedKind, Wire};
use uuid::Uuid;

/// The rows that would change nothing on a selection that is all locked.
const LOCK_GREYED: [&str; 5] = [
    "rotate_counterclockwise",
    "mirror_x",
    "mirror_y",
    "delete_selection",
    "cut",
];

/// One locked part, selected. Its reference is unique so the shared
/// Messages ring cannot mix it up with another test's line.
fn locked_part() -> Signex {
    let mut part = symbol(1, "LK2419", 10.0, 10.0);
    part.locked = true;
    let mut app = app_with(sheet_of(vec![part]));
    select_symbols(&mut app);
    app
}

fn id(command: &str) -> AppCommandId {
    AppCommandId::new(command).expect("catalog id")
}

fn item<'a>(
    rows: &'a [DropdownEntry<Message>],
    label: &str,
) -> &'a signex_widgets::active_bar_dropdown::DropdownItem<Message> {
    rows.iter()
        .find_map(|e| match e {
            DropdownEntry::Item(it) if it.label == label => Some(it),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing row {label:?}"))
}

#[test]
fn a_locked_selection_greys_the_rows_that_would_change_nothing() {
    let mut app = locked_part();
    open_menu(&mut app);
    let rows = canvas_rows(&app);
    for command in LOCK_GREYED {
        assert!(
            !row_is_live(&rows, command),
            "{command} live on a locked part"
        );
    }
    for command in ["copy", "duplicate", "edit_selected_object_properties"] {
        assert!(
            row_is_live(&rows, command),
            "{command} greyed on a locked part"
        );
    }
    // The Edit menu and the keys read the same fact.
    assert!(app.command_context(None).selection_all_locked);

    let mut free = one_symbol();
    open_menu(&mut free);
    let rows = canvas_rows(&free);
    for command in LOCK_GREYED {
        assert!(
            row_is_live(&rows, command),
            "{command} greyed on a free part"
        );
    }
}

#[test]
fn turning_a_locked_selection_is_refused_with_a_reason_and_names_the_lock() {
    let _ = crate::diagnostics::init_logging();
    let mut app = locked_part();
    let before = poses(&app);
    let (outcome, _task) =
        app.run_headless_command_in_test(&id("rotate_counterclockwise"), CommandArgs::none());
    match outcome {
        CommandOutcome::Disabled { reason } => {
            let reason = signex_i18n::RenderTranslatable::render_en_us(&reason);
            assert!(reason.contains("locked"), "reason: {reason}");
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
    assert_eq!(poses(&app), before, "the locked part turned");
    assert!(
        logged(&["LK2419 is locked", "rotated"]),
        "the lock was not named"
    );
}

#[test]
fn undo_and_redo_rows_step_the_sheet_history() {
    let mut app = one_symbol();
    open_menu(&mut app);
    let rows = canvas_rows(&app);
    assert!(!row_is_live(&rows, "undo"), "nothing to undo yet");
    assert!(!row_is_live(&rows, "redo"), "nothing to redo yet");

    let _ = press(&mut app, "delete_selection");
    assert!(doc(&app).symbols.is_empty(), "the part was not deleted");
    let _ = press(&mut app, "undo");
    assert_eq!(doc(&app).symbols.len(), 1, "undo did not restore the part");
    assert_menu_closed(&app, "undo");
    // Redo is a row of the empty-spot menu (Graphite mockup #62).
    select(&mut app, Vec::new());
    let _ = press(&mut app, "redo");
    assert!(doc(&app).symbols.is_empty(), "redo did not delete it again");
    assert_menu_closed(&app, "redo");
}

#[test]
fn duplicate_row_places_a_copy_of_the_selection() {
    let mut app = one_symbol();
    let task = press(&mut app, "duplicate");
    assert_menu_closed(&app, "duplicate");
    let text = app
        .interaction_state
        .clipboard_text
        .clone()
        .unwrap_or_default();
    settle_answering_clipboard(&mut app, task, &text);
    assert_eq!(doc(&app).symbols.len(), 2, "no copy was placed");
}

#[test]
fn select_all_row_selects_every_object_on_the_sheet() {
    let sheet = sheet_of(vec![symbol(1, "R1", 0.0, 0.0), symbol(2, "R2", 20.0, 0.0)]);
    let mut app = app_with(sheet.clone());
    app.interaction_state
        .active_canvas_mut()
        .set_render_cache(Some(
            crate::schematic_runtime::SchematicRenderCache::from_sheet(&sheet),
        ));
    select(&mut app, Vec::new());
    let _ = press(&mut app, "select_all");
    let picked: HashSet<Uuid> = app
        .interaction_state
        .active_canvas()
        .selected
        .iter()
        .map(|i| i.uuid)
        .collect();
    assert!(
        picked.contains(&Uuid::from_u128(1)) && picked.contains(&Uuid::from_u128(2)),
        "{picked:?}"
    );
    assert_menu_closed(&app, "select all");
}

fn highlight() -> crate::app::state::net_nav::NetHighlight {
    crate::app::state::net_nav::NetHighlight {
        project: crate::app::state::ProjectId::test_id(1),
        name: "VCC".to_string(),
        anchor: signex_net::NetAnchor::new(
            signex_net::SheetKey::new("sheet"),
            Uuid::from_u128(1),
            signex_types::schematic::InstancePath::default(),
        ),
        cursor: None,
        elements: std::collections::HashMap::new(),
    }
}

#[test]
fn clear_filter_row_ends_the_net_highlight() {
    let mut app = one_symbol();
    app.document_state.net_nav.highlight = Some(highlight());
    let _ = press(&mut app, "clear_net_highlighting");
    assert!(app.document_state.net_nav.highlight.is_none(), "still lit");
    assert_menu_closed(&app, "clear filter");

    // The same id through the reporting dispatch an agent reads.
    app.document_state.net_nav.highlight = Some(highlight());
    let (outcome, _task) =
        app.dispatch_command_reporting(&id("clear_net_highlighting"), CommandArgs::none(), None);
    assert_eq!(outcome, CommandOutcome::Ran);
    assert!(app.document_state.net_nav.highlight.is_none(), "still lit");
}

#[test]
fn net_color_row_opens_the_net_colour_palette() {
    let mut app = one_symbol();
    let _ = press(&mut app, "open_net_color_palette");
    assert!(app.ui_state.net_color_palette_open, "palette not opened");
    assert_menu_closed(&app, "net color");
}

/// The Highlight Net row is about where the menu was opened, not where the
/// pointer went to pick the row: the wire there is found, so the next thing
/// it says is that the sheet has no project (no project is open here).
#[test]
fn highlight_net_row_aims_at_the_point_the_menu_was_opened_over() {
    let _ = crate::diagnostics::init_logging();
    let wire = Wire {
        color: None,
        uuid: Uuid::from_u128(91),
        start: Point::new(0.0, 20.0),
        end: Point::new(10.0, 20.0),
        stroke_width: 0.0,
        unknown_columns: Default::default(),
        locked: false,
    };
    let mut sheet = sheet_of(Vec::new());
    sheet.wires = vec![wire];
    let mut app = app_with(sheet.clone());
    app.interaction_state
        .active_canvas_mut()
        .set_render_cache(Some(
            crate::schematic_runtime::SchematicRenderCache::from_sheet(&sheet),
        ));
    select(&mut app, Vec::new());
    app.ui_state.cursor_document = app.document_state.active_path.clone();
    app.ui_state.cursor_x = 5.0;
    app.ui_state.cursor_y = 20.0;
    open_menu(&mut app);
    // The cursor moved off to the menu row, away from the wire.
    app.ui_state.cursor_x = 500.0;
    let message = super::row_effects::live_row(&canvas_rows(&app), "highlight_net_under_cursor");
    let _ = app.update(message);
    assert_menu_closed(&app, "highlight net");
    assert!(
        logged(&[
            "Open the project (.snxprj) that owns this sheet",
            "net highlight"
        ]),
        "the wire at the opening point was not taken"
    );
}

#[test]
fn unbuilt_rows_are_greyed_and_say_why() {
    let mut app = one_symbol();
    open_menu(&mut app);
    let i18n = signex_i18n::I18n::en_us_ref();
    let top = canvas_rows(&app);
    // The planned rows sit in the "Planned (N)" flyout, not the menu (mockup
    // row 62); Unions is live (mockup row 158),
    // `the_unions_row_opens_a_live_submenu`.
    for label in ["References", "Pin Mapping...", "Supplier Links..."] {
        assert!(
            !top.iter()
                .any(|e| matches!(e, DropdownEntry::Item(it) if it.label == label)),
            "{label} is in the menu itself"
        );
    }
    let mut rows = top;
    rows.extend(super::super::submenu::planned_entries(i18n, TID));
    for (label, reason) in [
        ("References", menu_ids::CTX_REFERENCES_PLANNED),
        ("Pin Mapping...", menu_ids::CTX_PIN_MAPPING_PLANNED),
        ("Supplier Links...", menu_ids::CTX_SUPPLIER_LINKS_PLANNED),
        (
            "Assign Net Class...",
            menu_ids::CTX_ASSIGN_NET_CLASS_ELSEWHERE,
        ),
    ] {
        let row = item(&rows, label);
        assert!(row.disabled && row.on_press.is_none(), "{label} is live");
        let expected = i18n.message(reason, &[]).into_string();
        assert_eq!(row.tooltip.as_deref(), Some(expected.as_str()), "{label}");
    }
}

/// Mockup row 158: Unions is a live launcher now (it was a planned stub), and its
/// flyout lists Create, Add, Remove and Break; Create is live with a
/// selection and runs the command, which opens the Create Union popover.
#[test]
fn the_unions_row_opens_a_live_submenu() {
    let mut app = one_symbol();
    open_menu(&mut app);
    let rows = canvas_rows(&app);
    let unions = item(&rows, "Unions");
    assert!(
        unions.submenu && unions.on_hover.is_some(),
        "Unions is a launcher"
    );
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::SubmenuOpen(
        ContextSubmenu::Unions,
    )));
    assert_eq!(
        app.interaction_state.context_submenu,
        Some(ContextSubmenu::Unions)
    );
    let (selection, _) = app.canvas_menu_inputs(app.ui_state.main_window_id.expect("main"));
    let hint = |_: &str| String::new();
    let flyout =
        super::super::menus::unions_entries(signex_i18n::I18n::en_us_ref(), selection, &hint);
    let labels: Vec<&str> = flyout
        .iter()
        .filter_map(|e| match e {
            DropdownEntry::Item(it) => Some(it.label.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        labels,
        [
            "Create Union…",
            "Add Selection to Union",
            "Remove Selection from Union",
            "Break Union"
        ]
    );
    let create = item(&flyout, "Create Union…");
    assert!(!create.disabled, "a selection can become a union");
    let remove = item(&flyout, "Remove Selection from Union");
    assert!(remove.disabled, "the selection is in no union");
    let pressed = create.on_press.clone().expect("live row");
    let _ = app.update(pressed);
    // The command goes round the loop once more before the popover opens.
    let _ = app.update(Message::Edit(crate::app::EditMsg::Unions(
        crate::app::unions::UnionsMsg::CreateRequested,
    )));
    assert!(app.ui_state.unions.popover.is_some(), "the popover opened");
}

#[test]
fn part_and_sheet_actions_open_as_submenus() {
    let mut app = one_symbol();
    for kind in [ContextSubmenu::PartActions, ContextSubmenu::SheetActions] {
        open_menu(&mut app);
        let _ = app.update(Message::ContextMenu(ContextMenuMsg::SubmenuOpen(kind)));
        assert_eq!(app.interaction_state.context_submenu, Some(kind));
    }
    // The Part Actions rows reach the placed-part flow for the selected part.
    let _ = crate::diagnostics::init_logging();
    let mut app = app_with(sheet_of(vec![symbol(1, "PA2419", 10.0, 10.0)]));
    select(
        &mut app,
        vec![SelectedItem::new(Uuid::from_u128(1), SelectedKind::Symbol)],
    );
    let _ = press(&mut app, "open_datasheet");
    assert!(logged(&["PA2419"]), "the row did not reach the part");
    assert_menu_closed(&app, "open datasheet");
}
