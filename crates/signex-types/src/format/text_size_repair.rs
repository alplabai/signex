//! Text sizes no schematic uses, brought back to a drawable size on load.
//!
//! A file-supplied `font_size` drives glyph shaping and, past the atlas
//! ceiling, outline tessellation, so a hostile or corrupt size must not reach
//! the renderer as it is. [`MAX_TEXT_SIZE_MM`] is generous, so no real sheet
//! changes; one that does is reported through `SheetLoadRepair`.

use super::load_repair::{RepairedObject, SheetLoadRepair, kind};
use crate::schematic::SchematicSheet;

/// Largest text height (mm) a loaded sheet keeps. A title in a large format
/// is 10 to 25 mm; 100 mm text is a quarter of an A4 page, so nothing a
/// drawing needs is above it. At the canvas zoom limit (48 px/mm) and a 3x
/// display it stays under the drawn-text ceiling
/// (`signex_gfx::primitive::text::MAX_DRAWN_TEXT_DEVICE_PX`).
pub const MAX_TEXT_SIZE_MM: f64 = 100.0;

fn clamp(size: &mut f64, object: impl FnOnce() -> RepairedObject, made: &mut Vec<SheetLoadRepair>) {
    if size.is_finite() && *size <= MAX_TEXT_SIZE_MM {
        return;
    }
    let old = *size;
    // A size that is not a number has no real size to keep; the default is the
    // safe reading, not the largest one.
    let new = if old.is_finite() {
        MAX_TEXT_SIZE_MM
    } else {
        crate::schematic::SCHEMATIC_TEXT_MM
    };
    *size = new;
    made.push(SheetLoadRepair::TextSizeClamped {
        object: object(),
        old,
        new,
    });
}

/// Clamp every label, text note and symbol field text above
/// [`MAX_TEXT_SIZE_MM`]; one repair each.
pub(super) fn clamp_oversize_text(sheet: &mut SchematicSheet) -> Vec<SheetLoadRepair> {
    let mut made = Vec::new();
    for (i, label) in sheet.labels.iter_mut().enumerate() {
        let name = label.text.clone();
        clamp(
            &mut label.font_size,
            || RepairedObject {
                kind: kind::LABEL,
                name,
                position: i + 1,
            },
            &mut made,
        );
    }
    for (i, note) in sheet.text_notes.iter_mut().enumerate() {
        let name = note.text.clone();
        clamp(
            &mut note.font_size,
            || RepairedObject {
                kind: kind::TEXT_NOTE,
                name,
                position: i + 1,
            },
            &mut made,
        );
    }
    for (i, symbol) in sheet.symbols.iter_mut().enumerate() {
        let name = symbol.reference.clone();
        for prop in [symbol.ref_text.as_mut(), symbol.val_text.as_mut()]
            .into_iter()
            .flatten()
        {
            clamp(
                &mut prop.font_size,
                || RepairedObject {
                    kind: kind::SYMBOL,
                    name: name.clone(),
                    position: i + 1,
                },
                &mut made,
            );
        }
    }
    made
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schematic::{HAlign, Point, TextNote, VAlign};

    fn note(size: f64) -> TextNote {
        TextNote {
            uuid: uuid::Uuid::now_v7(),
            text: "big".into(),
            position: Point::new(0.0, 0.0),
            rotation: 0.0,
            font_size: size,
            font_family: None,
            justify_h: HAlign::Left,
            justify_v: VAlign::Top,
            locked: false,
        }
    }

    #[test]
    fn an_absurd_size_is_clamped_and_reported_and_real_sizes_are_kept() {
        let mut sheet = SchematicSheet {
            text_notes: vec![
                note(1.27),
                note(25.0),
                note(150.0),
                note(1.0e5),
                note(f64::MAX),
                note(f64::NAN),
            ],
            ..Default::default()
        };
        let repairs = clamp_oversize_text(&mut sheet);
        let sizes: Vec<f64> = sheet.text_notes.iter().map(|n| n.font_size).collect();
        assert_eq!(
            sizes,
            [
                1.27,
                25.0,
                100.0,
                MAX_TEXT_SIZE_MM,
                MAX_TEXT_SIZE_MM,
                crate::schematic::SCHEMATIC_TEXT_MM
            ],
            "an oversize size is capped, a non-finite one reads as the default"
        );
        assert_eq!(repairs.len(), 4);
        assert!(repairs[0].describe().contains("text note big #3"));
    }

    /// The repair runs on every parse, so a hostile file reaches the app
    /// already clamped, with the repair to report.
    #[test]
    fn parsing_a_file_with_an_absurd_text_size_clamps_it() {
        let sheet = SchematicSheet {
            text_notes: vec![note(1.0e5)],
            ..Default::default()
        };
        let text = crate::format::SnxSchematic {
            format: String::new(),
            sheet,
        }
        .write_string()
        .expect("writes");
        let mut parsed = crate::format::SnxSchematic::parse(&text)
            .expect("parses")
            .sheet;
        assert_eq!(parsed.text_notes[0].font_size, MAX_TEXT_SIZE_MM);
        assert_eq!(parsed.take_load_repairs().len(), 1);
    }
}
