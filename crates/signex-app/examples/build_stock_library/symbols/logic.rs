//! 74HC logic stock symbols: one whole-package symbol per part, in the
//! standard 14-pin DIP/SOIC pinout. Inputs on the left, outputs on the
//! right, VCC (pin 14) on top and GND (pin 7) at the bottom.

use signex_library::primitive::symbol::{
    PinDirection::{Input, Output, Power},
    PinOrientation::{Down, Left, Right, Up},
    Symbol, SymbolPin,
};

use super::{pin, rect, symbol};

/// One gate: input pin numbers and the output pin number.
type Gate = (&'static [u8], u8);

/// Quad 2-input gates 74HC00/08/32/86: 1A 1B 1Y 2A 2B 2Y GND 3Y 3A 3B 4Y 4A 4B VCC.
const QUAD: [Gate; 4] = [(&[1, 2], 3), (&[4, 5], 6), (&[9, 10], 8), (&[12, 13], 11)];

/// Quad NOR 74HC02: outputs lead each gate on pins 1, 4, 10, 13.
const QUAD_NOR: [Gate; 4] = [(&[2, 3], 1), (&[5, 6], 4), (&[8, 9], 10), (&[11, 12], 13)];

/// Hex inverters 74HC04/14: A on 1,3,5,9,11,13; Y on 2,4,6,8,10,12.
const HEX: [Gate; 6] = [
    (&[1], 2),
    (&[3], 4),
    (&[5], 6),
    (&[9], 8),
    (&[11], 10),
    (&[13], 12),
];

/// All 74HC symbols, in panel order.
pub fn all() -> Vec<Symbol> {
    vec![
        package("74HC00", "Quad 2-input NAND gate", &QUAD),
        package("74HC02", "Quad 2-input NOR gate", &QUAD_NOR),
        package("74HC04", "Hex inverter", &HEX),
        package("74HC08", "Quad 2-input AND gate", &QUAD),
        package("74HC32", "Quad 2-input OR gate", &QUAD),
        package("74HC86", "Quad 2-input XOR gate", &QUAD),
        package("74HC14", "Hex Schmitt-trigger inverter", &HEX),
    ]
}

fn package(name: &str, what: &str, gates: &[Gate]) -> Symbol {
    let n = gates.len();
    let k = gates[0].0.len();
    let pitch = 2.54 * k as f64;
    let half = n as f64 * pitch / 2.0;
    let mut pins: Vec<SymbolPin> = Vec::new();
    for (g, (inputs, out)) in gates.iter().enumerate() {
        let c = (n as f64 - 1.0) * pitch / 2.0 - g as f64 * pitch;
        let gate = g + 1;
        for (i, num) in inputs.iter().enumerate() {
            let y = if k == 2 {
                c + 1.27 - 2.54 * i as f64
            } else {
                c
            };
            let label = format!("{gate}{}", ["A", "B"][i]);
            pins.push(pin(&num.to_string(), &label, Input, -7.62, y, Right));
        }
        pins.push(pin(
            &out.to_string(),
            &format!("{gate}Y"),
            Output,
            7.62,
            c,
            Left,
        ));
    }
    pins.push(pin("14", "VCC", Power, 0.0, half + 2.54, Down));
    pins.push(pin("7", "GND", Power, 0.0, -half - 2.54, Up));
    pins.sort_by_key(|p| p.number.parse::<u8>().unwrap_or(0));
    symbol(
        name,
        "U?",
        &format!("{what}, 14-pin DIP/SOIC."),
        pins,
        vec![rect([-5.08, half], [5.08, -half])],
    )
}
