//! #2949, the headless path of the Variant Manager's top bar actions:
//! `compare_variants` answers the comparison the tab shows, `variant_bom`
//! the BOM lines Open in BOM Studio opens on, and
//! `project_set_default_variant` sets the variant defaults as Set as default
//! does, one General step.

use serde_json::Value;
use signex_i18n::RenderTranslatable;

use super::manager::VariantManagerMsg;
use super::manager_tests::{app, data, manage, open_manager};
use crate::app::Signex;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::keymap::AppCommandId;

fn run(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let document = format!("document={}", app.document_state.projects[0].path.display());
    let mut all = vec![document.as_str()];
    all.extend_from_slice(operands);
    let command = AppCommandId::new(id).expect("a catalog id");
    let args = CommandArgs::parse_operands(&command, &all).expect("operands parse");
    match app.run_query(&command, &args) {
        Some((outcome, _)) => outcome,
        None => app.run_operands(&command, &args).0,
    }
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

fn designators(value: &Value) -> Vec<&str> {
    value["parts"]
        .as_array()
        .expect("a parts list")
        .iter()
        .filter_map(|part| part["designator"].as_str())
        .collect()
}

/// With no sides named, Base against the first variant while Base is
/// active: R2, left out of Lite on its sheet, is the one difference.
#[test]
fn compare_variants_answers_base_against_the_first_variant() {
    let (mut app, _id, _dir) = app();
    let value = answer(&mut app, "compare_variants", &[]);
    assert_eq!(value["from"], "Base");
    assert_eq!(value["to"], "Lite");
    assert_eq!(designators(&value), ["R2"]);
    assert_eq!(value["parts"][0]["changes"][0]["kind"], "fit");
    assert_eq!(value["parts"][0]["from"]["fitted"], true);
    assert_eq!(value["parts"][0]["to"]["fitted"], false);
    assert_eq!(
        (value["from_fitted"].as_u64(), value["to_fitted"].as_u64()),
        (Some(2), Some(1))
    );
    assert_eq!(
        value["project"],
        app.document_state.projects[0].path.display().to_string()
    );
}

/// `to` defaults to the active variant; named sides are taken as named.
#[test]
fn compare_variants_takes_the_active_variant_and_named_sides() {
    let (mut app, _id, _dir) = app();
    app.document_state.projects[0].data.active_variant = Some("Lite".into());
    let value = answer(&mut app, "compare_variants", &["from=lite"]);
    assert_eq!(
        (value["from"].as_str(), value["to"].as_str()),
        (Some("Lite"), Some("Lite"))
    );
    assert!(designators(&value).is_empty(), "a variant against itself");
    let value = answer(&mut app, "compare_variants", &["from=Lite", "to=Base"]);
    assert_eq!(designators(&value), ["R2"]);
    assert_eq!(value["parts"][0]["from"]["fitted"], false);
}

/// A value of its own in Lite is a parameter difference, as the tab shows
/// it after the override is stored.
#[test]
fn compare_variants_lists_a_value_of_its_own() {
    let (mut app, id, _dir) = app();
    open_manager(&mut app, id);
    let r1 = super::manager_tests::cell(Some("Lite"), "R1");
    let _ = manage(&mut app, id, VariantManagerMsg::CellSelected(r1.clone()));
    let _ = manage(
        &mut app,
        id,
        VariantManagerMsg::OverrideEdited {
            parameter: signex_types::project::VALUE_PARAMETER.into(),
            text: "4k7".into(),
        },
    );
    let _ = manage(
        &mut app,
        id,
        VariantManagerMsg::OverrideSubmitted { cell: r1 },
    );
    let value = answer(&mut app, "compare_variants", &[]);
    assert_eq!(designators(&value), ["R1", "R2"]);
    assert_eq!(value["parts"][0]["changes"][0]["kind"], "parameter");
    assert_eq!(value["parts"][0]["changes"][0]["parameter"], "Value");
    assert_eq!(value["parts"][0]["from"]["parameters"]["Value"], "1k");
    assert_eq!(value["parts"][0]["to"]["parameters"]["Value"], "4k7");
}

#[test]
fn compare_variants_refuses_what_it_cannot_compare() {
    let (mut app, _id, _dir) = app();
    let refused = refusal(&mut app, "compare_variants", &["to=Ghost"]);
    assert!(refused.contains("no variant named `Ghost`"), "{refused}");

    app.document_state.engines.clear();
    let refused = refusal(&mut app, "compare_variants", &[]);
    assert!(
        refused.contains("no sheet of the project is open"),
        "{refused}"
    );

    app.document_state.projects[0]
        .data
        .variant_definitions
        .clear();
    let refused = refusal(&mut app, "compare_variants", &[]);
    assert!(refused.contains("no variant besides Base"), "{refused}");
}

/// The BOM of a variant counts the parts it fits; none is the active
/// variant, `Base` is Base.
#[test]
fn variant_bom_answers_the_lines_of_the_variant() {
    let (mut app, _id, _dir) = app();
    let value = answer(&mut app, "variant_bom", &["name=lite"]);
    assert_eq!(value["variant"], "Lite");
    assert_eq!(value["parts"], 1);
    assert!(
        value["lines"]
            .as_array()
            .is_some_and(|lines| !lines.is_empty())
    );

    let value = answer(&mut app, "variant_bom", &[]);
    assert_eq!(value["variant"], "Base");
    assert_eq!(value["parts"], 2);

    app.document_state.projects[0].data.active_variant = Some("Lite".into());
    let value = answer(&mut app, "variant_bom", &[]);
    assert_eq!(value["variant"], "Lite");
    let value = answer(&mut app, "variant_bom", &["name=Base"]);
    assert_eq!(value["variant"], "Base");
    assert_eq!(value["parts"], 2);
}

#[test]
fn variant_bom_refuses_an_unknown_variant_and_no_open_sheet() {
    let (mut app, _id, _dir) = app();
    let refused = refusal(&mut app, "variant_bom", &["name=Ghost"]);
    assert!(refused.contains("no variant named `Ghost`"), "{refused}");
    app.document_state.engines.clear();
    let refused = refusal(&mut app, "variant_bom", &[]);
    assert!(
        refused.contains("no sheet of the project is open"),
        "{refused}"
    );
}

/// The queries change nothing: no undo step, the project as it was.
#[test]
fn the_queries_change_nothing() {
    let (mut app, id, _dir) = app();
    let before = serde_json::to_value(data(&app)).expect("the project serialises");
    let _ = answer(&mut app, "compare_variants", &[]);
    let _ = answer(&mut app, "variant_bom", &["name=Lite"]);
    assert_eq!(
        serde_json::to_value(data(&app)).expect("the project serialises"),
        before
    );
    assert!(!app.project_tab_history_available(id).0);
    assert!(app.document_state.bom_preview.is_none(), "no dialog opens");
}

/// The command and Set as default make the same change, one General step;
/// `project_settings` answers the default.
#[test]
fn project_set_default_variant_matches_set_as_default() {
    let (mut by_command, id, _dir) = app();
    let app = &mut by_command;
    let value = answer(app, "project_set_default_variant", &["name=lite"]);
    assert_eq!(value["changed"], true);
    assert_eq!(value["variants"]["default"], "Lite");
    assert_eq!(data(app).default_variant(), Some("Lite"));
    assert_eq!(data(app).active_variant, None, "the active variant stays");
    let settings = answer(app, "project_settings", &[]);
    assert_eq!(settings["variants"]["default"], "Lite");

    let (mut by_click, click_id, _click_dir) = super::manager_tests::app();
    open_manager(&mut by_click, click_id);
    let _ = manage(
        &mut by_click,
        click_id,
        VariantManagerMsg::DefaultPicked {
            name: Some("Lite".into()),
        },
    );
    assert_eq!(
        data(&by_click).default_variant(),
        data(app).default_variant()
    );

    assert!(app.step_project_tab_history(id, true));
    assert_eq!(data(app).default_variant(), None);
    let value = answer(app, "project_set_default_variant", &["name=Lite"]);
    assert_eq!(value["changed"], true);
    let value = answer(app, "project_set_default_variant", &[]);
    assert_eq!(value["variants"]["default"], "Base");
    assert_eq!(data(app).default_variant(), None);
}

#[test]
fn project_set_default_variant_refuses_an_unknown_variant() {
    let (mut app, _id, _dir) = app();
    let outcome = run(&mut app, "project_set_default_variant", &["name=Ghost"]);
    assert!(
        matches!(outcome, CommandOutcome::Disabled { .. }),
        "{outcome:?}"
    );
    assert_eq!(data(&app).default_variant(), None);
}
