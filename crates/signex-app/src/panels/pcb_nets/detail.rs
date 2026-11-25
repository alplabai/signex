//! The selected net under the Nets list (mockup row 106): its class and
//! length, its pads, and its track length per layer plus vias; and the
//! selected differential pair under the Pairs list.

use iced::widget::{Column, Space, column, row, scrollable};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::format::parse_pcb_layer;
use signex_types::net_class::{NetClass, first_matching_class};
use signex_widgets::{Label, text};

use super::model::{PcbDiffPairRow, PcbNetPad, PcbNetRow, class_of};
use super::{SP, TS, board_classes};
use crate::panels::{PanelContext, PanelMsg};
use crate::pcb_canvas::layer_style::layer_name;
use crate::styles::{ti, tokens::resolve};

/// A layer by its catalog name, or as the board writes one this build does
/// not know.
pub(super) fn layer_text<'a>(i18n: &I18n, token: &str) -> Label<'a> {
    match parse_pcb_layer(token) {
        Some(layer) => layer_name(i18n, layer).into(),
        None => UserText::new(token.to_owned()).into(),
    }
}

/// Where a pad is: its one layer, or Multi for a drilled or multi-layer pad.
pub(super) fn pad_layer<'a>(i18n: &I18n, pad: &PcbNetPad) -> Label<'a> {
    match pad.layers.as_slice() {
        [one] if !pad.is_through => layer_text(i18n, one),
        _ => t!(i18n, ids::PANELS_PCB_NETS_DETAIL_MULTI).into(),
    }
}

/// `value` left, `note` right, in the detail's row look.
fn line<'a>(ctx: &PanelContext, value: Label<'a>, note: Label<'a>) -> Element<'a, PanelMsg> {
    let r = resolve(&ctx.tokens);
    row![
        text(value)
            .size(TS.body)
            .color(ti(ctx.tokens.text))
            .width(Length::Fill),
        text(note).size(TS.caption).color(r.ink3),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center)
    .boxed()
}

fn caption<'a>(ctx: &PanelContext, label: Label<'a>) -> Element<'a, PanelMsg> {
    text(label)
        .size(TS.section)
        .color(resolve(&ctx.tokens).ink3)
        .boxed()
}

/// A length is an engineering value: `.` in every locale.
fn mm<'a>(i18n: &I18n, value: f64) -> Label<'a> {
    t!(i18n, ids::PANELS_PCB_NETS_DETAIL_MM, "length" => format!("{value:.2}")).into()
}

fn head<'a>(ctx: &'a PanelContext, net: &'a PcbNetRow) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let class: Label<'a> = match class_of(board_classes(ctx), &net.name) {
        Some(name) => UserText::new(name.to_owned()).into(),
        None => t!(i18n, ids::PANELS_PCB_NETS_NO_CLASS).into(),
    };
    let length: Label<'a> = if net.length_mm > 0.0 {
        mm(i18n, net.length_mm)
    } else {
        t!(i18n, ids::PANELS_PCB_NETS_NO_LENGTH).into()
    };
    let r = resolve(&ctx.tokens);
    row![
        text(UserText::new(net.name.as_str()))
            .size(TS.body)
            .font(crate::fonts::iced_font_for_family(
                crate::fonts::CHROME_MONO_FONT
            ))
            .color(ti(ctx.tokens.text)),
        Space::new().width(Length::Fill),
        text(class).size(TS.caption).color(r.ink3),
        text(length).size(TS.caption).color(r.ink3),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center)
    .boxed()
}

fn pads<'a>(ctx: &'a PanelContext, net: &'a PcbNetRow) -> Column<Element<'a, PanelMsg>> {
    let i18n = &*ctx.i18n;
    let title = t!(i18n, ids::PANELS_PCB_NETS_DETAIL_PADS, "count" => net.pads.len());
    let mut col: Column<Element<'a, PanelMsg>> = column![caption(ctx, title.into())].spacing(SP.xs);
    for pad in &net.pads {
        let name = t!(
            i18n,
            ids::PANELS_PCB_NETS_DETAIL_PAD,
            "part" => pad.part.as_str(),
            "pad" => pad.pad.as_str()
        );
        col = col.push(line(ctx, name.into(), pad_layer(i18n, pad)));
    }
    col
}

fn tracks<'a>(ctx: &'a PanelContext, net: &'a PcbNetRow) -> Column<Element<'a, PanelMsg>> {
    let i18n = &*ctx.i18n;
    let title = t!(i18n, ids::PANELS_PCB_NETS_DETAIL_TRACKS);
    let mut col: Column<Element<'a, PanelMsg>> = column![caption(ctx, title.into())].spacing(SP.xs);
    for layer in &net.layers {
        let count = t!(i18n, ids::PANELS_PCB_NETS_DETAIL_TRACK_COUNT, "count" => layer.tracks);
        let name = layer_text(i18n, &layer.layer);
        let label = t!(
            i18n,
            ids::PANELS_PCB_NETS_DETAIL_LAYER_TRACKS,
            "layer" => name.as_str(),
            "tracks" => count.into_string()
        );
        col = col.push(line(ctx, label.into(), mm(i18n, layer.length_mm)));
    }
    let vias = t!(i18n, ids::PANELS_PCB_NETS_NODES, "count" => net.vias);
    col.push(line(
        ctx,
        t!(i18n, ids::PANELS_PCB_NETS_DETAIL_VIAS).into(),
        vias.into(),
    ))
}

