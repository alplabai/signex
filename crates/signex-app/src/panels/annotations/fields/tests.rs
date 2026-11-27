//! A row's review record (mockup row 198): which fields each kind lists,
//! the typed id before the title, the verdict pill and the field grid of an
//! expanded row.

use std::path::PathBuf;
use std::sync::Arc;

use iced::Size;
use signex_notebook::{BlockId, Citation, Severity, Verdict};

use super::{FieldLabel, RowField, RowValue, record_fields};
use crate::app::build_annotations_view as build_view;
use crate::layout_probe::{find, lay_out_texts};
use crate::panels::PanelContext;
use crate::panels::annotations::{
    AnnotationRow, AnnotationsState, IsoDate, ListStatus, NotebookFile, NotebookScan, RowKey, rows,
};

const WINDOW: Size = Size::new(360.0, 600.0);

const NOTEBOOK: &str = "#decision(id: \"DEC-002\", title: [Inductor], status: \"accepted\", owner: \"j.berg\", review: \"R-03\", chosen: [Candidate A], requirement: \"REQ-002\", supersedes: \"DEC-001\")\n\
#issue(id: \"ISS-005\", title: [Input ripple], status: \"open\", owner: \"a.lind\", severity: \"major\", due: \"rev C\", from: \"MEAS-010\")[Add a second capacitor.]\n\
#measurement(id: \"MEAS-010\", title: [Input rail], target: net(\"VIN\"), value: 310, unit: \"mV\", limit: (max: 250), instrument: \"Scope\", operator: \"j.berg\")\n\
#measurement(id: \"MEAS-3.2\", title: [Rails], rows: ((net(\"VIN_12V\"), 12.0, 5%, 11.96), (net(\"VBUS\"), 5.0, 5%, 4.78)))\n\
#note(id: \"NOTE-012\", author: \"j.berg\")[Keep the copper small.]\n";

fn blocks() -> Vec<signex_notebook::Annotation> {
    let (found, problems) = signex_notebook::scan(NOTEBOOK);
    assert!(problems.is_empty(), "{problems:?}");
    found
}

fn text(label: FieldLabel, value: &str) -> RowField {
    RowField {
        label,
        value: RowValue::Text(value.into()),
    }
}

fn ids(label: FieldLabel, list: &[&str]) -> RowField {
    RowField {
        label,
        value: RowValue::Ids(list.iter().map(|s| s.to_string()).collect()),
    }
}

#[test]
fn a_decision_lists_owner_review_choice_requirement_and_what_it_supersedes() {
    let found = blocks();
    assert_eq!(
        record_fields(&found[0], &[]),
        [
            text(FieldLabel::Owner, "j.berg"),
            text(FieldLabel::Review, "R-03"),
            text(FieldLabel::Chosen, "Candidate A"),
            text(FieldLabel::Requirement, "REQ-002"),
            ids(FieldLabel::Supersedes, &["DEC-001"]),
        ]
    );
}

#[test]
fn an_issue_lists_owner_due_severity_and_where_it_came_from() {
    let found = blocks();
    assert_eq!(
        record_fields(&found[1], &[]),
        [
            text(FieldLabel::Owner, "a.lind"),
            text(FieldLabel::Due, "rev C"),
            RowField {
                label: FieldLabel::Severity,
                value: RowValue::Severity(Severity::Major),
            },
            ids(FieldLabel::From, &["MEAS-010"]),
        ]
    );
}

#[test]
fn a_measurement_lists_its_bench_and_the_issues_it_opens() {
    let found = blocks();
    let opened = [(Citation::From, BlockId::parse("ISS-005").unwrap())];
    assert_eq!(
        record_fields(&found[2], &opened),
        [
            text(FieldLabel::Instrument, "Scope"),
            text(FieldLabel::Operator, "j.berg"),
            ids(FieldLabel::Opens, &["ISS-005"]),
        ]
    );
    assert_eq!(
        record_fields(&found[3], &[]),
        [RowField {
            label: FieldLabel::Result,
            value: RowValue::Verdicts([(Verdict::Pass, 1), (Verdict::Warn, 1), (Verdict::Fail, 0)]),
        }],
        "several rows: how many pass, warn and fail"
    );
    assert_eq!(
        record_fields(&found[4], &[]),
        [text(FieldLabel::Author, "j.berg")]
    );
}

