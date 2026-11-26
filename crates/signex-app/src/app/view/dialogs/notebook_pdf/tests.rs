//! Mockup row 202 on screen: the form shows every setting with the page
//! count and the document's paper, the stale banner turns Export PDF into
//! Export anyway and Fail greys it, a click sends its message, and the done
//! card shows the file, its facts and the checksum.

use std::path::PathBuf;

use iced::widget::column;
use iced::{Size, Widget as _};
use signex_i18n::I18n;
use signex_notebook::export::{
    BookmarkCounts, ExportOptions, ExportSummary, NotebookInspection, PaperName, PaperSize,
    StaleCheck, StaleRule, TitleBlock,
};
use signex_notebook::{PageInfo, Reference};
use signex_types::theme::{ThemeId, theme_tokens};

use super::done::{fact_lines, short_checksum};
use super::form::{export_state, paper_labels, paper_words};
use super::{file_size, parts};
use crate::app::contracts::ExportMsg;
use crate::app::notebook_pdf::{ExportDone, NotebookPdfDialog, PageChoice, Stage};
use crate::app::{Message, NotebookPdfMsg, ProjectId};
use crate::layout_probe::{lay_out_inputs, lay_out_texts, messages_on_click};

const WINDOW: Size = Size::new(660.0, 640.0);

/// `text` without Fluent's bidi isolation marks around its arguments.
fn plain(text: &str) -> String {
    text.replace(['\u{2068}', '\u{2069}'], "")
}

fn a4() -> PaperSize {
    PaperSize::of(&PageInfo {
        number: 1,
        width_pt: 595.28,
        height_pt: 841.89,
    })
}

fn inspection(stale: &[&str]) -> NotebookInspection {
    NotebookInspection {
        page_count: 12,
        paper: Some(a4()),
        title: TitleBlock {
            doc: Some("DN-0042".into()),
            rev: Some("B".into()),
            ..TitleBlock::default()
        },
        stale: StaleCheck {
            references: 31,
            stale: stale
                .iter()
                .map(|name| Reference::Component((*name).into()))
                .collect(),
        },
        diagnostics: Vec::new(),
    }
}

fn dialog(stage: Stage) -> NotebookPdfDialog {
    NotebookPdfDialog {
        project: ProjectId::test_id(1),
        project_dir: PathBuf::from("/p"),
        project_name: "buck-3v3".into(),
        notebook: "design.snxnbk".into(),
        variants: vec!["Base".into(), "Lite".into()],
        current_page: 1,
        choice: PageChoice::All,
        range_text: String::new(),
        options: ExportOptions::default(),
        path_text: "Project Outputs/notebook/buck-3v3-DN-0042-revB.pdf".into(),
        path_edited: false,
        stage,
        generation: 1,
    }
}

fn texts(dialog: &NotebookPdfDialog) -> Vec<String> {
    let i18n = I18n::en_us();
    let tokens = theme_tokens(ThemeId::BUILTINS[0]);
    let (header, body, footer) = parts(dialog, &i18n, &tokens);
    lay_out_texts(column![header, body, footer].boxed(), WINDOW)
        .into_iter()
        .map(|t| plain(&t.text))
        .collect()
}

fn clicks(dialog: &NotebookPdfDialog, label: &str) -> Vec<Message> {
    let i18n = I18n::en_us();
    let tokens = theme_tokens(ThemeId::BUILTINS[0]);
    let (header, body, footer) = parts(dialog, &i18n, &tokens);
    messages_on_click(column![header, body, footer].boxed(), WINDOW, label)
}

fn sent(messages: &[Message], want: impl Fn(&NotebookPdfMsg) -> bool) -> bool {
    messages.iter().any(|m| match m {
        Message::Export(ExportMsg::NotebookPdf(msg)) => want(msg),
        _ => false,
    })
}

fn has(texts: &[String], words: &str) -> bool {
    texts.iter().any(|t| t.contains(words))
}

