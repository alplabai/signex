//! The paste and solder mask titles of the pad forms (the Pads-mode pad
//! form and the sketch pad sub-form), read from the layer catalog so a pad
//! names its layers as the Layers panel does.

use signex_i18n::{I18n, Localized, ids, t};
use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};

use crate::pcb_canvas::layer_style::layer_name;

/// The title of a pad's paste or solder mask layer: the layer's catalog
/// name ("Top Paste"), for a solder mask with what it is ("Top Solder
/// mask", mockup row 125).
pub(in crate::panels::footprint_editor_properties) fn layer_title(
    i18n: &I18n,
    layer: TechnicalLayer,
) -> Localized {
    let name = layer_name(i18n, PcbLayer::Technical(layer));
    match layer {
        TechnicalLayer::SolderMaskTop | TechnicalLayer::SolderMaskBottom => {
            t!(i18n, ids::EDITORS_FP_PAD_SOLDER_MASK_LAYER, "layer" => name.into_string())
        }
        _ => name,
    }
}
