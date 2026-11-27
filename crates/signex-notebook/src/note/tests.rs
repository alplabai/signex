//! Rich note compiles (#1114): vector output, the cache key, diagnostics in
//! the note's own lines, and the sandbox (no package or file outside the
//! bundled set).

use super::*;

fn input(source: &str) -> NoteInput<'_> {
    NoteInput {
        source,
        width_mm: None,
        text_size_mm: 1.27,
        align: HAlign::Left,
    }
}

const EQUATION: &str =
    "= Buck stage\n*Keep the hot loop small.*\n$V_\"out\" = 0.8 dot (1 + R_7 / R_8)$\n";

#[test]
fn a_note_with_an_equation_draws_as_ink_paths() {
    let compiled = compile_note(&input(EQUATION));
    assert!(!compiled.has_errors(), "{:?}", compiled.diagnostics);
    let drawing = compiled.drawing.expect("a drawing");
    assert!(
        drawing.width_mm > 1.0 && drawing.height_mm > 1.0,
        "{drawing:?}"
    );
    assert!(drawing.shapes.len() > 20, "every glyph is a path");
    assert!(
        drawing
            .shapes
            .iter()
            .all(|s| s.paint == NotePaint::Ink && !s.path.is_empty()),
        "Typst's default black is the note's ink"
    );
    for shape in &drawing.shapes {
        let [x0, y0, x1, y1] = shape.bounds().expect("points");
        assert!(x0 >= -0.5 && y0 >= -0.5, "{x0},{y0}");
        assert!(x1 <= drawing.width_mm + 0.5 && y1 <= drawing.height_mm + 0.5);
    }
}

/// A golden of the vector output for a fixed source, Typst release and font
/// set: the same compile gives the same paths, and their count is pinned.
#[test]
fn the_same_source_gives_the_same_paths() {
    let first = compile_note(&input("$x^2$")).drawing.expect("drawing");
    let second = compile_note(&input("$x^2$")).drawing.expect("drawing");
    assert_eq!(first, second);
    assert_eq!(first.shapes.len(), 2, "x and the 2: {first:?}");
}

#[test]
fn the_key_moves_with_every_input_and_only_then() {
    let base = input("*a*");
    assert_eq!(note_key(&base), note_key(&input("*a*")));
    assert_ne!(note_key(&base), note_key(&input("*b*")));
    let wider = NoteInput {
        width_mm: Some(40.0),
        ..base
    };
    assert_ne!(note_key(&base), note_key(&wider));
    let bigger = NoteInput {
        text_size_mm: 2.54,
        ..base
    };
    assert_ne!(note_key(&base), note_key(&bigger));
    let centred = NoteInput {
        align: HAlign::Center,
        ..base
    };
    assert_ne!(note_key(&base), note_key(&centred));
    let unusable = NoteInput {
        width_mm: Some(f64::NAN),
        ..base
    };
    assert_eq!(
        note_key(&base),
        note_key(&unusable),
        "a width that is not a length lays out as Fit text"
    );
}

#[test]
fn a_narrower_width_wraps_the_text_taller() {
    let text = "Place C14 within 2 mm of U3 pin 1, on the same layer as the switch node.";
    let wide = compile_note(&input(text)).drawing.expect("drawing");
    let narrow = compile_note(&NoteInput {
        width_mm: Some(20.0),
        ..input(text)
    })
    .drawing
    .expect("drawing");
    assert!(
        narrow.height_mm > wide.height_mm * 1.5,
        "{narrow:?} {wide:?}"
    );
    assert!(narrow.width_mm <= 20.0 + 1e-6);
}

#[test]
fn an_error_names_its_line_in_the_note() {
    let compiled = compile_note(&input("fine\nalso fine\n#unknown-function()\n"));
    assert!(compiled.has_errors());
    assert!(compiled.drawing.is_none());
    let error = compiled.first_error().expect("an error");
    assert_eq!(error.file, NOTE_FILE);
    assert_eq!(error.line, 3, "{error:?}");
}

#[test]
fn a_package_outside_the_bundled_set_is_an_error() {
    let compiled = compile_note(&input("#import \"@preview/physica:0.9.3\": *\n$dv(f, x)$"));
    assert!(compiled.has_errors(), "{:?}", compiled.diagnostics);
    assert!(compiled.drawing.is_none());
}

#[test]
fn a_note_reads_no_file() {
    for source in ["#read(\"/etc/hostname\")", "#image(\"logo.png\")"] {
        let compiled = compile_note(&input(source));
        assert!(
            compiled.has_errors(),
            "{source}: {:?}",
            compiled.diagnostics
        );
    }
}

#[test]
fn a_link_is_kept_with_its_place_and_target() {
    let source = "See #link(\"assets/docs/layout-guide.pdf\")[the layout guide].";
    let drawing = compile_note(&input(source)).drawing.expect("drawing");
    assert_eq!(drawing.links.len(), 1, "{:?}", drawing.links);
    let link = &drawing.links[0];
    assert_eq!(link.target, "assets/docs/layout-guide.pdf");
    let [x0, y0, x1, y1] = link.rect;
    assert!(x1 > x0 && y1 > y0);
    assert!(link.contains((x0 + x1) / 2.0, (y0 + y1) / 2.0));
    assert!(!link.contains(x0 - 1.0, y0));
}

#[test]
fn a_colour_the_source_chose_is_kept() {
    let drawing = compile_note(&input("#text(fill: red)[A]"))
        .drawing
        .expect("drawing");
    assert!(
        drawing
            .shapes
            .iter()
            .any(|s| s.paint == NotePaint::Rgba([255, 65, 54, 255])),
        "{:?}",
        drawing.shapes.iter().map(|s| s.paint).collect::<Vec<_>>()
    );
}

#[test]
fn a_curve_flattens_into_closed_contours() {
    let shape = NoteShape {
        path: vec![
            NotePathOp::Move([0.0, 0.0]),
            NotePathOp::Quad([1.0, 2.0], [2.0, 0.0]),
            NotePathOp::Close,
            NotePathOp::Move([5.0, 5.0]),
            NotePathOp::Line([6.0, 5.0]),
            NotePathOp::Line([6.0, 6.0]),
            NotePathOp::Close,
        ],
        kind: NoteShapeKind::Fill { even_odd: false },
        paint: NotePaint::Ink,
    };
    let contours = shape.contours();
    assert_eq!(contours.len(), 2);
    assert_eq!(contours[0].len(), 9, "the start and eight steps");
    assert_eq!(contours[0][8], [2.0, 0.0]);
    assert_eq!(contours[1], [[5.0, 5.0], [6.0, 5.0], [6.0, 6.0]]);
    assert_eq!(shape.bounds(), Some([0.0, 0.0, 6.0, 6.0]));
}
