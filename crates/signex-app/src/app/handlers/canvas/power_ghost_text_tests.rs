//! The power-port placement preview draws the net name the way the placed
//! port does: same offset from the port, same turn, in the preview colour.

use iced::{Color, Rectangle};
use signex_types::theme::{ThemeId, canvas_colors};

use super::super::super::*;
use crate::active_bar::{ActiveBarAction, ActiveBarMsg};
use crate::schematic_runtime::{
    SceneOverrides, ScreenTransform, build_renderer_snapshot, power_port_preview_snapshot,
};

const PREVIEW: Color = Color::from_rgba(0.3, 0.8, 1.0, 0.7);

fn transform() -> ScreenTransform {
    ScreenTransform {
        offset_x: 0.0,
        offset_y: 0.0,
        scale: 4.0,
        device_px_per_logical_px: signex_types::schematic::UNSCALED_DEVICE_PX_PER_LOGICAL_PX,
    }
}

/// The armed power-port ghost, put where the cursor snaps to.
fn armed_ghost(app: &mut Signex, at: (f64, f64)) -> signex_types::schematic::Symbol {
    let _task = app.update(Message::ActiveBar(ActiveBarMsg::Action(
        ActiveBarAction::PlacePowerVCC,
    )));
    let mut ghost = app
        .interaction_state
        .active_canvas()
        .ghost_symbol
        .clone()
        .expect("the power-port tool arms a ghost");
    ghost.position = signex_types::schematic::Point::new(at.0, at.1);
    ghost
}

/// Net-name texts of a sheet that holds `port`, relative to the port.
fn placed_texts(port: &signex_types::schematic::Symbol) -> Vec<([f32; 2], f32, String)> {
    let mut sheet = signex_types::schematic::SchematicSheet::default();
    sheet.symbols.push(port.clone());
    let snap = build_renderer_snapshot(
        &sheet,
        &transform(),
        &canvas_colors(ThemeId::GraphiteLight),
        Rectangle::new(iced::Point::ORIGIN, iced::Size::new(2000.0, 2000.0)),
        None,
        SceneOverrides::default(),
    );
    snap.reference_value_texts
        .iter()
        .map(|t| (t.position, t.rotation_rad, t.content.clone()))
        .collect()
}

#[test]
fn the_power_port_preview_draws_its_net_name_like_a_placed_port() {
    let (mut app, _boot) = Signex::new();
    for (rotation, mirror_y) in [(0.0, false), (90.0, false), (180.0, true)] {
        let mut ghost = armed_ghost(&mut app, (12.7, 25.4));
        ghost.rotation = rotation;
        ghost.mirror_y = mirror_y;
        assert_eq!(ghost.value, "VCC");

        let preview = power_port_preview_snapshot(
            &ghost,
            &transform(),
            PREVIEW,
            &signex_types::schematic::SchematicDefaults::BUILT_IN,
        );
        let name: Vec<_> = preview
            .parameter_texts
            .iter()
            .filter(|t| t.content == "VCC")
            .collect();
        assert_eq!(name.len(), 1, "rotation {rotation}: one net name");

        let placed = placed_texts(&ghost);
        assert_eq!(placed.len(), 1, "rotation {rotation}: {placed:?}");
        assert_eq!(name[0].position, placed[0].0, "rotation {rotation}");
        assert_eq!(name[0].rotation_rad, placed[0].1, "rotation {rotation}");
        assert_eq!(
            name[0].color,
            [PREVIEW.r, PREVIEW.g, PREVIEW.b, PREVIEW.a],
            "the preview colour"
        );
    }
}
