//! The Parameters tab: every parameter the selected components share, each
//! editable as a draft until Enter and removable with its button, then a row
//! that adds a parameter to all of them (#2369). The designator and comment
//! texts are in the General tab, not here.

use iced::widget::{Column, Row, container, svg};
use iced::{Element, Length, Theme, Widget as _};
use signex_i18n::{MessageId, ids, t};
use signex_widgets::{button, text, text_input};

use super::general::draft_input;
use super::rows::{self, Form};
use super::{ComponentField, PcbComponentFacts, PcbComponentMsg};
use crate::panels::{PanelContext, PanelMsg, form_look};
use crate::styles::{Spacing, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The tab body.
pub(super) fn view<'a>(
    ctx: &'a PanelContext,
    facts: &'a PcbComponentFacts,
) -> Element<'a, PanelMsg> {
    let form = Form::new(ctx, facts);
    let mut col = Column::<Element<'a, PanelMsg>>::new()
        .spacing(0)
        .width(Length::Fill);
    if facts.parameters.is_empty() {
        col = col.push(
            container(
                text(t!(ctx.i18n, ids::PCB_COMPONENT_PARAMETERS_NONE))
                    .size(TS.body)
                    .color(resolve(&ctx.tokens).ink3),
            )
            .padding([SP.m, SP.l])
            .width(Length::Fill)
            .boxed(),
        );
    }
    for p in facts.parameters.iter().filter(|p| form.shows(&p.key)) {
        let field = ComponentField::Parameter(p.key.clone());
        let control = Row::new()
            .push(draft_input(facts, field, p.value.same().cloned()))
            .push(remove_button(&form, facts, &p.key))
            .spacing(SP.xs)
            .align_y(iced::Alignment::Center)
            .boxed();
        col = col.push(rows::row(&form, p.key.clone(), control));
    }
    if facts.is_editable() {
        col = col.push(add_row(&form, facts));
    }
    col.padding([SP.s, 0.0]).boxed()
}

/// The button that removes `key` from every selected component.
fn remove_button<'a>(
    form: &Form<'a>,
    facts: &'a PcbComponentFacts,
    key: &str,
) -> Element<'a, PanelMsg> {
    let ink = ti(form.ctx.tokens.text);
    let icon = svg(crate::icons::icon_close(form.ctx.theme_id))
        .width(TS.body)
        .height(TS.body)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        });
    let mut remove = button(icon)
        .padding([SP.s, SP.s])
        .style(form_look::ghost_button);
    if facts.is_editable() {
        remove = remove.on_press(PanelMsg::PcbComponent(PcbComponentMsg::ParameterRemoved {
            board: facts.board.clone(),
            footprints: facts.footprints.clone(),
            key: key.to_string(),
        }));
    }
    form.with_tip(remove.boxed(), ids::PCB_COMPONENT_PARAMETER_REMOVE_TIP)
}

/// The name and value of a new parameter and the Add button; Enter in
/// either field adds it too.
fn add_row<'a>(form: &Form<'a>, facts: &'a PcbComponentFacts) -> Element<'a, PanelMsg> {
    let added = PanelMsg::PcbComponent(PcbComponentMsg::ParameterAdded {
        board: facts.board.clone(),
        footprints: facts.footprints.clone(),
    });
    let field = |which: ComponentField, placeholder: MessageId| {
        let shown = facts.draft(&which).cloned().unwrap_or_default();
        let (board, footprints) = (facts.board.clone(), facts.footprints.clone());
        text_input(t!(form.ctx.i18n, placeholder), shown)
            .on_input(move |text| {
                PanelMsg::PcbComponent(PcbComponentMsg::FieldTyped {
                    board: board.clone(),
                    footprints: footprints.clone(),
                    field: which.clone(),
                    text,
                })
            })
            .on_submit(added.clone())
            .size(TS.body)
            .padding(SP.s)
            .style(form_look::text_input_style)
            .width(Length::Fill)
    };
    let add = button(text(t!(form.ctx.i18n, ids::PCB_COMPONENT_PARAMETER_ADD)).size(TS.body))
        .padding([SP.s, SP.m])
        .style(form_look::ghost_button)
        .on_press(added.clone());
    container(
        Row::<Element<'a, PanelMsg>>::new()
            .push(
                field(
                    ComponentField::NewParameterName,
                    ids::PCB_COMPONENT_PARAMETER_NAME,
                )
                .boxed(),
            )
            .push(
                field(
                    ComponentField::NewParameterValue,
                    ids::PCB_COMPONENT_PARAMETER_VALUE,
                )
                .boxed(),
            )
            .push(add.boxed())
            .spacing(SP.xs)
            .align_y(iced::Alignment::Center),
    )
    .padding([SP.s, SP.l])
    .width(Length::Fill)
    .boxed()
}
