//! Passive and discrete stock symbols: R, C, L, ferrite, fuse,
//! diodes, LED, bipolar and MOS transistors, crystal, the power
//! ports and a generic IC body.
//!
//! Two-terminal parts stand vertically: the top pin connects at
//! `(0, 3.81)`, the bottom at `(0, -3.81)`, each with a 2.54 mm stub,
//! so the body occupies `y` in `-1.27..=1.27`. Transistors have the
//! control pin on the left and the two power terminals on the right.
//! Pin positions are multiples of the 1.27 mm schematic grid.
//!
//! Power ports are ordinary symbols whose single pin is
//! `PinDirection::Power`, named after the net, with designator `#PWR?`
//! and the net name as comment. Library placement does not yet turn
//! them into power ports (`is_power`, `value` = net name), so ERC and
//! net naming don't see them; tracked in #928.

use signex_library::primitive::symbol::{PinDirection, PinOrientation, Symbol, SymbolPin};

use super::{
    arc, arrow_head, circle, labels_hidden, line, name_hidden, pin, polygon, rect, symbol,
    with_length,
};

use PinDirection::{Passive, Power, Unspecified};
use PinOrientation::{Down, Left, Right, Up};

/// All passive/discrete symbols, in panel order.
pub fn all() -> Vec<Symbol> {
    vec![
        resistor(),
        capacitor(),
        capacitor_polarized(),
        inductor(),
        ferrite(),
        fuse(),
        diode(),
        diode_schottky(),
        diode_zener(),
        led(),
        transistor_bjt("Q_NPN", true),
        transistor_bjt("Q_PNP", false),
        transistor_mos("Q_NMOS", true),
        transistor_mos("Q_PMOS", false),
        crystal(),
        power_port("GND"),
        power_port("VCC"),
        power_port("+3V3"),
        power_port("+5V"),
        ic_generic(),
    ]
}

/// Vertical two-terminal part. `top` / `bottom` are `(number, name)`.
fn two_terminal(
    name: &str,
    designator: &str,
    description: &str,
    top: (&str, &str),
    bottom: (&str, &str),
    body: Vec<super::SymbolGraphic>,
) -> Symbol {
    let pins = vec![
        name_hidden(pin(top.0, top.1, Passive, 0.0, 3.81, Down)),
        name_hidden(pin(bottom.0, bottom.1, Passive, 0.0, -3.81, Up)),
    ];
    symbol(name, designator, description, pins, body)
}

fn plain(
    name: &str,
    designator: &str,
    description: &str,
    body: Vec<super::SymbolGraphic>,
) -> Symbol {
    two_terminal(name, designator, description, ("1", "1"), ("2", "2"), body)
}

fn resistor() -> Symbol {
    plain(
        "R",
        "R?",
        "Resistor (IEC rectangle).",
        vec![rect([-1.016, 1.27], [1.016, -1.27])],
    )
}

/// Two parallel plates 1.016 mm apart with short leads to the pins.
fn plates() -> Vec<super::SymbolGraphic> {
    vec![
        line([0.0, 1.27], [0.0, 0.508]),
        line([-1.524, 0.508], [1.524, 0.508]),
        line([-1.524, -0.508], [1.524, -0.508]),
        line([0.0, -0.508], [0.0, -1.27]),
    ]
}

fn capacitor() -> Symbol {
    plain("C", "C?", "Capacitor, non-polarised.", plates())
}

fn capacitor_polarized() -> Symbol {
    let mut body = plates();
    // Plus sign beside the positive (top) plate.
    body.push(line([-2.286, 1.016], [-1.524, 1.016]));
    body.push(line([-1.905, 1.397], [-1.905, 0.635]));
    plain(
        "C_Polarized",
        "C?",
        "Polarised capacitor; pin 1 is the positive plate.",
        body,
    )
}

fn inductor() -> Symbol {
    // Four humps bulging to the right of the axis, 0.635 mm each.
    let body = [0.9525, 0.3175, -0.3175, -0.9525]
        .iter()
        .map(|&cy| arc([0.0, cy], 0.3175, 270.0, 90.0))
        .collect();
    plain("L", "L?", "Inductor (four-hump coil).", body)
}

fn ferrite() -> Symbol {
    plain(
        "Ferrite",
        "FB?",
        "Ferrite bead: a rectangle struck through by a diagonal.",
        vec![
            rect([-1.016, 1.27], [1.016, -1.27]),
            line([-1.016, -1.27], [1.016, 1.27]),
        ],
    )
}

