//! #3038, the headless path of the Variant Manager's part edits: a cell's
//! fit, an alternate part, a parameter override and Duplicate run as
//! catalog commands with operands, each one undo step of the General
//! history, and make the change the tab's own click makes.

use serde_json::Value;
use signex_i18n::RenderTranslatable;
use signex_types::project::{FOOTPRINT_PARAMETER, MPN_PARAMETER, VALUE_PARAMETER};
use signex_types::schematic::{ChildSheet, FillType, Point, SchematicSheet, Symbol};
use uuid::Uuid;

use super::manager::VariantManagerMsg;
use super::manager_tests::{app, cell, data, manage, open_manager, settle};
use crate::app::Signex;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::{app_workspace, open_with};
use crate::app::state::ProjectId;
use crate::keymap::AppCommandId;

fn run(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let document = format!("document={}", app.document_state.projects[0].path.display());
    let mut all = vec![document.as_str()];
    all.extend_from_slice(operands);
    let command = AppCommandId::new(id).expect("a catalog id");
    let args = CommandArgs::parse_operands(&command, &all).expect("operands parse");
    app.run_operands(&command, &args).0
}

fn answer(app: &mut Signex, id: &str, operands: &[&str]) -> Value {
    match run(app, id, operands) {
        CommandOutcome::Answered(value) => value,
        other => panic!("{id} did not answer: {other:?}"),
    }
}

fn refusal(app: &mut Signex, id: &str, operands: &[&str]) -> String {
    match run(app, id, operands) {
        CommandOutcome::Disabled { reason } => reason.render_en_us(),
        other => panic!("{id} was not refused: {other:?}"),
    }
}

fn settings(app: &Signex) -> signex_types::project::VariantSettings {
    data(app).variant_settings.clone()
}

/// The fit command stores what the Fitting switch stores, on the placed
/// part, as one undo step; a pick the sheets already give records nothing.
#[test]
fn a_fit_matches_the_fitting_switch_and_is_one_undo_step() {
    let (mut app, id, _dir) = app();
    open_manager(&mut app, id);
    let _ = manage(
        &mut app,
        id,
        VariantManagerMsg::FitPicked {
            cell: cell(Some("Lite"), "R1"),
            fitted: Some(false),
        },
    );
    let by_click = settings(&app);
    assert_eq!(by_click.parts.len(), 1);
    assert!(app.step_project_tab_history(id, true));
    assert!(settings(&app).is_empty());

    let fit = ["variant=Lite", "designator=R1", "fitted=false"];
    let value = answer(&mut app, "project_set_variant_fit", &fit);
    assert_eq!(value["changed"], true);
    assert_eq!(value["parts"][0]["designator"], "R1");
    assert_eq!(value["parts"][0]["fitted"], false);
    assert_eq!(settings(&app), by_click, "the same entry, on the same part");
    assert!(app.step_project_tab_history(id, true));
    assert!(settings(&app).is_empty(), "one step");

    // R2 is left out of Lite on its sheet: picking that again stores nothing.
    let again = ["variant=Lite", "designator=R2", "fitted=false"];
    let value = answer(&mut app, "project_set_variant_fit", &again);
    assert_eq!(value["changed"], false);
    assert!(settings(&app).is_empty());
    assert!(!app.project_tab_history_available(id).0);
}

/// No `fitted` lets the sheets decide again.
#[test]
fn a_fit_with_no_fitted_is_as_on_the_sheets() {
    let (mut app, _id, _dir) = app();
    let fit = ["variant=Lite", "designator=R1", "fitted=false"];
    let _ = answer(&mut app, "project_set_variant_fit", &fit);
    let value = answer(
        &mut app,
        "project_set_variant_fit",
        &["variant=Lite", "designator=R1"],
    );
    assert_eq!(value["changed"], true);
    assert_eq!(value["parts"][0]["fitted"], Value::Null);
    assert!(settings(&app).is_empty());
}

/// The override command stores what Enter in the overrides table stores;
/// an empty value is the Base value again.
#[test]
fn an_override_matches_the_overrides_table() {
    let (mut app, id, _dir) = app();
    open_manager(&mut app, id);
    let r1 = cell(Some("Lite"), "R1");
    let _ = manage(&mut app, id, VariantManagerMsg::CellSelected(r1.clone()));
    let _ = manage(
        &mut app,
        id,
        VariantManagerMsg::OverrideEdited {
            parameter: VALUE_PARAMETER.into(),
            text: "4k7".into(),
        },
    );
    let submitted = VariantManagerMsg::OverrideSubmitted { cell: r1 };
    let _ = manage(&mut app, id, submitted);
    let by_click = settings(&app);
    assert!(app.step_project_tab_history(id, true));

    let set = ["variant=Lite", "designator=R1", "field=Value", "value=4k7"];
    let value = answer(&mut app, "project_set_variant_override", &set);
    assert_eq!(value["changed"], true);
    assert_eq!(value["parts"][0]["parameters"]["Value"], "4k7");
    assert_eq!(settings(&app), by_click);

    let clear = ["variant=Lite", "designator=R1", "field=Value"];
    let _ = answer(&mut app, "project_set_variant_override", &clear);
    assert!(settings(&app).is_empty());
    assert!(app.step_project_tab_history(id, true), "a step");
    assert_eq!(settings(&app), by_click);
}

