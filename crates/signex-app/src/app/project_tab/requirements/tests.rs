//! #2339: the Requirements section, driven through `Signex::update` and the
//! catalog. Each edit dirties the `.snxprj`, writes nothing until Save, and
//! is one undo step; a refused one changes nothing and says why; Run checks
//! lands its report in the tab; a save writes the list and reads back the
//! same. The card look (Graphite mockup m38): outcome counts, which cards
//! open, the value summaries and where a refusal shows.

use std::sync::Arc;

use serde_json::{Value, json};
use signex_types::check::{CheckReport, Finding, Severity};
use signex_types::project::{
    RequirementEntry, RequirementField, RequirementProblem, parse_project, write_project,
};

use super::view::{OutcomeCounts, Verdict, is_open, outcome_counts, summary_words};
use super::{EntryKey, RequirementsMsg};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, ProjectTabMsg, Signex};
use crate::keymap::AppCommandId;

/// A project tab over a project file written to a folder of its own; the
/// folder lives as long as the returned guard.
fn app() -> (Signex, ProjectId, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("a folder");
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["a.snxsch"]);
    app.ui_state.erc_severity_override.clear();
    app.ui_state.pin_matrix_overrides.clear();
    let project = &app.document_state.projects[0];
    write_project(&project.path, &project.data).expect("the project file");
    app.open_project_tab(0);
    let id = app.document_state.projects[0].id;
    (app, id, dir)
}

fn send(app: &mut Signex, id: ProjectId, msg: RequirementsMsg) -> iced::Task<Message> {
    app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::Requirements {
            project_id: id,
            msg,
        },
    )))
}

fn stored(app: &Signex) -> Vec<Value> {
    app.document_state.projects[0]
        .data
        .requirements
        .iter()
        .map(|entry| entry.raw().clone())
        .collect()
}

fn on_disk(app: &Signex) -> Vec<u8> {
    std::fs::read(&app.document_state.projects[0].path).expect("the project file")
}

fn dirty(app: &Signex) -> bool {
    let path = &app.document_state.projects[0].path;
    app.document_state.is_document_dirty(path)
}

fn undo(app: &mut Signex, id: ProjectId, back: bool) -> bool {
    app.step_project_tab_history(id, back)
}

fn tab(app: &Signex, id: ProjectId) -> &super::RequirementsTabState {
    &app.document_state.project_tabs[&id].requirements
}

fn add(app: &mut Signex, id: ProjectId, kind: &'static str) {
    let _ = send(app, id, RequirementsMsg::AddPicked { kind });
}

fn type_and_submit(app: &mut Signex, id: ProjectId, field: RequirementField, text: &str) {
    let edited = RequirementsMsg::FieldEdited {
        index: 0,
        field,
        text: text.into(),
    };
    let _ = send(app, id, edited);
    let _ = send(app, id, RequirementsMsg::FieldSubmitted { index: 0, field });
}

fn run(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let document = format!("document={}", app.document_state.projects[0].path.display());
    let mut all = vec![document.as_str()];
    all.extend_from_slice(operands);
    let command = AppCommandId::new(id).expect("a catalog id");
    let args = CommandArgs::parse_operands(&command, &all).expect("operands parse");
    let (outcome, task) = match app.run_query(&command, &args) {
        Some(ran) => ran,
        None => app.run_operands(&command, &args),
    };
    crate::task_driver::TaskDriver::new()
        .settle(app, task)
        .expect("the task settles");
    outcome
}

fn answer(app: &mut Signex, id: &str, operands: &[&str]) -> Value {
    match run(app, id, operands) {
        CommandOutcome::Answered(value) => value,
        other => panic!("{id} did not answer: {other:?}"),
    }
}

fn refusal(app: &mut Signex, id: &str, operands: &[&str]) -> String {
    match run(app, id, operands) {
        CommandOutcome::Disabled { reason } => reason,
        other => panic!("{id} was not refused: {other:?}"),
    }
}

#[test]
fn an_add_is_one_undo_step_that_dirties_and_writes_nothing() {
    let (mut app, id, _dir) = app();
    let before = on_disk(&app);
    add(&mut app, id, "board_size");
    assert_eq!(
        stored(&app),
        [json!({"id": "board_size", "kind": "board_size",
                "max_width_mm": 100, "max_height_mm": 100})]
    );
    assert!(dirty(&app));
    assert_eq!(on_disk(&app), before, "nothing is written until Save");
    assert!(undo(&mut app, id, true));
    assert!(stored(&app).is_empty());
    assert!(!dirty(&app), "undo returned to the file's content");
    assert!(undo(&mut app, id, false));
    assert_eq!(stored(&app).len(), 1);
}

