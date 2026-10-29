//! The sheet frame and title block on an exported page (#1079).
//!
//! Lowered by [`crate::template::lower_sheet_frame`], the same lowering the
//! canvas draws, then turned into page elements here so the PDF content
//! stream and the print-preview raster draw it alike.

use super::layout::PageTransform;
use super::*;
use crate::svg::{SvgPoint, SvgStyle};
use crate::template::{FramePrimitive, FrameTextAnchor};

/// Frame and title-block elements for one page, in page units (top-left
/// origin, Y down). Empty unless the options ask for a title block.
///
/// The frame is lowered for the sheet's own paper and placed with the same
/// [`PageTransform`] as the content, which lays out that paper rectangle
/// while the frame is on, so the page matches the canvas (#1134). A chosen
/// template supplies its frame and title block; otherwise the standard ones
/// the canvas draws are used. The sheet's own frame settings then decide
/// the border, the zone counts and whether the title block is drawn.
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
    let (frame, title_block) = match opts
        .sheet_template
        .as_ref()
        .and_then(crate::template::load_builtin)
    {
        Some(template) => (template.frame, template.title_block),
        None => (
            crate::template::Frame::default(),
            crate::template::standard_title_block(),
        ),
    };

    let metadata = ctx.metadata.for_sheet(&sheet.schematic.title_block);
    let sub_ctx = sheet_substitution(sheet, opts, &metadata);
    let (paper_w_mm, paper_h_mm) = super::layout::sheet_paper_mm(sheet);
    let settings = &sheet.schematic.frame;
    let primitives = crate::template::lower_sheet_frame(
        paper_w_mm,
        paper_h_mm,
        &frame.with_sheet_settings(settings),
        settings.show_title_block.then_some(&title_block),
        |text| crate::resolve(text, &sub_ctx),
    );
    let xform = PageTransform::new(sheet, page_w_mm, page_h_mm, opts, units_per_mm);
    frame_elements(&primitives, &xform, opts.palette.sheet_border)
}

/// The special-string context for `sheet` on a page printed with `opts`:
/// the one the title block, and every text item, resolve their `${TOKEN}`s
/// with. `metadata` is the sheet's own ([`crate::ProjectMetadata::for_sheet`]).
pub(crate) fn sheet_substitution<'a>(
    sheet: &crate::SheetSnapshot,
    opts: &PdfOptions,
    metadata: &'a crate::ProjectMetadata,
) -> SubstitutionContext<'a> {
    SubstitutionContext {
        metadata,
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
    }
}

