//! Sheet frame lowering — border, zone markers and title block as plain
//! drawables (#1079).
//!
//! One lowering for every consumer: the schematic canvas draws these
//! primitives over its paper rectangle, the PDF exporter and the print
//! preview draw the same list over the page. Coordinates are mm with the
//! origin at the sheet's top-left corner and Y pointing down, the schematic
//! convention; each consumer applies its own mm → unit mapping.

use super::{FontStyle, Frame, TitleBlock};

/// Stroke width of the frame lines, in mm.
pub const FRAME_LINE_MM: f64 = 0.15;
/// Stroke width of the title block outline, in mm.
pub const TITLE_BLOCK_LINE_MM: f64 = 0.2;

/// How a [`FramePrimitive::Text`] sits on its anchor point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameTextAnchor {
    /// The anchor is the top-left corner of the text line.
    TopLeft,
    /// The anchor is the centre of the text line.
    Center,
}

/// One drawable of the sheet frame.
#[derive(Debug, Clone, PartialEq)]
pub enum FramePrimitive {
    Line {
        from: (f64, f64),
        to: (f64, f64),
        width_mm: f64,
    },
    Rect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        width_mm: f64,
    },
    Text {
        x: f64,
        y: f64,
        size_mm: f64,
        style: FontStyle,
        anchor: FrameTextAnchor,
        text: String,
    },
}

/// Lower a sheet's frame and title block to drawables.
///
/// - The drawing area is the sheet inset by `frame.border_margin_mm`.
/// - With zone markers on, a second border sits halfway into the margin and
///   the strip between the two carries the zone labels: 1, 2, 3… along the
///   top and bottom, A, B, C… down both sides.
/// - The title block sits in the drawing area's bottom-right corner. On a
///   sheet too small for it, it is scaled down to fit, never clipped.
/// - `resolve` turns a field's `${TOKEN}` text into what is shown; fields
///   that resolve to nothing are left out.
pub fn lower_sheet_frame(
    width_mm: f64,
    height_mm: f64,
    frame: &Frame,
    title_block: Option<&TitleBlock>,
    resolve: impl Fn(&str) -> String,
) -> Vec<FramePrimitive> {
    let mut out = Vec::new();
    let margin = frame.border_margin_mm.max(0.0);
    let inner_w = width_mm - 2.0 * margin;
    let inner_h = height_mm - 2.0 * margin;
    if inner_w <= 0.0 || inner_h <= 0.0 {
        return out;
    }

    out.push(FramePrimitive::Rect {
        x: margin,
        y: margin,
        w: inner_w,
        h: inner_h,
        width_mm: FRAME_LINE_MM,
    });

    if frame.show_zone_markers && margin > 0.0 {
        push_zones(&mut out, width_mm, height_mm, margin, frame);
    }

    if let Some(block) = title_block {
        push_title_block(&mut out, margin, inner_w, inner_h, block, &resolve);
    }

    out
}

fn push_zones(
    out: &mut Vec<FramePrimitive>,
    width_mm: f64,
    height_mm: f64,
    margin: f64,
    frame: &Frame,
) {
    let outer = margin / 2.0;
    out.push(FramePrimitive::Rect {
        x: outer,
        y: outer,
        w: width_mm - 2.0 * outer,
        h: height_mm - 2.0 * outer,
        width_mm: FRAME_LINE_MM,
    });

    let strip = margin - outer;
    let size_mm = strip * 0.5;
    let inner_w = width_mm - 2.0 * margin;
    let inner_h = height_mm - 2.0 * margin;

    let columns = usize::from(frame.horizontal_zones);
    if columns > 0 {
        let step = inner_w / columns as f64;
        for i in 0..columns {
            let x0 = margin + step * i as f64;
            if i > 0 {
                for (y0, y1) in [(outer, margin), (height_mm - margin, height_mm - outer)] {
                    out.push(FramePrimitive::Line {
                        from: (x0, y0),
                        to: (x0, y1),
                        width_mm: FRAME_LINE_MM,
                    });
                }
            }
            let label = (i + 1).to_string();
            for y in [outer + strip / 2.0, height_mm - outer - strip / 2.0] {
                out.push(zone_label(x0 + step / 2.0, y, size_mm, label.clone()));
            }
        }
    }

    let rows = usize::from(frame.vertical_zones);
    if rows > 0 {
        let step = inner_h / rows as f64;
        for i in 0..rows {
            let y0 = margin + step * i as f64;
            if i > 0 {
                for (x0, x1) in [(outer, margin), (width_mm - margin, width_mm - outer)] {
                    out.push(FramePrimitive::Line {
                        from: (x0, y0),
                        to: (x1, y0),
                        width_mm: FRAME_LINE_MM,
                    });
                }
            }
            let label = zone_letter(i);
            for x in [outer + strip / 2.0, width_mm - outer - strip / 2.0] {
                out.push(zone_label(x, y0 + step / 2.0, size_mm, label.clone()));
            }
        }
    }
}

