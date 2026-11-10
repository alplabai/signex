//! Board 5: a mixed through-hole / SMD 12 V to 5 V supply, two layers,
//! 50 x 40 mm with a 10 mm arc on one corner. A barrel jack with plated
//! slots, an SOD-123 reverse-polarity diode, a TO-220 7805, radial
//! electrolytics, bottom-side 0805 decoupling under the regulator, a 45-degree
//! LED indicator and a screw-terminal output. A board sketch (#2234) carries
//! four M3 mounting holes (two plated to GND) and a copper keepout.

use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use signex_types::pcb_sketch::{BoardSketch, PcbHole, PcbKeepout};
use signex_types::sketch::SketchData;
use signex_types::sketch::attr::{KeepoutAttr, KeepoutKinds, MountingHoleAttr};
use signex_types::sketch::constraint::{Constraint, ConstraintKind};
use signex_types::sketch::entity::{Entity, EntityKind};
use signex_types::sketch::id::{ConstraintId, SketchEntityId};
use signex_types::sketch::plane::{Plane, PlaneId, PlaneKind};

use super::builder::{BOTTOM, Builder, Rules, Side, TOP, arc_points, class, p, rect};
use super::parts;

const POWER: f64 = 1.0;
const SIGNAL: f64 = 0.3;

/// Mounting holes: centre, plated to GND.
const HOLES: [(f64, f64, bool); 4] = [
    (4.0, 4.0, true),
    (42.5, 7.5, false),
    (4.0, 36.0, false),
    (46.0, 36.0, true),
];

/// The copper keepout: no copper under the front-panel standoff.
const KEEPOUT: (f64, f64, f64, f64) = (28.0, 2.0, 36.0, 8.0);

