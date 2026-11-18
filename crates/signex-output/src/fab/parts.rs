//! The board's placed parts as the whole-job exports (IPC-2581, ODB++)
//! name them: the designated parts in board order, one package per
//! footprint name and pad set, and each package's pins in its own frame.

use pcb_geom::PointNm;
use signex_types::pcb::{Footprint, Pad, PcbBoard, Point};

use super::collect::pad_aperture;
use super::frame::FabFrame;
use super::marks::ApertureShape;

/// One pin of a package: its pad, its shape and its centre in the
/// footprint's own frame (unturned, top side, y up).
pub(crate) type PackagePin<'a> = (&'a Pad, ApertureShape, PointNm);

/// The board's parts.
pub(crate) struct Parts<'a> {
    /// Placed parts with a designator, in board order, and their package.
    pub(crate) placed: Vec<(&'a Footprint, Option<String>)>,
    /// Each package's name and the footprint that shapes it.
    pub(crate) packages: Vec<(String, &'a Footprint)>,
}

impl<'a> Parts<'a> {
    /// The parts of `board`, packages named by `name` from the footprint
    /// name. A part with no designator has no component: its pads are
    /// still copper, but nothing names them.
    pub(crate) fn of(board: &'a PcbBoard, name: fn(&str) -> String) -> Self {
        let mut packages: Vec<(String, &Footprint)> = Vec::new();
        let mut placed = Vec::new();
        for footprint in board.footprints.iter().filter(|f| !f.reference.is_empty()) {
            let package = package_of(&mut packages, footprint, name);
            placed.push((footprint, package));
        }
        Self { placed, packages }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.placed.is_empty()
    }
}

/// The package a footprint uses: one per footprint name and pad set. A
/// second footprint of the same name with other pads gets `_2`, `_3`, ...
fn package_of<'a>(
    packages: &mut Vec<(String, &'a Footprint)>,
    footprint: &'a Footprint,
    name: fn(&str) -> String,
) -> Option<String> {
    if footprint.pads.is_empty() {
        return None;
    }
    let base = name(if footprint.footprint_id.trim().is_empty() {
        &footprint.reference
    } else {
        &footprint.footprint_id
    });
    let mut n = 1;
    loop {
        let candidate = if n == 1 {
            base.clone()
        } else {
            format!("{base}_{n}")
        };
        match packages.iter().find(|(existing, _)| *existing == candidate) {
            Some((_, shaped)) if same_pads(shaped, footprint) => return Some(candidate),
            Some(_) => n += 1,
            None => {
                packages.push((candidate.clone(), footprint));
                return Some(candidate);
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

/// The numbered pads of a footprint the geometry holds, in pad order, in
/// the footprint's own frame. A pin's index here is its package pin index.
pub(crate) fn package_pins(footprint: &Footprint) -> Vec<PackagePin<'_>> {
    let local = Footprint {
        position: Point::new(0.0, 0.0),
        rotation: 0.0,
        layer: "TopCopper".into(),
        ..footprint.clone()
    };
    let frame = FabFrame {
        origin: PointNm::new(0, 0),
    };
    footprint
        .pads
        .iter()
        .zip(&local.pads)
        .filter(|(pad, _)| !pad.number.trim().is_empty())
        .filter_map(|(pad, local_pad)| {
            let (shape, at) = pad_aperture(frame, &local, local_pad, local_pad.size).ok()?;
            Some((pad, shape, at))
        })
        .collect()
}

/// The box round the pins' shapes (each reaching half its longer side from
/// its centre) as its low and high corners; a 1 mm box at the origin for a
/// package with no pin the geometry holds.
pub(crate) fn pin_box(pins: &[PackagePin<'_>]) -> (PointNm, PointNm) {
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
    (PointNm::new(lo.0, lo.1), PointNm::new(hi.0, hi.1))
}

/// The part number for a footprint: its value and footprint name.
pub(crate) fn oem_number(footprint: &Footprint) -> String {
    let value = footprint.value.trim();
    let package = footprint.footprint_id.trim();
    match (value.is_empty(), package.is_empty()) {
        (true, true) => "UNSPECIFIED".to_string(),
        (false, true) => value.to_string(),
        (true, false) => package.to_string(),
        (false, false) => format!("{value}|{package}"),
    }
}
