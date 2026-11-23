//! #1280 — the Signal Harness, Harness Connector and Harness Entry tools
//! place real objects (each one undo step), the harness Properties edit a
//! connector, the mismatch fix-it renames an entry, and a sheet entry takes a
//! harness type.

use signex_app::active_bar::{ActiveBarAction, ActiveBarMsg, is_planned};
use signex_app::app::{Message, Signex, TabInfo, TabKind};
use signex_app::canvas::CanvasEvent;
use signex_app::dock::DockMessage;
use signex_app::panels::{HarnessField, HarnessPanelMsg, PanelMsg};
use signex_types::schematic::{ChildSheet, FillType, PageOrigin, Point, SchematicSheet, SheetPin};
use std::path::PathBuf;

fn app_with(sheet: SchematicSheet) -> Signex {
    let path = PathBuf::from("harness.snxsch");
    let engine = signex_engine::Engine::new(sheet).expect("engine");
    let (mut app, _initial_task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.snap_enabled = false;
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "harness".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app
}

fn empty() -> SchematicSheet {
    SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    }
}

fn doc(app: &Signex) -> &SchematicSheet {
    app.document_state
        .engines
        .values()
        .next()
        .expect("engine")
        .document()
}

fn click(app: &mut Signex, x: f64, y: f64) {
    let _ = app.update(Message::CanvasEvent(CanvasEvent::Clicked {
        world_x: x,
        world_y: y,
    }));
}

fn arm(app: &mut Signex, action: ActiveBarAction) {
    let _ = app.update(Message::ActiveBar(ActiveBarMsg::Action(action)));
}

fn harness(app: &mut Signex, msg: HarnessPanelMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::Harness(msg))));
}

fn undo(app: &mut Signex) {
    let _ = app.update(Message::Edit(signex_app::app::EditMsg::Undo));
}

/// A connector placed with the tool: corner (10, 10), width 10.16.
fn app_with_connector() -> Signex {
    let mut app = app_with(empty());
    arm(&mut app, ActiveBarAction::PlaceHarnessConnector);
    click(&mut app, 10.0, 10.0);
    click(&mut app, 20.16, 10.0);
    app
}

#[test]
fn the_harness_tools_are_live() {
    for action in [
        ActiveBarAction::PlaceSignalHarness,
        ActiveBarAction::PlaceHarnessConnector,
        ActiveBarAction::PlaceHarnessEntry,
    ] {
        assert!(!is_planned(&action), "{action:?} is still planned");
    }
}

#[test]
fn two_clicks_place_a_connector_as_one_undo_step() {
    let mut app = app_with_connector();
    let c = &doc(&app).harness.connectors;
    assert_eq!(c.len(), 1);
    assert!((c[0].width - 10.16).abs() < 1e-9);
    assert_eq!(c[0].position, Point::new(10.0, 10.0));
    assert!(!c[0].harness_type.is_empty());
    undo(&mut app);
    assert!(doc(&app).harness.connectors.is_empty());
}

#[test]
fn a_click_on_a_connector_adds_an_entry() {
    let mut app = app_with_connector();
    arm(&mut app, ActiveBarAction::PlaceHarnessEntry);
    click(&mut app, 10.5, 11.0);
    let entries = &doc(&app).harness.connectors[0].entries;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "ENTRY_1");
    // Off any connector, nothing is placed.
    click(&mut app, 200.0, 200.0);
    assert_eq!(doc(&app).harness.connectors[0].entries.len(), 1);
}

#[test]
fn the_signal_harness_tool_draws_harness_wire_segments() {
    let mut app = app_with(empty());
    arm(&mut app, ActiveBarAction::PlaceSignalHarness);
    click(&mut app, 40.0, 20.0);
    click(&mut app, 60.0, 20.0);
    let wires = &doc(&app).harness.wires;
    assert_eq!(wires.len(), 1);
    assert_eq!(wires[0].start, Point::new(40.0, 20.0));
    assert_eq!(wires[0].end, Point::new(60.0, 20.0));
    assert!(doc(&app).wires.is_empty(), "no plain wire is drawn");
}

#[test]
fn typing_a_width_in_properties_updates_the_connector_as_one_step() {
    let mut app = app_with_connector();
    let id = doc(&app).harness.connectors[0].uuid;
    for text in ["1", "12", "12.7"] {
        harness(
            &mut app,
            HarnessPanelMsg::FieldTyped {
                connector: id,
                field: HarnessField::Width,
                text: text.to_string(),
            },
        );
    }
    assert!((doc(&app).harness.connectors[0].width - 12.7).abs() < 1e-9);
    undo(&mut app);
    assert!((doc(&app).harness.connectors[0].width - 10.16).abs() < 1e-9);
}

