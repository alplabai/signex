//! Resolved theme model for renderer-side scene emission.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

use serde::Deserialize;
use signex_gfx::style::ColorSlot;
use signex_types::theme::{CanvasColors, Color, ThemeId, canvas_colors};
use std::sync::OnceLock;

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedTheme {
    slots: [[f32; 4]; 32],
}

impl ResolvedTheme {
    pub fn builtin_default() -> Self {
        static BUILTIN: OnceLock<ResolvedTheme> = OnceLock::new();
        BUILTIN.get_or_init(load_builtin_theme).clone()
    }

    pub fn from_theme_id(theme_id: ThemeId) -> Self {
        Self::from_canvas_colors(canvas_colors(theme_id))
    }

    pub fn from_canvas_colors(canvas: CanvasColors) -> Self {
        let mut resolved = Self::empty();

        resolved.set_slot(ColorSlot::Wire, to_rgba(canvas.wire));
        resolved.set_slot(ColorSlot::Bus, to_rgba(canvas.bus));
        resolved.set_slot(ColorSlot::Junction, to_rgba(canvas.junction));
        resolved.set_slot(ColorSlot::SymbolBody, to_rgba(canvas.body));
        resolved.set_slot(ColorSlot::Pin, to_rgba(canvas.pin));
        resolved.set_slot(ColorSlot::Selection, to_rgba(canvas.selection));
        resolved.set_slot(ColorSlot::Grid, to_rgba(canvas.grid));
        resolved.set_slot(ColorSlot::Snap, to_rgba(canvas.cursor));
        resolved.set_slot(ColorSlot::ErcError, to_rgba(canvas.erc_error));
        resolved.set_slot(ColorSlot::ErcWarning, to_rgba(canvas.erc_warning));
        resolved.set_slot(ColorSlot::DrcError, to_rgba(canvas.drc_error));
        resolved.set_slot(ColorSlot::DrcWarning, to_rgba(canvas.drc_warning));
        resolved.set_slot(ColorSlot::ErcInfo, to_rgba(canvas.net_label));
        resolved.set_slot(ColorSlot::Ghost, to_rgba(canvas.body_fill));
        resolved.set_slot(ColorSlot::LassoStroke, to_rgba(canvas.selection));
        resolved.set_slot(ColorSlot::LassoFill, to_rgba(canvas.selection));
        resolved.set_slot(ColorSlot::Ratsnest, to_rgba(canvas.ratsnest_color()));
        // The board edge is the canvas's strongest ink; an opening shows the
        // canvas under the board; a keepout is where a DRC rule bites.
        resolved.set_slot(ColorSlot::BoardEdge, to_rgba(canvas.board_edge_color()));
        resolved.set_slot(ColorSlot::BoardOpening, to_rgba(canvas.background));
        resolved.set_slot(ColorSlot::Keepout, to_rgba(canvas.keepout_color()));

        resolved
    }

    pub fn with_slot(mut self, slot: ColorSlot, color: [f32; 4]) -> Self {
        self.set_slot(slot, color);
        self
    }

    pub fn color(&self, slot: ColorSlot) -> [f32; 4] {
        self.slots[slot as usize]
    }

    pub fn set_slot(&mut self, slot: ColorSlot, color: [f32; 4]) {
        self.slots[slot as usize] = color;
    }

    fn empty() -> Self {
        Self {
            slots: [[0.0; 4]; 32],
        }
    }
}

#[derive(Debug, Deserialize)]
struct BuiltinPaletteFile {
    slots: BuiltinSlots,
}

#[derive(Debug, Deserialize)]
struct BuiltinSlots {
    wire: [f32; 4],
    bus: [f32; 4],
    junction: [f32; 4],
    symbol_body: [f32; 4],
    pin: [f32; 4],
    selection: [f32; 4],
    grid: [f32; 4],
    snap: [f32; 4],
    erc_error: [f32; 4],
    erc_warning: [f32; 4],
    erc_info: [f32; 4],
    drc_error: [f32; 4],
    drc_warning: [f32; 4],
    ghost: [f32; 4],
    lasso_stroke: [f32; 4],
    lasso_fill: [f32; 4],
}

