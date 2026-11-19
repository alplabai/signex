//! Lookups and geometry over the signal harness model (#1280) beyond the
//! accessors `harness.rs` defines: finding an object by id, moving and turning
//! a connector, its drawn outline, and the names a new connector or entry
//! takes. The model itself and its persistence live in `harness.rs`; the
//! connectivity rules live in `signex-net` (`docs/SIGNAL_HARNESSES.md`).

use uuid::Uuid;

use super::{
    Aabb, HARNESS_TIP_MM, HarnessConnector, HarnessDefinition, HarnessEntry, HarnessWire, Point,
    SCHEMATIC_RENDER_BUS_STROKE_MM, SheetHarness,
};

/// Harness wire stroke width (mm). The mockup draws a harness wire 5 px
/// against a 4 px bus, so it is 5/4 of the bus stroke under the same law.
pub const SCHEMATIC_HARNESS_STROKE_MM: f64 = SCHEMATIC_RENDER_BUS_STROKE_MM * 1.25;

// A harness wire stays visibly thicker than a bus, on the canvas and in exports.
const _: () = assert!(SCHEMATIC_HARNESS_STROKE_MM > SCHEMATIC_RENDER_BUS_STROKE_MM);

/// Connector and entry outline width (mm), as the label marker outline.
pub const SCHEMATIC_HARNESS_OUTLINE_MM: f64 = 0.2;

/// Outline of a nested entry (a harness inside a harness): heavier (mm).
pub const SCHEMATIC_HARNESS_NESTED_OUTLINE_MM: f64 = 0.35;

/// Hit tolerance around a harness wire (mm).
pub const SCHEMATIC_HIT_HARNESS_TOLERANCE_MM: f64 = 0.7;

/// Width of a new connector body (mm): four coarse grid steps.
pub const DEFAULT_HARNESS_CONNECTOR_WIDTH_MM: f64 = 10.16;

/// The type a connector takes when the project defines none and none was
/// used before.
pub const FALLBACK_HARNESS_TYPE: &str = "HARNESS";

/// Length of an entry tab drawn on the flat side, along the flat-to-tip
/// direction (mm).
pub const HARNESS_ENTRY_TAB_MM: f64 = 1.27;

impl SheetHarness {
    /// The connector `id`.
    pub fn connector(&self, id: Uuid) -> Option<&HarnessConnector> {
        self.connectors.iter().find(|c| c.uuid == id)
    }

    /// The connector `id`, mutably.
    pub fn connector_mut(&mut self, id: Uuid) -> Option<&mut HarnessConnector> {
        self.connectors.iter_mut().find(|c| c.uuid == id)
    }

    /// The harness wire `id`.
    pub fn wire(&self, id: Uuid) -> Option<&HarnessWire> {
        self.wires.iter().find(|w| w.uuid == id)
    }

    /// The harness wire `id`, mutably.
    pub fn wire_mut(&mut self, id: Uuid) -> Option<&mut HarnessWire> {
        self.wires.iter_mut().find(|w| w.uuid == id)
    }

    /// The connector holding the entry `id`, with the entry's index.
    pub fn entry(&self, id: Uuid) -> Option<(&HarnessConnector, usize)> {
        self.connectors
            .iter()
            .find_map(|c| c.entry_index(id).map(|i| (c, i)))
    }

    /// The connector holding the entry `id`, mutably, with the entry's index.
    pub fn entry_owner_mut(&mut self, id: Uuid) -> Option<(&mut HarnessConnector, usize)> {
        self.connectors
            .iter_mut()
            .find_map(|c| c.entry_index(id).map(|i| (c, i)))
    }

    /// Whether any harness object (connector, entry or wire) carries `id`.
    pub fn holds_id(&self, id: Uuid) -> bool {
        self.connector(id).is_some() || self.wire(id).is_some() || self.entry(id).is_some()
    }

    /// The harness type of the sheet entry `pin`, when it is harness-typed.
    pub fn entry_type(&self, pin: Uuid) -> Option<&str> {
        self.entry_types.get(&pin).map(String::as_str)
    }

    /// A point on the harness object `item` to anchor on: a connector's tip,
    /// an entry's point, a wire's midpoint. `None` for any other kind.
    pub fn item_anchor(&self, item: &super::SelectedItem) -> Option<Point> {
        use super::SelectedKind;
        match item.kind {
            SelectedKind::HarnessConnector => self.connector(item.uuid).map(|c| c.tip_point()),
            SelectedKind::HarnessEntry => self.entry(item.uuid).map(|(c, i)| c.entry_point(i)),
            SelectedKind::HarnessWire => self
                .wire(item.uuid)
                .map(|w| Point::new((w.start.x + w.end.x) / 2.0, (w.start.y + w.end.y) / 2.0)),
            _ => None,
        }
    }

    /// The box around every connector and harness wire; `None` without any.
    pub fn extent(&self) -> Option<Aabb> {
        self.connectors
            .iter()
            .map(HarnessConnector::bounds)
            .chain(self.wires.iter().map(HarnessWire::bounds))
            .reduce(|a, b| a.union(&b))
    }

