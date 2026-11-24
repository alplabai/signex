//! What the schematic says about the components (#2367), in Schematic
//! Reference Information: the schematic designator, Design Item ID and
//! Source, read-only. When the board designator differs from the
//! schematic's, a warning note leads the section with an Update PCB button
//! (Graphite mockup row 94) that runs Update PCB as the Design menu does.

use iced::Element;
use signex_i18n::{MessageId, ids, t};

use super::rows::{self, Form};
use super::{PartIdentity, PcbComponentFacts, PcbComponentMsg, Shared};
use crate::panels::PanelMsg;

/// One fact of a read part, or the reason it could not be read.
fn shown(
    form: &Form<'_>,
    identity: &Shared<PartIdentity>,
    read: impl Fn(&PartIdentity) -> Option<String>,
    missing: MessageId,
) -> String {
    match identity {
        Shared::Same(PartIdentity::NotLinked) => form.text(ids::PCB_COMPONENT_NOT_LINKED),
        Shared::Same(PartIdentity::NotRead) => form.text(ids::PCB_COMPONENT_SCH_NOT_READ),
        Shared::Same(part) => read(part).unwrap_or_else(|| form.text(missing)),
        Shared::Mixed => String::new(),
    }
}

fn value_row<'a>(
    form: &Form<'a>,
    label: MessageId,
    value: impl FnOnce() -> String,
) -> Option<Element<'a, PanelMsg>> {
    let label = form.text(label);
    form.shows(&label)
        .then(|| rows::row(form, label, rows::read_only(form, value())))
}

/// The Design Item ID and Source rows.
pub(super) fn library_rows<'a>(
    form: &Form<'a>,
    facts: &'a PcbComponentFacts,
) -> Vec<Element<'a, PanelMsg>> {
    let identity = &facts.identity;
    let item = value_row(form, ids::PCB_COMPONENT_DESIGN_ITEM_ID, || {
        shown(
            form,
            identity,
            |part| match part {
                PartIdentity::Read { design_item_id, .. } => design_item_id.clone(),
                _ => None,
            },
            ids::PCB_COMPONENT_NO_LIBRARY,
        )
    });
    let source = value_row(form, ids::PCB_COMPONENT_SOURCE, || {
        shown(
            form,
            identity,
            |part| match part {
                PartIdentity::Read { source, .. } => source.clone(),
                _ => None,
            },
            ids::PCB_COMPONENT_SOURCE_NOT_OPEN,
        )
    });
    item.into_iter().chain(source).collect()
}

/// The mismatch note when a designator differs, then the schematic
/// Designator row.
pub(super) fn designator_rows<'a>(
    form: &Form<'a>,
    facts: &'a PcbComponentFacts,
) -> Vec<Element<'a, PanelMsg>> {
    let row = value_row(form, ids::PCB_COMPONENT_SCH_DESIGNATOR, || {
        shown(
            form,
            &facts.identity,
            |part| match part {
                PartIdentity::Read { designator, .. } => Some(designator.clone()),
                _ => None,
            },
            ids::PCB_COMPONENT_SCH_NOT_READ,
        )
    });
    let Some(row) = row else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if facts.designator_mismatches > 0 {
        out.push(mismatch_note(form, facts));
    }
    out.push(row);
    out
}

/// "The schematic names this component U4, the board U3." (or the count of
/// several) with the Update PCB button; the button is off on a read-only
/// board.
fn mismatch_note<'a>(form: &Form<'a>, facts: &'a PcbComponentFacts) -> Element<'a, PanelMsg> {
    let one = match (&facts.identity, &facts.designator) {
        (Shared::Same(PartIdentity::Read { designator, .. }), Shared::Same(board))
            if facts.footprints.len() == 1 =>
        {
            Some(t!(
                form.ctx.i18n, ids::PCB_COMPONENT_SCH_MISMATCH,
                "schematic" => designator.as_str(), "board" => board.as_str()
            ))
        }
        _ => None,
    };
    let line = one.unwrap_or_else(|| {
        t!(
            form.ctx.i18n, ids::PCB_COMPONENT_SCH_MISMATCH_COUNT,
            "count" => facts.designator_mismatches
        )
    });
    let pressed =
        (!facts.read_only).then_some(PanelMsg::PcbComponent(PcbComponentMsg::UpdatePcbPressed));
    let update = rows::small_button(t!(form.ctx.i18n, ids::PCB_COMPONENT_UPDATE_PCB), pressed);
    rows::callout(form, line, update)
}
