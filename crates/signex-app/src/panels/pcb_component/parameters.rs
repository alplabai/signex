//! The Parameters tab: every parameter the selected components share, each
//! editable as a draft until Enter. The designator and comment texts are in
//! the General tab, not here.

use iced::widget::{Column, container};
use iced::{Element, Length, Widget as _};
use signex_i18n::{ids, t};
use signex_widgets::text;

use super::general::draft_input;
use super::rows::{self, Form};
use super::{ComponentField, PcbComponentFacts};
use crate::panels::{PanelContext, PanelMsg};
use crate::styles::{Spacing, TypeScale, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The tab body.
pub(super) fn view<'a>(
    ctx: &'a PanelContext,
    facts: &'a PcbComponentFacts,
) -> Element<'a, PanelMsg> {
    let form = Form::new(ctx, facts);
    let rows: Vec<Element<'a, PanelMsg>> = facts
        .parameters
        .iter()
        .filter(|p| form.shows(&p.key))
        .map(|p| {
            let field = ComponentField::Parameter(p.key.clone());
            let control = draft_input(facts, field, p.value.same().cloned());
            rows::row(&form, p.key.clone(), control)
        })
        .collect();
    if facts.parameters.is_empty() {
        return container(
            text(t!(ctx.i18n, ids::PCB_COMPONENT_PARAMETERS_NONE))
                .size(TS.body)
                .color(resolve(&ctx.tokens).ink3),
        )
        .padding([SP.m, SP.l])
        .width(Length::Fill)
        .boxed();
    }
    rows.into_iter()
        .fold(
            Column::<Element<'a, PanelMsg>>::new()
                .spacing(0)
                .width(Length::Fill),
            |col, row| col.push(row),
        )
        .padding([SP.s, 0.0])
        .boxed()
}
