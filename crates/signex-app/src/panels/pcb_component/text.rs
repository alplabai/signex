//! The Designator and Comment rows (#1312, #2354): the text a single
//! component's fields carry, typed as a draft until Enter, and the eye that
//! shows or hides each text on the board. With several components the
//! Comment is one field for all of them ("Different comments" while they
//! differ) and the Designator says it is edited one at a time.

use std::path::PathBuf;

use iced::widget::Row;
use iced::{Element, Widget as _};
use signex_engine::pcb::designators::FootprintTextField;
use signex_i18n::{I18n, Localized, ids, t};
use uuid::Uuid;

use super::rows::{self, Form};
use super::{ComponentField, DESIGNATOR_FIELD, PcbComponentFacts, PcbComponentMsg, Shared};
use crate::panels::PanelMsg;
use crate::styles::Spacing;

const SP: Spacing = Spacing::DEFAULT;

/// Why the footprint's text cannot be edited, if it cannot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FootprintEditability {
    Editable,
    Locked,
    /// The board opened read-only (a newer format).
    ReadOnly,
}

/// The one selected footprint's Designator and Comment text as the panel
/// edits it.
#[derive(Debug, Clone, PartialEq)]
pub struct PcbFootprintFacts {
    pub board: PathBuf,
    pub uuid: Uuid,
    pub designator: String,
    pub value: String,
    /// The library footprint's name (`footprint_id`); may be empty.
    pub footprint_name: String,
    /// Text typed into the Designator field and not yet applied.
    pub designator_draft: Option<String>,
    /// Text typed into the Comment field and not yet applied.
    pub value_draft: Option<String>,
    pub editability: FootprintEditability,
    /// How many other footprints carry the same designator.
    pub also_on: usize,
}

/// A draft or an Enter in the selected footprint's Designator or Comment
/// field. Both name the board and footprint they were typed for, so a
/// selection change in between cannot send them to another footprint.
#[derive(Debug, Clone, PartialEq)]
pub enum PcbFootprintTextMsg {
    Typed {
        board: PathBuf,
        footprint: Uuid,
        field: FootprintTextField,
        text: String,
    },
    Submitted {
        board: PathBuf,
        footprint: Uuid,
        field: FootprintTextField,
    },
}

/// The Designator or Comment row: the text, editable for one component, and
/// the eye. Several components share one Comment; their designators are
/// edited one at a time.
pub(super) fn text_row<'a>(
    form: &Form<'a>,
    facts: &'a PcbComponentFacts,
    single: Option<&'a PcbFootprintFacts>,
    field: FootprintTextField,
) -> Option<Element<'a, PanelMsg>> {
    let (label, shared, shown) = match field {
        FootprintTextField::Designator => (
            ids::PCB_FOOTPRINT_DESIGNATOR,
            &facts.designator,
            facts.designator_shown.as_ref(),
        ),
        FootprintTextField::Value => (
            ids::PCB_COMPONENT_COMMENT,
            &facts.comment,
            facts.comment_shown.as_ref(),
        ),
    };
    let label = form.text(label);
    if !form.shows(&label) {
        return None;
    }
    let input = match (single, field) {
        (Some(fp), _) => single_input(fp, field),
        // One Comment for every selected component (#2369).
        (None, FootprintTextField::Value) => {
            let differ = (*shared == Shared::Mixed)
                .then(|| t!(form.ctx.i18n, ids::PCB_COMPONENT_COMMENTS_DIFFER));
            super::general::draft_input_with(
                facts,
                ComponentField::Comment,
                shared.same().cloned(),
                differ,
            )
        }
        // Designators are one per component.
        (None, FootprintTextField::Designator) if facts.footprints.len() > 1 => form.with_tip(
            rows::read_only_caption(
                form,
                t!(form.ctx.i18n, ids::PCB_COMPONENT_ONE_AT_A_TIME_SHORT),
            ),
            ids::PCB_COMPONENT_ONE_AT_A_TIME,
        ),
        (None, FootprintTextField::Designator) => {
            rows::input(shared.same().cloned().unwrap_or_default(), None)
        }
    };
    let toggled = (facts.is_editable() && shown.is_some()).then(|| {
        PanelMsg::PcbComponent(PcbComponentMsg::TextShownToggled {
            board: facts.board.clone(),
            footprints: facts.footprints.clone(),
            field,
            shown: matches!(shown, Some(Shared::Same(false))),
        })
    });
    let control = Row::new()
        .push(input)
        .push(rows::eye(form, shown, toggled))
        .spacing(SP.xs)
        .align_y(iced::Alignment::Center)
        .boxed();
    Some(match field {
        FootprintTextField::Designator => rows::linked_row(form, label, DESIGNATOR_FIELD, control),
        FootprintTextField::Value => rows::row(form, label, control),
    })
}

