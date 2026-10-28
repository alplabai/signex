//! Placing a power port with its own library entry (#1408).

use crate::Command;
use signex_types::schematic::{Symbol, power_port_lib_symbol};
use uuid::Uuid;

/// The commands that place the power port `symbol`: register the built-in
/// library entry for its style (keyed by `lib_id`, kept if the sheet already
/// has one) and place the instance. Run them as one batch so the pair is a
/// single undo step.
#[must_use]
pub fn place_power_port(symbol: Symbol) -> Vec<Command> {
    vec![
        Command::RegisterLibSymbol {
            lib_symbol: power_port_lib_symbol(&symbol.lib_id),
        },
        Command::PlaceSymbol { symbol },
    ]
}

/// The commands that re-style the power port `symbol_id` to `new_lib_id` and
/// turn it to `rotation_degrees`: register the new style's entry, re-point the
/// port at it and rotate it. Run them as one batch so the change is a single
/// undo step.
#[must_use]
pub fn restyle_power_port(
    symbol_id: Uuid,
    new_lib_id: &str,
    rotation_degrees: f64,
) -> Vec<Command> {
    vec![
        Command::RegisterLibSymbol {
            lib_symbol: power_port_lib_symbol(new_lib_id),
        },
        Command::UpdateSymbolLibId {
            symbol_id,
            lib_id: new_lib_id.to_owned(),
        },
        Command::SetSymbolRotation {
            symbol_id,
            rotation_degrees,
        },
    ]
}
