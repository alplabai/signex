//! #2945 (mockup row 201): the Generate options dialog's layout and texts.

use std::collections::BTreeSet;
use std::path::PathBuf;

use signex_i18n::{I18n, ids};
use signex_notebook::generate::{GenerateOptions, Section};

use super::{SECTION_COLUMNS, run_label, section_hint, section_label};
use crate::app::Signex;
use crate::app::notebook_generate::GenerateDialog;

fn dialog() -> GenerateDialog {
    GenerateDialog {
        project: PathBuf::from("/work/buck-3v3/buck-3v3.snxprj"),
        project_file: "buck-3v3.snxprj".into(),
        revision: Some("B".into()),
        requirements: 6,
        variants: vec!["Base".into()],
        variant: "Base".into(),
        options: GenerateOptions::default(),
        document_number: "DN-0043".into(),
        document_revision: "A".into(),
        running: false,
        problem: None,
    }
}

#[test]
fn the_two_columns_hold_every_section_once() {
    let laid_out: Vec<Section> = SECTION_COLUMNS.iter().flatten().copied().collect();
    let unique: BTreeSet<Section> = laid_out.iter().copied().collect();
    assert_eq!(laid_out.len(), Section::ALL.len());
    assert_eq!(unique.len(), Section::ALL.len());
    // As drawn: Summary and Bill of materials summary head the columns.
    assert_eq!(SECTION_COLUMNS[0][0], Section::Summary);
    assert_eq!(SECTION_COLUMNS[1][0], Section::Bom);
}

#[test]
fn every_section_has_its_own_label() {
    let labels: BTreeSet<&str> = Section::ALL
        .iter()
        .map(|s| section_label(*s).as_str())
        .collect();
    assert_eq!(labels.len(), Section::ALL.len());
}

#[test]
fn summary_and_checks_have_no_hint_and_the_requirements_hint_counts() {
    let i18n = I18n::en_us();
    let d = dialog();
    assert!(section_hint(Section::Summary, &d, &i18n).is_none());
    assert!(section_hint(Section::Checks, &d, &i18n).is_none());
    let hint = section_hint(Section::Requirements, &d, &i18n)
        .expect("a hint")
        .into_string();
    assert!(hint.contains('6'), "{hint}");
    for section in Section::ALL {
        if !matches!(section, Section::Summary | Section::Checks) {
            assert!(section_hint(section, &d, &i18n).is_some(), "{section:?}");
        }
    }
}

#[test]
fn the_call_to_action_reads_generating_while_a_run_is_under_way() {
    let mut d = dialog();
    assert_eq!(run_label(&d), ids::DIALOG_NB_GENERATE_RUN);
    d.running = true;
    assert_eq!(run_label(&d), ids::DIALOG_NB_GENERATE_RUNNING);
}

#[test]
fn the_dialog_builds_open_running_and_with_a_problem() {
    let (app, _boot) = Signex::new();
    let mut d = dialog();
    let _ = app.view_notebook_generate_dialog(&d);
    d.running = true;
    d.problem = Some("the project has no root sheet".into());
    d.revision = None;
    let _ = app.view_notebook_generate_dialog(&d);
}
