//! The By Net list: the heading row, then one row per net; an expanded net
//! lists its members on the selection fill, with a "+ N more" row past the
//! preview.

use iced::widget::{Column, Row, Space, column, container, row, svg};
use iced::{Alignment, Background, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{Localized, MessageId, UserText, ids, t};
use signex_widgets::chips::{ChipMetrics, ChipTone, kind_chip};
use signex_widgets::text;

use super::{
    NetInspectorMsg, NetInspectorView, NetMemberRow, NetMemberRowKind, NetRow, SP, TS, msg,
};
use crate::panels::{PanelContext, PanelMsg};
use crate::styles::{ChromeMetrics, ti, tokens::resolve};

const CM: ChromeMetrics = ChromeMetrics::DEFAULT;
/// Width of the Class column.
pub(super) const CLASS_W: f32 = TS.body * 7.0;
/// Width of the Pins column.
pub(super) const PINS_W: f32 = TS.body * 3.5;

/// The heading row and the nets, or the no-match card.
pub(super) fn list<'a>(ctx: &'a PanelContext, view: &'a NetInspectorView) -> Element<'a, PanelMsg> {
    if view.nets.is_empty() {
        return column![
            super::no_match(
                ctx,
                ids::PANELS_NET_INSPECTOR_EMPTY,
                ids::PANELS_NET_INSPECTOR_EMPTY_BODY,
                view.total,
            ),
            Space::new().width(Length::Fill).height(Length::Fill),
        ]
        .boxed();
    }
    let mut list: Column<Element<'a, PanelMsg>> = column![
        heading(
            ctx,
            &[
                (ids::PANELS_NET_INSPECTOR_COLUMN_NET, Length::Fill),
                (
                    ids::PANELS_NET_INSPECTOR_COLUMN_CLASS,
                    Length::Fixed(CLASS_W)
                ),
                (ids::PANELS_NET_INSPECTOR_COLUMN_PINS, Length::Fixed(PINS_W)),
            ],
        ),
        super::rule(ctx),
    ]
    .width(Length::Fill);
    for net in &view.nets {
        let hovered = view.hovered_net.as_deref() == Some(net.name.as_str());
        list = list.push(net_block(ctx, net, hovered));
    }
    super::scrolled(list.boxed())
}

/// A row of column headings, each `(id, width)`.
pub(super) fn heading<'a>(
    ctx: &'a PanelContext,
    columns: &[(MessageId, Length)],
) -> Element<'a, PanelMsg> {
    let muted = resolve(&ctx.tokens).ink3;
    let mut line: Row<Element<'a, PanelMsg>> = Row::new().spacing(SP.s).padding([0.0, SP.l]);
    for &(id, width) in columns {
        line = line.push(
            container(text(t!(ctx.i18n, id)).size(TS.section).color(muted))
                .width(width)
                .height(CM.row)
                .align_y(iced::alignment::Vertical::Center)
                .boxed(),
        );
    }
    line.boxed()
}

/// A net row and, when expanded, its members: one block on the selection
/// fill so the members read as the net's.
fn net_block<'a>(ctx: &'a PanelContext, net: &'a NetRow, hovered: bool) -> Element<'a, PanelMsg> {
    let mut block: Column<Element<'a, PanelMsg>> = column![net_row(ctx, net, hovered)];
    for member in &net.members {
        block = block.push(member_row(ctx, &net.name, member));
    }
    if net.more > 0 {
        block = block.push(more_row(
            ctx,
            more_members_label(&ctx.i18n, net.more),
            net.name.clone(),
        ));
    }
    expanded_fill(ctx, block.boxed(), net.expanded)
}

/// The overflow row of a net. It counts members (pins, labels, ports and
/// sheet entries), so it does not call them pins: the Pins column counts
/// pins alone (mockup row 58).
fn more_members_label(i18n: &signex_i18n::I18n, count: usize) -> Localized {
    t!(i18n, ids::PANELS_NET_INSPECTOR_MORE_MEMBERS, "count" => count)
}

/// `content` on the selection fill when `expanded`, then a soft rule.
pub(super) fn expanded_fill<'a>(
    ctx: &'a PanelContext,
    content: Element<'a, PanelMsg>,
    expanded: bool,
) -> Element<'a, PanelMsg> {
    let fill = crate::panels::graphite::selection_fill(&ctx.tokens);
    column![
        container(content)
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: expanded.then_some(Background::Color(fill)),
                ..container::Style::default()
            }),
        super::rule(ctx),
    ]
    .boxed()
}

