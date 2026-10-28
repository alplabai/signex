//! Everything on one net, found from a single point — the "highlight and fit
//! the whole net" input for a net-level ERC finding.

use signex_types::schematic::{Point, SchematicSheet};
use uuid::Uuid;

use super::merged_connectivity;

/// The wires, junctions and labels electrically on one net, plus the box that
/// holds them. Pins are not listed: a caller that already knows the net's pins
/// adds their tips to [`NetExtent::bounds`] itself.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NetExtent {
    pub wires: Vec<Uuid>,
    pub junctions: Vec<Uuid>,
    pub labels: Vec<Uuid>,
    /// `[min_x, min_y, max_x, max_y]` over every wire end, junction and label
    /// on the net; `None` when the net has none of them.
    pub bounds: Option<[f64; 4]>,
}

/// The wires, junctions and labels on the same net as `anchor` (a point on the
/// net, e.g. one of its pin tips). Uses the same [`merged_connectivity`] as
/// [`build_netlist`](super::build_netlist), so it follows the real net —
/// including a same-name label merge — exactly like the net-colour flood.
pub fn net_extent_at(sheet: &SchematicSheet, anchor: &Point) -> NetExtent {
    let mut conn = merged_connectivity(sheet);
    let root = conn.root_of(anchor);
    let mut out = NetExtent::default();
    let grow = |p: &Point, bounds: &mut Option<[f64; 4]>| {
        let b = bounds.get_or_insert([p.x, p.y, p.x, p.y]);
        b[0] = b[0].min(p.x);
        b[1] = b[1].min(p.y);
        b[2] = b[2].max(p.x);
        b[3] = b[3].max(p.y);
    };
    for w in &sheet.wires {
        if conn.root_of(&w.start) == root {
            out.wires.push(w.uuid);
            grow(&w.start, &mut out.bounds);
            grow(&w.end, &mut out.bounds);
        }
    }
    for j in &sheet.junctions {
        if conn.root_of(&j.position) == root {
            out.junctions.push(j.uuid);
            grow(&j.position, &mut out.bounds);
        }
    }
    for l in &sheet.labels {
        if conn.root_of(&l.position) == root {
            out.labels.push(l.uuid);
            grow(&l.position, &mut out.bounds);
        }
    }
    out
}
