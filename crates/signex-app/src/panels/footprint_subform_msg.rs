//! Events of the footprint role sub-forms (mockup row 126) that have no flat
//! `FpEditor*` message of their own: the keepout layer, the pour's corners
//! (#2598) and the sketch pad's paste and mask expansions. Handled in
//! `app/handlers/dock/sch_library/footprint/role.rs`.

use signex_sketch::id::SketchEntityId;
use signex_types::pcb_layer::{BoardSide, PcbLayer};
use signex_types::zone_pour_shape::PourCorners;

#[derive(Debug, Clone)]
pub enum FpSubformMsg {
    /// The Keepout form's Layer dropdown picked `layer`.
    KeepoutLayerPicked { id: SketchEntityId, layer: PcbLayer },
    /// The Pour form's Corners segment picked `corners` (#2598).
    PourCornersPicked {
        id: SketchEntityId,
        corners: PourCorners,
    },
    /// The Pour form's Corner size was edited: a number of mm from 0, or
    /// text that is refused and kept.
    PourCornerSizeEdited { id: SketchEntityId, value: String },
    /// A sketch pad's paste expansion on `side` was edited. Empty text
    /// follows the design rule; any other text is a length expression.
    SketchPadPasteExpansionEdited {
        id: SketchEntityId,
        side: BoardSide,
        value: String,
    },
    /// A sketch pad's solder mask expansion on `side` was edited, with the
    /// same meaning of empty text.
    SketchPadMaskExpansionEdited {
        id: SketchEntityId,
        side: BoardSide,
        value: String,
    },
}

/// The key under which the footprint editor keeps the refused text of a
/// pour's Corner size.
pub fn pour_corner_size_buffer_key(id: SketchEntityId) -> String {
    format!("pour.{}.corner_size", id.0)
}

/// The key under which the footprint editor keeps the text of a sketch pad
/// expansion field while it is not an expression yet (`-`, `0.`), so the
/// field shows what was typed instead of the stored value.
pub fn expansion_buffer_key(id: SketchEntityId, is_mask: bool, side: BoardSide) -> String {
    let what = if is_mask { "mask" } else { "paste" };
    let side = match side {
        BoardSide::Top => "top",
        BoardSide::Bottom => "bottom",
    };
    format!("sketch_pad.{}.{what}_{side}", id.0)
}
