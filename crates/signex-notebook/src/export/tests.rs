//! The export over Typst: reproducible bytes, the stale rule, the pages
//! chosen, the paper, the bookmarks and the settings the page reads.

use std::path::Path;

use super::*;
use crate::PACKAGE_IMPORT;
use crate::annotation::AnnotationKind;
use crate::reference::Reference;
use crate::sources::NotebookSources;

const IDENT: &str = "buck-3v3/notebook/design.snxnbk";

/// The corpus notebook with a chapter, as the app loads it.
fn corpus() -> NotebookSources {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../signex-types/tests/corpus/notebook/with-chapters");
    let (sources, problems) =
        NotebookSources::load("design.snxnbk", |path| std::fs::read(root.join(path)));
    assert!(problems.is_empty(), "{problems:?}");
    sources
}

fn live(_: &Reference) -> bool {
    true
}

fn export(sources: &NotebookSources, options: &ExportOptions) -> NotebookExport {
    match export_pdf(sources, options, IDENT, live) {
        Ok(export) => export,
        Err(error) => panic!("the export failed: {error}"),
    }
}

/// A notebook in the report layout with `body` after the header.
fn report_notebook(body: &str) -> NotebookSources {
    NotebookSources::new(
        "design.snxnbk",
        format!(
            "{PACKAGE_IMPORT}\n#show: report.with(doc: \"DN-7\", rev: \"A\", title: [T], date: \"2026-11-20\")\n{body}"
        ),
    )
}

#[test]
fn two_exports_of_the_notebook_are_the_same_file() {
    let options = ExportOptions::default();
    let first = export(&corpus(), &options);
    let second = export(&corpus(), &options);
    assert!(first.pdf.starts_with(b"%PDF-"));
    assert_eq!(first.pdf, second.pdf, "byte for byte");
    assert_eq!(first.summary.sha256, second.summary.sha256);
    assert_eq!(first.summary.sha256, sha256_hex(&first.pdf));
    assert_eq!(first.summary.sha256.len(), 64);
    assert_eq!(first.summary.bytes, first.pdf.len());
    let text = String::from_utf8_lossy(&first.pdf);
    assert!(!text.contains("/CreationDate"), "no creation time");
}

#[test]
fn the_summary_names_the_pages_paper_title_and_references() {
    let summary = export(&corpus(), &ExportOptions::default()).summary;
    assert!(summary.page_count >= 1);
    assert_eq!(
        (summary.first_page, summary.last_page),
        (1, summary.page_count)
    );
    assert_eq!(summary.pages, summary.page_count);
    assert_eq!(summary.paper.name, PaperName::A4);
    assert!(!summary.paper.landscape);
    assert!(
        (summary.paper.width_mm - 210.0).abs() < 0.5,
        "{:?}",
        summary.paper
    );
    assert_eq!(summary.title.doc.as_deref(), Some("DN-0042"));
    assert_eq!(summary.title.rev.as_deref(), Some("B"));
    assert_eq!(summary.variant, "Base");
    assert!(summary.references >= 6, "{}", summary.references);
    assert!(summary.stale.is_empty());
    let json = serde_json::to_value(&summary).expect("json");
    assert_eq!(json["paper"]["name"], "a4");
    assert_eq!(json["bookmarks"]["sections"], 2);
}

#[test]
fn letter_paper_lays_the_pages_out_again() {
    let options = ExportOptions {
        paper: Paper::Letter,
        ..ExportOptions::default()
    };
    let summary = export(&corpus(), &options).summary;
    assert_eq!(summary.paper.name, PaperName::Letter);
}

