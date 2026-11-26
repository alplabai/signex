//! The Net Inspector's By Component rows (#1274), read from the project net
//! index's parts (`signex_net::NetIndex::components`); nothing here
//! re-derives a net or a part. The By Net rows read the same parts for the
//! pin names they show, so a pin's net and name agree in both tabs.

use std::collections::HashMap;

use signex_net::{ComponentReport, NetAnchor, NetIndex};
use signex_types::designator::{DesignatorStandard, compare_references, display_reference};

use crate::app::state::net_nav::ProjectNetData;
use crate::panels::NetInspectorState;
use crate::panels::net_inspector::{ComponentPinRow, ComponentRow, preview};

/// What the expanded set knows a part by: its designator, or for a part not
/// annotated yet its first unit, which the designator cannot tell apart.
pub(super) fn component_key(part: &ComponentReport) -> String {
    if part.is_annotated() {
        return part.designator.clone();
    }
    let at = &part.anchor;
    format!("{}{}#{}", at.sheet, at.occurrence, at.id)
}

/// The rows for `data` under `state`, with every part of the project counted:
/// the parts whose designator, value or footprint holds the filter, by
/// designator; an expanded part with every pin, wired or not, in pin-number
/// order, the first few unless listed in full. The designator is shown as
/// `standard` shows it (`-QAC1`, #1416) and the value is the evaluated one
/// (#1986, read off the index).
pub(crate) fn component_rows(
    data: &ProjectNetData,
    state: &NetInspectorState,
    standard: Option<DesignatorStandard>,
) -> (Vec<ComponentRow>, usize) {
    let needle = state.filter.trim().to_lowercase();
    let parts = data.index.components();
    let total = parts.len();
    let rows = parts
        .into_iter()
        .filter(|part| {
            let shown = display_reference(standard, &part.designator);
            needle.is_empty()
                || [&*shown, part.value.as_str(), part.footprint.as_str()]
                    .iter()
                    .any(|field| field.to_lowercase().contains(&needle))
        })
        .map(|mut part| {
            let key = component_key(&part);
            part.designator = display_reference(standard, &part.designator).into_owned();
            let expanded = state.expanded_parts.contains(&key);
            let (pin_rows, more) = if expanded {
                preview(pin_rows(data, &part), state.full_parts.contains(&key))
            } else {
                (Vec::new(), 0)
            };
            ComponentRow {
                key,
                pins: part.pin_count(),
                expanded,
                pin_rows,
                more,
                designator: part.designator,
                value: part.value,
                footprint: part.footprint,
            }
        })
        .collect();
    (rows, total)
}

/// Every pin of `part`, the wired ones with their net, by pin number.
fn pin_rows(data: &ProjectNetData, part: &ComponentReport) -> Vec<ComponentPinRow> {
    let wired = part.pins.iter().map(|pin| ComponentPinRow {
        pin: pin.pin.clone(),
        name: pin.name.clone(),
        net: Some(pin.net_name.clone()),
        sheet: data.sheet_label(&pin.anchor.sheet),
        anchor: pin.anchor.clone(),
    });
    let open = part.unconnected.iter().map(|pin| ComponentPinRow {
        pin: pin.pin.clone(),
        name: pin.name.clone(),
        net: None,
        sheet: data.sheet_label(&pin.anchor.sheet),
        anchor: pin.anchor.clone(),
    });
    let mut rows: Vec<ComponentPinRow> = wired.chain(open).collect();
    rows.sort_by(|a, b| compare_references(&a.pin, &b.pin));
    rows
}

/// Every pin's library name by the anchor a By Net member carries, for the
/// pin names the By Net rows show beside `R1.2`.
pub(super) fn pin_names(index: &NetIndex) -> HashMap<NetAnchor, String> {
    index
        .components()
        .into_iter()
        .flat_map(|part| part.pins)
        .filter(|pin| !pin.name.is_empty())
        .map(|pin| (pin.anchor, pin.name))
        .collect()
}

#[cfg(test)]
#[path = "component_rows_tests.rs"]
mod tests;
