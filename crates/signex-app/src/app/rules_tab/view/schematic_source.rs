//! Where a rule came from (#1489): a rule Update PCB wrote from the
//! schematic's Net Parameters directives reads "from schematic" under its
//! name. The schematic wins on the next Update PCB, so an edit made here to
//! such a rule is rewritten unless the directive changes too.

use iced::{Element, Widget as _};
use signex_drc_dsl::table::RuleRow;
use signex_i18n::{ids, t};
use signex_widgets::text;

use super::{Frame, TS};
use crate::app::Message;

/// The source caption of `row`, `None` for a rule written here.
pub(super) fn caption<'a>(frame: &Frame<'a>, row: &RuleRow) -> Option<Element<'a, Message>> {
    let from_schematic = frame
        .layer
        .is_some_and(|layer| layer.is_from_schematic(&row.name));
    from_schematic.then(|| {
        text(t!(frame.i18n, ids::RULES_FROM_SCHEMATIC))
            .size(TS.caption)
            .color(crate::styles::tokens::resolve(frame.tokens).accent_text)
            .boxed()
    })
}