#[test]
fn a_field_commits_on_enter_and_a_refused_value_stays_with_its_reason() {
    let (mut app, id, _dir) = app();
    add(&mut app, id, "board_size");
    type_and_submit(&mut app, id, RequirementField::MaxWidthMm, "80.5");
    assert_eq!(stored(&app)[0]["max_width_mm"], json!(80.5));
    assert!(tab(&app, id).drafts.is_empty());

    type_and_submit(&mut app, id, RequirementField::MaxWidthMm, "-2");
    assert_eq!(stored(&app)[0]["max_width_mm"], json!(80.5), "refused");
    let state = tab(&app, id);
    assert_eq!(
        state
            .drafts
            .get(&(EntryKey::first("board_size"), RequirementField::MaxWidthMm))
            .map(String::as_str),
        Some("-2"),
        "the typed text stays in its box"
    );
    assert_eq!(
        state.problem,
        Some((
            Some(EntryKey::first("board_size")),
            RequirementProblem::NotPositive("-2".into())
        ))
    );

    // Undo steps the committed value back and shows the stored values again.
    assert!(undo(&mut app, id, true));
    assert_eq!(stored(&app)[0]["max_width_mm"], json!(100));
    assert!(tab(&app, id).drafts.is_empty());
    assert!(tab(&app, id).problem.is_none());
}

#[test]
fn a_pick_commits_at_once_and_toggle_and_remove_are_steps() {
    let (mut app, id, _dir) = app();
    add(&mut app, id, "erc_clean");
    let picked = RequirementsMsg::FieldPicked {
        index: 0,
        field: RequirementField::Severity,
        value: "warning".into(),
    };
    let _ = send(&mut app, id, picked);
    let allow = RequirementsMsg::FieldPicked {
        index: 0,
        field: RequirementField::AllowWarnings,
        value: "true".into(),
    };
    let _ = send(&mut app, id, allow);
    let _ = send(
        &mut app,
        id,
        RequirementsMsg::EnabledToggled {
            index: 0,
            on: false,
        },
    );
    assert_eq!(
        stored(&app),
        [
            json!({"id": "erc_clean", "kind": "erc_clean", "severity": "warning",
                "allow_warnings": true, "enabled": false})
        ]
    );
    let _ = send(&mut app, id, RequirementsMsg::Removed { index: 0 });
    assert!(stored(&app).is_empty());
    for _ in 0..4 {
        assert!(undo(&mut app, id, true));
    }
    assert_eq!(
        stored(&app),
        [json!({"id": "erc_clean", "kind": "erc_clean"})]
    );
}

#[test]
fn run_checks_lands_its_report_in_the_tab_off_the_ui_thread() {
    let (mut app, id, _dir) = app();
    add(&mut app, id, "board_size");
    let task = send(&mut app, id, RequirementsMsg::RunRequested);
    assert!(tab(&app, id).running, "the run is under way");
    crate::task_driver::TaskDriver::new()
        .settle(&mut app, task)
        .expect("the run settles");
    let state = tab(&app, id);
    assert!(!state.running);
    let report = state
        .last_run
        .as_ref()
        .expect("a run")
        .report
        .as_ref()
        .expect("a report");
    // The project lists no board, so the size cannot be met.
    assert_eq!(report.findings[0].rule, "req::board_size");
    assert!(report.passed.is_empty());
}

