//! The tools block above the list: the search field with Refresh, the kind
//! chips with their counts, and the Part, Status and Date filter fields.

use iced::widget::{Column, Row, container, pick_list, row, svg};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Theme, Widget as _};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_widgets::chips::{ChipMetrics, ChipStyle, count_text, dot_chip};
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::theme_ext::annotation_kind_colour;
use signex_widgets::{PickListPlaceholder as _, button, text, text_input};

use super::{
    AnnotationsMsg, AnnotationsView, DateWindow, KindChip, METRICS, PAD, PAD_TIGHT, Status,
    TOOL_GAP, TS, kind_label, msg, rows, status_label, tone,
};
use crate::panels::PanelContext;
use crate::panels::PanelMsg;
use crate::styles::{LINE_HEIGHT, ti, tokens::resolve};

/// The filter fields' side padding.
const FIELD_PAD: f32 = TS.body * 6.0 / 12.0;
/// The search and clear glyphs.
const GLYPH: f32 = TS.body;

const SEARCH: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><circle cx="7" cy="7" r="4.3" fill="none" stroke="currentColor" stroke-width="1.4"/><path d="M10.2 10.2l3.3 3.3" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>"#;
const CLEAR: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M4.5 4.5l7 7M11.5 4.5l-7 7" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>"#;
const REFRESH: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M13 8a5 5 0 11-1.5-3.6M13 2.5v3h-3" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;

/// The block: search, chips, filters; 8 / 8 / 4 padding, a soft rule under.
pub(super) fn tools<'a>(ctx: &'a PanelContext, view: &'a AnnotationsView) -> Element<'a, PanelMsg> {
    let block = Column::new()
        .push(search(ctx, view))
        .push(kind_chips(ctx, &view.kinds))
        .push(filter_row(ctx, view))
        .spacing(TOOL_GAP)
        .padding(Padding {
            top: PAD,
            right: PAD,
            bottom: PAD_TIGHT,
            left: PAD,
        })
        .width(Length::Fill);
    Column::new()
        .push(block.boxed())
        .push(super::rule(resolve(&ctx.tokens).line_soft))
        .width(Length::Fill)
        .boxed()
}

pub(super) fn glyph<'a>(bytes: &'static [u8], size: f32, ink: Color) -> Element<'a, PanelMsg> {
    svg(crate::icons::static_glyph(bytes))
        .width(size)
        .height(size)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..svg::Style::default()
        })
        .boxed()
}

/// The search field, and Refresh (the mock-up's More ▸ Refresh) beside it.
fn search<'a>(ctx: &'a PanelContext, view: &'a AnnotationsView) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let r = resolve(tokens);
    let input = text_input(t!(ctx.i18n, ids::ANNOTATIONS_SEARCH), &view.query)
        .on_input(|q| msg(AnnotationsMsg::QueryChanged(q)))
        .size(TS.body)
        .padding([0.0, PAD_TIGHT])
        .style(bare_input(tokens))
        .width(Length::Fill);
    let field = container(
        row![glyph(SEARCH, GLYPH, r.ink4), input]
            .align_y(Alignment::Center)
            .padding(Padding {
                left: PAD,
                ..Padding::ZERO
            }),
    )
    .width(Length::Fill)
    .center_y(METRICS.field)
    .style(crate::styles::field(tokens, false));
    let refresh = button(glyph(REFRESH, GLYPH, r.ink3))
        .width(METRICS.field)
        .height(METRICS.field)
        .padding(0)
        .on_press(msg(AnnotationsMsg::RefreshRequested))
        .style(crate::styles::menu_item(tokens));
    let refresh = crate::panels::graphite::hint_tooltip(
        container(refresh).center(METRICS.field).boxed(),
        t!(ctx.i18n, ids::ANNOTATIONS_REFRESH),
        tokens,
        iced::widget::tooltip::Position::Bottom,
    );
    row![field, refresh]
        .spacing(PAD_TIGHT)
        .align_y(Alignment::Center)
        .boxed()
}

/// A search field without its own frame: the field container draws it.
fn bare_input(
    tokens: &signex_types::theme::ThemeTokens,
) -> impl Fn(&Theme, text_input::Status) -> text_input::Style + 'static {
    let r = resolve(tokens);
    let (ink, accent) = (ti(tokens.text), ti(tokens.accent));
    move |_, _| text_input::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border::default(),
        placeholder: r.ink4,
        value: ink,
        selection: Color { a: 0.4, ..accent },
    }
}

/// One chip per kind, each with its colour dot and count; a chip that is
/// on shows its kind. The shared `dot_chip` with its dot slot.
fn kind_chips<'a>(ctx: &'a PanelContext, kinds: &[KindChip]) -> Element<'a, PanelMsg> {
    let metrics = ChipMetrics::from_text_size(TS.body);
    let style = chip_style(&ctx.tokens);
    let mut chips: Row<Element<'a, PanelMsg>> = Row::new().spacing(PAD_TIGHT);
    for chip in kinds {
        chips = chips.push(dot_chip(
            annotation_kind_colour(tone(chip.kind), &ctx.tokens),
            kind_label(&ctx.i18n, chip.kind),
            Some(count_text(&ctx.i18n, chip.count)),
            chip.on,
            style,
            metrics,
            Some(msg(AnnotationsMsg::KindToggled(chip.kind))),
        ));
    }
    chips.wrap().vertical_spacing(PAD_TIGHT).boxed()
}

