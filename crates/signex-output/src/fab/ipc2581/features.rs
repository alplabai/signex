//! The marks of one layer as an IPC-2581 `LayerFeature`, and the standard
//! shapes they flash, collected into the `DictionaryStandard`.

use std::collections::BTreeSet;

use pcb_geom::PointNm;

use super::xml::{Xml, qualified};
use crate::fab::frame::millionths;
use crate::fab::marks::{ApertureShape, Function, Mark, Shape, Step};

/// The dictionary id of a shape: the kind and sizes in mm, so it reads on
/// its own. The turn goes on the feature's `Xform`.
fn shape_id(shape: Shape) -> String {
    match shape {
        Shape::Circle { diameter } => format!("CIRCLE_{}", millionths(diameter)),
        Shape::Rect { w, h } => format!("RECT_{}x{}", millionths(w), millionths(h)),
        Shape::Oval { w, h } => format!("OVAL_{}x{}", millionths(w), millionths(h)),
        Shape::RoundRect { w, h, r } => format!(
            "RRECT_{}x{}_R{}",
            millionths(w),
            millionths(h),
            millionths(r)
        ),
    }
}

/// Every shape a feature flashes, written once in the content section.
#[derive(Debug, Default)]
pub(crate) struct Dictionary {
    shapes: BTreeSet<Shape>,
}

impl Dictionary {
    /// The id of `shape`, adding it.
    pub(crate) fn reference(&mut self, shape: Shape) -> String {
        self.shapes.insert(shape);
        shape_id(shape)
    }

    pub(crate) fn write(&self, xml: &mut Xml) {
        xml.open("DictionaryStandard", &[("units", "MILLIMETER".into())]);
        for shape in &self.shapes {
            xml.open("EntryStandard", &[("id", shape_id(*shape))]);
            match *shape {
                Shape::Circle { diameter } => {
                    xml.leaf("Circle", &[("diameter", millionths(diameter))]);
                }
                Shape::Rect { w, h } => {
                    xml.leaf(
                        "RectCenter",
                        &[("width", millionths(w)), ("height", millionths(h))],
                    );
                }
                Shape::Oval { w, h } => {
                    xml.leaf(
                        "Oval",
                        &[("width", millionths(w)), ("height", millionths(h))],
                    );
                }
                Shape::RoundRect { w, h, r } => {
                    let corner = || "true".to_string();
                    xml.leaf(
                        "RectRound",
                        &[
                            ("width", millionths(w)),
                            ("height", millionths(h)),
                            ("radius", millionths(r)),
                            ("upperRight", corner()),
                            ("upperLeft", corner()),
                            ("lowerLeft", corner()),
                            ("lowerRight", corner()),
                        ],
                    );
                }
            }
            xml.close();
        }
        xml.close();
    }
}

/// A point's `x` and `y` attributes in mm.
pub(crate) fn xy(p: PointNm) -> [(&'static str, String); 2] {
    [("x", millionths(p.x)), ("y", millionths(p.y))]
}

/// A flashed shape at `at` turned `udeg`: the optional `Xform`, the
/// `Location` and the dictionary reference.
pub(crate) fn placed_shape(
    xml: &mut Xml,
    dictionary: &mut Dictionary,
    aperture: ApertureShape,
    at: PointNm,
) {
    let (shape, udeg) = Shape::of(aperture);
    if udeg != 0 {
        xml.leaf("Xform", &[("rotation", millionths(udeg))]);
    }
    xml.leaf("Location", &xy(at));
    xml.leaf(
        "StandardPrimitiveRef",
        &[("id", dictionary.reference(shape))],
    );
}

/// One layer's marks.
pub(crate) fn layer_feature(
    xml: &mut Xml,
    dictionary: &mut Dictionary,
    layer: &str,
    marks: &[Mark],
) {
    if marks.is_empty() {
        return;
    }
    xml.open("LayerFeature", &[("layerRef", layer.to_string())]);
    for mark in marks {
        write_mark(xml, dictionary, mark);
    }
    xml.close();
}

fn write_mark(xml: &mut Xml, dictionary: &mut Dictionary, mark: &Mark) {
    match mark {
        Mark::Flash {
            shape,
            function,
            at,
            attributes,
        } => {
            let mut set = net_attr(attributes.net.as_deref());
            match pad_usage(*function) {
                Some(usage) => {
                    set.push(("padUsage", usage.to_string()));
                    xml.open("Set", &set);
                    xml.open("Pad", &[]);
                    placed_shape(xml, dictionary, *shape, *at);
                    if let Some((component, pin)) = &attributes.pin {
                        xml.leaf(
                            "PinRef",
                            &[("componentRef", qualified(component)), ("pin", pin.clone())],
                        );
                    }
                    xml.close();
                }
                None => {
                    xml.open("Set", &set);
                    xml.open("Features", &[]);
                    placed_shape(xml, dictionary, *shape, *at);
                    xml.close();
                }
            }
            xml.close();
        }
        Mark::Stroke {
            width,
            start,
            steps,
            attributes,
            ..
        } => {
            xml.open("Set", &net_attr(attributes.net.as_deref()));
            xml.open("Features", &[]);
            xml.leaf("Location", &xy(PointNm::new(0, 0)));
            polyline(xml, *start, steps, *width);
            xml.close();
            xml.close();
        }
        Mark::Region {
            outline,
            holes,
            attributes,
        } => {
            if outline.len() < 3 {
                return;
            }
            xml.open("Set", &net_attr(attributes.net.as_deref()));
            xml.open("Features", &[]);
            xml.leaf("Location", &xy(PointNm::new(0, 0)));
            contour(xml, "Contour", outline, holes);
            xml.close();
            xml.close();
        }
    }
}

fn net_attr(net: Option<&str>) -> Vec<(&'static str, String)> {
    net.map(|n| vec![("net", qualified(n))]).unwrap_or_default()
}

/// The `padUsage` of a flash that is a pad; `None` for a drawing.
fn pad_usage(function: Function) -> Option<&'static str> {
    match function {
        Function::SmdPad | Function::ComponentPad | Function::ConnectorPad => Some("TERMINATION"),
        Function::ViaPad => Some("VIA"),
        Function::NonPlatedPad => Some("NONE"),
        Function::PadOpening => Some("MASK"),
        Function::Conductor
        | Function::NonConductor
        | Function::Profile
        | Function::VScore
        | Function::Graphic => None,
    }
}

