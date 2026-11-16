//! The per-layer size rows and via span ends the via and pad panels show
//! (#2401, #2407), read from the board.

use signex_i18n::{I18n, ids, t};
use signex_types::pcb::{Pad, Point, Via};
use signex_types::pcb_layer::{CopperLayer, InnerCopper};
use signex_types::pcb_stack::StackMode;

use crate::panels::pcb_primitive_properties::{LayerChoice, LayerSizeRow, shared};
use crate::pcb_canvas::layer_style;

pub(super) fn layer_choice(i18n: &I18n, layer: CopperLayer) -> LayerChoice {
    LayerChoice {
        layer,
        label: layer_style::copper_name(i18n, layer).into_string(),
    }
}

/// The layers a size row sets in `mode`: Top, Middle (any inner layer) and
/// Bottom, or each of the board's copper layers.
fn row_layers(i18n: &I18n, mode: StackMode, copper: &[CopperLayer]) -> Vec<(CopperLayer, String)> {
    match mode {
        StackMode::Simple => Vec::new(),
        StackMode::TopMiddleBottom => {
            let middle = copper
                .iter()
                .copied()
                .find(|l| matches!(l, CopperLayer::Inner(_)))
                .or_else(|| InnerCopper::new(1).map(CopperLayer::Inner));
            let mut rows = vec![(CopperLayer::Top, t!(i18n, ids::PCB_PRIM_TOP).into_string())];
            if let Some(middle) = middle {
                rows.push((middle, t!(i18n, ids::PCB_PRIM_MIDDLE).into_string()));
            }
            rows.push((
                CopperLayer::Bottom,
                t!(i18n, ids::PCB_PRIM_BOTTOM).into_string(),
            ));
            rows
        }
        StackMode::Full => copper
            .iter()
            .map(|&l| (l, layer_style::copper_name(i18n, l).into_string()))
            .collect(),
    }
}

/// A via panel's size rows: the diameter on each layer the vias share.
pub(super) fn via_size_rows(
    i18n: &I18n,
    mode: Option<StackMode>,
    copper: &[CopperLayer],
    vias: &[&Via],
) -> Vec<LayerSizeRow> {
    let Some(mode) = mode else {
        return Vec::new();
    };
    row_layers(i18n, mode, copper)
        .into_iter()
        .map(|(layer, label)| LayerSizeRow {
            layer,
            label,
            x: shared(vias.iter().map(|v| v.diameter_on(layer))),
            y: None,
        })
        .collect()
}

/// A pad panel's size rows: the x and y size on each layer the pads share.
pub(super) fn pad_size_rows(
    i18n: &I18n,
    mode: Option<StackMode>,
    copper: &[CopperLayer],
    pads: &[&Pad],
) -> Vec<LayerSizeRow> {
    let Some(mode) = mode else {
        return Vec::new();
    };
    row_layers(i18n, mode, copper)
        .into_iter()
        .map(|(layer, label)| {
            let sizes: Vec<Point> = pads.iter().map(|p| p.size_on(layer)).collect();
            LayerSizeRow {
                layer,
                label,
                x: shared(sizes.iter().map(|s| s.x)),
                y: shared(sizes.iter().map(|s| s.y)),
            }
        })
        .collect()
}

/// The mode of one item's sizes.
pub(super) fn mode_of<T: Copy>(sizes: Option<&signex_types::pcb::LayerSizes<T>>) -> StackMode {
    sizes.map_or(StackMode::Simple, |s| s.mode())
}
