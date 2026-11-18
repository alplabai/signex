//! The ODB++ `eda/data` file (the packages, and the nets with each
//! subnet pointing at the copper features it is made of) and the two
//! component layers' `components` files, whose toeprints name their net
//! and subnet.

use std::collections::BTreeMap;
use std::ops::Range;

use pcb_geom::{Placement, PointNm, Side, mm_to_nm};
use signex_types::pcb::{Footprint, PadType};

use super::features::ring;
use super::text::{cw_degrees, mm, word};
use crate::fab::collect::pad_aperture;
use crate::fab::frame::{FabFrame, fab_angle_udeg};
use crate::fab::layers::Face;
use crate::fab::marks::{ApertureShape, Function, Mark, PadKey};
use crate::fab::naming::unique_names;
use crate::fab::parts::{PackagePin, Parts, oem_number, package_pins, pin_box};

/// The net a toeprint whose pad carries none is listed under.
const NO_NET: &str = "$NONE$";

/// One copper layer's marks and the feature numbers each became.
pub(crate) struct CopperFeatures<'m> {
    pub name: String,
    pub marks: Vec<(&'m Mark, Range<usize>)>,
}

/// A copper feature: its layer's place in the `LYR` list and its number.
type Fid = (usize, usize);

struct Toeprint {
    pin: usize,
    name: String,
    at: PointNm,
    net: usize,
    subnet: usize,
}

struct Component {
    package: usize,
    name: String,
    part: String,
    at: PointNm,
    udeg: i64,
    toeprints: Vec<Toeprint>,
}

enum SubnetKind {
    Toeprint {
        face: Face,
        component: usize,
        toeprint: usize,
    },
    Trace,
    Via,
    Plane,
}

struct Subnet {
    kind: SubnetKind,
    fids: Vec<Fid>,
}

#[derive(Default)]
struct NetCopper {
    toeprints: Vec<Subnet>,
    trace: Vec<Fid>,
    via: Vec<Fid>,
    planes: Vec<Vec<Fid>>,
}

/// The nets, packages and placed components of one step.
pub(crate) struct Eda<'a> {
    layers: Vec<String>,
    packages: &'a [(String, &'a Footprint)],
    nets: Vec<(String, Vec<Subnet>)>,
    components: [Vec<Component>; 2],
}

