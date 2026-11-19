//! The Net Inspector's By Component rows (#58): every part of the project
//! with its pins and the net of each, read from the same project net index
//! the By Net rows read (`NetIndex::parts`, `NetIndex::net_of_pin`), so a
//! pin's net and name agree in both modes.

use std::cmp::Ordering;
use std::collections::HashMap;

use signex_net::{NetAnchor, NetIndex};

use crate::app::state::net_nav::ProjectNetData;
use crate::panels::NetInspectorState;
use crate::panels::net_inspector::{PartPinRow, PartRow, preview};

/// The parts the filter keeps, by designator (`C2` before `C10`); an
/// expanded part with its pins in pin-number order.
pub(crate) fn part_rows(data: &ProjectNetData, state: &NetInspectorState) -> Vec<PartRow> {
    let needle = state.filter.trim().to_lowercase();
    let index = &data.index;
    let mut rows: Vec<PartRow> = index
        .parts()
        .iter()
        .filter(|part| {
            needle.is_empty()
                || [&part.designator, &part.value, &part.footprint]
                    .iter()
                    .any(|field| field.to_lowercase().contains(&needle))
        })
        .map(|part| {
            let expanded = state.expanded_parts.contains(&part.designator);
            let (pin_rows, more) = if expanded {
                let mut pins: Vec<PartPinRow> = part
                    .pins
                    .iter()
                    .map(|pin| {
                        let anchor = NetAnchor {
                            sheet: part.sheet.clone(),
                            id: pin.symbol,
                            occurrence: part.occurrence.clone(),
                            pin: Some(pin.id.clone()),
                        };
                        PartPinRow {
                            number: pin.id.clone(),
                            name: pin.name.clone(),
                            net: net_name(index, &anchor),
                            anchor,
                        }
                    })
                    .collect();
                pins.sort_by(|a, b| natural_order(&a.number, &b.number));
                preview(pins, state.full_parts.contains(&part.designator))
            } else {
                (Vec::new(), 0)
            };
            PartRow {
                designator: part.designator.clone(),
                value: part.value.clone(),
                footprint: part.footprint.clone(),
                pins: part.pins.len(),
                expanded,
                pin_rows,
                more,
            }
        })
        .collect();
    rows.sort_by(|a, b| natural_order(&a.designator, &b.designator));
    rows
}

/// The name of the net the pin `anchor` names is on; `None` when unconnected.
fn net_name(index: &NetIndex, anchor: &NetAnchor) -> Option<String> {
    let pin = anchor.pin.as_deref()?;
    let net = index.net_of_pin(&anchor.sheet, anchor.id, &anchor.occurrence, pin)?;
    index.net(net).map(|n| n.name.clone())
}

/// Every pin's library name by the anchor a By Net member carries, for the
/// pin names the By Net rows show beside `R1.2`.
pub(super) fn pin_names(index: &NetIndex) -> HashMap<NetAnchor, String> {
    index
        .parts()
        .iter()
        .flat_map(|part| {
            part.pins
                .iter()
                .filter(|pin| !pin.name.is_empty())
                .map(|pin| {
                    (
                        NetAnchor {
                            sheet: part.sheet.clone(),
                            id: pin.symbol,
                            occurrence: part.occurrence.clone(),
                            pin: Some(pin.id.clone()),
                        },
                        pin.name.clone(),
                    )
                })
        })
        .collect()
}

/// `C2` before `C10` and pin `2` before `10`: runs of digits compare by
/// value, everything else by case-folded text.
pub(crate) fn natural_order(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a, b);
    loop {
        match (a.chars().next(), b.chars().next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let (na, ra) = split_digits(a);
                let (nb, rb) = split_digits(b);
                let by_value = na
                    .trim_start_matches('0')
                    .len()
                    .cmp(&nb.trim_start_matches('0').len())
                    .then_with(|| na.trim_start_matches('0').cmp(nb.trim_start_matches('0')));
                if by_value != Ordering::Equal {
                    return by_value;
                }
                (a, b) = (ra, rb);
            }
            (Some(x), Some(y)) => {
                let by_char = x.to_lowercase().cmp(y.to_lowercase());
                if by_char != Ordering::Equal {
                    return by_char;
                }
                (a, b) = (&a[x.len_utf8()..], &b[y.len_utf8()..]);
            }
        }
    }
}

fn split_digits(s: &str) -> (&str, &str) {
    let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    s.split_at(end)
}

#[cfg(test)]
#[path = "parts_tests.rs"]
mod tests;
