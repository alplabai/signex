//! Board 3: a TQFP-32 microcontroller breakout (ATmega328P-AU pin-out), two
//! layers, 40 x 44 mm. Each side of the chip fans out to its own 1x8 header;
//! decoupling, the AVCC / AREF capacitors, the crystal and its load caps sit
//! on the bottom, reached by vias on the fan-out tracks. The bottom is a GND
//! pour; a short bottom track ties the two VCC pins together.
//!
//! The fan-out is computed once for the left side in chip-centred
//! coordinates and turned a quarter at a time for the other sides. On each
//! side the pins heading furthest out bend nearest the chip, so no two tracks
//! cross; the top and bottom fans bend beyond the left and right fans' reach
//! so the corners stay clear.

use super::builder::{BOTTOM, Builder, Rules, Side, TOP, class, p, rect};
use super::parts;
use signex_types::pcb::Point;

const SIGNAL: f64 = 0.25;
const CHIP: Point = Point::new(20.0, 22.0);

/// A bottom-side two-pin part: footprint, reference, value, side, position
/// and turn in the left frame of that side, and the nets of pins 1 and 2.
type BottomPart = (
    fn() -> parts::Part,
    &'static str,
    &'static str,
    usize,
    Point,
    f64,
    [&'static str; 2],
);

/// ATmega328P-AU pin names, pin 1 first.
const PINS: [&str; 32] = [
    "PD3", "PD4", "GND", "VCC", "GND", "VCC", "XTAL1", "XTAL2", "PD5", "PD6", "PD7", "PB0", "PB1",
    "PB2", "MOSI", "MISO", "SCK", "AVCC", "ADC6", "AREF", "GND", "ADC7", "PC0", "PC1", "PC2",
    "PC3", "SDA", "SCL", "RESET", "RXD", "TXD", "PD2",
];

/// A chip-centred point of side `side` (0 left, 1 bottom, 2 right, 3 top)
/// given in the left side's frame, on the board.
fn on_side(side: usize, q: Point) -> Point {
    let (x, y) = (0..side).fold((q.x, q.y), |(x, y), _| (y, -x));
    p(CHIP.x + x, CHIP.y + y)
}

/// The quarter turn of side `side`, degrees.
fn turn(side: usize) -> f64 {
    (270.0 * side as f64) % 360.0
}

/// First bend distance from the chip centre and header distance per side.
fn reach(side: usize) -> (f64, f64) {
    if side.is_multiple_of(2) {
        (5.5, 12.7)
    } else {
        (10.0, 15.24)
    }
}

/// The left-frame path of pad `i` (0 = first pin of the side) to header pin
/// `i`: out to its bend, along to the header row, into the header.
fn fan_path(side: usize, i: usize) -> [Point; 4] {
    let (d0, h) = reach(side);
    let y = -2.8 + 0.8 * i as f64;
    let target = -8.89 + 2.54 * i as f64;
    let rank = if i < 4 { i } else { 7 - i };
    let bend = -(d0 + 0.8 * rank as f64);
    [p(-4.25, y), p(bend, y), p(bend, target), p(-h, target)]
}

pub fn build() -> Builder {
    let mut b = Builder::new(
        3,
        Rules {
            copper_layers: 2,
            clearance: 0.2,
            track: SIGNAL,
            via_diameter: 0.6,
            via_drill: 0.3,
        },
    );
    b.net_class(class("Power", &["VCC", "AVCC", "GND"], 0.25, 0.3));
    b.net_class(class("Clock", &["XTAL1", "XTAL2"], 0.25, SIGNAL));
    b.outline(rect(0.0, 0.0, 40.0, 44.0));

    let nets: Vec<(String, &str)> = PINS
        .iter()
        .enumerate()
        .map(|(i, name)| ((i + 1).to_string(), *name))
        .collect();
    let chip_nets: Vec<(&str, &str)> = nets.iter().map(|(n, s)| (n.as_str(), *s)).collect();
    b.place(
        parts::tqfp32(),
        "U1",
        "ATmega328P-AU",
        CHIP,
        0.0,
        Side::Top,
        &chip_nets,
    );
    for side in 0..4 {
        let (_, h) = reach(side);
        let header_nets: Vec<(String, &str)> = (0..8)
            .map(|i| ((i + 1).to_string(), PINS[side * 8 + i]))
            .collect();
        let header_nets: Vec<(&str, &str)> =
            header_nets.iter().map(|(n, s)| (n.as_str(), *s)).collect();
        let j = b.place(
            parts::pin_header(8),
            &format!("J{}", side + 1),
            "Conn_01x08",
            on_side(side, p(-h, -8.89)),
            turn(side),
            Side::Top,
            &header_nets,
        );
        b.lock(j);
        for i in 0..8 {
            let path = fan_path(side, i).map(|q| on_side(side, q));
            b.track(PINS[side * 8 + i], TOP, SIGNAL, &path);
        }
    }

    // Vias on the fan-out tracks, left frame: (side, net, at).
    let vias = [
        (0, "GND", p(-5.6, -1.2)),
        (0, "VCC", p(-6.6, -0.4)),
        (0, "GND", p(-5.6, 0.4)),
        (0, "VCC", p(-6.6, 1.2)),
        (0, "XTAL1", p(-6.3, 4.4)),
        (0, "XTAL2", p(-5.5, 5.6)),
        (2, "AVCC", p(-5.6, -2.0)),
        (2, "AREF", p(-5.6, -0.4)),
        (2, "GND", p(-6.6, 0.4)),
    ];
    for (side, net, at) in vias {
        b.via(net, on_side(side, at), 0.6, 0.3);
    }

    // Bottom parts, placed in the left frame of their side.
    let bottom: [BottomPart; 7] = [
        (
            parts::c0603,
            "C1",
            "100nF",
            0,
            p(-3.3, 0.425),
            90.0,
            ["VCC", "GND"],
        ),
        (
            parts::c0805,
            "C2",
            "10uF",
            0,
            p(-1.0, 0.25),
            90.0,
            ["VCC", "GND"],
        ),
        (
            parts::crystal_5032,
            "Y1",
            "16MHz",
            0,
            p(-9.0, 8.5),
            90.0,
            ["XTAL2", "XTAL1"],
        ),
        (
            parts::c0603,
            "C5",
            "22pF",
            0,
            p(-9.0, 4.0),
            0.0,
            ["XTAL1", "GND"],
        ),
        (
            parts::c0603,
            "C6",
            "22pF",
            0,
            p(-9.0, 12.6),
            0.0,
            ["XTAL2", "GND"],
        ),
        (
            parts::c0603,
            "C3",
            "100nF",
            2,
            p(-8.0, -2.775),
            90.0,
            ["AVCC", "GND"],
        ),
        (
            parts::c0603,
            "C4",
            "100nF",
            2,
            p(-9.6, -1.175),
            90.0,
            ["AREF", "GND"],
        ),
    ];
    let [c1, c2, y1, c5, c6, c3, c4] =
        bottom.map(|(part, reference, value, side, at, rotation, [one, two])| {
            b.place(
                part(),
                reference,
                value,
                on_side(side, at),
                (rotation + turn(side)) % 360.0,
                Side::Bottom,
                &[("1", one), ("2", two)],
            )
        });

    // Bottom routing, left frame.
    let left = |q: Point| on_side(0, q);
    let right = |q: Point| on_side(2, q);
    b.track(
        "VCC",
        BOTTOM,
        0.3,
        &[
            left(p(-6.6, -0.4)),
            left(p(-6.6, 1.2)),
            b.pad_at(c1, "1"),
            b.pad_at(c2, "1"),
        ],
    );
    b.track(
        "XTAL1",
        BOTTOM,
        SIGNAL,
        &[left(p(-6.3, 4.4)), left(p(-6.3, 6.65)), b.pad_at(y1, "2")],
    );
    b.track(
        "XTAL1",
        BOTTOM,
        SIGNAL,
        &[left(p(-6.3, 4.4)), left(p(-6.7, 4.0)), b.pad_at(c5, "1")],
    );
    b.track(
        "XTAL2",
        BOTTOM,
        SIGNAL,
        &[left(p(-5.5, 5.6)), left(p(-5.5, 10.35)), b.pad_at(y1, "1")],
    );
    b.track(
        "XTAL2",
        BOTTOM,
        SIGNAL,
        &[left(p(-5.5, 10.35)), left(p(-5.5, 12.6)), b.pad_at(c6, "1")],
    );
    b.track(
        "AVCC",
        BOTTOM,
        0.3,
        &[right(p(-5.6, -2.0)), b.pad_at(c3, "1")],
    );
    b.track(
        "AREF",
        BOTTOM,
        SIGNAL,
        &[right(p(-5.6, -0.4)), b.pad_at(c4, "1")],
    );

    b.zone("GND", BOTTOM, rect(0.5, 0.5, 39.5, 43.5), Vec::new(), 0);
    b.board_text(
        "ATmega328P breakout",
        p(20.0, 41.5),
        "TopSilkscreen",
        1.2,
        0.0,
    );
    b.board_text("16 MHz", p(3.5, 30.5), "BottomSilkscreen", 0.8, 90.0);
    b
}
