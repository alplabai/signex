//! The placed parts of an IPC-2581 file: one `Package` per footprint kind,
//! a `Component` per placed part, the `Bom` that lists them, and a
//! `LogicalNet` per net with the pins its pads carry.

use std::collections::{BTreeMap, BTreeSet};

use pcb_geom::{Placement, PointNm, Side};
use signex_types::pcb::{Footprint, Pad, PadType, PcbBoard, Point};

use super::features::{Dictionary, placed_shape, xy};
use super::xml::{Xml, qualified};
use crate::fab::collect::pad_aperture;
use crate::fab::frame::{FabFrame, fab_angle_udeg, millionths};
use crate::fab::marks::{ApertureShape, Mark};

/// The board's parts as the file names them.
pub(crate) struct Parts<'a> {
    /// Placed parts with a designator, in board order, and their package.
    placed: Vec<(&'a Footprint, Option<String>)>,
    /// Each package's name and the footprint that shapes it.
    packages: Vec<(String, &'a Footprint)>,
}

impl<'a> Parts<'a> {
    /// The parts of `board`. A part with no designator has no component:
    /// its pads are still copper, but nothing names them.
    pub(crate) fn of(board: &'a PcbBoard) -> Self {
        let mut packages: Vec<(String, &Footprint)> = Vec::new();
        let mut placed = Vec::new();
        for footprint in board.footprints.iter().filter(|f| !f.reference.is_empty()) {
            let package = package_of(&mut packages, footprint);
            placed.push((footprint, package));
        }
        Self { placed, packages }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.placed.is_empty()
    }

    /// The `Bom`: one item per value and footprint, one designator once.
    pub(crate) fn write_bom(&self, xml: &mut Xml, bom: &str, step: &str) {
        let mut items: BTreeMap<String, Vec<(&Footprint, &Option<String>)>> = BTreeMap::new();
        let mut named = BTreeSet::new();
        for (footprint, package) in &self.placed {
            if named.insert(qualified(&footprint.reference)) {
                items
                    .entry(oem_number(footprint))
                    .or_default()
                    .push((footprint, package));
            }
        }
        xml.open("Bom", &[("name", bom.to_string())]);
        xml.open(
            "BomHeader",
            &[("assembly", step.to_string()), ("revision", "1".into())],
        );
        xml.leaf("StepRef", &[("name", step.to_string())]);
        xml.close();
        for (number, parts) in &items {
            let category = if parts[0].0.component.component_type.in_bom() {
                "ELECTRICAL"
            } else {
                "MECHANICAL"
            };
            xml.open(
                "BomItem",
                &[
                    ("OEMDesignNumberRef", number.clone()),
                    ("quantity", parts.len().to_string()),
                    ("category", category.into()),
                ],
            );
            for (footprint, package) in parts {
                let mut attrs = vec![("name", qualified(&footprint.reference))];
                if let Some(package) = package {
                    attrs.push(("packageRef", package.clone()));
                }
                attrs.push((
                    "populate",
                    footprint.component.component_type.in_bom().to_string(),
                ));
                attrs.push(("layerRef", side_layer(footprint).into()));
                xml.leaf("RefDes", &attrs);
            }
            xml.leaf("Characteristics", &[("category", category.into())]);
            xml.close();
        }
        xml.close();
    }

    /// The `Package` elements, then the `Component` elements. A part whose
    /// placement the geometry refuses is counted in `skipped`.
    pub(crate) fn write_step_parts(
        &self,
        xml: &mut Xml,
        dictionary: &mut Dictionary,
        frame: FabFrame,
        skipped: &mut usize,
    ) {
        for (name, footprint) in &self.packages {
            write_package(xml, dictionary, name, footprint);
        }
        for (footprint, package) in &self.placed {
            let Ok(placement) = Placement::of_footprint(footprint) else {
                *skipped += 1;
                continue;
            };
            let mut attrs = vec![("refDes", qualified(&footprint.reference))];
            if let Some(package) = package {
                attrs.push(("packageRef", package.clone()));
            }
            attrs.push(("part", oem_number(footprint)));
            attrs.push(("layerRef", side_layer(footprint).into()));
            attrs.push(("mountType", mount_type(footprint).into()));
            xml.open("Component", &attrs);
            let udeg = fab_angle_udeg(placement.rotation_deg);
            let mirror = placement.side == Side::Bottom;
            if udeg != 0 || mirror {
                let mut xform = vec![("rotation", millionths(udeg))];
                if mirror {
                    xform.push(("mirror", "true".into()));
                }
                xml.leaf("Xform", &xform);
            }
            xml.leaf("Location", &xy(frame.point(placement.origin)));
            xml.close();
        }
    }
}

/// One `LogicalNet` per net, listing the designated pins its copper pads
/// carry, from the copper images' pad flashes.
pub(crate) fn write_nets<'m>(xml: &mut Xml, copper: impl Iterator<Item = &'m Mark>) {
    let mut nets: BTreeMap<String, BTreeSet<(String, String)>> = BTreeMap::new();
    for mark in copper {
        if let Mark::Flash { attributes, .. } = mark
            && let (Some(net), Some((component, pin))) = (&attributes.net, &attributes.pin)
        {
            nets.entry(qualified(net))
                .or_default()
                .insert((qualified(component), pin.clone()));
        }
    }
    for (net, pins) in nets {
        xml.open("LogicalNet", &[("name", net)]);
        for (component, pin) in pins {
            xml.leaf("PinRef", &[("componentRef", component), ("pin", pin)]);
        }
        xml.close();
    }
}

