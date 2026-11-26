//! Rich text notes on the canvas (#1114, mockup row 203): the last good
//! compile of each note, drawn as vector polygons and lines in the content
//! scene (so the CPU replay and the GPU surface draw the same thing), with
//! the note's Frame section, and a small corner mark when the newest compile
//! failed or a Fixed box clips its text. The compile itself runs in a task
//! (`app::rich_note`); this module only reads what landed.

use std::collections::HashMap;
use std::sync::Arc;

use signex_gfx::primitive::line::LineSegment;
use signex_gfx::primitive::ordered::OrderedItem;
use signex_gfx::primitive::polygon::GpuPolygon;
use signex_notebook::note::{NoteDrawing, NoteKey, NotePaint, NoteShapeKind, PlacedNote};
use signex_types::schematic::{Aabb, NoteFrame, SchematicDefaults, TextNote, TextNoteFormat};

/// What the canvas shows for one rich note.
#[derive(Debug, Clone)]
pub struct ShownNote {
    /// The last compile that gave a drawing; `None` before the first one.
    pub drawing: Option<Arc<NoteDrawing>>,
    /// The key of the newest compile that landed for the note.
    pub key: NoteKey,
    /// The newest compile failed: the last good drawing stays, marked.
    pub failed: bool,
}

impl PartialEq for ShownNote {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
            && self.failed == other.failed
            && match (&self.drawing, &other.drawing) {
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            }
    }
}

/// The rich notes the canvas draws, by note uuid. `revision` moves with
/// every change, so a built scene or hit index knows it is stale.
#[derive(Debug, Clone, Default)]
pub struct RichNoteRenders {
    shown: HashMap<uuid::Uuid, ShownNote>,
    revision: u64,
}

impl RichNoteRenders {
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn get(&self, note: uuid::Uuid) -> Option<&ShownNote> {
        self.shown.get(&note)
    }

    /// Show `shown` for `note`; `false` when it was shown already.
    pub fn set(&mut self, note: uuid::Uuid, shown: ShownNote) -> bool {
        if self.shown.get(&note) == Some(&shown) {
            return false;
        }
        self.shown.insert(note, shown);
        self.revision += 1;
        true
    }

    /// Forget the notes `keep` says no to (gone from every open sheet).
    pub fn retain(&mut self, keep: impl Fn(uuid::Uuid) -> bool) {
        let before = self.shown.len();
        self.shown.retain(|id, _| keep(*id));
        if self.shown.len() != before {
            self.revision += 1;
        }
    }

    /// The drawing `note` is drawn with, when it is a rich note with one.
    #[must_use]
    pub fn drawing_of(&self, note: &TextNote) -> Option<&NoteDrawing> {
        if note.format != TextNoteFormat::Typst {
            return None;
        }
        self.get(note.uuid)?.drawing.as_deref()
    }
}

/// The inks a rich note is drawn in.
#[derive(Debug, Clone, Copy)]
pub(super) struct NoteInks {
    pub ink: [f32; 4],
    pub paper: [f32; 4],
    pub mark: [f32; 4],
}

/// The note's box on the sheet, when it is a rich note with a drawing: what
/// the hit test, the selection box and the culling read.
#[must_use]
pub fn rich_note_aabb(note: &TextNote, renders: Option<&RichNoteRenders>) -> Option<Aabb> {
    let drawing = renders?.drawing_of(note)?;
    let [x0, y0, x1, y1] = PlacedNote::of(note, Some(drawing)).bounds();
    Some(Aabb::new(x0, y0, x1, y1))
}

/// Lower the rich note `note` into `out`: its frame, its shapes and the
/// corner mark. `false` when it has no drawing yet, so the caller draws its
/// source as a plain note's text.
pub(super) fn push_rich_note(
    note: &TextNote,
    shown: Option<&ShownNote>,
    inks: NoteInks,
    defaults: &SchematicDefaults,
    out: &mut Vec<OrderedItem>,
) -> bool {
    let Some((drawing, failed)) = shown.and_then(|s| Some((s.drawing.as_deref()?, s.failed)))
    else {
        return false;
    };
    let placed = PlacedNote::of(note, Some(drawing));
    if note.rich.frame != NoteFrame::None {
        let border = note
            .rich
            .border_mm
            .unwrap_or_else(|| defaults.shape_stroke_mm(0.0));
        out.push(OrderedItem::Polygon(GpuPolygon {
            vertices: placed.corners().map(to_f32).to_vec(),
            holes: Vec::new(),
            fill_color: if note.rich.frame == NoteFrame::Filled {
                inks.paper
            } else {
                [0.0; 4]
            },
            stroke_color: Some(inks.ink),
            stroke_width: border as f32,
        }));
    }
    for shape in &drawing.shapes {
        if !shape.bounds().is_some_and(|b| placed.keeps(b)) {
            continue;
        }
        let color = match shape.paint {
            NotePaint::Ink => inks.ink,
            NotePaint::Rgba(rgba) => rgba.map(|c| f32::from(c) / 255.0),
        };
        let contours: Vec<Vec<[f32; 2]>> = shape
            .contours()
            .into_iter()
            .map(|c| {
                c.into_iter()
                    .map(|p| to_f32(placed.text_to_sheet(p)))
                    .collect()
            })
            .collect();
        match shape.kind {
            NoteShapeKind::Fill { .. } => {
                let mut contours = contours.into_iter().filter(|c| c.len() >= 3);
                let Some(vertices) = contours.next() else {
                    continue;
                };
                // Even-odd over every contour: a glyph's counters are holes.
                out.push(OrderedItem::Polygon(GpuPolygon {
                    vertices,
                    holes: contours.collect(),
                    fill_color: color,
                    stroke_color: None,
                    stroke_width: 0.0,
                }));
            }
            NoteShapeKind::Stroke { width_mm } => {
                for contour in contours {
                    for pair in contour.windows(2) {
                        out.push(OrderedItem::Line(LineSegment {
                            p0: pair[0],
                            p1: pair[1],
                            width: width_mm as f32,
                            color,
                            style: 0,
                            _pad: 0,
                        }));
                    }
                }
            }
        }
    }
    if failed || placed.clipped {
        out.push(corner_mark(&placed, inks.mark));
    }
    true
}

