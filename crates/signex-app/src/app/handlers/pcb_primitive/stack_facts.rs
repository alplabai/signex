//! The per-layer size rows, via span ends and via types the via and pad
//! panels show (#2401, #2407, #96), read from the board. The via types and
//! their protection class come from `signex_engine::pcb::via_types`; this
//! module only names them.

use signex_engine::pcb::via_types::{ViaTypeDef, is_overridden, min_via_drill, type_of, via_types};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::pcb::{Pad, PcbBoard, Point, Via, ViaType};
use signex_types::pcb_layer::{CopperLayer, InnerCopper};
use signex_types::pcb_stack::StackMode;
use signex_types::pcb_via_protection::{Ipc4761, ViaProtection};

use crate::panels::pcb_primitive_properties::{
    LayerChoice, LayerSizeRow, ViaFacts, ViaTypeChoice, format_number, shared,
};
use crate::pcb_canvas::layer_style;

/// A via's span name, "Thru 1:4": the one label the Name row and the via
/// type list share.
pub(super) fn span_label(i18n: &I18n, kind: ViaType, (from, to): (usize, usize)) -> String {
    let id = match kind {
        ViaType::Through => ids::PCB_PRIM_VIA_SPAN_THROUGH,
        ViaType::Blind => ids::PCB_PRIM_VIA_SPAN_BLIND,
        ViaType::Micro => ids::PCB_PRIM_VIA_SPAN_MICRO,
    };
    t!(i18n, id, "from" => from, "to" => to).into_string()
}

fn kind_word(i18n: &I18n, kind: ViaType) -> String {
    let id = match kind {
        ViaType::Through => ids::PCB_PRIM_VIA_KIND_THROUGH,
        ViaType::Blind => ids::PCB_PRIM_VIA_KIND_BLIND,
        ViaType::Micro => ids::PCB_PRIM_VIA_KIND_MICRO,
    };
    t!(i18n, id).into_string()
}

/// A drill pair, "Through 1–4".
fn drill_pair(i18n: &I18n, kind: ViaType, (from, to): (usize, usize)) -> String {
    let kind = kind_word(i18n, kind);
    t!(i18n, ids::PCB_PRIM_VIA_DRILL_PAIR, "kind" => kind, "from" => from, "to" => to).into_string()
}

/// A protection class as a via type's spec names it: "Type I-b", "open".
fn class_short(i18n: &I18n, class: Ipc4761) -> Localized {
    match class.code() {
        Some(code) => t!(i18n, ids::PCB_PRIM_IPC_CODE, "code" => code),
        None if class == Ipc4761::Open => t!(i18n, ids::PCB_PRIM_IPC_SHORT_OPEN),
        None => t!(i18n, ids::PCB_PRIM_IPC_CUSTOM),
    }
}

fn type_choice(i18n: &I18n, def: &ViaTypeDef) -> ViaTypeChoice {
    let name = span_label(i18n, def.kind, def.span);
    let spec = t!(
        i18n,
        ids::PCB_PRIM_VIA_TYPE_SPEC,
        "kind" => kind_word(i18n, def.kind),
        "from" => def.span.0,
        "to" => def.span.1,
        "diameter" => format_number(def.diameter),
        "drill" => format_number(def.drill),
        "class" => class_short(i18n, def.protection.class()).into_string()
    )
    .into_string();
    let label = t!(
        i18n,
        ids::PCB_PRIM_VIA_TYPE_ENTRY,
        "name" => name.clone(),
        "spec" => spec.clone()
    )
    .into_string();
    ViaTypeChoice {
        key: def.key(),
        name,
        spec,
        kind: def.kind,
        label,
    }
}

/// Fill in what `facts` shows of the vias' type (#96): the board's types,
/// the one the vias share, whether they override its size, the drill pair,
/// protection, finished hole, the board's minimum drill and the sides the
/// vias reach.
pub(super) fn fill_via_type_facts(
    facts: &mut ViaFacts,
    board: &PcbBoard,
    stack: &[CopperLayer],
    vias: &[&Via],
    i18n: &I18n,
) {
    let defs = via_types(board);
    facts.types = defs.iter().map(|d| type_choice(i18n, d)).collect();
    let shared_type = shared(vias.iter().map(|v| type_of(&defs, v))).flatten();
    facts.via_type = shared_type.map(|i| facts.types[i].clone());
    facts.type_size = shared_type.map(|i| (defs[i].diameter, defs[i].drill));
    facts.overridden = shared_type.map(|i| vias.iter().any(|v| is_overridden(&defs[i], v)));
    facts.protection = shared(vias.iter().map(|v| ViaProtection::of_via(v)));
    let spans: Vec<Option<(usize, usize)>> =
        vias.iter().map(|v| pcb_geom::via_span(stack, v)).collect();
    facts.drill_pair = shared(vias.iter().map(|v| v.via_type).zip(spans.iter().copied()))
        .and_then(|(kind, span)| Some(drill_pair(i18n, kind, span?)));
    facts.finished_hole = facts.hole.map(ViaTypeDef::finished_hole);
    facts.min_drill = min_via_drill(board);
    let last = stack.len();
    facts.reaches = (
        spans.iter().all(|s| s.is_some_and(|(from, _)| from == 1)),
        spans.iter().all(|s| s.is_some_and(|(_, to)| to == last)),
    );
}

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
