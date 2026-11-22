//! Mockup row 152: signal harnesses. The three tools, the connector's
//! Properties, the mismatch fix-it, a harness-typed sheet entry and the
//! project's harness definitions.

use signex_app::active_bar::ActiveBarAction;
use signex_app::panels::{HarnessField, HarnessPanelMsg, PanelMsg};
use signex_types::schematic::{
    ChildSheet, FillType, HarnessConnector, HarnessDefinition, HarnessDefinitionEntry,
    HarnessEntry, PageOrigin, Point, SheetPin,
};
use uuid::Uuid;

use super::support::{Bench, ProjectBench, bench, empty_sheet, project_bench, sheet_symbol};

fn spi_connector() -> HarnessConnector {
    HarnessConnector {
        uuid: Uuid::now_v7(),
        harness_type: "SPI_BUS".to_string(),
        position: Point::new(63.5, 16.51),
        width: 10.16,
        entry_pitch: 2.54,
        rotation: 0.0,
        entries: ["SCK", "MOSI", "MISO", "CS"]
            .iter()
            .map(|name| HarnessEntry {
                uuid: Uuid::now_v7(),
                name: (*name).to_string(),
            })
            .collect(),
        locked: false,
    }
}

fn connector_bench() -> (Bench, Uuid) {
    let mut sheet = empty_sheet();
    let connector = spi_connector();
    let id = connector.uuid;
    sheet.harness.connectors.push(connector);
    (bench(sheet), id)
}

fn harness(b: &mut Bench, msg: HarnessPanelMsg) {
    b.panel(PanelMsg::Harness(msg));
}

fn type_field(b: &mut Bench, connector: Uuid, field: HarnessField, text: &str) {
    harness(
        b,
        HarnessPanelMsg::FieldTyped {
            connector,
            field,
            text: text.to_string(),
        },
    );
}

fn connector(b: &Bench) -> &HarnessConnector {
    &b.sheet().harness.connectors[0]
}

#[test]
fn m152_two_clicks_place_a_connector_and_undo_takes_it_away() {
    let mut b = bench(empty_sheet());
    b.arm(ActiveBarAction::PlaceHarnessConnector);
    b.click(10.0, 10.0);
    assert!(b.sheet().harness.connectors.is_empty(), "the corner only");
    b.click(20.16, 10.0);
    assert_eq!(b.sheet().harness.connectors.len(), 1);
    assert!((connector(&b).width - 10.16).abs() < 1e-9);
    assert!(b.dirty());
    b.undo();
    assert!(b.sheet().harness.connectors.is_empty());
    assert!(!b.dirty());
}

#[test]
fn m152_the_entry_tool_adds_an_entry_on_a_connector_and_nothing_off_one() {
    let mut b = bench(empty_sheet());
    b.arm(ActiveBarAction::PlaceHarnessConnector);
    b.click(10.0, 10.0);
    b.click(20.16, 10.0);
    b.arm(ActiveBarAction::PlaceHarnessEntry);
    b.click(10.5, 11.0);
    assert_eq!(connector(&b).entries.len(), 1);
    b.click(200.0, 200.0);
    assert_eq!(connector(&b).entries.len(), 1, "off a connector: nothing");
    b.undo();
    assert!(connector(&b).entries.is_empty());
}

#[test]
fn m152_the_harness_wire_tool_draws_a_harness_wire_and_no_plain_wire() {
    let mut b = bench(empty_sheet());
    b.arm(ActiveBarAction::PlaceSignalHarness);
    b.click(40.0, 20.0);
    b.click(60.0, 20.0);
    let wires = &b.sheet().harness.wires;
    assert_eq!(wires.len(), 1);
    assert_eq!(wires[0].start, Point::new(40.0, 20.0));
    assert_eq!(wires[0].end, Point::new(60.0, 20.0));
    assert!(b.sheet().wires.is_empty(), "no plain wire is drawn");
    assert!(b.dirty());
}

