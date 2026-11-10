//! Board 4: a four-layer I2C environmental-sensor board, 26 x 18 mm. A
//! BME280-style LGA-8 sensor, a SOT-23-5 3.3 V LDO, I2C pull-ups and a 1x4
//! header (VIN, GND, SDA, SCL). Inner 1 is a solid GND plane, inner 2 the
//! +3V3 plane; every supply pin drops to its plane through a via, and each
//! plane carries a square antipad around every via and through-hole pad of
//! another net.

use super::builder::{Builder, IN1, IN2, Rules, Side, TOP, class, p, rect};
use super::parts;
use signex_types::pcb::Point;

const SIGNAL: f64 = 0.2;
const POWER: f64 = 0.4;
const VIA: (f64, f64) = (0.5, 0.25);

/// The board edge with 1 mm chamfered corners.
fn chamfered(w: f64, h: f64, c: f64) -> Vec<Point> {
    vec![
        p(c, 0.0),
        p(w - c, 0.0),
        p(w, c),
        p(w, h - c),
        p(w - c, h),
        p(c, h),
        p(0.0, h - c),
        p(0.0, c),
    ]
}

pub fn build() -> Builder {
    let mut b = Builder::new(
        4,
        Rules {
            copper_layers: 4,
            clearance: 0.15,
            track: SIGNAL,
            via_diameter: VIA.0,
            via_drill: VIA.1,
        },
    );
    b.plane_layer(IN1);
    b.plane_layer(IN2);
    b.layer_display_name(IN1, "GND plane");
    b.layer_display_name(IN2, "3V3 plane");
    b.net_class(class("Power", &["VIN", "+3V3", "GND"], 0.2, POWER));
    b.net_class(class("I2C", &["SDA", "SCL"], 0.15, SIGNAL));
    b.outline(chamfered(26.0, 18.0, 1.0));

    let j1 = b.place(
        parts::pin_header(4),
        "J1",
        "I2C",
        p(3.0, 5.19),
        0.0,
        Side::Top,
        &[("1", "VIN"), ("2", "GND"), ("3", "SDA"), ("4", "SCL")],
    );
    b.lock(j1);
    let u2 = b.place(
        parts::sot23_5(),
        "U2",
        "AP2112K-3.3",
        p(9.5, 4.5),
        0.0,
        Side::Top,
        &[("1", "VIN"), ("2", "GND"), ("3", "VIN"), ("5", "+3V3")],
    );
    let c1 = b.place(
        parts::c0603(),
        "C1",
        "1uF",
        p(6.5, 6.375),
        90.0,
        Side::Top,
        &[("1", "VIN"), ("2", "GND")],
    );
    let c2 = b.place(
        parts::c0603(),
        "C2",
        "1uF",
        p(12.6, 3.6),
        90.0,
        Side::Top,
        &[("1", "+3V3"), ("2", "GND")],
    );
    let u1 = b.place(
        parts::lga8_2p5(),
        "U1",
        "BME280",
        p(18.0, 9.0),
        0.0,
        Side::Top,
        &[
            ("1", "GND"),
            ("2", "+3V3"),
            ("3", "SDA"),
            ("4", "SCL"),
            ("5", "GND"),
            ("6", "+3V3"),
            ("7", "GND"),
            ("8", "+3V3"),
        ],
    );
    let c3 = b.place(
        parts::c0603(),
        "C3",
        "100nF",
        p(22.0, 11.6),
        0.0,
        Side::Top,
        &[("1", "+3V3"), ("2", "GND")],
    );
    let r1 = b.place(
        parts::r0603(),
        "R1",
        "4.7k",
        p(8.0, 9.445),
        90.0,
        Side::Top,
        &[("1", "+3V3"), ("2", "SDA")],
    );
    let r2 = b.place(
        parts::r0603(),
        "R2",
        "4.7k",
        p(10.0, 11.985),
        90.0,
        Side::Top,
        &[("1", "+3V3"), ("2", "SCL")],
    );

    // VIN: header to the LDO input, past C1; EN loops round to VIN.
    let vin = b.pad_at(u2, "1");
    let j1_1 = b.pad_at(j1, "1");
    b.track(
        "VIN",
        TOP,
        POWER,
        &[
            j1_1,
            p(4.0, j1_1.y),
            p(4.0 + vin.y - j1_1.y, vin.y),
            b.pad_at(c1, "1"),
            vin,
        ],
    );
    let en = b.pad_at(u2, "3");
    b.track("VIN", TOP, POWER, &[en, p(en.x, 7.0), p(vin.x, 7.0), vin]);
    // I2C from the sensor to the header, through the pull-ups.
    let sda = b.pad_at(u1, "3");
    let scl = b.pad_at(u1, "4");
    let j1_3 = b.pad_at(j1, "3");
    let j1_4 = b.pad_at(j1, "4");
    b.track(
        "SDA",
        TOP,
        SIGNAL,
        &[
            sda,
            p(14.0, sda.y),
            p(14.0 - (j1_3.y - sda.y), j1_3.y),
            b.pad_at(r1, "2"),
            j1_3,
        ],
    );
    b.track(
        "SCL",
        TOP,
        SIGNAL,
        &[
            scl,
            p(14.6, scl.y),
            p(14.6 - (j1_4.y - scl.y), j1_4.y),
            b.pad_at(r2, "2"),
            j1_4,
        ],
    );

    // Supply pins drop to their planes: (part, pin, net, via, bend).
    let drops: [(usize, &str, &str, Point, Option<Point>); 12] = [
        (u2, "2", "GND", p(9.5, 4.5), None),
        (u2, "5", "+3V3", p(8.55, 2.0), None),
        (c1, "2", "GND", p(6.5, 8.4), None),
        (c2, "1", "+3V3", p(12.6, 1.6), None),
        (c2, "2", "GND", p(12.6, 5.6), None),
        (u1, "1", "GND", p(16.3, 7.3), None),
        (u1, "2", "+3V3", p(15.3, 8.2), Some(p(15.8, 8.675))),
        (u1, "8", "+3V3", p(20.5, 7.5), Some(p(20.0, 8.025))),
        (u1, "7", "GND", p(21.3, 8.675), None),
        (u1, "6", "+3V3", p(20.5, 9.85), Some(p(20.0, 9.325))),
        (u1, "5", "GND", p(19.6, 11.1), Some(p(19.6, 10.6))),
        (r1, "1", "+3V3", p(8.0, 7.6), None),
    ];
    for (part, pin, net, via, bend) in drops {
        let pad = b.pad_at(part, pin);
        let path: Vec<Point> = [Some(pad), bend, Some(via)].into_iter().flatten().collect();
        b.track(net, TOP, SIGNAL, &path);
        b.via(net, via, VIA.0, VIA.1);
    }
    for (part, pin, net, via) in [
        (r2, "1", "+3V3", p(11.2, 11.16)),
        (c3, "1", "+3V3", p(21.225, 12.8)),
        (c3, "2", "GND", p(22.775, 12.8)),
    ] {
        let pad = b.pad_at(part, pin);
        b.track(net, TOP, SIGNAL, &[pad, via]);
        b.via(net, via, VIA.0, VIA.1);
    }

    let inset = rect(0.6, 0.6, 25.4, 17.4);
    b.plane("GND", IN1, inset.clone(), 1.0, 2.4);
    b.plane("+3V3", IN2, inset, 1.0, 2.4);
    b.board_text("BME280 I2C", p(18.0, 15.5), "TopSilkscreen", 1.0, 0.0);
    b.board_text("VIN GND SDA SCL", p(3.0, 15.8), "TopSilkscreen", 0.7, 90.0);
    b
}
