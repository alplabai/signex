//! What the bundled package prints for measurements and attachments
//! (mockup row 199): the verdict table with the same results the crate
//! gives, figures with their source line and stale mark, and a scope CSV
//! drawn as vectors.

use crate::tests_measurement::{EARLIER, MEAS_009};
use crate::{CompileOptions, CompiledNotebook, NotebookSources, PACKAGE_IMPORT, compile};

fn compiled(sources: &NotebookSources) -> CompiledNotebook {
    let compilation = compile(sources, &CompileOptions::default());
    assert!(
        compilation.diagnostics.is_empty(),
        "{:?}",
        compilation.diagnostics
    );
    compilation.notebook.expect("pages")
}

fn notebook(body: &str) -> NotebookSources {
    NotebookSources::new("design.snxnbk", format!("{PACKAGE_IMPORT}\n{body}"))
}

/// The ripple capture with its cursors.
fn ripple(sources: &mut NotebookSources) {
    let mut csv = String::from("Time (s),CH1 (V)\n");
    for i in 0..=200 {
        let t = f64::from(i) * 5e-8;
        let v = 3.3 + 0.009 * (2.0 * std::f64::consts::PI * 5e5 * t).sin();
        csv.push_str(&format!("{t:e},{v}\n"));
    }
    sources.insert("attachments/scope/ripple-2A-revB.csv", csv);
}

const FIGURE: &str = "#attachment(\"scope/ripple-2A-revB.csv\",\n  caption: [Output ripple on +3V3 at 2 A],\n  taken-with: (\"L1\": \"4.7 µH\", \"C15\": \"22 µF\"),\n  channels: ((name: \"CH1\", label: \"+3V3\", coupling: \"AC\", scale: \"5 mV\", bandwidth: \"20 MHz\"),),\n  cursors: (y1: \"9 mV\", y2: \"-9 mV\", x1: \"1 µs\", x2: \"3 µs\"),\n)\n";

#[test]
fn the_block_prints_its_rows_and_the_crates_verdicts() {
    let text = compiled(&notebook(MEAS_009)).text();
    for printed in [
        "MEASUREMENT",
        "MEAS-009",
        "Pass · 4 of 4",
        "Quantity",
        "Tol.",
        "ripple",
    ] {
        assert!(text.contains(printed), "{printed}: {text}");
    }
    // The crate's Error column for each row (tests_measurement).
    for error in ["+0.30", "\u{2212}40", "+1.3", "\u{2212}36"] {
        assert!(text.contains(error), "{error}: {text}");
    }
    assert!(text.contains("j.berg"), "{text}");
    assert!(text.contains("LAB-0117"), "{text}");
    assert!(text.contains("Table 1"), "numbered: {text}");
}

#[test]
fn a_reading_on_its_own_prints_its_result() {
    let text = compiled(&notebook(EARLIER)).text();
    assert!(text.contains("Fail"), "41 mV over 30 mV: {text}");
    assert!(text.contains("Pass"), "{text}");
    assert!(text.contains("+37"), "the error of 41 against 30: {text}");
}

#[test]
fn a_scope_csv_prints_as_a_vector_plot_with_its_labels() {
    let mut sources = notebook(FIGURE);
    ripple(&mut sources);
    let notebook = compiled(&sources);
    let text = notebook.text();
    for printed in [
        "Figure 1",
        "CH1 +3V3 AC",
        "mV/div",
        "ΔY = 18",
        "f = 500",
        "Y1",
        "attachments/scope/ripple-2A-revB.csv",
        "L1 4.7 µH",
    ] {
        assert!(text.contains(printed), "{printed}: {text}");
    }
    assert!(!text.contains("Stale"), "no design values set");
    let ident = "buck-3v3/notebook/design.snxnbk";
    let pdf = notebook.pdf(ident).expect("a pdf");
    let raw = String::from_utf8_lossy(&pdf);
    let pictures = raw.contains("/Subtype /Image");
    assert!(!pictures, "the plot is drawn, not a picture");
    let again = compiled(&sources).pdf(ident).expect("a pdf");
    assert_eq!(pdf, again, "the same file every time");
}

#[test]
fn a_changed_part_prints_the_stale_mark_and_the_change() {
    let mut sources = notebook(FIGURE);
    ripple(&mut sources);
    sources.set_design_values([("L1", "3.3 µH"), ("C15", "22 µF")].into_iter().collect());
    let text = compiled(&sources).text();
    assert!(text.contains("Stale"), "{text}");
    assert!(text.contains("L1 = 4.7 µH"), "{text}");
    assert!(text.contains("is now 3.3 µH"), "{text}");
}

/// The corpus notebook, read the way the app reads it: the files, then
/// the attachments they name.
#[test]
fn the_corpus_measurements_compile_with_their_capture() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../signex-types/tests/corpus/notebook/with-measurements");
    let read = |path: &str| std::fs::read(root.join(path));
    let (mut sources, problems) = NotebookSources::load("design.snxnbk", read);
    assert!(problems.is_empty(), "{problems:?}");
    let missing = sources.missing_attachments();
    assert_eq!(missing, ["attachments/scope/ripple-2A-revB.csv"]);
    for path in missing {
        sources.insert(&path, read(&path).expect("the capture"));
    }
    assert!(sources.missing_attachments().is_empty());
    let text = compiled(&sources).text();
    assert!(text.contains("Pass · 4 of 4"), "{text}");
    assert!(text.contains("ΔY = 18"), "{text}");
    let report = crate::report::measurements_report(&sources);
    assert_eq!(report.summary.readings, 5);
    assert_eq!(report.summary.attachments, 1);
}

#[test]
fn a_missing_or_unreadable_attachment_prints_instead_of_failing() {
    let sources = notebook("#attachment(\"thermal/u3.png\", caption: [Thermal])\n");
    let text = compiled(&sources).text();
    assert!(text.contains("Missing attachment"), "{text}");
    let mut sources = notebook("#attachment(\"scope/bad.csv\")\n");
    sources.insert("attachments/scope/bad.csv", "t,v\n0,1\n1,x\n");
    let text = compiled(&sources).text();
    assert!(text.contains("Cannot draw"), "{text}");
}
