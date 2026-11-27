//! #2948: the canvas draws the project's active variant's own value for a
//! part, picked in the Variant Manager or by `project_set_active_variant`;
//! Base and a cleared value draw the sheet's value again.

use std::collections::BTreeMap;

use signex_types::project::{VALUE_PARAMETER, VariantPart, VariantSettings};
use signex_types::schematic::{SchematicSheet, Symbol};
use uuid::Uuid;

use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::{app_workspace, open_with};
use crate::app::{Message, ProjectMsg, ProjectTabMsg, Signex};
use crate::keymap::AppCommandId;

/// A project with the open sheet `a.snxsch` (R1 10k, active) and the variant
/// Lite, which gives R1 the value 47k. The folder lives as long as the guard.
fn app() -> (Signex, Uuid, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("a folder");
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["a.snxsch"]);
    let r1 = Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "Device:R".to_string(),
        reference: "R1".to_string(),
        value: "10k".to_string(),
        ..Default::default()
    };
    let uuid = r1.uuid;
    let path = dir.path().join("a.snxsch");
    let sheet = SchematicSheet {
        symbols: vec![r1],
        ..Default::default()
    };
    open_with(&mut app.document_state, &path, sheet);
    app.document_state.active_path = Some(path);
    let data = &mut app.document_state.projects[0].data;
    data.variant_definitions = vec!["Lite".to_string()];
    data.variant_settings = VariantSettings {
        parts: vec![VariantPart {
            variant: "Lite".into(),
            designator: "R1".into(),
            parameters: BTreeMap::from([(VALUE_PARAMETER.to_string(), "47k".to_string())]),
            ..VariantPart::default()
        }],
        ..VariantSettings::default()
    };
    (app, uuid, dir)
}

fn canvas_value(app: &Signex, uuid: Uuid) -> Option<&str> {
    app.interaction_state
        .active_canvas()
        .variant_values
        .get(&uuid)
        .map(String::as_str)
}

/// The value text the canvas draws for the active sheet's `uuid`.
fn drawn(app: &Signex, uuid: Uuid) -> String {
    let canvas = app.interaction_state.active_canvas();
    let engine = app.document_state.active_engine().expect("an open sheet");
    let symbols = &engine.document().symbols;
    let symbol = symbols.iter().find(|s| s.uuid == uuid).expect("the part");
    crate::schematic_runtime::drawn_value::drawn_value(
        symbols,
        symbol,
        None,
        None,
        Some(&canvas.variant_values),
    )
}

#[test]
fn base_draws_the_sheet_value() {
    let (mut app, r1, _dir) = app();
    app.refresh_all_variant_values();
    assert_eq!(canvas_value(&app, r1), None);
    assert_eq!(drawn(&app, r1), "10k");
}

#[test]
fn the_active_variant_draws_its_value_and_base_draws_the_sheet_again() {
    let (mut app, r1, _dir) = app();
    app.document_state.projects[0].data.active_variant = Some("Lite".to_string());
    app.refresh_all_variant_values();
    assert_eq!(canvas_value(&app, r1), Some("47k"));
    assert_eq!(drawn(&app, r1), "47k");

    app.document_state.projects[0].data.active_variant = None;
    app.refresh_all_variant_values();
    assert_eq!(drawn(&app, r1), "10k");
}

#[test]
fn a_cleared_value_draws_the_sheet_value_again() {
    let (mut app, r1, _dir) = app();
    app.document_state.projects[0].data.active_variant = Some("Lite".to_string());
    app.refresh_all_variant_values();
    assert_eq!(drawn(&app, r1), "47k");
    app.document_state.projects[0]
        .data
        .set_variant_part_parameter("Lite", "R1", VALUE_PARAMETER, None)
        .expect("the value is cleared");
    app.refresh_all_variant_values();
    assert_eq!(canvas_value(&app, r1), None);
    assert_eq!(drawn(&app, r1), "10k");
}