pub fn build() -> Builder {
    let mut b = Builder::new(
        5,
        Rules {
            copper_layers: 2,
            clearance: 0.25,
            track: SIGNAL,
            via_diameter: 0.8,
            via_drill: 0.4,
        },
    );
    b.net_class(class("Power", &["VIN", "+12V", "+5V", "GND"], 0.3, POWER));
    outline(&mut b);

    // Barrel jack turned so the plug enters from the left edge.
    let j1 = b.place(
        parts::barrel_jack(),
        "J1",
        "12V DC",
        p(14.5, 20.0),
        270.0,
        Side::Top,
        &[("1", "VIN"), ("2", "GND"), ("3", "GND")],
    );
    b.lock(j1);
    let d1 = b.place(
        parts::sod123(),
        "D1",
        "SS14",
        p(20.0, 20.0),
        180.0,
        Side::Top,
        &[("1", "+12V"), ("2", "VIN")],
    );
    let c1 = b.place(
        parts::cap_radial_6p3(),
        "C1",
        "470uF 25V",
        p(26.0, 12.0),
        0.0,
        Side::Top,
        &[("1", "+12V"), ("2", "GND")],
    );
    let u1 = b.place(
        parts::to220_vertical(),
        "U1",
        "7805",
        p(30.0, 22.0),
        0.0,
        Side::Top,
        &[("1", "+12V"), ("2", "GND"), ("3", "+5V")],
    );
    let c2 = b.place(
        parts::cap_radial_6p3(),
        "C2",
        "100uF 10V",
        p(30.0, 31.0),
        0.0,
        Side::Top,
        &[("1", "+5V"), ("2", "GND")],
    );
    let c3 = b.place(
        parts::c0805(),
        "C3",
        "330nF",
        p(31.27, 24.5),
        0.0,
        Side::Bottom,
        &[("1", "GND"), ("2", "+12V")],
    );
    let c4 = b.place(
        parts::c0805(),
        "C4",
        "100nF",
        p(35.5, 26.0),
        90.0,
        Side::Bottom,
        &[("1", "GND"), ("2", "+5V")],
    );
    let j2 = b.place(
        parts::screw_terminal_2(),
        "J2",
        "5V OUT",
        p(44.0, 25.0),
        90.0,
        Side::Top,
        &[("1", "+5V"), ("2", "GND")],
    );
    b.lock(j2);
    let r1 = b.place(
        parts::r0805(),
        "R1",
        "1k",
        p(41.0, 15.0),
        45.0,
        Side::Top,
        &[("1", "LED_A"), ("2", "+5V")],
    );
    let d2 = b.place(
        parts::led0805(),
        "D2",
        "Red",
        p(36.0, 15.0),
        45.0,
        Side::Top,
        &[("1", "LED_A"), ("2", "GND")],
    );

    // Input: jack, diode, bulk cap, regulator.
    b.track("VIN", TOP, POWER, &[b.pad_at(j1, "1"), b.pad_at(d1, "2")]);
    let k = b.pad_at(d1, "1");
    let vin = b.pad_at(u1, "1");
    b.track("+12V", TOP, POWER, &[k, p(28.0, k.y), vin]);
    b.track(
        "+12V",
        TOP,
        POWER,
        &[p(25.0, k.y), p(25.0, 13.0), b.pad_at(c1, "1")],
    );
    // Output: regulator to the bulk cap, the terminal and the LED.
    let out = b.pad_at(u1, "3");
    b.track("+5V", TOP, POWER, &[out, p(41.0, out.y), b.pad_at(j2, "1")]);
    b.track(
        "+5V",
        TOP,
        POWER,
        &[out, p(out.x, 26.0), p(30.0, 26.0), b.pad_at(c2, "1")],
    );
    let r1_5v = b.pad_at(r1, "2");
    b.track(
        "+5V",
        TOP,
        SIGNAL,
        &[p(41.0, out.y), p(41.0, r1_5v.y + 0.645), r1_5v],
    );
    b.track(
        "LED_A",
        TOP,
        SIGNAL,
        &[b.pad_at(r1, "1"), b.pad_at(d2, "1")],
    );
    let k2 = b.pad_at(d2, "2");
    let via = p(k2.x + 0.9, k2.y + 0.9);
    b.track("GND", TOP, SIGNAL, &[k2, via]);
    b.via("GND", via, 0.8, 0.4);
    // Bottom-side decoupling straight onto the regulator legs.
    let c3_in = b.pad_at(c3, "2");
    let c3_gnd = b.pad_at(c3, "1");
    let gnd = b.pad_at(u1, "2");
    b.track(
        "+12V",
        BOTTOM,
        POWER,
        &[c3_in, p(vin.x, c3_in.y - 0.5), vin],
    );
    b.track(
        "GND",
        BOTTOM,
        POWER,
        &[c3_gnd, p(gnd.x, c3_gnd.y - 0.5), gnd],
    );
    let c4_out = b.pad_at(c4, "2");
    b.track(
        "+5V",
        BOTTOM,
        POWER,
        &[c4_out, p(out.x, c4_out.y - 1.05), out],
    );

    // The bottom GND pour follows the outline 0.6 mm in, arc included.
    let mut pour = vec![p(0.6, 0.6)];
    pour.extend(arc_points(p(40.0, 10.0), 9.4, -90.0, 90.0, 12));
    pour.extend([p(49.4, 39.4), p(0.6, 39.4)]);
    b.zone("GND", BOTTOM, pour, Vec::new(), 0);
    let sketch = board_sketch(&mut b);
    b.sketch(sketch);

    b.board_text("12V IN", p(8.0, 28.0), "TopSilkscreen", 1.2, 0.0);
    b.board_text("5V OUT", p(44.0, 18.5), "TopSilkscreen", 1.2, 0.0);
    b.board_text("+  -", p(44.0, 34.5), "TopSilkscreen", 1.2, 90.0);
    b.board_text(
        "7805 SUPPLY rev B",
        p(25.0, 38.0),
        "BottomSilkscreen",
        1.0,
        0.0,
    );
    b
}

