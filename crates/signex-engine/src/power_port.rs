//! Placing a power port with its own library entry (#1408).

use crate::Command;
use signex_types::schematic::{Symbol, power_port_lib_symbol};

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
