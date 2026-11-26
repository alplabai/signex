//! The Re-sync review's previews on the diagram (mockup row 210): a block
//! the Re-sync added gets a dashed outline in the added colour, a block
//! whose sheet was renamed or moved an outline in the moved colour, and a
//! block a checked row would unlink an amber hatch. Unticking a row takes
//! its hatch away.

use iced::widget::canvas::{Frame, LineDash, Path, Stroke};
use iced::{Color, Point, Size};
use signex_types::block_diagram::BlockDiagram;
use signex_types::schematic::Point as MmPoint;
use uuid::Uuid;

use super::canvas::{CanvasLook, Fit};
use super::state::ResyncReview;

/// What one block's preview is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preview {
    Added,
    Moved,
    Unlinked,
}

/// The preview of each block `review` names, the unlinked ones only while
/// their row is checked.
pub fn previews(review: &ResyncReview) -> Vec<(Uuid, Preview)> {
    let added = review.added.iter().map(|b| (*b, Preview::Added));
    let moved = review.moved.iter().map(|b| (*b, Preview::Moved));
    let unlinked = review
        .unlink
        .iter()
        .filter(|row| row.checked)
        .map(|row| (row.block, Preview::Unlinked));
    added.chain(moved).chain(unlinked).collect()
}

/// Draw the previews of `review` over the blocks of `diagram`.
pub fn draw_review(
    frame: &mut Frame,
    fit: Fit,
    diagram: &BlockDiagram,
    review: &ResyncReview,
    look: CanvasLook,
) {
    for (id, preview) in previews(review) {
        let Some(block) = diagram.block(id) else {
            continue;
        };
        let b = block.bounds();
        let pad = f64::from(look.slop) / fit.scale * 0.5;
        let top_left = fit.to_screen(MmPoint::new(b.x - pad, b.y - pad));
        let size = Size::new(
            ((b.width + 2.0 * pad) * fit.scale) as f32,
            ((b.height + 2.0 * pad) * fit.scale) as f32,
        );
        let outline = Path::rectangle(top_left, size);
        match preview {
            Preview::Added => {
                let dash = [look.slop, look.slop * 0.6];
                let stroke = Stroke {
                    line_dash: LineDash {
                        segments: &dash,
                        offset: 0,
                    },
                    ..Stroke::default().with_color(look.added).with_width(2.0)
                };
                frame.stroke(&outline, stroke);
            }
            Preview::Moved => {
                frame.stroke(
                    &outline,
                    Stroke::default().with_color(look.moved).with_width(2.0),
                );
            }
            Preview::Unlinked => hatch(frame, top_left, size, look),
        }
    }
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