/// The package a footprint uses: one per footprint name and pad set. A
/// second footprint of the same name with other pads gets `_2`, `_3`, ...
fn package_of<'a>(
    packages: &mut Vec<(String, &'a Footprint)>,
    footprint: &'a Footprint,
) -> Option<String> {
    if footprint.pads.is_empty() {
        return None;
    }
    let base = qualified(if footprint.footprint_id.trim().is_empty() {
        &footprint.reference
    } else {
        &footprint.footprint_id
    });
    let mut n = 1;
    loop {
        let name = if n == 1 {
            base.clone()
        } else {
            format!("{base}_{n}")
        };
        match packages.iter().find(|(existing, _)| *existing == name) {
            Some((_, shaped)) if same_pads(shaped, footprint) => return Some(name),
            Some(_) => n += 1,
            None => {
                packages.push((name.clone(), footprint));
                return Some(name);
            }
        }
    }
}

fn same_pads(a: &Footprint, b: &Footprint) -> bool {
    a.pads.len() == b.pads.len()
        && a.pads.iter().zip(&b.pads).all(|(p, q)| {
            p.number == q.number
                && p.shape == q.shape
                && p.pad_type == q.pad_type
                && p.position == q.position
                && p.size == q.size
                && p.rotation == q.rotation
        })
}

/// A package: its pins in the footprint's own frame (unturned, top side,
/// y up), and an outline round them.
fn write_package(xml: &mut Xml, dictionary: &mut Dictionary, name: &str, footprint: &Footprint) {
    let local = Footprint {
        position: Point::new(0.0, 0.0),
        rotation: 0.0,
        layer: "TopCopper".into(),
        ..footprint.clone()
    };
    let frame = FabFrame {
        origin: PointNm::new(0, 0),
    };
    let pins: Vec<(&Pad, ApertureShape, PointNm)> = local
        .pads
        .iter()
        .filter(|pad| !pad.number.trim().is_empty())
        .filter_map(|pad| {
            let (shape, at) = pad_aperture(frame, &local, pad, pad.size).ok()?;
            Some((pad, shape, at))
        })
        .collect();
    let mut attrs = vec![("name", name.to_string()), ("type", "OTHER".into())];
    if let Some((first, _, _)) = pins.first() {
        attrs.push(("pinOne", qualified(&first.number)));
    }
    attrs.push(("pinOneOrientation", "OTHER".into()));
    xml.open("Package", &attrs);
    let outline = pin_box(&pins);
    xml.open("Outline", &[]);
    super::features::closed_polygon(xml, "Polygon", &outline);
    xml.leaf(
        "LineDesc",
        &[("lineEnd", "ROUND".into()), ("lineWidth", "0".into())],
    );
    xml.close();
    for (pad, shape, at) in &pins {
        let kind = match pad.pad_type {
            PadType::Smd | PadType::Connect => "SURFACE",
            PadType::Thru | PadType::NpThru => "THRU",
        };
        let electrical = match pad.pad_type {
            PadType::NpThru => "MECHANICAL",
            _ => "ELECTRICAL",
        };
        xml.open(
            "Pin",
            &[
                ("number", qualified(&pad.number)),
                ("type", kind.into()),
                ("electricalType", electrical.into()),
            ],
        );
        placed_shape(xml, dictionary, *shape, *at);
        xml.close();
    }
    xml.close();
}

/// The box round the pins' shapes (each reaching half its longer side from
/// its centre), as a ring; a 1 mm box at the origin for a package with no
/// pin the geometry holds.
fn pin_box(pins: &[(&Pad, ApertureShape, PointNm)]) -> Vec<PointNm> {
    let reach = |shape: ApertureShape| -> i64 {
        match shape {
            ApertureShape::Circle { diameter } => diameter / 2,
            ApertureShape::Rect { w, h }
            | ApertureShape::Obround { w, h }
            | ApertureShape::TurnedRect { w, h, .. }
            | ApertureShape::TurnedObround { w, h, .. }
            | ApertureShape::RoundRect { w, h, .. } => w.max(h) / 2,
        }
    };
    let (mut lo, mut hi) = ((-500_000, -500_000), (500_000, 500_000));
    if let Some((_, shape, at)) = pins.first() {
        let r = reach(*shape);
        (lo, hi) = ((at.x - r, at.y - r), (at.x + r, at.y + r));
    }
    for (_, shape, at) in pins {
        let r = reach(*shape);
        lo = (lo.0.min(at.x - r), lo.1.min(at.y - r));
        hi = (hi.0.max(at.x + r), hi.1.max(at.y + r));
    }
    vec![
        PointNm::new(lo.0, lo.1),
        PointNm::new(hi.0, lo.1),
        PointNm::new(hi.0, hi.1),
        PointNm::new(lo.0, hi.1),
    ]
}

/// The BOM's part number for a footprint: its value and footprint name.
fn oem_number(footprint: &Footprint) -> String {
    let value = footprint.value.trim();
    let package = footprint.footprint_id.trim();
    match (value.is_empty(), package.is_empty()) {
        (true, true) => "UNSPECIFIED".to_string(),
        (false, true) => value.to_string(),
        (true, false) => package.to_string(),
        (false, false) => format!("{value}|{package}"),
    }
}

/// The copper layer a part sits on.
fn side_layer(footprint: &Footprint) -> &'static str {
    match Side::of_layer(&footprint.layer) {
        Side::Top => "TopCopper",
        Side::Bottom => "BottomCopper",
    }
}

fn mount_type(footprint: &Footprint) -> &'static str {
    if footprint.pads.is_empty() {
        "OTHER"
    } else if footprint
        .pads
        .iter()
        .any(|p| matches!(p.pad_type, PadType::Thru | PadType::NpThru))
    {
        "THMT"
    } else {
        "SMT"
    }
}
