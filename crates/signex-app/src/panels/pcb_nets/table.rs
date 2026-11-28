//! The Nets mode of the PCB panel (mockup row 106): filter, chips, the
//! Net / Class / Nodes / Length table and, under it, the selected net; and
//! the page every mode shares (list, detail, foot).

use iced::alignment::Horizontal;
use iced::widget::{Column, Row, column, container, row, scrollable};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, MessageId, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::chips::{ChipMetrics, ChipStyle, chip};
use signex_widgets::{Label, button, text, text_input};

use super::model::{PcbNetChip, PcbNetRow, PcbNetsSummary, PcbPanelMsg, class_of, net_chips};
use super::{SP, TS, board_classes, msg, separator};
use crate::panels::{PanelContext, PanelMsg};
use crate::styles::{ti, tokens::resolve};

/// Height of one table row.
const ROW_H: f32 = TS.body * 2.4;
const CLASS_W: f32 = TS.body * 5.0;
const NODES_W: f32 = TS.body * 3.5;
const LENGTH_W: f32 = TS.body * 4.5;
/// While a detail shows, the list keeps the height of its rows up to this
/// many net rows and the detail takes the rest, so a short list leaves the
/// detail room to show whole at the dock's usual height (#3001).
const LIST_MAX_ROWS: f32 = 8.0;
const LIST_MAX_H: f32 = ROW_H * LIST_MAX_ROWS;

pub(super) fn chip_style(tokens: &ThemeTokens) -> ChipStyle {
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

fn chip_label<'a>(i18n: &I18n, value: &PcbNetChip) -> Label<'a> {
    match value {
        PcbNetChip::All => t!(i18n, ids::PANELS_PCB_NETS_CHIP_ALL).into(),
        PcbNetChip::Unrouted => t!(i18n, ids::PANELS_PCB_NETS_CHIP_UNROUTED).into(),
        PcbNetChip::Class(name) => UserText::new(name.clone()).into(),
    }
}

/// The filter field and the All / Unrouted / per-class chips.
fn top<'a>(ctx: &'a PanelContext, summary: &PcbNetsSummary) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let ui = &ctx.pcb_panel;
    let classes = board_classes(ctx);
    let filter = text_input(t!(i18n, ids::PANELS_PCB_NETS_FILTER), &ui.filter)
        .on_input(|s| msg(PcbPanelMsg::FilterChanged(s)))
        .style(crate::panels::form_look::text_input_style)
        .size(TS.body)
        .padding([SP.xs, SP.s])
        .width(Length::Fill);
    let in_force = ui.chip_in_force(summary, classes);
    let metrics = ChipMetrics::from_text_size(TS.body);
    let mut chips: Row<Element<'a, PanelMsg>> = Row::new().spacing(metrics.row_gap);
    for (value, count) in net_chips(summary, classes) {
        chips = chips.push(chip(
            chip_label(i18n, &value),
            Some(u32::try_from(count).unwrap_or(u32::MAX)),
            value == in_force,
            chip_style(&ctx.tokens),
            metrics,
            i18n,
            Some(msg(PcbPanelMsg::ChipPicked(value))),
        ));
    }
    let chips = chips.wrap().vertical_spacing(metrics.row_gap).boxed();
    column![filter, chips]
        .spacing(SP.s)
        .padding([SP.m, SP.l])
        .width(Length::Fill)
        .boxed()
}

/// A fixed-height cell; numbers sit at its right edge.
fn cell<'a>(
    content: Element<'a, PanelMsg>,
    width: Length,
    align: Horizontal,
) -> Element<'a, PanelMsg> {
    container(content)
        .width(width)
        .height(ROW_H)
        .align_x(align)
        .align_y(iced::alignment::Vertical::Center)
        .clip(true)
        .boxed()
}

fn header<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let muted = resolve(&ctx.tokens).ink3;
    let head = |id: MessageId, width: Length, align: Horizontal| {
        cell(
            text(t!(i18n, id)).size(TS.section).color(muted).boxed(),
            width,
            align,
        )
    };
    row![
        head(
            ids::PANELS_PCB_NETS_COLUMN_NET,
            Length::Fill,
            Horizontal::Left
        ),
        head(
            ids::PANELS_PCB_NETS_COLUMN_CLASS,
            Length::Fixed(CLASS_W),
            Horizontal::Left
        ),
        head(
            ids::PANELS_PCB_NETS_COLUMN_NODES,
            Length::Fixed(NODES_W),
            Horizontal::Right
        ),
        head(
            ids::PANELS_PCB_NETS_COLUMN_LENGTH,
            Length::Fixed(LENGTH_W),
            Horizontal::Right
        ),
    ]
    .padding([0.0, SP.l])
    .boxed()
}