#[test]
fn the_commands_edit_through_the_same_path_and_refuse_with_the_reason() {
    let (mut app, id, _dir) = app();
    let value = answer(
        &mut app,
        "project_add_requirement",
        &["kind=bom_cost", "id=cost"],
    );
    assert_eq!(value["changed"], true);
    assert_eq!(value["requirements"][0]["id"], "cost");
    let value = answer(
        &mut app,
        "project_set_requirement",
        &["requirement=cost", "field=currency", "value=EUR"],
    );
    assert_eq!(value["requirements"][0]["currency"], "EUR");
    let value = answer(
        &mut app,
        "project_enable_requirement",
        &["requirement=cost", "on=false"],
    );
    assert_eq!(value["requirements"][0]["enabled"], false);

    let reason = refusal(
        &mut app,
        "project_set_requirement",
        &["requirement=cost", "field=quantity", "value=0"],
    );
    assert!(reason.contains("whole number"), "{reason}");
    let reason = refusal(
        &mut app,
        "project_remove_requirement",
        &["requirement=size"],
    );
    assert!(reason.contains("no requirement `size`"), "{reason}");
    let reason = refusal(&mut app, "project_add_requirement", &["kind=rail_budget"]);
    assert!(reason.contains("rail_budget"), "{reason}");

    let settings = answer(&mut app, "project_settings", &[]);
    assert_eq!(settings["requirements"][0]["id"], "cost");

    assert!(matches!(
        run(&mut app, "project_run_requirements", &[]),
        CommandOutcome::Ran
    ));
    assert!(
        tab(&app, id).last_run.is_some(),
        "the run landed in the tab"
    );

    let value = answer(
        &mut app,
        "project_remove_requirement",
        &["requirement=cost"],
    );
    assert_eq!(value["requirements"], json!([]));
    // Each edit was one step: four undos reach the empty list again.
    for _ in 0..4 {
        assert!(undo(&mut app, id, true));
    }
    assert!(stored(&app).is_empty());
    assert!(!undo(&mut app, id, true));
}

#[test]
fn a_save_writes_the_edited_list_and_reads_back_the_same() {
    let (mut app, id, _dir) = app();
    add(&mut app, id, "parts_state");
    type_and_submit(
        &mut app,
        id,
        RequirementField::States,
        "released, in_review",
    );
    add(&mut app, id, "net_class");
    let path = app.document_state.projects[0].path.clone();
    let task = app.save_project_at_path(&path).expect("the save starts");
    crate::task_driver::TaskDriver::new()
        .settle(&mut app, task)
        .expect("the save settles");
    assert!(!dirty(&app));
    let again = parse_project(&path).expect("the saved project");
    assert_eq!(
        again.requirements,
        app.document_state.projects[0].data.requirements
    );
    assert_eq!(
        again.requirements[0].raw()["states"],
        json!(["released", "in_review"])
    );
}

#[test]
fn the_section_renders_every_kind_an_unreadable_entry_drafts_and_a_run() {
    let (mut app, id, _dir) = app();
    for kind in signex_types::project::REQUIREMENT_KINDS {
        add(&mut app, id, kind);
    }
    app.document_state.projects[0]
        .data
        .requirements
        .push(serde_json::from_value(json!({"id": "rails", "kind": "rail_budget"})).unwrap());
    // A turned-off entry: its card is dimmed and inert and reads Off.
    let off = RequirementsMsg::EnabledToggled {
        index: 2,
        on: false,
    };
    let _ = send(&mut app, id, off);
    let _ = app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::SectionSelected {
            project_id: id,
            section: crate::app::ProjectSection::Requirements,
        },
    )));
    let task = send(&mut app, id, RequirementsMsg::RunRequested);
    crate::task_driver::TaskDriver::new()
        .settle(&mut app, task)
        .expect("the run settles");
    type_and_submit(&mut app, id, RequirementField::Id, "");
    assert!(tab(&app, id).problem.is_some(), "an empty id is refused");
    let _ = app.view_project_tab(id);

    // A passed entry with nothing typed renders closed to its header.
    let state = &mut app
        .document_state
        .project_tabs
        .get_mut(&id)
        .expect("the tab")
        .requirements;
    let mut run = state.last_run.as_deref().expect("a run").clone();
    let report = run.report.as_mut().expect("a report");
    report.findings.retain(|f| f.rule != "req::drc_clean");
    report.passed.push("req::drc_clean".into());
    state.last_run = Some(Arc::new(run));
    let report = state.last_run.as_ref().unwrap().report.as_ref().unwrap();
    let entries = &app.document_state.projects[0].data.requirements;
    let verdict = super::view::verdict(Some(report), &entries[1], 1);
    assert_eq!(verdict, Some(Verdict::Passed));
    assert!(!is_open(verdict, false, false), "a passed card is closed");
    let counts = outcome_counts(report, entries);
    assert_eq!(counts.off, 1, "the turned-off entry counts as Off");
    assert!(counts.passed >= 1);
    let total = counts.passed + counts.failed + counts.not_checked + counts.off;
    assert_eq!(total, entries.len(), "every entry is counted once");
    let _ = app.view_project_tab(id);
}