/// A stroked path drawn with a round pen of `width` nm.
fn polyline(xml: &mut Xml, start: PointNm, steps: &[Step], width: i64) {
    xml.open("Polyline", &[]);
    xml.leaf("PolyBegin", &xy(start));
    let mut from = start;
    for step in steps {
        poly_step(xml, from, *step);
        from = match step {
            Step::Line(to) | Step::Arc { to, .. } => *to,
        };
    }
    xml.leaf(
        "LineDesc",
        &[
            ("lineEnd", "ROUND".into()),
            ("lineWidth", millionths(width)),
        ],
    );
    xml.close();
}

/// One step from `from`. A full circle (an arc back to its start) is two
/// half arcs: a curve that ends where it starts names no turn.
fn poly_step(xml: &mut Xml, from: PointNm, step: Step) {
    match step {
        Step::Line(to) => xml.leaf("PolyStepSegment", &xy(to)),
        Step::Arc { to, center, ccw } => {
            if to == from {
                let opposite = PointNm::new(2 * center.x - from.x, 2 * center.y - from.y);
                curve(xml, opposite, center, ccw);
            }
            curve(xml, to, center, ccw);
        }
    }
}

fn curve(xml: &mut Xml, to: PointNm, center: PointNm, ccw: bool) {
    let [x, y] = xy(to);
    xml.leaf(
        "PolyStepCurve",
        &[
            x,
            y,
            ("centerX", millionths(center.x)),
            ("centerY", millionths(center.y)),
            ("clockwise", (!ccw).to_string()),
        ],
    );
}

/// A filled outline with polygon holes, as a `Contour` or a `Profile`.
pub(crate) fn contour(
    xml: &mut Xml,
    tag: &'static str,
    outline: &[PointNm],
    holes: &[Vec<PointNm>],
) {
    xml.open(tag, &[]);
    closed_polygon(xml, "Polygon", outline);
    for hole in holes.iter().filter(|h| h.len() >= 3) {
        closed_polygon(xml, "Cutout", hole);
    }
    xml.close();
}

/// A ring as a polygon that returns to its first point.
pub(crate) fn closed_polygon(xml: &mut Xml, tag: &'static str, ring: &[PointNm]) {
    let Some((&first, rest)) = ring.split_first() else {
        return;
    };
    xml.open(tag, &[]);
    xml.leaf("PolyBegin", &xy(first));
    for p in rest {
        xml.leaf("PolyStepSegment", &xy(*p));
    }
    if ring.last() != Some(&first) {
        xml.leaf("PolyStepSegment", &xy(first));
    }
    xml.close();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fab::marks::Attributes;

    #[test]
    fn a_turned_pad_flashes_its_upright_shape_with_a_turn() {
        let mut dictionary = Dictionary::default();
        let mut xml = Xml::fragment(0);
        let pad = Mark::Flash {
            shape: ApertureShape::TurnedRect {
                w: 1_000_000,
                h: 500_000,
                udeg: 30_000_000,
            },
            function: Function::SmdPad,
            at: PointNm::new(2_000_000, -1_500_000),
            attributes: Attributes {
                net: Some("A:B".into()),
                pin: Some(("R1".into(), "2".into())),
            },
        };
        layer_feature(&mut xml, &mut dictionary, "TopCopper", &[pad]);
        let text = xml.finish();
        assert!(
            text.contains("<Set net=\"A_B\" padUsage=\"TERMINATION\">"),
            "{text}"
        );
        assert!(text.contains("<Xform rotation=\"30\"/>"), "{text}");
        assert!(text.contains("<Location x=\"2\" y=\"-1.5\"/>"), "{text}");
        assert!(
            text.contains("<StandardPrimitiveRef id=\"RECT_1x0.5\"/>"),
            "{text}"
        );
        assert!(
            text.contains("<PinRef componentRef=\"R1\" pin=\"2\"/>"),
            "{text}"
        );
        let mut entries = Xml::fragment(0);
        dictionary.write(&mut entries);
        assert!(
            entries
                .finish()
                .contains("<RectCenter width=\"1\" height=\"0.5\"/>")
        );
    }

    #[test]
    fn a_full_circle_is_two_half_arcs() {
        let mut xml = Xml::fragment(0);
        let center = PointNm::new(0, 0);
        let start = PointNm::new(1_000_000, 0);
        polyline(
            &mut xml,
            start,
            &[Step::Arc {
                to: start,
                center,
                ccw: true,
            }],
            100_000,
        );
        let text = xml.finish();
        assert_eq!(text.matches("<PolyStepCurve").count(), 2, "{text}");
        assert!(text.contains("x=\"-1\" y=\"0\""), "{text}");
        assert!(text.contains("clockwise=\"false\""), "{text}");
    }
}