/// One net: a click selects it and highlights it on the board.
fn net_row<'a>(ctx: &'a PanelContext, net: &'a PcbNetRow) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let ink = ti(ctx.tokens.text);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    // A length is an engineering value: `.` in every locale.
    let length: Label<'a> = if net.length_mm > 0.0 {
        UserText::new(format!("{:.2}", net.length_mm)).into()
    } else {
        t!(i18n, ids::PANELS_PCB_NETS_NO_LENGTH).into()
    };
    let class: Label<'a> = match class_of(super::board_classes(ctx), &net.name) {
        Some(name) => UserText::new(name.to_owned()).into(),
        None => t!(i18n, ids::PANELS_PCB_NETS_NO_CLASS).into(),
    };
    let line = row![
        cell(
            text(UserText::new(net.name.as_str()))
                .size(TS.body)
                .font(mono)
                .color(ink)
                .boxed(),
            Length::Fill,
            Horizontal::Left
        ),
        cell(
            text(class).size(TS.caption).color(ink).boxed(),
            Length::Fixed(CLASS_W),
            Horizontal::Left
        ),
        cell(
            text(t!(i18n, ids::PANELS_PCB_NETS_NODES, "count" => net.nodes))
                .size(TS.body)
                .font(mono)
                .color(ink)
                .boxed(),
            Length::Fixed(NODES_W),
            Horizontal::Right
        ),
        cell(
            text(length).size(TS.body).font(mono).color(ink).boxed(),
            Length::Fixed(LENGTH_W),
            Horizontal::Right
        ),
    ];
    let is_selected = ctx.pcb_panel.selected.as_deref() == Some(net.name.as_str());
    button(line)
        .width(Length::Fill)
        .padding([0.0, SP.l])
        .on_press(msg(PcbPanelMsg::NetClicked(net.name.clone())))
        .style(crate::styles::hover_row(&ctx.tokens, is_selected))
        .boxed()
}

/// A note in a list's place: why it is empty.
pub(super) fn note<'a>(ctx: &PanelContext, id: MessageId) -> Element<'a, PanelMsg> {
    container(
        text(t!(&*ctx.i18n, id))
            .size(TS.body)
            .color(resolve(&ctx.tokens).ink3),
    )
    .padding([SP.m, SP.l])
    .width(Length::Fill)
    .boxed()
}

/// The rows, or why there are none.
fn rows<'a>(ctx: &'a PanelContext, summary: &'a PcbNetsSummary) -> Column<Element<'a, PanelMsg>> {
    let i18n = &*ctx.i18n;
    let shown = ctx.pcb_panel.shown(summary, board_classes(ctx), i18n);
    let mut list: Column<Element<'a, PanelMsg>> = Column::new().width(Length::Fill);
    if shown.is_empty() {
        let id = if summary.nets.is_empty() {
            ids::PANELS_PCB_NETS_EMPTY
        } else {
            ids::PANELS_PCB_NETS_NO_MATCH
        };
        list = list.push(note(ctx, id));
    }
    for net in shown {
        list = list.push(net_row(ctx, net));
        list = list.push(separator(ctx));
    }
    list
}

/// A mode's foot line: its counts.
pub(super) fn foot<'a>(ctx: &PanelContext, label: Label<'a>) -> Element<'a, PanelMsg> {
    container(
        text(label)
            .size(TS.caption)
            .color(resolve(&ctx.tokens).ink3),
    )
    .padding([SP.m, SP.l])
    .width(Length::Fill)
    .boxed()
}

fn footer<'a>(ctx: &'a PanelContext, summary: &PcbNetsSummary) -> Element<'a, PanelMsg> {
    let counts = t!(
        &*ctx.i18n,
        ids::PANELS_PCB_NETS_SUMMARY,
        "nets" => summary.nets.len(),
        "pads" => summary.pads
    );
    foot(ctx, counts.into())
}

/// A mode's page: `head` over the scrolling `list`, the selected item's
/// `detail` under it, and the `foot`. With a detail the list shrinks to
/// its rows, at most [`LIST_MAX_H`], and the detail fills the rest.
pub(super) fn page<'a>(
    ctx: &PanelContext,
    head: Vec<Element<'a, PanelMsg>>,
    list: Column<Element<'a, PanelMsg>>,
    detail: Option<Element<'a, PanelMsg>>,
    foot: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    // The find results box's bounded list: a shrinking scrollable in a
    // box that fits it up to the cap.
    let (scroll_h, box_h) = if detail.is_some() {
        (Length::Shrink, Length::Fit.max(LIST_MAX_H))
    } else {
        (Length::Fill, Length::Fill)
    };
    let list = container(
        scrollable(list)
            .width(Length::Fill)
            .height(scroll_h)
            .style(crate::panels::form_look::scrollbar_style),
    )
    .width(Length::Fill)
    .height(box_h)
    .clip(true)
    .boxed();
    let mut page: Column<Element<'a, PanelMsg>> =
        Column::new().width(Length::Fill).height(Length::Fill);
    for part in head {
        page = page.push(part);
    }
    page = page.push(list);
    if let Some(detail) = detail {
        page = page.push(separator(ctx));
        page = page.push(
            container(detail)
                .width(Length::Fill)
                .height(Length::Fill)
                .boxed(),
        );
    }
    page.push(separator(ctx)).push(foot).boxed()
}

/// The whole mode: list on top, the selected net's detail below it.
pub(super) fn nets_mode<'a>(
    ctx: &'a PanelContext,
    summary: &'a PcbNetsSummary,
) -> Element<'a, PanelMsg> {
    let detail = ctx
        .pcb_panel
        .selected_net(summary)
        .map(|net| super::detail::view(ctx, net));
    let head = vec![top(ctx, summary), header(ctx), separator(ctx)];
    page(ctx, head, rows(ctx, summary), detail, footer(ctx, summary))
}
