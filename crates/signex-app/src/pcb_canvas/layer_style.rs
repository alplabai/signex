//! The names and colours the board view gives a board's layers (#2223).
//! Names are catalog messages, except a mechanical layer the user named;
//! colours come from the board palette (`styles::board_canvas_colors`).

use iced::Color;
use signex_gfx::style::ColorSlot;
use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_renderer::pcb::{BOARD_OUTLINE, LayerRow, copper_slot};
use signex_renderer::theme::ResolvedTheme;
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};
use signex_types::theme::{BoardLayerColors, Color as ThemeColor, ThemeTokens};
use signex_widgets::Label;

/// The catalog name of `layer`.
pub fn layer_name(i18n: &I18n, layer: PcbLayer) -> Localized {
    match layer {
        PcbLayer::Copper(copper) => copper_name(i18n, copper),
        PcbLayer::Technical(technical) => i18n.message(technical_id(technical), &[]),
        PcbLayer::Mechanical(m) => {
            t!(i18n, ids::PCB_LAYER_MECHANICAL, "index" => u32::from(m.index()))
        }
    }
}

/// The catalog name of a copper layer.
pub fn copper_name(i18n: &I18n, layer: CopperLayer) -> Localized {
    match layer {
        CopperLayer::Top => t!(i18n, ids::PCB_LAYER_TOP_COPPER),
        CopperLayer::Bottom => t!(i18n, ids::PCB_LAYER_BOTTOM_COPPER),
        CopperLayer::Inner(n) => {
            t!(i18n, ids::PCB_LAYER_INNER_COPPER, "index" => u32::from(n.index()))
        }
    }
}

/// The name a Layers panel row shows: the user's name for a mechanical
/// layer they named, else the catalog name.
pub fn row_label(i18n: &I18n, row: &LayerRow) -> Label<'static> {
    match &row.user_name {
        Some(name) => signex_i18n::UserText::new(name.clone()).into(),
        None => layer_name(i18n, row.layer).into(),
    }
}

fn technical_id(layer: TechnicalLayer) -> MessageId {
    use TechnicalLayer as T;
    match layer {
        T::EdgeCuts => ids::PCB_LAYER_BOARD_OUTLINE,
        T::Margin => ids::PCB_LAYER_MARGIN,
        T::SilkscreenTop => ids::PCB_LAYER_SILKSCREEN_TOP,
        T::SilkscreenBottom => ids::PCB_LAYER_SILKSCREEN_BOTTOM,
        T::SolderMaskTop => ids::PCB_LAYER_SOLDER_MASK_TOP,
        T::SolderMaskBottom => ids::PCB_LAYER_SOLDER_MASK_BOTTOM,
        T::PasteTop => ids::PCB_LAYER_PASTE_TOP,
        T::PasteBottom => ids::PCB_LAYER_PASTE_BOTTOM,
        T::AdhesiveTop => ids::PCB_LAYER_ADHESIVE_TOP,
        T::AdhesiveBottom => ids::PCB_LAYER_ADHESIVE_BOTTOM,
        T::FabTop => ids::PCB_LAYER_FAB_TOP,
        T::FabBottom => ids::PCB_LAYER_FAB_BOTTOM,
        T::AssemblyTop => ids::PCB_LAYER_ASSEMBLY_TOP,
        T::AssemblyBottom => ids::PCB_LAYER_ASSEMBLY_BOTTOM,
        T::CourtyardTop => ids::PCB_LAYER_COURTYARD_TOP,
        T::CourtyardBottom => ids::PCB_LAYER_COURTYARD_BOTTOM,
        T::Body3dTop => ids::PCB_LAYER_BODY_3D_TOP,
        T::Body3dBottom => ids::PCB_LAYER_BODY_3D_BOTTOM,
        T::KeepoutTop => ids::PCB_LAYER_KEEPOUT_TOP,
        T::KeepoutBottom => ids::PCB_LAYER_KEEPOUT_BOTTOM,
        T::FiducialTop => ids::PCB_LAYER_FIDUCIAL_TOP,
        T::FiducialBottom => ids::PCB_LAYER_FIDUCIAL_BOTTOM,
        T::DesignatorTop => ids::PCB_LAYER_DESIGNATOR_TOP,
        T::DesignatorBottom => ids::PCB_LAYER_DESIGNATOR_BOTTOM,
        T::Dimensions => ids::PCB_LAYER_DIMENSIONS,
        T::Notes => ids::PCB_LAYER_NOTES,
        T::DrillDrawing => ids::PCB_LAYER_DRILL_DRAWING,
        T::Eco1 => ids::PCB_LAYER_ECO_1,
        T::Eco2 => ids::PCB_LAYER_ECO_2,
    }
}