/// An alternate part's part number, value and footprint are one step; a
/// value with no part number is refused and changes nothing.
#[test]
fn an_alternate_sets_its_part_number_value_and_footprint_in_one_step() {
    let (mut app, id, _dir) = app();
    let alternate = [
        "variant=Lite",
        "designator=R1",
        "mpn=RC0402FR-0747KL",
        "value=47k",
        "footprint=R_0402",
    ];
    let value = answer(&mut app, "project_set_variant_alternate", &alternate);
    assert_eq!(value["changed"], true);
    let entry = settings(&app);
    let own = &entry.parts[0].parameters;
    assert_eq!(own[MPN_PARAMETER], "RC0402FR-0747KL");
    assert_eq!(own[VALUE_PARAMETER], "47k");
    assert_eq!(own[FOOTPRINT_PARAMETER], "R_0402");
    assert!(app.step_project_tab_history(id, true));
    assert!(settings(&app).is_empty(), "one step takes all three");

    let no_mpn = ["variant=Lite", "designator=R1", "value=47k"];
    let refused = refusal(&mut app, "project_set_variant_alternate", &no_mpn);
    assert!(refused.contains("part number"), "{refused}");
    assert!(settings(&app).is_empty());

    let _ = answer(&mut app, "project_set_variant_alternate", &alternate);
    let back = ["variant=Lite", "designator=R1"];
    let _ = answer(&mut app, "project_set_variant_alternate", &back);
    assert!(settings(&app).is_empty(), "none given is the part's own");
}

/// The duplicate command adds the copy Duplicate adds, with the fits its
/// sheets give, under the same first free name; `value` names it.
#[test]
fn a_duplicate_matches_duplicate() {
    let (mut app, id, _dir) = app();
    open_manager(&mut app, id);
    let task = manage(
        &mut app,
        id,
        VariantManagerMsg::DuplicateRequested {
            from: Some("Lite".into()),
        },
    );
    settle(&mut app, task);
    let (definitions, by_click) = (data(&app).variant_definitions.clone(), settings(&app));
    assert!(app.step_project_tab_history(id, true));

    let value = answer(&mut app, "project_duplicate_variant", &["name=lite"]);
    assert_eq!(value["changed"], true);
    assert_eq!(value["name"], "Lite copy");
    assert_eq!(data(&app).variant_definitions, definitions);
    assert_eq!(settings(&app), by_click);
    assert!(app.step_project_tab_history(id, true));
    assert_eq!(data(&app).variant_definitions, ["Lite"], "one step");

    let spare = ["name=Base", "value=Spare"];
    let value = answer(&mut app, "project_duplicate_variant", &spare);
    assert_eq!(value["name"], "Spare");
    assert_eq!(data(&app).variant_definitions, ["Lite", "Spare"]);
    assert!(settings(&app).is_empty(), "a copy of Base changes nothing");
}

#[test]
fn an_unknown_part_a_bad_scope_and_base_are_refused() {
    let (mut app, _id, _dir) = app();
    let refused = refusal(
        &mut app,
        "project_set_variant_fit",
        &["variant=Lite", "designator=R9", "fitted=false"],
    );
    assert!(refused.contains("no part R9"), "{refused}");
    let refused = refusal(
        &mut app,
        "project_set_variant_fit",
        &["variant=Lite", "designator=R1", "scope=all"],
    );
    assert!(refused.contains("part or channels"), "{refused}");
    let refused = refusal(
        &mut app,
        "project_set_variant_fit",
        &["variant=Lite", "designator=R1", "scope=channels"],
    );
    assert!(refused.contains("not on a repeated sheet"), "{refused}");
    let outcome = run(
        &mut app,
        "project_set_variant_override",
        &["variant=Base", "designator=R1", "field=Value", "value=2k"],
    );
    assert!(
        matches!(outcome, CommandOutcome::Disabled { .. }),
        "{outcome:?}"
    );
    assert!(settings(&app).is_empty());
}