/// The note under the Comment row when several components with different
/// comments are selected: Enter overwrites all of them.
pub(super) fn comment_note<'a>(
    form: &Form<'a>,
    facts: &'a PcbComponentFacts,
    single: Option<&'a PcbFootprintFacts>,
) -> Option<Element<'a, PanelMsg>> {
    let label = form.text(ids::PCB_COMPONENT_COMMENT);
    let several = single.is_none() && facts.footprints.len() > 1;
    (several && facts.comment == Shared::Mixed && facts.is_editable() && form.shows(&label)).then(
        || {
            let line = t!(
                form.ctx.i18n, ids::PCB_COMPONENT_COMMENT_ALL,
                "count" => facts.footprints.len()
            );
            rows::note(form, line, true)
        },
    )
}

/// The input of one footprint's text: its draft, else the stored text;
/// editable unless the footprint is locked or the board read-only.
fn single_input<'a>(fp: &'a PcbFootprintFacts, field: FootprintTextField) -> Element<'a, PanelMsg> {
    let (typed, stored) = match field {
        FootprintTextField::Designator => (fp.designator_draft.as_ref(), &fp.designator),
        FootprintTextField::Value => (fp.value_draft.as_ref(), &fp.value),
    };
    let shown = typed.unwrap_or(stored).clone();
    if fp.editability != FootprintEditability::Editable {
        return rows::input(shown, None);
    }
    let (board, footprint) = (fp.board.clone(), fp.uuid);
    let submitted = PanelMsg::PcbFootprintText(PcbFootprintTextMsg::Submitted {
        board: board.clone(),
        footprint,
        field,
    });
    let typed = move |text| {
        PanelMsg::PcbFootprintText(PcbFootprintTextMsg::Typed {
            board: board.clone(),
            footprint,
            field,
            text,
        })
    };
    rows::input(shown, Some((Box::new(typed), submitted)))
}

/// The lines under the Properties rows of one component: why its text is
/// read-only, a designator other footprints share, and how to apply a draft.
/// An editable designator has no standing note: a mismatch with the schematic
/// shows as the callout in Schematic Reference Information.
pub(super) fn notes(fp: &PcbFootprintFacts, i18n: &I18n) -> Vec<(Localized, bool)> {
    let mut lines = Vec::new();
    let why = match fp.editability {
        FootprintEditability::Editable => None,
        FootprintEditability::Locked => Some(ids::PCB_FOOTPRINT_LOCKED),
        FootprintEditability::ReadOnly => Some(ids::PCB_FOOTPRINT_READ_ONLY),
    };
    if let Some(why) = why {
        lines.push((t!(i18n, why), false));
    }
    if fp.also_on > 0 {
        lines.push((
            t!(
                i18n, ids::PCB_FOOTPRINT_DUPLICATE,
                "designator" => fp.designator.as_str(), "count" => fp.also_on
            ),
            true,
        ));
    }
    if fp.designator_draft.is_some() || fp.value_draft.is_some() {
        lines.push((t!(i18n, ids::PCB_FOOTPRINT_ENTER_HINT), false));
    }
    lines
}
