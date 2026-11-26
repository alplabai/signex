//! The miniature of a compiled sheet instance (#2658): its wires, buses and
//! part bodies with the designators and net names that instance has, for a
//! channel card. Every name is read off the compile ([`CompiledSheet`]);
//! nothing here names a part or a net.

use std::collections::HashMap;

use signex_types::schematic::{Aabb, Point, SchematicSheet, body_bounds};
use uuid::Uuid;

use crate::CompiledSheet;

/// Clear space around the drawn content, in mm.
const MARGIN_MM: f64 = 2.54;

/// What a channel card draws, in sheet mm.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetMiniature {
    /// The drawn area: the content with a margin.
    pub bounds: Aabb,
    pub wires: Vec<(Point, Point)>,
    pub buses: Vec<(Point, Point)>,
    /// The body box of each part that draws one.
    pub bodies: Vec<Aabb>,
    /// Each part's designator in this instance, at its body's centre (its
    /// origin when it draws no body).
    pub designators: Vec<MiniatureText>,
    /// Each net touching this instance, at one of its elements here.
    pub nets: Vec<MiniatureText>,
}

/// A name drawn on the miniature.
#[derive(Debug, Clone, PartialEq)]
pub struct MiniatureText {
    pub at: Point,
    pub text: String,
    /// A net named by a power port or a Power label.
    pub is_power: bool,
}

/// The miniature of `compiled`, an instance of `sheet`. `None` when the
/// sheet draws nothing a card could show.
pub fn sheet_miniature(sheet: &SchematicSheet, compiled: &CompiledSheet) -> Option<SheetMiniature> {
    // In the sheet's order, so two compiles of one sheet draw alike.
    let bodies: Vec<(Uuid, Aabb)> = sheet
        .symbols
        .iter()
        .filter(|s| !s.is_power)
        .filter_map(|s| {
            let lib = sheet.body_of(s)?;
            body_bounds(lib, s).map(|b| (s.uuid, b))
        })
        .collect();
    let bodies_by_symbol: HashMap<Uuid, Aabb> = bodies.iter().copied().collect();
    let origin_of = |id: Uuid| sheet.symbols.iter().find(|s| s.uuid == id);
    let designators: Vec<MiniatureText> = compiled
        .components
        .iter()
        .filter_map(|part| {
            let at = match bodies_by_symbol.get(&part.target.id) {
                Some(b) => Point::new((b.min_x + b.max_x) / 2.0, (b.min_y + b.max_y) / 2.0),
                None => origin_of(part.target.id)?.position,
            };
            Some(MiniatureText {
                at,
                text: part.designator.clone(),
                is_power: false,
            })
        })
        .collect();
    let nets: Vec<MiniatureText> = compiled
        .net_names
        .iter()
        .filter_map(|net| {
            Some(MiniatureText {
                at: net.target.at?,
                text: net.name.clone(),
                is_power: net.is_power,
            })
        })
        .collect();
    let miniature = SheetMiniature {
        bounds: Aabb::new(0.0, 0.0, 0.0, 0.0),
        wires: sheet.wires.iter().map(|w| (w.start, w.end)).collect(),
        buses: sheet.buses.iter().map(|b| (b.start, b.end)).collect(),
        bodies: bodies.into_iter().map(|(_, b)| b).collect(),
        designators,
        nets,
    };
    let bounds = miniature.content_bounds()?.expand(MARGIN_MM);
    Some(SheetMiniature {
        bounds,
        ..miniature
    })
}

impl SheetMiniature {
    /// The box around everything drawn, `None` when nothing is.
    fn content_bounds(&self) -> Option<Aabb> {
        let point = |p: &Point| Aabb::new(p.x, p.y, p.x, p.y);
        let lines = self.wires.iter().chain(&self.buses);
        let texts = self.designators.iter().chain(&self.nets);
        lines
            .flat_map(|(a, b)| [point(a), point(b)])
            .chain(self.bodies.iter().copied())
            .chain(texts.map(|t| point(&t.at)))
            .reduce(|a, b| a.union(&b))
    }
}
