//! A rich text note (#1114, mockup row 203) in the SVG and PDF export: its
//! Typst source compiled as the canvas compiles it and drawn as vector
//! paths, with its Frame section. A note that does not compile prints its
//! source as text, as a plain note would. Rules:
//! `docs/architecture/snxsch-note-format.md`, "Drawing".

use signex_notebook::note::{
    NoteDrawing, NoteInput, NotePaint, NotePathOp, NoteShapeKind, PlacedNote, compile_note,
};
use signex_types::schematic::{NoteFrame, SchematicDefaults, TextNote};

use super::{SvgElement, SvgPathCommand, SvgPoint, SvgStyle};
use crate::pdf::layout::PageTransform;

/// The rich note's elements, or `None` when its source does not compile.
pub(super) fn rich_note_elements(
    note: &TextNote,
    xform: &PageTransform,
    ink: (f32, f32, f32),
    paper: (f32, f32, f32),
    defaults: &SchematicDefaults,
) -> Option<Vec<SvgElement>> {
    let drawing = compile_note(&NoteInput::of(note)).drawing?;
    Some(drawing_elements(
        note, &drawing, xform, ink, paper, defaults,
    ))
}

/// The elements of `note` drawn as `drawing`: its frame, then its shapes.
pub(super) fn drawing_elements(
    note: &TextNote,
    drawing: &NoteDrawing,
    xform: &PageTransform,
    ink: (f32, f32, f32),
    paper: (f32, f32, f32),
    defaults: &SchematicDefaults,
) -> Vec<SvgElement> {
    let placed = PlacedNote::of(note, Some(drawing));
    let point = |p: [f64; 2]| SvgPoint {
        x: xform.x(p[0]),
        y: xform.px_y(p[1]),
    };
    let mut out = Vec::with_capacity(drawing.shapes.len() + 1);
    if note.rich.frame != NoteFrame::None {
        let border = note
            .rich
            .border_mm
            .unwrap_or_else(|| defaults.shape_stroke_mm(0.0));
        let corners = placed.corners();
        let mut commands = vec![SvgPathCommand::MoveTo(point(corners[0]))];
        commands.extend(
            corners[1..]
                .iter()
                .map(|c| SvgPathCommand::LineTo(point(*c))),
        );
        commands.push(SvgPathCommand::Close);
        out.push(SvgElement::Path {
            commands,
            style: SvgStyle {
                stroke_alpha: 1.0,
                stroke_rgb: Some(ink),
                fill_rgb: (note.rich.frame == NoteFrame::Filled).then_some(paper),
                stroke_width: (border * xform.mm_to_unit) as f32,
            },
        });
    }
    for shape in &drawing.shapes {
        if !shape.bounds().is_some_and(|b| placed.keeps(b)) {
            continue;
        }
        let colour = match shape.paint {
            NotePaint::Ink => ink,
            NotePaint::Rgba([r, g, b, _]) => (
                f32::from(r) / 255.0,
                f32::from(g) / 255.0,
                f32::from(b) / 255.0,
            ),
        };
        let at = |p: [f64; 2]| point(placed.text_to_sheet(p));
        let mut commands = Vec::with_capacity(shape.path.len());
        let mut last = [0.0, 0.0];
        for op in &shape.path {
            match *op {
                NotePathOp::Move(p) => {
                    commands.push(SvgPathCommand::MoveTo(at(p)));
                    last = p;
                }
                NotePathOp::Line(p) => {
                    commands.push(SvgPathCommand::LineTo(at(p)));
                    last = p;
                }
                NotePathOp::Quad(c, p) => {
                    // The cubic that draws the same quadratic.
                    let c1 = [
                        last[0] + 2.0 / 3.0 * (c[0] - last[0]),
                        last[1] + 2.0 / 3.0 * (c[1] - last[1]),
                    ];
                    let c2 = [
                        p[0] + 2.0 / 3.0 * (c[0] - p[0]),
                        p[1] + 2.0 / 3.0 * (c[1] - p[1]),
                    ];
                    commands.push(SvgPathCommand::CubicTo(at(c1), at(c2), at(p)));
                    last = p;
                }
                NotePathOp::Cubic(c1, c2, p) => {
                    commands.push(SvgPathCommand::CubicTo(at(c1), at(c2), at(p)));
                    last = p;
                }
                NotePathOp::Close => commands.push(SvgPathCommand::Close),
            }
        }
        let style = match shape.kind {
            NoteShapeKind::Fill { .. } => SvgStyle {
                stroke_alpha: 1.0,
                stroke_rgb: None,
                fill_rgb: Some(colour),
                stroke_width: 0.0,
            },
            NoteShapeKind::Stroke { width_mm } => SvgStyle {
                stroke_alpha: 1.0,
                stroke_rgb: Some(colour),
                fill_rgb: None,
                stroke_width: (width_mm * xform.mm_to_unit) as f32,
            },
        };
        out.push(SvgElement::Path { commands, style });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{
        HAlign, NoteSize, Point, RichNoteLayout, TextNoteFormat, VAlign,
    };

    fn rich_note(text: &str, frame: NoteFrame) -> TextNote {
        TextNote {
            uuid: uuid::Uuid::from_u128(7),
            text: text.into(),
            position: Point::new(10.0, 10.0),
            rotation: 0.0,
            font_size: 1.27,
            font_family: None,
            justify_h: HAlign::Left,
            justify_v: VAlign::Top,
            locked: false,
            format: TextNoteFormat::Typst,
            rich: RichNoteLayout {
                frame,
                size: NoteSize::FixedWidth,
                width_mm: 40.0,
                ..RichNoteLayout::default()
            },
        }
    }

    fn unit() -> PageTransform {
        PageTransform {
            mm_to_unit: 1.0,
            translate_x: 0.0,
            translate_y: 0.0,
        }
    }

    const INK: (f32, f32, f32) = (0.1, 0.2, 0.3);
    const PAPER: (f32, f32, f32) = (1.0, 1.0, 1.0);

    #[test]
    fn a_rich_note_exports_as_ink_paths_and_no_text() {
        let note = rich_note("$V_\"out\" = 3.3 \"V\"$", NoteFrame::None);
        let elements = rich_note_elements(&note, &unit(), INK, PAPER, &SchematicDefaults::BUILT_IN)
            .expect("it compiles");
        assert!(elements.len() > 5);
        for element in &elements {
            let SvgElement::Path { commands, style } = element else {
                panic!("a rich note is paths only: {element:?}");
            };
            assert_eq!(style.fill_rgb, Some(INK));
            let SvgPathCommand::MoveTo(start) = commands[0] else {
                panic!("a path starts with a move");
            };
            assert!(start.x >= 9.5 && start.y >= 9.5, "{start:?}");
        }
    }

    #[test]
    fn a_filled_frame_is_drawn_first_in_the_paper_tone() {
        let note = rich_note("Notes", NoteFrame::Filled);
        let elements = rich_note_elements(&note, &unit(), INK, PAPER, &SchematicDefaults::BUILT_IN)
            .expect("it compiles");
        let SvgElement::Path { commands, style } = &elements[0] else {
            panic!("the frame is a path");
        };
        assert_eq!(style.fill_rgb, Some(PAPER));
        assert_eq!(style.stroke_rgb, Some(INK));
        assert_eq!(commands.len(), 5, "four corners and a close");
        let SvgPathCommand::LineTo(right) = commands[1] else {
            panic!("a line to the top-right");
        };
        assert!(
            (right.x - 50.0).abs() < 1e-4,
            "40 mm wide from x 10: {right:?}"
        );
    }

    #[test]
    fn a_note_that_does_not_compile_has_no_paths() {
        let note = rich_note("#nope(", NoteFrame::Border);
        assert!(
            rich_note_elements(&note, &unit(), INK, PAPER, &SchematicDefaults::BUILT_IN).is_none()
        );
    }
}