/// The corner mark's leg, in mm.
const MARK_MM: f64 = 1.27;

/// A small triangle in the box's top-right corner.
fn corner_mark(placed: &PlacedNote, color: [f32; 4]) -> OrderedItem {
    let (w, _) = placed.box_mm;
    let leg = MARK_MM.min(w);
    let vertices = [[w - leg, 0.0], [w, 0.0], [w, leg]]
        .map(|p| to_f32(placed.to_sheet(p)))
        .to_vec();
    OrderedItem::Polygon(GpuPolygon {
        vertices,
        holes: Vec::new(),
        fill_color: color,
        stroke_color: None,
        stroke_width: 0.0,
    })
}

fn to_f32(p: [f64; 2]) -> [f32; 2] {
    [p[0] as f32, p[1] as f32]
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_notebook::note::{NoteInput, compile_note, note_key};
    use signex_types::schematic::{HAlign, Point, RichNoteLayout, VAlign};

    fn note(frame: NoteFrame) -> TextNote {
        TextNote {
            uuid: uuid::Uuid::from_u128(3),
            text: "*Keep the hot loop small.*".into(),
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
                ..RichNoteLayout::default()
            },
        }
    }

    const INKS: NoteInks = NoteInks {
        ink: [1.0, 1.0, 1.0, 1.0],
        paper: [0.0, 0.0, 0.0, 1.0],
        mark: [1.0, 0.0, 0.0, 1.0],
    };

    fn shown(note: &TextNote, failed: bool) -> ShownNote {
        let input = NoteInput::of(note);
        ShownNote {
            drawing: compile_note(&input).drawing.map(Arc::new),
            key: note_key(&input),
            failed,
        }
    }

    #[test]
    fn a_rich_note_lowers_to_ink_polygons_inside_its_box() {
        let note = note(NoteFrame::Border);
        let shown = shown(&note, false);
        let mut out = Vec::new();
        assert!(push_rich_note(
            &note,
            Some(&shown),
            INKS,
            &SchematicDefaults::BUILT_IN,
            &mut out
        ));
        let OrderedItem::Polygon(frame) = &out[0] else {
            panic!("the frame is first");
        };
        assert_eq!(frame.stroke_color, Some(INKS.ink));
        assert_eq!(frame.fill_color, [0.0; 4], "a Border frame is not filled");
        let glyphs = out[1..]
            .iter()
            .filter(|item| matches!(item, OrderedItem::Polygon(p) if p.fill_color == INKS.ink))
            .count();
        assert!(glyphs > 10, "every glyph is a polygon: {glyphs}");
        let aabb = rich_note_aabb(&note, None);
        assert!(aabb.is_none(), "no renders, no rich box");
    }

    #[test]
    fn a_failed_compile_keeps_the_drawing_and_adds_the_mark() {
        let note = note(NoteFrame::None);
        let good = shown(&note, false);
        let marked = ShownNote {
            failed: true,
            ..good.clone()
        };
        let (mut plain, mut with_mark) = (Vec::new(), Vec::new());
        assert!(push_rich_note(
            &note,
            Some(&good),
            INKS,
            &SchematicDefaults::BUILT_IN,
            &mut plain
        ));
        assert!(push_rich_note(
            &note,
            Some(&marked),
            INKS,
            &SchematicDefaults::BUILT_IN,
            &mut with_mark
        ));
        assert_eq!(with_mark.len(), plain.len() + 1);
        let Some(OrderedItem::Polygon(mark)) = with_mark.last() else {
            panic!("the mark is last");
        };
        assert_eq!(mark.fill_color, INKS.mark);
    }

    #[test]
    fn a_note_without_a_drawing_falls_back_to_its_text() {
        let note = note(NoteFrame::None);
        let mut out = Vec::new();
        assert!(!push_rich_note(
            &note,
            None,
            INKS,
            &SchematicDefaults::BUILT_IN,
            &mut out
        ));
        assert!(out.is_empty());
    }

    #[test]
    fn the_renders_move_their_revision_only_on_a_change() {
        let note = note(NoteFrame::None);
        let mut renders = RichNoteRenders::default();
        let shown = shown(&note, false);
        assert!(renders.set(note.uuid, shown.clone()));
        let revision = renders.revision();
        assert!(!renders.set(note.uuid, shown), "the same drawing again");
        assert_eq!(renders.revision(), revision);
        let aabb = rich_note_aabb(&note, Some(&renders)).expect("a box");
        assert!(
            (aabb.min_x - 10.0).abs() < 1e-9 && aabb.max_x > 15.0,
            "{aabb:?}"
        );
        renders.retain(|_| false);
        assert!(renders.revision() > revision);
        assert!(renders.get(note.uuid).is_none());
    }
}
