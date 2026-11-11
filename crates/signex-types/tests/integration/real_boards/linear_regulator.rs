//! Board 1: the 5 V to 3.3 V linear regulator of the sample project
//! (`crates/signex-app/assets/sample-project/linear-regulator.snxsch`).
//! Two layers, 36 x 20 mm, every footprint linked to its schematic symbol.
//! Signals on top, a GND pour on the bottom reached by vias and the THT
//! header pins.

use uuid::Uuid;

use super::builder::{BOTTOM, Builder, Rules, Side, TOP, class, p, rect};
use super::parts;

/// The sample schematic's symbol uuids, by designator.
pub const SCHEMATIC_UNITS: [(&str, u128); 7] = [
    ("J1", 0x5a3c0000_0000_4000_8000_000000000001),
    ("C1", 0x5a3c0000_0000_4000_8000_000000000002),
    ("U1", 0x5a3c0000_0000_4000_8000_000000000003),
    ("C2", 0x5a3c0000_0000_4000_8000_000000000004),
    ("R1", 0x5a3c0000_0000_4000_8000_000000000005),
    ("D1", 0x5a3c0000_0000_4000_8000_000000000006),
    ("J2", 0x5a3c0000_0000_4000_8000_000000000007),
];

const POWER: f64 = 0.5;
const SIGNAL: f64 = 0.25;

pub fn build() -> Builder {
    let mut b = Builder::new(
        1,
        Rules {
            copper_layers: 2,
            clearance: 0.2,
            track: SIGNAL,
            via_diameter: 0.6,
            via_drill: 0.3,
        },
    );
    b.net_class(class("Power", &["+5V", "+3V3", "GND"], 0.25, POWER));
    b.outline(rect(0.0, 0.0, 36.0, 20.0));

    let j1 = b.place(
        parts::pin_header(2),
        "J1",
        "5V in",
        p(3.5, 8.0),
        0.0,
        Side::Top,
        &[("1", "+5V"), ("2", "GND")],
    );
    let c1 = b.place(
        parts::c0805(),
        "C1",
        "10uF",
        p(10.0, 6.8),
        90.0,
        Side::Top,
        &[("1", "+5V"), ("2", "GND")],
    );
    // Turned 180 so VIN (pin 3) faces the input.
    let u1 = b.place(
        parts::sot223(),
        "U1",
        "LDO_3V3",
        p(17.0, 9.0),
        180.0,
        Side::Top,
        &[("1", "GND"), ("2", "+3V3"), ("3", "+5V"), ("4", "+3V3")],
    );
    let c2 = b.place(
        parts::c0805(),
        "C2",
        "10uF",
        p(23.0, 13.1),
        90.0,
        Side::Top,
        &[("1", "+3V3"), ("2", "GND")],
    );
    let r1 = b.place(
        parts::r0603(),
        "R1",
        "470R",
        p(26.5, 12.15),
        0.0,
        Side::Top,
        &[("1", "+3V3"), ("2", "LED_A")],
    );
    let d1 = b.place(
        parts::led0603(),
        "D1",
        "Green",
        p(29.5, 14.5),
        90.0,
        Side::Top,
        &[("1", "LED_A"), ("2", "GND")],
    );
    let j2 = b.place(
        parts::pin_header(2),
        "J2",
        "3V3 out",
        p(32.5, 8.0),
        0.0,
        Side::Top,
        &[("1", "+3V3"), ("2", "GND")],
    );
    for (index, (reference, unit)) in [j1, c1, u1, c2, r1, d1, j2]
        .into_iter()
        .zip(SCHEMATIC_UNITS)
    {
        assert_eq!(b.footprint_ref(index), reference);
        b.link(index, Uuid::from_u128(unit));
    }
    b.lock(j1);
    b.lock(j2);

    // +5V: J1.1, down the 45 to the C1 / VIN row.
    let vin = b.pad_at(u1, "3");
    let j1_1 = b.pad_at(j1, "1");
    let row = vin.y;
    b.track(
        "+5V",
        TOP,
        POWER,
        &[
            j1_1,
            p(j1_1.x + 2.0, j1_1.y),
            p(j1_1.x + 2.0 + (j1_1.y - row), row),
            b.pad_at(c1, "1"),
            vin,
        ],
    );
    // +3V3: pin 2 under the body to the tab, out to C2, R1 and J2.
    let tab = b.pad_at(u1, "4");
    b.track("+3V3", TOP, POWER, &[b.pad_at(u1, "2"), tab]);
    let c2_1 = b.pad_at(c2, "1");
    let r1_1 = b.pad_at(r1, "1");
    b.track("+3V3", TOP, POWER, &[tab, c2_1, r1_1]);
    let j2_1 = b.pad_at(j2, "1");
    let branch = p(24.6, c2_1.y);
    b.track(
        "+3V3",
        TOP,
        POWER,
        &[
            branch,
            p(branch.x, j2_1.y + 1.0),
            p(branch.x + 1.0, j2_1.y),
            j2_1,
        ],
    );
    // LED string.
    let r1_2 = b.pad_at(r1, "2");
    let d1_a = b.pad_at(d1, "1");
    b.track("LED_A", TOP, SIGNAL, &[r1_2, p(d1_a.x, r1_2.y), d1_a]);

    // GND: SMD returns drop to the bottom pour.
    for (index, pin, via) in [
        (c1, "2", p(10.0, 9.6)),
        (u1, "1", p(21.2, 5.85)),
        (c2, "2", p(23.0, 16.0)),
        (d1, "2", p(29.5, 17.4)),
    ] {
        let pad = b.pad_at(index, pin);
        b.track("GND", TOP, POWER, &[pad, via]);
        b.via("GND", via, 0.8, 0.4);
    }
    b.zone("GND", BOTTOM, rect(0.5, 0.5, 35.5, 19.5), Vec::new(), 0);

    b.board_text("LDO 3V3", p(18.0, 17.5), "TopSilkscreen", 1.2, 0.0);
    b.board_text("5V IN", p(3.5, 3.5), "TopSilkscreen", 1.0, 0.0);
    b.board_text("3V3 OUT", p(32.5, 3.5), "TopSilkscreen", 1.0, 0.0);
    b.board_text("rev A", p(33.5, 18.0), "BottomSilkscreen", 0.8, 0.0);
    b
}