/// #2784: Width and Entry Pitch refuse what no connector can hold; the
/// connector stays and the box keeps the text.
#[test]
fn a_size_that_is_not_a_positive_number_is_refused_with_the_text_kept() {
    let mut app = app_with_connector();
    let id = doc(&app).harness.connectors[0].uuid;
    let before = doc(&app).harness.connectors[0].clone();
    for field in [HarnessField::Width, HarnessField::EntryPitch] {
        for bad in ["nan", "inf", "-inf", "-1", "0", "abc"] {
            harness(
                &mut app,
                HarnessPanelMsg::FieldTyped {
                    connector: id,
                    field,
                    text: bad.to_string(),
                },
            );
            assert_eq!(doc(&app).harness.connectors[0], before, "{field:?} {bad}");
            assert_eq!(
                app.document_state.panel_ctx.harness.draft,
                Some((field, bad.to_string())),
                "the box keeps {bad}"
            );
        }
    }
}

/// #2789: the Y box counts from the sheet's page origin.
#[test]
fn a_typed_y_counts_from_the_page_origin() {
    for origin in [PageOrigin::LowerLeft, PageOrigin::UpperLeft] {
        let mut sheet = empty();
        sheet.page.origin = origin;
        let mut app = app_with(sheet);
        arm(&mut app, ActiveBarAction::PlaceHarnessConnector);
        click(&mut app, 10.0, 10.0);
        click(&mut app, 20.16, 10.0);
        let id = doc(&app).harness.connectors[0].uuid;
        harness(
            &mut app,
            HarnessPanelMsg::FieldTyped {
                connector: id,
                field: HarnessField::Y,
                text: "20".to_string(),
            },
        );
        let expected = doc(&app).page_y(20.0);
        let stored = doc(&app).harness.connectors[0].position.y;
        assert!((stored - expected).abs() < 1e-9, "{origin}: {stored}");
    }
}

#[test]
fn the_rename_fix_renames_the_entry_as_one_step() {
    let mut app = app_with_connector();
    arm(&mut app, ActiveBarAction::PlaceHarnessEntry);
    click(&mut app, 10.5, 11.0);
    let c = &doc(&app).harness.connectors[0];
    let (connector, entry) = (c.uuid, c.entries[0].uuid);
    harness(
        &mut app,
        HarnessPanelMsg::RenameFixClicked {
            connector,
            entry,
            to: "CS_ADC".into(),
        },
    );
    assert_eq!(doc(&app).harness.connectors[0].entries[0].name, "CS_ADC");
    undo(&mut app);
    assert_eq!(doc(&app).harness.connectors[0].entries[0].name, "ENTRY_1");
}

#[test]
fn a_sheet_entry_takes_a_harness_type_from_properties() {
    let pin = uuid::Uuid::from_u128(7);
    let mut sheet = empty();
    sheet.child_sheets.push(ChildSheet {
        uuid: uuid::Uuid::from_u128(6),
        name: "sensor".into(),
        filename: "sensor.snxsch".into(),
        position: Point::new(50.0, 10.0),
        size: (20.0, 20.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: vec![SheetPin {
            uuid: pin,
            name: "SPI_BUS".into(),
            direction: String::new(),
            position: Point::new(50.0, 20.0),
            rotation: 0.0,
            auto_generated: false,
            user_moved: false,
        }],
        instances: Vec::new(),
    });
    let mut app = app_with(sheet);
    harness(
        &mut app,
        HarnessPanelMsg::SheetEntryTypePicked {
            pin,
            harness_type: Some("SPI_BUS".into()),
        },
    );
    assert_eq!(doc(&app).harness.entry_type(pin), Some("SPI_BUS"));
    harness(
        &mut app,
        HarnessPanelMsg::SheetEntryTypePicked {
            pin,
            harness_type: None,
        },
    );
    assert_eq!(doc(&app).harness.entry_type(pin), None);
}

#[test]
fn the_definitions_view_opens_and_closes() {
    let mut app = app_with(empty());
    harness(&mut app, HarnessPanelMsg::DefinitionsOpened);
    assert!(app.document_state.panel_ctx.harness.definitions_open);
    harness(&mut app, HarnessPanelMsg::DefinitionsClosed);
    assert!(!app.document_state.panel_ctx.harness.definitions_open);
}