/// The GUI path: the Variant Manager's Active variant select, through
/// `update`, redraws the canvas with Lite's value.
#[test]
fn picking_the_active_variant_in_the_manager_redraws_the_canvas() {
    let (mut app, r1, _dir) = app();
    let project_id = app.document_state.projects[0].id;
    let _ = app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::ActiveVariantPicked {
            project_id,
            name: Some("Lite".to_string()),
        },
    )));
    app.refresh_panel_ctx();
    assert_eq!(canvas_value(&app, r1), Some("47k"));
}

/// The headless path: `project_set_active_variant` picks Lite as the select
/// does, and the canvas follows.
#[test]
fn the_active_variant_command_redraws_the_canvas() {
    let (mut app, r1, _dir) = app();
    let document = format!("document={}", app.document_state.projects[0].path.display());
    let command = AppCommandId::new("project_set_active_variant").expect("a catalog id");
    let args = CommandArgs::parse_operands(&command, &[document.as_str(), "name=Lite"])
        .expect("operands parse");
    let (outcome, _) = app.run_operands(&command, &args);
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
    app.refresh_panel_ctx();
    assert_eq!(canvas_value(&app, r1), Some("47k"));
}

/// The Value the Object List lists for the active sheet's part `designator`.
fn listed_value(app: &Signex, designator: &str) -> String {
    use crate::panels::object_list::model::{RowName, RowValue};
    let rows = &app.ui_state.object_list.source.rows;
    let row = (rows.iter())
        .find(|r| matches!(&r.name, RowName::Text(n) if n == designator))
        .unwrap_or_else(|| panic!("{designator} is listed: {rows:?}"));
    match &row.value {
        RowValue::Text(text) => text.clone(),
        other => panic!("{designator}'s value is text: {other:?}"),
    }
}

/// The Object List shows the value the canvas draws: Lite's own value for
/// R1, then an `=` variant value evaluated and formatted as the canvas
/// draws it, then Base's sheet value again.
#[test]
fn the_object_list_shows_the_value_the_canvas_draws() {
    let (mut app, r1, _dir) = app();
    let r2 = Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "Device:R".to_string(),
        reference: "R2".to_string(),
        value: "10kΩ".to_string(),
        ..Default::default()
    };
    let path = app.document_state.active_path.clone().expect("a sheet");
    let mut sheet = app
        .document_state
        .active_engine()
        .expect("open")
        .document()
        .clone();
    sheet.symbols.push(r2);
    open_with(&mut app.document_state, &path, sheet);
    app.document_state.tabs.push(crate::app::TabInfo {
        title: "a".to_string(),
        path: path.clone(),
        project_id: None,
        kind: crate::app::TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(path);
    app.document_view.dock.add_panel(
        crate::dock::PanelPosition::Right,
        crate::panels::PanelKind::ObjectList,
    );
    app.ui_state.object_list.scope = crate::panels::ObjectScope::ThisSheet;
    let settle = |app: &mut Signex| {
        app.refresh_all_variant_values();
        let _ = app.sync_object_list();
    };

    app.document_state.projects[0].data.active_variant = Some("Lite".to_string());
    settle(&mut app);
    assert_eq!(drawn(&app, r1), "47k");
    assert_eq!(listed_value(&app, "R1"), drawn(&app, r1));

    app.document_state.projects[0]
        .data
        .set_variant_part_parameter("Lite", "R1", VALUE_PARAMETER, Some("= 2 * R2.Value"))
        .expect("the value is set");
    settle(&mut app);
    assert_eq!(drawn(&app, r1), "20kΩ");
    assert_eq!(listed_value(&app, "R1"), drawn(&app, r1));

    app.document_state.projects[0].data.active_variant = None;
    settle(&mut app);
    assert_eq!(listed_value(&app, "R1"), "10k");
    assert_eq!(listed_value(&app, "R1"), drawn(&app, r1));
}
