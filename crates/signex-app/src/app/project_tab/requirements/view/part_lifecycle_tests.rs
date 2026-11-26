//! #2686, mockup row 38: a Part lifecycle card has a Variant pick and a
//! Fitted parts only box, both committed through the tab's edit path; a
//! fitted-only shortfall counts fitted parts; a net class card changed since
//! the run that judged it says its edits are not checked yet.

use std::sync::Arc;

use serde_json::{Value, json};
use signex_types::check::CheckReport;
use signex_types::project::{RequirementField, RequirementProblem};

use super::super::{RequirementsMsg, RequirementsRun};
use super::card::variant_choices;
use super::tests::{app, count, line, sentence, texts};
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, ProjectTabMsg, Signex};

const EDITS_NOTE: &str = "last run · edits not checked yet";

fn send(app: &mut Signex, id: ProjectId, msg: RequirementsMsg) {
    let _ = app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::Requirements {
            project_id: id,
            msg,
        },
    )));
}

fn stored(app: &Signex, index: usize) -> Value {
    app.document_state.projects[0].data.requirements[index]
        .raw()
        .clone()
}

fn pick(app: &mut Signex, id: ProjectId, field: RequirementField, value: &str) {
    let msg = RequirementsMsg::FieldPicked {
        index: 0,
        field,
        value: value.into(),
    };
    send(app, id, msg);
}

/// A finished run that passed `passed`, over the list as it is now.
fn finish_run(app: &mut Signex, id: ProjectId, passed: &[&str]) {
    let report = CheckReport::new(
        "requirements",
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .with_passed(passed.iter().map(|rule| rule.to_string()).collect());
    let checked = app.document_state.projects[0]
        .data
        .requirements
        .iter()
        .map(|entry| entry.raw().clone())
        .collect();
    let tab = app
        .document_state
        .project_tabs
        .get_mut(&id)
        .expect("the tab");
    tab.requirements.last_run = Some(Arc::new(RequirementsRun {
        report: Ok(report),
        board_mm: None,
        finished_at: chrono::Utc::now(),
        checked,
    }));
}

#[test]
fn a_part_lifecycle_card_shows_variant_and_fitted_parts_only() {
    let (app, id, _dir) = app(json!([
        {"id": "parts", "kind": "parts_state"},
        {"id": "erc", "kind": "erc_clean"},
    ]));
    let texts = texts(&app, id);
    assert_eq!(count(&texts, "Variant"), 1, "{texts:#?}");
    assert_eq!(count(&texts, "Fitted parts only"), 1, "{texts:#?}");
    assert_eq!(count(&texts, "Lifecycle states"), 1, "{texts:#?}");
}

#[test]
fn the_variant_pick_lists_the_active_variant_base_and_each_variant() {
    let i18n = signex_i18n::I18n::en_us_ref();
    let variants = vec!["Lite".to_string(), "Pro".to_string()];
    let plain = |text: &str| text.replace(['\u{2068}', '\u{2069}'], "");
    let (choices, picked) = variant_choices(&variants, "Pro", &json!({}), i18n);
    let labels: Vec<String> = choices.iter().map(|c| plain(&c.label)).collect();
    assert_eq!(labels, ["Active variant (Pro)", "Base", "Lite", "Pro"]);
    assert_eq!(
        picked.value, "",
        "no stored value follows the active variant"
    );
    let (_, picked) = variant_choices(&variants, "Pro", &json!({"variant": "lite"}), i18n);
    assert_eq!(picked.value, "Lite");
    let (_, picked) = variant_choices(&variants, "Pro", &json!({"variant": "Gone"}), i18n);
    assert_eq!(
        (picked.value.as_str(), picked.label.as_str()),
        ("Gone", "Gone"),
        "a removed variant stays shown as written"
    );
}

#[test]
fn picking_a_variant_and_unticking_fitted_parts_only_are_undo_steps() {
    let (mut app, id, _dir) = app(json!([{"id": "parts", "kind": "parts_state"}]));
    app.document_state.projects[0].data.variant_definitions = vec!["Lite".into()];
    pick(&mut app, id, RequirementField::Variant, "Lite");
    assert_eq!(stored(&app, 0)["variant"], json!("Lite"));
    pick(&mut app, id, RequirementField::FittedOnly, "false");
    assert_eq!(stored(&app, 0)["fitted_only"], json!(false));
    assert!(app.step_project_tab_history(id, true));
    assert!(
        stored(&app, 0).get("fitted_only").is_none(),
        "one step back"
    );
    assert!(app.step_project_tab_history(id, true));
    assert_eq!(
        stored(&app, 0),
        json!({"id": "parts", "kind": "parts_state"})
    );
    pick(&mut app, id, RequirementField::Variant, "Gone");
    assert_eq!(
        stored(&app, 0),
        json!({"id": "parts", "kind": "parts_state"})
    );
    let problem = app.document_state.project_tabs[&id]
        .requirements
        .problem
        .as_ref()
        .map(|(_, problem)| problem.clone());
    assert_eq!(
        problem,
        Some(RequirementProblem::UnknownVariant("Gone".into()))
    );
    let texts = texts(&app, id);
    assert_eq!(
        count(
            &texts,
            "Gone is no variant of this project: Base or one of its variants."
        ),
        1,
        "{texts:#?}"
    );
}

#[test]
fn a_fitted_only_shortfall_counts_fitted_parts() {
    let mut fitted = line("R3", 1, None, None);
    fitted.evidence["fitted_only"] = json!(true);
    assert_eq!(
        sentence(&[fitted]).as_deref(),
        Some(
            "1 of 5 fitted parts is not released or deprecated: R3 was not placed from a \
             library row."
        )
    );
    let mut all = line("R3", 1, None, None);
    all.evidence["fitted_only"] = json!(false);
    assert_eq!(
        sentence(&[all]).as_deref(),
        Some("1 of 5 parts is not released or deprecated: R3 was not placed from a library row.")
    );
}

#[test]
fn a_net_class_card_changed_since_its_run_says_its_edits_are_not_checked() {
    let (mut app, id, _dir) = app(json!([
        {"id": "power", "kind": "net_class", "nets": "V*", "min_clearance_mm": 0.2},
        {"id": "parts", "kind": "parts_state"},
    ]));
    assert_eq!(count(&texts(&app, id), EDITS_NOTE), 0, "no run yet");
    finish_run(&mut app, id, &["req::power", "req::parts"]);
    assert_eq!(count(&texts(&app, id), EDITS_NOTE), 0, "unchanged since");
    let data = &mut app.document_state.projects[0].data;
    data.set_requirement_field(0, RequirementField::MinClearanceMm, "0.3")
        .unwrap();
    data.set_requirement_field(1, RequirementField::States, "draft")
        .unwrap();
    assert_eq!(
        count(&texts(&app, id), EDITS_NOTE),
        1,
        "the net class card only"
    );
    finish_run(&mut app, id, &["req::power", "req::parts"]);
    assert_eq!(count(&texts(&app, id), EDITS_NOTE), 0, "the next run");
}
