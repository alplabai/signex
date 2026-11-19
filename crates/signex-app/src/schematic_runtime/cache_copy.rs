//! The render cache's partial copy: which sheet fields a [`RenderInvalidation`]
//! refreshes, and the exhaustive pattern that keeps a new `SchematicSheet` field
//! from being silently left out (#2502).
//!
//! This lives beside `mod.rs` because the exhaustive destructure would otherwise
//! push the cache module past the god-file cap (ADR-0001, vertical slices).

use super::RenderInvalidation;
use signex_types::schematic::SchematicSheet;

/// Copy the sheet fields selected by `invalidation` from `src` into `dst`.
///
/// The pattern names every [`SchematicSheet`] field. When the sheet gains a
/// field this stops compiling until the field is either copied under a mask bit
/// or listed below as deliberately kept (#2502), which is how #2444's missing
/// `font_family` copy reached users: nothing made the omission visible.
pub(crate) fn copy_invalidated(
    dst: &mut SchematicSheet,
    src: &SchematicSheet,
    invalidation: RenderInvalidation,
) {
    use RenderInvalidation as R;
    let SchematicSheet {
        // The caller only takes the partial path when both sheets are the same
        // document (the uuids match), so the copy never needs to refresh it.
        uuid: _,
        // Not drawn.
        version: _,
        generator: _,
        generator_version: _,
        // File-only payload kept while re-saving; no edit or mask touches it.
        unknown: _,
        // Measurement caches: `text_widths` is keyed on the resolved family and
        // `text_extents` comes from the file, so neither is rendered state the
        // cache refreshes.
        text_widths: _,
        text_extents: _,
        paper_size,
        font_family,
        root_sheet_page,
        symbols,
        wires,
        junctions,
        labels,
        child_sheets,
        no_connects,
        text_notes,
        buses,
        bus_entries,
        drawings,
        no_erc_directives,
        directives,
        title_block,
        lib_symbols,
        net_colors,
        drawing_placements,
        frame,
        page,
    } = src;

    // The sheet default font is a scalar, not one of the collections below.
    // A font-family change invalidates the four text collections, so refresh
    // it with them; otherwise the cache keeps drawing the old family (#2444).
    if invalidation.intersects(R::SYMBOLS | R::LABELS | R::TEXT_NOTES | R::CHILD_SHEETS) {
        dst.font_family.clone_from(font_family);
    }
    if invalidation.intersects(R::SYMBOLS) {
        dst.symbols.clone_from(symbols);
    }
    if invalidation.intersects(R::WIRES) {
        dst.wires.clone_from(wires);
    }
    if invalidation.intersects(R::LABELS) {
        dst.labels.clone_from(labels);
    }
    if invalidation.intersects(R::TEXT_NOTES) {
        dst.text_notes.clone_from(text_notes);
    }
    if invalidation.intersects(R::BUSES) {
        dst.buses.clone_from(buses);
    }
    if invalidation.intersects(R::BUS_ENTRIES) {
        dst.bus_entries.clone_from(bus_entries);
    }
    if invalidation.intersects(R::JUNCTIONS) {
        dst.junctions.clone_from(junctions);
    }
    if invalidation.intersects(R::NO_CONNECTS) {
        dst.no_connects.clone_from(no_connects);
    }
    if invalidation.intersects(R::NO_ERC_DIRECTIVES) {
        dst.no_erc_directives.clone_from(no_erc_directives);
    }
    if invalidation.intersects(R::DIRECTIVES) {
        dst.directives.clone_from(directives);
    }
    if invalidation.intersects(R::CHILD_SHEETS) {
        dst.child_sheets.clone_from(child_sheets);
    }
    if invalidation.intersects(R::DRAWINGS) {
        dst.drawings.clone_from(drawings);
        // Placements are edited under the same patch (#1583).
        dst.drawing_placements.clone_from(drawing_placements);
    }
    if invalidation.intersects(R::LIB_SYMBOLS) {
        dst.lib_symbols.clone_from(lib_symbols);
    }
    if invalidation.intersects(R::NET_COLORS) {
        dst.net_colors.clone_from(net_colors);
    }
    if invalidation.intersects(R::PAPER) {
        dst.paper_size.clone_from(paper_size);
        dst.title_block.clone_from(title_block);
        dst.root_sheet_page.clone_from(root_sheet_page);
        dst.frame = *frame;
        dst.page = *page;
    }
}