/// `a.snxsch` places `child.snxsch` as `Repeat(CH, 1, 3)`; the child holds
/// R5. Both are open; Lite is defined. The folder lives as long as the guard.
pub(super) fn repeated() -> (Signex, ProjectId, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("a folder");
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["a.snxsch", "child.snxsch"]);
    let root = SchematicSheet {
        child_sheets: vec![ChildSheet {
            uuid: Uuid::from_u128(0x3038_0001),
            name: "Repeat(CH, 1, 3)".to_string(),
            filename: "child.snxsch".to_string(),
            position: Point::new(20.0, 20.0),
            size: (40.64, 43.18),
            stroke_width: 0.0,
            fill: FillType::None,
            stroke_color: None,
            fill_color: None,
            fields_autoplaced: false,
            pins: Vec::new(),
            instances: Vec::new(),
        }],
        ..Default::default()
    };
    let child = SchematicSheet {
        symbols: vec![Symbol {
            uuid: Uuid::from_u128(0x3038_0005),
            lib_id: "Device:R".to_string(),
            reference: "R5".to_string(),
            value: "31.6k".to_string(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let (root_path, child_path) = (dir.path().join("a.snxsch"), dir.path().join("child.snxsch"));
    open_with(&mut app.document_state, &root_path, root);
    open_with(&mut app.document_state, &child_path, child);
    app.document_state.projects[0].data.variant_definitions = vec!["Lite".to_string()];
    let id = app.document_state.projects[0].id;
    (app, id, dir)
}

/// #2662: a channel's copy is set on its own; `scope=channels` sets every
/// channel's copy at once, one undo step.
#[test]
fn a_channels_copy_is_set_alone_and_every_channel_in_one_step() {
    let (mut app, id, _dir) = repeated();
    let one = ["variant=Lite", "designator=CH2_R5", "fitted=false"];
    let value = answer(&mut app, "project_set_variant_fit", &one);
    assert_eq!(value["changed"], true);
    let entries = settings(&app).parts;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].designator, "CH2_R5");
    assert!(app.step_project_tab_history(id, true));

    let all = [
        "variant=Lite",
        "designator=CH2_R5",
        "fitted=false",
        "scope=channels",
    ];
    let value = answer(&mut app, "project_set_variant_fit", &all);
    let listed: Vec<&str> = value["parts"]
        .as_array()
        .expect("the parts set")
        .iter()
        .filter_map(|part| part["designator"].as_str())
        .collect();
    assert_eq!(listed, ["CH1_R5", "CH2_R5", "CH3_R5"]);
    assert_eq!(settings(&app).parts.len(), 3);
    assert!(app.step_project_tab_history(id, true));
    assert!(settings(&app).is_empty(), "one step takes every channel");
}

/// Mockup rows 208 and 209: `scope=channels` gives every channel's copy the
/// alternate, or the value, at once, one undo step each; the canvas's
/// picker and popover apply the same section command.
#[test]
fn an_alternate_and_a_value_for_every_channel_are_one_step_each() {
    let (mut app, id, _dir) = repeated();
    let alternate = [
        "variant=Lite",
        "designator=CH1_R5",
        "mpn=RC0603FR-0747KL",
        "scope=channels",
    ];
    let value = answer(&mut app, "project_set_variant_alternate", &alternate);
    assert_eq!(value["changed"], true);
    assert_eq!(value["parts"].as_array().map(Vec::len), Some(3));
    let entries = settings(&app).parts;
    assert_eq!(entries.len(), 3);
    assert!(
        entries
            .iter()
            .all(|entry| entry.parameters[MPN_PARAMETER] == "RC0603FR-0747KL")
    );
    assert!(app.step_project_tab_history(id, true));
    assert!(settings(&app).is_empty(), "one step takes every channel");

    let set = [
        "variant=Lite",
        "designator=CH2_R5",
        "field=Value",
        "value=47k",
        "scope=channels",
    ];
    let _ = answer(&mut app, "project_set_variant_override", &set);
    let entries = settings(&app).parts;
    assert_eq!(entries.len(), 3);
    assert!(
        entries
            .iter()
            .all(|entry| entry.parameters[VALUE_PARAMETER] == "47k")
    );
    assert!(app.step_project_tab_history(id, true));
    assert!(settings(&app).is_empty(), "one step");

    // `part` (the default) sets the copy named only.
    let one = [
        "variant=Lite",
        "designator=CH2_R5",
        "field=Value",
        "value=47k",
    ];
    let _ = answer(&mut app, "project_set_variant_override", &one);
    assert_eq!(settings(&app).parts.len(), 1);
}

/// A bad scope, and every channel of a part on a plain sheet, are refused
/// for the alternate and the value as for the fit, and change nothing.
#[test]
fn an_alternate_or_a_value_with_a_bad_scope_is_refused() {
    let (mut app, _id, _dir) = app();
    let refused = refusal(
        &mut app,
        "project_set_variant_alternate",
        &["variant=Lite", "designator=R1", "mpn=X1", "scope=all"],
    );
    assert!(refused.contains("part or channels"), "{refused}");
    let refused = refusal(
        &mut app,
        "project_set_variant_override",
        &[
            "variant=Lite",
            "designator=R1",
            "field=Value",
            "value=2k",
            "scope=channels",
        ],
    );
    assert!(refused.contains("not on a repeated sheet"), "{refused}");
    assert!(settings(&app).is_empty());
}
