//! Sheet frame lowering — border, zone markers and title block as plain
//! drawables (#1079).
//!
//! One lowering for every consumer: the schematic canvas draws these
//! primitives over its paper rectangle, the PDF exporter and the print
//! preview draw the same list over the page. Coordinates are mm with the
//! origin at the sheet's top-left corner and Y pointing down, the schematic
//! convention; each consumer applies its own mm → unit mapping.

use super::{FontStyle, Frame, TitleBlock, TitleBlockCell};

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
/// - The drawing area is the sheet inset by `frame.border_margin_mm`. With
///   `frame.show_border` off, no border or zones are drawn around it.
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

    if frame.show_border {
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

    for cell in &block.cells {
        push_cell(out, x, y, scale, cell, resolve);
    }

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

/// Caption text height in a title-block cell, in mm (before scaling).
pub const CELL_CAPTION_MM: f64 = 1.6;
/// Inset of a cell's caption and value from its top-left corner, in mm.
const CELL_PAD_MM: f64 = 0.7;
/// Gap between the caption and the value, in mm.
const CELL_GAP_MM: f64 = 0.3;
/// Stroke width of the cell borders inside a title block, in mm: thinner
/// than the block outline, as a table's inner rules are.
pub const CELL_LINE_MM: f64 = 0.12;
/// Advance of one character per mm of text height, generous enough for
/// Roboto's widest common glyphs: a value is shrunk until its estimate fits.
const CHAR_ADVANCE_PER_MM: f64 = 0.62;
/// The smallest a value is shrunk to; a longer value runs past its cell.
const MIN_VALUE_MM: f64 = 1.2;

/// One title-block cell: its border, its caption, and its value under the
/// caption, shrunk to fit the cell's width. A value that resolves to nothing
/// leaves the cell empty but drawn.
fn push_cell(
    out: &mut Vec<FramePrimitive>,
    block_x: f64,
    block_y: f64,
    scale: f64,
    cell: &TitleBlockCell,
    resolve: &impl Fn(&str) -> String,
) {
    let (x, y) = (block_x + cell.x_mm * scale, block_y + cell.y_mm * scale);
    out.push(FramePrimitive::Rect {
        x,
        y,
        w: cell.w_mm * scale,
        h: cell.h_mm * scale,
        width_mm: CELL_LINE_MM,
    });
    let pad = CELL_PAD_MM * scale;
    if !cell.caption.is_empty() {
        out.push(FramePrimitive::Text {
            x: x + pad,
            y: y + pad,
            size_mm: CELL_CAPTION_MM * scale,
            style: FontStyle::Normal,
            anchor: FrameTextAnchor::TopLeft,
            text: cell.caption.clone(),
        });
    }
    let value = resolve(&cell.value);
    if value.trim().is_empty() {
        return;
    }
    let room = cell.w_mm - 2.0 * CELL_PAD_MM;
    let chars = value.chars().count().max(1) as f64;
    let fitted = (room / (chars * CHAR_ADVANCE_PER_MM))
        .min(cell.value_size_mm)
        .max(MIN_VALUE_MM);
    out.push(FramePrimitive::Text {
        x: x + pad,
        y: y + pad + (CELL_CAPTION_MM + CELL_GAP_MM) * scale,
        size_mm: fitted * scale,
        style: cell.value_style,
        anchor: FrameTextAnchor::TopLeft,
        text: value,
    });
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
            show_border: true,
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
            (297.0 - 10.0 - 110.0, 210.0 - 10.0 - 28.0, 110.0, 28.0)
        );
        let title = prims
            .iter()
            .find_map(|p| match p {
                FramePrimitive::Text { x, y, text, .. } if text == "Power supply" => Some((*x, *y)),
                _ => None,
            })
            .unwrap();
        // The title's value sits under the "Title" caption of the first cell.
        let (dx, dy) = (title.0 - rect.0, title.1 - rect.1);
        assert!((dx - 0.7).abs() < 1e-9, "{dx}");
        assert!((dy - (0.7 + CELL_CAPTION_MM + 0.3)).abs() < 1e-9, "{dy}");
    }

    #[test]
    fn a_title_block_wider_than_the_sheet_is_scaled_to_fit() {
        let block = standard_title_block();
        // A 100 mm wide sheet: the drawing area is 80 mm wide, the block 110.
        let prims = lower_sheet_frame(100.0, 210.0, &Frame::default(), Some(&block), |s| {
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
        assert!((w - 80.0).abs() < 1e-9);
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
        assert!(labels.contains(&"${REV}"));
        // The empty title's cell keeps its border and caption.
        assert!(labels.contains(&"Title"));
    }

    /// #1201 — the standard block is a table: one bordered cell per value,
    /// each with its caption, all inside the block's outline.
    #[test]
    fn the_standard_block_is_a_table_of_labelled_cells() {
        let block = standard_title_block();
        let prims = lower_sheet_frame(297.0, 210.0, &Frame::default(), Some(&block), |_| {
            String::new()
        });
        let outline = prims
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
        let cells: Vec<_> = prims
            .iter()
            .filter_map(|p| match p {
                FramePrimitive::Rect {
                    x,
                    y,
                    w,
                    h,
                    width_mm,
                } if *width_mm == CELL_LINE_MM => Some((*x, *y, *w, *h)),
                _ => None,
            })
            .collect();
        assert_eq!(cells.len(), block.cells.len());
        let eps = 1e-9;
        for (x, y, w, h) in &cells {
            assert!(*x >= outline.0 - eps && *y >= outline.1 - eps);
            assert!(x + w <= outline.0 + outline.2 + eps);
            assert!(y + h <= outline.1 + outline.3 + eps);
        }
        // The cells tile the block: their areas add up to its area.
        let area: f64 = cells.iter().map(|(_, _, w, h)| w * h).sum();
        assert!((area - outline.2 * outline.3).abs() < 1e-6, "{area}");
        let labels = texts(&prims);
        for caption in [
            "Title", "Doc No", "Rev", "Sheet", "Date", "Company", "File", "Drawn", "Checked",
            "Approved",
        ] {
            assert!(labels.contains(&caption), "{caption}");
        }
        // About a third of an A4 landscape sheet, not 60 %.
        assert!(outline.2 <= 297.0 * 0.4);
    }

    #[test]
    fn a_long_value_is_shrunk_to_fit_its_cell() {
        let block = standard_title_block();
        let long = "a-rather-long-schematic-file-name-for-a-small-cell.snxsch";
        let prims = lower_sheet_frame(297.0, 210.0, &Frame::default(), Some(&block), |s| {
            s.replace("${FILENAME}", long)
        });
        let size = prims
            .iter()
            .find_map(|p| match p {
                FramePrimitive::Text { size_mm, text, .. } if text == long => Some(*size_mm),
                _ => None,
            })
            .unwrap();
        assert!(size < 2.5, "{size}");
    }

    #[test]
    fn without_the_border_only_the_title_block_is_drawn() {
        let block = standard_title_block();
        let frame = Frame {
            show_border: false,
            ..Frame::default()
        };
        let prims = lower_sheet_frame(297.0, 210.0, &frame, Some(&block), |s| s.to_string());
        // The frame is pushed before the title block: dropping it leaves
        // exactly the title block's drawables.
        let with_border = lower_sheet_frame(297.0, 210.0, &Frame::default(), Some(&block), |s| {
            s.to_string()
        });
        let border_only =
            lower_sheet_frame(297.0, 210.0, &Frame::default(), None, |s| s.to_string());
        assert!(!prims.is_empty());
        assert_eq!(prims, with_border[border_only.len()..]);
    }

    #[test]
    fn sheet_settings_set_the_border_and_the_zone_counts() {
        use signex_types::schematic::{MAX_ZONES, SheetFrameSettings};
        let frame = Frame::default().with_sheet_settings(&SheetFrameSettings {
            show_border: false,
            show_title_block: true,
            horizontal_zones: 200,
            vertical_zones: 3,
        });
        assert!(!frame.show_border);
        assert_eq!(frame.horizontal_zones, MAX_ZONES);
        assert_eq!(frame.vertical_zones, 3);
        assert_eq!(frame.border_margin_mm, Frame::default().border_margin_mm);

        let no_zones = Frame::default().with_sheet_settings(&SheetFrameSettings {
            horizontal_zones: 0,
            vertical_zones: 0,
            ..SheetFrameSettings::default()
        });
        assert!(!no_zones.show_zone_markers, "no zones, no zone strip");
        assert_eq!(
            Frame::default().with_sheet_settings(&SheetFrameSettings::default()),
            Frame::default()
        );
    }

    #[test]
    fn a_sheet_smaller_than_its_margins_draws_nothing() {
        let prims = lower_sheet_frame(15.0, 15.0, &Frame::default(), None, |s| s.to_string());
        assert!(prims.is_empty());
    }
}
