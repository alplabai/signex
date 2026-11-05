//! Which width a text is boxed and drawn with (#2029): the live measurement
//! while its face is installed, else the width stored in the file, else the
//! substitute face's. The decision is `signex_types::schematic::decide_width_mm`;
//! this wires it to the sheet's cache and stored widths.

use signex_types::font::{family_width_mm_checked, requested_family};
use signex_types::schematic::{
    SchematicSheet, TextExtents, TextField, TextProbe, TextWidths, decide_width_mm,
    face_is_available,
};
use uuid::Uuid;

/// The sheet's live cache and the widths its file stored.
#[derive(Clone, Copy)]
pub(super) struct WidthSources<'a> {
    widths: &'a TextWidths,
    extents: &'a TextExtents,
}

impl<'a> WidthSources<'a> {
    pub(super) fn of(sheet: &'a SchematicSheet) -> Self {
        Self {
            widths: &sheet.text_widths,
            extents: &sheet.text_extents,
        }
    }
}

/// One text's place in the sheet's tables, and the family it asks for.
#[derive(Clone, Copy)]
pub(super) struct SlotInfo<'a> {
    sources: WidthSources<'a>,
    id: Uuid,
    field: TextField,
    requested: Option<&'a str>,
}

pub(super) type WidthSlot<'a> = Option<SlotInfo<'a>>;

/// The slot of an item's text: its own family, else the sheet's, is asked for.
pub(super) fn slot<'a>(
    sources: Option<WidthSources<'a>>,
    id: Uuid,
    field: TextField,
    item_family: Option<&'a str>,
    document_family: Option<&'a str>,
) -> WidthSlot<'a> {
    sources.map(|sources| SlotInfo {
        sources,
        id,
        field,
        requested: requested_family(item_family, document_family),
    })
}

/// Width in mm of `text` at cap height `size_mm`. `resolved` is the face the
/// text draws in (`None` is the bundled default).
pub(super) fn text_width_mm(
    slot: WidthSlot<'_>,
    text: &str,
    resolved: Option<&str>,
    size_mm: f64,
) -> f64 {
    let live = || family_width_mm_checked(text, resolved, size_mm);
    let Some(slot) = slot else {
        return live().0;
    };
    let SlotInfo {
        sources,
        id,
        field,
        requested,
    } = slot;
    let probe = TextProbe {
        id,
        field,
        text,
        requested,
        size_mm,
    };
    let decide = || decide_width_mm(&probe, resolved, sources.extents, live);
    // A stored record is checked on every read, not through the cache: the
    // cache is keyed by the resolved face, which is the default for any
    // missing one.
    if sources.extents.contains(id, field) && !face_is_available(requested, resolved) {
        return decide().0;
    }
    sources
        .widths
        .width_mm(id, field, text, resolved, size_mm, decide)
}

/// The measurement a save writes into the file's stored widths: `Some` only
/// for a text whose face is installed and whose measurement is final, so a
/// substitute face's width is never stored. Before the font scan has landed a
/// system family counts as missing, and its loaded record is kept.
pub(crate) fn live_extent_measure() -> impl FnMut(&TextProbe<'_>) -> Option<f64> {
    |probe| {
        let resolved = probe
            .requested
            .and_then(|family| crate::fonts::resolve::resolve_installed(Some(family), None));
        if !face_is_available(probe.requested, resolved.as_deref()) {
            return None;
        }
        let (width_mm, is_final) =
            family_width_mm_checked(probe.text, resolved.as_deref(), probe.size_mm);
        is_final.then_some(width_mm)
    }
}
