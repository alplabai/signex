//! Resolved theme model for renderer-side scene emission.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

use serde::Deserialize;
use signex_gfx::style::ColorSlot;
use signex_types::theme::{canvas_colors, CanvasColors, Color, ThemeId};
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
        resolved.set_slot(ColorSlot::ErcError, to_rgba(canvas.no_connect));
        resolved.set_slot(ColorSlot::ErcWarning, to_rgba(canvas.power));
        resolved.set_slot(ColorSlot::ErcInfo, to_rgba(canvas.net_label));
        resolved.set_slot(ColorSlot::Ghost, to_rgba(canvas.body_fill));
        resolved.set_slot(ColorSlot::LassoStroke, to_rgba(canvas.selection));
        resolved.set_slot(ColorSlot::LassoFill, to_rgba(canvas.selection));

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
    ghost: [f32; 4],
    lasso_stroke: [f32; 4],
    lasso_fill: [f32; 4],
}

fn load_builtin_theme() -> ResolvedTheme {
    let palette: BuiltinPaletteFile = serde_json::from_str(include_str!("../data/builtin_schematic_palette.json"))
        .expect("valid builtin_schematic_palette.json");

    let mut resolved = ResolvedTheme::empty();
    resolved.set_slot(ColorSlot::Wire, palette.slots.wire);
    resolved.set_slot(ColorSlot::Bus, palette.slots.bus);
    resolved.set_slot(ColorSlot::Junction, palette.slots.junction);
    resolved.set_slot(ColorSlot::SymbolBody, palette.slots.symbol_body);
    resolved.set_slot(ColorSlot::Pin, palette.slots.pin);
    resolved.set_slot(ColorSlot::Selection, palette.slots.selection);
    resolved.set_slot(ColorSlot::Grid, palette.slots.grid);
    resolved.set_slot(ColorSlot::Snap, palette.slots.snap);
    resolved.set_slot(ColorSlot::ErcError, palette.slots.erc_error);
    resolved.set_slot(ColorSlot::ErcWarning, palette.slots.erc_warning);
    resolved.set_slot(ColorSlot::ErcInfo, palette.slots.erc_info);
    resolved.set_slot(ColorSlot::Ghost, palette.slots.ghost);
    resolved.set_slot(ColorSlot::LassoStroke, palette.slots.lasso_stroke);
    resolved.set_slot(ColorSlot::LassoFill, palette.slots.lasso_fill);

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
    use signex_types::theme::ThemeId;

    #[test]
    fn builtin_theme_exposes_wire_and_erc_slots() {
        let theme = ResolvedTheme::builtin_default();
        assert_eq!(theme.color(ColorSlot::Wire), [0.2, 0.2, 0.9, 1.0]);
        assert_eq!(theme.color(ColorSlot::ErcError), [0.93, 0.29, 0.31, 1.0]);
    }

    #[test]
    fn theme_id_conversion_maps_canvas_colors() {
        let from_theme_id = ResolvedTheme::from_theme_id(ThemeId::Signex);

        let wire = from_theme_id.color(ColorSlot::Wire);
        assert_eq!(wire[0], 0.0);
        assert_eq!(wire[1], 0.0);
        assert!(wire[2] > 0.49);
        assert_eq!(wire[3], 1.0);
    }
}