#[test]
fn m152_typing_x_width_and_pitch_edits_the_connector_in_one_undo_step() {
    let (mut b, id) = connector_bench();
    type_field(&mut b, id, HarnessField::X, "70");
    type_field(&mut b, id, HarnessField::Width, "12.7");
    type_field(&mut b, id, HarnessField::EntryPitch, "3.81");
    assert_eq!(connector(&b).position.x, 70.0);
    assert_eq!(connector(&b).width, 12.7);
    assert_eq!(connector(&b).entry_pitch, 3.81);
    assert!(b.dirty());
    b.undo();
    assert_eq!(connector(&b).position.x, 63.5);
    assert_eq!(connector(&b).width, 10.16);
    assert_eq!(connector(&b).entry_pitch, 2.54);
    assert!(!b.dirty());
}

/// Inference: the row only draws an X / Y box. Every other schematic Y box
/// counts from the sheet's page origin, so this one is expected to as well.
#[test]
fn m152_the_y_box_counts_from_the_page_origin_like_the_other_y_boxes() {
    let mut sheet = empty_sheet();
    sheet.page.origin = PageOrigin::LowerLeft;
    let c = spi_connector();
    let id = c.uuid;
    sheet.harness.connectors.push(c);
    let mut b = bench(sheet);
    type_field(&mut b, id, HarnessField::Y, "20");
    let expected = b.sheet().page_y(20.0);
    assert!(
        (connector(&b).position.y - expected).abs() < 1e-9,
        "typed Y 20 on a Lower Left page is stored Y {expected}, got {}",
        connector(&b).position.y
    );
}

#[test]
fn m152_a_bad_size_leaves_the_connector_and_the_dirty_mark() {
    let (mut b, id) = connector_bench();
    for bad in ["wide", "0", "-2", "nan", "inf"] {
        type_field(&mut b, id, HarnessField::Width, bad);
        assert_eq!(connector(&b).width, 10.16, "{bad:?}");
        type_field(&mut b, id, HarnessField::EntryPitch, bad);
        assert_eq!(connector(&b).entry_pitch, 2.54, "{bad:?}");
    }
    assert!(!b.dirty());
}

/// DECISIONS 2026-11-22: a refused number stays in the box as typed.
#[test]
fn m152_a_refused_width_stays_in_the_box_as_typed() {
    let (mut b, id) = connector_bench();
    for bad in ["wide", "0", "-2"] {
        type_field(&mut b, id, HarnessField::Width, bad);
        let draft = b.app.document_state.panel_ctx.harness.draft.clone();
        assert_eq!(
            draft,
            Some((HarnessField::Width, bad.to_string())),
            "the box keeps {bad:?}"
        );
    }
}

#[test]
fn m152_picking_a_type_retypes_the_connector_as_one_step() {
    let (mut b, id) = connector_bench();
    harness(
        &mut b,
        HarnessPanelMsg::TypePicked {
            connector: id,
            harness_type: "UART".to_string(),
        },
    );
    assert_eq!(connector(&b).harness_type, "UART");
    assert!(b.dirty());
    b.undo();
    assert_eq!(connector(&b).harness_type, "SPI_BUS");
    assert!(!b.dirty());
}

#[test]
fn m152_the_rename_fix_renames_the_entry_and_undo_restores_it() {
    let (mut b, id) = connector_bench();
    let entry = connector(&b).entries[3].uuid;
    harness(
        &mut b,
        HarnessPanelMsg::RenameFixClicked {
            connector: id,
            entry,
            to: "CS_ADC".to_string(),
        },
    );
    assert_eq!(connector(&b).entries[3].name, "CS_ADC");
    b.undo();
    assert_eq!(connector(&b).entries[3].name, "CS");
    assert!(!b.dirty());
}