/// The Name cell of a net row: chevron and name, and on hover the row's
/// Highlight on the right, as a board row has it (mockup row 58).
fn name_cell<'a>(ctx: &'a PanelContext, net: &'a NetRow, hovered: bool) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let r = resolve(tokens);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let mut cell: Row<Element<'a, PanelMsg>> = Row::new()
        .push(signex_widgets::chevron::chevron(net.expanded, TS.body, r.ink3).boxed())
        .push(
            text(UserText::new(net.name.as_str()))
                .size(TS.body)
                .font(mono)
                .color(ti(tokens.text))
                .wrapping(iced::widget::text::Wrapping::None)
                .boxed(),
        )
        .spacing(SP.s)
        .align_y(Alignment::Center);
    if hovered {
        let highlight = text(t!(ctx.i18n, ids::PANELS_NET_INSPECTOR_HIGHLIGHT))
            .size(TS.caption)
            .color(r.ink3);
        cell = cell
            .push(
                Space::new()
                    .width(Length::Fill)
                    .height(Length::Shrink)
                    .boxed(),
            )
            .push(
                signex_widgets::button(highlight)
                    .padding([SP.xs, SP.s])
                    .on_press(msg(NetInspectorMsg::NetHighlighted(net.name.clone())))
                    .style(crate::styles::menu_item(tokens))
                    .boxed(),
            );
    }
    container(cell).width(Length::Fill).clip(true).boxed()
}

/// A net: chevron and name, its class as a pill, pin count. Clicking folds
/// or unfolds it; the pointer over it shows its Highlight.
fn net_row<'a>(ctx: &'a PanelContext, net: &'a NetRow, hovered: bool) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let r = resolve(tokens);
    let ink = ti(tokens.text);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let (class, tone): (signex_widgets::Label<'a>, ChipTone) = match &net.class {
        Some(class) => (
            UserText::new(class.as_str()).into(),
            ChipTone {
                fill: r.accent_soft,
                ink: r.accent_soft_ink,
            },
        ),
        None => (
            t!(i18n, ids::PANELS_NET_INSPECTOR_NO_CLASS).into(),
            ChipTone {
                fill: r.sunken,
                ink: r.ink3,
            },
        ),
    };
    let line = row![
        name_cell(ctx, net, hovered),
        container(kind_chip(class, tone, ChipMetrics::from_text_size(TS.body)))
            .width(CLASS_W)
            .clip(true),
        container(
            text(t!(i18n, ids::PANELS_NET_INSPECTOR_PINS, "count" => net.pins))
                .size(TS.body)
                .font(mono)
                .color(ink),
        )
        .width(PINS_W)
        .align_x(iced::alignment::Horizontal::Right),
    ]
    .spacing(SP.s)
    .height(CM.field)
    .align_y(Alignment::Center);
    let row = signex_widgets::button(line)
        .width(Length::Fill)
        .padding([0.0, SP.l])
        .on_press(msg(NetInspectorMsg::NetToggled(net.name.clone())))
        .style(crate::styles::menu_item(tokens));
    iced::widget::mouse_area(row)
        .on_enter(msg(NetInspectorMsg::NetRowEntered(net.name.clone())))
        .on_exit(msg(NetInspectorMsg::NetRowLeft(net.name.clone())))
        .boxed()
}

/// A member of an expanded net: its kind icon (the kind word as tooltip),
/// its text, its sheet on the right. Clicking goes to it.
fn member_row<'a>(
    ctx: &'a PanelContext,
    net: &str,
    member: &'a NetMemberRow,
) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let (ink, muted) = (ti(tokens.text), resolve(tokens).ink3);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let mut what = row![
        text(UserText::new(member.text.as_str()))
            .size(TS.body)
            .font(mono)
            .color(ink)
            .wrapping(iced::widget::text::Wrapping::None),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    if !member.pin_name.is_empty() {
        what = what.push(
            text(UserText::new(member.pin_name.as_str()))
                .size(TS.caption)
                .font(mono)
                .color(muted)
                .wrapping(iced::widget::text::Wrapping::None)
                .boxed(),
        );
    }
    let line = row![
        Space::new().width(SP.xl).height(Length::Shrink),
        kind_icon(ctx, member.kind),
        container(what).width(Length::Fill).clip(true),
        text(UserText::new(member.sheet.as_str()))
            .size(TS.caption)
            .color(muted)
            .wrapping(iced::widget::text::Wrapping::None),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    signex_widgets::button(line)
        .width(Length::Fill)
        .padding([SP.xs, SP.l])
        .on_press(msg(NetInspectorMsg::MemberClicked {
            net: net.to_string(),
            anchor: member.anchor.clone(),
        }))
        .style(crate::styles::menu_item(tokens))
        .boxed()
}

/// The "+ N more" row under a long list; a click lists the rest of `name`.
pub(super) fn more_row<'a>(
    ctx: &'a PanelContext,
    label: Localized,
    name: String,
) -> Element<'a, PanelMsg> {
    let muted = resolve(&ctx.tokens).ink3;
    let line = row![
        Space::new().width(SP.xl).height(Length::Shrink),
        text(label).size(TS.caption).color(muted),
    ]
    .align_y(Alignment::Center);
    signex_widgets::button(line)
        .width(Length::Fill)
        .padding([SP.xs, SP.l])
        .on_press(msg(NetInspectorMsg::MoreMembersShown(name)))
        .style(crate::styles::menu_item(&ctx.tokens))
        .boxed()
}

