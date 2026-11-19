//! #2415: Place on a sheet lists every placement tool of the Active Bar,
//! grouped like the bar, planned tools greyed with the reason. #2416: Edit ▸
//! Find Similar Objects, View ▸ Selection Filter and Design ▸ Net Classes.

use super::*;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::keymap::{AppCommandId, CatalogTextExt};

/// The live Place rows #2415 added, and the tool each one arms.
const NEW_TOOL_ROWS: &[(&str, &str)] = &[
    ("Place/Bus Entry", "BusEntry"),
    ("Place/Text Frame", "Text"),
    ("Place/Note", "Text"),
    ("Place/Ports & Connectors/Port", "Label"),
    ("Place/Ports & Connectors/Off Sheet Connector", "Label"),
    ("Place/Ports & Connectors/Sheet Symbol", "Sheet"),
    ("Place/Directives/Generic No ERC", "NoErcDirective"),
    ("Place/Graphics/Arc", "Arc"),
    ("Place/Graphics/Full Circle", "Circle"),
    ("Place/Graphics/Line", "Line"),
    ("Place/Graphics/Rectangle", "Rectangle"),
    ("Place/Graphics/Polygon", "Polyline"),
];

/// The rows #2416 added.
const NEW_SHEET_ROWS: &[&str] = &[
    "Edit/Find Similar Objects...",
    "View/Selection Filter",
    "Design/Net Classes",
];

/// The command ids behind the new rows, for the agent path.
const NEW_COMMANDS: &[&str] = &[
    "place_bus_entry",
    "place_text_frame",
    "place_note",
    "place_port",
    "place_off_sheet_connector",
    "place_sheet_symbol",
    "place_no_erc",
    "draw_arc",
    "draw_full_circle",
    "draw_line",
    "draw_rectangle",
    "draw_polygon",
    "find_similar_objects",
    "open_selection_filter",
    "open_net_classes",
];

#[test]
fn each_new_place_row_arms_its_tool() {
    for (path, tool) in NEW_TOOL_ROWS {
        let mut app = one_symbol_selected();
        assert!(row_is_live(&app, path), "{path} is greyed on a sheet");
        let _ = click(&mut app, path);
        assert_eq!(probe(&app).tool, *tool, "{path}");
    }
}

/// #1267, #1300: Bezier went live; Table and the Dimension tools are new.
#[test]
fn the_annotation_rows_arm_their_tools_and_grey_without_a_sheet() {
    for (path, tool) in [
        ("Place/Graphics/Bezier", "Bezier"),
        ("Place/Graphics/Table", "Table"),
        ("Place/Graphics/Dimension/Linear", "LinearDimension"),
        ("Place/Graphics/Dimension/Aligned", "AlignedDimension"),
    ] {
        let mut app = one_symbol_selected();
        assert!(row_is_live(&app, path), "{path} is greyed on a sheet");
        let _ = click(&mut app, path);
        assert_eq!(probe(&app).tool, tool, "{path}");
        assert!(!row_is_live(&bare_app(), path), "{path} without a sheet");
    }
}

#[test]
fn port_rows_arm_their_port_kind() {
    use signex_types::schematic::LabelType;
    for (path, kind) in [
        ("Place/Ports & Connectors/Port", LabelType::Global),
        (
            "Place/Ports & Connectors/Off Sheet Connector",
            LabelType::Hierarchical,
        ),
    ] {
        let mut app = one_symbol_selected();
        let _ = click(&mut app, path);
        let pending = app.interaction_state.pending_port.as_ref().map(|p| p.0);
        assert_eq!(pending, Some(kind), "{path}");
    }
}

#[test]
fn the_new_rows_are_greyed_without_a_sheet() {
    let app = bare_app();
    for path in NEW_TOOL_ROWS
        .iter()
        .map(|(path, _)| *path)
        .chain(NEW_SHEET_ROWS.iter().copied())
    {
        assert!(!row_is_live(&app, path), "{path} is live with no schematic");
    }
}

/// Every planned tool of the bar is a greyed row with the planned reason.
#[test]
fn planned_tools_are_greyed_with_the_planned_reason() {
    let app = one_symbol_selected();
    let planned = app
        .ui_state
        .i18n
        .message(signex_i18n::menu_ids::MENU_PLANNED_TOOLTIP, &[])
        .to_string();
    let rows = place_items(&app);
    for (action, _) in crate::active_bar::PLANNED_TOOLS {
        let id = crate::app::command::active_bar::id_for_action(action)
            .unwrap_or_else(|| panic!("{action:?} has a command id"));
        let label = crate::menu_bar::cmd_label(&app.ui_state.i18n, id, id);
        let row = rows
            .iter()
            .find(|row| row.label == label)
            .unwrap_or_else(|| panic!("Place lists no row for `{id}` ({label})"));
        assert!(!row.is_enabled(), "{label}");
        assert!(
            matches!(&row.state, ItemState::Placeholder { reason: Some(reason) } if *reason == planned),
            "{label}: {:?}",
            row.state
        );
    }
}

