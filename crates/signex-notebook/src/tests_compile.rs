//! The compile pipeline over Typst: pages, chapter includes, diagnostics in
//! the file's own lines, the preview image and a reproducible PDF.

use std::path::Path;

use crate::{
    CompileOptions, CompiledNotebook, DiagnosticSeverity, IsoDate, NotebookSources, PACKAGE_IMPORT,
    compile,
};

/// The corpus notebook with a chapter, loaded the way the app loads it.
fn corpus_notebook() -> NotebookSources {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../signex-types/tests/corpus/notebook/with-chapters");
    let (sources, problems) =
        NotebookSources::load("design.snxnbk", |path| std::fs::read(root.join(path)));
    assert!(problems.is_empty(), "{problems:?}");
    sources
}

fn compiled(sources: &NotebookSources) -> CompiledNotebook {
    let compilation = compile(sources, &CompileOptions::default());
    assert!(
        compilation.diagnostics.is_empty(),
        "{:?}",
        compilation.diagnostics
    );
    compilation.notebook.expect("pages")
}

fn one_file(text: &str) -> NotebookSources {
    NotebookSources::new("design.snxnbk", text)
}

#[test]
fn the_corpus_notebook_compiles_with_its_chapter() {
    let sources = corpus_notebook();
    assert_eq!(
        sources.paths().collect::<Vec<_>>(),
        ["chapters/power.snxnbk", "design.snxnbk"]
    );
    let notebook = compiled(&sources);
    let pages = notebook.pages();
    assert!(!pages.is_empty());
    assert_eq!(pages.len(), notebook.page_count());
    assert_eq!(pages[0].number, 1);
    // A4 portrait from the report layout: 595.28 x 841.89 pt.
    assert!((pages[0].width_pt - 595.28).abs() < 0.1, "{:?}", pages[0]);
    assert!((pages[0].height_pt - 841.89).abs() < 0.1, "{:?}", pages[0]);
}

#[test]
fn two_exports_of_the_same_notebook_are_the_same_file() {
    let sources = corpus_notebook();
    let ident = "buck-3v3/notebook/design.snxnbk";
    let first = compiled(&sources).pdf(ident).expect("a pdf");
    let second = compiled(&corpus_notebook()).pdf(ident).expect("a pdf");
    assert!(first.starts_with(b"%PDF-"), "a PDF");
    assert_eq!(first, second, "byte for byte");
}

#[test]
fn the_pdf_carries_no_creation_time() {
    let pdf = compiled(&corpus_notebook()).pdf("id").expect("a pdf");
    let text = String::from_utf8_lossy(&pdf);
    assert!(!text.contains("/CreationDate"), "no creation time");
    assert!(!text.contains("/ModDate"), "no modification time");
}

#[test]
fn the_pdf_identifier_follows_the_ident() {
    let notebook = compiled(&corpus_notebook());
    let a = notebook.pdf("project-a/design.snxnbk").expect("a pdf");
    let b = notebook.pdf("project-b/design.snxnbk").expect("a pdf");
    assert_ne!(a, b, "the document id comes from the ident");
}

#[test]
fn a_page_is_drawn_for_the_preview() {
    let notebook = compiled(&one_file(&format!(
        "{PACKAGE_IMPORT}\n#set page(width: 100pt, height: 50pt)\nHello\n"
    )));
    let image = notebook.render_page(0, 2.0).expect("an image");
    assert_eq!((image.width, image.height), (200, 100));
    assert_eq!(image.rgba.len(), 200 * 100 * 4);
    assert_eq!(&image.rgba[..4], &[255, 255, 255, 255], "white paper");
    assert!(notebook.render_page(1, 2.0).is_none(), "past the last page");
    assert!(notebook.render_page(0, 0.0).is_none());
    assert!(notebook.render_page(0, f64::NAN).is_none());
}

