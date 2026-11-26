//! The export settings, the default file name, the title block, the
//! bookmark count, the stale check and the Output Job entry, without a
//! compile.

use std::collections::BTreeSet;

use super::*;
use crate::annotation::AnnotationKind;
use crate::reference::Reference;
use crate::sources::NotebookSources;

#[test]
fn the_default_settings_are_the_dialog_as_drawn() {
    let options = ExportOptions::default();
    assert_eq!(options.pages, PageSelection::All);
    assert_eq!(options.paper, Paper::FromDocument);
    assert_eq!(options.variant, "Base");
    assert!(options.title_block && options.contents && options.bookmarks);
    assert!(!options.python_source, "Python source stays out of print");
    assert_eq!(options.annotations.len(), AnnotationKind::ALL.len());
    assert_eq!(options.stale, StaleRule::Warn);
}

#[test]
fn a_page_selection_reads_all_current_a_range_or_one_page() {
    assert_eq!(PageSelection::parse("all", 4), Some(PageSelection::All));
    assert_eq!(
        PageSelection::parse("current", 4),
        Some(PageSelection::Current { page: 4 })
    );
    let range = PageSelection::Range { first: 3, last: 7 };
    assert_eq!(PageSelection::parse("3-7", 1), Some(range));
    assert_eq!(PageSelection::parse(" 3 \u{2013} 7 ", 1), Some(range));
    assert_eq!(
        PageSelection::parse("5", 1),
        Some(PageSelection::Range { first: 5, last: 5 })
    );
    for bad in ["0", "7-3", "x", "", "1-", "-2"] {
        assert_eq!(PageSelection::parse(bad, 1), None, "{bad:?}");
    }
}

#[test]
fn a_page_selection_resolves_against_the_page_count() {
    assert_eq!(PageSelection::All.resolve(12), Some((1, 12)));
    assert_eq!(PageSelection::All.resolve(0), None);
    assert_eq!(PageSelection::Current { page: 3 }.resolve(12), Some((3, 3)));
    assert_eq!(PageSelection::Current { page: 13 }.resolve(12), None);
    let range = PageSelection::Range { first: 3, last: 20 };
    assert_eq!(range.resolve(12), Some((3, 12)), "cut at the last page");
    let past = PageSelection::Range {
        first: 13,
        last: 20,
    };
    assert_eq!(past.resolve(12), None);
}

#[test]
fn paper_and_stale_tokens_read_back() {
    for paper in Paper::ALL {
        assert_eq!(Paper::parse(paper.token()), Some(paper));
    }
    for rule in StaleRule::ALL {
        assert_eq!(StaleRule::parse(rule.token()), Some(rule));
    }
    assert_eq!(Paper::parse("tabloid"), None);
    assert_eq!(StaleRule::parse("ignore"), None);
    assert_eq!(Paper::Letter.typst_name(), Some("us-letter"));
    assert_eq!(Paper::FromDocument.typst_name(), None);
}

#[test]
fn the_typst_inputs_carry_every_setting() {
    let mut options = ExportOptions::default();
    options.annotations.remove(&AnnotationKind::Issue);
    options.annotations.remove(&AnnotationKind::Note);
    options.paper = Paper::Letter;
    options.contents = false;
    let inputs = options.typst_inputs(&["R7".to_string(), "U3.14".to_string()]);
    let get = |key: &str| {
        inputs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    };
    assert_eq!(get("signex-export"), Some("true"));
    assert_eq!(get("signex-contents"), Some("false"));
    assert_eq!(get("signex-title-block"), Some("true"));
    assert_eq!(get("signex-python-source"), Some("false"));
    assert_eq!(get("signex-hidden-kinds"), Some("note,issue"));
    assert_eq!(get("signex-paper"), Some("us-letter"));
    assert_eq!(get("signex-stale"), Some("R7\nU3.14"));
    assert_eq!(get("signex-variant"), Some("Base"));
    let mut sorted = inputs.clone();
    sorted.sort();
    assert_eq!(inputs, sorted, "one order every time");
    options.paper = Paper::FromDocument;
    let inputs = options.typst_inputs(&[]);
    assert!(inputs.iter().all(|(k, _)| k != "signex-paper"));
}

#[test]
fn the_default_file_name_is_project_document_and_revision() {
    let title = TitleBlock {
        doc: Some("DN-0042".into()),
        rev: Some("B".into()),
        ..TitleBlock::default()
    };
    assert_eq!(
        default_file_name("buck-3v3", "design.snxnbk", &title),
        "buck-3v3-DN-0042-revB.pdf"
    );
    let no_rev = TitleBlock {
        doc: Some("DN-0042".into()),
        ..TitleBlock::default()
    };
    assert_eq!(
        default_file_name("buck-3v3", "design.snxnbk", &no_rev),
        "buck-3v3-DN-0042.pdf"
    );
    assert_eq!(
        default_file_name("buck-3v3", "chapters/power.snxnbk", &TitleBlock::default()),
        "buck-3v3-power.pdf",
        "the file's stem stands in for the document"
    );
    let odd = TitleBlock {
        doc: Some("DN/42: draft?".into()),
        rev: Some(" C. ".into()),
        ..TitleBlock::default()
    };
    assert_eq!(
        default_file_name("a<b>", "design.snxnbk", &odd),
        "a_b_-DN_42_ draft_-revC.pdf"
    );
    assert_eq!(OUTPUT_FOLDERS, ["Project Outputs", "notebook"]);
}

