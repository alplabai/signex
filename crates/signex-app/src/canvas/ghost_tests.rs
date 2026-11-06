//! #2026: the power-port placement ghost strokes with the sheet's project
//! defaults, so it matches the port a click places.

use super::tests::test_prefs;
use super::*;
use signex_types::schematic::{SchematicDefaults, Symbol};

#[test]
fn the_power_port_ghost_strokes_with_the_project_defaults() {
    let slot = CanvasSlot::new();
    let overrides = Default::default();
    let filters = Default::default();
    let mut prefs = test_prefs(&overrides, &filters);
    prefs.schematic_defaults = SchematicDefaults {
        symbol_line_width_mm: 1.0,
        ..SchematicDefaults::BUILT_IN
    };
    let port = Symbol {
        lib_id: "power:GND".into(),
        is_power: true,
        unit: 1,
        ..Symbol::default()
    };
    let transform = crate::schematic_runtime::ScreenTransform {
        offset_x: 0.0,
        offset_y: 0.0,
        scale: 4.0,
        device_px_per_logical_px: 1.0,
    };
    let ghost = SchematicCanvas::new(&slot, prefs).power_port_ghost(&port, &transform);
    assert!(!ghost.wires.is_empty(), "the ghost draws its glyph");
    assert!(
        ghost.wires.iter().all(|w| (w.width_mm - 1.0).abs() < 1e-6),
        "{:?}",
        ghost.wires.iter().map(|w| w.width_mm).collect::<Vec<_>>()
    );
}
