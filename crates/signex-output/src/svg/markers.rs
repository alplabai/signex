//! ERC-related schematic markers: the no-connect X and the generic
//! No-ERC directive diamond. Kept apart so the two glyphs stay visibly
//! distinct in print, matching the canvas.

use super::*;
use crate::pdf::layout::PageTransform;
use signex_types::schematic::Point;

const MARKER_STROKE_WIDTH: f32 = 0.8;
/// Half extent of both glyphs, in millimetres.
const MARKER_HALF_MM: f64 = 1.0;

fn marker_style(rgb: (f32, f32, f32)) -> SvgStyle {
    SvgStyle {
        stroke_rgb: Some(rgb),
        fill_rgb: None,
        stroke_width: MARKER_STROKE_WIDTH,
    }
}

/// Two crossing strokes at `position`.
pub(super) fn push_no_connect_x(
    out: &mut Vec<SvgElement>,
    xform: &PageTransform,
    rgb: (f32, f32, f32),
    position: Point,
) {
    let arm = (MARKER_HALF_MM * xform.mm_to_unit) as f32;
    let cx = xform.x(position.x);
    let cy = xform.px_y(position.y);
    let style = marker_style(rgb);
    out.push(SvgElement::Path {
        commands: vec![
            SvgPathCommand::MoveTo(pt(cx - arm, cy - arm)),
            SvgPathCommand::LineTo(pt(cx + arm, cy + arm)),
        ],
        style,
    });
    out.push(SvgElement::Path {
        commands: vec![
            SvgPathCommand::MoveTo(pt(cx - arm, cy + arm)),
            SvgPathCommand::LineTo(pt(cx + arm, cy - arm)),
        ],
        style,
    });
}

/// Closed diamond outline at `position`.
pub(super) fn push_no_erc_diamond(
    out: &mut Vec<SvgElement>,
    xform: &PageTransform,
    rgb: (f32, f32, f32),
    position: Point,
) {
    let half = (MARKER_HALF_MM * xform.mm_to_unit) as f32;
    let cx = xform.x(position.x);
    let cy = xform.px_y(position.y);
    out.push(SvgElement::Path {
        commands: vec![
            SvgPathCommand::MoveTo(pt(cx, cy - half)),
            SvgPathCommand::LineTo(pt(cx + half, cy)),
            SvgPathCommand::LineTo(pt(cx, cy + half)),
            SvgPathCommand::LineTo(pt(cx - half, cy)),
            SvgPathCommand::Close,
        ],
        style: marker_style(rgb),
    });
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use signex_types::schematic::{NoConnect, SchematicSheet};
    use uuid::Uuid;

    use super::*;
    use crate::SheetSnapshot;
    use crate::pdf::PdfOptions;

    fn snapshot(with_markers: bool) -> SheetSnapshot {
        let marker = || NoConnect {
            uuid: Uuid::new_v4(),
            position: Point::new(30.0, 40.0),
        };
        let mut schematic = SchematicSheet {
            paper_size: "A4".to_string(),
            ..Default::default()
        };
        if with_markers {
            schematic.no_connects.push(marker());
            schematic.no_erc_directives.push(marker());
        }
        SheetSnapshot {
            path: PathBuf::from("markers.snxsch"),
            schematic,
            sheet_name: "Sheet1".to_string(),
            sheet_number: 1,
            sheet_count: 1,
            placement: Default::default(),
        }
    }

    fn render(sheet: &SheetSnapshot, no_erc: bool, no_connect: bool) -> String {
        let opts = PdfOptions {
            include_no_erc_markers: no_erc,
            include_no_connect_markers: no_connect,
            ..PdfOptions::default()
        };
        let (w_mm, h_mm) = opts.page_size.dimensions_mm(opts.orientation);
        SvgRenderContext::from_sheet(sheet, &opts, w_mm, h_mm, 72.0 / 25.4, None).svg_document()
    }

    #[test]
    fn each_toggle_controls_only_its_own_marker() {
        let bare = render(&snapshot(false), true, true);
        let marked = snapshot(true);

        assert_eq!(
            render(&marked, false, false),
            bare,
            "both off draws neither marker"
        );
        let directive_only = render(&marked, true, false);
        let no_connect_only = render(&marked, false, true);
        assert_ne!(directive_only, bare, "No-ERC toggle draws the directive");
        assert_ne!(no_connect_only, bare, "No-Connect toggle draws the X");
        assert_ne!(
            directive_only, no_connect_only,
            "the two glyphs are not the same drawing"
        );
        assert_ne!(render(&marked, true, true), directive_only);
    }
}
