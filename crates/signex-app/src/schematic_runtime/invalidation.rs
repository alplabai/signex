//! Which collections of the render snapshot an edit changed, so the scene
//! cache copies only those.

use signex_engine::DocumentPatch;
use signex_types::schematic::SchematicSheet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RenderInvalidation(u32);

impl RenderInvalidation {
    pub const NONE: Self = Self(0);
    pub const SYMBOLS: Self = Self(1 << 0);
    pub const WIRES: Self = Self(1 << 1);
    pub const LABELS: Self = Self(1 << 2);
    pub const TEXT_NOTES: Self = Self(1 << 3);
    pub const BUSES: Self = Self(1 << 4);
    pub const BUS_ENTRIES: Self = Self(1 << 5);
    pub const JUNCTIONS: Self = Self(1 << 6);
    pub const NO_CONNECTS: Self = Self(1 << 7);
    pub const CHILD_SHEETS: Self = Self(1 << 8);
    pub const DRAWINGS: Self = Self(1 << 9);
    pub const LIB_SYMBOLS: Self = Self(1 << 10);
    pub const PAPER: Self = Self(1 << 11);
    pub const NO_ERC_DIRECTIVES: Self = Self(1 << 12);
    /// The sheet's net colours: a repaint with no geometry or ERC change.
    pub const NET_COLORS: Self = Self(1 << 13);
    /// Blankets, Compile Masks and Differential Pair directives (#2420).
    pub const DIRECTIVES: Self = Self(1 << 14);
    /// Signal harness connectors, wires and typed sheet entries (#1280).
    pub const HARNESS: Self = Self(1 << 15);
    /// Named unions (mockup row 158): drawn in the selection layer only.
    pub const UNIONS: Self = Self(1 << 20);
    pub const FULL: Self = Self(u32::MAX);

    /// True when any bit of `mask` is also set in `self`.
    pub fn intersects(self, mask: Self) -> bool {
        self.0 & mask.0 != 0
    }
}

impl std::ops::BitOr for RenderInvalidation {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for RenderInvalidation {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// The render collections each document change redraws. The sheet default
/// font is inherited by every kind of text, so it redraws all four.
const PROJECTION: [(DocumentPatch, RenderInvalidation); 18] = {
    use RenderInvalidation as R;
    [
        (DocumentPatch::SYMBOLS, R::SYMBOLS),
        (DocumentPatch::WIRES, R::WIRES),
        (DocumentPatch::LABELS, R::LABELS),
        (DocumentPatch::TEXT_NOTES, R::TEXT_NOTES),
        (DocumentPatch::BUSES, R::BUSES),
        (DocumentPatch::BUS_ENTRIES, R::BUS_ENTRIES),
        (DocumentPatch::JUNCTIONS, R::JUNCTIONS),
        (DocumentPatch::NO_CONNECTS, R::NO_CONNECTS),
        (DocumentPatch::NO_ERC_DIRECTIVES, R::NO_ERC_DIRECTIVES),
        (DocumentPatch::DIRECTIVES, R::DIRECTIVES),
        (DocumentPatch::HARNESS, R::HARNESS),
        (DocumentPatch::CHILD_SHEETS, R::CHILD_SHEETS),
        (DocumentPatch::DRAWINGS, R::DRAWINGS),
        (DocumentPatch::LIB_SYMBOLS, R::LIB_SYMBOLS),
        (DocumentPatch::PAPER, R::PAPER),
        (DocumentPatch::NET_COLORS, R::NET_COLORS),
        (DocumentPatch::UNIONS, R::UNIONS),
        (
            DocumentPatch::FONT_FAMILY,
            R(R::SYMBOLS.0 | R::LABELS.0 | R::TEXT_NOTES.0 | R::CHILD_SHEETS.0),
        ),
    ]
};

/// What the render cache re-copies for a document change. Applied once,
/// where the GUI refreshes; decisions read the `DocumentPatch` itself.
impl From<DocumentPatch> for RenderInvalidation {
    fn from(patch: DocumentPatch) -> Self {
        if patch == DocumentPatch::FULL {
            return Self::FULL;
        }
        PROJECTION
            .iter()
            .filter(|(bit, _)| patch.contains(*bit))
            .fold(Self::NONE, |acc, (_, render)| acc | *render)
    }
}

/// Copy the sheet collections selected by `invalidation` from `src` into `dst`.
/// `unknown`, `version`, `generator` and `generator_version` are skipped: they are not drawn.
pub(super) fn copy_invalidated(
    dst: &mut SchematicSheet,
    src: &SchematicSheet,
    invalidation: RenderInvalidation,
) {
    use RenderInvalidation as R;
    // The sheet default font is a scalar, not one of the collections below.
    // A font-family change invalidates the four text collections, so refresh
    // it with them; otherwise the cache keeps drawing the old family (#2444).
    if invalidation.intersects(R::SYMBOLS | R::LABELS | R::TEXT_NOTES | R::CHILD_SHEETS) {
        dst.font_family.clone_from(&src.font_family);
    }
    if invalidation.intersects(R::SYMBOLS) {
        dst.symbols.clone_from(&src.symbols);
    }
    if invalidation.intersects(R::WIRES) {
        dst.wires.clone_from(&src.wires);
    }
    if invalidation.intersects(R::LABELS) {
        dst.labels.clone_from(&src.labels);
    }
    if invalidation.intersects(R::TEXT_NOTES) {
        dst.text_notes.clone_from(&src.text_notes);
    }
    if invalidation.intersects(R::BUSES) {
        dst.buses.clone_from(&src.buses);
    }
    if invalidation.intersects(R::BUS_ENTRIES) {
        dst.bus_entries.clone_from(&src.bus_entries);
    }
    if invalidation.intersects(R::JUNCTIONS) {
        dst.junctions.clone_from(&src.junctions);
    }
    if invalidation.intersects(R::NO_CONNECTS) {
        dst.no_connects.clone_from(&src.no_connects);
    }
    if invalidation.intersects(R::NO_ERC_DIRECTIVES) {
        dst.no_erc_directives.clone_from(&src.no_erc_directives);
    }
    if invalidation.intersects(R::DIRECTIVES) {
        dst.directives.clone_from(&src.directives);
    }
    if invalidation.intersects(R::HARNESS) {
        dst.harness.clone_from(&src.harness);
    }
    if invalidation.intersects(R::UNIONS) {
        dst.unions.clone_from(&src.unions);
    }
    if invalidation.intersects(R::CHILD_SHEETS) {
        dst.child_sheets.clone_from(&src.child_sheets);
    }
    if invalidation.intersects(R::DRAWINGS) {
        dst.drawings.clone_from(&src.drawings);
        // Placements are edited under the same patch (#1583).
        dst.drawing_placements.clone_from(&src.drawing_placements);
    }
    if invalidation.intersects(R::LIB_SYMBOLS) {
        dst.lib_symbols.clone_from(&src.lib_symbols);
    }
    if invalidation.intersects(R::NET_COLORS) {
        dst.net_colors.clone_from(&src.net_colors);
    }
    if invalidation.intersects(R::PAPER) {
        dst.paper_size.clone_from(&src.paper_size);
        dst.title_block.clone_from(&src.title_block);
        dst.root_sheet_page.clone_from(&src.root_sheet_page);
        dst.frame = src.frame;
        dst.page = src.page;
    }
}
