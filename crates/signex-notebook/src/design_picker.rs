//! The reference picker that typing `@` opens in the source editor
//! (mockup row 196): the parts, pins and nets of the design that match
//! what is typed after the `@`. Picking one inserts the Typst call; the
//! `@` itself is never written (it is a label reference in Typst).

use crate::design::{DesignIndex, natural_order};
use crate::reference::Reference;

/// The most entries the picker lists.
pub const PICKER_LIMIT: usize = 12;

/// One entry of the picker.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PickerEntry {
    pub reference: Reference,
    /// The second line: a part's value, a pin's name, a net's pin count.
    pub detail: String,
    /// What picking it inserts in markup: `#component("R7")`.
    pub insert: String,
}

impl PickerEntry {
    fn new(reference: Reference, detail: String) -> Self {
        Self {
            insert: reference.to_markup(),
            reference,
            detail,
        }
    }
}

/// The entries matching `query`, the text typed after `@`, any case.
/// `U3.` lists the pins of U3 (by number or name); other text lists the
/// parts, then the nets, whose name starts with it, then those that only
/// contain it. An empty query lists the parts, then the nets. At most
/// `limit` entries.
pub fn picker_entries(design: &DesignIndex, query: &str, limit: usize) -> Vec<PickerEntry> {
    let query = query.trim();
    if let Some((part, pin)) = query.split_once('.') {
        return pin_entries(design, part, pin, limit);
    }
    let needle = query.to_lowercase();
    let mut starts = Vec::new();
    let mut contains = Vec::new();
    let mut take = |name: &str, entry: PickerEntry| {
        let lower = name.to_lowercase();
        if lower.starts_with(&needle) {
            starts.push(entry);
        } else if lower.contains(&needle) {
            contains.push(entry);
        }
    };
    for part in design.parts() {
        let entry = PickerEntry::new(
            Reference::Component(part.designator.clone()),
            part.value.clone(),
        );
        take(&part.designator, entry);
    }
    for net in design.nets() {
        let entry = PickerEntry::new(Reference::Net(net.name.clone()), net.pins.len().to_string());
        take(&net.name, entry);
    }
    starts.extend(contains);
    starts.truncate(limit);
    starts
}

/// The pins of the part named `part` (exact designator, any case) whose
/// number or name starts with `pin`.
fn pin_entries(design: &DesignIndex, part: &str, pin: &str, limit: usize) -> Vec<PickerEntry> {
    let Some(found) = design
        .parts()
        .iter()
        .find(|p| p.designator.eq_ignore_ascii_case(part))
    else {
        return Vec::new();
    };
    let needle = pin.to_lowercase();
    let mut pins: Vec<_> = found
        .pins
        .iter()
        .filter(|p| {
            p.number.to_lowercase().starts_with(&needle)
                || p.name.to_lowercase().starts_with(&needle)
        })
        .collect();
    pins.sort_by(|a, b| natural_order(&a.number, &b.number));
    pins.into_iter()
        .take(limit)
        .map(|p| {
            PickerEntry::new(
                Reference::Pin {
                    component: found.designator.clone(),
                    pin: p.number.clone(),
                },
                p.name.clone(),
            )
        })
        .collect()
}
