//! ERC-related schematic markers: the no-connect X and the generic
//! No-ERC directive diamond. Kept apart so the two glyphs stay visibly
//! distinct in print. Size and stroke are the canvas constants.

use super::*;
use crate::pdf::layout::PageTransform;
use signex_types::schematic::Point;

use signex_types::schematic::{
    SCHEMATIC_NO_CONNECT_STROKE_MM, SCHEMATIC_RENDER_NO_CONNECT_HALF_LEN_MM,
};

fn marker_style(rgb: (f32, f32, f32), xform: &PageTransform) -> SvgStyle {
    SvgStyle {
        stroke_rgb: Some(rgb),
        fill_rgb: None,
        stroke_width: (SCHEMATIC_NO_CONNECT_STROKE_MM * xform.mm_to_unit) as f32,
    }
}

/// Two crossing strokes at `position`.
pub(super) fn push_no_connect_x(
    out: &mut Vec<SvgElement>,
    xform: &PageTransform,
    rgb: (f32, f32, f32),
    position: Point,
) {
    let arm = (SCHEMATIC_RENDER_NO_CONNECT_HALF_LEN_MM * xform.mm_to_unit) as f32;
    let cx = xform.x(position.x);
    let cy = xform.px_y(position.y);
    let style = marker_style(rgb, xform);
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
    let half = (SCHEMATIC_RENDER_NO_CONNECT_HALF_LEN_MM * xform.mm_to_unit) as f32;
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
        style: marker_style(rgb, xform),
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
            uuid: Uuid::now_v7(),
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

    fn first_path(out: &[SvgElement]) -> (&[SvgPathCommand], SvgStyle) {
        match &out[0] {
            SvgElement::Path { commands, style } => (commands, *style),
            other => panic!("expected a path, got {other:?}"),
        }
    }

    #[test]
    fn markers_use_the_canvas_half_length_and_stroke() {
        let xform = PageTransform {
            mm_to_unit: 2.0,
            translate_x: 0.0,
            translate_y: 0.0,
        };
        let arm = (SCHEMATIC_RENDER_NO_CONNECT_HALF_LEN_MM * 2.0) as f32;
        let mut out = Vec::new();
        push_no_connect_x(&mut out, &xform, (0.0, 0.0, 0.0), Point::new(10.0, 10.0));
        let (commands, style) = first_path(&out);
        assert_eq!(
            style.stroke_width,
            (SCHEMATIC_NO_CONNECT_STROKE_MM * 2.0) as f32
        );
        let SvgPathCommand::MoveTo(from) = &commands[0] else {
            panic!("X arm starts with a move");
        };
        assert_eq!(from.x, 20.0 - arm);

        let mut out = Vec::new();
        push_no_erc_diamond(&mut out, &xform, (0.0, 0.0, 0.0), Point::new(10.0, 10.0));
        let (commands, style) = first_path(&out);
        assert_eq!(
            style.stroke_width,
            (SCHEMATIC_NO_CONNECT_STROKE_MM * 2.0) as f32
        );
        let SvgPathCommand::MoveTo(top) = &commands[0] else {
            panic!("diamond starts with a move");
        };
        assert_eq!(top.y, 20.0 - arm);
    }

    #[test]
    fn a_junction_prints_as_a_circle() {
        let mut with = snapshot(false);
        with.schematic
            .junctions
            .push(signex_types::schematic::Junction {
                position: Point::new(30.0, 40.0),
                ..Default::default()
            });
        let cubics = |svg: &str| svg.matches("C ").count();
        let bare = render(&snapshot(false), true, true);
        assert_eq!(cubics(&render(&with, true, true)), cubics(&bare) + 4);
    }
}
