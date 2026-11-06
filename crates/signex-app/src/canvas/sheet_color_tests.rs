//! #2034: the canvas paper is the sheet's own colour when it has one, else
//! the theme's, including after a theme switch.

use super::tests::test_prefs;
use super::*;
use signex_types::schematic::{SchematicSheet, SheetPageSettings};
use signex_types::theme::{ThemeId, canvas_colors};

fn slot_for(color: Option<signex_types::theme::Color>) -> CanvasSlot {
    let sheet = SchematicSheet {
        paper_size: "A4".into(),
        page: SheetPageSettings {
            color,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut slot = CanvasSlot::new();
    slot.set_render_cache(Some(
        crate::schematic_runtime::SchematicRenderCache::from_sheet(&sheet),
    ));
    slot
}

fn paper(slot: &CanvasSlot, theme: ThemeId) -> Color {
    let overrides = Default::default();
    let filters = Default::default();
    let mut prefs = test_prefs(&overrides, &filters);
    prefs.theme_paper = crate::render_config::to_iced(&canvas_colors(theme).paper);
    SchematicCanvas::new(slot, prefs).paper_color()
}

#[test]
fn the_default_sheet_colour_follows_a_theme_switch() {
    let slot = slot_for(None);
    for theme in [ThemeId::Signex, ThemeId::CatppuccinMocha] {
        assert_eq!(
            paper(&slot, theme),
            crate::render_config::to_iced(&canvas_colors(theme).paper)
        );
    }
}

#[test]
fn a_set_sheet_colour_does_not_follow_a_theme_switch() {
    let gray = signex_types::theme::Color::new(0xd0, 0xd0, 0xd0, 0xff);
    let slot = slot_for(Some(gray));
    let want = crate::render_config::to_iced(&gray);
    assert_eq!(paper(&slot, ThemeId::Signex), want);
    assert_eq!(paper(&slot, ThemeId::CatppuccinMocha), want);
}

/// Review of #2034: paper is opaque on the canvas, as in export, whatever
/// alpha the file stores.
#[test]
fn a_translucent_sheet_colour_draws_opaque() {
    let half = signex_types::theme::Color::new(0xd0, 0xd0, 0xd0, 0x80);
    let slot = slot_for(Some(half));
    let paper = paper(&slot, ThemeId::Signex);
    assert_eq!(paper.a, 1.0);
    assert_eq!(
        paper,
        crate::render_config::to_iced(&signex_types::theme::Color { a: 0xff, ..half })
    );
}