fn fuse() -> Symbol {
    plain(
        "Fuse",
        "F?",
        "Fuse: a rectangle with the element drawn along its axis.",
        vec![
            rect([-1.016, 1.27], [1.016, -1.27]),
            line([0.0, 1.27], [0.0, -1.27]),
        ],
    )
}

/// Anode on top: filled-direction triangle pointing down at the
/// cathode bar. Pin 1 = cathode, pin 2 = anode (common pad order).
fn diode_pins_and_triangle() -> (Vec<SymbolPin>, Vec<super::SymbolGraphic>) {
    (
        vec![
            name_hidden(pin("2", "A", Passive, 0.0, 3.81, Down)),
            name_hidden(pin("1", "K", Passive, 0.0, -3.81, Up)),
        ],
        vec![
            polygon(vec![[-1.27, 1.27], [1.27, 1.27], [0.0, -1.27]]),
            line([-1.27, -1.27], [1.27, -1.27]),
        ],
    )
}

fn diode() -> Symbol {
    let (pins, body) = diode_pins_and_triangle();
    symbol("D", "D?", "Diode; pin 1 cathode, pin 2 anode.", pins, body)
}

fn diode_schottky() -> Symbol {
    let (pins, mut body) = diode_pins_and_triangle();
    // Squared S-hooks on the cathode bar.
    body.push(line([-1.27, -1.27], [-1.27, -0.762]));
    body.push(line([1.27, -1.27], [1.27, -1.778]));
    symbol(
        "D_Schottky",
        "D?",
        "Schottky diode; pin 1 cathode, pin 2 anode.",
        pins,
        body,
    )
}

fn diode_zener() -> Symbol {
    let (pins, mut body) = diode_pins_and_triangle();
    // Angled Z-hooks on the cathode bar.
    body.push(line([-1.27, -1.27], [-1.778, -0.762]));
    body.push(line([1.27, -1.27], [1.778, -1.778]));
    symbol(
        "D_Zener",
        "D?",
        "Zener diode; pin 1 cathode, pin 2 anode.",
        pins,
        body,
    )
}

fn led() -> Symbol {
    let (pins, mut body) = diode_pins_and_triangle();
    // Two emission arrows leaving the junction toward the upper right.
    for dy in [0.0, -1.016] {
        let (from, tip) = ([1.524, 0.508 + dy], [2.794, 1.778 + dy]);
        body.push(line(from, tip));
        body.push(arrow_head(tip, from, 0.635, 0.2));
    }
    symbol(
        "LED",
        "D?",
        "Light-emitting diode; pin 1 cathode, pin 2 anode.",
        pins,
        body,
    )
}

/// Bipolar transistor, base left, collector up-right, emitter
/// down-right. Pins follow the common SOT-23 order: 1 B, 2 E, 3 C.
fn transistor_bjt(name: &str, is_npn: bool) -> Symbol {
    let pins = vec![
        pin("1", "B", Passive, -5.08, 0.0, Right),
        pin("2", "E", Passive, 2.54, -5.08, Up),
        pin("3", "C", Passive, 2.54, 5.08, Down),
    ];
    let mut body = vec![
        line([-2.54, 0.0], [-1.27, 0.0]),
        line([-1.27, 1.905], [-1.27, -1.905]),
        line([-1.27, 0.635], [2.54, 2.54]),
        line([-1.27, -0.635], [2.54, -2.54]),
    ];
    // Emitter arrow: out of the base for NPN, into it for PNP.
    body.push(if is_npn {
        arrow_head([1.905, -2.2225], [0.9525, -1.7463], 0.9, 0.3)
    } else {
        arrow_head([-0.635, -0.9525], [0.9525, -1.7463], 0.9, 0.3)
    });
    let desc = if is_npn {
        "NPN bipolar transistor; pins 1 B, 2 E, 3 C."
    } else {
        "PNP bipolar transistor; pins 1 B, 2 E, 3 C."
    };
    symbol(name, "Q?", desc, pins, body)
}