/// A detail under a list, scrolling on its own.
fn scrolled<'a>(content: Column<Element<'a, PanelMsg>>) -> Element<'a, PanelMsg> {
    scrollable(
        content
            .spacing(SP.m)
            .padding([SP.m, SP.l])
            .width(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(crate::panels::form_look::scrollbar_style)
    .boxed()
}

/// The detail, scrolling on its own under the list.
pub(super) fn view<'a>(ctx: &'a PanelContext, net: &'a PcbNetRow) -> Element<'a, PanelMsg> {
    scrolled(column![head(ctx, net), pads(ctx, net), tracks(ctx, net)])
}

/// A routed length in mm with two decimals, or `—` with nothing routed.
/// An engineering value: `.` in every locale.
pub(super) fn length_value(i18n: &I18n, mm: f64) -> String {
    if mm > 0.0 {
        format!("{mm:.2}")
    } else {
        t!(i18n, ids::PANELS_PCB_NETS_NO_LENGTH).into_string()
    }
}

/// A class rule in mm with three decimals, or `board` while it is unset.
pub(super) fn rule_value(i18n: &I18n, mm: f64) -> String {
    if mm > 0.0 {
        format!("{mm:.3}")
    } else {
        t!(i18n, ids::PANELS_PCB_BOARD_VALUE).into_string()
    }
}

/// The gap and width a pair keeps: its class's pair rules, else `board`.
pub(super) fn pair_rules(i18n: &I18n, class: Option<&NetClass>) -> (String, String) {
    let value = |mm: Option<f64>| rule_value(i18n, mm.unwrap_or(0.0));
    (
        value(class.map(|c| c.diff_pair_gap)),
        value(class.map(|c| c.diff_pair_width)),
    )
}

/// The pair's Gap / Width: its class's two fields, typed in like the class
/// detail's; read-only `board` values with no class or no project table.
fn gap_width<'a>(ctx: &'a PanelContext, class: Option<&NetClass>) -> Element<'a, PanelMsg> {
    use crate::panels::board_net_classes::{self as classes, BoardNetClassField as F};
    let i18n = &*ctx.i18n;
    let label = t!(i18n, ids::PANELS_PCB_PAIRS_DETAIL_GAP_WIDTH);
    let table = ctx
        .net_classes
        .board
        .as_ref()
        .filter(|view| view.in_project);
    let (Some(class), Some(view)) = (class, table) else {
        let (gap, width) = pair_rules(i18n, class);
        let two = t!(i18n, ids::PANELS_PCB_PAIR_OF_VALUES, "first" => gap, "second" => width);
        return line(ctx, label.into(), mm_label(i18n, two.into_string()));
    };
    row![
        text(label)
            .size(TS.body)
            .color(ti(ctx.tokens.text))
            .width(Length::Fill),
        classes::rule_pair(ctx, view, class, [F::DiffPairGap, F::DiffPairWidth]),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center)
    .boxed()
}

/// The selected pair: its two nets, the gap and width it keeps, the routed
/// length of each half and the skew between them.
pub(super) fn pair<'a>(ctx: &'a PanelContext, pair: &'a PcbDiffPairRow) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let class = first_matching_class(board_classes(ctx), &pair.positive);
    let net = |name: &str| -> Label<'a> { UserText::new(name.to_owned()).into() };
    let label = |id: signex_i18n::MessageId| -> Label<'a> { t!(i18n, id).into() };
    let routed = t!(
        i18n,
        ids::PANELS_PCB_PAIRS_DETAIL_ROUTED_VALUE,
        "positive" => length_value(i18n, pair.positive_mm),
        "negative" => length_value(i18n, pair.negative_mm)
    );
    let skew =
        t!(i18n, ids::PANELS_PCB_NETS_DETAIL_MM, "length" => format!("{:.2}", pair.skew_mm()));
    scrolled(column![
        caption(ctx, UserText::new(pair.name.clone()).into()),
        line(
            ctx,
            label(ids::PANELS_PCB_PAIRS_DETAIL_POSITIVE),
            net(&pair.positive)
        ),
        line(
            ctx,
            label(ids::PANELS_PCB_PAIRS_DETAIL_NEGATIVE),
            net(&pair.negative)
        ),
        gap_width(ctx, class),
        line(
            ctx,
            label(ids::PANELS_PCB_PAIRS_DETAIL_ROUTED),
            routed.into()
        ),
        line(ctx, label(ids::PANELS_PCB_PAIRS_DETAIL_SKEW), skew.into()),
    ])
}

/// `value mm`, for a value already formatted.
fn mm_label<'a>(i18n: &I18n, value: String) -> Label<'a> {
    t!(i18n, ids::PANELS_PCB_NETS_DETAIL_MM, "length" => value).into()
}
