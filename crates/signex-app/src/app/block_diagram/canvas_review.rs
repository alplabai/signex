//! The Re-sync review's previews on the diagram (mockup row 210): a block
//! the Re-sync added gets a dashed outline in the added colour, a block
//! whose sheet was renamed or moved an outline in the moved colour, and a
//! block a checked row would unlink an amber hatch; each carries its tag
//! ("new", "renamed", "moved", "unlinked") in its preview's colour.
//! Unticking a row takes its hatch and tag away. The block of the row the
//! pointer is over in the side sheet is outlined in the accent.

use iced::widget::canvas::{self, Frame, LineDash, Path, Stroke};
use iced::{Color, Point, Size, alignment};
use signex_types::block_diagram::BlockDiagram;
use signex_types::schematic::Point as MmPoint;
use uuid::Uuid;

use super::canvas::{CanvasLook, Fit};
use super::state::ResyncReview;

/// What one block's preview is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preview {
    Added,
    Renamed,
    Moved,
    Unlinked,
}

/// The tag words, in the user's language (the view reads the catalog).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PreviewTags {
    pub added: String,
    pub renamed: String,
    pub moved: String,
    pub unlinked: String,
}

impl PreviewTags {
    fn word(&self, preview: Preview) -> &str {
        match preview {
            Preview::Added => &self.added,
            Preview::Renamed => &self.renamed,
            Preview::Moved => &self.moved,
            Preview::Unlinked => &self.unlinked,
        }
    }
}

impl CanvasLook {
    /// The colour of `preview`, its outline's and its tag's.
    pub fn preview_ink(&self, preview: Preview) -> Color {
        match preview {
            Preview::Added => self.added,
            Preview::Renamed | Preview::Moved => self.moved,
            Preview::Unlinked => self.unlinked,
        }
    }
}

/// The preview of each block `review` names, the unlinked ones only while
/// their row is checked.
pub fn previews(review: &ResyncReview) -> Vec<(Uuid, Preview)> {
    let added = review.added.iter().map(|b| (*b, Preview::Added));
    let moved = review.moved.iter().map(|b| {
        let kind = if review.renamed.contains(b) {
            Preview::Renamed
        } else {
            Preview::Moved
        };
        (*b, kind)
    });
    let unlinked = review
        .unlink
        .iter()
        .filter(|row| row.checked)
        .map(|row| (row.block, Preview::Unlinked));
    added.chain(moved).chain(unlinked).collect()
}

/// The tag each previewed block carries: its word and its colour (what
/// `draw_review` draws; read by the tests).
#[cfg(test)]
pub fn tags(
    review: &ResyncReview,
    words: &PreviewTags,
    look: &CanvasLook,
) -> Vec<(Uuid, String, Color)> {
    previews(review)
        .into_iter()
        .map(|(id, preview)| {
            (
                id,
                words.word(preview).to_string(),
                look.preview_ink(preview),
            )
        })
        .collect()
}

/// The box a preview outlines around `block`, screen px.
fn preview_box(
    fit: Fit,
    look: CanvasLook,
    diagram: &BlockDiagram,
    id: Uuid,
) -> Option<(Point, Size)> {
    let b = diagram.block(id)?.bounds();
    let pad = f64::from(look.slop) / fit.scale * 0.5;
    let top_left = fit.to_screen(MmPoint::new(b.x - pad, b.y - pad));
    let size = Size::new(
        ((b.width + 2.0 * pad) * fit.scale) as f32,
        ((b.height + 2.0 * pad) * fit.scale) as f32,
    );
    Some((top_left, size))
}

/// Draw the previews of `review` over the blocks of `diagram`.
pub fn draw_review(
    frame: &mut Frame,
    fit: Fit,
    diagram: &BlockDiagram,
    review: &ResyncReview,
    look: CanvasLook,
    words: &PreviewTags,
) {
    for (id, preview) in previews(review) {
        let Some((top_left, size)) = preview_box(fit, look, diagram, id) else {
            continue;
        };
        let outline = Path::rectangle(top_left, size);
        let ink = look.preview_ink(preview);
        match preview {
            Preview::Added => {
                let dash = [look.slop, look.slop * 0.6];
                let stroke = Stroke {
                    line_dash: LineDash {
                        segments: &dash,
                        offset: 0,
                    },
                    ..Stroke::default().with_color(ink).with_width(2.0)
                };
                frame.stroke(&outline, stroke);
            }
            Preview::Renamed | Preview::Moved => {
                frame.stroke(&outline, Stroke::default().with_color(ink).with_width(2.0));
            }
            Preview::Unlinked => hatch(frame, top_left, size, look),
        }
        tag(frame, top_left, size, words.word(preview), ink, look);
    }
    let hovered = review
        .hovered
        .and_then(|id| preview_box(fit, look, diagram, id));
    if let Some((top_left, size)) = hovered {
        let grow = look.slop * 0.5;
        let outer = Path::rectangle(
            Point::new(top_left.x - grow, top_left.y - grow),
            Size::new(size.width + 2.0 * grow, size.height + 2.0 * grow),
        );
        frame.stroke(
            &outer,
            Stroke::default().with_color(look.accent).with_width(2.5),
        );
    }
}

/// A tag pill at the box's top right, its word in `ink` on a faint fill of
/// the same colour.
fn tag(frame: &mut Frame, top_left: Point, size: Size, word: &str, ink: Color, look: CanvasLook) {
    if word.is_empty() {
        return;
    }
    let height = look.text_size * 1.3;
    let width = look.text_size * 0.6 * word.chars().count() as f32 + look.text_size;
    let at = Point::new(top_left.x + size.width - width, top_left.y - height * 0.5);
    let fill = Color { a: 0.18, ..ink };
    frame.fill(
        &Path::rectangle(at, Size::new(width, height)),
        Color {
            a: 1.0,
            ..look.background
        },
    );
    frame.fill(&Path::rectangle(at, Size::new(width, height)), fill);
    frame.fill_text(canvas::Text {
        content: word.to_string(),
        position: Point::new(at.x + width / 2.0, at.y + height / 2.0),
        color: ink,
        size: iced::Pixels(look.text_size * 0.85),
        align_x: iced::advanced::text::Alignment::Center,
        align_y: alignment::Vertical::Center,
        ..canvas::Text::default()
    });
}

/// Diagonal lines over the box at `top_left` of `size`, in the unlinked
/// colour, with its edge.
fn hatch(frame: &mut Frame, top_left: Point, size: Size, look: CanvasLook) {
    let ink = Color {
        a: 0.55,
        ..look.unlinked
    };
    let step = (look.slop * 1.5).max(4.0);
    let (x0, y0) = (top_left.x, top_left.y);
    let (x1, y1) = (x0 + size.width, y0 + size.height);
    let path = Path::new(|p| {
        let mut c = x0 + y0 + step;
        while c < x1 + y1 {
            // The line x + y = c inside the box.
            let start = (c - y1).max(x0);
            let end = (c - y0).min(x1);
            if start < end {
                p.move_to(Point::new(start, c - start));
                p.line_to(Point::new(end, c - end));
            }
            c += step;
        }
    });
    frame.stroke(&path, Stroke::default().with_color(ink).with_width(1.0));
    frame.stroke(
        &Path::rectangle(top_left, size),
        Stroke::default().with_color(look.unlinked).with_width(2.0),
    );
}
