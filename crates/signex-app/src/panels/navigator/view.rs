//! The Navigator panel (Graphite mockup #54): the Filter sheets field, the Sheets heading
//! with its count, then one row per visible sheet placement (sheet icon,
//! name, file name in muted mono, page number on the right). The focused
//! sheet gets the accent bar and fill; problem rows read in the error
//! colour, loose pages in muted italic. Row clicks and chevrons emit the
//! tree messages the dock handler already routes (`PanelMsg::Navigator`).

use iced::widget::{Column, Space, column, container, row, svg};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Theme, Widget as _};
use signex_i18n::{UserText, ids, t};
use signex_widgets::chips::{ChipMetrics, ChipTone, kind_chip};
use signex_widgets::tree_view::{TreeIcon, TreeMsg};
use signex_widgets::{text, text_input};

use super::{NavigatorMsg, NavigatorRow};
use crate::panels::graphite::selection_fill;
use crate::panels::{EmptyStateAction, PanelContext, PanelMsg, empty_state_card_with_actions};
use crate::styles::{ChromeMetrics, HAIRLINE, Radii, Spacing, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
const CM: ChromeMetrics = ChromeMetrics::DEFAULT;
/// Indent per level of the hierarchy.
const INDENT: f32 = TS.body * 1.35;
/// Width of the page-number column.
const NUMBER_W: f32 = TS.body * 2.0;

fn msg(m: NavigatorMsg) -> PanelMsg {
    PanelMsg::NavigatorPanel(m)
}

/// Render the panel.
pub(crate) fn view<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let navigator = &ctx.navigator;
    if navigator.tree.is_empty() {
        return container(empty_state_card_with_actions(
            &ctx.tokens,
            t!(i18n, ids::PANELS_NAVIGATOR_NO_PROJECT),
            t!(i18n, ids::PANELS_NAVIGATOR_NO_PROJECT_BODY),
            None,
            vec![EmptyStateAction {
                label: t!(i18n, ids::PANELS_NAVIGATOR_OPEN_PROJECT),
                on_press: msg(NavigatorMsg::OpenProjectPressed),
                primary: true,
            }],
        ))
        .padding([SP.l, SP.l])
        .width(Length::Fill)
        .boxed();
    }
    let rows = navigator.visible_rows(i18n);
    let body: Element<'a, PanelMsg> = if rows.is_empty() {
        no_match(ctx)
    } else {
        let mut list: Column<Element<'a, PanelMsg>> = Column::new().width(Length::Fill);
        for line in rows {
            list = list.push(sheet_row(ctx, line));
        }
        iced::widget::scrollable(list)
            .width(Length::Fill)
            .height(Length::Shrink)
            .style(crate::panels::form_look::scrollbar_style)
            .boxed()
    };
    let muted = resolve(&ctx.tokens).ink3;
    column![
        filter_field(ctx),
        heading(ctx),
        body,
        container(
            text(t!(i18n, ids::PANELS_NAVIGATOR_HINT))
                .size(TS.caption)
                .color(muted),
        )
        .padding([SP.s, SP.l]),
    ]
    .width(Length::Fill)
    .boxed()
}

fn filter_field<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    container(
        text_input(
            t!(ctx.i18n, ids::PANELS_NAVIGATOR_FILTER),
            &ctx.navigator.filter,
        )
        .on_input(|s| msg(NavigatorMsg::FilterChanged(s)))
        .style(crate::panels::form_look::text_input_style)
        .size(TS.body)
        .padding([SP.xs, SP.s])
        .width(Length::Fill),
    )
    .padding(iced::Padding {
        top: SP.m,
        right: SP.l,
        bottom: SP.xs,
        left: SP.l,
    })
    .width(Length::Fill)
    .boxed()
}

/// "SHEETS" with the count of sheet placements as a pill.
fn heading<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let r = resolve(&ctx.tokens);
    let title = crate::panels::widgets::section_title(
        t!(ctx.i18n, ids::PANELS_NAVIGATOR_SHEETS),
        &ctx.tokens,
        &ctx.i18n,
    );
    let count = kind_chip(
        t!(ctx.i18n, ids::PANELS_NAVIGATOR_SHEET_COUNT, "count" => ctx.navigator.sheet_count()),
        ChipTone {
            fill: r.sunken,
            ink: r.ink3,
        },
        ChipMetrics::from_text_size(TS.body),
    );
    container(row![title, count].spacing(SP.s).align_y(Alignment::Center))
        .padding([SP.xs, SP.l])
        .width(Length::Fill)
        .boxed()
}