fn finding(rule: &str) -> Finding {
    Finding {
        rule: rule.into(),
        severity: Severity::Error,
        message: format!("{rule} is not met"),
        evidence: json!({}),
        items: Vec::new(),
        sheet: String::new(),
        key: rule.into(),
    }
}

fn report(failed: &[&str], passed: &[&str]) -> CheckReport {
    let findings = failed.iter().map(|rule| finding(rule)).collect();
    let mut report = CheckReport::new("requirements", findings, Vec::new(), Vec::new(), Vec::new());
    report.passed = passed.iter().map(|rule| rule.to_string()).collect();
    report
}

#[test]
fn the_counts_put_a_turned_off_entry_under_off_not_under_not_checked() {
    let entries: Vec<RequirementEntry> = [
        json!({"id": "a", "kind": "erc_clean"}),
        json!({"id": "b", "kind": "erc_clean"}),
        json!({"id": "c", "kind": "erc_clean", "enabled": false}),
        json!({"id": "d", "kind": "erc_clean"}),
        json!({"id": "e", "kind": "erc_clean", "enabled": false}),
    ]
    .into_iter()
    .map(RequirementEntry::read)
    .collect();
    // `e` is off even though the report names it: the switch wins.
    let report = report(&["req::a", "req::e"], &["req::b", "req::c"]);
    assert_eq!(
        outcome_counts(&report, &entries),
        OutcomeCounts {
            passed: 1,
            failed: 1,
            not_checked: 1,
            off: 2,
        }
    );
    assert_eq!(
        super::view::verdict(None, &entries[2], 2),
        Some(Verdict::Off),
        "a turned-off entry reads Off before any run"
    );
    assert_eq!(super::view::verdict(None, &entries[0], 0), None);
}

/// An entry with no id is filed under its place in the list, `req::#1`
/// for the first, as the check engine names it; a blank id counts as none.
#[test]
fn an_entry_with_no_id_reads_its_verdict_by_its_place() {
    let entries: Vec<RequirementEntry> = [
        json!({"kind": "erc_clean"}),
        json!({"id": "  ", "kind": "erc_clean"}),
        json!({"id": "c", "kind": "erc_clean"}),
    ]
    .into_iter()
    .map(RequirementEntry::read)
    .collect();
    let report = report(&["req::#1"], &["req::#2", "req::c"]);
    let verdict = |index: usize| super::view::verdict(Some(&report), &entries[index], index);
    assert_eq!(verdict(0), Some(Verdict::Failed));
    assert_eq!(verdict(1), Some(Verdict::Passed));
    assert_eq!(verdict(2), Some(Verdict::Passed));
    assert_eq!(
        outcome_counts(&report, &entries),
        OutcomeCounts {
            passed: 2,
            failed: 1,
            not_checked: 0,
            off: 0,
        }
    );
}

#[test]
fn a_passed_card_is_closed_every_other_open_and_a_flip_inverts_it() {
    assert!(!is_open(Some(Verdict::Passed), false, false));
    for open in [
        Some(Verdict::Failed),
        Some(Verdict::NotChecked),
        Some(Verdict::Off),
        None,
    ] {
        assert!(is_open(open, false, false), "{open:?} opens");
        assert!(!is_open(open, false, true), "{open:?} closes when flipped");
    }
    assert!(is_open(Some(Verdict::Passed), false, true), "flipped open");
    assert!(
        is_open(Some(Verdict::Passed), true, false),
        "a draft or a refusal opens a passed card"
    );
    assert!(
        is_open(Some(Verdict::Failed), true, true),
        "a draft or a refusal keeps a flipped card open"
    );
}

