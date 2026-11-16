//! The lines of a board text frame (#2424): its text wrapped to the width of
//! its box, one line under the other from the top, each centred across the
//! box. Words wrap at spaces, a word longer than a line is cut, `\n` breaks
//! a line, and lines past the bottom of the box are not drawn. Rules:
//! `docs/RENDERING_RULES.md`, "pcb-text-frame".

use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{BoardGraphic, PCB_DEFAULT_TEXT_SIZE_MM, Point};
use signex_types::pcb_graphic::BoardGraphicKind;
use signex_types::pcb_layer::BoardSide;

use crate::text::TextPlacement;
use crate::units::{GeomError, PointNm};

/// A glyph's advance, as a share of the text height. The board font is a
/// monospace narrower than this, so a wrapped line always fits its box.
pub const TEXT_FRAME_ADVANCE_EM: f64 = 0.6;
/// From one line's centre to the next, as a share of the text height.
pub const TEXT_FRAME_LINE_PITCH_EM: f64 = 1.4;
/// The gap between the box and its text, as a share of the text height.
pub const TEXT_FRAME_MARGIN_EM: f64 = 0.25;

/// One line of a text frame, placed on the board.
#[derive(Debug, Clone, PartialEq)]
pub struct FrameLine {
    pub text: String,
    pub placement: TextPlacement,
}

/// The text height of a text frame in mm: its own, else the PCB default.
pub fn text_frame_size_mm(graphic: &BoardGraphic) -> f64 {
    if graphic.font_size.is_finite() && graphic.font_size > 0.0 {
        graphic.font_size
    } else {
        PCB_DEFAULT_TEXT_SIZE_MM
    }
}

/// The lines `graphic` draws: none for a drawing that is not a text frame or
/// lacks its corners. Refuses a corner the geometry cannot hold.
pub fn text_frame_lines(graphic: &BoardGraphic) -> Result<Vec<FrameLine>, GeomError> {
    if graphic.kind() != Some(BoardGraphicKind::TextFrame) {
        return Ok(Vec::new());
    }
    let (Some(a), Some(b)) = (graphic.start, graphic.end) else {
        return Ok(Vec::new());
    };
    PointNm::from_mm(a, "text_frame.start")?;
    PointNm::from_mm(b, "text_frame.end")?;
    let size = text_frame_size_mm(graphic);
    let margin = size * TEXT_FRAME_MARGIN_EM;
    let pitch = size * TEXT_FRAME_LINE_PITCH_EM;
    let (left, right) = (a.x.min(b.x), a.x.max(b.x));
    let (top, bottom) = (a.y.min(b.y), a.y.max(b.y));
    let columns = ((right - left - 2.0 * margin) / (size * TEXT_FRAME_ADVANCE_EM)).floor();
    let rows = ((bottom - top - 2.0 * margin) / pitch).floor();
    // At least one line shows, however small the box.
    let columns = if columns >= 1.0 { columns as usize } else { 1 };
    let rows = if rows >= 1.0 { rows as usize } else { 1 };
    let side = parse_pcb_layer(&graphic.layer).and_then(|layer| layer.side());
    let mirrored = matches!(side, Some(BoardSide::Bottom));
    let centre_x = (left + right) / 2.0;
    let mut lines = Vec::new();
    for (row, text) in wrap(&graphic.text, columns)
        .into_iter()
        .take(rows)
        .enumerate()
    {
        if text.is_empty() {
            continue;
        }
        let y = top + margin + pitch * (row as f64 + 0.5);
        lines.push(FrameLine {
            text,
            placement: TextPlacement {
                anchor: PointNm::from_mm(Point::new(centre_x, y), "text_frame.line")?,
                rotation_deg: 0.0,
                mirrored,
            },
        });
    }
    Ok(lines)
}

/// `text` broken into lines of at most `columns` characters: at each `\n`,
/// then between words; a word longer than a line is cut. An empty line
/// keeps its place.
pub fn wrap(text: &str, columns: usize) -> Vec<String> {
    let columns = columns.max(1);
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        let mut len = 0;
        for word in paragraph.split_whitespace() {
            let mut word: Vec<char> = word.chars().collect();
            while word.len() > columns {
                if len > 0 {
                    lines.push(std::mem::take(&mut line));
                    len = 0;
                }
                let rest = word.split_off(columns);
                lines.push(word.into_iter().collect());
                word = rest;
            }
            if word.is_empty() {
                continue;
            }
            if len > 0 && len + 1 + word.len() > columns {
                lines.push(std::mem::take(&mut line));
                len = 0;
            }
            if len > 0 {
                line.push(' ');
                len += 1;
            }
            len += word.len();
            line.extend(word);
        }
        lines.push(line);
    }
    lines
}

#[cfg(test)]
#[path = "text_frame_tests.rs"]
mod tests;