fn load_builtin_theme() -> ResolvedTheme {
    let palette: BuiltinPaletteFile =
        serde_json::from_str(include_str!("../data/builtin_schematic_palette.json"))
            .expect("valid builtin_schematic_palette.json");

    let mut resolved = ResolvedTheme::empty();
    resolved.set_slot(ColorSlot::Wire, palette.slots.wire);
    resolved.set_slot(ColorSlot::Bus, palette.slots.bus);
    resolved.set_slot(ColorSlot::Junction, palette.slots.junction);
    resolved.set_slot(ColorSlot::SymbolBody, palette.slots.symbol_body);
    resolved.set_slot(ColorSlot::Pin, palette.slots.pin);
    resolved.set_slot(ColorSlot::Selection, palette.slots.selection);
    resolved.set_slot(ColorSlot::Ratsnest, palette.slots.selection);
    resolved.set_slot(ColorSlot::Grid, palette.slots.grid);
    resolved.set_slot(ColorSlot::Snap, palette.slots.snap);
    resolved.set_slot(ColorSlot::ErcError, palette.slots.erc_error);
    resolved.set_slot(ColorSlot::ErcWarning, palette.slots.erc_warning);
    resolved.set_slot(ColorSlot::ErcInfo, palette.slots.erc_info);
    resolved.set_slot(ColorSlot::DrcError, palette.slots.drc_error);
    resolved.set_slot(ColorSlot::DrcWarning, palette.slots.drc_warning);
    resolved.set_slot(ColorSlot::Ghost, palette.slots.ghost);
    resolved.set_slot(ColorSlot::LassoStroke, palette.slots.lasso_stroke);
    resolved.set_slot(ColorSlot::LassoFill, palette.slots.lasso_fill);
    resolved.set_slot(ColorSlot::BoardEdge, palette.slots.snap);
    resolved.set_slot(ColorSlot::BoardOpening, palette.slots.pin);
    resolved.set_slot(ColorSlot::Keepout, palette.slots.drc_warning);

    resolved
}

fn to_rgba(color: Color) -> [f32; 4] {
    [
        f32::from(color.r) / 255.0,
        f32::from(color.g) / 255.0,
        f32::from(color.b) / 255.0,
        f32::from(color.a) / 255.0,
    ]
}

#[cfg(test)]
mod tests {
    use super::ResolvedTheme;
    use signex_gfx::style::ColorSlot;
    use signex_types::theme::{Color, ThemeId, canvas_colors};

    #[test]
    fn builtin_theme_exposes_wire_and_erc_slots() {
        let theme = ResolvedTheme::builtin_default();
        assert_eq!(theme.color(ColorSlot::Wire), [0.2, 0.2, 0.9, 1.0]);
        assert_eq!(theme.color(ColorSlot::ErcError), [0.93, 0.29, 0.31, 1.0]);
    }

    #[test]
    fn theme_id_conversion_maps_canvas_colors() {
        let from_theme_id = ResolvedTheme::from_theme_id(ThemeId::GraphiteLight);
        let px = |v: u8| f32::from(v) / 255.0;

        let wire = from_theme_id.color(ColorSlot::Wire);
        assert_eq!(wire, [px(0x2B), px(0x2D), px(0x33), 1.0]);
    }

    #[test]
    fn the_ratsnest_has_its_own_slot_apart_from_the_selection() {
        let theme = ResolvedTheme::from_theme_id(ThemeId::GraphiteLight);
        let px = |v: u8| f32::from(v) / 255.0;
        assert_eq!(theme.color(ColorSlot::Ratsnest)[2], px(0xAE));
        assert_ne!(
            theme.color(ColorSlot::Ratsnest),
            theme.color(ColorSlot::Selection)
        );
        assert_eq!(
            ResolvedTheme::builtin_default().color(ColorSlot::Ratsnest),
            ResolvedTheme::builtin_default().color(ColorSlot::Selection)
        );
    }

    #[test]
    fn board_features_take_their_colours_from_each_graphite_canvas() {
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let canvas = canvas_colors(id);
            let theme = ResolvedTheme::from_canvas_colors(canvas);
            let rgba = super::to_rgba;
            assert_eq!(
                theme.color(ColorSlot::BoardEdge),
                rgba(canvas.board_edge_color())
            );
            assert_eq!(
                theme.color(ColorSlot::BoardOpening),
                rgba(canvas.background)
            );
            assert_eq!(
                theme.color(ColorSlot::Keepout),
                rgba(canvas.keepout_color())
            );
            assert_ne!(
                theme.color(ColorSlot::Keepout),
                rgba(canvas.drc_warning),
                "{id:?}: a keepout is not a DRC marker"
            );
            assert_ne!(
                theme.color(ColorSlot::BoardEdge),
                theme.color(ColorSlot::BoardOpening),
                "{id:?}: the board edge shows against an opening"
            );
        }
    }

    #[test]
    fn markers_use_their_own_theme_slots() {
        let mut canvas = canvas_colors(ThemeId::GraphiteLight);
        canvas.erc_error = Color::new(1, 2, 3, 255);
        canvas.erc_warning = Color::new(4, 5, 6, 255);
        canvas.drc_error = Color::new(7, 8, 9, 255);
        canvas.drc_warning = Color::new(10, 11, 12, 255);
        let theme = ResolvedTheme::from_canvas_colors(canvas);
        let px = |v: u8| f32::from(v) / 255.0;
        assert_eq!(theme.color(ColorSlot::ErcError)[0], px(1));
        assert_eq!(theme.color(ColorSlot::ErcWarning)[0], px(4));
        assert_eq!(theme.color(ColorSlot::DrcError)[0], px(7));
        assert_eq!(theme.color(ColorSlot::DrcWarning)[0], px(10));
    }
}
