//! The names an IPC-2581 file gives nets and components, each unique in
//! the file: a qualified name holds no `:`, so `A:B` and `A_B` would be
//! one net without the shared unique-naming pass.

use std::collections::BTreeMap;

use super::xml::qualified;
use crate::fab::marks::{Attributes, Mark};
use crate::fab::naming::unique_names;
use crate::fab::parts::Parts;

pub(crate) struct Names<'a> {
    nets: BTreeMap<String, String>,
    parts: &'a Parts<'a>,
}

impl<'a> Names<'a> {
    /// The names of every net `marks` carry and of `parts`' components.
    pub(crate) fn of<'m>(parts: &'a Parts<'a>, marks: impl Iterator<Item = &'m Mark>) -> Self {
        let nets = unique_names(
            marks.filter_map(|mark| mark_attributes(mark).net.as_deref()),
            qualified,
        );
        Self { nets, parts }
    }

    /// The name a net is written with.
    pub(crate) fn net(&self, net: &str) -> String {
        self.nets
            .get(net)
            .cloned()
            .unwrap_or_else(|| qualified(net))
    }

    /// The component a pad flash belongs to: its part's unique designator,
    /// or for a pad outside the parts list its own designator.
    pub(crate) fn component(&self, attributes: &Attributes) -> Option<String> {
        let designated = attributes
            .pad
            .and_then(|(footprint, _)| self.parts.designator(footprint));
        match designated {
            Some(designator) => Some(designator.to_string()),
            None => attributes
                .pin
                .as_ref()
                .map(|(component, _)| qualified(component)),
        }
    }
}

fn mark_attributes(mark: &Mark) -> &Attributes {
    match mark {
        Mark::Flash { attributes, .. }
        | Mark::Stroke { attributes, .. }
        | Mark::Region { attributes, .. } => attributes,
    }
}
