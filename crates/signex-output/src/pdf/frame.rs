//! The sheet frame and title block on an exported page (#1079).
//!
//! Lowered by [`crate::template::lower_sheet_frame`], the same lowering the
//! canvas draws, then turned into page elements here so the PDF content
//! stream and the print-preview raster draw it alike.

use super::*;
use crate::svg::{SvgPoint, SvgStyle};
use crate::template::{FramePrimitive, FrameTextAnchor};

/// Frame and title-block elements for one page, in page units (top-left
/// origin, Y down). Empty unless the options ask for a title block from a
/// template that exists.
pub(crate) fn sheet_frame_elements(
    sheet: &crate::SheetSnapshot,
    opts: &PdfOptions,
    ctx: &ExportContext,
    page_w_mm: f64,
    page_h_mm: f64,
    units_per_mm: f64,
) -> Vec<SvgElement> {
    if !opts.include_title_block {
        return Vec::new();
    }
    let Some(template) = opts
        .sheet_template
        .as_ref()
        .and_then(crate::template::load_builtin)
    else {
        return Vec::new();
    };

    let sub_ctx = SubstitutionContext {
        metadata: &ctx.metadata,
        filename: sheet
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        sheet_name: sheet.sheet_name.clone(),
        sheet_number: sheet.sheet_number,
        sheet_count: sheet.sheet_count,
        signex_version: env!("CARGO_PKG_VERSION"),
        variant: opts.variant.clone(),
        physical_structure: opts.use_physical_structure,
        physical_sheet_number: opts.physical_sheet_number,
        physical_document_number: opts.physical_document_number,
    };
    let primitives = crate::template::lower_sheet_frame(
        page_w_mm,
        page_h_mm,
        &template.frame,
        Some(&template.title_block),
        |text| crate::resolve(text, &sub_ctx),
    );
    frame_elements(&primitives, units_per_mm, opts.palette.sheet_border)
}

/// Turn frame primitives (mm) into page elements (`units_per_mm` units).
pub(crate) fn frame_elements(
    primitives: &[FramePrimitive],
    units_per_mm: f64,
    rgb: (f32, f32, f32),
) -> Vec<SvgElement> {
    let u = |mm: f64| (mm * units_per_mm) as f32;
    let at = |x: f64, y: f64| SvgPoint { x: u(x), y: u(y) };
    let stroke = |width_mm: f64| SvgStyle {
        stroke_rgb: Some(rgb),
        fill_rgb: None,
        stroke_width: u(width_mm),
    };
    primitives
        .iter()
        .map(|primitive| match primitive {
            FramePrimitive::Line { from, to, width_mm } => SvgElement::Path {
                commands: vec![
                    SvgPathCommand::MoveTo(at(from.0, from.1)),
                    SvgPathCommand::LineTo(at(to.0, to.1)),
                ],
                style: stroke(*width_mm),
            },
            FramePrimitive::Rect {
                x,
                y,
                w,
                h,
                width_mm,
            } => SvgElement::Path {
                commands: vec![
                    SvgPathCommand::MoveTo(at(*x, *y)),
                    SvgPathCommand::LineTo(at(x + w, *y)),
                    SvgPathCommand::LineTo(at(x + w, y + h)),
                    SvgPathCommand::LineTo(at(*x, y + h)),
                    SvgPathCommand::Close,
                ],
                style: stroke(*width_mm),
            },
            FramePrimitive::Text {
                x,
                y,
                size_mm,
                style,
                anchor,
                text,
            } => {
                let (align, v_align) = match anchor {
                    FrameTextAnchor::TopLeft => (SvgTextAlign::Left, SvgTextVAlign::Top),
                    FrameTextAnchor::Center => (SvgTextAlign::Center, SvgTextVAlign::Center),
                };
                SvgElement::Text {
                    x: u(*x),
                    y: u(*y),
                    font_alias: PdfFont::for_style(*style).alias(),
                    size_pt: u(*size_mm),
                    align,
                    v_align,
                    rotation_deg: 0.0,
                    fill_rgb: rgb,
                    text: text.clone(),
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::template::{Frame, standard_title_block};
    use crate::{ProjectMetadata, SheetSnapshot};

    fn sheet() -> SheetSnapshot {
        SheetSnapshot {
            path: PathBuf::from("/p/power.snxsch"),
            schematic: signex_types::schematic::SchematicSheet::default(),
            sheet_name: "Power".into(),
            sheet_number: 2,
            sheet_count: 3,
        }
    }

    fn ctx(sheet: SheetSnapshot) -> ExportContext {
        ExportContext {
            metadata: ProjectMetadata {
                title: "Power supply".into(),
                ..ProjectMetadata::default()
            },
            sheets: vec![sheet],
            netlist: None,
            netlist_sheets: Vec::new(),
            netlist_sheet_placements: Vec::new(),
            current_sheet: None,
        }
    }

    fn templated() -> PdfOptions {
        PdfOptions {
            sheet_template: Some(crate::TemplateId::from("iso_a4_landscape")),
            include_title_block: true,
            ..PdfOptions::default()
        }
    }

    #[test]
    fn the_page_draws_the_same_primitives_as_the_canvas_lowering() {
        let sheet = sheet();
        let ctx = ctx(sheet.clone());
        let units = MM_TO_PT;
        let page = sheet_frame_elements(&sheet, &templated(), &ctx, 297.0, 210.0, units);

        let canvas = crate::template::lower_sheet_frame(
            297.0,
            210.0,
            &Frame::default(),
            Some(&standard_title_block()),
            |text| {
                text.replace("${TITLE}", "Power supply")
                    .replace("${SHEETNUMBER}", "2")
                    .replace("${SHEETCOUNT}", "3")
                    .replace("${FILENAME}", "power.snxsch")
                    .replace("${REV}", "")
                    .replace("${DATE}", "")
                    .replace("${COMPANY}", "")
            },
        );
        let expected = frame_elements(&canvas, units, PdfOptions::default().palette.sheet_border);
        assert_eq!(format!("{page:?}"), format!("{expected:?}"));
    }

    #[test]
    fn the_title_block_text_lands_in_the_bottom_right_of_the_page() {
        let sheet = sheet();
        let ctx = ctx(sheet.clone());
        let page = sheet_frame_elements(&sheet, &templated(), &ctx, 297.0, 210.0, 1.0);
        let (x, y) = page
            .iter()
            .find_map(|e| match e {
                SvgElement::Text { x, y, text, .. } if text == "Power supply" => Some((*x, *y)),
                _ => None,
            })
            .expect("title drawn");
        // 10 mm margin, 180 x 40 mm block, title 4 mm into it.
        assert!((x - (297.0 - 10.0 - 180.0 + 4.0)).abs() < 1e-3, "{x}");
        assert!((y - (210.0 - 10.0 - 40.0 + 4.0)).abs() < 1e-3, "{y}");
        let sheet_line = page
            .iter()
            .any(|e| matches!(e, SvgElement::Text { text, .. } if text == "Sheet 2 of 3"));
        assert!(sheet_line);
    }

    #[test]
    fn nothing_is_drawn_without_a_title_block_or_a_template() {
        let sheet = sheet();
        let ctx = ctx(sheet.clone());
        let off = PdfOptions {
            include_title_block: false,
            ..templated()
        };
        assert!(sheet_frame_elements(&sheet, &off, &ctx, 297.0, 210.0, 1.0).is_empty());
        let none = PdfOptions {
            sheet_template: None,
            ..templated()
        };
        assert!(sheet_frame_elements(&sheet, &none, &ctx, 297.0, 210.0, 1.0).is_empty());
    }
}
