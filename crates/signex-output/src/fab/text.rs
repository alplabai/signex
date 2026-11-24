//! Board and footprint text as Gerber strokes (#2368): designators and
//! comments, a footprint's text drawings, board strings and text frame
//! lines, each written in the board stroke font (`pcb_geom::text_strokes`)
//! on the image of its layer, placed as the canvas places it.

use pcb_geom::{GeomError, GraphicShape, PointNm, TextPlacement, mm_to_nm};
use signex_types::pcb::{PCB_DEFAULT_TEXT_SIZE_MM, PcbBoard};
use signex_types::schematic::HAlign;

use super::collect::{Images, board_drawing_function, graphic_function};
use super::frame::FabFrame;
use super::layers::FabLayer;
use super::marks::{Attributes, Function, Mark, Step};
use super::notes::Tally;

/// One text to write.
struct Line<'a> {
    content: &'a str,
    at: TextPlacement,
    size_mm: f64,
    justify: HAlign,
    /// The longest the line may be, mm: a text frame line is condensed to
    /// its box.
    room_mm: Option<f64>,
    /// A board string or text frame line: conductor on copper (#2883).
    on_board: bool,
}

/// Every text of `board` on a fabrication layer, as strokes on its image.
/// A text the geometry refuses is tallied as skipped.
pub(crate) fn texts(images: &mut Images, board: &PcbBoard, frame: FabFrame, tally: &mut Tally) {
    for footprint in &board.footprints {
        // A footprint that cannot be placed was tallied with its drawings.
        if pcb_geom::Placement::of_footprint(footprint).is_err() {
            continue;
        }
        for text in pcb_geom::shown_footprint_texts(footprint) {
            let Ok(text) = text else {
                tally.skipped += 1;
                continue;
            };
            let line = Line {
                content: &text.content,
                at: text.placement,
                size_mm: text.size_mm,
                justify: text.justify,
                room_mm: None,
                on_board: false,
            };
            write(images, FabLayer::of(text.layer), &line, frame, tally);
        }
        for graphic in footprint
            .graphics
            .iter()
            .filter(|g| g.graphic_type == "text")
        {
            let Some(layer) = FabLayer::of_token(&graphic.layer) else {
                continue;
            };
            match pcb_geom::footprint_graphic_placement(footprint, graphic) {
                Ok(Some(placed)) => {
                    if let GraphicShape::Text(at) = placed.shape {
                        let line = Line {
                            content: &graphic.text,
                            at,
                            size_mm: size_or_default(graphic.font_size),
                            justify: HAlign::Center,
                            room_mm: None,
                            on_board: false,
                        };
                        write(images, Some(layer), &line, frame, tally);
                    }
                }
                Ok(None) => {}
                Err(_) => tally.skipped += 1,
            }
        }
    }
    for graphic in &board.graphics {
        let Some(layer) = FabLayer::of_token(&graphic.layer) else {
            continue;
        };
        let size_mm = pcb_geom::text_frame_size_mm(graphic);
        // Corners the geometry refuses give no lines: the box was counted
        // as skipped.
        let lines = pcb_geom::text_frame_lines(graphic).unwrap_or_default();
        for frame_line in &lines {
            let line = Line {
                content: &frame_line.text,
                at: frame_line.placement,
                size_mm,
                justify: HAlign::Center,
                room_mm: Some(frame_line.width_mm),
                on_board: true,
            };
            write(images, Some(layer), &line, frame, tally);
        }
    }
    for text in &board.texts {
        let Some(layer) = FabLayer::of_token(&text.layer) else {
            continue;
        };
        match pcb_geom::board_text_placement(text) {
            Ok(at) => {
                let line = Line {
                    content: &text.text,
                    at,
                    size_mm: size_or_default(text.font_size),
                    justify: HAlign::Center,
                    room_mm: None,
                    on_board: true,
                };
                write(images, Some(layer), &line, frame, tally);
            }
            Err(_) => tally.skipped += 1,
        }
    }
}

fn size_or_default(size: f64) -> f64 {
    if size.is_finite() && size > 0.0 {
        size
    } else {
        PCB_DEFAULT_TEXT_SIZE_MM
    }
}

/// Write `line` on `layer`'s image, unless the layer has none or the text
/// is blank.
fn write(
    images: &mut Images,
    layer: Option<FabLayer>,
    line: &Line<'_>,
    frame: FabFrame,
    tally: &mut Tally,
) {
    let Some(layer) = layer else {
        return;
    };
    if line.content.trim().is_empty() {
        return;
    }
    match marks(line, layer, frame) {
        Ok(marks) => images.entry(layer).or_default().extend(marks),
        Err(_) => tally.skipped += 1,
    }
}

fn marks(line: &Line<'_>, layer: FabLayer, frame: FabFrame) -> Result<Vec<Mark>, GeomError> {
    let pen = pcb_geom::text_stroke_width_mm(line.size_mm);
    let width = mm_to_nm(pen, "text.pen")?.abs();
    let strokes = match line.room_mm {
        Some(room) => {
            pcb_geom::text_strokes_within(line.content, &line.at, line.size_mm, line.justify, room)?
        }
        None => pcb_geom::text_strokes(line.content, &line.at, line.size_mm, line.justify)?,
    };
    let function = if line.on_board {
        board_drawing_function(layer)
    } else {
        graphic_function(layer)
    };
    Ok(strokes
        .into_iter()
        .filter_map(|points| stroke(&points, width, function, frame))
        .collect())
}

/// One polyline as a stroke; a lone point is a dot, a stroke of no length.
fn stroke(points: &[PointNm], width: i64, function: Function, frame: FabFrame) -> Option<Mark> {
    let (&first, rest) = points.split_first()?;
    let start = frame.point(first);
    let steps = if rest.is_empty() {
        vec![Step::Line(start)]
    } else {
        rest.iter().map(|&p| Step::Line(frame.point(p))).collect()
    };
    Some(Mark::Stroke {
        width,
        function,
        start,
        steps,
        attributes: Attributes::default(),
    })
}
