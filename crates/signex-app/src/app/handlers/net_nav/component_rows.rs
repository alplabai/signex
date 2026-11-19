//! The Net Inspector's By Component rows (#1274), read from the project net
//! index's parts (`signex_net::NetIndex::components`); nothing here
//! re-derives a net or a part.

use signex_net::ComponentReport;
use signex_types::designator::display_reference;

use crate::app::state::net_nav::ProjectNetData;
use crate::panels::NetInspectorState;
use crate::panels::net_inspector::{ComponentPinRow, ComponentRow};

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
/// designator; an expanded part with its connected pins. The designator is
/// shown as `standard` shows it (`-QAC1`, #1416) and the value is the
/// evaluated one (#1986, read off the index).
pub(super) fn component_rows(
    data: &ProjectNetData,
    state: &NetInspectorState,
    standard: Option<signex_types::designator::DesignatorStandard>,
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
            let pin_rows = if expanded {
                part.pins
                    .iter()
                    .map(|pin| ComponentPinRow {
                        pin: pin.pin.clone(),
                        net: pin.net_name.clone(),
                        sheet: data.sheet_label(&pin.anchor.sheet),
                        anchor: pin.anchor.clone(),
                    })
                    .collect()
            } else {
                Vec::new()
            };
            ComponentRow {
                key,
                pins: part.pins.len(),
                expanded,
                pin_rows,
                designator: part.designator,
                value: part.value,
                footprint: part.footprint,
            }
        })
        .collect();
    (rows, total)
}
