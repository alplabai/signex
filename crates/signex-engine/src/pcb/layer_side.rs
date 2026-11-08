//! Side swap for board layer names, through the typed [`PcbLayer`] (#2173).
//! A name this build does not know is kept as it is.

use signex_types::format::{parse_pcb_layer, pcb_layer_token};
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer as T};

/// The layer on the other face of the board: top and bottom pairs swap;
/// inner copper, board-wide and user layers stay. Swapping twice gives back
/// the input.
pub fn opposite_side(layer: PcbLayer) -> PcbLayer {
    match layer {
        PcbLayer::Copper(CopperLayer::Top) => PcbLayer::Copper(CopperLayer::Bottom),
        PcbLayer::Copper(CopperLayer::Bottom) => PcbLayer::Copper(CopperLayer::Top),
        PcbLayer::Technical(t) => PcbLayer::Technical(opposite_technical(t)),
        other => other,
    }
}

fn opposite_technical(t: T) -> T {
    match t {
        T::SilkscreenTop => T::SilkscreenBottom,
        T::SilkscreenBottom => T::SilkscreenTop,
        T::SolderMaskTop => T::SolderMaskBottom,
        T::SolderMaskBottom => T::SolderMaskTop,
        T::PasteTop => T::PasteBottom,
        T::PasteBottom => T::PasteTop,
        T::AdhesiveTop => T::AdhesiveBottom,
        T::AdhesiveBottom => T::AdhesiveTop,
        T::FabTop => T::FabBottom,
        T::FabBottom => T::FabTop,
        T::AssemblyTop => T::AssemblyBottom,
        T::AssemblyBottom => T::AssemblyTop,
        T::CourtyardTop => T::CourtyardBottom,
        T::CourtyardBottom => T::CourtyardTop,
        T::Body3dTop => T::Body3dBottom,
        T::Body3dBottom => T::Body3dTop,
        T::KeepoutTop => T::KeepoutBottom,
        T::KeepoutBottom => T::KeepoutTop,
        T::FiducialTop => T::FiducialBottom,
        T::FiducialBottom => T::FiducialTop,
        T::DesignatorTop => T::DesignatorBottom,
        T::DesignatorBottom => T::DesignatorTop,
        other => other,
    }
}

/// The on-disk name of the opposite-side layer; an unknown name is returned
/// unchanged.
pub fn opposite_side_name(name: &str) -> String {
    match parse_pcb_layer(name) {
        Some(layer) => pcb_layer_token(opposite_side(layer)),
        None => name.to_owned(),
    }
}

/// Swap a layer name's side in place.
pub(super) fn swap_side_in_place(name: &mut String) {
    *name = opposite_side_name(name);
}
