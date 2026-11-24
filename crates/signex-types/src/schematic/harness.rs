//! Signal harnesses (#1279): a harness connector gathers named signals into
//! one harness, a harness wire carries the harness between connectors and
//! sheet entries, and a project-wide harness definition lists what a harness
//! type carries. The connectors, wires and harness-typed sheet entries live on
//! the sheet ([`SheetHarness`]); the definitions live in the project
//! (`crate::project::SchematicDesignSettings`). File format:
//! `docs/architecture/snxsch-refinement-objects.md`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Aabb, Point};

/// How far the harness tip reaches past the body (mm), one coarse grid step.
pub const HARNESS_TIP_MM: f64 = 2.54;

/// The default distance between two entries of a connector (mm).
pub const DEFAULT_HARNESS_ENTRY_PITCH_MM: f64 = 2.54;

fn default_entry_pitch() -> f64 {
    DEFAULT_HARNESS_ENTRY_PITCH_MM
}

/// `skip_serializing_if` predicate: a lock is written only when set.
fn is_false(value: &bool) -> bool {
    !*value
}

/// A harness connector: a body with one entry per signal on its flat side and
/// a tip on the opposite side where the harness wire leaves.
///
/// At rotation 0 the flat side faces left and the tip points right; each
/// turn is 90 degrees counter-clockwise on screen. Whatever the rotation,
/// `position` is the top-left corner of the body's box.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HarnessConnector {
    pub uuid: Uuid,
    /// The harness definition this connector carries (`SPI_BUS`).
    pub harness_type: String,
    /// Top-left corner of the body (mm).
    pub position: Point,
    /// From the flat side to the base of the tip (mm).
    pub width: f64,
    /// Distance between two entries along the flat side (mm).
    #[serde(default = "default_entry_pitch")]
    pub entry_pitch: f64,
    /// 0, 90, 180 or 270 degrees.
    #[serde(default)]
    pub rotation: f64,
    /// The entries from the top of the flat side (at rotation 0) down.
    #[serde(default)]
    pub entries: Vec<HarnessEntry>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub locked: bool,
}

/// One signal entry on a harness connector.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HarnessEntry {
    pub uuid: Uuid,
    /// The signal (or nested harness) name, matched against the definition.
    pub name: String,
}

/// A harness wire: one straight segment carrying a whole harness.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HarnessWire {
    pub uuid: Uuid,
    pub start: Point,
    pub end: Point,
    #[serde(default, skip_serializing_if = "is_false")]
    pub locked: bool,
}

/// The harness objects of one sheet.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SheetHarness {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub connectors: Vec<HarnessConnector>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wires: Vec<HarnessWire>,
    /// Sheet-pin uuid to the harness type of a harness-typed sheet entry.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub entry_types: BTreeMap<Uuid, String>,
}

impl SheetHarness {
    /// True when the sheet has no harness object (and so saves nothing).
    pub fn is_empty(&self) -> bool {
        self.connectors.is_empty() && self.wires.is_empty() && self.entry_types.is_empty()
    }
}

/// One harness type of the project: the signals (and nested harnesses) it
/// carries, in order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HarnessDefinition {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default)]
    pub entries: Vec<HarnessDefinitionEntry>,
}

/// One line of a [`HarnessDefinition`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HarnessDefinitionEntry {
    pub name: String,
    /// The nested harness type; `None` for a signal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness: Option<String>,
}

impl HarnessConnector {
    /// Length of the flat side (mm): one pitch above the first entry, one
    /// below the last.
    pub fn height(&self) -> f64 {
        self.entry_pitch * (self.entries.len() as f64 + 1.0)
    }

    /// Where a signal wire meets entry `index` on the flat side.
    pub fn entry_point(&self, index: usize) -> Point {
        let along = self.entry_pitch * (index as f64 + 1.0);
        self.place(Point::new(0.0, along))
    }

    /// Where the harness wire leaves: [`HARNESS_TIP_MM`] past the body,
    /// centred on the side opposite the flat side.
    pub fn tip_point(&self) -> Point {
        self.place(Point::new(self.width + HARNESS_TIP_MM, self.height() * 0.5))
    }

    /// The body and the tip.
    pub fn bounds(&self) -> Aabb {
        let (w, h) = self.turned_extent();
        let body = Aabb::new(
            self.position.x,
            self.position.y,
            self.position.x + w,
            self.position.y + h,
        );
        let tip = self.tip_point();
        body.union(&Aabb::new(tip.x, tip.y, tip.x, tip.y))
    }

    /// Quarter turns counter-clockwise, `0..4`; the rotation rounded to the
    /// nearest multiple of 90 degrees.
    fn quarter_turns(&self) -> i64 {
        ((self.rotation / 90.0).round() as i64).rem_euclid(4)
    }

    /// Width and height of the body's box once turned.
    fn turned_extent(&self) -> (f64, f64) {
        if self.quarter_turns() % 2 == 0 {
            (self.width, self.height())
        } else {
            (self.height(), self.width)
        }
    }

    /// A point given in the unturned body frame (origin at the top-left,
    /// flat side on x = 0, tip towards +x) on the sheet: turned about the
    /// body's centre, then moved so the turned box starts at `position`.
    fn place(&self, local: Point) -> Point {
        let (half_w, half_h) = (self.width * 0.5, self.height() * 0.5);
        let (mut dx, mut dy) = (local.x - half_w, local.y - half_h);
        // y points down, so one counter-clockwise turn maps (dx, dy) to (dy, -dx).
        for _ in 0..self.quarter_turns() {
            (dx, dy) = (dy, 0.0 - dx);
        }
        let (w, h) = self.turned_extent();
        Point::new(
            self.position.x + w * 0.5 + dx,
            self.position.y + h * 0.5 + dy,
        )
    }
}