    /// Every uuid the harness objects carry, entries included.
    pub fn ids(&self) -> Vec<Uuid> {
        let mut ids = Vec::new();
        for c in &self.connectors {
            ids.push(c.uuid);
            ids.extend(c.entries.iter().map(|e| e.uuid));
        }
        ids.extend(self.wires.iter().map(|w| w.uuid));
        ids
    }
}

impl HarnessConnector {
    /// The index of the entry `id`.
    pub fn entry_index(&self, id: Uuid) -> Option<usize> {
        self.entries.iter().position(|e| e.uuid == id)
    }

    /// The entry named `name`.
    pub fn entry_named(&self, name: &str) -> Option<&HarnessEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    /// Move the connector by `(dx, dy)`.
    pub fn translate(&mut self, dx: f64, dy: f64) {
        self.position = Point::new(self.position.x + dx, self.position.y + dy);
    }

    /// The centre of the body.
    pub fn centre(&self) -> Point {
        let b = self.bounds();
        Point::new((b.min_x + b.max_x) / 2.0, (b.min_y + b.max_y) / 2.0)
    }

    /// Unit vector from the flat side toward the tip, snapped to an axis.
    pub fn tip_direction(&self) -> (f64, f64) {
        let c = self.centre();
        let t = self.tip_point();
        axis_of(t.x - c.x, t.y - c.y)
    }

    /// The outline as a closed polygon (first point not repeated): the body
    /// from the flat side, then the two edges that meet at the tip point.
    pub fn outline(&self) -> Vec<Point> {
        let b = self.bounds();
        let (ux, uy) = self.tip_direction();
        // Length along the tip direction (body and tip) and across it.
        let (along, across) = if ux != 0.0 {
            (b.width(), b.height())
        } else {
            (b.height(), b.width())
        };
        let body = (along - HARNESS_TIP_MM).max(0.0);
        // Corners in the local frame (flat side at s = 0, tip at s = along),
        // mapped back onto the bounds.
        let local = [
            (0.0, 0.0),
            (body, 0.0),
            (along, across / 2.0),
            (body, across),
            (0.0, across),
        ];
        local
            .iter()
            .map(|&(s, t)| frame_point(&b, (ux, uy), s, t))
            .collect()
    }

    /// Whether `p` lies on the body (its bounds).
    pub fn body_contains(&self, p: Point) -> bool {
        self.bounds().contains(p.x, p.y)
    }

    /// The tab drawn for entry `index`: a small closed polygon that sits on
    /// the flat side and points toward the tip.
    pub fn entry_tab(&self, index: usize) -> Vec<Point> {
        let e = self.entry_point(index);
        let (ux, uy) = self.tip_direction();
        let (nx, ny) = (-uy, ux);
        let half = (self.entry_pitch * 0.25).clamp(0.3, 0.8);
        let len = HARNESS_ENTRY_TAB_MM;
        let at = |s: f64, t: f64| Point::new(e.x + ux * s + nx * t, e.y + uy * s + ny * t);
        vec![
            at(0.0, -half),
            at(len * 0.65, -half),
            at(len, 0.0),
            at(len * 0.65, half),
            at(0.0, half),
        ]
    }

    /// Where the name of entry `index` is written: just past its tab.
    pub fn entry_label_point(&self, index: usize) -> Point {
        let e = self.entry_point(index);
        let (ux, uy) = self.tip_direction();
        let gap = HARNESS_ENTRY_TAB_MM + 0.4;
        Point::new(e.x + ux * gap, e.y + uy * gap)
    }

    /// The slot a new entry clicked at `p` takes: the index whose entry
    /// point, once inserted, lies nearest `p`.
    pub fn slot_at(&self, p: Point) -> usize {
        (0..=self.entries.len())
            .min_by(|&a, &b| {
                let da = dist2(self.entry_point(a), p);
                let db = dist2(self.entry_point(b), p);
                da.total_cmp(&db)
            })
            .unwrap_or(0)
    }

    /// The connector after `map` moves every point of it (a rotation or a
    /// mirror about some centre). The tip and the body centre fix the new
    /// turn; a map that swaps the flat side's ends (a mirror along the
    /// flat-to-tip axis) reverses the entries so each keeps its wire.
    pub fn mapped(&self, map: impl Fn(Point) -> Point) -> Self {
        let tip = map(self.tip_point());
        let centre = map(self.centre());
        let want = axis_of(tip.x - centre.x, tip.y - centre.y);
        let mut out = self.clone();
        out.position = Point::new(0.0, 0.0);
        out.rotation = [0.0, 90.0, 180.0, 270.0]
            .into_iter()
            .max_by(|a, b| {
                let score = |r: f64| {
                    let mut probe = out.clone();
                    probe.rotation = r;
                    let d = probe.tip_direction();
                    d.0 * want.0 + d.1 * want.1
                };
                score(*a).total_cmp(&score(*b))
            })
            .unwrap_or(0.0);
        let at_origin = out.tip_point();
        out.position = Point::new(tip.x - at_origin.x, tip.y - at_origin.y);
        let n = out.entries.len();
        if n >= 2 {
            let first = map(self.entry_point(0));
            if dist2(out.entry_point(n - 1), first) < dist2(out.entry_point(0), first) {
                out.entries.reverse();
            }
        }
        out
    }

