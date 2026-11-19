//! Events of the footprint role sub-forms (#126) that have no flat
//! `FpEditor*` message of their own: the keepout layer and the sketch pad's
//! paste and mask expansions. Handled in
//! `app/handlers/dock/sch_library/footprint/role.rs`.

use signex_sketch::id::SketchEntityId;
use signex_types::pcb_layer::{BoardSide, PcbLayer};

#[derive(Debug, Clone)]
pub enum FpSubformMsg {
    /// The Keepout form's Layer dropdown picked `layer`.
    KeepoutLayerPicked { id: SketchEntityId, layer: PcbLayer },
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