/// Turn frame primitives (sheet mm) into page elements through `xform`.
pub(crate) fn frame_elements(
    primitives: &[FramePrimitive],
    xform: &PageTransform,
    rgb: (f32, f32, f32),
) -> Vec<SvgElement> {
    let u = |mm: f64| (mm * xform.mm_to_unit) as f32;
    let at = |x: f64, y: f64| SvgPoint {
        x: xform.x(x),
        y: xform.px_y(y),
    };
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
                let p = at(*x, *y);
                SvgElement::Text {
                    x: p.x,
                    y: p.y,
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
            placement: Default::default(),
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
            netlist_placements: Vec::new(),
            current_sheet: None,
        }
    }

    /// The frame on, on a page the size of the sheet with no margins, at
    /// 1:1, so page units are sheet mm.
    fn on_sheet() -> PdfOptions {
        PdfOptions {
            include_title_block: true,
            margins: Margins {
                top_mm: 0.0,
                right_mm: 0.0,
                bottom_mm: 0.0,
                left_mm: 0.0,
            },
            scale: PdfScale::OneToOne,
            ..PdfOptions::default()
        }
    }

    fn text_at(page: &[SvgElement], wanted: &str) -> Option<(f32, f32)> {
        page.iter().find_map(|e| match e {
            SvgElement::Text { x, y, text, .. } if text == wanted => Some((*x, *y)),
            _ => None,
        })
    }

    #[test]
    fn the_page_draws_the_same_primitives_as_the_canvas_lowering() {
        let sheet = sheet();
        let ctx = ctx(sheet.clone());
        let opts = PdfOptions::default();
        let page = sheet_frame_elements(&sheet, &opts, &ctx, 297.0, 210.0, MM_TO_PT);

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
                    .replace("${DOCUMENTNUMBER}", "")
                    .replace("${DRAWNBY}", "")
                    .replace("${CHECKEDBY}", "")
                    .replace("${APPROVEDBY}", "")
            },
        );
        let xform = PageTransform::new(&sheet, 297.0, 210.0, &opts, MM_TO_PT);
        let expected = frame_elements(&canvas, &xform, opts.palette.sheet_border);
        assert_eq!(format!("{page:?}"), format!("{expected:?}"));
    }

    #[test]
    fn the_frame_prints_by_default() {
        let sheet = sheet();
        let ctx = ctx(sheet.clone());
        let page = sheet_frame_elements(&sheet, &PdfOptions::default(), &ctx, 297.0, 210.0, 1.0);
        assert!(text_at(&page, "Power supply").is_some());
        assert!(text_at(&page, "2 of 3").is_some());
    }

    #[test]
    fn the_sheet_frame_settings_reach_the_page() {
        let mut sheet = sheet();
        sheet.schematic.frame = signex_types::schematic::SheetFrameSettings {
            show_border: true,
            show_title_block: false,
            horizontal_zones: 3,
            vertical_zones: 2,
        };
        let export = ctx(sheet.clone());
        let page = sheet_frame_elements(&sheet, &on_sheet(), &export, 297.0, 210.0, 1.0);
        assert!(
            text_at(&page, "Power supply").is_none(),
            "title block hidden"
        );
        assert!(text_at(&page, "3").is_some() && text_at(&page, "4").is_none());
        assert!(text_at(&page, "B").is_some() && text_at(&page, "C").is_none());

        sheet.schematic.frame.show_border = false;
        let export = ctx(sheet.clone());
        let page = sheet_frame_elements(&sheet, &on_sheet(), &export, 297.0, 210.0, 1.0);
        assert!(page.is_empty(), "{page:?}");
    }

    #[test]
    fn the_title_block_text_lands_where_the_canvas_draws_it() {
        let sheet = sheet();
        let ctx = ctx(sheet.clone());
        let page = sheet_frame_elements(&sheet, &on_sheet(), &ctx, 297.0, 210.0, 1.0);
        let (x, y) = text_at(&page, "Power supply").expect("title drawn");
        // 10 mm margin, 110 x 28 mm block, the title's value under its
        // caption in the first cell.
        let below_caption = (0.7 + crate::template::CELL_CAPTION_MM + 0.3) as f32;
        assert!((x - (297.0 - 10.0 - 110.0 + 0.7)).abs() < 1e-3, "{x}");
        assert!(
            (y - (210.0 - 10.0 - 28.0 + below_caption)).abs() < 1e-3,
            "{y}"
        );
    }

    #[test]
    fn content_keeps_its_place_against_the_title_block() {
        // A wire end placed inside the title block's area on the sheet
        // stays inside it on the page, at any page size and margin: content
        // and frame share one transform.
        use signex_types::schematic::{Point, Wire};
        let mut sheet = sheet();
        sheet.schematic.wires.push(Wire {
            start: Point::new(20.0, 20.0),
            end: Point::new(200.0, 180.0),
            stroke_width: 0.15,
            ..Default::default()
        });
        let ctx = ctx(sheet.clone());
        let opts = PdfOptions {
            page_size: PageSize::IsoA3,
            ..PdfOptions::default()
        };
        let (pw, ph) = (420.0, 297.0);
        let page = sheet_frame_elements(&sheet, &opts, &ctx, pw, ph, 1.0);
        let xform = PageTransform::new(&sheet, pw, ph, &opts, 1.0);
        let block_stroke = (crate::template::frame::TITLE_BLOCK_LINE_MM * xform.mm_to_unit) as f32;
        let block = page
            .iter()
            .find_map(|e| match e {
                SvgElement::Path { commands, style }
                    if (style.stroke_width - block_stroke).abs() < 1e-6 =>
                {
                    match (&commands[0], &commands[2]) {
                        (SvgPathCommand::MoveTo(a), SvgPathCommand::LineTo(b)) => {
                            Some((a.x, a.y, b.x, b.y))
                        }
                        _ => None,
                    }
                }
                _ => None,
            })
            .expect("title block outline");
        let (wx, wy) = (xform.x(200.0), xform.px_y(180.0));
        assert!(
            wx > block.0 && wx < block.2 && wy > block.1 && wy < block.3,
            "wire end {wx},{wy} outside block {block:?}"
        );
    }

    #[test]
    fn nothing_is_drawn_without_a_title_block() {
        let sheet = sheet();
        let ctx = ctx(sheet.clone());
        let off = PdfOptions {
            include_title_block: false,
            ..PdfOptions::default()
        };
        assert!(sheet_frame_elements(&sheet, &off, &ctx, 297.0, 210.0, 1.0).is_empty());
    }
}