    /// Every point that moves with the connector, for a group's centre.
    pub fn anchor_points(&self) -> Vec<Point> {
        let b = self.bounds();
        vec![Point::new(b.min_x, b.min_y), Point::new(b.max_x, b.max_y)]
    }
}

impl HarnessWire {
    /// Move both ends by `(dx, dy)`.
    pub fn translate(&mut self, dx: f64, dy: f64) {
        self.start = Point::new(self.start.x + dx, self.start.y + dy);
        self.end = Point::new(self.end.x + dx, self.end.y + dy);
    }

    /// Both ends after `map`.
    pub fn map_points(&mut self, map: impl Fn(Point) -> Point) {
        self.start = map(self.start);
        self.end = map(self.end);
    }

    /// The segment's box.
    pub fn bounds(&self) -> Aabb {
        Aabb::new(self.start.x, self.start.y, self.end.x, self.end.y)
    }
}

impl HarnessDefinition {
    /// How many entries the definition lists.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// The entry names, in order.
    pub fn entry_names(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.name.as_str()).collect()
    }
}

/// The definition named `name`.
pub fn harness_definition<'a>(
    definitions: &'a [HarnessDefinition],
    name: &str,
) -> Option<&'a HarnessDefinition> {
    definitions.iter().find(|d| d.name == name)
}

/// The type a new connector takes: the last one used, else the project's
/// first definition, else [`FALLBACK_HARNESS_TYPE`].
pub fn new_connector_type(last_used: Option<&str>, definitions: &[HarnessDefinition]) -> String {
    last_used
        .filter(|t| !t.trim().is_empty())
        .map(str::to_string)
        .or_else(|| definitions.first().map(|d| d.name.clone()))
        .unwrap_or_else(|| FALLBACK_HARNESS_TYPE.to_string())
}

/// The entries a new connector of type `harness_type` starts with: one per
/// entry of its definition, in order; none without a definition.
pub fn seeded_entries(
    harness_type: &str,
    definitions: &[HarnessDefinition],
    mut new_id: impl FnMut() -> Uuid,
) -> Vec<HarnessEntry> {
    harness_definition(definitions, harness_type)
        .map(|d| {
            d.entries
                .iter()
                .map(|e| HarnessEntry {
                    uuid: new_id(),
                    name: e.name.clone(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The name a new entry on `connector` takes: the first entry of its
/// definition the connector lacks, else the first free `ENTRY_n`.
pub fn next_entry_name(connector: &HarnessConnector, definitions: &[HarnessDefinition]) -> String {
    let used = |name: &str| connector.entries.iter().any(|e| e.name == name);
    if let Some(def) = harness_definition(definitions, &connector.harness_type)
        && let Some(free) = def.entries.iter().find(|e| !used(&e.name))
    {
        return free.name.clone();
    }
    (1..)
        .map(|n| format!("ENTRY_{n}"))
        .find(|name| !used(name))
        .unwrap_or_else(|| "ENTRY".to_string())
}

/// How many connectors of the sheets use the type `name`.
pub fn harness_type_uses<'a>(
    sheets: impl IntoIterator<Item = &'a SheetHarness>,
    name: &str,
) -> usize {
    sheets
        .into_iter()
        .flat_map(|h| h.connectors.iter())
        .filter(|c| c.harness_type == name)
        .count()
}

/// `(dx, dy)` snapped to the nearest axis unit vector; `(1, 0)` for a zero
/// vector.
fn axis_of(dx: f64, dy: f64) -> (f64, f64) {
    if dx.abs() >= dy.abs() {
        (if dx < 0.0 { -1.0 } else { 1.0 }, 0.0)
    } else {
        (0.0, if dy < 0.0 { -1.0 } else { 1.0 })
    }
}

/// The point at `s` along `(ux, uy)` from the flat side of `b` and `t`
/// across it, measured from the corner where the frame starts.
fn frame_point(b: &Aabb, (ux, uy): (f64, f64), s: f64, t: f64) -> Point {
    match (ux as i8, uy as i8) {
        // Tip right: flat side at min_x, across runs down.
        (1, 0) => Point::new(b.min_x + s, b.min_y + t),
        // Tip left: flat side at max_x.
        (-1, 0) => Point::new(b.max_x - s, b.min_y + t),
        // Tip down: flat side at min_y, across runs right.
        (0, 1) => Point::new(b.min_x + t, b.min_y + s),
        // Tip up: flat side at max_y.
        _ => Point::new(b.min_x + t, b.max_y - s),
    }
}

fn dist2(a: Point, b: Point) -> f64 {
    (a.x - b.x).powi(2) + (a.y - b.y).powi(2)
}

#[cfg(test)]
#[path = "harness_ops_tests.rs"]
mod tests;