#[test]
fn the_form_shows_every_setting_as_drawn() {
    let shown = texts(&dialog(Stage::Ready(Box::new(inspection(&[])))));
    for words in [
        "Export Design Notebook PDF",
        "design.snxnbk · DN-0042 rev B",
        "All 12",
        "Current page",
        "Range",
        "Letter re-flows the pages",
        "values and designators resolve as in Base",
        "Title block and revision",
        "Contents",
        "PDF bookmarks",
        "sections, figures and tables",
        "Python cell source",
        "results and plots always print",
        "Note",
        "Decision",
        "Warn",
        "Fail the export",
        "stale = renamed or deleted after it was written",
        "Browse…",
        "Add as Output Job output…",
        "Cancel",
        "Export PDF",
    ] {
        assert!(has(&shown, words), "{words:?} in {shown:?}");
    }
    assert!(
        !has(&shown, "stale reference"),
        "no banner without stale references"
    );
}

#[test]
fn the_paper_list_names_the_documents_paper_first() {
    let i18n = I18n::en_us();
    let labels: Vec<String> = paper_labels(Some(&a4()), &i18n)
        .into_iter()
        .map(|(_, label)| label)
        .collect();
    assert_eq!(labels, ["From the document (A4 portrait)", "A4", "Letter"]);
    let unknown = paper_labels(None, &i18n);
    assert_eq!(unknown[0].1, "From the document (Custom)");
}

#[test]
fn the_save_to_field_shows_the_default_file() {
    let i18n = I18n::en_us();
    let tokens = theme_tokens(ThemeId::BUILTINS[0]);
    let dialog = dialog(Stage::Ready(Box::new(inspection(&[]))));
    let (header, body, footer) = parts(&dialog, &i18n, &tokens);
    let inputs = lay_out_inputs(column![header, body, footer].boxed(), WINDOW);
    assert!(
        inputs
            .iter()
            .any(|i| i.text == "Project Outputs/notebook/buck-3v3-DN-0042-revB.pdf"),
        "{inputs:?}"
    );
}

#[test]
fn a_stale_reference_shows_the_banner_and_export_anyway() {
    let dialog = dialog(Stage::Ready(Box::new(inspection(&["R7", "U7"]))));
    let shown = texts(&dialog);
    assert!(has(&shown, "2 stale references (R7, U7)"), "{shown:?}");
    assert!(has(&shown, "Review…"));
    assert!(has(&shown, "Export anyway"));
    assert_eq!(export_state(&dialog), (true, true));
    assert!(sent(&clicks(&dialog, "Export anyway"), |m| {
        matches!(m, NotebookPdfMsg::ExportRequested)
    }));
    assert!(sent(&clicks(&dialog, "Review…"), |m| {
        matches!(m, NotebookPdfMsg::ReviewRequested)
    }));
}

#[test]
fn fail_the_export_greys_export_while_a_reference_is_stale() {
    let mut dialog = dialog(Stage::Ready(Box::new(inspection(&["R7"]))));
    dialog.options.stale = StaleRule::Fail;
    assert_eq!(export_state(&dialog), (false, true));
    assert!(!sent(&clicks(&dialog, "Export anyway"), |m| {
        matches!(m, NotebookPdfMsg::ExportRequested)
    }));
}

#[test]
fn a_range_that_names_no_page_greys_export() {
    let mut dialog = dialog(Stage::Ready(Box::new(inspection(&[]))));
    dialog.choice = PageChoice::Range;
    dialog.range_text = "9-3".into();
    assert_eq!(export_state(&dialog), (false, false));
    dialog.range_text = "3-7".into();
    assert!(export_state(&dialog).0);
}

#[test]
fn a_click_sends_its_setting() {
    let dialog = dialog(Stage::Ready(Box::new(inspection(&[]))));
    assert!(sent(&clicks(&dialog, "Current page"), |m| {
        matches!(m, NotebookPdfMsg::PagesPicked(PageChoice::Current))
    }));
    assert!(sent(&clicks(&dialog, "Fail the export"), |m| {
        matches!(m, NotebookPdfMsg::StaleRulePicked(StaleRule::Fail))
    }));
    assert!(sent(&clicks(&dialog, "Python cell source"), |m| {
        matches!(
            m,
            NotebookPdfMsg::IncludeToggled(crate::app::notebook_pdf::IncludeFlag::PythonSource)
        )
    }));
    assert!(sent(&clicks(&dialog, "Browse…"), |m| {
        matches!(m, NotebookPdfMsg::BrowseRequested)
    }));
    assert!(sent(&clicks(&dialog, "Cancel"), |m| {
        matches!(m, NotebookPdfMsg::Closed)
    }));
    assert!(
        clicks(&dialog, "Add as Output Job output…").is_empty(),
        "no Export Job to add it to yet"
    );
}