#[test]
fn the_title_block_is_read_from_the_report_call() {
    let source = "+++\nformat = \"snxnbk/1\"\n+++\n#import \"@signex/notebook:0.1.0\": *\n\
                  #show: report.with(doc: \"DN-0042\", rev: \"B\", title: [buck-3v3 Design Notebook], date: \"2026-11-20\")\n= Intro\n";
    let title = TitleBlock::of(source);
    assert_eq!(title.doc.as_deref(), Some("DN-0042"));
    assert_eq!(title.rev.as_deref(), Some("B"));
    assert_eq!(title.title.as_deref(), Some("buck-3v3 Design Notebook"));
    assert_eq!(title.date, crate::IsoDate::parse("2026-11-20"));
    assert_eq!(TitleBlock::of("= No report\n"), TitleBlock::default());
    let computed = TitleBlock::of("#show: report.with(doc: name, rev: 2)\n");
    assert_eq!(computed.doc, None, "a computed value is left out");
    assert_eq!(computed.rev.as_deref(), Some("2"));
    let json = serde_json::to_value(&title).expect("json");
    assert_eq!(json["date"], "2026-11-20");
}

#[test]
fn bookmarks_are_counted_per_section_subsection_figure_and_table() {
    let mut counts = BookmarkCounts::default();
    counts.add_source(
        "= One\n== One a\n=== One a i\n= Two\n\
         #heading(level: 2)[Two a]\n#heading[Three]\n\
         #figure(image(\"scope.png\"), caption: [Ripple])\n\
         #figure(table(columns: 2)[a][b], caption: [Parts])\n\
         #figure(rect(), kind: table, caption: [Also a table])\n\
         #figure(image(\"x.png\"))\n\
         // = not a heading\n\
         `= not a heading either`\n",
    );
    assert_eq!(
        counts,
        BookmarkCounts {
            sections: 3,
            subsections: 3,
            figures: 1,
            tables: 2,
        },
        "an uncaptioned figure and headings in comments and raw text do not count"
    );
    assert_eq!(counts.total(), 9);
}

#[test]
fn bookmarks_are_counted_in_every_chapter() {
    let mut sources = NotebookSources::new("design.snxnbk", "= Intro\n#include \"ch/a.snxnbk\"\n");
    sources.insert("ch/a.snxnbk", "= Power\n== Inductor\n");
    sources.insert("attachments/scope.csv", "= not typst\n");
    let counts = BookmarkCounts::of(&sources);
    assert_eq!((counts.sections, counts.subsections), (2, 1));
}

fn names(list: &[&str]) -> Option<BTreeSet<String>> {
    Some(list.iter().map(|s| (*s).to_string()).collect())
}

#[test]
fn a_reference_the_design_no_longer_has_is_stale() {
    let design = DesignNames {
        components: names(&["U3", "R8"]),
        pins: names(&["U3.3", "U3.12"]),
        nets: names(&["+3V3"]),
    };
    let sources = NotebookSources::new(
        "design.snxnbk",
        "#component(\"R7\") #component(\"R8\") #pin(\"U3.14\") #pin(\"U3\", \"3\")\n\
         #net(\"+3V3\") #net(\"VBUS\") #component(\"R7\")\n",
    );
    let check = StaleCheck::run(&sources, |r| design.has(r));
    assert_eq!(check.references, 6, "distinct references");
    assert_eq!(check.stale_names(), ["R7", "U3.14", "VBUS"]);
}

#[test]
fn a_list_that_was_not_read_calls_nothing_stale() {
    let unknown = DesignNames::default();
    assert!(unknown.has(&Reference::Component("R99".into())));
    assert!(unknown.has(&Reference::Net("X".into())));
    let parts_only = DesignNames {
        components: names(&["U3"]),
        ..DesignNames::default()
    };
    let pin = |component: &str| Reference::Pin {
        component: component.into(),
        pin: "99".into(),
    };
    assert!(parts_only.has(&pin("U3")), "no pin list: the part decides");
    assert!(!parts_only.has(&pin("U9")));
}

#[test]
fn the_output_job_entry_keeps_the_settings_and_reads_back() {
    let options = ExportOptions {
        pages: PageSelection::Current { page: 4 },
        stale: StaleRule::Fail,
        annotations: [AnnotationKind::Issue, AnnotationKind::Decision]
            .into_iter()
            .collect(),
        ..ExportOptions::default()
    };
    let output = NotebookPdfOutput::new("design.snxnbk", &options);
    assert_eq!(output.kind, NOTEBOOK_PDF_OUTPUT_KIND);
    assert!(output.enabled);
    assert_eq!(
        output.options.pages,
        PageSelection::Range { first: 4, last: 4 },
        "a job has no current page"
    );
    assert_eq!(output.options.stale, StaleRule::Fail);
    let text = output.to_toml().expect("toml");
    assert_eq!(NotebookPdfOutput::from_toml(&text), Ok(output.clone()));
    let other = text.replace(NOTEBOOK_PDF_OUTPUT_KIND, "schematic_pdf");
    assert!(NotebookPdfOutput::from_toml(&other).is_err());
}

#[test]
fn the_settings_read_back_from_json_and_fill_in_defaults() {
    let options = ExportOptions {
        pages: PageSelection::Range { first: 3, last: 7 },
        paper: Paper::Letter,
        ..ExportOptions::default()
    };
    let json = serde_json::to_string(&options).expect("json");
    let back: ExportOptions = serde_json::from_str(&json).expect("read back");
    assert_eq!(back, options);
    let sparse: ExportOptions = serde_json::from_str(r#"{"stale": "fail"}"#).expect("sparse");
    assert_eq!(sparse.stale, StaleRule::Fail);
    assert_eq!(sparse.variant, "Base", "the rest is the default");
    let value = serde_json::to_value(&options).expect("json");
    assert_eq!(value["annotations"][0], "note");
    assert!(serde_json::from_str::<ExportOptions>(r#"{"annotations": ["memo"]}"#).is_err());
}