#[cfg(test)]
mod special_string_tests {
    use super::*;
    use crate::svg::{SvgEvaluatorInputs, SvgRenderContext};
    use crate::{ProjectMetadata, SheetSnapshot};
    use std::collections::{BTreeMap, HashMap};

    const NOTE: &str = "${PROJECT_NAME}: ${TITLE} by ${DRAWNBY} (${SHEETNUMBER}/${SHEETCOUNT})";

    /// A text note that names tokens prints, in the SVG/PDF page, the string
    /// the resolver gives the title block, with the sheet override applied.
    #[test]
    fn a_text_note_resolves_the_same_tokens_as_the_title_block() {
        let mut schematic = signex_types::schematic::SchematicSheet::default();
        schematic.title_block.insert("title".into(), "Power".into());
        schematic
            .text_notes
            .push(signex_types::schematic::TextNote {
                uuid: uuid::Uuid::now_v7(),
                text: NOTE.into(),
                position: signex_types::schematic::Point::new(10.0, 10.0),
                rotation: 0.0,
                font_size: 1.27,
                font_family: None,
                justify_h: Default::default(),
                justify_v: Default::default(),
                locked: false,
            });
        let sheet = SheetSnapshot {
            path: std::path::PathBuf::from("/p/power.snxsch"),
            schematic,
            sheet_name: "Power".into(),
            sheet_number: 2,
            sheet_count: 3,
            placement: Default::default(),
        };
        let mut metadata = ProjectMetadata {
            project_parameters: Some(BTreeMap::from([
                ("title".to_string(), "Project".to_string()),
                ("drawn_by".to_string(), "A. Engineer".to_string()),
            ])),
            ..ProjectMetadata::default()
        };
        metadata
            .custom_fields
            .insert("project_name".into(), "Oscar".into());
        let opts = PdfOptions {
            include_notes: true,
            ..PdfOptions::default()
        };
        let sheet_metadata = metadata.for_sheet(&sheet.schematic.title_block);
        let special = sheet_substitution(&sheet, &opts, &sheet_metadata);
        let wanted = "Oscar: Power by A. Engineer (2/3)";
        assert_eq!(crate::resolve(NOTE, &special), wanted);

        let (refdes, nets) = (HashMap::new(), HashMap::new());
        let inputs = SvgEvaluatorInputs {
            special_strings: Some(&special),
            global_refdes: &refdes,
            net_name_by_symbol_pin: &nets,
            cell: "",
        };
        let page = SvgRenderContext::from_sheet(&sheet, &opts, 297.0, 210.0, 1.0, Some(&inputs));
        assert!(
            page.elements
                .iter()
                .any(|e| matches!(e, SvgElement::Text { text, .. } if text == wanted)),
            "the note prints the resolved string"
        );
    }
}
