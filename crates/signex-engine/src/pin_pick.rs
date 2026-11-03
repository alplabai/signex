//! Which pin of a placed symbol a point picks.

use signex_types::schematic::{
    PIN_LENGTH_MM, Point, SchematicSheet, SymbolTransform, instance_pins, pin_direction,
};
use uuid::Uuid;

/// How close to a pin's line or end a point must be to pick that pin (mm):
/// half the default grid. Anywhere else inside the body picks the part.
pub const PIN_PICK_TOLERANCE_MM: f64 = 1.27;

/// The key (number, else name) of the pin of `symbol` nearest `at` within
/// [`PIN_PICK_TOLERANCE_MM`] of its line or end, if any.
#[must_use]
pub fn symbol_pin_at(document: &SchematicSheet, symbol: Uuid, at: Point) -> Option<String> {
    let sym = document.symbols.iter().find(|s| s.uuid == symbol)?;
    let lib = document.resolve_lib_symbol(sym)?;
    let xform = SymbolTransform::from_symbol(sym);
    instance_pins(&lib, sym)
        .filter_map(|lp| {
            let pin = &lp.pin;
            let (dx, dy) = pin_direction(pin.rotation);
            let len = if pin.length > 0.0 {
                pin.length
            } else {
                PIN_LENGTH_MM
            };
            let tip = xform.apply(pin.position);
            let inner = xform.apply(Point::new(
                pin.position.x + dx * len,
                pin.position.y + dy * len,
            ));
            let distance = distance_to_segment(at, tip, inner);
            (distance <= PIN_PICK_TOLERANCE_MM).then(|| {
                let key = if pin.number.is_empty() {
                    pin.name.clone()
                } else {
                    pin.number.clone()
                };
                (distance, key)
            })
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, key)| key)
}

fn distance_to_segment(p: Point, a: Point, b: Point) -> f64 {
    let (abx, aby) = (b.x - a.x, b.y - a.y);
    let len_sq = abx * abx + aby * aby;
    let t = if len_sq > 0.0 {
        (((p.x - a.x) * abx + (p.y - a.y) * aby) / len_sq).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p.x - (a.x + abx * t)).hypot(p.y - (a.y + aby * t))
}