fn zone_label(x: f64, y: f64, size_mm: f64, text: String) -> FramePrimitive {
    FramePrimitive::Text {
        x,
        y,
        size_mm,
        style: FontStyle::Normal,
        anchor: FrameTextAnchor::Center,
        text,
    }
}

/// Row letter for zone `index`: A…Z, then AA, AB… as spreadsheet columns do.
pub fn zone_letter(index: usize) -> String {
    let mut n = index + 1;
    let mut letters = Vec::new();
    while n > 0 {
        let rem = (n - 1) % 26;
        letters.push(char::from(b'A' + rem as u8));
        n = (n - 1) / 26;
    }
    letters.iter().rev().collect()
}

fn push_title_block(
    out: &mut Vec<FramePrimitive>,
    margin: f64,
    inner_w: f64,
    inner_h: f64,
    block: &TitleBlock,
    resolve: &impl Fn(&str) -> String,
) {
    if block.width_mm <= 0.0 || block.height_mm <= 0.0 {
        return;
    }
    let scale = (inner_w / block.width_mm)
        .min(inner_h / block.height_mm)
        .min(1.0);
    let w = block.width_mm * scale;
    let h = block.height_mm * scale;
    let x = margin + inner_w - w;
    let y = margin + inner_h - h;
    out.push(FramePrimitive::Rect {
        x,
        y,
        w,
        h,
        width_mm: TITLE_BLOCK_LINE_MM,
    });

    for field in &block.fields {
        let text = resolve(&field.default_text);
        if text.trim().is_empty() {
            continue;
        }
        out.push(FramePrimitive::Text {
            x: x + field.x_mm * scale,
            y: y + field.y_mm * scale,
            size_mm: field.font_size_mm * scale,
            style: field.font_style,
            anchor: FrameTextAnchor::TopLeft,
            text,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template::builtin::standard_title_block;

    fn texts(prims: &[FramePrimitive]) -> Vec<&str> {
        prims
            .iter()
            .filter_map(|p| match p {
                FramePrimitive::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn zone_letters_run_past_z() {
        assert_eq!(zone_letter(0), "A");
        assert_eq!(zone_letter(25), "Z");
        assert_eq!(zone_letter(26), "AA");
        assert_eq!(zone_letter(27), "AB");
    }

    #[test]
    fn zone_labels_follow_the_frame_counts() {
        let frame = Frame {
            border_margin_mm: 10.0,
            show_zone_markers: true,
            horizontal_zones: 4,
            vertical_zones: 3,
        };
        let prims = lower_sheet_frame(297.0, 210.0, &frame, None, |s| s.to_string());
        let labels = texts(&prims);
        // Each number twice (top and bottom), each letter twice (left, right).
        for n in ["1", "2", "3", "4"] {
            assert_eq!(labels.iter().filter(|t| **t == n).count(), 2, "{n}");
        }
        for l in ["A", "B", "C"] {
            assert_eq!(labels.iter().filter(|t| **t == l).count(), 2, "{l}");
        }
        assert!(!labels.contains(&"5") && !labels.contains(&"D"));
        // Inner border, outer border, then 3 + 2 dividers on each of two sides.
        let rects = prims
            .iter()
            .filter(|p| matches!(p, FramePrimitive::Rect { .. }))
            .count();
        let lines = prims
            .iter()
            .filter(|p| matches!(p, FramePrimitive::Line { .. }))
            .count();
        assert_eq!(rects, 2);
        assert_eq!(lines, 2 * 3 + 2 * 2);
    }

    #[test]
    fn zone_one_is_left_and_zone_a_is_top() {
        let prims = lower_sheet_frame(297.0, 210.0, &Frame::default(), None, |s| s.to_string());
        let at = |label: &str| {
            prims.iter().find_map(|p| match p {
                FramePrimitive::Text { x, y, text, .. } if text == label => Some((*x, *y)),
                _ => None,
            })
        };
        let (x1, _) = at("1").unwrap();
        let (x8, _) = at("8").unwrap();
        assert!(x1 < x8);
        let (_, ya) = at("A").unwrap();
        let (_, yf) = at("F").unwrap();
        assert!(ya < yf);
        // Labels sit in the strip between the two borders.
        assert!((7.5 - at("A").unwrap().0).abs() < 1e-9);
    }

    #[test]
    fn without_zone_markers_only_the_drawing_border_is_drawn() {
        let frame = Frame {
            show_zone_markers: false,
            ..Frame::default()
        };
        let prims = lower_sheet_frame(297.0, 210.0, &frame, None, |s| s.to_string());
        assert_eq!(
            prims,
            vec![FramePrimitive::Rect {
                x: 10.0,
                y: 10.0,
                w: 277.0,
                h: 190.0,
                width_mm: FRAME_LINE_MM,
            }]
        );
    }

    #[test]
    fn the_title_block_sits_in_the_bottom_right_of_the_drawing_area() {
        let block = standard_title_block();
        let prims = lower_sheet_frame(297.0, 210.0, &Frame::default(), Some(&block), |s| {
            s.replace("${TITLE}", "Power supply")
        });
        let rect = prims
            .iter()
            .find_map(|p| match p {
                FramePrimitive::Rect {
                    x,
                    y,
                    w,
                    h,
                    width_mm,
                } if *width_mm == TITLE_BLOCK_LINE_MM => Some((*x, *y, *w, *h)),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            rect,
            (297.0 - 10.0 - 180.0, 210.0 - 10.0 - 40.0, 180.0, 40.0)
        );
        let title = prims
            .iter()
            .find_map(|p| match p {
                FramePrimitive::Text { x, y, text, .. } if text == "Power supply" => Some((*x, *y)),
                _ => None,
            })
            .unwrap();
        assert_eq!(title, (rect.0 + 4.0, rect.1 + 4.0));
    }

    #[test]
    fn a_title_block_wider_than_the_sheet_is_scaled_to_fit() {
        let block = standard_title_block();
        // A5 portrait: the drawing area is 128 mm wide, the block 180 mm.
        let prims = lower_sheet_frame(148.0, 210.0, &Frame::default(), Some(&block), |s| {
            s.to_string()
        });
        let (x, w) = prims
            .iter()
            .find_map(|p| match p {
                FramePrimitive::Rect { x, w, width_mm, .. } if *width_mm == TITLE_BLOCK_LINE_MM => {
                    Some((*x, *w))
                }
                _ => None,
            })
            .unwrap();
        assert!((x - 10.0).abs() < 1e-9);
        assert!((w - 128.0).abs() < 1e-9);
    }

    #[test]
    fn fields_that_resolve_to_nothing_are_left_out() {
        let block = standard_title_block();
        let prims = lower_sheet_frame(297.0, 210.0, &Frame::default(), Some(&block), |s| {
            if s == "${TITLE}" {
                String::new()
            } else {
                s.to_string()
            }
        });
        let labels = texts(&prims);
        assert!(!labels.contains(&""));
        assert!(labels.contains(&"Rev: ${REV}"));
    }

    #[test]
    fn a_sheet_smaller_than_its_margins_draws_nothing() {
        let prims = lower_sheet_frame(15.0, 15.0, &Frame::default(), None, |s| s.to_string());
        assert!(prims.is_empty());
    }
}
