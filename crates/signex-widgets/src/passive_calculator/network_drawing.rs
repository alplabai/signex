//! The series/parallel drawing beside the result's part cards: the network
//! tree laid out as line art for the property illustration (BB2).
//!
//! A series connection is a left-to-right chain; a parallel connection
//! stacks its two branches between two rails. A resistor is a zigzag, a
//! capacitor a plate pair and an inductor four bumps, each labelled with
//! its designator (`R1`, `C2`, ...) in the order the expression and the
//! cards number them. Every symbol is linked to its part, so hovering it
//! lights up the symbol and its card. Coordinates are view units; the
//! widget fits them into its box.

use std::f32::consts::PI;

use iced::{Point, Radians};

use super::domain::ComponentKind;
use super::network::{BoundaryCondition, Connection, Network};
use super::widgets::data;
use crate::property_illustration::{
    Anchor, FieldId, Illustration, Ink, Part, Primitive, Style, TextLabel, TextRole,
};

/// Width of one symbol with its two leads.
const LEAF_W: f32 = 40.0;
/// Height of one symbol with its label under it.
const LEAF_H: f32 = 28.0;
/// Height of a symbol's wire above the top of its box.
const LEAF_WIRE: f32 = 9.0;
/// Length of each lead between the box edge and the symbol body.
const LEAD: f32 = 8.0;
/// Half the height of a zigzag.
const ZIGZAG_HALF: f32 = 5.0;
/// Half the height of a capacitor plate, and half the plate gap.
const PLATE_HALF: f32 = 7.0;
const PLATE_GAP_HALF: f32 = 2.0;
/// Radius of an inductor bump; four bumps span the body.
const BUMP_R: f32 = 3.0;
/// Gap of an open circuit.
const OPEN_GAP_HALF: f32 = 4.0;
/// Distance of a designator below its wire.
const LABEL_DY: f32 = 11.0;
/// Gap between two stacked branches.
const BRANCH_GAP: f32 = 4.0;
/// Lead between a parallel block's terminal and its rail.
const RAIL_PAD: f32 = 6.0;
/// Clear margin round the whole drawing.
const MARGIN: f32 = 2.0;
/// Bounds of the drawing height, in captions.
const MIN_HEIGHT_EM: f32 = 3.0;
const MAX_HEIGHT_EM: f32 = 12.0;

/// The id that links the drawing of part `index` (from 0) to its card.
pub fn part_field(index: usize) -> FieldId {
    FieldId::owned(format!("part-{index}"))
}

/// The drawing of `network`, sized for a box `width_px` wide at a caption
/// of `text_size` px.
pub fn network_illustration(
    network: &Network,
    kind: ComponentKind,
    width_px: f32,
    text_size: f32,
) -> Illustration {
    let extent = Extent::of(network);
    let mut sketch = Sketch {
        kind,
        next_index: 0,
        parts: Vec::new(),
    };
    sketch.draw(network, MARGIN, MARGIN);
    let view_w = extent.w + 2.0 * MARGIN;
    let view_h = extent.h + 2.0 * MARGIN;
    let height_em =
        (width_px * view_h / view_w / text_size.max(1.0)).clamp(MIN_HEIGHT_EM, MAX_HEIGHT_EM);
    Illustration::new(view_w, view_h)
        .height_em(height_em)
        .parts(sketch.parts)
}

/// The box a sub-network takes: its size and the height of its terminals.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Extent {
    w: f32,
    h: f32,
    wire: f32,
}

impl Extent {
    const LEAF: Self = Self {
        w: LEAF_W,
        h: LEAF_H,
        wire: LEAF_WIRE,
    };

    fn of(network: &Network) -> Self {
        match network {
            Network::Boundary { .. } | Network::Component { .. } => Self::LEAF,
            Network::Connected {
                connection: Connection::Series,
                left,
                right,
            } => {
                let (a, b) = (Self::of(left), Self::of(right));
                let wire = a.wire.max(b.wire);
                Self {
                    w: a.w + b.w,
                    h: (wire - a.wire + a.h).max(wire - b.wire + b.h),
                    wire,
                }
            }
            Network::Connected {
                connection: Connection::Parallel,
                left,
                right,
            } => {
                let (a, b) = (Self::of(left), Self::of(right));
                let h = a.h + BRANCH_GAP + b.h;
                Self {
                    w: a.w.max(b.w) + 2.0 * RAIL_PAD,
                    h,
                    wire: h / 2.0,
                }
            }
        }
    }
}

/// The parts drawn so far and the next designator number.
struct Sketch {
    kind: ComponentKind,
    next_index: usize,
    parts: Vec<Part>,
}

