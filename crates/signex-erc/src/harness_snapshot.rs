//! What ERC reads of a sheet's signal harnesses (#1280): the connectors with
//! their entries, the harness wire segments and the bundles `signex-net`
//! derives. The project's harness definitions travel in the
//! [`ErcConfig`](crate::ErcConfig).

use signex_net::{HarnessBundle, HarnessLeaf, harness_bundles, harness_segments_touch};
use signex_types::schematic::{HarnessConnector, Point, SchematicSheet};

/// The harness part of an [`ErcContext`](crate::ErcContext).
#[derive(Debug, Clone, Default)]
pub struct ErcHarness {
    /// Every connector of the sheet, with its entries.
    pub connectors: Vec<HarnessConnector>,
    /// The harness wires as `(start, end)`.
    pub segments: Vec<(Point, Point)>,
    /// The sheet's bundles, top-level and nested.
    pub bundles: Vec<HarnessBundle>,
}

impl ErcHarness {
    /// Project `sheet`'s harnesses.
    pub fn from_sheet(sheet: &SchematicSheet) -> Self {
        Self {
            connectors: sheet.harness.connectors.clone(),
            segments: sheet
                .harness
                .wires
                .iter()
                .map(|w| (w.start, w.end))
                .collect(),
            bundles: harness_bundles(sheet),
        }
    }

    /// Whether `p` lies on a harness wire: a sheet entry or port there is
    /// joined by the harness, not floating.
    pub fn touches(&self, p: &Point) -> bool {
        harness_segments_touch(self.segments.iter().copied(), p)
    }

    /// Every connector entry point: a wire ending there goes into the
    /// harness, so it neither dangles nor ends its net on this sheet.
    pub fn entry_points(&self) -> Vec<Point> {
        self.connectors
            .iter()
            .flat_map(|c| (0..c.entries.len()).map(move |i| c.entry_point(i)))
            .collect()
    }

    /// The leaves of the top-level bundle with the harness port `name`,
    /// nested entries expanded, when the sheet has such a port.
    pub fn port_leaves(&self, name: &str) -> Option<&[HarnessLeaf]> {
        self.bundles
            .iter()
            .filter(|b| !b.nested)
            .find(|b| b.ports.iter().any(|p| p.name == name))
            .map(|b| b.leaves.as_slice())
    }

    /// The connector `id`.
    pub fn connector(&self, id: uuid::Uuid) -> Option<&HarnessConnector> {
        self.connectors.iter().find(|c| c.uuid == id)
    }
}
