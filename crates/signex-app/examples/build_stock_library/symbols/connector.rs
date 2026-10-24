//! Pin-header and screw-terminal stock symbols.
//!
//! Single-row parts stand vertically with pin 1 at the top and the
//! pins on the left. Dual-row parts follow a 2.54 mm box header: odd
//! pins on the left, even pins on the right, pins 1 and 2 on the top
//! row. Pitch is two grid steps; every pin is `Passive`.

use signex_library::primitive::symbol::{PinDirection::Passive, PinOrientation, Symbol, SymbolPin};

use super::{SymbolGraphic, circle, name_hidden, pin, rect, symbol};

/// Row pitch in mm (two schematic grid steps).
const PITCH_MM: f64 = 2.54;

/// All connector symbols, in panel order.
pub fn all() -> Vec<Symbol> {
    let mut out: Vec<Symbol> = [2, 3, 4, 5, 6, 8, 10].into_iter().map(single_row).collect();
    out.extend([5, 10].map(dual_row));
    out.extend([2, 3].map(screw_terminal));
    out
}

/// Y of row `k` out of `rows`, centred on the origin.
fn row_y(k: usize, rows: usize) -> f64 {
    (rows as f64 - 1.0) * PITCH_MM / 2.0 - k as f64 * PITCH_MM
}

fn single_row_pins(n: usize) -> Vec<SymbolPin> {
    (0..n)
        .map(|k| {
            let num = (k + 1).to_string();
            name_hidden(pin(
                &num,
                &format!("Pin_{num}"),
                Passive,
                -5.08,
                row_y(k, n),
                PinOrientation::Right,
            ))
        })
        .collect()
}

fn single_row(n: usize) -> Symbol {
    let half = n as f64 * PITCH_MM / 2.0;
    symbol(
        &format!("Conn_1x{n:02}"),
        "J?",
        &format!("{n}-pin single-row header, 2.54 mm pitch."),
        single_row_pins(n),
        vec![rect([-2.54, half], [1.27, -half])],
    )
}

fn dual_row(rows: usize) -> Symbol {
    let mut pins = Vec::new();
    for k in 0..rows {
        let y = row_y(k, rows);
        for (side, x, orientation) in [
            (0, -7.62, PinOrientation::Right),
            (1, 7.62, PinOrientation::Left),
        ] {
            let num = (2 * k + 1 + side).to_string();
            pins.push(name_hidden(pin(
                &num,
                &format!("Pin_{num}"),
                Passive,
                x,
                y,
                orientation,
            )));
        }
    }
    let half = rows as f64 * PITCH_MM / 2.0;
    symbol(
        &format!("Conn_2x{rows:02}"),
        "J?",
        &format!(
            "{}-pin dual-row header, 2.54 mm pitch; odd pins left, even pins right.",
            rows * 2
        ),
        pins,
        vec![rect([-5.08, half], [5.08, -half])],
    )
}

fn screw_terminal(n: usize) -> Symbol {
    let half = n as f64 * PITCH_MM / 2.0;
    let mut body: Vec<SymbolGraphic> = vec![rect([-2.54, half], [1.27, -half])];
    body.extend((0..n).map(|k| circle([-0.635, row_y(k, n)], 0.635)));
    symbol(
        &format!("Screw_1x{n:02}"),
        "J?",
        &format!("{n}-position screw terminal block."),
        single_row_pins(n),
        body,
    )
}