#[test]
fn a_stale_reference_fails_the_export_only_under_fail() {
    let design = DesignNames {
        components: Some(
            ["U3", "R8", "L1"]
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
        ),
        ..DesignNames::default()
    };
    let fail = ExportOptions {
        stale: StaleRule::Fail,
        ..ExportOptions::default()
    };
    match export_pdf(&corpus(), &fail, IDENT, |r| design.has(r)) {
        Err(ExportError::Stale(check)) => assert_eq!(check.stale_names(), ["R7"]),
        other => panic!("refused as stale, got {other:?}"),
    }
    let warned = export_pdf(&corpus(), &ExportOptions::default(), IDENT, |r| {
        design.has(r)
    })
    .expect("Warn exports");
    assert_eq!(warned.summary.stale, [Reference::Component("R7".into())]);
    let clean = export(&corpus(), &ExportOptions::default());
    assert_ne!(
        warned.pdf, clean.pdf,
        "the stale reference carries its mark"
    );
}

#[test]
fn the_pages_chosen_are_the_pages_written() {
    let sources = report_notebook("= One\nA\n#pagebreak()\n= Two\nB\n#pagebreak()\n= Three\nC\n");
    let all = export(&sources, &ExportOptions::default()).summary;
    assert_eq!(all.page_count, 3);
    let options = ExportOptions {
        pages: PageSelection::Range { first: 2, last: 3 },
        ..ExportOptions::default()
    };
    let range = export(&sources, &options);
    assert_eq!(
        (
            range.summary.pages,
            range.summary.first_page,
            range.summary.last_page
        ),
        (2, 2, 3)
    );
    let current = ExportOptions {
        pages: PageSelection::Current { page: 1 },
        ..ExportOptions::default()
    };
    assert_eq!(export(&sources, &current).summary.pages, 1);
    let past = ExportOptions {
        pages: PageSelection::Range { first: 9, last: 9 },
        ..ExportOptions::default()
    };
    assert_eq!(
        export_pdf(&sources, &past, IDENT, live).err(),
        Some(ExportError::NoPages { page_count: 3 })
    );
}

#[test]
fn a_notebook_that_does_not_compile_is_refused_with_its_errors() {
    let sources = NotebookSources::new("design.snxnbk", "#nope\n");
    match export_pdf(&sources, &ExportOptions::default(), IDENT, live) {
        Err(error @ ExportError::Compile(_)) => {
            assert!(error.to_string().contains("design.snxnbk:1:"), "{error}");
        }
        other => panic!("a compile error, got {other:?}"),
    }
}

#[test]
fn bookmarks_left_out_write_none_and_count_none() {
    let sources = report_notebook(
        "= Power\n== Inductor\n#figure(rect(), caption: [Ripple])\n\
         #figure(table(columns: 1)[a], caption: [Parts])\n",
    );
    let on = export(&sources, &ExportOptions::default());
    assert_eq!(
        on.summary.bookmarks,
        BookmarkCounts {
            sections: 1,
            subsections: 1,
            figures: 1,
            tables: 1,
        }
    );
    assert!(String::from_utf8_lossy(&on.pdf).contains("/Outlines"));
    let off_options = ExportOptions {
        bookmarks: false,
        ..ExportOptions::default()
    };
    let off = export(&sources, &off_options);
    assert_eq!(off.summary.bookmarks, BookmarkCounts::default());
    assert_ne!(off.pdf, on.pdf, "no bookmark is written");
}

/// #3021: the bookmark count reads the laid-out pages: a captioned
/// `#attachment` counts as a figure, and `--pages` counts only the pages
/// exported.
#[test]
fn bookmarks_count_attachment_figures_and_only_the_pages_exported() {
    let mut sources = report_notebook(
        "= One\n#attachment(\"scope/a.csv\", caption: [Ripple])\n#pagebreak()\n= Two\nB\n",
    );
    // Where the app reads it from: `notebook/attachments/scope/a.csv`.
    sources.insert("attachments/scope/a.csv", "t,v\n0,1\n1,2\n");
    assert!(sources.missing_attachments().is_empty(), "the CSV loads");
    let printed = crate::compile(&sources, &crate::CompileOptions::default())
        .notebook
        .expect("compiles")
        .text();
    assert!(printed.contains("Ripple"), "{printed}");
    assert!(!printed.contains("Missing attachment"), "{printed}");
    let all = export(&sources, &ExportOptions::default())
        .summary
        .bookmarks;
    assert_eq!((all.sections, all.figures), (2, 1));
    let second = ExportOptions {
        pages: PageSelection::Range { first: 2, last: 2 },
        ..ExportOptions::default()
    };
    let part = export(&sources, &second).summary.bookmarks;
    assert_eq!((part.sections, part.figures), (1, 0));
}

