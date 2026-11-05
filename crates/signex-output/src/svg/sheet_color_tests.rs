//! #2034: a sheet with its own paper colour fills the exported page with it;
//! a sheet at Default takes the palette (theme) paper.

use std::path::PathBuf;

use signex_types::schematic::{SchematicSheet, SheetPageSettings};
use signex_types::theme::{Color, ThemeId, canvas_colors};

use super::*;
use crate::SheetSnapshot;
use crate::pdf::PdfOptions;
use crate::pdf::palette::SchematicPalette;

fn page_fill(color: Option<Color>, theme: ThemeId) -> (f32, f32, f32) {
    let snapshot = SheetSnapshot {
        path: PathBuf::from("test.snxsch"),
        schematic: SchematicSheet {
            paper_size: "A4".to_string(),
            page: SheetPageSettings {
                color,
                ..Default::default()
            },
            ..Default::default()
        },
        sheet_name: "Sheet1".to_string(),
        sheet_number: 1,
        sheet_count: 1,
        placement: Default::default(),
    };
    let opts = PdfOptions {
        palette: SchematicPalette::from(&canvas_colors(theme)),
        ..PdfOptions::default()
    };
    let page = SvgRenderContext::from_sheet(&snapshot, &opts, 297.0, 210.0, 1.0, None);
    assert_eq!(page.paper, {
        match &page.elements[0] {
            SvgElement::Path { style, .. } => style.fill_rgb.expect("page fill"),
            SvgElement::Text { .. } => panic!("the page fill comes first"),
        }
    });
    page.paper
}

#[test]
fn the_default_sheet_colour_follows_the_theme_and_a_set_one_does_not() {
    let light = page_fill(None, ThemeId::Signex);
    let dark = page_fill(None, ThemeId::CatppuccinMocha);
    assert_ne!(light, dark, "Default follows the theme");
    let gray = Color::new(0xd0, 0xd0, 0xd0, 0xff);
    let want = (208.0 / 255.0, 208.0 / 255.0, 208.0 / 255.0);
    assert_eq!(page_fill(Some(gray), ThemeId::Signex), want);
    assert_eq!(page_fill(Some(gray), ThemeId::CatppuccinMocha), want);
}