/// Enhancement MOSFET, gate left, drain up-right, source down-right.
/// Pins follow the common SOT-23 order: 1 G, 2 S, 3 D.
fn transistor_mos(name: &str, is_n_channel: bool) -> Symbol {
    let pins = vec![
        pin("1", "G", Passive, -5.08, 0.0, Right),
        pin("2", "S", Passive, 2.54, -5.08, Up),
        pin("3", "D", Passive, 2.54, 5.08, Down),
    ];
    let mut body = vec![
        // Gate lead, gate plate, three-segment channel.
        line([-2.54, 0.0], [-1.905, 0.0]),
        line([-1.905, 1.905], [-1.905, -1.905]),
        line([-1.27, 1.905], [-1.27, 1.27]),
        line([-1.27, 0.635], [-1.27, -0.635]),
        line([-1.27, -1.27], [-1.27, -1.905]),
        // Drain and source stubs out to the pin ends.
        line([-1.27, 1.5875], [2.54, 1.5875]),
        line([2.54, 1.5875], [2.54, 2.54]),
        line([-1.27, -1.5875], [2.54, -1.5875]),
        line([2.54, -1.5875], [2.54, -2.54]),
        // Body tie from the channel centre to the source rail.
        line([-1.27, 0.0], [1.905, 0.0]),
        line([1.905, 0.0], [1.905, -1.5875]),
    ];
    // Body-tie arrow: toward the channel for N, away from it for P.
    body.push(if is_n_channel {
        arrow_head([-1.27, 0.0], [0.0, 0.0], 0.762, 0.254)
    } else {
        arrow_head([0.0, 0.0], [-1.27, 0.0], 0.762, 0.254)
    });
    let desc = if is_n_channel {
        "N-channel enhancement MOSFET; pins 1 G, 2 S, 3 D."
    } else {
        "P-channel enhancement MOSFET; pins 1 G, 2 S, 3 D."
    };
    symbol(name, "Q?", desc, pins, body)
}

fn crystal() -> Symbol {
    let pins = vec![
        name_hidden(pin("1", "1", Passive, -3.81, 0.0, Right)),
        name_hidden(pin("2", "2", Passive, 3.81, 0.0, Left)),
    ];
    let body = vec![
        line([-1.27, 0.0], [-1.016, 0.0]),
        line([-1.016, 1.27], [-1.016, -1.27]),
        rect([-0.508, 1.524], [0.508, -1.524]),
        line([1.016, 1.27], [1.016, -1.27]),
        line([1.016, 0.0], [1.27, 0.0]),
    ];
    symbol(
        "Crystal",
        "Y?",
        "Two-terminal crystal resonator.",
        pins,
        body,
    )
}

/// Single-pin supply symbol. The pin connects at the origin and the
/// symbol draws away from it: ground below, supplies above.
fn power_port(name: &str) -> Symbol {
    let (orientation, body, desc) = match name {
        "GND" => (
            Down,
            vec![
                line([-1.27, -1.27], [1.27, -1.27]),
                line([-0.762, -1.778], [0.762, -1.778]),
                line([-0.254, -2.286], [0.254, -2.286]),
            ],
            "Ground power port.",
        ),
        "VCC" => (
            Up,
            vec![line([-1.016, 1.27], [1.016, 1.27])],
            "VCC supply power port (bar).",
        ),
        "+3V3" => (
            Up,
            vec![polygon(vec![[-0.635, 1.27], [0.635, 1.27], [0.0, 2.032]])],
            "+3V3 supply power port (arrow).",
        ),
        _ => (
            Up,
            vec![circle([0.0, 1.905], 0.635)],
            "+5V supply power port (circle).",
        ),
    };
    let pins = vec![labels_hidden(with_length(
        pin("1", name, Power, 0.0, 0.0, orientation),
        1.27,
    ))];
    let mut sym = symbol(name, "#PWR?", desc, pins, body);
    sym.comment = name.to_string();
    sym
}

/// Eight-pin rectangular IC body, four pins per side.
fn ic_generic() -> Symbol {
    let mut pins = Vec::new();
    for (i, y) in [3.81, 1.27, -1.27, -3.81].into_iter().enumerate() {
        let (left, right) = (i + 1, 8 - i);
        pins.push(pin(
            &left.to_string(),
            &format!("P{left}"),
            Unspecified,
            -7.62,
            y,
            Right,
        ));
        pins.push(pin(
            &right.to_string(),
            &format!("P{right}"),
            Unspecified,
            7.62,
            y,
            Left,
        ));
    }
    symbol(
        "IC_Generic",
        "U?",
        "Generic 8-pin rectangular IC body; rename pins per part.",
        pins,
        vec![rect([-5.08, 5.08], [5.08, -5.08])],
    )
}