/// One sheet placement. A click selects it (a second click opens it), the
/// chevron folds its branch.
fn sheet_row<'a>(ctx: &'a PanelContext, line: NavigatorRow) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let r = resolve(tokens);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let name_ink = if line.is_problem {
        ti(tokens.error)
    } else if line.is_loose {
        r.ink3
    } else {
        ti(tokens.text)
    };
    let name_font = if line.is_loose {
        Font {
            style: iced::font::Style::Italic,
            ..signex_widgets::ui_font::regular()
        }
    } else {
        signex_widgets::ui_font::regular()
    };
    let chevron: Element<'a, PanelMsg> = if line.has_children {
        signex_widgets::button(signex_widgets::chevron::chevron(
            line.is_expanded,
            TS.body,
            r.ink3,
        ))
        .padding(0)
        .on_press(PanelMsg::Navigator(TreeMsg::Toggle(line.key.clone())))
        .style(crate::styles::ghost_button(tokens))
        .boxed()
    } else {
        Space::new()
            .width(signex_widgets::chevron::side(TS.body))
            .height(Length::Shrink)
            .boxed()
    };
    let icon = svg(TreeIcon::Sheet.svg()).width(TS.body).height(TS.body);
    let number: signex_widgets::Label<'a> = match line.number {
        Some(number) => t!(ctx.i18n, ids::PANELS_NAVIGATOR_SHEET_NUMBER, "number" => number).into(),
        None => UserText::new(String::new()).into(),
    };
    let content = row![
        Space::new()
            .width(INDENT * line.depth as f32)
            .height(Length::Shrink),
        chevron,
        icon,
        container(
            row![
                text(UserText::new(line.label))
                    .size(TS.body)
                    .font(name_font)
                    .color(name_ink)
                    .wrapping(iced::widget::text::Wrapping::None),
                text(UserText::new(line.file))
                    .size(TS.caption)
                    .font(mono)
                    .color(r.ink3)
                    .wrapping(iced::widget::text::Wrapping::None),
            ]
            .spacing(SP.s)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .clip(true),
        container(text(number).size(TS.caption).font(mono).color(r.ink3))
            .width(NUMBER_W)
            .align_x(iced::alignment::Horizontal::Right),
    ]
    .spacing(SP.xs)
    .height(CM.row)
    .align_y(Alignment::Center);
    let is_selected = ctx.navigator.selected.as_deref() == Some(line.path.as_slice());
    let fill = if line.is_active {
        Some(signex_widgets::theme_ext::accent_soft(tokens))
    } else if is_selected {
        Some(selection_fill(tokens))
    } else {
        None
    };
    let bar_ink = line.is_active.then(|| ti(tokens.accent));
    let pressed = signex_widgets::button(content)
        .width(Length::Fill)
        .padding([0.0, SP.s])
        .on_press(PanelMsg::Navigator(TreeMsg::Select(line.key)))
        .style(crate::styles::menu_item(tokens));
    let framed = row![accent_bar(bar_ink), pressed].align_y(Alignment::Center);
    container(framed)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: fill.map(Background::Color),
            border: Border {
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The accent bar on the left of the focused sheet's row; a clear strip of
/// the same width on every other row so the rows line up.
fn accent_bar<'a>(ink: Option<Color>) -> Element<'a, PanelMsg> {
    container(Space::new())
        .width(2.0 * HAIRLINE)
        .height(CM.row)
        .style(move |_: &Theme| container::Style {
            background: ink.map(Background::Color),
            ..container::Style::default()
        })
        .boxed()
}

/// No row holds the filter text: say so, with Clear filter.
fn no_match<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    container(empty_state_card_with_actions(
        &ctx.tokens,
        t!(i18n, ids::PANELS_NAVIGATOR_NO_MATCH),
        t!(
            i18n,
            ids::PANELS_NAVIGATOR_NO_MATCH_BODY,
            "filter" => ctx.navigator.filter.trim(),
            "total" => ctx.navigator.sheet_count()
        ),
        None,
        vec![EmptyStateAction {
            label: t!(i18n, ids::PANELS_NAVIGATOR_CLEAR_FILTER),
            on_press: msg(NavigatorMsg::FilterCleared),
            primary: false,
        }],
    ))
    .padding([SP.xs, SP.l])
    .width(Length::Fill)
    .boxed()
}
