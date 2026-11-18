//! The placed parts of an IPC-2581 file: one `Package` per footprint kind,
//! a `Component` per placed part, the `Bom` that lists them, and a
//! `LogicalNet` per net with the pins its pads carry. Which parts and
//! packages there are comes from `fab::parts`, shared with ODB++.

use std::collections::{BTreeMap, BTreeSet};

use pcb_geom::{Placement, PointNm, Side};
use signex_types::pcb::{Footprint, PadType};

use super::features::{Dictionary, placed_shape, xy};
use super::names::Names;
use super::xml::{Xml, qualified};
use crate::fab::frame::{FabFrame, fab_angle_udeg, millionths};
use crate::fab::marks::Mark;
use crate::fab::parts::{Parts, Placed, oem_number, package_pins, pin_box};

impl Parts<'_> {
    /// The `Bom`: one item per value and footprint, listing every placed
    /// part by its unique designator, as the components are.
    pub(crate) fn write_bom(&self, xml: &mut Xml, bom: &str, step: &str) {
        let mut items: BTreeMap<String, Vec<&Placed<'_>>> = BTreeMap::new();
        for placed in &self.placed {
            items
                .entry(oem_number(placed.footprint))
                .or_default()
                .push(placed);
        }
        xml.open("Bom", &[("name", bom.to_string())]);
        xml.open(
            "BomHeader",
            &[("assembly", step.to_string()), ("revision", "1".into())],
        );
        xml.leaf("StepRef", &[("name", step.to_string())]);
        xml.close();
        for (number, parts) in &items {
            let category = if parts[0].footprint.component.component_type.in_bom() {
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
            for placed in parts {
                let footprint = placed.footprint;
                let mut attrs = vec![("name", placed.designator.clone())];
                if let Some(package) = &placed.package {
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
        for placed in &self.placed {
            let footprint = placed.footprint;
            let Ok(placement) = Placement::of_footprint(footprint) else {
                *skipped += 1;
                continue;
            };
            let mut attrs = vec![("refDes", placed.designator.clone())];
            if let Some(package) = &placed.package {
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
pub(crate) fn write_nets<'m>(
    xml: &mut Xml,
    copper: impl Iterator<Item = &'m Mark>,
    names: &Names<'_>,
) {
    let mut nets: BTreeMap<String, BTreeSet<(String, String)>> = BTreeMap::new();
    for mark in copper {
        if let Mark::Flash { attributes, .. } = mark
            && let (Some(net), Some((_, pin))) = (&attributes.net, &attributes.pin)
            && let Some(component) = names.component(attributes)
        {
            nets.entry(names.net(net))
                .or_default()
                .insert((component, pin.clone()));
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

/// A package: its pins in the footprint's own frame (unturned, top side,
/// y up), and an outline round them.
fn write_package(xml: &mut Xml, dictionary: &mut Dictionary, name: &str, footprint: &Footprint) {
    let pins = package_pins(footprint);
    let mut attrs = vec![("name", name.to_string()), ("type", "OTHER".into())];
    if let Some((first, _, _)) = pins.first() {
        attrs.push(("pinOne", qualified(&first.number)));
    }
    attrs.push(("pinOneOrientation", "OTHER".into()));
    xml.open("Package", &attrs);
    let (lo, hi) = pin_box(&pins);
    let outline = [lo, PointNm::new(hi.x, lo.y), hi, PointNm::new(lo.x, hi.y)];
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
        placed_shape(xml, dictionary, shape, *at);
        xml.close();
    }
    xml.close();
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