/// The colour `layer` is listed with, from the board palette the canvas
/// draws with. Copper, Board Outline and the keep-out layers take the slots
/// their geometry is drawn in. Board views use one palette in both themes,
/// so the chrome `tokens` do not change a swatch.
pub fn layer_colour(layer: PcbLayer, _tokens: &ThemeTokens) -> Color {
    let canvas = crate::styles::board_canvas_colors();
    let slot_colour = |slot: ColorSlot| {
        let [r, g, b, a] = ResolvedTheme::from_canvas_colors(canvas).color(slot);
        Color { r, g, b, a }
    };
    if let Some(slot) = drawn_slot(layer) {
        return slot_colour(slot);
    }
    let colors = canvas.board_layer_colors();
    let cycle = |n: u8| colors.layer_cycle[usize::from(n) % colors.layer_cycle.len()];
    let c = match layer {
        PcbLayer::Copper(copper) => return slot_colour(copper_slot(copper)),
        PcbLayer::Mechanical(n) => cycle(n.index()),
        PcbLayer::Technical(t) => technical_colour(&colors, t),
    };
    Color::from_rgba8(c.r, c.g, c.b, f32::from(c.a) / 255.0)
}

/// The colour the Multi-Layer row is listed with: the slot vias and
/// through-hole pads are drawn in.
pub fn multi_layer_colour() -> Color {
    let [r, g, b, a] = ResolvedTheme::from_canvas_colors(crate::styles::board_canvas_colors())
        .color(ColorSlot::MultiLayer);
    Color { r, g, b, a }
}

/// The canvas slot `layer`'s geometry is drawn in, for the layers whose
/// swatch is that slot.
fn drawn_slot(layer: PcbLayer) -> Option<ColorSlot> {
    match layer {
        _ if layer == BOARD_OUTLINE => Some(ColorSlot::BoardEdge),
        PcbLayer::Technical(TechnicalLayer::KeepoutTop | TechnicalLayer::KeepoutBottom) => {
            Some(ColorSlot::Keepout)
        }
        _ => None,
    }
}

