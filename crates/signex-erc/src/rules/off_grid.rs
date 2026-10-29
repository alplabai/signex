//! Rule: OffGrid (#1286). A pin tip or wire endpoint off the connection grid
//! will not meet a wire drawn with grid snap on. Tested in the shared 1 µm key
//! space (`pt_key`), so a float rounding residue is never "off grid".

use signex_net::pt_key;
use signex_types::schematic::{Point, SelectedKind};

use crate::context::ErcContext;
use crate::diagnostic::Diagnostic;
use crate::{RuleKind, sel};

/// The connection grid in `pt_key` units (µm): 1.27 mm, 50 mil, half the
/// coarse pin grid (`signex_types::schematic::GRID_MM`).
pub(crate) const CONNECTION_GRID_UM: i64 = 1270;

fn is_off_grid(p: &Point) -> bool {
    let (x, y) = pt_key(p);
    x.rem_euclid(CONNECTION_GRID_UM) != 0 || y.rem_euclid(CONNECTION_GRID_UM) != 0
}

/// One finding per symbol (at its first off-grid pin) and per wire (at its
/// first off-grid end), so a part dropped off-grid is one row, not N.
pub(crate) fn off_grid(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    for symbol in ctx.symbols.iter() {
        let off: Vec<&Point> = symbol
            .pins
            .iter()
            .map(|p| &p.world_pos)
            .filter(|p| is_off_grid(p))
            .collect();
        let Some(first) = off.first() else {
            continue;
        };
        let reference = match symbol.reference.trim() {
            "" => "(unnamed)",
            r => r,
        };
        out.push(
            Diagnostic::new(
                RuleKind::OffGrid,
                format!(
                    "{} pin(s) of {reference} are off the 1.27 mm connection grid",
                    off.len()
                ),
                **first,
            )
            .with_primary(sel(symbol.uuid, SelectedKind::Symbol)),
        );
    }
    for wire in &ctx.wires {
        let Some(end) = [wire.start, wire.end].into_iter().find(is_off_grid) else {
            continue;
        };
        out.push(
            Diagnostic::new(
                RuleKind::OffGrid,
                format!(
                    "Wire end at ({:.3}, {:.3}) mm is off the 1.27 mm connection grid",
                    end.x, end.y
                ),
                end,
            )
            .with_primary(sel(wire.uuid, SelectedKind::Wire)),
        );
    }
}