/// The outline: a 50 x 40 rectangle whose top-right corner is a 10 mm arc
/// (drawn on `EdgeCuts` as two lines, one arc and two more lines).
fn outline(b: &mut Builder) {
    let mut points = vec![p(0.0, 0.0), p(40.0, 0.0)];
    let arc = arc_points(p(40.0, 10.0), 10.0, -90.0, 90.0, 16);
    points.extend(arc[1..].iter().copied());
    points.extend([p(50.0, 40.0), p(0.0, 40.0)]);
    b.outline_polygon(points);
    b.edge("line", Some(p(0.0, 0.0)), Some(p(40.0, 0.0)), None, 0.0);
    b.edge(
        "arc",
        Some(p(40.0, 0.0)),
        Some(p(50.0, 10.0)),
        Some(p(40.0, 10.0)),
        10.0,
    );
    b.edge("line", Some(p(50.0, 10.0)), Some(p(50.0, 40.0)), None, 0.0);
    b.edge("line", Some(p(50.0, 40.0)), Some(p(0.0, 40.0)), None, 0.0);
    b.edge("line", Some(p(0.0, 40.0)), Some(p(0.0, 0.0)), None, 0.0);
}

/// The board sketch and what it bakes to: the four hole circles, each
/// centre fixed, and a fixed keepout rectangle tagged on its first edge.
fn board_sketch(b: &mut Builder) -> BoardSketch {
    let plane = PlaneId(b.uuid());
    let mut sketch = SketchData::default();
    sketch.planes.push(Plane {
        id: plane,
        kind: PlaneKind::BoardTop,
    });
    sketch.parameters.insert("m3_drill", "3.2mm");
    let mut fixed = Vec::new();
    let mut holes = Vec::new();
    for (x, y, plated) in HOLES {
        let centre = SketchEntityId(b.uuid());
        sketch
            .entities
            .push(Entity::new(centre, plane, EntityKind::Point { x, y }));
        fixed.push(centre);
        let circle_id = SketchEntityId(b.uuid());
        let mut circle = Entity::new(
            circle_id,
            plane,
            EntityKind::Circle {
                center: centre,
                radius: 1.6,
            },
        );
        circle.mounting_hole = Some(MountingHoleAttr {
            drill_expr: Some("= m3_drill".into()),
            plated,
            net: plated.then(|| "GND".to_string()),
        });
        sketch.entities.push(circle);
        holes.push(PcbHole {
            uuid: circle_id.0,
            position: p(x, y),
            drill: 3.2,
            plated,
            net: plated.then(|| b.net("GND")),
            unknown_columns: Default::default(),
        });
    }
    let (x0, y0, x1, y1) = KEEPOUT;
    let corners: Vec<SketchEntityId> = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
        .into_iter()
        .map(|(x, y)| {
            let id = SketchEntityId(b.uuid());
            sketch
                .entities
                .push(Entity::new(id, plane, EntityKind::Point { x, y }));
            fixed.push(id);
            id
        })
        .collect();
    let kinds = KeepoutKinds {
        no_routing: true,
        no_copper: true,
        no_vias: true,
        ..KeepoutKinds::default()
    };
    let mut seed = None;
    for i in 0..4 {
        let id = SketchEntityId(b.uuid());
        let mut edge = Entity::new(
            id,
            plane,
            EntityKind::Line {
                start: corners[i],
                end: corners[(i + 1) % 4],
            },
        );
        if i == 0 {
            edge.keepout = Some(KeepoutAttr {
                layer: PcbLayer::Copper(CopperLayer::Top),
                kinds,
            });
            seed = Some(id);
        }
        sketch.entities.push(edge);
    }
    for point in fixed {
        sketch.constraints.push(Constraint {
            id: ConstraintId(b.uuid()),
            kind: ConstraintKind::Fixed { point },
        });
    }
    let keepout = PcbKeepout {
        uuid: seed.expect("tagged edge").0,
        boundary: rect(x0, y0, x1, y1),
        layers: vec![PcbLayer::Copper(CopperLayer::Top)],
        forbids: kinds,
    };
    BoardSketch {
        sketch,
        keepouts: vec![keepout],
        cutouts: Vec::new(),
        holes,
        v_scores: Vec::new(),
    }
}