/// #2547: a flipped card follows its entry like the drafts; a removed
/// entry's flip goes with it and never lands on another card.
#[test]
fn a_flipped_card_follows_its_entry_and_goes_with_it() {
    let (mut app, id, _dir) = app();
    add(&mut app, id, "board_size");
    add(&mut app, id, "erc_clean");
    let flip = |index| RequirementsMsg::ExpandToggled { index };
    let flipped = |app: &Signex, name: &str| tab(app, id).toggled.contains(&EntryKey::first(name));
    let _ = send(&mut app, id, flip(1));
    assert!(flipped(&app, "erc_clean"));
    let _ = send(&mut app, id, flip(1));
    assert!(tab(&app, id).toggled.is_empty(), "a second flip undoes it");

    let _ = send(&mut app, id, flip(1));
    add(&mut app, id, "drc_clean");
    assert!(flipped(&app, "erc_clean"), "an add keeps it on its entry");
    assert_eq!(tab(&app, id).toggled.len(), 1, "and flips no other card");

    let _ = send(&mut app, id, flip(0));
    let _ = send(&mut app, id, RequirementsMsg::Removed { index: 1 });
    assert!(
        !flipped(&app, "erc_clean"),
        "the remove took the removed entry's flip"
    );
    assert!(flipped(&app, "board_size"), "and kept the other entry's");
    assert_eq!(tab(&app, id).toggled.len(), 1, "drc_clean was not flipped");

    answer(
        &mut app,
        "project_remove_requirement",
        &["requirement=board_size"],
    );
    assert!(
        tab(&app, id).toggled.is_empty(),
        "a catalog remove took its entry's flip"
    );
}

#[test]
fn each_kind_summarises_its_values_on_one_line() {
    let i18n = signex_i18n::I18n::en_us();
    let words = |value: Value| {
        let entry = RequirementEntry::read(value);
        let RequirementEntry::Known { requirement, raw } = &entry else {
            panic!("{entry:?} reads");
        };
        summary_words(requirement, raw, &i18n)
            .map(|words| words.to_string().replace(['\u{2068}', '\u{2069}'], ""))
    };
    assert_eq!(
        words(json!({"id": "erc", "kind": "erc_clean"})).as_deref(),
        Some("0 errors or warnings")
    );
    assert_eq!(
        words(json!({"id": "drc", "kind": "drc_clean", "allow_warnings": true})).as_deref(),
        Some("0 errors · warnings allowed")
    );
    assert_eq!(
        words(json!({"id": "size", "kind": "board_size",
                     "max_width_mm": 50, "max_height_mm": 40.5}))
        .as_deref(),
        Some("≤ 50 × 40.5 mm")
    );
    assert_eq!(
        words(json!({"id": "cost", "kind": "bom_cost",
                     "quantity": 1000, "currency": "EUR", "max_unit_cost": 5.5}))
        .as_deref(),
        Some("≤ 5.5 EUR per board at 1000 boards")
    );
    assert_eq!(
        words(json!({"id": "parts", "kind": "parts_state",
                     "states": ["released", "in_review"]}))
        .as_deref(),
        Some("Only released or in_review parts")
    );
    assert_eq!(
        words(json!({"id": "power", "kind": "net_class", "nets": "VIN_*",
                     "class": "Power", "min_trace_width_mm": 0.8}))
        .as_deref(),
        Some("VIN_*: class Power and track ≥ 0.8 mm")
    );
    assert_eq!(
        words(json!({"id": "gap", "kind": "net_class", "nets": "*",
                     "min_clearance_mm": 0.2}))
        .as_deref(),
        Some("*: clearance ≥ 0.2 mm")
    );
    assert_eq!(
        words(json!({"id": "none", "kind": "net_class", "nets": "*"})),
        None,
        "a net_class that asks nothing has no summary"
    );
}

/// A refused field commit names its field, so the card shows the reason
/// under that box; the next edit clears both, and a refusal that is not a
/// field commit names none.
#[test]
fn a_refused_field_names_its_field_until_the_next_edit() {
    let (mut app, id, _dir) = app();
    add(&mut app, id, "board_size");
    type_and_submit(&mut app, id, RequirementField::MaxWidthMm, "-2");
    let state = tab(&app, id);
    assert!(state.problem.is_some());
    assert_eq!(state.problem_field, Some(RequirementField::MaxWidthMm));

    let edited = RequirementsMsg::FieldEdited {
        index: 0,
        field: RequirementField::MaxHeightMm,
        text: "50".into(),
    };
    let _ = send(&mut app, id, edited);
    let state = tab(&app, id);
    assert!(state.problem.is_none());
    assert_eq!(state.problem_field, None);

    type_and_submit(&mut app, id, RequirementField::MaxHeightMm, "0");
    assert_eq!(
        tab(&app, id).problem_field,
        Some(RequirementField::MaxHeightMm)
    );
    let _ = send(&mut app, id, RequirementsMsg::Removed { index: 5 });
    let state = tab(&app, id);
    assert_eq!(
        state.problem,
        Some((None, RequirementProblem::NoEntry(5))),
        "a remove of no entry is refused, at no entry"
    );
    assert_eq!(state.problem_field, None, "and names no field");
}

