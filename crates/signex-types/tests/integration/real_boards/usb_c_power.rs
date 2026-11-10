//! Board 2: a USB-C 5 V power input with protection, two layers, 30 x 22 mm
//! with rounded corners. A power-only receptacle (5.1 k CC pull-downs on the
//! bottom side), a 1206 polyfuse, a SOD-123 TVS, bulk capacitance, a power
//! LED and a 2-pin output header. CC escapes through vias to the bottom; the
//! bottom is a GND pour.

use super::builder::{BOTTOM, Builder, Rules, Side, TOP, class, p, rounded_rect};
use super::parts;

const POWER: f64 = 0.8;
const SIGNAL: f64 = 0.25;

pub fn build() -> Builder {
    let mut b = Builder::new(
        2,
        Rules {
            copper_layers: 2,
            clearance: 0.2,
            track: SIGNAL,
            via_diameter: 0.6,
            via_drill: 0.3,
        },
    );
    b.net_class(class("Power", &["VBUS", "+5V", "GND"], 0.25, POWER));
    b.net_class(class("CC", &["CC1", "CC2"], 0.2, SIGNAL));
    b.outline(rounded_rect(0.0, 0.0, 30.0, 22.0, 1.5, 6));

    // Turned so the plug opening faces the left board edge.
    let j1 = b.place(
        parts::usb_c_6p(),
        "J1",
        "USB_C_Power",
        p(4.5, 11.0),
        270.0,
        Side::Top,
        &[
            ("A12", "GND"),
            ("B12", "GND"),
            ("A9", "VBUS"),
            ("B9", "VBUS"),
            ("A5", "CC1"),
            ("B5", "CC2"),
            ("S1", "GND"),
        ],
    );
    b.lock(j1);
    let f1 = b.place(
        parts::fuse1206(),
        "F1",
        "1.5A hold",
        p(11.5, 11.0),
        0.0,
        Side::Top,
        &[("1", "VBUS"), ("2", "+5V")],
    );
    let d1 = b.place(
        parts::sod123(),
        "D1",
        "SMF5.0A",
        p(15.5, 13.5),
        90.0,
        Side::Top,
        &[("1", "+5V"), ("2", "GND")],
    );
    let c1 = b.place(
        parts::c0805(),
        "C1",
        "22uF",
        p(18.5, 11.95),
        90.0,
        Side::Top,
        &[("1", "+5V"), ("2", "GND")],
    );
    let r3 = b.place(
        parts::r0603(),
        "R3",
        "1k",
        p(22.0, 13.0),
        90.0,
        Side::Top,
        &[("1", "+5V"), ("2", "LED_A")],
    );
    let d2 = b.place(
        parts::led0805(),
        "D2",
        "Green",
        p(22.0, 16.5),
        90.0,
        Side::Top,
        &[("1", "LED_A"), ("2", "GND")],
    );
    let j2 = b.place(
        parts::pin_header(2),
        "J2",
        "5V out",
        p(27.0, 11.0),
        0.0,
        Side::Top,
        &[("1", "+5V"), ("2", "GND")],
    );
    // CC pull-downs on the bottom, turned so pin 1 faces the connector.
    let r1 = b.place(
        parts::r0603(),
        "R1",
        "5.1k",
        p(8.0, 17.5),
        180.0,
        Side::Bottom,
        &[("1", "CC1"), ("2", "GND")],
    );
    let r2 = b.place(
        parts::r0603(),
        "R2",
        "5.1k",
        p(8.0, 4.5),
        180.0,
        Side::Bottom,
        &[("1", "CC2"), ("2", "GND")],
    );

    // VBUS: both contacts join behind the receptacle, then to the fuse.
    let (a9, b9) = (b.pad_at(j1, "A9"), b.pad_at(j1, "B9"));
    let spine = 5.6;
    b.track(
        "VBUS",
        TOP,
        POWER,
        &[a9, p(spine, a9.y), p(spine, b9.y), b9],
    );
    let f1_1 = b.pad_at(f1, "1");
    b.track("VBUS", TOP, POWER, &[p(spine, f1_1.y), f1_1]);
    // +5V rail: fuse, TVS, bulk cap (C1 pin 1 sits on it), LED feed, output.
    let rail = b.pad_at(f1, "2");
    let j2_1 = b.pad_at(j2, "1");
    b.track("+5V", TOP, POWER, &[rail, j2_1]);
    for (index, pin) in [(d1, "1"), (r3, "1")] {
        let pad = b.pad_at(index, pin);
        b.track("+5V", TOP, POWER, &[p(pad.x, rail.y), pad]);
    }
    b.track(
        "LED_A",
        TOP,
        SIGNAL,
        &[b.pad_at(r3, "2"), b.pad_at(d2, "1")],
    );
    // GND returns to the bottom pour.
    for (index, pin, via) in [
        (d1, "2", p(15.5, 16.8)),
        (c1, "2", p(18.5, 14.4)),
        (d2, "2", p(22.0, 19.2)),
    ] {
        let pad = b.pad_at(index, pin);
        b.track("GND", TOP, POWER, &[pad, via]);
        b.via("GND", via, 0.8, 0.4);
    }
    // The GND contacts tie into the shell slots next to them.
    for (pin, shell) in [("A12", p(3.5, 15.32)), ("B12", p(3.5, 6.68))] {
        let pad = b.pad_at(j1, pin);
        b.track("GND", TOP, 0.4, &[pad, shell]);
    }
    // CC: a via beside each contact, then the bottom to the pull-downs.
    for (net, pin, r, dir) in [("CC1", "A5", r1, 1.0), ("CC2", "B5", r2, -1.0)] {
        let pad = b.pad_at(j1, pin);
        let via = p(4.4, pad.y);
        b.track(net, TOP, SIGNAL, &[pad, via]);
        b.via(net, via, 0.6, 0.3);
        let end = b.pad_at(r, "1");
        b.track(
            net,
            BOTTOM,
            SIGNAL,
            &[
                via,
                p(5.4, pad.y + dir),
                p(5.4, end.y - dir),
                p(6.4, end.y),
                end,
            ],
        );
    }
    b.zone(
        "GND",
        BOTTOM,
        rounded_rect(0.5, 0.5, 29.5, 21.5, 1.0, 4),
        Vec::new(),
        0,
    );

    b.board_text("USB-C 5V IN", p(15.0, 3.0), "TopSilkscreen", 1.2, 0.0);
    b.board_text("5V", p(27.0, 7.5), "TopSilkscreen", 1.0, 0.0);
    b.board_text("CC 5k1", p(8.0, 2.0), "BottomSilkscreen", 0.8, 0.0);
    b
}