impl Sketch {
    /// Draws `network` with its box's top-left at (`x`, `top`).
    fn draw(&mut self, network: &Network, x: f32, top: f32) {
        let extent = Extent::of(network);
        match network {
            Network::Boundary { condition } => self.boundary(*condition, x, top + extent.wire),
            Network::Component { .. } => self.symbol(x, top + extent.wire),
            Network::Connected {
                connection: Connection::Series,
                left,
                right,
            } => {
                let (a, b) = (Extent::of(left), Extent::of(right));
                self.draw(left, x, top + extent.wire - a.wire);
                self.draw(right, x + a.w, top + extent.wire - b.wire);
            }
            Network::Connected {
                connection: Connection::Parallel,
                left,
                right,
            } => self.parallel(left, right, extent, x, top),
        }
    }

    fn parallel(&mut self, upper: &Network, lower: &Network, extent: Extent, x: f32, top: f32) {
        let (a, b) = (Extent::of(upper), Extent::of(lower));
        let wire = top + extent.wire;
        let rail_l = x + RAIL_PAD;
        let rail_r = x + extent.w - RAIL_PAD;
        let lower_top = top + a.h + BRANCH_GAP;
        let upper_wire = top + a.wire;
        let lower_wire = lower_top + b.wire;
        self.wire(x, wire, rail_l, wire);
        self.wire(rail_r, wire, x + extent.w, wire);
        let rail_top = upper_wire.min(wire);
        let rail_bottom = lower_wire.max(wire);
        self.wire(rail_l, rail_top, rail_l, rail_bottom);
        self.wire(rail_r, rail_top, rail_r, rail_bottom);
        self.draw(upper, rail_l, top);
        self.wire(rail_l + a.w, upper_wire, rail_r, upper_wire);
        self.draw(lower, rail_l, lower_top);
        self.wire(rail_l + b.w, lower_wire, rail_r, lower_wire);
    }

    fn wire(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        if (x0 - x1).abs() < f32::EPSILON && (y0 - y1).abs() < f32::EPSILON {
            return;
        }
        self.parts.push(Part::new(
            Primitive::Line {
                from: Point::new(x0, y0),
                to: Point::new(x1, y1),
            },
            Style::edge(),
        ));
    }

    fn boundary(&mut self, condition: BoundaryCondition, x: f32, wire: f32) {
        match condition {
            BoundaryCondition::WireBridge => self.wire(x, wire, x + LEAF_W, wire),
            BoundaryCondition::OpenCircuit => {
                let mid = x + LEAF_W / 2.0;
                self.wire(x, wire, mid - OPEN_GAP_HALF, wire);
                self.wire(mid + OPEN_GAP_HALF, wire, x + LEAF_W, wire);
            }
        }
    }

    /// One part's symbol, leads included, and its designator.
    fn symbol(&mut self, x: f32, wire: f32) {
        let index = self.next_index;
        self.next_index += 1;
        let field = part_field(index);
        let body: Vec<Primitive> = match self.kind {
            ComponentKind::Resistor => vec![zigzag(x, wire)],
            ComponentKind::Capacitor => plates(x, wire),
            ComponentKind::Inductor => coil(x, wire),
        };
        for primitive in body {
            self.parts
                .push(Part::new(primitive, Style::edge()).linked(field.clone()));
        }
        let label = TextLabel {
            at: Point::new(x + LEAF_W / 2.0, wire + LABEL_DY),
            text: data(format!("{}{}", self.kind.symbol(), index + 1)),
            anchor: Anchor::Middle,
            role: TextRole::Mono,
        };
        self.parts
            .push(Part::label(label, Ink::Tertiary).linked(field));
    }
}

/// A resistor: lead, three full zigzag periods, lead, as one polyline.
fn zigzag(x: f32, wire: f32) -> Primitive {
    let start = x + LEAD;
    let step = (LEAF_W - 2.0 * LEAD) / 6.0;
    let mut points = vec![Point::new(x, wire), Point::new(start, wire)];
    for k in 0..6 {
        let peak = if k % 2 == 0 {
            -ZIGZAG_HALF
        } else {
            ZIGZAG_HALF
        };
        points.push(Point::new(start + step * (k as f32 + 0.5), wire + peak));
    }
    points.push(Point::new(x + LEAF_W - LEAD, wire));
    points.push(Point::new(x + LEAF_W, wire));
    Primitive::Polyline {
        points,
        is_closed: false,
    }
}

/// A capacitor: two leads and two plates.
fn plates(x: f32, wire: f32) -> Vec<Primitive> {
    let mid = x + LEAF_W / 2.0;
    let (left, right) = (mid - PLATE_GAP_HALF, mid + PLATE_GAP_HALF);
    let line = |x0: f32, y0: f32, x1: f32, y1: f32| Primitive::Line {
        from: Point::new(x0, y0),
        to: Point::new(x1, y1),
    };
    vec![
        line(x, wire, left, wire),
        line(left, wire - PLATE_HALF, left, wire + PLATE_HALF),
        line(right, wire - PLATE_HALF, right, wire + PLATE_HALF),
        line(right, wire, x + LEAF_W, wire),
    ]
}

