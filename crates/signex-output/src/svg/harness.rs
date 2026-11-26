//! Signal harnesses on paper (#1280), drawn as on the canvas: harness wires
//! thicker than a bus in the harness colour, connectors with a pointed tip
//! filled with the soft harness tint, entry tabs on the flat side with their
//! names, the type name above each connector, and a harness-typed sheet
//! entry as a tab with its type. PDF pages are drawn from these elements.

use signex_types::schematic::{
    Point, SCHEMATIC_HARNESS_OUTLINE_MM, SCHEMATIC_HARNESS_STROKE_MM, SCHEMATIC_TEXT_MM,
    SchematicSheet,
};

use super::labels::label_size_pt;
use super::{SvgElement, SvgPathCommand, SvgStyle, SvgTextAlign, SvgTextVAlign, pt};
use crate::pdf::PdfOptions;
use crate::pdf::layout::PageTransform;
use crate::pdf::palette::SchematicPalette;

/// The font alias harness names print in, as net labels.
const FONT: &str = "F3";

/// Append every harness object of `sheet` to `elements`.
pub(super) fn push_harness(
    elements: &mut Vec<SvgElement>,
    sheet: &SchematicSheet,
    xform: &PageTransform,
    palette: &SchematicPalette,
    opts: &PdfOptions,
) {
    let h = &sheet.harness;
    if h.connectors.is_empty() && h.wires.is_empty() && h.entry_types.is_empty() {
        return;
    }
    let at = |p: Point| pt(xform.x(p.x), xform.px_y(p.y));
    let outline = SvgStyle {
        stroke_alpha: 1.0,
        stroke_rgb: Some(palette.harness),
        fill_rgb: Some(palette.harness_soft),
        stroke_width: (SCHEMATIC_HARNESS_OUTLINE_MM * xform.mm_to_unit) as f32,
    };
    let size_pt = label_size_pt(SCHEMATIC_TEXT_MM, xform.mm_to_unit, &opts.scale, FONT);
    let text = |p: Point, align: SvgTextAlign, v_align: SvgTextVAlign, s: &str| SvgElement::Text {
        x: xform.x(p.x),
        y: xform.px_y(p.y),
        font_alias: FONT,
        size_pt,
        align,
        v_align,
        rotation_deg: 0.0,
        fill_rgb: palette.harness,
        text: s.to_string(),
    };

    for w in &h.wires {
        elements.push(SvgElement::Path {
            commands: vec![
                SvgPathCommand::MoveTo(at(w.start)),
                SvgPathCommand::LineTo(at(w.end)),
            ],
            style: SvgStyle {
                stroke_alpha: 1.0,
                stroke_rgb: Some(palette.harness),
                fill_rgb: None,
                stroke_width: (SCHEMATIC_HARNESS_STROKE_MM * xform.mm_to_unit) as f32,
            },
        });
    }

    for c in &h.connectors {
        elements.push(polygon(&c.outline(), &at, outline));
        for (i, e) in c.entries.iter().enumerate() {
            elements.push(polygon(&c.entry_tab(i), &at, outline));
            elements.push(text(
                c.entry_label_point(i),
                SvgTextAlign::Left,
                SvgTextVAlign::Center,
                &e.name,
            ));
        }
        let b = c.bounds();
        elements.push(text(
            Point::new(b.min_x, b.min_y - 0.5),
            SvgTextAlign::Left,
            SvgTextVAlign::Bottom,
            &c.harness_type,
        ));
    }

    // A harness-typed sheet entry: a tab in the harness colour and its type.
    for cs in &sheet.child_sheets {
        for pin in &cs.pins {
            let Some(ty) = h.entry_type(pin.uuid) else {
                continue;
            };
            let p = pin.position;
            let tab = [
                Point::new(p.x - 1.27, p.y - 0.6),
                Point::new(p.x + 0.6, p.y - 0.6),
                Point::new(p.x + 1.27, p.y),
                Point::new(p.x + 0.6, p.y + 0.6),
                Point::new(p.x - 1.27, p.y + 0.6),
            ];
            elements.push(polygon(&tab, &at, outline));
            elements.push(text(
                Point::new(p.x + 1.8, p.y),
                SvgTextAlign::Left,
                SvgTextVAlign::Center,
                ty,
            ));
        }
    }
}

fn polygon(
    points: &[Point],
    at: &impl Fn(Point) -> super::SvgPoint,
    style: SvgStyle,
) -> SvgElement {
    let mut commands = Vec::with_capacity(points.len() + 1);
    for (i, p) in points.iter().enumerate() {
        commands.push(if i == 0 {
            SvgPathCommand::MoveTo(at(*p))
        } else {
            SvgPathCommand::LineTo(at(*p))
        });
    }
    commands.push(SvgPathCommand::Close);
    SvgElement::Path { commands, style }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{HarnessConnector, HarnessEntry, HarnessWire};
    use uuid::Uuid;

    fn sheet() -> SchematicSheet {
        let mut s = SchematicSheet {
            paper_size: "A4".into(),
            ..Default::default()
        };
        s.harness.connectors.push(HarnessConnector {
            uuid: Uuid::from_u128(1),
            harness_type: "SPI_BUS".into(),
            position: Point::new(20.0, 20.0),
            width: 10.16,
            entry_pitch: 2.54,
            rotation: 0.0,
            entries: vec![HarnessEntry {
                uuid: Uuid::from_u128(2),
                name: "SCK".into(),
            }],
            locked: false,
        });
        s.harness.wires.push(HarnessWire {
            uuid: Uuid::from_u128(3),
            start: Point::new(40.0, 22.0),
            end: Point::new(60.0, 22.0),
            locked: false,
        });
        s
    }

    fn drawn(s: &SchematicSheet) -> Vec<SvgElement> {
        let xform = PageTransform {
            mm_to_unit: 1.0,
            translate_x: 0.0,
            translate_y: 0.0,
        };
        let mut out = Vec::new();
        push_harness(
            &mut out,
            s,
            &xform,
            &SchematicPalette::classic(),
            &PdfOptions::default(),
        );
        out
    }

    #[test]
    fn a_harness_prints_its_wire_connector_entries_and_names() {
        let out = drawn(&sheet());
        let palette = SchematicPalette::classic();
        let widths: Vec<f32> = out
            .iter()
            .filter_map(|e| match e {
                SvgElement::Path { style, .. } if style.fill_rgb.is_none() => {
                    Some(style.stroke_width)
                }
                _ => None,
            })
            .collect();
        // The harness wire, thicker than a bus.
        assert_eq!(widths, [SCHEMATIC_HARNESS_STROKE_MM as f32]);
        let texts: Vec<&str> = out
            .iter()
            .filter_map(|e| match e {
                SvgElement::Text { text, fill_rgb, .. } => {
                    assert_eq!(*fill_rgb, palette.harness);
                    Some(text.as_str())
                }
                _ => None,
            })
            .collect();
        assert_eq!(texts, ["SCK", "SPI_BUS"]);
    }

    #[test]
    fn a_sheet_without_harness_prints_nothing_more() {
        let s = SchematicSheet::default();
        assert!(drawn(&s).is_empty());
    }
}
