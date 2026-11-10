//! The names and colours the board view gives a board's layers (#2223).
//! Names are catalog messages, except a mechanical layer the user named;
//! colours are the one place a layer's colour is decided, so the Graphite
//! board palette can plug in here.

use iced::Color;
use signex_gfx::style::ColorSlot;
use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_renderer::pcb::{BOARD_OUTLINE, LayerRow};
use signex_renderer::theme::ResolvedTheme;
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};
use signex_types::theme::ThemeTokens;
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

/// The colour `layer` is listed with: red top, blue bottom, a cycle of hues
/// for the inner and mechanical layers, one per technical family. Board
/// Outline takes the board edge's own colour, so its swatch matches the
/// drawn edge. `tokens` is unused until the Graphite board palette (#2229)
/// decides the rest.
pub fn layer_colour(layer: PcbLayer, _tokens: &ThemeTokens) -> Color {
    if layer == BOARD_OUTLINE {
        let [r, g, b, a] = ResolvedTheme::from_canvas_colors(crate::styles::board_canvas_colors())
            .color(ColorSlot::BoardEdge);
        return Color { r, g, b, a };
    }
    const CYCLE: [[u8; 3]; 6] = [
        [0xC8, 0xA0, 0x2C],
        [0x3C, 0xA8, 0x6A],
        [0xA0, 0x58, 0xC8],
        [0xC8, 0x6A, 0x2C],
        [0x2C, 0xA8, 0xB8],
        [0xB8, 0x4C, 0x8C],
    ];
    let [r, g, b] = match layer {
        PcbLayer::Copper(CopperLayer::Top) => [0xD0, 0x40, 0x40],
        PcbLayer::Copper(CopperLayer::Bottom) => [0x38, 0x6C, 0xD0],
        PcbLayer::Copper(CopperLayer::Inner(n)) => CYCLE[usize::from(n.index()) % CYCLE.len()],
        PcbLayer::Mechanical(n) => CYCLE[usize::from(n.index()) % CYCLE.len()],
        PcbLayer::Technical(t) => technical_colour(t),
    };
    Color::from_rgb8(r, g, b)
}

fn technical_colour(layer: TechnicalLayer) -> [u8; 3] {
    use TechnicalLayer as T;
    match layer {
        T::Margin => [0xE0, 0xD0, 0x40],
        T::SilkscreenTop | T::SilkscreenBottom => [0xE8, 0xE8, 0xE8],
        T::SolderMaskTop | T::SolderMaskBottom => [0x40, 0x9C, 0x58],
        T::PasteTop | T::PasteBottom => [0xA8, 0xA8, 0xB8],
        T::KeepoutTop | T::KeepoutBottom => [0xE0, 0x50, 0x90],
        T::Dimensions | T::Notes | T::DrillDrawing | T::Eco1 | T::Eco2 => [0x88, 0x88, 0xA0],
        _ => [0x70, 0x90, 0xB0],
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
}
