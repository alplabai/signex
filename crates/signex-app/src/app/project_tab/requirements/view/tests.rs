//! Mockup row 38, laid out: before the first run every card switched on
//! reads Not checked and the summary counts them; a failed card reads its
//! findings as one sentence, not as key: value evidence.

use std::sync::Arc;

use serde_json::json;
use signex_types::check::{CheckReport, Finding, Severity};
use signex_types::project::RequirementEntry;

use super::super::RequirementsRun;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, ProjectSection, ProjectTabMsg, Signex};

const WINDOW: iced::Size = iced::Size::new(1400.0, 1000.0);

/// A project tab on the Requirements section over `list`.
fn app(list: serde_json::Value) -> (Signex, ProjectId, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("a folder");
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["a.snxsch"]);
    let entries: Vec<RequirementEntry> = serde_json::from_value(list).expect("a list");
    app.document_state.projects[0].data.requirements = entries;
    app.open_project_tab(0);
    let id = app.document_state.projects[0].id;
    let _ = app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::SectionSelected {
            project_id: id,
            section: ProjectSection::Requirements,
        },
    )));
    (app, id, dir)
}

/// Every text the tab lays out, without Fluent's isolation marks.
fn texts(app: &Signex, id: ProjectId) -> Vec<String> {
    crate::layout_probe::lay_out_texts(app.view_project_tab(id), WINDOW)
        .into_iter()
        .map(|laid| laid.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

fn count(texts: &[String], label: &str) -> usize {
    texts.iter().filter(|text| text.as_str() == label).count()
}

#[test]
fn before_the_first_run_each_card_switched_on_reads_not_checked_and_is_counted() {
    let (app, id, _dir) = app(json!([
        {"id": "erc", "kind": "erc_clean"},
        {"id": "size", "kind": "board_size", "max_width_mm": 50, "max_height_mm": 40},
        {"id": "off", "kind": "drc_clean", "enabled": false},
    ]));
    let texts = texts(&app, id);
    assert_eq!(
        count(&texts, "Not checked"),
        2,
        "one pill per card switched on"
    );
    assert_eq!(count(&texts, "Off"), 1, "the card switched off reads Off");
    assert_eq!(
        count(&texts, "2 Not checked"),
        1,
        "the summary counts the cards not checked, not the one switched off"
    );
    assert!(
        !texts
            .iter()
            .any(|t| t.ends_with(" Passed") || t.ends_with(" Failed")),
        "no Passed or Failed counts before a run: {texts:#?}"
    );
    assert_eq!(count(&texts, "Not run yet."), 1);
}

fn part(designator: &str, state: &str) -> Finding {
    Finding {
        rule: "req::released".into(),
        severity: Severity::Error,
        message: format!("{designator} is {state}, not released"),
        evidence: json!({
            "kind": "parts_state",
            "designator": designator,
            "library_row": format!("lib/{designator}"),
            "state": state,
            "allowed": ["released"],
            "parts": 21,
        }),
        items: Vec::new(),
        sheet: String::new(),
        key: designator.into(),
    }
}

#[test]
fn a_failed_card_reads_its_findings_as_one_sentence() {
    let (mut app, id, _dir) = app(json!([
        {"id": "released", "kind": "parts_state", "states": ["released"]},
    ]));
    let report = CheckReport::new(
        "requirements",
        vec![part("L1", "draft"), part("C6", "obsolete")],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let tab = app
        .document_state
        .project_tabs
        .get_mut(&id)
        .expect("the tab");
    tab.requirements.last_run = Some(Arc::new(RequirementsRun {
        report: Ok(report),
        finished_at: chrono::Utc::now(),
        board_mm: None,
    }));
    let texts = texts(&app, id);
    // The report puts its findings in one order (`CheckReport::new`, by
    // message), whatever order the check filed them in: C6 reads first.
    assert_eq!(
        count(
            &texts,
            "2 of 21 parts are not released: C6 is obsolete and L1 is draft."
        ),
        1,
        "{texts:#?}"
    );
    assert_eq!(count(&texts, "Failed"), 1, "the card's pill");
    assert!(
        !texts
            .iter()
            .any(|t| t.contains("designator:") || t.contains("library_row:")),
        "no raw key: value evidence: {texts:#?}"
    );
}

/// `findings` read as the card's sentence, without Fluent's isolation marks.
fn sentence(findings: &[Finding]) -> Option<String> {
    let findings: Vec<&Finding> = findings.iter().collect();
    super::findings::findings_sentence(&findings, signex_i18n::I18n::en_us_ref())
        .map(|words| words.replace(['\u{2068}', '\u{2069}'], ""))
}

/// A finding of a `parts_state` run over 5 parts for one BOM line.
fn line(designator: &str, count: usize, library_row: Option<&str>, state: Option<&str>) -> Finding {
    let mut finding = part(designator, state.unwrap_or(""));
    finding.evidence = json!({
        "kind": "parts_state",
        "designator": designator,
        "count": count,
        "library_row": library_row,
        "state": state,
        "allowed": ["released", "deprecated"],
        "parts": 5,
    });
    finding
}

#[test]
fn a_parts_sentence_counts_the_parts_of_a_grouped_line() {
    let findings = [
        line("C1,C2", 2, Some("lib/r2"), Some("draft")),
        line("R3", 1, None, None),
        line("R4", 1, Some("other/r9"), None),
    ];
    assert_eq!(
        sentence(&findings).as_deref(),
        Some(
            "4 of 5 parts are not released or deprecated: C1,C2 are draft, R3 was not \
             placed from a library row, and R4's library row is in no open library."
        )
    );
    assert_eq!(
        sentence(&[line("R3", 1, None, None)]).as_deref(),
        Some("1 of 5 parts is not released or deprecated: R3 was not placed from a library row.")
    );
    assert_eq!(sentence(&[]), None);
}

#[test]
fn other_findings_read_as_their_messages_joined() {
    let mut track = part("x", "");
    track.message = "Class USB sets a track width of 0.15 mm, under 0.2 mm".into();
    track.evidence = json!({"kind": "net_class", "class": "USB"});
    let mut net = track.clone();
    net.message = "Net USB_DM is in no class, not USB".into();
    assert_eq!(
        sentence(&[track, net]).as_deref(),
        Some(
            "Class USB sets a track width of 0.15 mm, under 0.2 mm; net USB_DM is in no class, not USB."
        )
    );
}
