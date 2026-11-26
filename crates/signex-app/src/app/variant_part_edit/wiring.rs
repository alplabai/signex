//! Which pin an alternate part would leave unwired (mockup row 208): the
//! placed part's pins that carry a wire, read from the sheet's netlist in the
//! candidates' read, against the candidate's pins. Pure.

use signex_types::net::Netlist;
use uuid::Uuid;

/// A pin the compare strip names when the pins differ.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinProblem {
    /// A wired pin of the placed part the candidate does not have.
    LosesWire { pin: String, name: String },
    /// A pin of the candidate no wire reaches on the sheet.
    NoWire { pin: String, name: String },
}

/// The pins (number, or name for a pin without one) of the part made of
/// `units` that sit on a net with a wire or another pin, sorted, each once.
pub fn wired_pins(netlist: &Netlist, units: &[Uuid]) -> Vec<String> {
    let mut pins: Vec<String> = netlist
        .nets
        .iter()
        .filter(|net| !net.wires.is_empty() || net.terminals.len() > 1)
        .flat_map(|net| net.terminals.iter())
        .filter(|terminal| units.contains(&terminal.symbol))
        .map(|terminal| terminal.pin.clone())
        .collect();
    pins.sort();
    pins.dedup();
    pins
}

/// How a pin is named on a net: its number, or its name without one.
fn key(pin: &(String, String)) -> &str {
    if pin.0.is_empty() {
        pin.1.as_str()
    } else {
        pin.0.as_str()
    }
}

/// The first pin that would change its wiring if `candidate` replaced the
/// part with `base` pins of which `wired` carry a wire: a wired pin the
/// candidate lacks first, then a candidate pin no wire reaches.
pub fn pin_problem(
    base: &[(String, String)],
    wired: &[String],
    candidate: &[(String, String)],
) -> Option<PinProblem> {
    let has = |pins: &[(String, String)], wanted: &str| pins.iter().any(|pin| key(pin) == wanted);
    let is_wired = |pin: &(String, String)| wired.iter().any(|w| w == key(pin));
    if let Some(lost) = base
        .iter()
        .find(|&pin| is_wired(pin) && !has(candidate, key(pin)))
    {
        return Some(PinProblem::LosesWire {
            pin: key(lost).to_string(),
            name: lost.1.clone(),
        });
    }
    candidate
        .iter()
        .find(|&pin| !is_wired(pin))
        .map(|pin| PinProblem::NoWire {
            pin: key(pin).to_string(),
            name: pin.1.clone(),
        })
}