/// #3020: a compile that never ends is stopped at its limit and reported,
/// so the preview and the command line go on.
#[test]
fn a_looping_notebook_is_stopped_and_reported() {
    let sources = NotebookSources::new(
        "design.snxnbk",
        "#for i in range(100000) { for j in range(100000) {} }\n",
    );
    let started = std::time::Instant::now();
    let compilation = crate::compile::compile_within(
        &sources,
        &crate::CompileOptions::default(),
        &[],
        std::time::Duration::from_secs(1),
    );
    assert!(started.elapsed() < std::time::Duration::from_secs(10));
    assert!(compilation.notebook.is_none());
    assert!(compilation.has_errors());
    let said = &compilation.diagnostics[0].message;
    assert!(said.contains("did not finish compiling"), "{said}");
}

#[test]
fn an_annotation_kind_left_out_prints_nothing() {
    let sources = report_notebook("#issue(id: \"ISS-001\", title: [Hot])[Too warm.]\n");
    let all = export(&sources, &ExportOptions::default());
    let mut options = ExportOptions::default();
    options.annotations.remove(&AnnotationKind::Issue);
    let without = export(&sources, &options);
    assert_ne!(all.pdf, without.pdf);
    options.annotations.insert(AnnotationKind::Issue);
    assert_eq!(
        export(&sources, &options).pdf,
        all.pdf,
        "the setting alone decides"
    );
}

#[test]
fn the_title_block_and_contents_print_only_when_asked() {
    let sources = report_notebook("= One\nText.\n");
    let both = export(&sources, &ExportOptions::default());
    let neither = ExportOptions {
        title_block: false,
        contents: false,
        ..ExportOptions::default()
    };
    let neither = export(&sources, &neither);
    assert_ne!(both.pdf, neither.pdf);
}

#[test]
fn the_revision_date_answers_today() {
    let sources = report_notebook("#datetime.today().display()\n");
    export(&sources, &ExportOptions::default());
    let undated = NotebookSources::new("design.snxnbk", "#datetime.today().display()\n");
    assert!(matches!(
        export_pdf(&undated, &ExportOptions::default(), IDENT, live),
        Err(ExportError::Compile(_))
    ));
}

#[test]
fn the_inspection_counts_pages_and_names_the_paper_and_title() {
    let inspection = inspect(&corpus(), |r| r.name() != "R7");
    assert!(inspection.page_count >= 1);
    assert_eq!(inspection.paper.map(|p| p.name), Some(PaperName::A4));
    assert_eq!(inspection.title.doc.as_deref(), Some("DN-0042"));
    assert_eq!(inspection.stale.stale_names(), ["R7"]);
    assert!(
        inspection.diagnostics.is_empty(),
        "{:?}",
        inspection.diagnostics
    );
    let broken = inspect(&NotebookSources::new("design.snxnbk", "#nope\n"), live);
    assert_eq!((broken.page_count, broken.paper), (0, None));
    assert!(!broken.diagnostics.is_empty());
}

#[test]
fn a_page_size_is_named_a4_letter_or_other() {
    let page = |w: f64, h: f64| crate::PageInfo {
        number: 1,
        width_pt: w,
        height_pt: h,
    };
    assert_eq!(PaperSize::of(&page(595.28, 841.89)).name, PaperName::A4);
    let landscape = PaperSize::of(&page(841.89, 595.28));
    assert_eq!((landscape.name, landscape.landscape), (PaperName::A4, true));
    assert_eq!(PaperSize::of(&page(612.0, 792.0)).name, PaperName::Letter);
    assert_eq!(PaperSize::of(&page(100.0, 50.0)).name, PaperName::Other);
}