#[test]
fn reading_and_unreadable_say_so() {
    assert!(has(&texts(&dialog(Stage::Reading)), "Reading the notebook"));
    let shown = texts(&dialog(Stage::Unreadable(
        "notebook/design.snxnbk: gone".into(),
    )));
    assert!(has(&shown, "notebook/design.snxnbk: gone"), "{shown:?}");
    assert_eq!(export_state(&dialog(Stage::Reading)), (false, false));
}

fn done(same_as_last: Option<bool>) -> ExportDone {
    ExportDone {
        path: PathBuf::from("/p/Project Outputs/notebook/buck-3v3-DN-0042-revB.pdf"),
        summary: ExportSummary {
            pages: 12,
            page_count: 12,
            first_page: 1,
            last_page: 12,
            paper: a4(),
            bytes: 1_468_006,
            references: 31,
            stale: Vec::new(),
            variant: "Base".into(),
            bookmarks: BookmarkCounts {
                sections: 5,
                subsections: 11,
                figures: 6,
                tables: 3,
            },
            sha256: format!("3f9c{}a41e", "0".repeat(56)),
            title: TitleBlock::default(),
            warnings: Vec::new(),
        },
        same_as_last,
    }
}

#[test]
fn the_done_card_shows_the_file_its_facts_and_the_checksum() {
    let i18n = I18n::en_us();
    let lines = |done: &ExportDone| -> Vec<String> {
        fact_lines(done, &i18n).iter().map(|l| plain(l)).collect()
    };
    assert_eq!(
        lines(&done(Some(true))),
        [
            "12 pages · A4 portrait · 1.4 MB · 31 references, 0 stale · variant Base",
            "Bookmarks written: 5 sections, 11 subsections, 6 figures, 3 tables",
            "Checksum sha256 3f9c…a41e · same as the last export",
        ]
    );
    let first = lines(&done(None));
    assert_eq!(first[2], "Checksum sha256 3f9c…a41e");
    let changed = lines(&done(Some(false)));
    assert!(
        changed[2].ends_with("differs from the last export"),
        "{changed:?}"
    );
    let shown = texts(&dialog(Stage::Done(Box::new(done(None)))));
    for words in [
        "Notebook PDF exported",
        "buck-3v3-DN-0042-revB.pdf",
        "Show in folder",
        "Close",
        "Open PDF",
    ] {
        assert!(has(&shown, words), "{words:?} in {shown:?}");
    }
    let card = dialog(Stage::Done(Box::new(done(None))));
    assert!(sent(&clicks(&card, "Open PDF"), |m| {
        matches!(m, NotebookPdfMsg::OpenPdfRequested)
    }));
    assert!(sent(&clicks(&card, "Show in folder"), |m| {
        matches!(m, NotebookPdfMsg::ShowInFolderRequested)
    }));
}

#[test]
fn sizes_and_checksums_read_short() {
    assert_eq!(file_size(10), "1 KB");
    assert_eq!(file_size(820 * 1024), "820 KB");
    assert_eq!(file_size(1_468_006), "1.4 MB");
    assert_eq!(short_checksum("abcdef0123456789"), "abcd…6789");
    assert_eq!(short_checksum("abc"), "abc");
}

#[test]
fn a_landscape_letter_page_reads_so() {
    let i18n = I18n::en_us();
    let paper = PaperSize::of(&PageInfo {
        number: 1,
        width_pt: 792.0,
        height_pt: 612.0,
    });
    assert_eq!(paper.name, PaperName::Letter);
    assert_eq!(plain(&paper_words(&paper, &i18n)), "Letter landscape");
}