/// The texts typed and not committed, in no order.
fn drafted(app: &Signex, id: ProjectId) -> Vec<String> {
    let mut texts: Vec<String> = tab(app, id).drafts.values().cloned().collect();
    texts.sort();
    texts
}

fn add_by_catalog(app: &mut Signex, name: &str) {
    let id_operand = format!("id={name}");
    answer(
        app,
        "project_add_requirement",
        &["kind=board_size", &id_operand],
    );
}

fn index_of(app: &Signex, name: &str) -> usize {
    stored(app)
        .iter()
        .position(|entry| entry["id"] == name)
        .expect("the entry")
}

/// #2547: a draft on requirement b survives an add and a remove of other
/// entries, from the tab, the catalog, undo and redo; Enter then commits it
/// into b and nowhere else.
#[test]
fn a_draft_survives_an_add_or_remove_of_another_requirement_from_any_origin() {
    let (mut app, id, _dir) = app();
    for name in ["a", "b", "c"] {
        add_by_catalog(&mut app, name);
    }
    let field = RequirementField::MaxWidthMm;
    let edited = RequirementsMsg::FieldEdited {
        index: 1,
        field,
        text: "55".into(),
    };
    let _ = send(&mut app, id, edited);

    add_by_catalog(&mut app, "d");
    assert_eq!(drafted(&app, id), ["55"], "an agent's add");
    let _ = send(&mut app, id, RequirementsMsg::Removed { index: 0 });
    assert_eq!(drafted(&app, id), ["55"], "the person's remove of a");
    assert!(undo(&mut app, id, true));
    assert_eq!(drafted(&app, id), ["55"], "the undo that brings a back");
    assert!(undo(&mut app, id, false));
    assert_eq!(drafted(&app, id), ["55"], "the redo that removes a again");
    answer(&mut app, "project_remove_requirement", &["requirement=c"]);
    assert_eq!(drafted(&app, id), ["55"], "an agent's remove of c");

    let at = index_of(&app, "b");
    let _ = send(
        &mut app,
        id,
        RequirementsMsg::FieldSubmitted { index: at, field },
    );
    for entry in stored(&app) {
        let expected = if entry["id"] == "b" {
            json!(55)
        } else {
            json!(100)
        };
        assert_eq!(entry["max_width_mm"], expected, "{entry}");
    }
    assert!(drafted(&app, id).is_empty(), "committed");
}

/// #2547: the removed entry's drafts go with it; a later entry in its place,
/// even one given its id, starts from its stored values, and Enter there
/// commits nothing.
#[test]
fn the_removed_requirements_drafts_go_with_it_and_never_land_elsewhere() {
    let (mut app, id, _dir) = app();
    for name in ["a", "b"] {
        add_by_catalog(&mut app, name);
    }
    let field = RequirementField::MaxWidthMm;
    let edited = RequirementsMsg::FieldEdited {
        index: 0,
        field,
        text: "33".into(),
    };
    let _ = send(&mut app, id, edited);
    answer(&mut app, "project_remove_requirement", &["requirement=a"]);
    assert!(drafted(&app, id).is_empty(), "a's draft went with a");

    add_by_catalog(&mut app, "a");
    for index in 0..2 {
        let _ = send(
            &mut app,
            id,
            RequirementsMsg::FieldSubmitted { index, field },
        );
    }
    for entry in stored(&app) {
        assert_eq!(entry["max_width_mm"], json!(100), "{entry}");
    }
}