/// Every placement tool of the bar has a Place row on a sheet. Two run
/// under another id in the top rows: the part picker (Component...) and
/// Text String (Text).
#[test]
fn place_lists_every_placement_tool_of_the_active_bar() {
    const UNDER_ANOTHER_ID: &[&str] = &["place_component", "place_text_string"];
    let app = one_symbol_selected();
    let labels: Vec<String> = place_items(&app).into_iter().map(|row| row.label).collect();
    let mut checked = 0;
    for (id, _) in crate::app::command::active_bar::ACTIVE_BAR_COMMANDS {
        let Some(metadata) = crate::keymap::metadata_for_str(id) else {
            continue;
        };
        if metadata.category != "place" || UNDER_ANOTHER_ID.contains(id) {
            continue;
        }
        let label = metadata.menu_label_in(&app.ui_state.i18n);
        assert!(labels.contains(&label), "Place lacks `{id}` ({label})");
        checked += 1;
    }
    assert!(checked >= 30, "only {checked} placement tools checked");
}

/// Place ▸ No Connect runs `place_no_connect`, so it shows the key #2416 gave
/// it (`P X` in the default profile) and still arms the No Connect tool.
#[test]
fn no_connect_row_shows_its_key_and_arms_the_tool() {
    let mut app = one_symbol_selected();
    let command = AppCommandId::new("place_no_connect").expect("a command id");
    let key = app
        .ui_state
        .active_keymap
        .shortcut_label(&command)
        .expect("the default profile binds No Connect");
    let shown = with_row(&app, "Place/No Connect", |row| {
        row.shortcut.as_ref().map(|s| s.text.clone())
    });
    assert_eq!(shown, Some(key));
    let _ = click(&mut app, "Place/No Connect");
    assert_eq!(probe(&app).tool, "NoConnect");
}

#[test]
fn find_similar_objects_opens_its_dialog_from_the_selection() {
    let mut app = one_symbol_selected();
    assert!(row_is_live(&app, "Edit/Find Similar Objects..."));
    let _ = click(&mut app, "Edit/Find Similar Objects...");
    assert!(app.ui_state.find_similar.is_some(), "the dialog opened");
}

#[test]
fn selection_filter_opens_the_active_bars_filter() {
    let mut app = one_symbol_selected();
    let _ = click(&mut app, "View/Selection Filter");
    assert_eq!(
        app.interaction_state.active_bar_menu,
        Some(crate::active_bar::ActiveBarMenu::Filter)
    );
}

#[test]
fn net_classes_opens_the_net_classes_panel() {
    let mut app = one_symbol_selected();
    assert!(
        app.document_state
            .dock
            .locate(crate::panels::PanelKind::NetClasses)
            .is_none(),
        "closed before"
    );
    let _ = click(&mut app, "Design/Net Classes");
    assert!(
        app.document_state
            .dock
            .locate(crate::panels::PanelKind::NetClasses)
            .is_some(),
        "the panel opened"
    );
}

/// The agent path: the same ids report `Ran` on a sheet and are refused with
/// the reason without one.
#[test]
fn the_new_commands_report_their_outcome_to_an_agent() {
    for id in NEW_COMMANDS {
        let command = AppCommandId::new(*id).expect("a command id");
        let mut bare = bare_app();
        let (outcome, _task) = bare.dispatch_command_reporting(&command, CommandArgs::none(), None);
        let reason = match *id {
            "find_similar_objects" => "Nothing is selected on a sheet to find similar objects to",
            // The net classes open on a board too (#2400).
            "open_net_classes" => "No schematic or PCB is open",
            _ => "No schematic is open",
        };
        assert_eq!(
            outcome,
            CommandOutcome::Disabled {
                reason: reason.to_string()
            },
            "{id} without a sheet"
        );
        let mut app = one_symbol_selected();
        let (outcome, _task) = app.dispatch_command_reporting(&command, CommandArgs::none(), None);
        assert_eq!(outcome, CommandOutcome::Ran, "{id} on a sheet");
    }
}

/// Every row of the Place menu, submenus included, top to bottom.
fn place_items(app: &Signex) -> Vec<MenuItem> {
    let ctx = menu_context(app);
    let model = menu_bar_model(&ctx);
    let place = model
        .menus
        .iter()
        .find(|menu| menu.title == "Place")
        .expect("a Place menu");
    let mut out = Vec::new();
    collect_items(place, &mut out);
    out
}

fn collect_items(menu: &Menu, out: &mut Vec<MenuItem>) {
    for entry in &menu.entries {
        match entry {
            MenuEntry::Item(item) => out.push(item.clone()),
            MenuEntry::Submenu(sub) => collect_items(sub, out),
            MenuEntry::Separator => {}
        }
    }
}
