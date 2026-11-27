//! PDF layers (optional content groups) for the schematic export.
//!
//! With `PdfOptions::layers` set, the catalog carries `/OCProperties`, every
//! group is an `/OCG` object, each page lists the groups under
//! `/Properties`, and the page content wraps each kind of drawing in
//! `/OC /<name> BDC ... EMC`. Without it the PDF is flat. Rules and group
//! contents: docs/PDF_EXPORT.md, "PDF layers".

use std::collections::BTreeSet;

use pdf_writer::writers::Resources;
use pdf_writer::{Finish, Name, Pdf, Ref, TextStr};

/// One group a PDF reader can switch on and off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PdfLayer {
    WiresAndBuses,
    /// Harness wires, connectors and their entries, and harness-typed sheet
    /// entries (#2690).
    SignalHarnesses,
    Parts,
    NetLabelsAndPorts,
    PowerPorts,
    TitleBlockAndBorder,
    Watermark,
}

impl PdfLayer {
    /// Every group, in the order the reader lists them.
    pub const ALL: [PdfLayer; 7] = [
        PdfLayer::WiresAndBuses,
        PdfLayer::SignalHarnesses,
        PdfLayer::Parts,
        PdfLayer::NetLabelsAndPorts,
        PdfLayer::PowerPorts,
        PdfLayer::TitleBlockAndBorder,
        PdfLayer::Watermark,
    ];

    /// The group's `/Name` in the file. English whatever the UI language:
    /// PDF text is locale-invariant (docs/DECISIONS.md, 2026-11-07).
    pub fn group_name(self) -> &'static str {
        match self {
            PdfLayer::WiresAndBuses => "Wires and buses",
            PdfLayer::SignalHarnesses => "Signal harnesses",
            PdfLayer::Parts => "Parts",
            PdfLayer::NetLabelsAndPorts => "Net labels and ports",
            PdfLayer::PowerPorts => "Power ports",
            PdfLayer::TitleBlockAndBorder => "Title block and border",
            PdfLayer::Watermark => "Watermark",
        }
    }

    /// The `/Properties` resource name the page content refers to.
    fn property_name(self) -> String {
        let index = PdfLayer::ALL
            .iter()
            .position(|layer| *layer == self)
            .unwrap_or_default();
        format!("OC{index}")
    }
}

/// Which groups are hidden when the PDF opens. Every group not listed is
/// visible. A hidden Watermark group still prints.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PdfLayerVisibility {
    pub hidden_on_open: BTreeSet<PdfLayer>,
}

/// The groups a document writes: every drawing group, and the Watermark
/// group only when there is a watermark to put in it.
pub(super) fn written_layers(has_watermark: bool) -> Vec<PdfLayer> {
    PdfLayer::ALL
        .into_iter()
        .filter(|layer| has_watermark || *layer != PdfLayer::Watermark)
        .collect()
}

/// The `/OCG` object of each written group, numbered from `first_ref`.
pub(super) fn assign_refs(layers: &[PdfLayer], first_ref: i32) -> Vec<(PdfLayer, Ref)> {
    layers
        .iter()
        .enumerate()
        .map(|(i, layer)| (*layer, Ref::new(first_ref + i as i32)))
        .collect()
}

/// Write the catalog's `/OCProperties` entry. `catalog` is the catalog
/// dictionary being written.
pub(super) fn write_oc_properties(
    catalog: &mut pdf_writer::Dict<'_>,
    groups: &[(PdfLayer, Ref)],
    visibility: &PdfLayerVisibility,
) {
    let mut properties = catalog.insert(Name(b"OCProperties")).dict();
    properties
        .insert(Name(b"OCGs"))
        .array()
        .items(groups.iter().map(|(_, r)| *r));
    let mut config = properties.insert(Name(b"D")).dict();
    config
        .insert(Name(b"Order"))
        .array()
        .items(groups.iter().map(|(_, r)| *r));
    config.insert(Name(b"OFF")).array().items(
        groups
            .iter()
            .filter(|(layer, _)| visibility.hidden_on_open.contains(layer))
            .map(|(_, r)| *r),
    );
    // Printing switches the Watermark group on, even when it opens hidden.
    if let Some((_, watermark)) = groups.iter().find(|(l, _)| *l == PdfLayer::Watermark) {
        let mut auto_states = config.insert(Name(b"AS")).array();
        let mut print = auto_states.push().dict();
        print.pair(Name(b"Event"), Name(b"Print"));
        print.insert(Name(b"OCGs")).array().item(*watermark);
        print.insert(Name(b"Category")).array().item(Name(b"Print"));
        print.finish();
        auto_states.finish();
    }
    config.finish();
    properties.finish();
}

/// Write one `/OCG` object per group.
pub(super) fn write_groups(pdf: &mut Pdf, groups: &[(PdfLayer, Ref)]) {
    for (layer, id) in groups {
        let mut group = pdf.indirect(*id).dict();
        group.pair(Name(b"Type"), Name(b"OCG"));
        group.pair(Name(b"Name"), TextStr(layer.group_name()));
        if *layer == PdfLayer::Watermark {
            let mut usage = group.insert(Name(b"Usage")).dict();
            usage
                .insert(Name(b"Print"))
                .dict()
                .pair(Name(b"PrintState"), Name(b"ON"));
            usage.finish();
        }
        group.finish();
    }
}

/// A page's `/Properties` resource: the names its marked content uses.
pub(super) fn write_page_properties(resources: &mut Resources<'_>, groups: &[(PdfLayer, Ref)]) {
    if groups.is_empty() {
        return;
    }
    let mut properties = resources.insert(Name(b"Properties")).dict();
    for (layer, id) in groups {
        properties.pair(Name(layer.property_name().as_bytes()), *id);
    }
    properties.finish();
}

/// The operator that opens a group's marked content.
pub(super) fn begin_operator(layer: PdfLayer) -> String {
    format!("/OC /{} BDC\n", layer.property_name())
}

/// The operator that closes the open marked content.
pub(super) const END_OPERATOR: &str = "EMC\n";

/// Element-index ranges of a page's drawing, each in one group. Ranges
/// are in drawing order and never overlap; adjacent ranges of one group
/// are merged.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct LayerSpans(Vec<(std::ops::Range<usize>, PdfLayer)>);

impl LayerSpans {
    /// Put the elements in `range` into `layer`. An empty range is ignored.
    pub(crate) fn mark(&mut self, layer: PdfLayer, range: std::ops::Range<usize>) {
        if range.is_empty() {
            return;
        }
        if let Some((last, last_layer)) = self.0.last_mut()
            && *last_layer == layer
            && last.end == range.start
        {
            last.end = range.end;
            return;
        }
        self.0.push((range, layer));
    }

    /// The group whose range starts at element `index`, with the index
    /// after its last element.
    pub(crate) fn starting_at(&self, index: usize) -> Option<(PdfLayer, usize)> {
        // Ranges are pushed in drawing order, so their starts are sorted.
        let found = self
            .0
            .binary_search_by_key(&index, |(range, _)| range.start)
            .ok()?;
        let (range, layer) = &self.0[found];
        Some((*layer, range.end))
    }
}