#[test]
fn an_error_is_placed_on_its_line_in_the_file() {
    let text = "+++\nformat = \"snxnbk/1\"\n+++\n= Title\n#undefined-function()\n";
    let compilation = compile(&one_file(text), &CompileOptions::default());
    assert!(compilation.has_errors());
    assert!(compilation.notebook.is_none());
    let error = &compilation.diagnostics[0];
    assert_eq!(error.severity, DiagnosticSeverity::Error);
    assert_eq!(
        (error.file.as_str(), error.line, error.column),
        ("design.snxnbk", 5, 2)
    );
    assert!(error.message.contains("unknown variable"), "{error:?}");
}

#[test]
fn an_error_in_a_chapter_names_the_chapter() {
    let mut sources = one_file("= Notebook\n#include \"chapters/broken.snxnbk\"\n");
    sources.insert(
        "chapters/broken.snxnbk",
        "+++\n+++\nFine.\n\nStill fine #nope\n",
    );
    let compilation = compile(&sources, &CompileOptions::default());
    let error = &compilation.diagnostics[0];
    assert_eq!(
        (error.file.as_str(), error.line),
        ("chapters/broken.snxnbk", 5)
    );
}

#[test]
fn a_missing_chapter_is_an_error_where_it_is_included() {
    let sources = one_file("= Notebook\n\n#include \"chapters/missing.snxnbk\"\n");
    let compilation = compile(&sources, &CompileOptions::default());
    assert!(compilation.has_errors());
    let error = &compilation.diagnostics[0];
    assert_eq!((error.file.as_str(), error.line), ("design.snxnbk", 3));
}

#[test]
fn only_the_bundled_package_is_served() {
    let sources = one_file("#import \"@preview/anything:1.0.0\": *\nBody\n");
    let compilation = compile(&sources, &CompileOptions::default());
    assert!(compilation.has_errors(), "no download, no other package");
    let sources = one_file("#import \"@signex/notebook:9.9.9\": *\nBody\n");
    assert!(compile(&sources, &CompileOptions::default()).has_errors());
}

#[test]
fn today_is_only_what_the_caller_passes() {
    let sources = one_file("#datetime.today().display()\n");
    assert!(
        compile(&sources, &CompileOptions::default()).has_errors(),
        "no clock by default"
    );
    let options = CompileOptions {
        today: IsoDate::parse("2026-11-25"),
    };
    let compilation = compile(&sources, &options);
    assert!(!compilation.has_errors(), "{:?}", compilation.diagnostics);
}

#[test]
fn every_block_function_and_reference_prints() {
    let text = format!(
        "{PACKAGE_IMPORT}\n\
         #note(id: \"NOTE-012\", title: [Copper], target: net(\"SW\"))[Keep it small.]\n\
         #calculation(id: \"CALC-2.1\", title: [Output], status: \"current\")[$ V = 3.3 $]\n\
         #measurement(id: \"MEAS-010\", target: net(\"VIN\"), value: 310, unit: \"mV\", limit: (max: 250))\n\
         #issue(id: \"ISS-004\", title: [Above +2 %], status: \"open\", owner: \"j.berg\")[Text.]\n\
         #decision(id: \"DEC-002\", status: \"accepted\", chosen: [A], target: (component(\"L1\"), pin(\"U3\", \"3\")))[Why.]\n\
         #annotation(kind: \"issue\", status: \"open\", date: \"2026-11-17\")[Old form.]\n\
         #component(\"R7\") #pin(\"U3\", \"12\") #pin(\"U3.3\") #net(\"+3V3\")\n"
    );
    let notebook = compiled(&one_file(&text));
    assert_eq!(notebook.page_count(), 1);
}

#[test]
fn a_diagnostic_is_a_json_object() {
    let compilation = compile(&one_file("#nope\n"), &CompileOptions::default());
    let json = serde_json::to_value(&compilation.diagnostics[0]).expect("json");
    assert_eq!(json["severity"], "error");
    assert_eq!(json["file"], "design.snxnbk");
    assert_eq!(json["line"], 1);
}
