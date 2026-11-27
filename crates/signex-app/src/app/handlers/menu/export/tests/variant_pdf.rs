//! The Enhanced Output exit criterion "a 4-channel design with two variants
//! exports a correct per-variant PDF" (docs/ROADMAP.md): the variant-BOM
//! fixture project (`variant_bom.rs`) exported through Export PDF once per
//! variant, Base included, with Use Physical Structure on and a note that
//! prints `${VARIANT}`, each file compared with its golden in
//! `testdata/variant_pdf/`.
//!
//! The golden is the PDF as text: its page count, every outline (bookmark)
//! title, every text-showing operator line, and the count and FNV-1a hash
//! of the other drawing operator lines, each number to two decimals. Font
//! programs and object numbers are left out. Regenerate them with
//! `UPDATE_GOLDEN=1 cargo test -p signex-app variant_pdf`; a diff is a
//! behaviour change of the PDF. Layout: `docs/PDF_EXPORT.md`, "Per-variant
//! PDF goldens".

use std::path::{Path, PathBuf};

use signex_types::schematic::{HAlign, Point, TextNote, VAlign};

use super::base_variant::export;
use super::open_with;
use super::variant_bom::fixture;
use crate::app::Signex;

const GOLDEN_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/app/handlers/menu/export/tests/testdata/variant_pdf"
);

/// The variants, Base first, by golden name.
const VARIANTS: [(&str, Option<&str>); 3] = [
    ("base", None),
    ("full", Some("Full")),
    ("lite", Some("Lite")),
];

/// The PDF content operators the golden keeps. A text-showing one is
/// listed line by line; the rest are counted and hashed.
const TEXT_OPERATORS: &[&str] = &["Tj", "TJ"];
const DRAWING_OPERATORS: &[&str] = &[
    "m", "l", "c", "v", "y", "h", "re", "S", "s", "f", "F", "f*", "B", "B*", "b", "b*", "n", "W",
    "W*", "q", "Q", "cm", "w", "J", "j", "M", "d", "gs", "rg", "RG", "g", "G", "k", "K", "BT",
    "ET", "Tf", "Td", "TD", "Tm", "T*", "BDC", "EMC", "Do",
];

/// The fixture project with a note on `top` that prints the variant.
fn project() -> Signex {
    let mut app = fixture();
    let top = app
        .document_state
        .active_path
        .clone()
        .expect("top is in view");
    let mut sheet = app.document_state.engines[&top].document().clone();
    sheet.text_notes.push(TextNote {
        uuid: uuid::Uuid::from_u128(0x0E07),
        text: "Variant: [${VARIANT}]".to_string(),
        position: Point::new(20.0, 20.0),
        rotation: 0.0,
        font_size: 1.27,
        font_family: None,
        justify_h: HAlign::Left,
        justify_v: VAlign::Bottom,
        locked: false,
    });
    open_with(&mut app.document_state, &top, sheet);
    app
}

/// A number token to two decimals, `-0.00` as `0.00`; any other token as
/// it is.
fn token(token: &str) -> String {
    match token.parse::<f64>() {
        Ok(value) if token.contains('.') => {
            let value = format!("{value:.2}");
            if value == "-0.00" {
                "0.00".to_string()
            } else {
                value
            }
        }
        _ => token.to_string(),
    }
}