/// The tint of a member kind's glyph. `None` keeps the asset's colours: the
/// pin and sheet-entry glyphs are multi-colour, and a tint repaints every
/// path. A power port takes the power colour it has on the sheet.
fn kind_ink(kind: NetMemberRowKind, muted: Color, power: Color) -> Option<Color> {
    match kind {
        NetMemberRowKind::Pin | NetMemberRowKind::SheetEntry => None,
        NetMemberRowKind::Label => Some(muted),
        NetMemberRowKind::PowerPort => Some(power),
    }
}

/// The glyph of a member kind, its kind word as the tooltip.
fn kind_icon<'a>(ctx: &'a PanelContext, kind: NetMemberRowKind) -> Element<'a, PanelMsg> {
    let theme = ctx.theme_id;
    let (handle, word): (svg::Handle, MessageId) = match kind {
        NetMemberRowKind::Pin => (
            signex_widgets::tree_view::TreeIcon::Pin.svg(),
            ids::PANELS_NET_INSPECTOR_KIND_PIN,
        ),
        NetMemberRowKind::Label => (
            crate::icons::icon_dd_net_label(theme),
            ids::PANELS_NET_INSPECTOR_KIND_LABEL,
        ),
        NetMemberRowKind::PowerPort => (
            crate::icons::icon_dd_vcc(theme),
            ids::PANELS_NET_INSPECTOR_KIND_PORT,
        ),
        NetMemberRowKind::SheetEntry => (
            crate::icons::icon_dd_sheet_entry(theme),
            ids::PANELS_NET_INSPECTOR_KIND_SHEET_ENTRY,
        ),
    };
    let ink = kind_ink(kind, resolve(&ctx.tokens).ink3, ti(ctx.canvas_colors.power));
    let glyph = svg(handle)
        .width(TS.body)
        .height(TS.body)
        .style(move |_: &Theme, _| svg::Style {
            color: ink,
            ..Default::default()
        });
    crate::panels::graphite::hint_tooltip(
        container(glyph).boxed(),
        t!(ctx.i18n, word),
        &ctx.tokens,
        iced::widget::tooltip::Position::Top,
    )
}

#[cfg(test)]
#[path = "by_net_tests.rs"]
mod hover_tests;

#[cfg(test)]
mod tests {
    use super::*;

    /// The multi-colour glyphs keep their asset's colours; the one-colour
    /// glyphs take their ink.
    #[test]
    fn only_the_one_colour_member_glyphs_are_tinted() {
        let (muted, power) = (Color::BLACK, Color::WHITE);
        let ink = |kind| kind_ink(kind, muted, power);
        assert_eq!(ink(NetMemberRowKind::Pin), None);
        assert_eq!(ink(NetMemberRowKind::SheetEntry), None);
        assert_eq!(ink(NetMemberRowKind::Label), Some(muted));
        assert_eq!(ink(NetMemberRowKind::PowerPort), Some(power));
    }

    fn plain(text: Localized) -> String {
        text.into_string().replace(['\u{2068}', '\u{2069}'], "")
    }

    /// Mockup row 58: a net's overflow row counts members and does not call
    /// them pins, so it cannot contradict the Pins column.
    #[test]
    fn a_nets_overflow_row_counts_members_not_pins() {
        let i18n = signex_i18n::I18n::en_us();
        assert_eq!(plain(more_members_label(&i18n, 1)), "+ 1 more");
        assert_eq!(plain(more_members_label(&i18n, 3)), "+ 3 more");
    }

    /// A part's overflow row does count pins, in the singular for one.
    #[test]
    fn a_parts_overflow_row_handles_the_singular() {
        let i18n = signex_i18n::I18n::en_us();
        let more =
            |count: usize| plain(t!(i18n, ids::PANELS_NET_INSPECTOR_MORE_PINS, "count" => count));
        assert_eq!(more(1), "+ 1 more pin");
        assert_eq!(more(8), "+ 8 more pins");
    }
}