#[test]
fn m152_a_sheet_entry_takes_a_harness_type_and_goes_back_to_plain() {
    let pin = Uuid::now_v7();
    let mut sheet = empty_sheet();
    sheet.child_sheets.push(ChildSheet {
        uuid: Uuid::now_v7(),
        name: "sensor".to_string(),
        filename: "sensor.snxsch".to_string(),
        position: Point::new(50.0, 10.0),
        size: (20.0, 20.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: vec![SheetPin {
            uuid: pin,
            name: "SPI_BUS".to_string(),
            direction: String::new(),
            position: Point::new(50.0, 20.0),
            rotation: 0.0,
            auto_generated: false,
            user_moved: false,
        }],
        instances: Vec::new(),
    });
    let mut b = bench(sheet);
    harness(
        &mut b,
        HarnessPanelMsg::SheetEntryTypePicked {
            pin,
            harness_type: Some("SPI_BUS".to_string()),
        },
    );
    assert_eq!(b.sheet().harness.entry_type(pin), Some("SPI_BUS"));
    assert!(b.dirty());
    harness(
        &mut b,
        HarnessPanelMsg::SheetEntryTypePicked {
            pin,
            harness_type: None,
        },
    );
    assert_eq!(b.sheet().harness.entry_type(pin), None);
}

fn definitions_bench() -> ProjectBench {
    let mut root = empty_sheet();
    root.child_sheets.push(sheet_symbol("sensor"));
    root.harness.connectors.push(spi_connector());
    let mut b = project_bench(root, empty_sheet());
    b.data_mut().schematic_design.harness_definitions = vec![HarnessDefinition {
        name: "SPI_BUS".to_string(),
        description: "Sensor SPI, mode 0".to_string(),
        entries: ["SCK", "MOSI", "MISO", "CS"]
            .iter()
            .map(|name| HarnessDefinitionEntry {
                name: (*name).to_string(),
                harness: None,
            })
            .collect(),
    }];
    b
}

fn definitions(b: &ProjectBench) -> &[HarnessDefinition] {
    &b.data().schematic_design.harness_definitions
}

fn definition_msg(b: &mut ProjectBench, msg: HarnessPanelMsg) {
    b.panel(PanelMsg::Harness(msg));
}

#[test]
fn m152_the_definitions_list_counts_the_uses_of_each_type() {
    let mut b = definitions_bench();
    definition_msg(&mut b, HarnessPanelMsg::DefinitionsOpened);
    let panel = &b.app.document_state.panel_ctx.harness;
    assert!(panel.definitions_open);
    assert_eq!(panel.definitions.len(), 1);
    assert_eq!(panel.uses, [1], "one connector uses SPI_BUS");
    definition_msg(&mut b, HarnessPanelMsg::DefinitionsClosed);
    assert!(!b.app.document_state.panel_ctx.harness.definitions_open);
}

#[test]
fn m152_new_harness_type_adds_a_definition_as_one_project_step() {
    let mut b = definitions_bench();
    definition_msg(&mut b, HarnessPanelMsg::NewTypeClicked);
    assert_eq!(definitions(&b).len(), 2);
    assert_eq!(definitions(&b)[1].name, "HARNESS_1");
    assert_eq!(
        b.app.document_state.panel_ctx.harness.selected_definition, 1,
        "the new type is the one listed"
    );
    assert!(b.is_dirty(&b.file));
    assert!(b.undo_project());
    assert_eq!(definitions(&b).len(), 1);
    assert!(!b.project_can_undo(), "it was one step");
}

#[test]
fn m152_renaming_a_type_renames_it_on_every_connector_of_the_open_sheets() {
    let mut b = definitions_bench();
    definition_msg(
        &mut b,
        HarnessPanelMsg::DefinitionNameTyped {
            index: 0,
            text: "SPI_MAIN".to_string(),
        },
    );
    assert_eq!(definitions(&b)[0].name, "SPI_MAIN");
    let root = b.root.clone();
    assert_eq!(b.doc(&root).harness.connectors[0].harness_type, "SPI_MAIN");
}

#[test]
fn m152_a_definitions_description_and_entry_are_edited_in_the_project() {
    let mut b = definitions_bench();
    definition_msg(
        &mut b,
        HarnessPanelMsg::DefinitionDescriptionTyped {
            index: 0,
            text: "Sensor SPI, mode 3".to_string(),
        },
    );
    definition_msg(
        &mut b,
        HarnessPanelMsg::DefinitionEntryTyped {
            index: 0,
            entry: 3,
            text: "CS_ADC".to_string(),
        },
    );
    assert_eq!(definitions(&b)[0].description, "Sensor SPI, mode 3");
    assert_eq!(definitions(&b)[0].entries[3].name, "CS_ADC");
}

#[test]
fn m152_go_to_opens_the_sheet_the_mismatch_names() {
    let mut b = definitions_bench();
    b.send(signex_app::app::Message::Dock(
        signex_app::dock::DockMessage::Panel(PanelMsg::Harness(
            HarnessPanelMsg::GoToSheetClicked {
                far: "sensor".to_string(),
            },
        )),
    ));
    assert_eq!(
        b.app.document_state.active_path.as_deref(),
        Some(b.child.as_path())
    );
}
