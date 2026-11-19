//! The Component view's header block (Graphite m94; order per decision
//! pcb-08): the title row (Component, the selection pill, the footprint name
//! of one component), the General / Parameters tabs with the parameter
//! count, then the row search. The illustration follows it on the General
//! tab.

use iced::widget::{Column, Row, Space, container, svg};
use iced::{Element, Length, Padding, Theme, Widget as _};
use signex_i18n::{UserText, ids, t};
use signex_widgets::{button, text, text_input};

use super::rows::{self, Form};
use super::{PcbComponentFacts, PcbComponentMsg, Shared};
use crate::panels::{PanelContext, PanelMsg, form_look};
use crate::styles::tokens::resolve;
use crate::styles::{PillTone, Spacing, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The header block.
pub(super) fn view<'a>(
    ctx: &'a PanelContext,
    facts: &'a PcbComponentFacts,
) -> Element<'a, PanelMsg> {
    let form = Form::new(ctx, facts);
    container(
        Column::<Element<'a, PanelMsg>>::new()
            .push(title(ctx, facts))
            .push(tabs(ctx, facts))
            .push(search(&form, facts))
            .spacing(SP.m),
    )
    .padding(Padding {
        top: SP.m,
        right: SP.m,
        bottom: SP.xs,
        left: SP.m,
    })
    .width(Length::Fill)
    .boxed()
}

/// "Component", the accent pill with the designator (or the count of
/// several), and the footprint name of one component at the end.
fn title<'a>(ctx: &'a PanelContext, facts: &'a PcbComponentFacts) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let semibold = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    };
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let one = facts.footprints.len() == 1;
    let summary: signex_widgets::Label<'a> = match &facts.designator {
        Shared::Same(designator) if one => UserText::new(designator.clone()).into(),
        _ => t!(i18n, ids::PCB_COMPONENT_COUNT, "count" => facts.footprints.len()).into(),
    };
    let pill = container(text(summary).size(TS.caption))
        .padding([0.0, SP.s])
        .style(crate::styles::pill(tokens, PillTone::Accent));
    let mut row = Row::<Element<'a, PanelMsg>>::new()
        .push_boxed(
            text(t!(i18n, ids::PCB_COMPONENT_TITLE))
                .size(TS.label)
                .font(semibold)
                .color(ti(tokens.text)),
        )
        .push_boxed(pill)
        .push_boxed(Space::new().width(Length::Fill));
    if let (true, Shared::Same(name)) = (one, &facts.footprint_name) {
        row = row.push_boxed(
            text(UserText::new(name.clone()))
                .size(TS.caption)
                .font(mono)
                .color(resolve(tokens).ink4)
                .wrapping(iced::widget::text::Wrapping::None)
                .ellipsis(iced::widget::text::Ellipsis::End),
        );
    }
    row.spacing(SP.s).align_y(iced::Alignment::Center).boxed()
}

/// General / Parameters as a segmented control; the Parameters cell carries
/// the parameter count.
fn tabs<'a>(ctx: &'a PanelContext, facts: &'a PcbComponentFacts) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let on_parameters = ctx.properties_tab == 1;
    let count = text(t!(
        i18n,
        ids::PCB_COMPONENT_PARAMETER_COUNT,
        "count" => facts.parameters.len()
    ))
    .size(TS.caption)
    .font(crate::fonts::iced_font_for_family(
        crate::fonts::CHROME_MONO_FONT,
    ))
    .color(resolve(&ctx.tokens).ink4);
    let cell = |content: Row<Element<'a, PanelMsg>>, active: bool, index: usize| {
        button(
            container(content.spacing(SP.xs).align_y(iced::Alignment::Center))
                .center_x(Length::Fill),
        )
        .padding([SP.xs, SP.s])
        .width(Length::Fill)
        .on_press(PanelMsg::PropertiesTab(index))
        .style(move |theme: &Theme, status| {
            let hovered = matches!(status, iced::widget::button::Status::Hovered);
            form_look::segment_cell(theme, active, hovered && !active)
        })
        .boxed()
    };
    let general =
        Row::new().push_boxed(text(t!(i18n, ids::PANELS_PROPERTIES_TAB_GENERAL)).size(TS.body));
    let parameters = Row::new()
        .push_boxed(text(t!(i18n, ids::PANELS_PROPERTIES_TAB_PARAMETERS)).size(TS.body))
        .push_boxed(count);
    container(
        Row::new()
            .push(cell(general, !on_parameters, 0))
            .push(cell(parameters, on_parameters, 1))
            .spacing(SP.xs)
            .width(Length::Fill),
    )
    .padding(SP.xs)
    .width(Length::Fill)
    .style(form_look::segmented_frame)
    .boxed()
}

/// The row search: the search glyph and a bare input in the field frame.
fn search<'a>(form: &Form<'a>, facts: &'a PcbComponentFacts) -> Element<'a, PanelMsg> {
    let ink = resolve(&form.ctx.tokens).ink4;
    let icon = svg(crate::icons::icon_chrome_search(
        signex_types::theme::ThemeId::GraphiteDark,
    ))
    .width(TS.body)
    .height(TS.body)
    .style(move |_: &Theme, _| svg::Style {
        color: Some(ink),
        ..Default::default()
    });
    let input = text_input(
        t!(form.ctx.i18n, ids::PCB_COMPONENT_SEARCH),
        facts.search.clone(),
    )
    .on_input(|query| PanelMsg::PcbComponent(PcbComponentMsg::SearchChanged(query)))
    .size(TS.body)
    .padding([SP.s, SP.s])
    .style(rows::bare_input_style)
    .width(Length::Fill);
    rows::framed(
        form,
        Row::new()
            .push_boxed(icon)
            .push_boxed(input)
            .align_y(iced::Alignment::Center)
            .boxed(),
    )
}