fn context() -> PanelContext {
    let (app, _boot) = crate::app::Signex::new();
    app.document_view.panel_ctx.clone()
}

/// The rows the panel builds from [`NOTEBOOK`], the row on `line` expanded.
fn rows_of(expanded: Option<u32>) -> Vec<AnnotationRow> {
    let path = PathBuf::from("/p/notebook/design.snxnbk");
    let (annotations, problems) = signex_notebook::scan(NOTEBOOK);
    let state = AnnotationsState {
        scan: Some(Arc::new(NotebookScan {
            folder_found: true,
            files: vec![NotebookFile {
                path: path.clone(),
                label: "notebook/design.snxnbk".into(),
                annotations,
                problems,
                read_error: None,
            }],
            unlisted: Vec::new(),
            parts: None,
            design: None,
            references: Vec::new(),
        })),
        status: ListStatus::Ready,
        expanded: expanded.map(|line| RowKey { file: path, line }),
        ..AnnotationsState::default()
    };
    let view = build_view(&state, IsoDate::EPOCH);
    view.groups.into_iter().flat_map(|g| g.rows).collect()
}

fn row(rows: &[AnnotationRow], id: &str) -> AnnotationRow {
    rows.iter()
        .find(|r| r.id.as_deref() == Some(id))
        .cloned()
        .unwrap_or_else(|| panic!("no row {id}"))
}

#[test]
fn a_row_carries_its_id_verdict_and_the_blocks_citing_it() {
    let rows = rows_of(None);
    let measurement = row(&rows, "MEAS-010");
    assert_eq!(measurement.verdict, Some(Verdict::Fail));
    assert!(
        measurement
            .fields
            .contains(&ids(FieldLabel::Opens, &["ISS-005"])),
        "{:?}",
        measurement.fields
    );
    assert_eq!(row(&rows, "MEAS-3.2").verdict, Some(Verdict::Warn));
    assert_eq!(row(&rows, "DEC-002").verdict, None);
}

#[test]
fn the_id_sits_before_the_title_and_the_verdict_beside_the_chips() {
    let ctx = context();
    let measurement = row(&rows_of(None), "MEAS-010");
    let texts = lay_out_texts(rows::annotation_row(&ctx, &measurement), WINDOW);
    let id = find(&texts, "MEAS-010").bounds;
    let title = find(&texts, "Input rail").bounds;
    assert!(id.x + id.width <= title.x, "{id:?} before {title:?}");
    assert!(
        (id.center_y() - title.center_y()).abs() < 2.0,
        "on one line"
    );
    let pill = find(&texts, "Fail").bounds;
    assert!(pill.y > title.y, "the verdict sits on the chip line");
}

#[test]
fn an_expanded_row_lists_its_record_above_its_source() {
    let ctx = context();
    let line = blocks()[1].line;
    let issue = row(&rows_of(Some(line)), "ISS-005");
    assert!(issue.expanded);
    let texts = lay_out_texts(rows::annotation_row(&ctx, &issue), WINDOW);
    let owner = find(&texts, "Owner").bounds;
    let value = find(&texts, "a.lind").bounds;
    assert!(
        owner.x + owner.width <= value.x,
        "the label, then its value"
    );
    assert!((owner.y - value.y).abs() < 2.0, "on one line");
    find(&texts, "Major");
    find(&texts, "MEAS-010");
    let source = find(&texts, &issue.source).bounds;
    assert!(owner.y < source.y, "the record above the source");
}

#[test]
fn a_collapsed_row_shows_no_record() {
    let ctx = context();
    let issue = row(&rows_of(None), "ISS-005");
    let texts = lay_out_texts(rows::annotation_row(&ctx, &issue), WINDOW);
    assert!(texts.iter().all(|t| t.text != "Owner"), "{texts:?}");
}