/// An inductor: two leads and four bumps over the wire.
fn coil(x: f32, wire: f32) -> Vec<Primitive> {
    let start = x + LEAF_W / 2.0 - 4.0 * BUMP_R;
    let end = x + LEAF_W / 2.0 + 4.0 * BUMP_R;
    let mut parts = vec![
        Primitive::Line {
            from: Point::new(x, wire),
            to: Point::new(start, wire),
        },
        Primitive::Line {
            from: Point::new(end, wire),
            to: Point::new(x + LEAF_W, wire),
        },
    ];
    for k in 0..4 {
        parts.push(Primitive::Arc {
            center: Point::new(start + BUMP_R * (2.0 * k as f32 + 1.0), wire),
            radius: BUMP_R,
            start: Radians(PI),
            end: Radians(2.0 * PI),
        });
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::passive_calculator::domain::{PreferredComponent, PreferredNumber, Tolerance};

    fn leaf() -> Network {
        Network::component(
            PreferredComponent {
                number: PreferredNumber {
                    significand: 47,
                    decimal_places: 1,
                },
                decade: 3,
            },
            Tolerance::Percent1,
        )
    }

    /// The zigzags of a resistor drawing: (part id, mean y, min x, max x).
    fn zigzags(illustration: &Illustration) -> Vec<(FieldId, f32, f32, f32)> {
        illustration
            .visible_parts()
            .filter_map(|part| match &part.primitive {
                Primitive::Polyline { points, .. } => {
                    let field = part.primary_field()?.clone();
                    let y = points.iter().map(|p| p.y).sum::<f32>() / points.len() as f32;
                    let min_x = points.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
                    let max_x = points.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max);
                    Some((field, y, min_x, max_x))
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_series_chain_draws_one_zigzag_per_part_left_to_right() {
        let network = Network::connected(Connection::Series, leaf(), leaf());
        let drawing = network_illustration(&network, ComponentKind::Resistor, 172.0, 10.9);
        let zigzags = zigzags(&drawing);
        assert_eq!(zigzags.len(), network.part_count());
        assert_eq!(zigzags[0].0, part_field(0));
        assert_eq!(zigzags[1].0, part_field(1));
        assert!((zigzags[0].1 - zigzags[1].1).abs() < 1e-3, "one wire");
        assert!(zigzags[0].3 <= zigzags[1].2 + 1e-3, "left to right");
    }

    #[test]
    fn parallel_branches_stack_between_rails() {
        let network = Network::connected(
            Connection::Parallel,
            leaf(),
            Network::connected(Connection::Series, leaf(), leaf()),
        );
        let drawing = network_illustration(&network, ComponentKind::Resistor, 172.0, 10.9);
        let zigzags = zigzags(&drawing);
        assert_eq!(zigzags.len(), 3);
        let (upper, lower) = (&zigzags[0], &zigzags[1]);
        assert!(
            lower.1 > upper.1 + LEAF_H / 2.0,
            "the second branch sits below"
        );
        assert!(
            upper.2 < lower.3 && lower.2 < upper.3,
            "the branches overlap in x"
        );
        assert!(
            (zigzags[1].1 - zigzags[2].1).abs() < 1e-3,
            "a series branch is one wire"
        );
        let rails = drawing
            .visible_parts()
            .filter(|part| {
                matches!(part.primitive, Primitive::Line { from, to }
                    if (from.x - to.x).abs() < 1e-3 && (from.y - to.y).abs() > 1.0)
            })
            .count();
        assert_eq!(rails, 2, "one rail on each side");
    }

    #[test]
    fn every_part_gets_its_designator_in_order() {
        let network = Network::connected(Connection::Series, leaf(), leaf());
        let drawing = network_illustration(&network, ComponentKind::Capacitor, 172.0, 10.9);
        let labels: Vec<String> = drawing
            .visible_parts()
            .filter_map(|part| match &part.primitive {
                Primitive::Label(label) => Some(label.text.as_str().to_owned()),
                _ => None,
            })
            .collect();
        assert_eq!(labels, ["C1", "C2"]);
    }

    #[test]
    fn a_wide_chain_is_drawn_flatter_than_a_stack() {
        let chain = Network::connected(Connection::Series, leaf(), leaf());
        let stack = Network::connected(Connection::Parallel, leaf(), leaf());
        let wide = network_illustration(&chain, ComponentKind::Inductor, 172.0, 10.9);
        let tall = network_illustration(&stack, ComponentKind::Inductor, 172.0, 10.9);
        assert!(wide.height_in_em() < tall.height_in_em());
    }
}