/// FNV-1a, 64 bit: the hash of the drawing lines.
fn fnv1a(lines: &[String]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in lines.iter().flat_map(|line| line.bytes().chain(*b"\n")) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// The PDF as its golden spells it.
fn golden_text(pdf: &[u8]) -> String {
    let mut pages = 0usize;
    let mut outlines = Vec::new();
    let mut texts = Vec::new();
    let mut drawing = Vec::new();
    for raw in pdf.split(|byte| *byte == b'\n') {
        if raw.is_empty() || !raw.iter().all(|byte| (0x20..0x7f).contains(byte)) {
            continue;
        }
        let line = String::from_utf8_lossy(raw);
        let line = line.trim();
        if line == "/Type /Page" {
            pages += 1;
        }
        if let Some(at) = line.find("/Title ") {
            outlines.push(line[at + "/Title ".len()..].to_string());
            continue;
        }
        let Some(operator) = line.split_whitespace().last() else {
            continue;
        };
        let normalised = line
            .split_whitespace()
            .map(token)
            .collect::<Vec<_>>()
            .join(" ");
        if TEXT_OPERATORS.contains(&operator) {
            texts.push(normalised);
        } else if DRAWING_OPERATORS.contains(&operator) {
            drawing.push(normalised);
        }
    }
    let mut out = format!("pages {pages}\n");
    for title in outlines {
        out.push_str(&format!("outline {title}\n"));
    }
    for text in texts {
        out.push_str(&format!("text {text}\n"));
    }
    out.push_str(&format!(
        "drawing {} lines, fnv1a {:016x}\n",
        drawing.len(),
        fnv1a(&drawing)
    ));
    out
}

fn golden(name: &str) -> PathBuf {
    Path::new(GOLDEN_DIR).join(format!("{name}.pdf.txt"))
}

fn assert_golden(name: &str, actual: &str) {
    let path = golden(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(GOLDEN_DIR).expect("the golden folder");
        std::fs::write(&path, actual).expect("write the golden");
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|why| {
            panic!(
                "read {}: {why}; write it with UPDATE_GOLDEN=1 cargo test -p signex-app variant_pdf",
                path.display()
            )
        })
        .replace("\r\n", "\n");
    assert_eq!(
        actual,
        expected,
        "the {name} PDF differs from {}; a diff is a behaviour change",
        path.display()
    );
}

#[test]
fn each_variant_exports_its_golden_pdf() {
    for (name, variant) in VARIANTS {
        let pdf = golden_text(&export(&mut project(), variant));
        // A second project built the same way prints the same PDF, so the
        // golden pins the export, not an ordering of this run.
        let again = golden_text(&export(&mut project(), variant));
        assert_eq!(pdf, again, "the {name} PDF is not deterministic");
        assert_golden(name, &pdf);
    }
}

/// The goldens say what the variants are for, so a regenerated golden that
/// lost a variant's difference does not pass unseen: Base names no variant,
/// Full and Lite tag their sheet bookmarks and print their name.
#[test]
fn the_pdf_goldens_differ_where_the_variants_do() {
    let read = |name| std::fs::read_to_string(golden(name)).expect("a golden");
    let (base, full, lite) = (read("base"), read("full"), read("lite"));
    let outlines = |golden: &str| -> Vec<String> {
        golden
            .lines()
            .filter(|line| line.starts_with("outline "))
            .map(str::to_string)
            .collect()
    };
    let tagged = |golden: &str, tag: &str| outlines(golden).iter().any(|line| line.contains(tag));
    assert!(
        !tagged(&base, "[Full]") && !tagged(&base, "[Lite]"),
        "{base}"
    );
    assert!(
        tagged(&full, " [Full]") && !tagged(&full, "[Lite]"),
        "{full}"
    );
    assert!(
        tagged(&lite, " [Lite]") && !tagged(&lite, "[Full]"),
        "{lite}"
    );
    let texts = |golden: &str| -> Vec<String> {
        golden
            .lines()
            .filter(|line| line.starts_with("text "))
            .map(str::to_string)
            .collect()
    };
    assert_ne!(texts(&base), texts(&full), "Full prints its name");
    assert_ne!(texts(&full), texts(&lite), "Lite prints its name");
    // Lite leaves parts out: they draw greyed and crossed out (#1294), so
    // its drawing is not Base's.
    let drawing = |golden: &str| {
        (golden.lines())
            .find(|line| line.starts_with("drawing "))
            .map(str::to_string)
    };
    assert_ne!(
        drawing(&base),
        drawing(&lite),
        "Lite draws its not-fitted parts"
    );
    for golden in [&base, &full, &lite] {
        assert!(golden.starts_with("pages "), "{golden}");
        assert!(!texts(golden).is_empty(), "the pages print text");
    }
}

/// The golden text keeps what it is for and drops what it is not.
#[test]
fn the_golden_text_keeps_pages_outlines_text_and_a_drawing_hash() {
    let pdf = b"%PDF-1.7\n<<\n  /Type /Pages\n>>\n<<\n  /Type /Page\n>>\n\
        /Title (top [Lite])\nq\n1.004 -0.001 m\n3 4 l\n<0012> Tj\nQ\n\x00\xff m\n/Length 12\n";
    let text = golden_text(pdf);
    assert_eq!(
        text.lines().take(3).collect::<Vec<_>>(),
        ["pages 1", "outline (top [Lite])", "text <0012> Tj"]
    );
    let drawing = ["q", "1.00 0.00 m", "3 4 l", "Q"].map(str::to_string);
    assert_eq!(
        text.lines().nth(3),
        Some(format!("drawing 4 lines, fnv1a {:016x}", fnv1a(&drawing)).as_str())
    );
    assert_eq!(text.lines().count(), 4);
}