impl<'a> Eda<'a> {
    /// The nets from the copper features' net and pin attributes, and the
    /// components of `parts`. A part whose placement the geometry refuses
    /// is counted in `skipped`; a part with no pads has no package and is
    /// left out.
    pub(crate) fn of(
        parts: &'a Parts<'a>,
        frame: FabFrame,
        copper: &[CopperFeatures<'_>],
        skipped: &mut usize,
    ) -> Self {
        let mut nets: BTreeMap<String, NetCopper> = BTreeMap::new();
        // Keyed by the pad itself: a designator and pad number repeat (two
        // pads numbered alike, a panel's copies of one part).
        let mut pins: BTreeMap<PadKey, (Option<String>, Vec<Fid>)> = BTreeMap::new();
        for (layer, features) in copper.iter().enumerate() {
            for (mark, range) in &features.marks {
                let fids = range.clone().map(|f| (layer, f));
                match mark {
                    Mark::Flash {
                        function,
                        attributes,
                        ..
                    } => match (attributes.pad, &attributes.net) {
                        (Some(pad), net) => {
                            let entry = pins.entry(pad).or_default();
                            entry.0 = entry.0.take().or_else(|| net.clone());
                            entry.1.extend(fids);
                        }
                        (None, Some(net)) => {
                            let net_copper = nets.entry(net.clone()).or_default();
                            if *function == Function::ViaPad {
                                net_copper.via.extend(fids);
                            } else {
                                net_copper.trace.extend(fids);
                            }
                        }
                        (None, None) => {}
                    },
                    Mark::Stroke { attributes, .. } => {
                        if let Some(net) = &attributes.net {
                            nets.entry(net.clone()).or_default().trace.extend(fids);
                        }
                    }
                    Mark::Region { attributes, .. } => {
                        if let Some(net) = &attributes.net {
                            nets.entry(net.clone())
                                .or_default()
                                .planes
                                .push(fids.collect());
                        }
                    }
                }
            }
        }
        let mut components: [Vec<Component>; 2] = Default::default();
        for placed in &parts.placed {
            let (index, footprint) = (placed.index, placed.footprint);
            let Some(package) = placed
                .package
                .as_ref()
                .and_then(|name| parts.packages.iter().position(|(n, _)| n == name))
            else {
                continue;
            };
            let Ok(placement) = Placement::of_footprint(footprint) else {
                *skipped += 1;
                continue;
            };
            let face = match placement.side {
                Side::Top => Face::Top,
                Side::Bottom => Face::Bottom,
            };
            let component = components[side(face)].len();
            let mut toeprints = Vec::new();
            for (pin, (pad, _, _)) in package_pins(footprint).into_iter().enumerate() {
                let Ok((_, at)) = pad_aperture(frame, footprint, pad, pad.size) else {
                    continue;
                };
                let pad_index = footprint.pads.iter().position(|p| std::ptr::eq(p, pad));
                let (net, fids) = pad_index
                    .and_then(|pad_index| pins.remove(&(index, pad_index)))
                    .unwrap_or_default();
                let kind = SubnetKind::Toeprint {
                    face,
                    component,
                    toeprint: toeprints.len(),
                };
                nets.entry(net.unwrap_or_else(|| NO_NET.to_string()))
                    .or_default()
                    .toeprints
                    .push(Subnet { kind, fids });
                toeprints.push(Toeprint {
                    pin,
                    name: word(&pad.number),
                    at,
                    net: 0,
                    subnet: 0,
                });
            }
            components[side(face)].push(Component {
                package,
                name: placed.designator.clone(),
                part: word(&oem_number(footprint)),
                at: frame.point(placement.origin),
                udeg: fab_angle_udeg(placement.rotation_deg),
                toeprints,
            });
        }
        // A pad no toeprint stands for (no number, or a part with no
        // designator or no package) is still copper of its net.
        for (net, fids) in pins.into_values() {
            if let Some(net) = net {
                nets.entry(net).or_default().trace.extend(fids);
            }
        }
        // Net names are unique: `VCC 3V3` and `VCC_3V3` are one word.
        let names = unique_names(nets.keys().map(String::as_str), word);
        let nets = nets
            .into_iter()
            .enumerate()
            .map(|(number, (name, net_copper))| {
                let subnets = subnets(number, net_copper, &mut components);
                (names[&name].clone(), subnets)
            })
            .collect();
        Self {
            layers: copper.iter().map(|c| c.name.clone()).collect(),
            packages: &parts.packages,
            nets,
            components,
        }
    }

    /// The `eda/data` file.
    pub(crate) fn data(&self) -> String {
        let mut out = format!("UNITS=MM\nHDR Signex\nLYR {}\n#\n", self.layers.join(" "));
        for (number, (name, subnets)) in self.nets.iter().enumerate() {
            out.push_str(&format!("# NET {number}\nNET {name}\n"));
            for subnet in subnets {
                match subnet.kind {
                    SubnetKind::Toeprint {
                        face,
                        component,
                        toeprint,
                    } => {
                        let side = if face == Face::Top { "T" } else { "B" };
                        out.push_str(&format!("SNT TOP {side} {component} {toeprint}\n"));
                    }
                    SubnetKind::Trace => out.push_str("SNT TRC\n"),
                    SubnetKind::Via => out.push_str("SNT VIA\n"),
                    SubnetKind::Plane => out.push_str("SNT PLN S E 0\n"),
                }
                for (layer, feature) in &subnet.fids {
                    out.push_str(&format!("FID C {layer} {feature}\n"));
                }
            }
            out.push_str("#\n");
        }
        for (number, (name, footprint)) in self.packages.iter().enumerate() {
            out.push_str(&format!("# PKG {number}\n"));
            write_package(&mut out, name, footprint);
            out.push_str("#\n");
        }
        out
    }

    /// A side's `components` file.
    pub(crate) fn components(&self, face: Face) -> String {
        let mirror = if face == Face::Bottom { "M" } else { "N" };
        let mut out = String::from("UNITS=MM\n#\n");
        for (number, component) in self.components[side(face)].iter().enumerate() {
            let rotation = cw_degrees(component.udeg);
            out.push_str(&format!(
                "# CMP {number}\nCMP {} {} {} {rotation} {mirror} {} {}\n",
                component.package,
                mm(component.at.x),
                mm(component.at.y),
                component.name,
                component.part
            ));
            for toeprint in &component.toeprints {
                out.push_str(&format!(
                    "TOP {} {} {} {rotation} {mirror} {} {} {}\n",
                    toeprint.pin,
                    mm(toeprint.at.x),
                    mm(toeprint.at.y),
                    toeprint.net,
                    toeprint.subnet,
                    toeprint.name
                ));
            }
            out.push_str("#\n");
        }
        out
    }
}

/// A net's subnets in order (its toeprints, traces, vias, then each
/// plane), telling each toeprint its net and subnet number.
fn subnets(net: usize, copper: NetCopper, components: &mut [Vec<Component>; 2]) -> Vec<Subnet> {
    let mut subnets = copper.toeprints;
    for (number, subnet) in subnets.iter().enumerate() {
        if let SubnetKind::Toeprint {
            face,
            component,
            toeprint,
        } = subnet.kind
        {
            let toeprint = &mut components[side(face)][component].toeprints[toeprint];
            toeprint.net = net;
            toeprint.subnet = number;
        }
    }
    let mut add = |kind: SubnetKind, fids: Vec<Fid>| {
        if !fids.is_empty() {
            subnets.push(Subnet { kind, fids });
        }
    };
    add(SubnetKind::Trace, copper.trace);
    add(SubnetKind::Via, copper.via);
    for plane in copper.planes {
        add(SubnetKind::Plane, plane);
    }
    subnets
}

fn side(face: Face) -> usize {
    match face {
        Face::Top => 0,
        Face::Bottom => 1,
    }
}

/// A `PKG` record: the pins' box, its outline, then each pin with its
/// outline, in the footprint's own frame.
fn write_package(out: &mut String, name: &str, footprint: &Footprint) {
    let pins = package_pins(footprint);
    let (lo, hi) = pin_box(&pins);
    out.push_str(&format!(
        "PKG {name} {} {} {} {} {}\n",
        mm(pitch(&pins).unwrap_or((hi.x - lo.x).max(hi.y - lo.y))),
        mm(lo.x),
        mm(lo.y),
        mm(hi.x),
        mm(hi.y)
    ));
    out.push_str(&rect(lo, hi.x - lo.x, hi.y - lo.y));
    for (pad, shape, at) in &pins {
        let (kind, mount) = match pad.pad_type {
            PadType::Smd | PadType::Connect => ("S", "S"),
            PadType::Thru => ("T", "T"),
            PadType::NpThru => ("T", "H"),
        };
        let electrical = if pad.pad_type == PadType::NpThru {
            "M"
        } else {
            "E"
        };
        let hole = pad
            .drill
            .as_ref()
            .filter(|_| kind == "T")
            .and_then(|d| mm_to_nm(d.diameter, "pad.drill").ok())
            .map_or(0, i64::abs);
        out.push_str(&format!(
            "PIN {} {kind} {} {} {} {electrical} {mount}\n",
            word(&pad.number),
            mm(at.x),
            mm(at.y),
            mm(hole)
        ));
        out.push_str(&pin_outline(shape, *at));
    }
}

/// The closest two pins' centres are apart; `None` under two pins.
fn pitch(pins: &[PackagePin<'_>]) -> Option<i64> {
    let mut closest: Option<i64> = None;
    for (i, (_, _, a)) in pins.iter().enumerate() {
        for (_, _, b) in &pins[i + 1..] {
            let d = ((b.x - a.x) as f64).hypot((b.y - a.y) as f64).round() as i64;
            closest = Some(closest.map_or(d, |c| c.min(d)));
        }
    }
    closest
}

/// A pin's outline: a circle for a round pin, a contour for a polygon
/// pin, else its box (a turned pin's square reaching its longer side).
fn pin_outline(shape: &ApertureShape, at: PointNm) -> String {
    let (w, h) = match *shape {
        ApertureShape::Circle { diameter } => {
            return format!("CR {} {} {}\n", mm(at.x), mm(at.y), mm(diameter / 2));
        }
        ApertureShape::Outline { ref corners } if corners.len() >= 3 => {
            let points: Vec<PointNm> = corners
                .iter()
                .map(|c| PointNm::new(at.x + c.x, at.y + c.y))
                .collect();
            let mut out = String::from("CT\n");
            ring(&mut out, &points, false);
            out.push_str("CE\n");
            return out;
        }
        // Fewer than three corners: a point, as the copper writes none.
        ApertureShape::Outline { .. } => (0, 0),
        ApertureShape::Rect { w, h } | ApertureShape::Obround { w, h } => (w, h),
        ApertureShape::RoundRect { w, h, udeg, .. } if udeg % 180_000_000 == 0 => (w, h),
        ApertureShape::TurnedRect { w, h, .. }
        | ApertureShape::TurnedObround { w, h, .. }
        | ApertureShape::RoundRect { w, h, .. } => (w.max(h), w.max(h)),
    };
    rect(PointNm::new(at.x - w / 2, at.y - h / 2), w, h)
}

/// An `RC` outline: lower-left corner, width and height.
fn rect(lower_left: PointNm, w: i64, h: i64) -> String {
    format!(
        "RC {} {} {} {}\n",
        mm(lower_left.x),
        mm(lower_left.y),
        mm(w),
        mm(h)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_polygon_pin_is_a_contour_and_a_box_pin_a_rectangle() {
        let corners = vec![
            PointNm::new(-500_000, -500_000),
            PointNm::new(500_000, -500_000),
            PointNm::new(500_000, 500_000),
            PointNm::new(-500_000, 500_000),
        ];
        let at = PointNm::new(1_000_000, 0);
        assert_eq!(
            pin_outline(&ApertureShape::Outline { corners }, at),
            "CT\nOB 0.5 0.5 I\nOS 1.5 0.5\nOS 1.5 -0.5\nOS 0.5 -0.5\nOS 0.5 0.5\nOE\nCE\n"
        );
        let rect_pin = ApertureShape::Rect {
            w: 1_000_000,
            h: 500_000,
        };
        assert_eq!(pin_outline(&rect_pin, at), "RC 0.5 -0.25 1 0.5\n");
    }
}