/// Drafts are keyed by requirement id (#2547): a remove through the catalog
/// keeps the other entries' drafts, and Enter afterwards cannot commit a
/// draft to the entry that moved into the removed one's place.
#[test]
fn a_catalog_remove_keeps_other_drafts_and_none_lands_on_the_wrong_entry() {
    let (mut app, id, _dir) = app();
    for name in ["a", "b", "c"] {
        let id_operand = format!("id={name}");
        answer(
            &mut app,
            "project_add_requirement",
            &["kind=board_size", &id_operand],
        );
    }
    let field = RequirementField::MaxWidthMm;
    let edited = RequirementsMsg::FieldEdited {
        index: 1,
        field,
        text: "55".into(),
    };
    let _ = send(&mut app, id, edited);
    assert!(!tab(&app, id).drafts.is_empty(), "a draft on entry b");

    answer(&mut app, "project_remove_requirement", &["requirement=a"]);
    assert_eq!(
        tab(&app, id).drafts.get(&(EntryKey::first("b"), field)),
        Some(&"55".to_string()),
        "b keeps its draft through the remove of a",
    );
    let _ = send(
        &mut app,
        id,
        RequirementsMsg::FieldSubmitted { index: 1, field },
    );

    let entries = stored(&app);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["id"], "b");
    assert_eq!(entries[1]["id"], "c");
    for entry in &entries {
        assert_eq!(entry["max_width_mm"], json!(100), "{entry}");
    }
}

fn edit_field(app: &mut Signex, id: ProjectId, index: usize, field: RequirementField, text: &str) {
    let edited = RequirementsMsg::FieldEdited {
        index,
        field,
        text: text.into(),
    };
    let _ = send(app, id, edited);
}

fn submit(app: &mut Signex, id: ProjectId, index: usize, field: RequirementField) {
    let _ = send(app, id, RequirementsMsg::FieldSubmitted { index, field });
}

/// #2547 review: the loader keeps two entries with one id (a hand edit, a
/// merge). A draft typed on the first is the first's alone: Enter on the
/// second commits nothing.
#[test]
fn entries_sharing_an_id_keep_a_draft_each() {
    let (mut app, id, _dir) = app();
    let twin = || {
        serde_json::from_value(json!({"id": "x", "kind": "board_size",
            "max_width_mm": 100, "max_height_mm": 100}))
        .expect("an entry")
    };
    app.document_state.projects[0].data.requirements = vec![twin(), twin()];
    let field = RequirementField::MaxWidthMm;
    edit_field(&mut app, id, 0, field, "55");
    submit(&mut app, id, 1, field);
    let widths: Vec<Value> = stored(&app)
        .iter()
        .map(|entry| entry["max_width_mm"].clone())
        .collect();
    assert_eq!(widths, [json!(100), json!(100)]);
    assert_eq!(drafted(&app, id), ["55"], "the first's draft stays");
}

/// #2547 review: a committed id moves the entry's other drafts to the new id.
#[test]
fn a_committed_id_takes_the_entrys_other_drafts_along() {
    let (mut app, id, _dir) = app();
    for name in ["a", "b"] {
        add_by_catalog(&mut app, name);
    }
    let width = RequirementField::MaxWidthMm;
    edit_field(&mut app, id, 1, width, "55");
    edit_field(&mut app, id, 1, RequirementField::Id, "b2");
    submit(&mut app, id, 1, RequirementField::Id);
    assert_eq!(stored(&app)[1]["id"], "b2");
    assert_eq!(drafted(&app, id), ["55"], "the width draft followed");
    submit(&mut app, id, 1, width);
    assert_eq!(stored(&app)[1]["max_width_mm"], json!(55));
    assert_eq!(stored(&app)[0]["max_width_mm"], json!(100));
}

/// #2547 review: a refusal stays with its entry when an agent removes an
/// earlier one, and goes when its own entry is removed.
#[test]
fn a_refusal_stays_with_its_entry_through_a_remove_of_another() {
    let (mut app, id, _dir) = app();
    for name in ["a", "b", "c"] {
        add_by_catalog(&mut app, name);
    }
    let width = RequirementField::MaxWidthMm;
    edit_field(&mut app, id, 1, width, "-2");
    submit(&mut app, id, 1, width);
    let shown_at = |app: &Signex| {
        tab(app, id).problem_shown_at(&app.document_state.projects[0].data.requirements)
    };
    assert_eq!(shown_at(&app), Some(1), "refused on b");

    answer(&mut app, "project_remove_requirement", &["requirement=a"]);
    assert_eq!(shown_at(&app), Some(index_of(&app, "b")), "still on b");

    answer(&mut app, "project_remove_requirement", &["requirement=b"]);
    assert_eq!(shown_at(&app), None, "gone with b");

    // A new b is a new entry: the old b's refusal must not come back on it.
    add_by_catalog(&mut app, "b");
    assert_eq!(shown_at(&app), None, "not on the re-added b");
}