fn technical_colour(colors: &BoardLayerColors, layer: TechnicalLayer) -> ThemeColor {
    use TechnicalLayer as T;
    match layer {
        T::Margin => colors.margin,
        T::SilkscreenTop | T::SilkscreenBottom => colors.silkscreen,
        T::SolderMaskTop | T::SolderMaskBottom => colors.solder_mask,
        T::PasteTop | T::PasteBottom => colors.paste,
        T::Dimensions | T::Notes | T::DrillDrawing | T::Eco1 | T::Eco2 => colors.drawing,
        _ => colors.other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every layer has a catalog name: none falls back to its message id.
    #[test]
    fn every_layer_has_a_catalog_name() {
        let i18n = I18n::en_us();
        for layer in PcbLayer::all() {
            let name = layer_name(&i18n, layer).into_string();
            assert!(!name.starts_with("pcb-layer-"), "{layer:?}: {name}");
        }
        assert_eq!(
            copper_name(&i18n, CopperLayer::Top).into_string(),
            "Top Layer"
        );
    }

    /// The Altium name of every layer kind (decided 2026-11-10). Courtyard,
    /// Fab, Assembly, 3D Body, Fiducial and Designator keep descriptive
    /// names; the edge is the Board Outline row.
    fn altium_name(layer: PcbLayer) -> String {
        use TechnicalLayer as T;
        let technical = |t: TechnicalLayer| match t {
            T::EdgeCuts => "Board Outline",
            T::Margin => "Margin",
            T::SilkscreenTop => "Top Overlay",
            T::SilkscreenBottom => "Bottom Overlay",
            T::SolderMaskTop => "Top Solder",
            T::SolderMaskBottom => "Bottom Solder",
            T::PasteTop => "Top Paste",
            T::PasteBottom => "Bottom Paste",
            T::AdhesiveTop => "Top Adhesive",
            T::AdhesiveBottom => "Bottom Adhesive",
            T::FabTop => "Top Fab",
            T::FabBottom => "Bottom Fab",
            T::AssemblyTop => "Top Assembly",
            T::AssemblyBottom => "Bottom Assembly",
            T::CourtyardTop => "Top Courtyard",
            T::CourtyardBottom => "Bottom Courtyard",
            T::Body3dTop => "Top 3D Body",
            T::Body3dBottom => "Bottom 3D Body",
            T::KeepoutTop => "Top Keep-Out Layer",
            T::KeepoutBottom => "Bottom Keep-Out Layer",
            T::FiducialTop => "Top Fiducial",
            T::FiducialBottom => "Bottom Fiducial",
            T::DesignatorTop => "Top Designator",
            T::DesignatorBottom => "Bottom Designator",
            T::Dimensions => "Dimensions",
            T::Notes => "Notes",
            T::DrillDrawing => "Drill Drawing",
            T::Eco1 => "ECO 1",
            T::Eco2 => "ECO 2",
        };
        match layer {
            PcbLayer::Copper(CopperLayer::Top) => "Top Layer".to_owned(),
            PcbLayer::Copper(CopperLayer::Bottom) => "Bottom Layer".to_owned(),
            PcbLayer::Copper(CopperLayer::Inner(n)) => format!("Mid-Layer {}", n.index()),
            PcbLayer::Mechanical(m) => format!("Mechanical {}", m.index()),
            PcbLayer::Technical(t) => technical(t).to_owned(),
        }
    }

    #[test]
    fn every_layer_kind_has_its_altium_name() {
        let i18n = I18n::en_us();
        let mut seen = std::collections::HashSet::new();
        for layer in PcbLayer::all() {
            // Fluent wraps a placeable ($index) in bidi isolation marks.
            let name: String = layer_name(&i18n, layer)
                .into_string()
                .chars()
                .filter(|c| !matches!(c, '\u{2068}' | '\u{2069}'))
                .collect();
            assert_eq!(name, altium_name(layer), "{layer:?}");
            assert!(seen.insert(name.clone()), "two layers named {name}");
        }
        assert_eq!(
            layer_name(&i18n, BOARD_OUTLINE).into_string(),
            "Board Outline"
        );
    }

    #[test]
    fn the_board_outline_swatch_is_the_drawn_board_edge_colour() {
        let edge = ResolvedTheme::from_canvas_colors(crate::styles::board_canvas_colors())
            .color(ColorSlot::BoardEdge);
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight);
        let swatch = layer_colour(BOARD_OUTLINE, &tokens);
        assert_eq!([swatch.r, swatch.g, swatch.b, swatch.a], edge);
    }

    /// Copper on the canvas is the colour its layer is listed with, on every
    /// copper layer and in both themes.
    #[test]
    fn copper_draws_in_its_swatch_colour() {
        use signex_types::theme::{ThemeId, theme_tokens};
        let theme = ResolvedTheme::from_canvas_colors(crate::styles::board_canvas_colors());
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let tokens = theme_tokens(id);
            for copper in CopperLayer::all() {
                let swatch = layer_colour(PcbLayer::Copper(copper), &tokens);
                let drawn = theme.color(copper_slot(copper));
                assert_eq!(
                    [swatch.r, swatch.g, swatch.b, swatch.a],
                    drawn,
                    "{copper:?}"
                );
            }
        }
    }

    /// A top track and pad and a bottom pad, as the canvas scene draws them,
    /// are their layers' swatch colours.
    #[test]
    fn drawn_track_and_pads_match_the_layers_panel_swatches() {
        use signex_gfx::scene::{DirtyFlags, Scene};
        use signex_renderer::pcb::{CopperInk, PadInput, PcbRenderer, PcbSnapshot, TraceInput};
        use signex_renderer::schematic::ViewRenderer;
        use signex_types::pcb::PadType;
        use signex_types::theme::{ThemeId, theme_tokens};

        let square = vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let pad = |ink| PadInput {
            center: [0.5, 0.5],
            size_mm: [1.0, 1.0],
            outline: square.clone(),
            pad_type: PadType::Smd,
            drill_mm: 0.0,
            dimmed: false,
            ink: CopperInk::Layer(ink),
        };
        let snapshot = PcbSnapshot {
            traces: vec![TraceInput {
                p0: [0.0, 0.0],
                p1: [5.0, 0.0],
                width_mm: 0.25,
                net: 1,
                dimmed: false,
                ink: CopperInk::Layer(CopperLayer::Top),
            }],
            pads: vec![pad(CopperLayer::Top), pad(CopperLayer::Bottom)],
            ..PcbSnapshot::default()
        };
        let theme = ResolvedTheme::from_canvas_colors(crate::styles::board_canvas_colors());
        let mut scene = Scene::default();
        PcbRenderer::build_scene(
            &snapshot,
            &theme,
            DirtyFlags::LINES | DirtyFlags::POLYGONS,
            &mut scene,
        );
        let swatch = |copper| {
            let c = layer_colour(
                PcbLayer::Copper(copper),
                &theme_tokens(ThemeId::GraphiteDark),
            );
            [c.r, c.g, c.b, c.a]
        };
        assert_eq!(scene.lines[0].color, swatch(CopperLayer::Top));
        assert_eq!(scene.top_polygons[0].fill_color, swatch(CopperLayer::Top));
        assert_eq!(
            scene.top_polygons[1].fill_color,
            swatch(CopperLayer::Bottom)
        );
    }

    /// A via and a through-hole pad as drawn are the Multi-Layer swatch.
    #[test]
    fn through_items_match_the_multi_layer_swatch() {
        use signex_gfx::scene::{DirtyFlags, Scene};
        use signex_renderer::pcb::{CopperInk, PadInput, PcbRenderer, PcbSnapshot, ViaInput};
        use signex_renderer::schematic::ViewRenderer;
        use signex_types::pcb::PadType;

        let snapshot = PcbSnapshot {
            vias: vec![ViaInput {
                center: [0.0, 0.0],
                diameter_mm: 0.8,
                drill_mm: 0.4,
                net: 1,
                dimmed: false,
                ink: CopperInk::MultiLayer,
            }],
            pads: vec![PadInput {
                center: [0.5, 0.5],
                size_mm: [1.0, 1.0],
                outline: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
                pad_type: PadType::Thru,
                drill_mm: 0.5,
                dimmed: false,
                ink: CopperInk::MultiLayer,
            }],
            ..PcbSnapshot::default()
        };
        let theme = ResolvedTheme::from_canvas_colors(crate::styles::board_canvas_colors());
        let mut scene = Scene::default();
        PcbRenderer::build_scene(
            &snapshot,
            &theme,
            DirtyFlags::CIRCLES | DirtyFlags::POLYGONS,
            &mut scene,
        );
        let c = multi_layer_colour();
        let swatch = [c.r, c.g, c.b, c.a];
        assert_eq!(scene.circles[0].color, swatch, "via ring");
        assert_eq!(scene.top_polygons[0].fill_color, swatch, "THT pad");
        let palette = crate::styles::board_canvas_colors().board_layer_colors();
        assert_eq!(rgba(c), palette_rgba(palette.multi_layer));
    }

    fn rgba(c: Color) -> [u8; 4] {
        c.into_rgba8()
    }

    fn palette_rgba(c: ThemeColor) -> [u8; 4] {
        [c.r, c.g, c.b, c.a]
    }

    /// Every swatch is a board-palette colour: keep-out is the drawn
    /// keepout slot, copper and the other families their palette entry,
    /// and the light and dark chrome list the same colours.
    #[test]
    fn every_swatch_is_its_board_palette_colour() {
        use signex_types::theme::{ThemeId, theme_tokens};
        let canvas = crate::styles::board_canvas_colors();
        let theme = ResolvedTheme::from_canvas_colors(canvas);
        let colors = canvas.board_layer_colors();
        let light = theme_tokens(ThemeId::GraphiteLight);
        let dark = theme_tokens(ThemeId::GraphiteDark);
        let keepout = theme.color(ColorSlot::Keepout);
        for t in [TechnicalLayer::KeepoutTop, TechnicalLayer::KeepoutBottom] {
            let s = layer_colour(PcbLayer::Technical(t), &light);
            assert_eq!([s.r, s.g, s.b, s.a], keepout, "{t:?}");
        }
        let top = layer_colour(PcbLayer::Copper(CopperLayer::Top), &light);
        assert_eq!(rgba(top), palette_rgba(colors.top_copper));
        let bottom = layer_colour(PcbLayer::Copper(CopperLayer::Bottom), &light);
        assert_eq!(rgba(bottom), palette_rgba(colors.bottom_copper));
        let silk = layer_colour(PcbLayer::Technical(TechnicalLayer::SilkscreenTop), &light);
        assert_eq!(rgba(silk), palette_rgba(colors.silkscreen));

        let mut palette: Vec<[u8; 4]> = colors.layer_cycle.map(palette_rgba).to_vec();
        palette.extend(
            [
                colors.top_copper,
                colors.bottom_copper,
                colors.margin,
                colors.silkscreen,
                colors.solder_mask,
                colors.paste,
                colors.drawing,
                colors.other,
                canvas.board_edge_color(),
                canvas.keepout_color(),
            ]
            .map(palette_rgba),
        );
        for layer in PcbLayer::all() {
            let swatch = layer_colour(layer, &light);
            assert!(palette.contains(&rgba(swatch)), "{layer:?}: {swatch:?}");
            assert_eq!(swatch, layer_colour(layer, &dark), "{layer:?}");
        }
    }

    /// The board palette lives on the Graphite Dark canvas, the one both
    /// themes draw boards with.
    #[test]
    fn the_board_layer_colours_are_set_on_the_dark_canvas() {
        use signex_types::theme::{ThemeId, canvas_colors};
        assert!(canvas_colors(ThemeId::GraphiteDark).board_layers.is_some());
        assert_eq!(
            crate::styles::board_canvas_colors().board_layer_colors(),
            canvas_colors(ThemeId::GraphiteDark).board_layer_colors()
        );
    }
}