/// The colours of a kind chip.
fn chip_style(tokens: &signex_types::theme::ThemeTokens) -> ChipStyle {
    let r = resolve(tokens);
    ChipStyle {
        fill: r.field,
        edge: ti(tokens.border),
        hover: ti(tokens.hover),
        ink: r.ink3,
        on_fill: r.accent_soft,
        on_ink: r.accent_soft_ink,
    }
}

/// Part, Status and Date as three equal fields. A field with a value shows
/// it with an x that clears it; an empty one is a dropdown.
fn filter_row<'a>(ctx: &'a PanelContext, view: &'a AnnotationsView) -> Element<'a, PanelMsg> {
    let i18n: &'a I18n = &ctx.i18n;
    let part: Element<'a, PanelMsg> = match &view.part {
        Some(part) => picked(
            ctx,
            UserText::new(part.as_str()).into(),
            true,
            AnnotationsMsg::PartCleared,
        ),
        None => dropdown(
            pick_list(None::<String>, view.parts.clone(), String::clone)
                .placeholder_label(t!(i18n, ids::ANNOTATIONS_PART_ANY))
                .on_select(|part| msg(AnnotationsMsg::PartPicked(part))),
        ),
    };
    let status: Element<'a, PanelMsg> = match view.status_filter {
        Some(status) => picked(
            ctx,
            status_label(i18n, status).into(),
            false,
            AnnotationsMsg::StatusCleared,
        ),
        None => dropdown(
            pick_list(None::<Status>, view.statuses.clone(), move |s: &Status| {
                status_label(i18n, *s).into_string()
            })
            .placeholder_label(t!(i18n, ids::ANNOTATIONS_STATUS_ANY))
            .on_select(|status| msg(AnnotationsMsg::StatusPicked(status))),
        ),
    };
    let date: Element<'a, PanelMsg> = match view.date {
        DateWindow::Any => dropdown(
            pick_list(
                Some(DateWindow::Any),
                DateWindow::ALL.to_vec(),
                move |w: &DateWindow| date_label(i18n, *w).into_string(),
            )
            .on_select(|window| msg(AnnotationsMsg::DatePicked(window))),
        ),
        window => picked(
            ctx,
            date_label(i18n, window).into(),
            false,
            AnnotationsMsg::DatePicked(DateWindow::Any),
        ),
    };
    row![cell(part), cell(status), cell(date),]
        .spacing(PAD_TIGHT)
        .boxed()
}

fn cell<'a>(content: Element<'a, PanelMsg>) -> Element<'a, PanelMsg> {
    container(content)
        .width(Length::FillPortion(1))
        .height(METRICS.row)
        .boxed()
}

pub(super) fn date_label(i18n: &I18n, window: DateWindow) -> Localized {
    match window {
        DateWindow::Any => t!(i18n, ids::ANNOTATIONS_DATE_ANY),
        DateWindow::Last7Days => t!(i18n, ids::ANNOTATIONS_DATE_LAST_7),
        DateWindow::Last30Days => t!(i18n, ids::ANNOTATIONS_DATE_LAST_30),
    }
}

/// A Graphite dropdown at caption size filling its cell.
fn dropdown<'a, T, L, V>(
    list: iced::widget::PickList<'a, T, L, V, PanelMsg>,
) -> Element<'a, PanelMsg>
where
    T: PartialEq + Clone + 'a,
    L: std::borrow::Borrow<[T]> + 'a,
    V: std::borrow::Borrow<T> + 'a,
{
    // The padding makes the closed field the 24 px row tall.
    let pad_y = ((METRICS.row - TS.caption * LINE_HEIGHT) / 2.0).max(0.0);
    list.graphite()
        .text_size(TS.caption)
        .padding([pad_y, FIELD_PAD])
        .width(Length::Fill)
        .ellipsis(iced::widget::text::Ellipsis::End)
        .boxed()
}

/// A filter field holding a value, focused-looking, with an x to clear it.
fn picked<'a>(
    ctx: &'a PanelContext,
    value: signex_widgets::Label<'a>,
    mono: bool,
    clear: AnnotationsMsg,
) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let ink3 = resolve(tokens).ink3;
    let mut value = text(value)
        .size(TS.caption)
        .color(ti(tokens.text))
        .wrapping(iced::widget::text::Wrapping::None)
        .ellipsis(iced::widget::text::Ellipsis::End)
        .width(Length::Fill);
    if mono {
        value = value.font(rows::mono());
    }
    let x = button(glyph(CLEAR, GLYPH, ink3))
        .padding(0)
        .on_press(msg(clear))
        .style(crate::styles::menu_item(tokens));
    let x = crate::panels::graphite::hint_tooltip(
        x.boxed(),
        t!(ctx.i18n, ids::ANNOTATIONS_CLEAR_FILTER),
        tokens,
        iced::widget::tooltip::Position::Bottom,
    );
    container(row![value, x].align_y(Alignment::Center).spacing(PAD_TIGHT))
        .padding([0.0, FIELD_PAD])
        .center_y(METRICS.row)
        .width(Length::Fill)
        .style(crate::styles::field(tokens, true))
        .boxed()
}
