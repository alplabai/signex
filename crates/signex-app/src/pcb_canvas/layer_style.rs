//! The names and colours the board view gives a board's layers (#2223).
//! Names are catalog messages, except a mechanical layer the user named;
//! colours come from the board palette (`styles::board_canvas_colors`).

use iced::Color;
use signex_gfx::style::ColorSlot;
use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_renderer::pcb::{BoardPaint, LayerRow};
use signex_renderer::theme::ResolvedTheme;
use signex_types::format::pcb_layer_token;
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};
use signex_types::theme::ThemeTokens;
use signex_widgets::Label;

use crate::layer_strip_prefs::LayerNameStyle;

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

/// The name of `layer` in `style` (#2394): the short and medium names come
/// from the catalog; a technical layer has no medium name and keeps its
/// long one.
pub fn styled_layer_name(i18n: &I18n, layer: PcbLayer, style: LayerNameStyle) -> Localized {
    match (style, layer) {
        (LayerNameStyle::Long, _) => layer_name(i18n, layer),
        (LayerNameStyle::Short, PcbLayer::Copper(CopperLayer::Inner(n))) => {
            t!(i18n, ids::PCB_LAYER_SHORT_INNER, "index" => u32::from(n.index()))
        }
        (LayerNameStyle::Short, PcbLayer::Mechanical(m)) => {
            t!(i18n, ids::PCB_LAYER_SHORT_MECHANICAL, "index" => u32::from(m.index()))
        }
        (LayerNameStyle::Short, _) => {
            t!(i18n, ids::PCB_LAYER_SHORT, "layer" => pcb_layer_token(layer))
        }
        (LayerNameStyle::Medium, PcbLayer::Copper(CopperLayer::Top)) => {
            t!(i18n, ids::PCB_LAYER_MEDIUM_TOP)
        }
        (LayerNameStyle::Medium, PcbLayer::Copper(CopperLayer::Bottom)) => {
            t!(i18n, ids::PCB_LAYER_MEDIUM_BOTTOM)
        }
        (LayerNameStyle::Medium, PcbLayer::Copper(CopperLayer::Inner(n))) => {
            t!(i18n, ids::PCB_LAYER_MEDIUM_INNER, "index" => u32::from(n.index()))
        }
        (LayerNameStyle::Medium, PcbLayer::Mechanical(m)) => {
            t!(i18n, ids::PCB_LAYER_MEDIUM_MECHANICAL, "index" => u32::from(m.index()))
        }
        (LayerNameStyle::Medium, PcbLayer::Technical(_)) => layer_name(i18n, layer),
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

/// The colour `layer` is listed with. Board views use one palette in both
/// themes, so the chrome `tokens` do not change a swatch.
pub fn layer_colour(layer: PcbLayer, _tokens: &ThemeTokens) -> Color {
    board_layer_colour(layer)
}

/// The colour of `layer` in the board palette the canvas draws it in:
/// `ResolvedTheme::board_layer_color`, the one lookup for both.
pub fn board_layer_colour(layer: PcbLayer) -> Color {
    let [r, g, b, a] = ResolvedTheme::from_canvas_colors(crate::styles::board_canvas_colors())
        .board_layer_color(layer);
    Color { r, g, b, a }
}

/// The colour of `layer` in a board view: the colour the view gave it
/// (#2363), at the palette colour's alpha, else the palette's. The Layers
/// panel chip and the board text layer read it; the scene takes the same
/// override through `ResolvedTheme::with_paint`.
pub fn painted_layer_colour(paint: &BoardPaint, layer: PcbLayer) -> Color {
    let palette = board_layer_colour(layer);
    match paint.colour(layer) {
        Some([r, g, b]) => Color {
            a: palette.a,
            ..Color::from_rgb8(r, g, b)
        },
        None => palette,
    }
}

/// The colour the ratsnest (Connection Lines) is drawn in.
pub fn ratsnest_colour() -> Color {
    let [r, g, b, a] = ResolvedTheme::from_canvas_colors(crate::styles::board_canvas_colors())
        .color(ColorSlot::Ratsnest);
    Color { r, g, b, a }
}

/// The colour the board grid is drawn in.
pub fn grid_colour() -> Color {
    crate::render_config::to_iced(&crate::styles::board_canvas_colors().grid)
}

/// The colour the board origin marker is drawn in.
pub fn origin_marker_colour() -> Color {
    crate::render_config::to_iced(&crate::styles::board_canvas_colors().cursor)
}

/// The colour the Multi-Layer row is listed with: the slot vias and
/// through-hole pads are drawn in.
pub fn multi_layer_colour() -> Color {
    let [r, g, b, a] = ResolvedTheme::from_canvas_colors(crate::styles::board_canvas_colors())
        .color(ColorSlot::MultiLayer);
    Color { r, g, b, a }
}

/// How much darker the board's edge is than its mask in 3D.
const BOARD_3D_EDGE_SHADE: f32 = 0.55;
/// How strongly a selected part is washed in the selection colour in 3D.
const BOARD_3D_HIGHLIGHT_ALPHA: f32 = 0.45;

/// The colours of the board 3D view (#2390), from this palette: the solder
/// mask over the faces (opaque), its darker shade on the edge, the copper
/// and Multi-Layer colours, and the 3D Body layer for a part with no colour
/// of its own.
pub fn board_3d_colors() -> signex_renderer::board3d::Board3dColors {
    let rgba = |c: Color| [c.r, c.g, c.b, c.a];
    let mask = Color {
        a: 1.0,
        ..board_layer_colour(PcbLayer::Technical(TechnicalLayer::SolderMaskTop))
    };
    let edge = Color {
        r: mask.r * BOARD_3D_EDGE_SHADE,
        g: mask.g * BOARD_3D_EDGE_SHADE,
        b: mask.b * BOARD_3D_EDGE_SHADE,
        a: 1.0,
    };
    signex_renderer::board3d::Board3dColors {
        mask: rgba(mask),
        edge: rgba(edge),
        top_copper: rgba(board_layer_colour(PcbLayer::Copper(CopperLayer::Top))),
        bottom_copper: rgba(board_layer_colour(PcbLayer::Copper(CopperLayer::Bottom))),
        multi_layer: rgba(multi_layer_colour()),
        body: rgba(board_layer_colour(PcbLayer::Technical(
            TechnicalLayer::Body3dTop,
        ))),
    }
}

/// The wash over a selected part in the board 3D view: the selection slot.
pub fn board_3d_highlight() -> [f32; 4] {
    let [r, g, b, _] = ResolvedTheme::from_canvas_colors(crate::styles::board_canvas_colors())
        .color(ColorSlot::Selection);
    [r, g, b, BOARD_3D_HIGHLIGHT_ALPHA]
}

/// The board 3D view's background: the board canvas's.
pub fn board_3d_background() -> Color {
    let c = crate::styles::board_canvas_colors().background;
    Color::from_rgba8(c.r, c.g, c.b, f32::from(c.a) / 255.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_renderer::pcb::{BOARD_OUTLINE, copper_slot};
    use signex_types::theme::Color as ThemeColor;

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

    /// The house name of every layer kind (decided 2026-11-10; Silkscreen
    /// since 2026-11-18). Courtyard, Fab, Assembly, 3D Body, Fiducial and
    /// Designator keep descriptive names; the edge is the Board Outline row.
    fn house_name(layer: PcbLayer) -> String {
        use TechnicalLayer as T;
        let technical = |t: TechnicalLayer| match t {
            T::EdgeCuts => "Board Outline",
            T::Margin => "Margin",
            T::SilkscreenTop => "Top Silkscreen",
            T::SilkscreenBottom => "Bottom Silkscreen",
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
    fn every_layer_kind_has_its_house_name() {
        let i18n = I18n::en_us();
        let mut seen = std::collections::HashSet::new();
        for layer in PcbLayer::all() {
            // Fluent wraps a placeable ($index) in bidi isolation marks.
            let name: String = layer_name(&i18n, layer)
                .into_string()
                .chars()
                .filter(|c| !matches!(c, '\u{2068}' | '\u{2069}'))
                .collect();
            assert_eq!(name, house_name(layer), "{layer:?}");
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
                object: signex_renderer::pcb::BoardObject::Tracks,
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

        let t = colors.technical;
        let mut palette: Vec<[u8; 4]> = colors.layer_cycle.map(palette_rgba).to_vec();
        for pair in [
            t.adhesive,
            t.fab,
            t.assembly,
            t.courtyard,
            t.body_3d,
            t.fiducial,
            t.designator,
        ] {
            palette.extend([pair.top, pair.bottom].map(palette_rgba));
        }
        palette.extend(
            [
                colors.top_copper,
                colors.bottom_copper,
                colors.margin,
                colors.silkscreen,
                colors.solder_mask,
                colors.paste,
                t.dimensions,
                t.notes,
                t.drill_drawing,
                t.eco_1,
                t.eco_2,
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

    /// Summed per-channel distance, the measure the theme tests use.
    fn apart(a: Color, b: Color) -> u32 {
        let (a, b) = (rgba(a), rgba(b));
        (0..3).map(|i| u32::from(a[i].abs_diff(b[i]))).sum()
    }

    fn luminance(c: Color) -> f32 {
        let lin = |v: f32| {
            if v <= 0.040_45 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
    }

    fn contrast(a: Color, b: Color) -> f32 {
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    fn theme_colour(c: ThemeColor) -> Color {
        Color::from_rgba8(c.r, c.g, c.b, f32::from(c.a) / 255.0)
    }

    fn hue_degrees(c: Color) -> f32 {
        let (max, min) = (c.r.max(c.g).max(c.b), c.r.min(c.g).min(c.b));
        let d = max - min;
        let h = if max == c.r {
            ((c.g - c.b) / d).rem_euclid(6.0)
        } else if max == c.g {
            (c.b - c.r) / d + 2.0
        } else {
            (c.r - c.g) / d + 4.0
        };
        h * 60.0
    }

    fn dark_tokens() -> ThemeTokens {
        signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteDark)
    }

    /// Every technical layer except the outline and keep-outs, which take
    /// their drawn slots.
    fn listed_technical() -> impl Iterator<Item = TechnicalLayer> {
        TechnicalLayer::ALL.into_iter().filter(|t| {
            !matches!(
                t,
                TechnicalLayer::EdgeCuts
                    | TechnicalLayer::KeepoutTop
                    | TechnicalLayer::KeepoutBottom
            )
        })
    }

    /// No two technical layers that can show together share a colour: same
    /// side, or either is board-wide. Copper, outline and keep-out are
    /// kept apart too.
    #[test]
    fn technical_layers_on_one_side_are_told_apart() {
        let tokens = dark_tokens();
        let colour = |t: TechnicalLayer| layer_colour(PcbLayer::Technical(t), &tokens);
        let fixed = [
            PcbLayer::Copper(CopperLayer::Top),
            PcbLayer::Copper(CopperLayer::Bottom),
            BOARD_OUTLINE,
            PcbLayer::Technical(TechnicalLayer::KeepoutTop),
        ];
        for a in listed_technical() {
            for b in listed_technical().filter(|&b| b != a) {
                let together = a.side().is_none() || b.side().is_none() || a.side() == b.side();
                let distance = apart(colour(a), colour(b));
                assert!(
                    !together || distance >= 48,
                    "{a:?} and {b:?} are {distance} apart"
                );
            }
            for f in fixed {
                let distance = apart(colour(a), layer_colour(f, &tokens));
                assert!(distance >= 48, "{a:?} and {f:?} are {distance} apart");
            }
        }
    }

    /// One related pair per kind: the top layer is the brighter.
    #[test]
    fn a_top_technical_layer_is_brighter_than_its_bottom() {
        let tokens = dark_tokens();
        for top in
            listed_technical().filter(|t| t.side() == Some(signex_types::pcb_layer::BoardSide::Top))
        {
            if matches!(
                top,
                TechnicalLayer::SilkscreenTop
                    | TechnicalLayer::SolderMaskTop
                    | TechnicalLayer::PasteTop
            ) {
                continue;
            }
            let t = layer_colour(PcbLayer::Technical(top), &tokens);
            let b = layer_colour(PcbLayer::Technical(top.flipped()), &tokens);
            assert!(luminance(t) > luminance(b), "{top:?}");
            assert!(
                (hue_degrees(t) - hue_degrees(b)).abs() < 10.0,
                "{top:?} pair is not one hue"
            );
        }
    }

    /// The Board Outline is magenta; the keep-out is its own, paler
    /// magenta, clearly apart from the outline.
    #[test]
    fn the_board_outline_is_magenta_and_the_keep_out_is_told_apart() {
        let tokens = dark_tokens();
        let outline = layer_colour(BOARD_OUTLINE, &tokens);
        let keepout = layer_colour(PcbLayer::Technical(TechnicalLayer::KeepoutTop), &tokens);
        for (name, c) in [("outline", outline), ("keep-out", keepout)] {
            let hue = hue_degrees(c);
            assert!((285.0..=330.0).contains(&hue), "{name} hue {hue}");
            assert!(c.r > 0.8 && c.b > 0.8 && c.g < c.r && c.g < c.b, "{name}");
        }
        assert!(apart(outline, keepout) >= 96, "outline and keep-out");
        assert!(luminance(keepout) > luminance(outline), "keep-out is paler");
    }

    /// Every swatch reads on the black board area and on the workspace.
    #[test]
    fn every_board_layer_colour_contrasts_with_the_board_and_workspace() {
        let canvas = crate::styles::board_canvas_colors();
        let grounds = [
            ("board area", theme_colour(canvas.board_area_color())),
            ("workspace", theme_colour(canvas.background)),
        ];
        let tokens = dark_tokens();
        let swatches = PcbLayer::all()
            .map(|l| (format!("{l:?}"), layer_colour(l, &tokens)))
            .chain([("Multi-Layer".to_owned(), multi_layer_colour())]);
        for (layer, swatch) in swatches {
            for (ground, g) in grounds {
                let ratio = contrast(swatch, g);
                assert!(ratio >= 3.0, "{layer} on the {ground}: {ratio:.2}");
            }
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
